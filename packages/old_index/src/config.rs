//! Configuration for the Subbit index daemon.
//!
//! Every value can be set via environment variable (loaded from `.env`) or
//! a CLI flag. CLI flags take precedence over environment variables.
//!
//! # Quick start
//!
//! ```ini
//! # .env
//! BACKEND=ogmios
//! OGMIOS_URL=ws://localhost:1337
//! ADDRESS=addr1...
//! START=2024-01-01
//! POLL_INTERVAL=5s
//! ```
//!
//! ```sh
//! subbit-index --backend ogmios --ogmios-url ws://localhost:1337 \
//!              --address addr1... --start 2024-01-01
//! ```

use std::path::PathBuf;

use cardano_sdk::{Address, Network, address::kind};
use thiserror::Error;

use crate::{Cmd, cardano::Point, cmd::FeedKind, feed, store};

// ─────────────────────────────────────────────────────────────────── Errors ──

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("requires requires --network (or NETWORK)")]
    MissingNetwork,

    #[error("ogmios backend requires --ogmios-url (or OGMIOS_URL)")]
    MissingOgmiosUrl,

    #[error("blockfrost backend requires --blockfrost-project-id (or BLOCKFROST_PROJECT_ID)")]
    MissingBlockfrostProjectId,

    #[error("Mock and Recording require a path to read and write the recording")]
    MissingFeedRecording,

    #[error("ADDRESS: invalid bech32 address: {0}")]
    InvalidAddress(String),
}

// ─────────────────────────────────────────────────────────────────── Config ──

/// Resolved, validated configuration — the single source of truth at runtime.
#[derive(Debug, Clone)]
pub struct Config {
    pub feed: feed::Config,
    pub store: store::Config,
    pub network: Network,
    pub address: Address<kind::Any>,
    pub start: Point,
    pub db_path: PathBuf,
}

impl TryFrom<Cmd> for Config {
    type Error = ConfigError;

    fn try_from(cmd: Cmd) -> Result<Self, ConfigError> {
        let network = cmd.network;
        let feed_provider = match cmd.feed {
            FeedKind::Ogmios => feed::Provider::Ogmios {
                url: cmd.ogmios_url.ok_or(ConfigError::MissingOgmiosUrl)?,
            },
            FeedKind::Blockfrost => feed::Provider::Blockfrost {
                project_id: cmd
                    .blockfrost_project_id
                    .ok_or(ConfigError::MissingBlockfrostProjectId)?,
                interval: cmd.poll_interval.into(),
            },
            FeedKind::Mock => feed::Provider::Mock {
                path: cmd
                    .feed_recording
                    .clone()
                    .ok_or(ConfigError::MissingFeedRecording)?,
            },
        };
        let recording = cmd
            .record
            .then(|| cmd.feed_recording.ok_or(ConfigError::MissingFeedRecording))
            .transpose()?;

        let feed = feed::Config {
            recording,
            provider: feed_provider,
        };

        let address = cmd.address;

        let store = store::Config::default();

        Ok(Config {
            store,
            feed,
            network,
            address,
            start: cmd.start,
            db_path: cmd.db_path,
        })
    }
}
