use std::{path::PathBuf, sync::Arc, time::Duration};

use cardano_sdk::{Address, Network, address::kind};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{error, info};
use url::Url;

pub mod blockfrost;
pub mod mock;
pub mod recorder;

use crate::cardano::Hash32;

#[derive(Debug, Clone)]
pub struct Config {
    pub provider: Provider,
    pub recording: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub enum Provider {
    Ogmios {
        url: Url,
    },
    Blockfrost {
        project_id: String,
        interval: std::time::Duration,
    },
    Mock {
        path: PathBuf,
    },
}

pub fn mk_feed(config: Config, network: Network, address: Address<kind::Any>) -> Arc<dyn Feed> {
    let real: Arc<dyn Feed> = match config.provider {
        Provider::Ogmios { .. } => todo!(),
        Provider::Blockfrost {
            project_id,
            interval,
        } => {
            let client = blockfrost::client::BlockfrostClient::new(&project_id, network);
            Arc::new(blockfrost::Feed::new(client, address, interval))
        }
        Provider::Mock { path } => Arc::new(mock::Feed::new(path, Duration::from_millis(100))),
    };

    match config.recording {
        Some(path) if path.exists() => {
            panic!("Recording file already exists: {path:?}");
        }
        Some(path) => {
            info!(?path, "recording feed to file");
            Arc::new(recorder::Feed::new(real, path))
        }
        None => real,
    }
}

#[async_trait::async_trait]
pub trait Feed: Send + Sync {
    async fn run(
        &self,
        initial_tip: Option<BlockId>,
        tx: mpsc::Sender<ChainEvent>,
    ) -> Result<(), FeedError>;
}
// feed.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FeedError {
    #[error("network error: {0}")]
    Network(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("provider error: {0}")]
    Provider(String),
}

impl<T> From<tokio::sync::mpsc::error::SendError<T>> for FeedError {
    fn from(e: tokio::sync::mpsc::error::SendError<T>) -> Self {
        FeedError::Provider(e.to_string())
    }
}

impl From<tokio::task::JoinError> for FeedError {
    fn from(e: tokio::task::JoinError) -> Self {
        FeedError::Provider(e.to_string())
    }
}

impl From<serde_json::Error> for FeedError {
    fn from(e: serde_json::Error) -> Self {
        FeedError::Parse(e.to_string())
    }
}

impl From<std::io::Error> for FeedError {
    fn from(e: std::io::Error) -> Self {
        FeedError::Provider(e.to_string())
    }
}

// ─────────────────────────────────────────────────────────── Chain event types ─

/// Identifies a block by both slot and hash.
/// Slot is used for anchoring queries; hash is used for rollback detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockId {
    pub slot: u64,
    pub block_height: u64,
    pub hash: Hash32,
}

/// A block containing only txs relevant to the watched address, as raw CBOR.
#[derive(Debug, Serialize, Deserialize)]
pub struct Block {
    pub id: BlockId,
    pub txs: Vec<Vec<u8>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ChainEvent {
    Go(Block),
    Back(BlockId),
}
