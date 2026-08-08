use minicbor::{Decode, Encode};

use crate::{Auth, Duration, Iou, Tag, VerifyingKey, tbs};

/// Basic body
#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct Body {
    #[n(0)]
    pub key: VerifyingKey,
    #[n(1)]
    pub tag: Tag,
    #[n(2)]
    pub ttl: Duration,
}

impl Body {
    pub fn tbs(&self) -> Vec<u8> {
        tbs::subbit(self)
    }
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct Request {
    #[n(0)]
    pub auth: Auth<Body, Body>,
    #[n(1)]
    pub iou: Option<Iou>,
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum Response {
    #[n(0)]
    Ok(#[n(0)] Status),
    #[n(1)]
    Ko(#[n(0)] Error),
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct Status {
    #[n(0)]
    pub committed: Option<u64>,
    #[n(1)]
    pub spent: Option<u64>,
    #[n(2)]
    pub uncommitted: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum Error {
    #[n(0)]
    Other,
    #[n(1)]
    NoAccount,
    #[n(2)]
    Inactive,
    #[n(3)]
    NoAuth,
    #[n(4)]
    InvalidInput,
    #[n(5)]
    InsufficientFunds,
}
