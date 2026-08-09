use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response as AxumResponse},
    routing::post,
};
use clap::{Parser, Subcommand};
use subbit_core::envelope::{Error, Request};
use tracing_subscriber::EnvFilter;

// `subbit_server` is the lib crate — config/ctx/db/etc all live there with no
// axum, clap, or other executable-only deps. This binary is the only place
// that pulls those in.
use subbit_server::{config::Config, ctx::Ctx};

#[derive(Parser, Debug)]
#[command(name = "subbit", about = "Subbit metering service", version)]
struct Cli {
    /// Path to the TOML config file
    #[arg(short, long, default_value = "config.toml", global = true)]
    config: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run the server
    Run {
        /// Address to bind the HTTP server on
        #[arg(long, default_value = "0.0.0.0:8080")]
        listen: SocketAddr,
    },
    /// Write a starter config.toml (default values) to --config and exit
    Init {
        /// Overwrite the file if it already exists
        #[arg(long)]
        force: bool,
    },
}

impl Command {
    async fn run(self, config_path: &PathBuf) -> anyhow::Result<()> {
        match self {
            Command::Init { force } => Self::init(config_path, force),
            Command::Run { listen } => Self::serve(config_path, listen).await,
        }
    }

    /// `subbit init [--config path] [--force]` — writes a `Config::default()`
    /// out as TOML so the user has something to edit rather than writing
    /// config.toml from scratch.
    fn init(path: &PathBuf, force: bool) -> anyhow::Result<()> {
        if path.exists() && !force {
            anyhow::bail!(
                "{} already exists — pass --force to overwrite",
                path.display()
            );
        }
        write_config(path)?;
        println!("wrote starter config to {}", path.display());
        Ok(())
    }

    /// `subbit run [--listen addr]` — load the config, build a `Ctx`, serve it.
    async fn serve(config_path: &PathBuf, listen: SocketAddr) -> anyhow::Result<()> {
        let config = load_config(config_path)?;
        let ctx = Arc::new(Ctx::from_config(config)?);

        let app = router(ctx);

        tracing::info!(%listen, "starting subbit");
        let listener = tokio::net::TcpListener::bind(listen).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    cli.command.run(&cli.config).await
}

// --- HTTP wiring below. Deliberately not part of subbit_server: `Ctx` owns
// every bit of actual behaviour, this is just mapping HTTP <-> `Ctx::request`.

fn router(ctx: Arc<Ctx>) -> Router {
    Router::new()
        .route("/v1/{*path}", post(handle_request))
        .with_state(ctx)
}

async fn handle_request(
    State(ctx): State<Arc<Ctx>>,
    Path(path): Path<String>,
    Json(req): Json<Request>,
) -> AxumResponse {
    // NOTE: adjust the axum::extract::Path pattern above / this `path` value
    // to match whatever `costings::lookup(url)` actually expects (full path,
    // just an endpoint name, etc).
    match ctx.request(req, &path) {
        Ok(status) => (StatusCode::OK, Json(status)).0.into_response(),
        Err(err) => error_response(err),
    }
}

/// `envelope::Error` is defined in `subbit_core`, so we can't `impl
/// IntoResponse for Error` directly (orphan rule) — a free function does the
/// same job.
fn error_response(err: Error) -> AxumResponse {
    // (Inactive, OldAuth, OldIou, Unspendable, RateLimited) — I don't have
    // doc comments on `envelope::Error` to confirm exact semantics, so
    // sanity-check each one, especially Inactive vs NoChannel.
    let status = match &err {
        Error::InvalidAuth => StatusCode::UNAUTHORIZED,
        Error::OldAuth => StatusCode::UNAUTHORIZED,
        Error::NoChannel => StatusCode::NOT_FOUND,
        Error::Inactive => StatusCode::FORBIDDEN,
        Error::InvalidIou => StatusCode::BAD_REQUEST,
        Error::OldIou => StatusCode::CONFLICT,
        Error::NoIou => StatusCode::CONFLICT,
        Error::Unspendable => StatusCode::PAYMENT_REQUIRED,
        Error::RateLimited => StatusCode::TOO_MANY_REQUESTS,
        Error::Other => StatusCode::INTERNAL_SERVER_ERROR,
    };

    (status, format!("{err:?}")).into_response()
}

// --- Config file I/O. Also not part of subbit_server: `Config` is just data
// there, TOML-on-disk is a concern of this binary, not the lib.

fn load_config(path: &PathBuf) -> anyhow::Result<Config> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("reading config {}: {e}", path.display()))?;
    let config: Config = toml::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("parsing config {}: {e}", path.display()))?;
    Ok(config)
}

/// Write a `Default` config out to `path` as TOML. Handy for generating a
/// starter config file (`subbit init` in the CLI above).
fn write_config(path: &PathBuf) -> anyhow::Result<()> {
    let toml = toml::to_string_pretty(&Config::default())
        .map_err(|e| anyhow::anyhow!("serializing default config: {e}"))?;
    std::fs::write(path, toml)
        .map_err(|e| anyhow::anyhow!("writing config {}: {e}", path.display()))?;
    Ok(())
}
