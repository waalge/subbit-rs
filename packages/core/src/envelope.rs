use minicbor::{Decode, Encode};

use crate::{Auth, Duration, Iou, Mac, Tag, VerifyingKey, tbs};

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

pub type Response = Result<Status, Error>;

// Available amount to spend is: iou.amount - spent
#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct Status {
    #[n(0)]
    pub iou: Iou,
    #[n(1)]
    pub spendable: Option<u64>,
    #[n(2)]
    pub uncommitted: Option<u64>,
    #[n(3)]
    pub mac: Option<Mac<Body>>,
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum Error {
    #[n(0)]
    Other,
    #[n(1)]
    NoChannel,
    #[n(2)]
    Inactive,
    #[n(3)]
    OldAuth,
    #[n(4)]
    InvalidAuth,
    #[n(5)]
    NoIou,
    #[n(6)]
    OldIou,
    #[n(7)]
    InvalidIou,
    // Insufficient funds, or other reason
    #[n(8)]
    Unspendable,
    #[n(9)]
    RateLimited,
}
