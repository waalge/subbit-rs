use std::{path::PathBuf, sync::Arc};

use tokio::sync::mpsc;

use crate::{
    feed::{BlockId, ChainEvent},
    tx::ResolverError,
};

use sled;

mod mock;

#[derive(Debug, Clone, Default)]
pub struct Config {
    provider: Provider,
}

#[derive(Debug, Clone)]
pub enum Provider {
    Mock,
    Sled { db_path: PathBuf },
}

impl Default for Provider {
    fn default() -> Self {
        Self::Mock
    }
}

pub fn mk_store(config: Config) -> Arc<dyn Store> {
    match config.provider {
        Provider::Mock => Arc::new(mock::Store::new()),
        Provider::Sled { .. } => todo!(),
    }
}

pub struct Keytag(pub Vec<u8>);
pub struct Lineage(pub Vec<u8>);

#[async_trait::async_trait]
pub trait Store: Send + Sync {
    async fn run(&self, rx: mpsc::Receiver<ChainEvent>) -> Result<(), StoreError>;
    async fn tip(&self) -> Result<Option<BlockId>, StoreError>;
    async fn channel(&self, id: &Keytag) -> Result<Vec<Lineage>, StoreError>;
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("storage: {0}")]
    Storage(String),

    #[error("codec: {0}")]
    Codec(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("Resolver: {0}")]
    Resolver(#[from] ResolverError),
}

impl From<sled::Error> for StoreError {
    fn from(e: sled::Error) -> Self {
        StoreError::Storage(e.to_string())
    }
}
