use std::collections::BTreeMap;

use cardano_sdk::{Address, Credential, Hash, Value, address::kind, cbor::ToCbor};
use subbit_core::{Constants, Currency, Duration, Stage};

// ----------------------------------------------------------------------------
// Constants
// ----------------------------------------------------------------------------

// FIXME: derive from plutus.json at build time
fn subbit_hash() -> [u8; 28] {
    let hex = std::env::var("SUBBIT_HASH")
        .unwrap_or_else(|_| "f745370aed2791109b453e81ced6b818395bca21fc8ee55fd21bad95".to_string());
    hex::decode(hex)
        .expect("SUBBIT_HASH: invalid hex")
        .try_into()
        .expect("SUBBIT_HASH: must be 28 bytes")
}

// FIXME: derive from tx builder or core
const MIN_ADA_BUFFER: u64 = 2_000_000;

// ----------------------------------------------------------------------------
// Types
// ----------------------------------------------------------------------------

/// A script output at this validator address.
#[derive(Debug, Clone)]
pub struct Output {
    fixed: Fixed,
    variable: Variable,
}

/// We have ~problem. Constants are not strictly constant
#[derive(Debug, Clone)]
pub struct Fixed {
    delegation: Option<Credential>,
    constants: Constants,
}

/// A script output at this validator address.
#[derive(Debug, Clone)]
pub struct Variable {
    index: usize,
    amount: u64,
    stage: StageV,
}

/// Reduced stage.
/// Maybe its what stage should actually be
#[derive(Debug, Clone)]
pub enum StageV {
    Opened { subbed: u64 },
    Closed { subbed: u64, elapse_at: Duration },
    Settled,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum OutputError {
    #[error("Bad address")]
    Address(#[from] AddressError),
    #[error("Bad value")]
    Value(#[from] ValueError),
    #[error("Datum")]
    Datum(#[from] DatumError),
    #[error("Script Ref")]
    ScriptRef,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum AddressError {
    #[error("Must be Shelley Era.")]
    Era,
    #[error("Payment credential must be script")]
    Script,
    #[error("Wrong hash")]
    Hash,
}

pub fn validate_address(
    script_hash: Hash<28>,
    address: &Address<kind::Any>,
) -> Result<Option<Credential>, AddressError> {
    let address = address.as_shelley().ok_or(AddressError::Era)?;
    let payment = address.payment().as_script().ok_or(AddressError::Script)?;
    if payment != script_hash {
        return Err(AddressError::Hash);
    }
    Ok(address.delegation())
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum DatumError {
    #[error("Must has a datum")]
    None,
    #[error("Must be an inline datum")]
    Inline,
    #[error("Failed to parse cbor into Subbit datum")]
    Parse,
    #[error("`own_hash` must match expected value")]
    Hash,
}

pub fn validate_datum(
    script_hash: Hash<28>,
    datum: Option<&cardano_sdk::Datum>,
) -> Result<Stage, DatumError> {
    let datum = datum.ok_or(DatumError::None)?;
    let cardano_sdk::Datum::Inline(datum) = datum else {
        return Err(DatumError::Inline);
    };
    let datum =
        minicbor::decode::<subbit_core::Datum>(&datum.to_cbor()).map_err(|_| DatumError::Parse)?;
    if datum.own_hash.to_bytes() != <[u8; 28]>::from(script_hash) {
        return Err(DatumError::Hash);
    }
    Ok(datum.stage)
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ValueError {
    #[error("Must have at most Ada and channel currency")]
    Foreign,
    #[error("Malformed value")]
    Form,
}

impl From<SingletonError> for ValueError {
    fn from(_value: SingletonError) -> Self {
        ValueError::Foreign
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum SingletonError {
    #[error("Must have at most one key")]
    Singleton,
}

pub fn singleton<K, V>(val: &BTreeMap<K, V>) -> Result<Option<(&K, &V)>, SingletonError> {
    match val.len() {
        0 => Ok(None),
        1 => Ok(val.iter().next()),
        _ => Err(SingletonError::Singleton),
    }
}

pub fn single_asset<Quantity: Clone>(
    assets: &BTreeMap<Hash<28>, BTreeMap<Vec<u8>, Quantity>>,
) -> Result<Option<(Hash<28>, Vec<u8>, Quantity)>, ValueError> {
    match singleton(assets)? {
        Some((hash, names)) => {
            let (name, amount) = singleton(names)?.ok_or(ValueError::Form)?;
            Ok(Some((hash.clone(), name.clone(), amount.clone())))
        }
        None => Ok(None),
    }
}

pub fn validate_value(currency: &Currency, val: &Value<u64>) -> Result<u64, ValueError> {
    match currency {
        subbit_core::Currency::Ada => {
            if !val.assets().is_empty() {
                return Err(ValueError::Foreign);
            }
            Ok(val.lovelace().saturating_sub(MIN_ADA_BUFFER))
        }
        subbit_core::Currency::ByHash { hash } => match single_asset(val.assets())? {
            Some((got_hash, _, amount)) => {
                if <[u8; 28]>::from(hash.clone()) != <[u8; 28]>::from(got_hash) {
                    return Err(ValueError::Foreign);
                }
                Ok(amount)
            }
            None => Ok(0),
        },
        subbit_core::Currency::ByClass { hash, name } => match single_asset(val.assets())? {
            Some((got_hash, got_name, amount)) => {
                if <[u8; 28]>::from(hash.clone()) != <[u8; 28]>::from(got_hash) || got_name != *name
                {
                    return Err(ValueError::Foreign);
                }
                Ok(amount)
            }
            None => Ok(0),
        },
    }
}

impl TryFrom<cardano_sdk::Output> for Output {
    type Error = OutputError;

    fn try_from(value: cardano_sdk::Output) -> Result<Self, Self::Error> {
        let subbit_hash = Hash::<28>::from(subbit_hash());
        let delegation = validate_address(subbit_hash, value.address())?;
        let stage = validate_datum(subbit_hash, value.datum())?;
        let amount = match stage.constants() {
            Some(constants) => validate_value(constants.currency(), value.value())?,
            None => 0,
        };
        if value.script().is_some() {
            return Err(OutputError::ScriptRef);
        }
        ///
        let fixed = Fixed {
            delegation,
            constants,
        };
        Some(Output { fixed })
    }
}
