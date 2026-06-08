use cardano_sdk::LeakableSigningKey;
use subbit_core::Duration;

use crate::{json, meta, pretty, wallet::WalletEnv};

/// Show
#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    Info {
        #[clap(flatten)]
        wallet: WalletEnv,
        #[clap(long, env = meta::CLOSE_PERIOD)]
        closed_period: Duration,
    },
    Tip {
        #[clap(flatten)]
        wallet: WalletEnv,
    },
}

impl Cmd {
    pub(crate) async fn run(self) -> anyhow::Result<()> {
        match self {
            Cmd::Info {
                wallet,
                closed_period,
            } => {
                let wallet = wallet.into_config()?.build();
                let wallet_json = serde_json::to_value(wallet.info()).unwrap();
                let close_period_json =
                    serde_json::json!({"close_period" : closed_period.to_string() });
                let combo = json::merge(vec![wallet_json, close_period_json]);
                println!("{}", serde_json::to_string_pretty(&combo).unwrap());
                Ok(())
            }
            Cmd::Tip { wallet } => {
                let wallet = wallet.into_config()?.build();
                let utxos = wallet.utxos().await;
                let Ok(utxos) = utxos else {
                    return Err(anyhow::anyhow!("Failed to query chain"));
                };
                println!("{}", pretty::to_json_truncated(&utxos, 100)?);
                Ok(())
            }
        }
    }
}
