use crate::meta;
mod show;
mod tx;
mod tx_subbit;

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
    /// Build transactions related to admin duties.
    #[clap(subcommand)]
    Tx(tx::Cmd),
}

impl Cmd {
    pub(crate) async fn run(self) -> anyhow::Result<()> {
        if let Cmd::Init = self {
            let mut env_str = "# ./.env or ./.env.admin\n".to_string();
            let env_content = serde_json::json!({
                meta::BLOCKFROST_PROJECT_ID: "mainnetxxxxxxxxxxxxxxxxxxxx",
                meta::SIGNING_KEY: hex::encode(crate::wallet::rand_bytes32()),
            });
            env_str.push_str(
                &toml::to_string_pretty(&env_content)
                    .unwrap()
                    .replace(" = ", "="),
            );
            Ok(())
        } else {
            match self {
                Cmd::Show(cmd) => cmd.run().await,
                Cmd::Tx(cmd) => cmd.run().await,
                Cmd::Init => unreachable!(),
            }
        }
    }
}
