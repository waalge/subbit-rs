use core::fmt;
use minicbor::{Decode, Encode};

use crate::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
#[cfg_attr(feature = "test-utils", derive(proptest_derive::Arbitrary))]
#[cbor(transparent)]
pub struct Signature(#[cbor(with = "minicbor::bytes")] [u8; 64]);

impl Signature {
    pub fn new(bytes: [u8; 64]) -> Self {
        Self(bytes)
    }

    pub fn to_bytes(self) -> [u8; 64] {
        self.0
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&hex::encode(self.0))
    }
}

impl AsRef<[u8]> for Signature {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<[u8; 64]> for Signature {
    fn from(b: [u8; 64]) -> Self {
        Self(b)
    }
}

impl From<Signature> for [u8; 64] {
    fn from(s: Signature) -> Self {
        s.0
    }
}

impl TryFrom<&[u8]> for Signature {
    type Error = core::array::TryFromSliceError;
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        <[u8; 64]>::try_from(value).map(Self)
    }
}

impl TryFrom<Vec<u8>> for Signature {
    type Error = Vec<u8>;
    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        <[u8; 64]>::try_from(value).map(Self)
    }
}
