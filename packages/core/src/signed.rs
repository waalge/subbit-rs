use minicbor::{Decode, Encode};

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct Signed<B, S> {
    #[n(0)]
    pub body: B,
    #[n(1)]
    pub sig: S,
}
