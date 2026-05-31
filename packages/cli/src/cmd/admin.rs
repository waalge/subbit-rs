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

    // /// Show current configuration.
    #[clap(subcommand)]
    Show(show::Cmd),
    // /// Build transactions related to admin duties.
    // #[clap(subcommand)]
    // Tx(tx::Cmd),
}

impl Cmd {
    pub(crate) async fn run(self) -> anyhow::Result<()> {
        if let Cmd::Init = self {
            println!("Not yet impl");
            Ok(())
        } else {
            match self {
                Cmd::Show(cmd) => cmd.run().await,
                // Cmd::Tx(cmd) => cmd.run(&config).await,
                Cmd::Init => unreachable!(),
            }
        }
    }
}
