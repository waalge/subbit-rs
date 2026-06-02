use crate::wallet::WalletEnv;

/// Show
#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    Info {
        #[clap(flatten)]
        wallet: WalletEnv,
    },
    Tip {
        #[clap(flatten)]
        wallet: WalletEnv,
    },
}

impl Cmd {
    pub(crate) async fn run(self) -> anyhow::Result<()> {
        match self {
            Cmd::Info { wallet } => {
                let wallet = wallet.into_config()?.build();
                println!("{}", &serde_json::to_string_pretty(&wallet.info()).unwrap());
                Ok(())
            }
            Cmd::Tip { wallet } => {
                let wallet = wallet.into_config()?.build();
                let utxos = wallet.utxos().await;
                let Ok(utxos) = utxos else {
                    return Err(anyhow::anyhow!("Failed to query chain"));
                };
                for u in utxos.iter() {
                    println!("{:?}", u);
                }
                Ok(())
            }
        }
    }
}
