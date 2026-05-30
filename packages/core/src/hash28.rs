use minicbor::{Decode, Encode};
use core::fmt;
use core::str::FromStr;

use crate::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
#[cfg_attr(feature = "test-utils", derive(proptest_derive::Arbitrary))]
#[cbor(transparent)]
pub struct Hash28(#[cbor(with = "minicbor::bytes")] [u8; 28]);

impl Hash28 {
    pub fn new(bytes: [u8; 28]) -> Self {
        Self(bytes)
    }

    pub fn to_bytes(self) -> [u8; 28] {
        self.0
    }
}

impl fmt::Display for Hash28 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&hex::encode(self.0))
    }
}

impl FromStr for Hash28 {
    type Err = hex::FromHexError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = hex::decode(s)?;
        bytes.try_into().map_err(|_| hex::FromHexError::InvalidStringLength)
    }
}

impl AsRef<[u8]> for Hash28 {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<[u8; 28]> for Hash28 {
    fn from(b: [u8; 28]) -> Self { Self(b) }
}

impl From<Hash28> for [u8; 28] {
    fn from(k: Hash28) -> Self { k.0 }
}

impl TryFrom<&[u8]> for Hash28 {
    type Error = core::array::TryFromSliceError;
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        <[u8; 28]>::try_from(value).map(Self)
    }
}

impl TryFrom<Vec<u8>> for Hash28 {
    type Error = Vec<u8>;
    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        <[u8; 28]>::try_from(value).map(Self)
    }
}

