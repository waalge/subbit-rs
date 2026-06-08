use std::{path::PathBuf, time::Duration};

use tokio::sync::mpsc;
use tracing::info;

use crate::feed::{BlockId, ChainEvent, FeedError};

// feed/mock.rs
pub struct Feed {
    path: PathBuf,
    interval: Duration, // simulate polling cadence
}

impl Feed {
    pub fn new(path: impl Into<PathBuf>, interval: Duration) -> Self {
        Self {
            path: path.into(),
            interval,
        }
    }
}

#[async_trait::async_trait]
impl super::Feed for Feed {
    async fn run(
        &self,
        _initial_tip: Option<BlockId>,
        tx: mpsc::Sender<ChainEvent>,
    ) -> Result<(), FeedError> {
        let contents = tokio::fs::read_to_string(&self.path).await?;
        for line in contents.lines() {
            let event: ChainEvent = serde_json::from_str(line)?;
            match event {
                ChainEvent::Go(ref block) => info!(height = ?block.id.block_height, "Go"),
                ChainEvent::Back(ref block_id) => info!(height = block_id.block_height, "Back"),
            };
            tx.send(event).await?;
            tokio::time::sleep(self.interval).await;
        }
        info!("mock feed exhausted recording");
        Ok(())
    }
}
