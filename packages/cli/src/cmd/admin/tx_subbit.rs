//! Subbit channel transaction CLI.
//!
//! Two entry points are provided:
//!
//! - [`Cmd`] — full control: compose any combination of channel actions in one
//!   transaction using `-x` entries or a JSON file.
//! - [`SimpleCmd`] — simplified interface for end-users: `open` and `close` only;
//!   `elapse` and `end` are derived automatically from on-chain state.

use anyhow::Context;
use cardano_sdk::{Address, Credential, Hash, NetworkId, address::kind};
use serde::Deserialize;
use std::{collections::BTreeMap, path::PathBuf};
use subbit_core::{Constants, Currency, Duration, Hash28, Iou, Tag, VerifyingKey};
use subbit_tx::{NetworkParameters, VALIDATOR, step::Want};

use crate::wallet::WalletEnv;

// ---------------------------------------------------------------------------
// Full CLI (Cmd)
// ---------------------------------------------------------------------------

/// Build and submit a Subbit channel transaction.
///
/// Entries describe one channel action each. Multiple `-x` flags (or a JSON
/// file via `-f`) are batched into a single on-chain transaction.
///
/// # Channel identifier
///
/// Every entry must identify its channel by exactly one of:
///
///   iou_key=<hex64>,tag=<hex8>     — preferred; stable across UTxO moves
///   tx_hash=<hex64>,index=<n>      — use when you only know the UTxO
///
/// # Actions
///
///   Open    — open a new channel
///   Add     — top up an existing channel
///   Sub     — redeem an IOU against a channel
///   Close   — begin the close window
///   Settle  — settle with a final IOU after close
///   End     — collect funds after the close window has passed
///   Elapse  — advance the protocol clock (no-op if nothing to do)
///
/// # Examples
///
/// Open a new ADA channel:
///   -x "iou_key=aa..,tag=bb..,action=Open,currency=ADA,provider=cc..,consumer=dd..,close_period=3600,amount=5000000"
///
/// Top up a channel:
///   -x "iou_key=aa..,tag=bb..,action=Add,amount=2000000"
///
/// Redeem an IOU:
///   -x "iou_key=aa..,tag=bb..,action=Sub,iou=<cbor-hex>"
///
/// Begin closing:
///   -x "iou_key=aa..,tag=bb..,action=Close,relative_upper=1h"
///
/// Settle after close:
///   -x "iou_key=aa..,tag=bb..,action=Settle,iou=<cbor-hex>"
///
/// Collect after close window:
///   -x "iou_key=aa..,tag=bb..,action=End"
///
/// Batch from JSON file (same fields as above, as a JSON array):
///   -f ./entries.json
#[derive(Debug, clap::Args)]
pub struct Cmd {
    #[clap(flatten)]
    wallet: WalletEnv,

    /// Address holding the reference script UTxO.
    ///
    /// Defaults to the signer's own address. Override when the script is
    /// hosted at a shared address (e.g. a protocol-operated script host).
    #[clap(long, env = crate::meta::SCRIPT_HOST)]
    script_host: Address<kind::Shelley>,

    /// A channel action as comma-separated key=value pairs.
    ///
    /// May be repeated. All entries are batched into one transaction.
    /// See the command-level docs for the full format.
    #[clap(
        short = 'x',
        long = "entry",
        value_name = "KEY=VAL,...",
        value_parser = parse_entry_kv,
        verbatim_doc_comment,
    )]
    pub entries: Vec<Entry>,

    /// Path to a JSON file containing a batch of entries.
    ///
    /// The file must be a JSON array of objects using the same field names as
    /// the `-x` format. Merged with any `-x` entries before submission.
    #[clap(short = 'f', long = "file", value_name = "PATH")]
    pub file: Option<PathBuf>,
}

impl Cmd {
    pub(crate) async fn run(self) -> anyhow::Result<()> {
        let wallet = self.wallet.into_config()?.build();

        let mut opens = Vec::new();
        let mut step_entries = Vec::new();

        for entry in self.entries {
            match entry.action {
                Action::Open(open) => opens.push((entry.channel, open)),
                Action::Want(want) => step_entries.push((entry.channel, want)),
            }
        }

        let script_host = self.script_host;
        let host_utxos = wallet
            .utxos_at(script_host.payment(), script_host.delegation())
            .await?;

        // FIXME: support transactions that don't require the reference script
        let Some(script_utxo) = host_utxos.into_iter().find(|x| {
            x.1.script()
                .map(|s| Hash::<28>::from(s) == VALIDATOR.hash)
                .is_some()
        }) else {
            return Err(anyhow::anyhow!(
                "no UTxO found at {} carrying the validator script (hash: {})\n\
                 hint: check --script-host or SCRIPT_HOST and ensure the script is deployed",
                script_host,
                VALIDATOR.hash,
            ));
        };

        let resolved_opens = opens
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
                    Ok(subbit_tx::tx::Open::new(
                        open.delegation,
                        constants,
                        open.amount,
                    ))
                }
                ChannelRef::UtxoRef(_, _) => Err(anyhow::anyhow!(
                    "Open requires iou_key + tag; UTxO ref is only valid for existing channels"
                )),
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let fuel: BTreeMap<_, _> = wallet
            .utxos()
            .await?
            .into_iter()
            .filter(|x| x.1.script().is_none())
            .collect();

        let network_parameters = NetworkParameters {
            network_id: NetworkId::from(wallet.network()),
            protocol_parameters: wallet.protocol_parameters().await?,
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

        tx.sign_with(|hash| (wallet.verification_key(), wallet.sign(hash.as_ref())));
        wallet.submit(&tx).await?;
        Ok(())
    }

    /// Merge `-x` entries with any entries loaded from `-f`.
    pub fn all_entries(&self) -> anyhow::Result<Vec<Entry>> {
        let mut entries = self.entries.clone();
        if let Some(path) = &self.file {
            let json = std::fs::read_to_string(path)
                .with_context(|| format!("could not read entries file: {}", path.display()))?;
            let raws: Vec<RawEntry> =
                serde_json::from_str(&json).context("entries file is not a valid JSON array")?;
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
// Simple CLI (SimpleCmd)
// ---------------------------------------------------------------------------

/// Open or close a Subbit channel, with automatic housekeeping.
///
/// This is a streamlined interface that covers the common user workflow.
/// `Elapse` and `End` steps are derived automatically from on-chain state
/// and do not need to be requested explicitly.
///
/// # Examples
///
/// Open a new channel:
///   subbit simple open --iou-key aa.. --tag bb.. --provider cc.. --amount 5000000
///
/// Close your channel:
///   subbit simple close --iou-key aa.. --tag bb..
#[derive(Debug, clap::Args)]
pub struct SimpleCmd {
    #[clap(flatten)]
    wallet: WalletEnv,

    /// Address holding the reference script UTxO.
    #[clap(long, env = crate::meta::SCRIPT_HOST)]
    script_host: Address<kind::Shelley>,

    #[clap(subcommand)]
    action: SimpleAction,
}

#[derive(Debug, clap::Subcommand)]
enum SimpleAction {
    /// Open a new payment channel.
    Open(SimpleOpen),
    /// Begin closing your payment channel.
    ///
    /// Also automatically submits any pending Elapse steps first.
    Close(SimpleClose),
}

/// Open a new payment channel.
#[derive(Debug, clap::Args)]
struct SimpleOpen {
    /// Your IOU key (hex-encoded 32-byte verifying key).
    #[clap(long, value_name = "HEX")]
    iou_key: VerifyingKey,

    /// Channel tag (hex-encoded 4 bytes).
    #[clap(long, value_name = "HEX")]
    tag: Tag,

    /// Provider's payment credential hash (hex-encoded 28 bytes).
    #[clap(long, value_name = "HEX")]
    provider: Hash28,

    /// Initial deposit in lovelace.
    #[clap(long, value_name = "LOVELACE")]
    amount: u64,

    /// Currency. One of: ADA, BY_HASH:<hex28>, BY_CLASS:<hex28+name>
    /// Defaults to ADA.
    #[clap(long, value_name = "CURRENCY", default_value = "ADA", value_parser = parse_currency)]
    currency: Currency,

    /// Close window duration in seconds.
    #[clap(long, value_name = "SECONDS", default_value = "1h")]
    close_period: String,

    /// Optional delegation credential. Format: KEY:<hex28> or SCRIPT:<hex28>
    #[clap(long, value_name = "KEY:<hex>|SCRIPT:<hex>", value_parser = parse_delegation)]
    delegation: Option<Credential>,
}

/// Close an existing payment channel.
#[derive(Debug, clap::Args)]
struct SimpleClose {
    /// Your IOU key (hex-encoded 32-byte verifying key).
    #[clap(long, value_name = "HEX")]
    iou_key: VerifyingKey,

    /// Channel tag (hex-encoded 4 bytes).
    #[clap(long, value_name = "HEX")]
    tag: Tag,

    /// Upper bound for the close window (relative, e.g. 1h, 30m, 3600s).
    /// Defaults to 1h.
    #[clap(long, value_name = "DURATION", default_value = "1h")]
    upper: Duration,
}

impl SimpleCmd {
    pub(crate) async fn run(self) -> anyhow::Result<()> {
        let entries = match self.action {
            SimpleAction::Open(o) => {
                let channel = ChannelRef::KeyTag(o.iou_key, o.tag.clone());
                vec![Entry {
                    channel,
                    action: Action::Open(Open {
                        currency: o.currency,
                        // consumer defaults to the wallet's own payment credential
                        consumer: todo!("derive consumer from wallet"),
                        provider: o.provider,
                        close_period: o.close_period.parse::<Duration>()?,
                        amount: o.amount,
                        delegation: o.delegation,
                    }),
                }]
            }
            SimpleAction::Close(c) => {
                let channel = ChannelRef::KeyTag(c.iou_key, c.tag);
                // Elapse is injected automatically before Close when needed;
                // the tx builder will skip it if there is nothing to elapse.
                vec![
                    Entry {
                        channel: channel.clone(),
                        action: Action::Want(Want::Elapse),
                    },
                    Entry {
                        channel,
                        action: Action::Want(Want::Close { upper: c.upper }),
                    },
                ]
            }
        };

        // Delegate to Cmd::run by constructing a full Cmd from the derived entries.
        let cmd = Cmd {
            wallet: self.wallet,
            script_host: self.script_host,
            entries,
            file: None,
        };
        cmd.run().await
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
    /// Stable identifier: survives UTxO moves.
    KeyTag(VerifyingKey, Tag),
    /// UTxO-based identifier: use when iou_key/tag are unavailable.
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
// Raw intermediate (flat JSON / kv deserialisation target)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RawEntry {
    // Channel identifier — exactly one pair must be present.
    iou_key: Option<String>,
    tag: Option<String>,
    tx_hash: Option<String>,
    index: Option<u32>,

    action: String,

    // Action-specific fields (all optional at this layer; validated in TryFrom).
    amount: Option<u64>,
    iou: Option<String>,
    /// Relative duration string, e.g. "1h", "30m", "3600s".
    relative_upper: Option<String>,
    currency: Option<String>,
    consumer: Option<String>,
    provider: Option<String>,
    /// Close window in milliseconds.
    close_period: Option<String>,
    /// Delegation credential. Format: KEY:<hex28> or SCRIPT:<hex28>
    delegation: Option<String>,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

fn parse_entry_kv(s: &str) -> anyhow::Result<Entry> {
    eprintln!("DEBUG raw input: {s:?}");
    let rewritten = s.replace(',', "&");
    eprintln!("DEBUG rewritten: {rewritten:?}");
    let raw: RawEntry = serde_urlencoded::from_str(&rewritten)
        .with_context(|| format!("failed to parse key=value pairs\n  input: {s}"))?;
    eprintln!("DEBUG parsed action: {:?}", raw.action);
    eprintln!("DEBUG parsed close_period: {:?}", raw.close_period);
    let raw: RawEntry = serde_urlencoded::from_str(&s.replace(',', "&"))
        .with_context(|| format!("failed to parse key=value pairs\n  input: {s}"))?;

    if raw.action.is_empty() {
        return Err(anyhow::anyhow!(
            "missing 'action' field\n\
             expected one of: Open, Add, Sub, Close, Settle, End, Elapse\n\
             input: {s}"
        ));
    }

    raw.try_into()
}

impl TryFrom<RawEntry> for Entry {
    type Error = anyhow::Error;

    fn try_from(raw: RawEntry) -> anyhow::Result<Self> {
        let channel = match (raw.iou_key, raw.tag, raw.tx_hash, raw.index) {
            (Some(k), Some(t), None, None) => ChannelRef::KeyTag(
                k.parse().context("iou_key: invalid verifying key hex")?,
                t.parse().context("tag: invalid tag hex")?,
            ),
            (None, None, Some(h), Some(i)) => {
                let bytes: [u8; 32] = hex::decode(&h)
                    .context("tx_hash: invalid hex")?
                    .try_into()
                    .map_err(|v: Vec<u8>| {
                        anyhow::anyhow!("tx_hash: expected 32 bytes, got {}", v.len())
                    })?;
                ChannelRef::UtxoRef(bytes.into(), i)
            }
            (None, None, Some(_), None) => {
                return Err(anyhow::anyhow!(
                    "tx_hash provided without index\n\
                 hint: add index=<n>"
                ));
            }
            (Some(_), None, None, None) => {
                return Err(anyhow::anyhow!(
                    "iou_key provided without tag\n\
                 hint: add tag=<hex>"
                ));
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "ambiguous channel identifier\n\
                 provide either (iou_key + tag) or (tx_hash + index), not both"
                ));
            }
        };

        let action = match raw.action.as_str() {
            "Open" => Action::Open(Open {
                currency: parse_currency(
                    raw.currency.as_deref()
                        .ok_or_else(|| anyhow::anyhow!("Open: missing currency\n\
                            hint: add currency=ADA (or BY_HASH:<hex28> or BY_CLASS:<hex28+name>)"))?,
                )?,
                consumer: raw.consumer.as_deref()
                    .ok_or_else(|| anyhow::anyhow!("Open: missing consumer\n\
                        hint: add consumer=<hex28> (your payment credential hash)"))?
                    .parse().context("Open: consumer")?,
                provider: raw.provider.as_deref()
                    .ok_or_else(|| anyhow::anyhow!("Open: missing provider\n\
                        hint: add provider=<hex28> (provider's payment credential hash)"))?
                    .parse().context("Open: provider")?,
                close_period: raw.close_period.as_deref()
                    .ok_or_else(|| anyhow::anyhow!("Open: missing close_period\n\
                        hint: add close_period=<duration>, e.g. close_period=1h or close_period=3600s"))?
                    .parse::<Duration>()
                    .context("Open: close_period")?
                ,
                amount: raw.amount
                    .ok_or_else(|| anyhow::anyhow!("Open: missing amount\n\
                        hint: add amount=<lovelace>"))?,
                delegation: raw.delegation.as_deref().map(parse_delegation).transpose()?,
            }),

            "Add" => Action::Want(Want::Add {
                amount: raw.amount
                    .ok_or_else(|| anyhow::anyhow!("Add: missing amount\n\
                        hint: add amount=<lovelace>"))?,
            }),

            "Sub" => Action::Want(Want::Sub {
                iou: parse_iou(
                    raw.iou.as_deref()
                        .ok_or_else(|| anyhow::anyhow!("Sub: missing iou\n\
                            hint: add iou=<cbor-hex> (hex-encoded CBOR of the signed IOU)"))?,
                )?,
            }),

            "Close" => Action::Want(Want::Close {
                upper: raw.relative_upper
                    .ok_or_else(|| anyhow::anyhow!("Close: missing relative_upper\n\
                        hint: add relative_upper=<duration>, e.g. relative_upper=1h or relative_upper=3600s"))?
                    .parse::<Duration>()
                    .context("Close: relative_upper")?,
            }),

            "Settle" => Action::Want(Want::Settle {
                iou: parse_iou(
                    raw.iou.as_deref()
                        .ok_or_else(|| anyhow::anyhow!("Settle: missing iou\n\
                            hint: add iou=<cbor-hex> (hex-encoded CBOR of the final signed IOU)"))?,
                )?,
            }),

            "End"    => Action::Want(Want::End),
            "Elapse" => Action::Want(Want::Elapse),

            other => return Err(anyhow::anyhow!(
                "unknown action: {other:?}\n\
                 expected one of: Open, Add, Sub, Close, Settle, End, Elapse"
            )),
        };

        Ok(Entry { channel, action })
    }
}

fn parse_iou(s: &str) -> anyhow::Result<Iou> {
    let bytes = hex::decode(s).context("iou: invalid hex")?;
    minicbor::decode(&bytes).context("iou: invalid CBOR (expected a signed IOU)")
}

/// Parse a currency specifier.
///
/// Accepted formats:
///   ADA
///   BY_HASH:<hex28>             — native token by policy ID
///   BY_CLASS:<hex28><name-hex>  — native token by policy ID + asset name
fn parse_currency(s: &str) -> anyhow::Result<Currency> {
    match s {
        "ADA" => Ok(Currency::Ada),
        s if s.starts_with("BY_HASH:") => {
            let hash: [u8; 28] = hex::decode(&s[8..])
                .context("BY_HASH: invalid hex")?
                .try_into()
                .map_err(|v: Vec<u8>| {
                    anyhow::anyhow!("BY_HASH: expected 28 bytes (policy ID), got {}", v.len())
                })?;
            Ok(Currency::ByHash { hash })
        }
        s if s.starts_with("BY_CLASS:") => {
            let bytes = hex::decode(&s[9..]).context("BY_CLASS: invalid hex")?;
            if bytes.len() < 28 || bytes.len() > 60 {
                return Err(anyhow::anyhow!(
                    "BY_CLASS: expected 28–60 bytes (28-byte policy ID + up to 32-byte name), got {}",
                    bytes.len()
                ));
            }
            let hash: [u8; 28] = bytes[..28].try_into().unwrap();
            let name = bytes[28..].to_vec();
            Ok(Currency::ByClass { hash, name })
        }
        _ => Err(anyhow::anyhow!(
            "unrecognised currency: {s:?}\n\
             expected: ADA, BY_HASH:<hex28>, or BY_CLASS:<hex28+name>"
        )),
    }
}

/// Parse a delegation credential.
///
/// Accepted formats:
///   KEY:<hex28>     — key credential (payment key hash)
///   SCRIPT:<hex28>  — script credential (script hash)
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
        Err(anyhow::anyhow!(
            "unrecognised delegation format: {s:?}\n\
             expected: KEY:<hex28> or SCRIPT:<hex28>"
        ))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use subbit_core::Signature;

    const IOU_KEY: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const TAG: &str = "bbbbbbbb";
    const HASH28: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    const HASH32: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    fn kv(s: &str) -> Entry {
        parse_entry_kv(s).unwrap()
    }
    fn kv_err(s: &str) -> anyhow::Error {
        parse_entry_kv(s).unwrap_err()
    }
    fn test_iou() -> Iou {
        Iou::new(10000, Signature::from([0u8; 64]))
    }
    fn iou_hex() -> String {
        hex::encode(minicbor::to_vec(&test_iou()).unwrap())
    }

    // ── happy paths ──────────────────────────────────────────────────────────

    #[test]
    fn parse_add() {
        let e = kv(&format!(
            "iou_key={IOU_KEY},tag={TAG},action=Add,amount=5000000"
        ));
        assert!(matches!(
            e.action,
            Action::Want(Want::Add { amount: 5000000 })
        ));
        assert!(matches!(e.channel, ChannelRef::KeyTag(_, _)));
    }

    #[test]
    fn parse_sub() {
        let e = kv(&format!(
            "iou_key={IOU_KEY},tag={TAG},action=Sub,iou={}",
            iou_hex()
        ));
        assert!(matches!(e.action, Action::Want(Want::Sub { .. })));
    }

    #[test]
    fn parse_close() {
        let e = kv(&format!(
            "iou_key={IOU_KEY},tag={TAG},action=Close,relative_upper=1000s"
        ));
        assert!(matches!(e.action, Action::Want(Want::Close { .. })));
    }

    #[test]
    fn parse_settle() {
        let e = kv(&format!(
            "iou_key={IOU_KEY},tag={TAG},action=Settle,iou={}",
            iou_hex()
        ));
        assert!(matches!(e.action, Action::Want(Want::Settle { .. })));
    }

    #[test]
    fn parse_end() {
        let e = kv(&format!("iou_key={IOU_KEY},tag={TAG},action=End"));
        assert!(matches!(e.action, Action::Want(Want::End)));
    }

    #[test]
    fn parse_elapse() {
        let e = kv(&format!("iou_key={IOU_KEY},tag={TAG},action=Elapse"));
        assert!(matches!(e.action, Action::Want(Want::Elapse)));
    }

    #[test]
    fn parse_utxo_ref_identifier() {
        let e = kv(&format!(
            "tx_hash={HASH32},index=0,action=Add,amount=5000000"
        ));
        assert!(matches!(e.channel, ChannelRef::UtxoRef(_, 0)));
    }

    #[test]
    fn parse_open_ada_no_delegation() {
        let e = kv(&format!(
            "iou_key={IOU_KEY},tag={TAG},action=Open,\
             currency=ADA,consumer={HASH28},provider={HASH28},\
             close_period=3600s,amount=5000000"
        ));
        let Action::Open(open) = e.action else {
            panic!("expected Open")
        };
        assert!(matches!(open.currency, Currency::Ada));
        assert!(open.delegation.is_none());
        assert_eq!(open.amount, 5000000);
    }

    #[test]
    fn parse_open_with_key_delegation() {
        let e = kv(&format!(
            "iou_key={IOU_KEY},tag={TAG},action=Open,\
             currency=ADA,consumer={HASH28},provider={HASH28},\
             close_period=3600s,amount=5000000,delegation=KEY:{HASH28}"
        ));
        let Action::Open(open) = e.action else {
            panic!("expected Open")
        };
        assert!(open.delegation.and_then(|c| c.as_key()).is_some());
    }

    #[test]
    fn parse_open_with_native_currency() {
        let e = kv(&format!(
            "iou_key={IOU_KEY},tag={TAG},action=Open,\
             currency=BY_HASH:{HASH28},consumer={HASH28},provider={HASH28},\
             close_period=3600s,amount=5000000"
        ));
        let Action::Open(open) = e.action else {
            panic!("expected Open")
        };
        assert!(matches!(open.currency, Currency::ByHash { .. }));
    }

    // ── error paths ──────────────────────────────────────────────────────────

    #[test]
    fn error_add_missing_amount() {
        let err = kv_err(&format!("iou_key={IOU_KEY},tag={TAG},action=Add"));
        assert!(err.to_string().contains("missing amount"), "{err}");
    }

    #[test]
    fn error_unknown_action() {
        let err = kv_err(&format!("iou_key={IOU_KEY},tag={TAG},action=Explode"));
        assert!(err.to_string().contains("unknown action"), "{err}");
    }

    #[test]
    fn error_ambiguous_identifier() {
        let err = kv_err(&format!(
            "iou_key={IOU_KEY},tag={TAG},tx_hash={HASH32},index=0,action=Add,amount=5000000"
        ));
        assert!(err.to_string().contains("ambiguous"), "{err}");
    }

    #[test]
    fn error_iou_key_without_tag() {
        let err = kv_err(&format!("iou_key={IOU_KEY},action=Add,amount=5000000"));
        assert!(err.to_string().contains("without tag"), "{err}");
    }

    #[test]
    fn error_tx_hash_without_index() {
        let err = kv_err(&format!("tx_hash={HASH32},action=Add,amount=5000000"));
        assert!(err.to_string().contains("without index"), "{err}");
    }

    // ── JSON batch ───────────────────────────────────────────────────────────

    #[test]
    fn parse_json_batch() {
        let json = format!(
            r#"[
            {{"iou_key":"{IOU_KEY}","tag":"{TAG}","action":"Add","amount":5000000}},
            {{"iou_key":"{IOU_KEY}","tag":"{TAG}","action":"End"}}
        ]"#
        );
        let raws: Vec<RawEntry> = serde_json::from_str(&json).unwrap();
        let entries: Vec<Entry> = raws.into_iter().map(|r| r.try_into().unwrap()).collect();
        assert_eq!(entries.len(), 2);
        assert!(matches!(
            entries[0].action,
            Action::Want(Want::Add { amount: 5000000 })
        ));
        assert!(matches!(entries[1].action, Action::Want(Want::End)));
    }
}
