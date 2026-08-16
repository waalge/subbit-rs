//! Local test client, no network I/O: `init` writes a starter settings,
//! `show` prints it back, `spend` builds a request envelope for a URL path,
//! `response` applies a base64 response envelope you got some other way.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use subbit_issuer::{Account, Cache, Costings, Issuer, costings};

#[derive(Parser)]
struct Cli {
    #[arg(long, default_value = "settings.toml")]
    settings: PathBuf,
    #[arg(long, default_value = "/tmp/subbit-state.cbor")]
    state: PathBuf,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Write a starter settings to file.
    Init,
    /// Print the current settings.
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
struct Settings {
    server_url: String,
    signing_key: String, // hex
    tag: String,         // hex
    ttl_relative_secs: u64,
    costings: costings::Config,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server_url: "https://example.com/subbit".into(),
            signing_key: hex::encode([0;32]),
            tag: "deadbeef".to_string(),
            ttl_relative_secs: 3600,
            costings: Default::default(),
        }
    }
}

fn read_settings(path: &PathBuf) -> Settings {
    let raw = std::fs::read_to_string(path).expect("read settings");
    toml::from_str(&raw).expect("parse settings")
}

fn read_cache(path: &PathBuf) -> Cache {
    std::fs::read(path)
        .ok()
        .and_then(|b| minicbor::decode(&b).ok())
        .unwrap_or_default()
}
 
fn write_cache(path: &PathBuf, cache: &Cache) {
    std::fs::write(path, minicbor::to_vec(cache).expect("encode state")).expect("write state");
}

fn build(settings: &Settings, cache: Cache) -> (Issuer, Costings) {
    let key: [u8; 32] = hex::decode(&settings.signing_key)
        .expect("signing_key hex")
        .try_into()
        .expect("signing_key must be 32 bytes");
    // TODO: confirm `Tag`/`Duration` constructors — assumed `From<Vec<u8>>` / `from_secs`.
    let tag = subbit_core::Tag::from(hex::decode(&settings.tag).expect("tag hex"));
    let account = Account::new(ed25519_dalek::SigningKey::from_bytes(&key), tag);
    let ttl = subbit_core::Duration::from_secs(settings.ttl_relative_secs);

    let config = subbit_issuer::Config::new(account, ttl);
    let issuer = Issuer::from_parts(config, cache);
    let costings = Costings::new(settings.costings.clone()).expect("cost table");
    (issuer, costings)
}

fn main() {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Init => {
            let toml = toml::to_string_pretty(&Settings::default()).expect("serialize settings");
            std::fs::write(&cli.settings, toml).expect("write settings");
            println!("wrote {}", cli.settings.display());
        }
        Cmd::Show => {
            let mut settings = read_settings(&cli.settings);
            settings.signing_key = "<redacted>".into();
            println!("{}", toml::to_string_pretty(&settings).expect("serialize settings"));
        }
        Cmd::Spend { path, json } => {
            let (mut issuer, costings) = build(&read_settings(&cli.settings), read_cache(&cli.state));
            let envelope = issuer.spend(costings.lookup(&path));
            if json {
                todo!("Not yet implemented")
                // println!("{}", serde_json::json!({ HEADER_NAME: envelope }));
            } else {
                println!("{envelope}");
            }
        }
        Cmd::Response { body } => {
            let (mut issuer, _) = build(&read_settings(&cli.settings), read_cache(&cli.state));
            match issuer.response(&body) {
                Ok(()) => {
                    write_cache(&cli.state, issuer.cache());
                    println!("balance: {}", issuer.spent());
                }
                Err(e) => eprintln!("error: {e}"),
            }
        }
    }
}
