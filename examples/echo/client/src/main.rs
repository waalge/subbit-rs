use anyhow::Context;
use clap::Parser;

static ENV_PATH: &str = ".env.examples.echo";

#[derive(Parser)]
struct Cli {
    #[arg(
        long,
        env = "ECHO_CLIENT_BASE_URL",
        default_value = "http://127.0.0.1:3246",
        value_parser = reqwest::Url::parse,
    )]
    base_url: reqwest::Url,
    /// data to send; use @file to read from a file
    #[arg(short = 'd', long = "data", default_value_t = default_data())]
    data: String,
}

impl Cli {
    async fn run(self) -> anyhow::Result<()> {
        let body: Vec<u8> = match self.data.strip_prefix('@') {
            Some(path) => std::fs::read(path)?,
            None => self.data.into_bytes(),
        };
        serde_json::from_slice::<serde_json::Value>(&body).context("--data is not valid JSON")?;

        let resp = reqwest::Client::new()
            .post(self.base_url.join("echo")?)
            .body(body)
            .send()
            .await?
            .error_for_status()
            .context("server returned an error status")?
            .text()
            .await?;
        println!("{resp}");
        Ok(())
    }
}

fn init_tracing() {
    let filter =
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    dotenvy::from_filename(ENV_PATH).ok();
    Cli::parse().run().await
}

fn default_data() -> String {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "world".into());
    format!("hello, {user}")
}
