use anyhow::{Result, bail};
use clap::Parser;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tracing::debug;
use url::Url;

mod wire;
use wire::{Row, parse_row, parse_rows, rows_to_map};

/// Mock client for pushing (keytag -> backing) rows to the server.
#[derive(Parser)]
struct Args {
    /// File of rows, one per line (no header unless --header is set)
    #[arg(long)]
    file: Option<PathBuf>,
    #[arg(long, requires = "file")]
    header: bool,
    /// Inline row, same format as a file line. Repeatable.
    #[arg(long = "row")]
    rows: Vec<String>,
    /// Base server URL, e.g. http://127.0.0.1:7822/v1/b
    #[arg(long, default_value = "http://127.0.0.1:7822/v1/b", value_parser = Url::parse)]
    endpoint: Url,
}

fn collect(file: Option<&Path>, header: bool, inline: &[String]) -> Result<Vec<Row>> {
    let mut rows = file
        .map(|f| load_file(f, header))
        .transpose()?
        .unwrap_or_default();
    rows.extend(
        inline
            .iter()
            .map(|r| parse_row(r))
            .collect::<Result<Vec<Row>>>()?,
    );
    Ok(rows)
}

fn load_file(path: &Path, header: bool) -> Result<Vec<Row>> {
    parse_rows(&std::fs::read(path)?, header)
}

#[derive(Serialize)]
struct SendResult {
    count: usize,
    status: u16,
}

async fn send(endpoint: &Url, rows: Vec<Row>) -> Result<SendResult> {
    let count = rows.len();
    let backings = rows_to_map(rows)?;
    let body = minicbor::to_vec(&backings)?;
    let resp = reqwest::Client::new()
        .post(endpoint.clone())
        .header("content-type", "application/cbor")
        .body(body)
        .send()
        .await?;

    let status = resp.status();
    debug!(%status, %endpoint, "response received");
    if !status.is_success() {
        bail!(
            "endpoint returned {status}: {}",
            resp.text().await.unwrap_or_default()
        );
    }
    Ok(SendResult {
        count,
        status: status.as_u16(),
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args = Args::parse();
    if args.file.is_none() && args.rows.is_empty() {
        bail!("pass --file <path> and/or --row \"hex,amount,subbed\"");
    }
    let rows = collect(args.file.as_deref(), args.header, &args.rows)?;
    let result = send(&args.endpoint, rows).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
