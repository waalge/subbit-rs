//! Local test client, no network I/O: `init` writes a starter config,
//! `show` prints it back, `spend` builds a request envelope for a URL path,
//! `response` applies a base64 response envelope you got some other way.

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use subbit_issuer::{
    Account, Cache, Issuer,
    cost::url_lookup::{self, UrlLookup},
};

#[derive(Parser)]
struct Cli {
    #[command(flatten)]
    sources: subbit_config::Args,
    #[arg(long, default_value = "/tmp/subbit-state.cbor")]
    cache: PathBuf,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Write a starter config to file.
    Init,
    /// Print the current config.
    Show,
    /// Build a request envelope for spending against a URL path.
    Request {
        path: String,
        /// Print as `{"subbit": "..."}` instead of raw base64.
        #[arg(long)]
        json: bool,
    },
    /// Apply a base64 response envelope.
    Response { body: String },
}

#[derive(Serialize, Deserialize)]
struct Config {
    server_url: String,
    signing_key: String, // hex
    tag: String,         // hex
    ttl_relative_secs: u64,
    cost: url_lookup::Config,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_url: "https://example.com/subbit".into(),
            signing_key: hex::encode([0; 32]),
            tag: "deadbeef".to_string(),
            ttl_relative_secs: 3600,
            cost: Default::default(),
        }
    }
}

fn read_cache(path: &PathBuf) -> Option<Cache> {
    std::fs::read(path)
        .ok()
        .and_then(|b| minicbor::decode(&b).ok())
}

fn write_cache(path: &PathBuf, cache: &Cache) {
    std::fs::write(path, minicbor::to_vec(cache).expect("encode state")).expect("write state");
}

fn build(config: &Config, cache: Cache) -> (Issuer, UrlLookup) {
    let key: [u8; 32] = hex::decode(&config.signing_key)
        .expect("signing_key hex")
        .try_into()
        .expect("signing_key must be 32 bytes");
    let tag = subbit_core::Tag::from(hex::decode(&config.tag).expect("tag hex"));
    let account = Account::new(ed25519_dalek::SigningKey::from_bytes(&key), tag);
    let ttl = subbit_core::Duration::from_secs(config.ttl_relative_secs);

    let issuer_config = subbit_issuer::Config::new(account, ttl);
    let issuer = Issuer::from_parts(issuer_config, cache);
    let cost = UrlLookup::new(config.cost.clone()).expect("cost table");
    (issuer, cost)
}

fn main() {
    let cli = Cli::parse();
    let sources = cli
        .sources
        .into_sources(Path::new("subbit-issuer-config.toml"));

    if let Cmd::Init = cli.cmd {
        let toml = toml::to_string_pretty(&Config::default()).expect("serialize config");
        std::fs::write(sources.base, toml).expect("write config");
        println!("wrote {}", sources.base.display());
        return;
    }

    let config: Config = sources.load().expect("failed to load config");

    match cli.cmd {
        Cmd::Init => unreachable!(),
        Cmd::Show => {
            let mut config = config;
            config.signing_key = "<redacted>".into();
            println!(
                "{}",
                toml::to_string_pretty(&config).expect("serialize config")
            );
            println!("{:?}", read_cache(&cli.cache));
        }
        Cmd::Request { path, json } => {
            let (mut issuer, cost) = build(&config, read_cache(&cli.cache).unwrap_or_default());
            let envelope = issuer.request(cost.lookup(&path));
            if json {
                todo!("Not yet implemented")
                // println!("{}", serde_json::json!({ HEADER_NAME: envelope }));
            } else {
                println!("{envelope}");
            }
            write_cache(&cli.cache, issuer.cache());
        }
        Cmd::Response { body } => {
            let (mut issuer, _) = build(&config, read_cache(&cli.cache).unwrap_or_default());
            match issuer.response(&body) {
                Ok(()) => {
                    write_cache(&cli.cache, issuer.cache());
                    println!("balance: {}", issuer.spent());
                }
                Err(e) => eprintln!("error: {e}"),
            }
        }
    }
}
