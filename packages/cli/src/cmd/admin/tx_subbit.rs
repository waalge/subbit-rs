use anyhow::Context;
use cardano_sdk::{Address, Credential, Hash, NetworkId, address::kind};
use serde::Deserialize;
use std::{collections::BTreeMap, path::PathBuf};
use subbit_core::{Constants, Currency, Duration, Hash28, Iou, Tag, VerifyingKey};
use subbit_tx::{NetworkParameters, VALIDATOR, step::Want};

use crate::wallet::WalletEnv;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Debug, clap::Args)]
pub struct Cmd {
    #[clap(flatten)]
    wallet: WalletEnv,

    /// Where the reference script will live (defaults to signer's own address)
    #[clap(long, env = crate::meta::SCRIPT_HOST)]
    script_host: Address<kind::Shelley>,

    /// Channel action as key=value pairs.
    ///
    /// Format: iou_key=<hex>,tag=<hex>,action=<Action>[,params...]
    ///    or:  tx_hash=<hex>,index=<n>,action=<Action>[,params...]
    ///
    /// Actions:
    ///   Open    - currency,provider,close_period,amount[,delegation]
    ///   Add     - amount
    ///   Sub     - iou (hex-encoded cbor)
    ///   Close   - upper
    ///   Settle  - iou (hex-encoded cbor)
    ///   End     - (no params)
    ///   Elapse  - (no params)
    ///
    /// Examples:
    ///   -x "iou_key=aa...,tag=bb,action=Add,amount=5000000"
    ///   -x "tx_hash=cc...,index=0,action=Close,upper=1000"
    #[clap(short = 'x', long = "entry", value_parser = parse_entry_kv, verbatim_doc_comment)]
    pub entries: Vec<Entry>,

    /// Read entries from a JSON file
    #[clap(short = 'f', long = "file")]
    pub file: Option<PathBuf>,
}

impl Cmd {
    pub(crate) async fn run(self) -> anyhow::Result<()> {
        let wallet = self.wallet.into_config()?.build();

        let mut opens = Vec::new();
        let mut step_entries = Vec::new();

        for entry in self.entries {
            match entry.action {
                Action::Open(open) => {
                    opens.push((entry.channel, open));
                }
                Action::Want(want) => {
                    step_entries.push((entry.channel, want));
                }
            }
        }

        let script_host = self.script_host;
        let host_utxos = wallet
            .utxos_at(script_host.payment(), script_host.delegation())
            .await?;
        // FIXME :: support no script when not required
        let Some(script_utxo) = host_utxos.into_iter().find(|x| {
            x.1.script()
                .map(|x| Hash::<28>::from(x) == VALIDATOR.hash)
                .is_some()
        }) else {
            return Err(anyhow::anyhow!("No script found matching hash"));
        };

        let resolved_opens: Vec<subbit_tx::tx::Open> = opens
            .into_iter()
            .map(|(ref_, open)| match ref_ {
                ChannelRef::KeyTag(iou_key, tag) => {
                    let constants = Constants::new(
                        tag,
                        open.currency,
                        iou_key,
                        open.consumer,
                        open.provider,
                        open.close_period,
                    );
                    let x = subbit_tx::tx::Open::new(open.delegation, constants, open.amount);
                    Ok(x)
                }
                ChannelRef::UtxoRef(_, _) => {
                    Err(anyhow::anyhow!("Open requires iou_key + tag, not utxo ref"))
                }
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let fuel: BTreeMap<_, _> = wallet
            .utxos()
            .await?
            .into_iter()
            .filter(|x| x.1.script().is_none())
            .collect();

        let network = wallet.network();
        let network_id = NetworkId::from(network);
        let protocol_parameters = wallet.protocol_parameters().await?;
        let network_parameters = NetworkParameters {
            network_id,
            protocol_parameters,
        };
        let mut tx = subbit_tx::tx::tx(
            &network_parameters,
            Some(&script_utxo),
            wallet.address().into(),
            &BTreeMap::new(),
            subbit_tx::step::Tx::default(),
            resolved_opens,
            &fuel,
        )?;

        tx.sign_with(|hash| {
            let sig = wallet.sign(hash.as_ref());
            (wallet.verification_key(), sig)
        });
        wallet.submit(&tx).await?;
        Ok(())
    }

    pub fn all_entries(&self) -> anyhow::Result<Vec<Entry>> {
        let mut entries = self.entries.clone();
        if let Some(path) = &self.file {
            let json = std::fs::read_to_string(path)?;
            let raws: Vec<RawEntry> = serde_json::from_str(&json)?;
            entries.extend(
                raws.into_iter()
                    .map(TryInto::try_into)
                    .collect::<anyhow::Result<Vec<_>>>()?,
            );
        }
        Ok(entries)
    }
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Entry {
    pub channel: ChannelRef,
    pub action: Action,
}

#[derive(Debug, Clone)]
pub enum ChannelRef {
    KeyTag(VerifyingKey, Tag),
    UtxoRef(Hash<32>, u32),
}

#[derive(Debug, Clone)]
pub enum Action {
    Open(Open),
    Want(Want),
}

#[derive(Debug, Clone)]
pub struct Open {
    pub currency: Currency,
    pub consumer: Hash28,
    pub provider: Hash28,
    pub close_period: Duration,
    pub amount: u64,
    pub delegation: Option<Credential>,
}

// ---------------------------------------------------------------------------
// Raw intermediate (flat, all optional)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RawEntry {
    iou_key: Option<String>,
    tag: Option<String>,
    tx_hash: Option<String>,
    index: Option<u32>,

    action: String,

    amount: Option<u64>,
    iou: Option<String>,
    // Relative time: eg 20m, 3600s
    relative_upper: Option<String>,
    currency: Option<String>,
    consumer: Option<String>,
    provider: Option<String>,
    close_period: Option<u64>,
    delegation: Option<String>,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

fn parse_entry_kv(s: &str) -> anyhow::Result<Entry> {
    let raw: RawEntry = serde_urlencoded::from_str(&s.replace(',', "&"))
        .map_err(|e| anyhow::anyhow!("failed to parse key=value pairs: {e}\n  input: {s}"))?;

    if raw.action.is_empty() {
        return Err(anyhow::anyhow!(
            "missing 'action' field. Expected one of: Open, Add, Sub, Close, Settle, End, Elapse\n  input: {s}"
        ));
    }

    raw.try_into()
}

impl TryFrom<RawEntry> for Entry {
    type Error = anyhow::Error;

    fn try_from(raw: RawEntry) -> anyhow::Result<Self> {
        let channel = match (raw.iou_key, raw.tag, raw.tx_hash, raw.index) {
            (Some(k), Some(t), None, None) => ChannelRef::KeyTag(k.parse()?, t.parse()?),
            (None, None, Some(h), Some(i)) => ChannelRef::UtxoRef(hex::decode(h)?.try_into()?, i),
            _ => {
                return Err(anyhow::anyhow!(
                    "provide (iou_key + tag) or (tx_hash + index)"
                ));
            }
        };

        let action = match raw.action.as_str() {
            "Open" => {
                let currency = raw
                    .currency
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("Open: missing currency"))?;
                let consumer = raw
                    .consumer
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("Open: missing consumer"))?;
                let provider = raw
                    .provider
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("Open: missing provider"))?;
                let close_period = raw
                    .close_period
                    .ok_or_else(|| anyhow::anyhow!("Open: missing close_period"))?;
                let amount = raw
                    .amount
                    .ok_or_else(|| anyhow::anyhow!("Open: missing amount"))?;
                let delegation = raw
                    .delegation
                    .as_deref()
                    .map(parse_delegation)
                    .transpose()?;

                Action::Open(Open {
                    currency: parse_currency(currency)?,
                    consumer: consumer.parse().context("consumer")?,
                    provider: provider.parse().context("provider")?,
                    close_period: Duration::from_millis(close_period),
                    amount,
                    delegation,
                })
            }
            "Add" => Action::Want(Want::Add {
                amount: raw
                    .amount
                    .ok_or_else(|| anyhow::anyhow!("Add: missing amount"))?,
            }),
            "Sub" => Action::Want(Want::Sub {
                iou: parse_iou(
                    raw.iou
                        .as_deref()
                        .ok_or_else(|| anyhow::anyhow!("Sub: missing iou"))?,
                )?,
            }),
            "Close" => Action::Want(Want::Close {
                upper: raw
                    .relative_upper
                    .ok_or_else(|| anyhow::anyhow!("Close: missing upper"))?
                    .parse::<Duration>()?,
            }),
            "Settle" => Action::Want(Want::Settle {
                iou: parse_iou(
                    raw.iou
                        .as_deref()
                        .ok_or_else(|| anyhow::anyhow!("Settle: missing iou"))?,
                )?,
            }),
            "End" => Action::Want(Want::End),
            "Elapse" => Action::Want(Want::Elapse),
            other => return Err(anyhow::anyhow!("unknown action: {other}")),
        };

        Ok(Entry { channel, action })
    }
}

fn parse_iou(s: &str) -> anyhow::Result<Iou> {
    let bytes = hex::decode(s).context("iou: invalid hex")?;
    minicbor::decode(&bytes).context("iou: invalid cbor")
}

fn parse_currency(s: &str) -> anyhow::Result<Currency> {
    match s {
        "ADA" => Ok(Currency::Ada),
        s if s.starts_with("BY_HASH:") => {
            let hash: [u8; 28] = hex::decode(&s[8..])
                .context("BY_HASH: invalid hex")?
                .try_into()
                .map_err(|v: Vec<u8>| {
                    anyhow::anyhow!("BY_HASH: expected 28 bytes, got {}", v.len())
                })?;
            Ok(Currency::ByHash { hash })
        }
        s if s.starts_with("BY_CLASS:") => {
            let bytes = hex::decode(&s[9..]).context("BY_CLASS: invalid hex")?;
            if bytes.len() < 28 || bytes.len() > 60 {
                return Err(anyhow::anyhow!(
                    "BY_CLASS: expected 28-60 bytes, got {}",
                    bytes.len()
                ));
            }
            let hash: [u8; 28] = bytes[..28].try_into().unwrap();
            let name = bytes[28..].to_vec();
            Ok(Currency::ByClass { hash, name })
        }
        _ => Err(anyhow::anyhow!(
            "expected ADA, BY_HASH:<hex>, or BY_CLASS:<hex>"
        )),
    }
}

fn parse_delegation(s: &str) -> anyhow::Result<Credential> {
    if let Some(hex_str) = s.strip_prefix("KEY:") {
        let bytes: [u8; 28] = hex::decode(hex_str)
            .context("KEY: invalid hex")?
            .try_into()
            .map_err(|v: Vec<u8>| anyhow::anyhow!("KEY: expected 28 bytes, got {}", v.len()))?;
        Ok(Credential::from_key(Hash::<28>::from(bytes)))
    } else if let Some(hex_str) = s.strip_prefix("SCRIPT:") {
        let bytes: [u8; 28] = hex::decode(hex_str)
            .context("SCRIPT: invalid hex")?
            .try_into()
            .map_err(|v: Vec<u8>| anyhow::anyhow!("SCRIPT: expected 28 bytes, got {}", v.len()))?;
        Ok(Credential::from_script(Hash::<28>::from(bytes)))
    } else {
        Err(anyhow::anyhow!("expected KEY:<hex> or SCRIPT:<hex>"))
    }
}

#[cfg(test)]
mod tests {
    use subbit_core::Signature;

    use super::*;

    // -x "iou_key=...,tag=...,action=Add,amount=5000000"
    #[test]
    fn parse_add() {
        let entry = parse_entry_kv(
            "iou_key=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\
             tag=bbbbbbbb,\
             action=Add,\
             amount=5000000",
        )
        .unwrap();
        assert!(matches!(
            entry.action,
            Action::Want(Want::Add { amount: 5000000 })
        ));
        assert!(matches!(entry.channel, ChannelRef::KeyTag(_, _)));
    }

    // -x "iou_key=...,tag=...,action=Sub,iou=d818..."
    #[test]
    fn parse_sub() {
        let iou_hex = hex::encode(minicbor::to_vec(&test_iou()).unwrap());
        let entry = parse_entry_kv(&format!(
            "iou_key=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\
             tag=bbbbbbbb,\
             action=Sub,\
             iou={iou_hex}"
        ))
        .unwrap();
        assert!(matches!(entry.action, Action::Want(Want::Sub { .. })));
    }

    // -x "iou_key=...,tag=...,action=Close,upper=1000s"
    #[test]
    fn parse_close() {
        let entry = parse_entry_kv(
            "iou_key=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\
             tag=bbbbbbbb,\
             action=Close,\
             relative_upper=1000s",
        )
        .unwrap();
        assert!(matches!(entry.action, Action::Want(Want::Close { .. })));
    }

    // -x "iou_key=...,tag=...,action=Settle,iou=d818..."
    #[test]
    fn parse_settle() {
        let iou_hex = hex::encode(minicbor::to_vec(&test_iou()).unwrap());
        let entry = parse_entry_kv(&format!(
            "iou_key=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\
             tag=bbbbbbbb,\
             action=Settle,\
             iou={iou_hex}"
        ))
        .unwrap();
        assert!(matches!(entry.action, Action::Want(Want::Settle { .. })));
    }

    // -x "iou_key=...,tag=...,action=End"
    #[test]
    fn parse_end() {
        let entry = parse_entry_kv(
            "iou_key=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\
             tag=bbbbbbbb,\
             action=End",
        )
        .unwrap();
        assert!(matches!(entry.action, Action::Want(Want::End)));
    }

    // -x "iou_key=...,tag=...,action=Elapse"
    #[test]
    fn parse_elapse() {
        let entry = parse_entry_kv(
            "iou_key=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\
             tag=bbbbbbbb,\
             action=Elapse",
        )
        .unwrap();
        assert!(matches!(entry.action, Action::Want(Want::Elapse)));
    }

    // -x "tx_hash=...,index=0,action=Add,amount=5000000"
    #[test]
    fn parse_utxo_ref_identifier() {
        let entry = parse_entry_kv(
            "tx_hash=cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc,\
             index=0,\
             action=Add,\
             amount=5000000",
        )
        .unwrap();
        assert!(matches!(entry.channel, ChannelRef::UtxoRef(_, 0)));
    }

    // -x "iou_key=...,tag=...,action=Open,currency=ADA,provider=...,close_period=3600,amount=5000000"
    #[test]
    fn parse_open_ada_no_delegation() {
        let entry = parse_entry_kv(
            "iou_key=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\
             tag=bbbbbbbb,\
             action=Open,\
             currency=ADA,\
             consumer=dddddddddddddddddddddddddddddddddddddddddddddddddddddddd,\
             provider=cccccccccccccccccccccccccccccccccccccccccccccccccccccccc,\
             close_period=3600,\
             amount=5000000",
        )
        .unwrap();
        match entry.action {
            Action::Open(open) => {
                assert!(matches!(open.currency, Currency::Ada));
                assert!(open.delegation.is_none());
                assert_eq!(open.amount, 5000000);
            }
            _ => panic!("expected Open"),
        }
    }

    // -x "...,action=Open,...,delegation=KEY:..."
    #[test]
    fn parse_open_with_key_delegation() {
        let entry = parse_entry_kv(
            "iou_key=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\
             tag=bbbbbbbb,\
             action=Open,\
             currency=ADA,\
             consumer=dddddddddddddddddddddddddddddddddddddddddddddddddddddddd,\
             provider=cccccccccccccccccccccccccccccccccccccccccccccccccccccccc,\
             close_period=3600,\
             amount=5000000,\
             delegation=KEY:dddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        )
        .unwrap();
        match entry.action {
            Action::Open(open) => assert!(open.delegation.map(|x| x.as_key()).is_some()),
            _ => panic!("expected Open"),
        }
    }

    // -x "...,action=Open,...,currency=BY_HASH:..."
    #[test]
    fn parse_open_with_native_currency() {
        let entry = parse_entry_kv(
            "iou_key=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\
             tag=bbbbbbbb,\
             action=Open,\
             currency=BY_HASH:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee,\
             consumer=dddddddddddddddddddddddddddddddddddddddddddddddddddddddd,\
             provider=cccccccccccccccccccccccccccccccccccccccccccccccccccccccc,\
             close_period=3600,\
             amount=5000000",
        )
        .unwrap();
        match entry.action {
            Action::Open(open) => assert!(matches!(open.currency, Currency::ByHash { .. })),
            _ => panic!("expected Open"),
        }
    }

    // Error: missing required field
    #[test]
    fn parse_add_missing_amount() {
        let result = parse_entry_kv(
            "iou_key=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\
             tag=bbbbbbbb,\
             action=Add",
        );
        assert!(result.is_err());
    }

    // Error: unknown action
    #[test]
    fn parse_unknown_action() {
        let result = parse_entry_kv(
            "iou_key=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\
             tag=bbbbbbbb,\
             action=Explode",
        );
        assert!(result.is_err());
    }

    // Error: ambiguous identifier
    #[test]
    fn parse_ambiguous_identifier() {
        let result = parse_entry_kv(
            "iou_key=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\
             tag=bbbbbbbb,\
             tx_hash=cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc,\
             index=0,\
             action=Add,\
             amount=5000000",
        );
        assert!(result.is_err());
    }

    // JSON: same structure, from file content
    #[test]
    fn parse_json_batch() {
        let json = r#"[
            {
                "iou_key": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "tag": "bbbbbbbb",
                "action": "Add",
                "amount": 5000000
            },
            {
                "iou_key": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "tag": "bbbbbbbb",
                "action": "End"
            }
        ]"#;
        let raws: Vec<RawEntry> = serde_json::from_str(json).unwrap();
        let entries: Vec<Entry> = raws.into_iter().map(|r| r.try_into().unwrap()).collect();
        assert_eq!(entries.len(), 2);
        assert!(matches!(
            entries[0].action,
            Action::Want(Want::Add { amount: 5000000 })
        ));
        assert!(matches!(entries[1].action, Action::Want(Want::End)));
    }

    // TODO: replace with actual test IOU construction
    fn test_iou() -> Iou {
        Iou::new(10000, Signature::from([0u8; 64]))
    }
}
