use core::fmt;
use core::str::FromStr;
use minicbor::{Decode, Encode};

use crate::prelude::*;

#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "test-utils", derive(proptest_derive::Arbitrary))]
#[cbor(transparent)]
pub struct Tag(#[cbor(with = "minicbor::bytes")] Vec<u8>);

impl Tag {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|b| write!(f, "{:02x}", b))
    }
}

impl FromStr for Tag {
    type Err = hex::FromHexError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        hex::decode(s).map(Tag)
    }
}

impl AsRef<[u8]> for Tag {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<Vec<u8>> for Tag {
    fn from(v: Vec<u8>) -> Self {
        Tag(v)
    }
}

impl From<Tag> for Vec<u8> {
    fn from(t: Tag) -> Self {
        t.0
    }
}

impl<'a> From<&'a [u8]> for Tag {
    fn from(s: &'a [u8]) -> Self {
        Tag(s.to_vec())
    }
}
