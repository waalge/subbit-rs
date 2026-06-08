//! Minimal Blockfrost HTTP client covering exactly three endpoints:
//!
//!   GET /addresses/{address}/transactions
//!   GET /blocks/{height}
//!   GET /txs/{hash}/cbor
//!
//! All hex fields are decoded to bytes immediately on deserialisation.
//! No dependency on the `blockfrost` crate.
//!
//! # TODO
//!
//! - [ ] Test `AddressTxsParams::to` bounds the response correctly
//! - [ ] Test `AddressTxsParams::count` limits page size (verify < 100 results)
//! - [ ] Test `AddressTxsParams::page` returns the next page of results
//! - [ ] Test `AddressTxsParams::order = "desc"` returns newest-first
//! - [ ] Test `from` with `"block_number:tx_index"` format (colon form)
//! - [ ] Test `from` + `to` together as a bounded window
//! - [ ] Verify 429 rate-limit error surfaces as `ClientError::RateLimited`
//! - [ ] Test pagination: address with > 100 txs requires multiple pages

use serde::Deserialize;
use serde_with::{hex::Hex, serde_as};
use thiserror::Error;

use cardano_sdk::Network;

use crate::cardano::Hash32;

// ─────────────────────────────────────────────────────────────────── Errors ──

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("HTTP error {status} from {url}")]
    Http { status: u16, url: String },

    #[error("rate limited (429) — back off and retry")]
    RateLimited,

    #[error("not found (404): {url}")]
    NotFound { url: String },

    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("deserialise: {0}")]
    Deserialize(#[from] serde_json::Error),
}

impl From<ClientError> for crate::feed::FeedError {
    fn from(e: ClientError) -> Self {
        Self::Network(e.to_string())
    }
}

type Result<T> = std::result::Result<T, ClientError>;

// ──────────────────────────────────────────── Query parameters ────────────────

/// Parameters for `GET /addresses/{address}/transactions`.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct AddressTxsParams {
    /// Inclusive lower bound: `"<block_height>"` or `"<block_height>:<tx_index>"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// Inclusive upper bound: same format as `from`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// `"asc"` (default) or `"desc"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<&'static str>,
    /// 1–100, default 100.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u8>,
    /// Page number, default 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
}

impl AddressTxsParams {
    pub fn asc_from(slot: u64) -> Self {
        Self {
            from: Some(slot.to_string()),
            order: Some("asc"),
            ..Default::default()
        }
    }

    fn to_query_string(&self) -> String {
        let qs = serde_urlencoded::to_string(self).unwrap_or_default();
        // HACK: serde_urlencoded percent-encodes ':' in `from`/`to` values
        // like "block_height:tx_index". Blockfrost expects the literal colon.
        // Safe because no other field value contains a colon.
        let qs = qs.replace("%3A", ":");
        if qs.is_empty() {
            String::new()
        } else {
            format!("?{qs}")
        }
    }
}

// ─────────────────────────────────────────────────── Response types ───────────

/// One entry from `GET /addresses/{address}/transactions`.
#[serde_as]
#[derive(Debug, Clone, Deserialize)]
pub struct AddressTx {
    #[serde_as(as = "Hex")]
    pub tx_hash: Hash32,
    pub tx_index: u32,
    pub block_height: u64,
    pub block_time: u64,
}

/// Response from `GET /blocks/slot/{slot}`.
/// Only the fields we need; Blockfrost returns many more.
#[serde_as]
#[derive(Debug, Clone, Deserialize)]
pub struct SlotBlock {
    #[serde_as(as = "Hex")]
    pub hash: Hash32,
    pub slot: u64,
    pub height: u64,
    pub time: u64,
}

/// Response from `GET /txs/{hash}/cbor`.
#[serde_as]
#[derive(Debug, Deserialize)]
struct TxCborResponse {
    #[serde_as(as = "Hex")]
    cbor: Vec<u8>,
}

// ─────────────────────────────────────────────────── Error response ───────────

#[derive(Debug, Deserialize)]
struct ErrorBody {
    #[allow(dead_code)]
    status_code: u16,
    error: String,
    message: String,
}

// ─────────────────────────────────────────────────────── Client ───────────────

#[derive(Debug, Clone)]
pub struct BlockfrostClient {
    http: reqwest::Client,
    base_url: &'static str,
}

impl BlockfrostClient {
    pub fn new(project_id: &str, network: Network) -> Self {
        let base_url = match network {
            Network::Mainnet => "https://cardano-mainnet.blockfrost.io/api/v0",
            Network::Preprod => "https://cardano-preprod.blockfrost.io/api/v0",
            Network::Preview => "https://cardano-preview.blockfrost.io/api/v0",
        };
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "project_id",
            reqwest::header::HeaderValue::from_str(project_id).expect("project_id must be ASCII"),
        );
        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .expect("failed to build HTTP client");
        Self { http, base_url }
    }

    // ── Endpoints ─────────────────────────────────────────────────────────────

    /// `GET /addresses/{address}/transactions`
    ///
    /// Returns up to 100 transactions per call (one page). Use `params.page`
    /// to paginate, or `params.from` to anchor near the current tip.
    pub async fn address_transactions(
        &self,
        address: &str,
        params: &AddressTxsParams,
    ) -> Result<Vec<AddressTx>> {
        let url = format!(
            "{}/addresses/{}/transactions{}",
            self.base_url,
            address,
            params.to_query_string()
        );
        self.get(&url).await
    }

    pub async fn block_at_height(&self, height: u64) -> Result<SlotBlock> {
        let url = format!("{}/blocks/{}", self.base_url, height);
        self.get(&url).await
    }

    /// `GET /txs/{hash}/cbor`
    pub async fn tx_cbor(&self, tx_hash: &Hash32) -> Result<Vec<u8>> {
        let url = format!("{}/txs/{}/cbor", self.base_url, hex::encode(tx_hash));
        let resp: TxCborResponse = self.get(&url).await?;
        Ok(resp.cbor)
    }

    // ── HTTP ──────────────────────────────────────────────────────────────────

    async fn get<T: for<'de> Deserialize<'de>>(&self, url: &str) -> Result<T> {
        let response = self.http.get(url).send().await?;
        let status = response.status().as_u16();

        match status {
            200 => {
                let bytes = response.bytes().await?;
                Ok(serde_json::from_slice(&bytes)?)
            }
            404 => Err(ClientError::NotFound {
                url: url.to_owned(),
            }),
            429 => Err(ClientError::RateLimited),
            _ => {
                // try to extract blockfrost error body for context
                let body = response.text().await.unwrap_or_default();
                let msg = serde_json::from_str::<ErrorBody>(&body)
                    .map(|e| format!("{}: {}", e.error, e.message))
                    .unwrap_or(body);
                tracing::error!(status, url, error = %msg, "blockfrost error");
                Err(ClientError::Http {
                    status,
                    url: url.to_owned(),
                })
            }
        }
    }
}
