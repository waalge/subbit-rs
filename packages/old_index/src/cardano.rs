use cardano_sdk::{
    Credential, Hash, Input, Transaction,
    cbor::{self, FromCbor},
    transaction::state::ReadyForSigning,
};
use subbit_core::{Redeemer, Stage, Step};
use tracing::info;

pub type Address = String;

pub type Hash32 = [u8; 32];
pub type Hash28 = [u8; 28];

// ─────────────────────────────────────────────── Point ─────────────────

/// How far back to sync from.
#[derive(Debug, Clone)]
pub enum Point {
    /// By specific slot + block hash.
    Slot { slot: u64, hash: String },
    /// By calendar date (converted to approximate slot at runtime).
    Date(chrono::NaiveDate),
    ///  Chain tip
    Tip,
}

impl std::str::FromStr for Point {
    type Err = PointError;

    fn from_str(s: &str) -> Result<Self, PointError> {
        if s.eq_ignore_ascii_case("tip") {
            return Ok(Self::Tip);
        }
        if let Some(rest) = s.strip_prefix("slot:") {
            let (slot, hash) = rest.split_once(':').ok_or(PointError::InvalidSlotFormat)?;
            return Ok(Self::Slot {
                slot: slot.parse().map_err(PointError::InvalidSlotNumber)?,
                hash: hash.to_owned(),
            });
        }
        if let Ok(date) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
            return Ok(Self::Date(date));
        }
        Err(PointError::Unrecognised(s.to_owned()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PointError {
    #[error("slot format must be slot:<number>:<block-hash>")]
    InvalidSlotFormat,

    #[error("slot number is not a valid integer: {0}")]
    InvalidSlotNumber(#[source] std::num::ParseIntError),

    #[error("unrecognised start point: {0:?} — expected: tip | <YYYY-MM-DD> | slot:<n>:<hash>")]
    Unrecognised(String),
}

pub fn parse_txs(
    bytes: &Vec<Vec<u8>>,
) -> Result<Vec<Transaction<ReadyForSigning>>, cardano_sdk::cbor::decode::Error> {
    bytes
        .into_iter()
        .map(parse_tx)
        .collect::<Result<Vec<_>, _>>()
}

pub fn parse_tx(
    bytes: &Vec<u8>,
) -> Result<Transaction<ReadyForSigning>, cardano_sdk::cbor::decode::Error> {
    cbor::decode(bytes)
}
