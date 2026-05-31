use std::fs;

use clap::Parser;

mod admin;
/// Role based cli
#[derive(Debug, clap::Parser)]
#[clap(version = env!("CARGO_PKG_VERSION"), about, long_about = None)]
pub(crate) enum Cmd {
    #[clap(subcommand)]
    Admin(admin::Cmd),
}

impl Cmd {
    pub(crate) async fn run(self) -> anyhow::Result<()> {
        match self {
            Self::Admin(cmd) => cmd.run().await,
        }
    }

    pub(crate) fn init() -> anyhow::Result<Self> {
        // Conditionally load any user-specific environment, based on command's names.
        let arg = std::env::args_os().nth(1);
        if let Some(arg_str) = arg.as_ref().and_then(|arg| arg.to_str()) {
            let role = format!(".env.{arg_str}");
            load_if_exists(&role)?;
        }
        // Load the global environment, after, so that the user-specific env takes precedence.
        load_if_exists(".env")?;
        Ok(Self::parse())
    }
}

pub fn load_if_exists(path: &str) -> anyhow::Result<()> {
    if fs::exists(path)? {
        dotenvy::from_filename(path).map_err(|err| anyhow::anyhow!("{}", err))?;
    }
    Ok(())
}
