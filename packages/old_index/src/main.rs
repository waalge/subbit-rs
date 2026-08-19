use subbit_index::Cmd;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Logging: structured JSON in production, human-readable in dev.
    // Controlled by RUST_LOG (defaults to info).
    // Examples:
    //   RUST_LOG=debug               — everything at debug+
    //   RUST_LOG=subbit_index=trace  — only this crate at trace
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("tracing is on");

    Cmd::init()?.run().await?;
    Ok(())
}
