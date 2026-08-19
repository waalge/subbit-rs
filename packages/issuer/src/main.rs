//! Local test client, no network I/O: `init` writes a starter config,
//! `show` prints it back, `spend` builds a request envelope for a URL path,
//! `response` applies a base64 response envelope you got some other way.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use subbit_issuer::{Account, Cache, Costings, Issuer, costings};

#[derive(Parser)]
struct Cli {
    #[arg(long, default_value = "subbit-issuer-config.toml")]
    config: PathBuf,
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
    Spend {
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
    costings: costings::Config,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_url: "https://example.com/subbit".into(),
            signing_key: hex::encode([0; 32]),
            tag: "deadbeef".to_string(),
            ttl_relative_secs: 3600,
            costings: Default::default(),
        }
    }
}

fn read_config(path: &PathBuf) -> Config {
    let raw = std::fs::read_to_string(path).expect("read config");
    toml::from_str(&raw).expect("parse config")
}

fn read_cache(path: &PathBuf) -> Option<Cache> {
    std::fs::read(path)
        .ok()
        .and_then(|b| minicbor::decode(&b).ok())
}

fn write_cache(path: &PathBuf, cache: &Cache) {
    std::fs::write(path, minicbor::to_vec(cache).expect("encode state")).expect("write state");
}

fn build(config: &Config, cache: Cache) -> (Issuer, Costings) {
    let key: [u8; 32] = hex::decode(&config.signing_key)
        .expect("signing_key hex")
        .try_into()
        .expect("signing_key must be 32 bytes");
    let tag = subbit_core::Tag::from(hex::decode(&config.tag).expect("tag hex"));
    let account = Account::new(ed25519_dalek::SigningKey::from_bytes(&key), tag);
    let ttl = subbit_core::Duration::from_secs(config.ttl_relative_secs);

    let issuer_config = subbit_issuer::Config::new(account, ttl);
    let issuer = Issuer::from_parts(issuer_config, cache);
    let costings = Costings::new(config.costings.clone()).expect("cost table");
    (issuer, costings)
}

fn main() {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Init => {
            let toml = toml::to_string_pretty(&Config::default()).expect("serialize config");
            std::fs::write(&cli.config, toml).expect("write config");
            println!("wrote {}", cli.config.display());
        }
        Cmd::Show => {
            let mut config = read_config(&cli.config);
            config.signing_key = "<redacted>".into();
            println!(
                "{}",
                toml::to_string_pretty(&config).expect("serialize config")
            );
            println!("{:?}", read_cache(&cli.cache),);
        }
        Cmd::Spend { path, json } => {
            let (mut issuer, costings) = build(
                &read_config(&cli.config),
                read_cache(&cli.cache).unwrap_or_default(),
            );
            let envelope = issuer.spend(costings.lookup(&path));
            if json {
                todo!("Not yet implemented")
                // println!("{}", serde_json::json!({ HEADER_NAME: envelope }));
            } else {
                println!("{envelope}");
            }
            write_cache(&cli.cache, issuer.cache());
        }
        Cmd::Response { body } => {
            let (mut issuer, _) = build(
                &read_config(&cli.config),
                read_cache(&cli.cache).unwrap_or_default(),
            );
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
