use cardano_sdk::{Address, Value, address::kind};

use crate::wallet::WalletEnv;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    Publish {
        #[clap(flatten)]
        wallet: WalletEnv,

        /// Where the reference script will live (defaults to signer's own address)
        #[clap(long, env = crate::meta::SCRIPT_HOST)]
        host_address: Option<Address<kind::Shelley>>,

        /// Spend all utxos, including those with ref scripts (defaults to false)
        #[clap(long, default_value_t = false)]
        spend_all: bool,
    },
    Send {
        #[clap(flatten)]
        wallet: WalletEnv,

        /// One or more recipients in the format <Address>:<amount>
        #[clap(long = "to", value_name = "ADDRESS:AMOUNT", value_parser = parse_recipient, num_args = 0..)]
        recipients: Vec<(Address<kind::Shelley>, u64)>,

        /// Change address (defaults to signer's own address)
        #[clap(long)]
        change_address: Option<Address<kind::Shelley>>,

        /// Spend all utxos, including those with ref scripts (defaults to false)
        #[clap(long, default_value_t = false)]
        spend_all: bool,
    },
    Subbit(super::tx_subbit::Cmd),
}

fn parse_recipient(s: &str) -> Result<(Address<kind::Shelley>, u64), String> {
    let (addr, amount) = s
        .split_once(':')
        .ok_or_else(|| format!("expected <Address>:<amount>, got '{s}'"))?;

    let address = addr
        .parse::<Address<kind::Shelley>>()
        .map_err(|e| e.to_string())?;
    let amount = amount
        .replace('_', "")
        .parse::<u64>()
        .map_err(|e| e.to_string())?;

    Ok((address, amount))
}

impl Cmd {
    pub(crate) async fn run(self) -> anyhow::Result<()> {
        match self {
            Self::Send {
                wallet,
                recipients,
                change_address,
                spend_all,
            } => {
                let wallet = wallet.into_config()?.build();
                let own_address = wallet.address();
                let change_address = change_address.unwrap_or(own_address.clone());
                if recipients.is_empty() && !spend_all && change_address == own_address {
                    return Err(anyhow::anyhow!("Vacuous tx. must have effect"));
                }
                let recipients = recipients
                    .into_iter()
                    .map(|(a, v)| (a.into(), Value::new(v)))
                    .collect();
                let mut utxos = wallet.utxos().await?;
                if !spend_all {
                    utxos = utxos
                        .into_iter()
                        .filter(|(_, o)| o.script().is_none())
                        .collect();
                }
                let protocol_parameters = wallet.protocol_parameters().await?;
                let mut tx = subbit_tx::ops::send(
                    &protocol_parameters,
                    &utxos,
                    recipients,
                    change_address.into(),
                )?;
                tx.sign_with(|hash| {
                    let sig = wallet.sign(hash.as_ref()); // or whatever Hash exposes
                    (wallet.verification_key(), sig)
                });
                wallet.submit(&tx).await?;
                Ok(())
            }
            Self::Publish {
                wallet,
                host_address,
                spend_all,
            } => {
                let wallet = wallet.into_config()?.build();
                let host_address = host_address.unwrap_or(wallet.address());
                let change_address = wallet.address();
                let mut utxos = wallet.utxos().await?;
                if !spend_all {
                    utxos = utxos
                        .into_iter()
                        .filter(|(_, o)| o.script().is_none())
                        .collect();
                }
                let protocol_parameters = wallet.protocol_parameters().await?;
                let script = subbit_tx::validator::VALIDATOR.script.clone();
                let mut tx = subbit_tx::ops::publish(
                    &protocol_parameters,
                    &utxos,
                    script,
                    host_address.into(),
                    change_address.into(),
                )?;
                tx.sign_with(|hash| {
                    let sig = wallet.sign(hash.as_ref()); // or whatever Hash exposes
                    (wallet.verification_key(), sig)
                });
                wallet.submit(&tx).await?;
                Ok(())
            }
            Self::Subbit(cmd) => cmd.run().await,
        }
    }
}
