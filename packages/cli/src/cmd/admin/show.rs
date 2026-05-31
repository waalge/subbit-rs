use crate::wallet::WalletEnv;

/// Show
#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    Tip {
        #[clap(flatten)]
        wallet: WalletEnv,
    },
}

impl Cmd {
    pub(crate) async fn run(self) -> anyhow::Result<()> {
        match self {
            Cmd::Tip { wallet } => {
                let wallet = wallet.into_config()?.build();
                let utxos = wallet.utxos().await;
                println!("{:?}", utxos);
                Ok(())
            }
        }
    }
}
