//! Integration tests against the real Blockfrost API.
//!
//! Driven by environment variables, loaded from `.env.test` if present:
//!
//!   SUBBIT_BLOCKFROST_PROJECT_ID=preprod...
//!   SUBBIT_TEST_ADDRESS=addr_test1...
//!   SUBBIT_TEST_TX_HASH=abc123...
//!
//! Run with:
//!
//!   cargo test -p subbit-index --test blockfrost_integration -- --ignored --nocapture

use cardano_sdk::{Network, Transaction, transaction::state::ReadyForSigning};
use subbit_index::cardano::Hash32;
use subbit_index::feed::blockfrost::client::{AddressTxsParams, BlockfrostClient, ClientError};

// ─────────────────────────────────────────────────────────── Fixture ──────────

struct Fixture {
    client: BlockfrostClient,
    address: String,
    tx_hash: Hash32,
}

impl Fixture {
    fn load() -> Self {
        dotenvy::from_filename(".env.test").ok();
        Self {
            client: BlockfrostClient::new(&env("SUBBIT_BLOCKFROST_PROJECT_ID"), Network::Preprod),
            address: env("SUBBIT_TEST_ADDRESS"),
            tx_hash: env_hash32("SUBBIT_TEST_TX_HASH"),
        }
    }
}

fn env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("{key} must be set (or in .env.test)"))
}

fn env_hash32(key: &str) -> Hash32 {
    let hex = env(key);
    let bytes = hex::decode(&hex).unwrap_or_else(|e| panic!("{key}: invalid hex: {e}"));
    bytes
        .try_into()
        .unwrap_or_else(|v: Vec<u8>| panic!("{key}: expected 32 bytes, got {}", v.len()))
}

// ─────────────────────────────────────────────────────────── Tests ────────────

#[tokio::test]
#[ignore]
async fn address_transactions_default() {
    let f = Fixture::load();

    let txs = f
        .client
        .address_transactions(&f.address, &AddressTxsParams::default())
        .await
        .expect("address_transactions failed");

    assert!(!txs.is_empty(), "expected transactions for {}", f.address);
    assert!(
        txs.windows(2)
            .all(|w| w[0].block_height <= w[1].block_height),
        "default order should be asc"
    );

    eprintln!("{} transactions", txs.len());
    for tx in txs.iter().take(3) {
        eprintln!(
            "  slot={} idx={} tx={}",
            tx.block_height,
            tx.tx_index,
            hex::encode(tx.tx_hash)
        );
    }
}

#[tokio::test]
#[ignore]
async fn address_transactions_from_recent_slot() {
    let f = Fixture::load();

    // first get all txs to find a mid-point slot
    let all = f
        .client
        .address_transactions(&f.address, &AddressTxsParams::default())
        .await
        .expect("initial fetch failed");

    assert!(all.len() >= 2, "need at least 2 txs for this test");

    let mid_slot = all[all.len() / 2].block_height;
    let params = AddressTxsParams::asc_from(mid_slot);

    let from_mid = f
        .client
        .address_transactions(&f.address, &params)
        .await
        .expect("from-slot fetch failed");

    assert!(!from_mid.is_empty());
    assert!(
        from_mid[0].block_height >= mid_slot,
        "first result should be at or after from slot"
    );

    eprintln!(
        "from slot {mid_slot}: {} txs (of {})",
        from_mid.len(),
        all.len()
    );
}

#[tokio::test]
#[ignore]
async fn block_at_height_returns_correct_slot() {
    let f = Fixture::load();

    let txs = f
        .client
        .address_transactions(&f.address, &AddressTxsParams::default())
        .await
        .expect("address_transactions failed");

    let tx = txs.first().expect("need at least one tx");
    let height = tx.block_height;

    let block = f
        .client
        .block_at_height(height) // block number, not slot
        .await
        .expect("block_at_height failed");

    assert_eq!(block.height, height);
    assert!(
        block.slot > height,
        "slot should be larger than block height on Cardano"
    );
    assert_ne!(block.hash, [0u8; 32]);

    eprintln!(
        "height={} slot={} hash={}",
        block.height,
        block.slot,
        hex::encode(block.hash)
    );
}

#[tokio::test]
#[ignore]
async fn tx_cbor_decodes_to_bytes() {
    let f = Fixture::load();

    let cbor = f.client.tx_cbor(&f.tx_hash).await.expect("tx_cbor failed");

    assert!(!cbor.is_empty());
    // CBOR arrays start with 0x8_ or maps with 0xa_; tx is typically an array
    eprintln!(
        "tx cbor: {} bytes, first byte=0x{:02x}",
        cbor.len(),
        cbor[0]
    );
}

#[tokio::test]
#[ignore]
async fn tx_cbor_roundtrip_via_cardano_sdk() {
    let f = Fixture::load();

    let cbor = f.client.tx_cbor(&f.tx_hash).await.expect("tx_cbor failed");

    // smoke test: cardano_sdk can parse what blockfrost gives us
    //
    let tx: Transaction<ReadyForSigning> =
        cardano_sdk::cbor::decode(&cbor).expect("cardano_sdk failed to parse tx cbor");

    eprintln!(
        "parsed tx: {} inputs, {} outputs",
        tx.inputs().count(),
        tx.outputs().count(),
    );
}

#[tokio::test]
#[ignore]
async fn not_found_returns_client_error() {
    let f = Fixture::load();

    let bogus: Hash32 = [0u8; 32];
    let result = f.client.tx_cbor(&bogus).await;

    assert!(
        matches!(result, Err(ClientError::NotFound { .. })),
        "expected NotFound, got {result:?}"
    );
}
