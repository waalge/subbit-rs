use minicbor::{Decode, Encode};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{Tag, VerifyingKey};

/// The account identity
///
/// `key` and `tag` together identify the account.
/// We could just concat, but for consistency, we don't.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "test-utils", derive(proptest_derive::Arbitrary))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Account {
    #[n(0)]
    pub key: VerifyingKey,
    #[n(1)]
    pub tag: Tag,
}
