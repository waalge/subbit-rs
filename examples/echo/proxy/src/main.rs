mod cli;
mod proxy;
#[cfg(feature = "subbit")]
mod subbit;

use clap::Parser;
use cli::Cli;

static ENV_PATH: &str = ".env.examples.echo";

fn init_tracing() {
    let filter =
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

fn main() {
    init_tracing();
    dotenvy::from_filename(ENV_PATH).ok();
    Cli::parse().run();
}
