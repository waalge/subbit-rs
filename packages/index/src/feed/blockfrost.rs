//! Blockfrost implementation of the chain feed.
//!
//! Polls Blockfrost for transactions at the watched address and emits
//! [`ChainEvent`]s. The orchestrator hands the store's tip once at startup;
//! the feed maintains its own cursor thereafter.
//!
//! # Poll cycle
//!
//!   1. `GET /addresses/{address}/transactions?from={cursor.block_height}&order=asc`
//!   2. Diff against cursor → detect rollback or new txs
//!   3. Group new txs by block_height
//!   4. `GET /blocks/{height}` → block hash + slot for each new block
//!   5. `GET /txs/{hash}/cbor` → raw CBOR for each tx (concurrent per block)
//!   6. Emit `ChainEvent::Do(Block)` per block, `ChainEvent::Undo(BlockId)` on rollback

use std::collections::BTreeMap;

use cardano_sdk::{Address, address::kind};
use futures::future::try_join_all;
use tokio::sync::mpsc;
use tracing::{debug, instrument, warn};

pub mod client;
use client::{AddressTx, AddressTxsParams, BlockfrostClient};

use crate::{
    cardano::Hash32,
    feed::{Block, BlockId, ChainEvent},
};

// The cursor is the last tx we successfully emitted. We match on both
// tx_hash and block_height — a tx resubmitted after a rollback will appear
// at a different block_height, so both must agree for a clean continuation.

#[derive(Debug, Clone)]
struct Cursor {
    tx_hash: Hash32,
    block_height: u64,
}

pub struct Feed {
    client: BlockfrostClient,
    address: Address<kind::Any>,
    interval: std::time::Duration,
}

impl Feed {
    pub fn new(
        client: BlockfrostClient,
        address: Address<kind::Any>,
        interval: std::time::Duration,
    ) -> Self {
        Self {
            client,
            address,
            interval,
        }
    }

    // ── Poll cycle ────────────────────────────────────────────────────────────

    #[instrument(skip(self, tx), fields(cursor = ?cursor.as_ref().map(|c| c.block_height)))]
    async fn poll(
        &self,
        cursor: &mut Option<Cursor>,
        tx: &mpsc::Sender<ChainEvent>,
    ) -> Result<(), super::FeedError> {
        // anchor from one block before cursor for overlap — this is how we
        // detect rollbacks: if our cursor tx is absent from the results,
        // the chain has diverged
        tracing::debug!(?cursor, "poll tick");
        let params = match cursor {
            Some(c) => AddressTxsParams::asc_from(c.block_height.saturating_sub(1)),
            None => AddressTxsParams {
                order: Some("asc"),
                ..Default::default()
            },
        };

        let txs = self
            .client
            .address_transactions(&self.address.to_string(), &params)
            .await?;

        let (to_undo, to_apply) = self.diff(cursor, &txs);

        for block_id in to_undo.into_iter().rev() {
            warn!(
                block_height = block_id.block_height,
                slot = block_id.slot,
                "rollback"
            );
            tx.send(ChainEvent::Back(block_id)).await?;
        }

        if to_apply.is_empty() {
            debug!("up to date");
            return Ok(());
        }

        // group new txs by block_height, BTreeMap preserves ascending order
        let mut by_block: BTreeMap<u64, Vec<Hash32>> = BTreeMap::new();
        for atx in &to_apply {
            by_block
                .entry(atx.block_height)
                .or_default()
                .push(atx.tx_hash);
        }

        for (block_height, tx_hashes) in by_block {
            let (bf_block, cbors) = tokio::try_join!(
                self.client.block_at_height(block_height),
                try_join_all(tx_hashes.iter().map(|h| self.client.tx_cbor(h))),
            )?;

            let block = Block {
                id: BlockId {
                    slot: bf_block.slot,
                    block_height: bf_block.height,
                    hash: bf_block.hash,
                },
                txs: cbors,
            };

            *cursor = Some(Cursor {
                tx_hash: *tx_hashes.last().unwrap(),
                block_height: bf_block.height,
            });
            tx.send(ChainEvent::Go(block)).await?;
        }

        Ok(())
    }

    // ── Diff ──────────────────────────────────────────────────────────────────
    //
    // Returns (to_undo, to_apply).
    //
    // Clean:    cursor tx found at expected block_height → apply everything after it.
    // Rollback: cursor tx absent → emit Undo for cursor, apply all current txs.
    //           The store walks its own history to find the common ancestor.

    fn diff(&self, cursor: &Option<Cursor>, txs: &[AddressTx]) -> (Vec<BlockId>, Vec<AddressTx>) {
        let Some(cursor) = cursor else {
            return (vec![], txs.to_vec());
        };

        match txs
            .iter()
            .position(|t| t.tx_hash == cursor.tx_hash && t.block_height == cursor.block_height)
        {
            Some(pos) => (vec![], txs[pos + 1..].to_vec()),
            None => {
                // rollback — we emit a single Undo; the BlockId slot/hash will
                // be filled by the store which has the full history
                let undo = BlockId {
                    slot: 0, // store resolves this from its own history
                    block_height: cursor.block_height,
                    hash: cursor.tx_hash, // used as identity, not block hash
                };
                (vec![undo], txs.to_vec())
            }
        }
    }
}

#[async_trait::async_trait]
impl super::Feed for Feed {
    async fn run(
        &self,
        initial_tip: Option<BlockId>,
        tx: mpsc::Sender<ChainEvent>,
    ) -> Result<(), super::FeedError> {
        let mut cursor: Option<Cursor> = initial_tip.map(|b| Cursor {
            tx_hash: b.hash,
            block_height: b.block_height,
        });
        loop {
            self.poll(&mut cursor, &tx).await?;
            tokio::time::sleep(self.interval).await;
        }
    }
}
