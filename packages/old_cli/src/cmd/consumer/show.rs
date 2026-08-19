use cardano_sdk::LeakableSigningKey;

use crate::{json, meta, pretty, wallet::WalletEnv};

/// Show
#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    Info {
        #[clap(flatten)]
        wallet: WalletEnv,
        #[clap(long, env = meta::IOU_KEY)]
        iou_key: LeakableSigningKey,
    },
    Tip {
        #[clap(flatten)]
        wallet: WalletEnv,
    },
}

impl Cmd {
    pub(crate) async fn run(self) -> anyhow::Result<()> {
        match self {
            Cmd::Info { wallet, iou_key } => {
                let wallet = wallet.into_config()?.build();
                let wallet_json = serde_json::to_value(wallet.info()).unwrap();
                let iou_json =
                    serde_json::json!({"iou_key" : hex::encode(&iou_key.to_verification_key())});
                let combo = json::merge(vec![wallet_json, iou_json]);
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
