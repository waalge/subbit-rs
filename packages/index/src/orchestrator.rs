use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::feed::{BlockId, ChainEvent, Feed};
use crate::store::Store;

pub struct Orchestrator {
    feed: Arc<dyn Feed>,
    store: Arc<dyn Store>,
}

impl Orchestrator {
    pub fn new(feed: Arc<dyn Feed>, store: Arc<dyn Store>) -> Self {
        Self { feed, store }
    }

    pub async fn run(&self) -> Result<(), anyhow::Error> {
        let initial_tip: Option<BlockId> = None;
        let (tx, rx) = mpsc::channel::<ChainEvent>(256);

        let store = Arc::clone(&self.store);
        let store_handle = tokio::spawn(async move {
            match store.run(rx).await {
                Ok(()) => info!("store exited cleanly"),
                Err(e) => error!(err = %e, "store exited with error"),
            }
        });

        let feed = Arc::clone(&self.feed);
        let feed_handle = tokio::spawn(async move {
            match feed.run(initial_tip, tx).await {
                Ok(()) => info!("feed exited cleanly"),
                Err(e) => error!(err = %e, "feed exited with error"),
            }
        });

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("received Ctrl+C, shutting down");
            }
            _ = feed_handle => {
                info!("feed task exited (maybe unexpectedly)");
            }
            _ = store_handle => {
                warn!("store task exited unexpectedly");
            }
        }

        Ok(())
    }
}
