use std::{path::PathBuf, sync::Arc};

use tokio::{io::AsyncWriteExt, sync::mpsc};

use crate::feed::{BlockId, ChainEvent, FeedError};

// feed/recording.rs
pub struct Feed {
    inner: Arc<dyn super::Feed>,
    path: PathBuf,
}

impl Feed {
    pub fn new(inner: Arc<dyn super::Feed>, path: impl Into<PathBuf>) -> Self {
        Self {
            inner,
            path: path.into(),
        }
    }
}

#[async_trait::async_trait]
impl super::Feed for Feed {
    async fn run(
        &self,
        initial_tip: Option<BlockId>,
        tx: mpsc::Sender<ChainEvent>,
    ) -> Result<(), FeedError> {
        let (inner_tx, mut inner_rx) = mpsc::channel::<ChainEvent>(256);
        let path = self.path.clone();
        let tee = tokio::spawn(async move {
            let mut file = tokio::fs::File::create(&path)
                .await
                .map_err(|e| FeedError::Provider(e.to_string()))?;
            while let Some(event) = inner_rx.recv().await {
                let mut line =
                    serde_json::to_string(&event).map_err(|e| FeedError::Parse(e.to_string()))?;
                line.push('\n');
                file.write_all(line.as_bytes())
                    .await
                    .map_err(|e| FeedError::Provider(e.to_string()))?;
                tx.send(event).await?; // already has From impl
            }
            Ok::<(), FeedError>(())
        });
        self.inner.run(initial_tip, inner_tx).await?;
        tee.await??;
        Ok(())
    }
}
