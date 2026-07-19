use minicbor::{Decode, Encode};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{Duration, Iou, Tag, VerifyingKey};

/// The account identity and validity window for a session.
///
/// `key` and `tag` together identify the account.
/// `ttl` is optional - omitting the ttl should result in
/// token lasting forever.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "test-utils", derive(proptest_derive::Arbitrary))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TokenBody {
    #[n(0)]
    pub key: VerifyingKey,
    #[n(1)]
    pub tag: Tag,
    #[n(2)]
    pub ttl: Option<Duration>,
}

/// The complete payload carried in one header value.
///
/// - `token` is always present.
/// - `iou` is present only when raising the claimed balance.
/// - `sig` is opaque proof over `token` — absent, MAC-length, or
///   signature-length bytes, distinguished only by the application
///   (this crate does not fix a MAC length or a signature length).
///
/// **Invariant this crate does not enforce at the type level, but that
/// every implementation must check:** `iou` and `sig` cannot both be
/// absent. With no `iou`, nothing is signed and `token` alone is a bare,
/// forgeable claim. See [`Header::is_wellformed`].
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "test-utils", derive(proptest_derive::Arbitrary))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Header {
    #[n(0)]
    pub token: TokenBody,
    #[n(1)]
    pub iou: Option<Iou>,
    #[n(2)]
    pub sig: Option<Vec<u8>>,
}

impl Header {
    /// Structural check only — not cryptographic verification.
    /// Rejects the one shape that can never be valid under any policy:
    /// no `Iou` and no `sig`, meaning nothing in the message is signed.
    pub fn is_wellformed(&self) -> bool {
        self.iou.is_some() || self.sig.is_some()
    }
}

// --- canonical wire form: base64url, no padding ---

impl std::fmt::Display for Header {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bytes = minicbor::to_vec(self).expect("Header encoding is infallible");
        f.write_str(&base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
    }
}

impl std::str::FromStr for Header {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(s)
            .map_err(|_| crate::Error::Malformed)?;
        let header: Header = minicbor::decode(&bytes).map_err(|_| crate::Error::Malformed)?;
        if !header.is_wellformed() {
            return Err(crate::Error::Malformed);
        }
        Ok(header)
    }
}
