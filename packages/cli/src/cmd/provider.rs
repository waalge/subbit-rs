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
    /// See also provider --help.
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
            let mut env_str = "# ./.env or ./.env.provider\n".to_string();
            let env_content = serde_json::json!({
                meta::BLOCKFROST_PROJECT_ID: "mainnetxxxxxxxxxxxxxxxxxxxx",
                meta::SIGNING_KEY: hex::encode(crate::wallet::rand_bytes32()),
                meta::CLOSE_PERIOD: "24h",
            });
            env_str.push_str(
                &toml::to_string_pretty(&env_content)
                    .unwrap()
                    .replace(" = ", "="),
            );
            println!("{}", env_str);
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
