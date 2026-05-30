use core::fmt;
use core::str::FromStr;
use minicbor::{Decode, Encode};

use crate::prelude::*;

// =========================================================================
// VerifyingKey
// =========================================================================

/// Ed25519 verification key (public key), 32 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
#[cfg_attr(feature = "test-utils", derive(proptest_derive::Arbitrary))]
#[cbor(transparent)]
pub struct VerifyingKey(#[cbor(with = "minicbor::bytes")] [u8; 32]);

impl VerifyingKey {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn to_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Display for VerifyingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&hex::encode(self.0))
    }
}

impl FromStr for VerifyingKey {
    type Err = hex::FromHexError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = hex::decode(s)?;
        bytes
            .try_into()
            .map_err(|_| hex::FromHexError::InvalidStringLength)
    }
}

impl AsRef<[u8]> for VerifyingKey {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<[u8; 32]> for VerifyingKey {
    fn from(b: [u8; 32]) -> Self {
        Self(b)
    }
}

impl From<VerifyingKey> for [u8; 32] {
    fn from(k: VerifyingKey) -> Self {
        k.0
    }
}

impl TryFrom<&[u8]> for VerifyingKey {
    type Error = core::array::TryFromSliceError;
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        <[u8; 32]>::try_from(value).map(Self)
    }
}

impl TryFrom<Vec<u8>> for VerifyingKey {
    type Error = Vec<u8>;
    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        <[u8; 32]>::try_from(value).map(Self)
    }
}
