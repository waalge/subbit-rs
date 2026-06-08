// store/mock.rs
use std::collections::HashMap;
use std::sync::Mutex;

use tokio::sync::mpsc;
use tracing::{info, warn};

use super::{Keytag, StoreError};
use crate::{
    feed::{Block, BlockId, ChainEvent},
    store::Lineage,
    tx::Resolver,
};

pub struct Store {
    tip: Mutex<Option<BlockId>>,
    blocks: Mutex<HashMap<u64, Vec<Keytag>>>, // block_height → Vec<ChannelId>
    channel: Mutex<HashMap<Vec<u8>, Vec<[u8; 36]>>>, // ChannelId    → Vec<LineageId>
    lineage: Mutex<HashMap<[u8; 36], Vec<u8>>>, // LineageId    → (Constants, Vec<ChannelUtxo>)
}

impl Store {
    pub fn new() -> Self {
        Self {
            tip: Mutex::new(None),
            blocks: Mutex::new(HashMap::new()),
            channel: Mutex::new(HashMap::new()),
            lineage: Mutex::new(HashMap::new()),
        }
    }

    fn go(&self, block: Block) -> Result<(), StoreError> {
        for tx in &block.txs {
            let Some(mut resolver) = Resolver::new(&tx).ok() else {
                info!("Failed to parse!");
                continue;
            };
            match resolver.propagate()? {
                crate::tx::PropagateResult::Resolved => {
                    for io in resolver.to_io().iter() {
                        info!("IO :: {io}",);
                    }
                }
                crate::tx::PropagateResult::Unresolved => todo!(),
                crate::tx::PropagateResult::Unchanged => todo!(),
            };
        }
        Ok(())
    }

    fn back(&self, block_id: BlockId) -> Result<(), StoreError> {
        let mut blocks = self.blocks.lock().unwrap();
        let mut channel = self.channel.lock().unwrap();
        let mut lineage = self.lineage.lock().unwrap();
        let mut tip = self.tip.lock().unwrap();

        // collect all block heights to roll back
        let heights_to_undo: Vec<u64> = blocks
            .keys()
            .copied()
            .filter(|&h| h > block_id.block_height)
            .collect();

        for height in &heights_to_undo {
            let channel_ids = blocks.remove(height).unwrap_or_default();
            for key in channel_ids {
                let lineage_ids = channel.get(&key.0).cloned().unwrap_or_default();
                for lid in lineage_ids {
                    lineage.remove(&lid);
                }
                channel.remove(&key.0);
            }
        }

        *tip = Some(block_id);
        Ok(())
    }
}

#[async_trait::async_trait]
impl super::Store for Store {
    async fn run(&self, mut rx: mpsc::Receiver<ChainEvent>) -> Result<(), StoreError> {
        while let Some(event) = rx.recv().await {
            match event {
                ChainEvent::Go(block) => {
                    let block_id = block.id.clone();
                    self.go(block)?;
                    *self.tip.lock().unwrap() = Some(block_id);
                }
                ChainEvent::Back(block_id) => {
                    self.back(block_id)?;
                }
            }
        }
        Ok(())
    }

    async fn tip(&self) -> Result<Option<BlockId>, StoreError> {
        Ok(self.tip.lock().unwrap().clone())
    }

    async fn channel(&self, _id: &Keytag) -> Result<Vec<Lineage>, StoreError> {
        todo!("query lineages by keytag")
    }
}

fn log_event(event: &ChainEvent) {
    match event {
        ChainEvent::Go(block) => {
            info!(
                slot          = block.id.slot,
                block_height  = block.id.block_height,
                hash         = %hex::encode(block.id.hash),
                tx_count      = block.txs.len(),
                "applied block"
            );
        }
        ChainEvent::Back(block_id) => {
            warn!(
                slot         = block_id.slot,
                block_height = block_id.block_height,
                hash         = %hex::encode(block_id.hash),
                "rolled back block"
            );
        }
    }
}
