use cardano_sdk::{Address, Network, address::kind};
use clap::Parser;
use std::{fs, path::PathBuf};
use url::Url;

use crate::{
    Config,
    cardano::Point,
    feed::{self},
    meta, orchestrator, store,
};

/// Subbit on-chain index daemon.
///
/// Follows a Cardano address and maintains a local view of its UTxO set and
/// channel state. All options can also be set via environment variable or
/// a `.env` file in the working directory.
#[derive(Debug, Parser)]
#[command(version, about, verbatim_doc_comment)]
pub struct Cmd {
    /// Network. One of: mainnet, preprod, preview.
    #[arg(long, env = meta::NETWORK, value_name = "NET", default_value = "preprod", value_parser = |s : &str| Network::try_from(s))]
    pub network: Network,

    /// Cardano chain feed to use.
    ///
    /// Accepted values: ogmios, blockfrost
    #[arg(long, env = meta::FEED, value_name = "NAME")]
    pub feed: FeedKind,

    /// WebSocket URL for the Ogmios backend.
    ///
    /// Required when --backend=ogmios. Example: ws://localhost:1337
    #[arg(long, env = meta::OGMIOS_URL, value_name = "URL")]
    pub ogmios_url: Option<Url>,

    /// Blockfrost project ID.
    ///
    /// Required when --backend=blockfrost.
    #[arg(long, env = meta::BLOCKFROST_PROJECT_ID, value_name = "ID")]
    pub blockfrost_project_id: Option<String>,

    // ── what to follow ────────────────────────────────────────────────────────
    /// Cardano address to follow (bech32).
    ///
    /// All UTxOs at this address will be tracked.
    #[arg(long, env = meta::ADDRESS, value_name = "ADDR")]
    pub address: Address<kind::Any>,

    // ── sync start ────────────────────────────────────────────────────────────
    /// Where to start syncing from.
    ///
    /// Accepted values:
    ///   tip                        — only index blocks from now on
    ///   <YYYY-MM-DD>               — index from this date (approximate)
    ///   slot:<n>:<block-hash>      — index from an exact chain point
    ///
    /// Defaults to "tip".
    #[arg(long, env = meta::START, value_name = "POINT", default_value = "tip")]
    pub start: Point,

    // ── polling ───────────────────────────────────────────────────────────────
    /// How often to poll for new blocks when at the tip.
    ///
    /// Accepts human-readable durations: 5s, 500ms, 1m. Defaults to 5s.
    #[arg(long, env = meta::POLL_INTERVAL, value_name = "DURATION", default_value = "5s")]
    pub poll_interval: humantime::Duration,

    // ── misc ──────────────────────────────────────────────────────────────────
    /// Path to the database file. Defaults to ./subbit-index.db
    #[arg(long, env = meta::DB_PATH, value_name = "PATH", default_value = "subbit-index.db")]
    pub db_path: PathBuf,

    // ── mock feed ──────────────────────────────────────────────────────────────────
    /// Path to the database file. Defaults to ./subbit-index.db
    #[arg(long, env = meta::FEED_RECORDING, value_name = "PATH", default_value = "tmp-feed-data.jsonl")]
    pub feed_recording: Option<PathBuf>,

    #[arg(long, default_value = "false")]
    pub record: bool,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum FeedKind {
    Ogmios,
    Blockfrost,
    Mock,
}

impl Cmd {
    pub async fn run(self) -> anyhow::Result<()> {
        let config: Config = self.try_into()?;

        let store = store::mk_store(config.store);
        let feed = feed::mk_feed(config.feed, config.network, config.address);

        orchestrator::Orchestrator::new(feed, store).run().await?;
        Ok(())
    }

    pub fn init() -> anyhow::Result<Self> {
        load_if_exists(".env.index")?;
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
