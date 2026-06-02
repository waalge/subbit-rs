use crate::meta;
mod data;
mod show;
// mod tx;

/// Admin CLI
#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Create a configuration with sensible defaults.
    ///
    /// Defaults can be overridden manually via options or via environment variables.
    /// See also admin --help.
    Init,

    /// Show current configuration.
    #[clap(subcommand)]
    Show(show::Cmd),
    /// Show current configuration.
    #[clap(subcommand)]
    Data(data::Cmd),
    // /// Build transactions related to admin duties.
    // #[clap(subcommand)]
    // Tx(tx::Cmd),
}

impl Cmd {
    pub(crate) async fn run(self) -> anyhow::Result<()> {
        if let Cmd::Init = self {
            println!("# ./.env or ./.env.consumer");
            println!(
                "{}=\"{}\"",
                meta::BLOCKFROST_PROJECT_ID,
                "mainnetxxxxxxxxxxxxxxxxxxxx"
            );
            println!(
                "{}=\"{}\"",
                meta::SIGNING_KEY,
                hex::encode(crate::wallet::rand_bytes32())
            );
            println!(
                "{}=\"{}\"",
                meta::IOU_KEY,
                hex::encode(crate::wallet::rand_bytes32())
            );
            Ok(())
        } else {
            match self {
                Cmd::Show(cmd) => cmd.run().await,
                Cmd::Data(cmd) => cmd.run().await,
                // Cmd::Tx(cmd) => cmd.run().await,
                Cmd::Init => unreachable!(),
            }
        }
    }
}
