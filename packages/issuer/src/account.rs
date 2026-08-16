use ed25519_dalek::{Signer as _, SigningKey};

use subbit_core::{Duration, Iou, Pop, Signature, Tag, TagTbs, VerifyingKey, envelope::Body};

#[derive(minicbor::Encode, minicbor::Decode)]
pub struct Account {
    #[n(0)]
    #[cbor(
        encode_with = "signing_key_cbor::encode",
        decode_with = "signing_key_cbor::decode"
    )]
    key: SigningKey,
    #[n(1)]
    tag: Tag,
}

impl Account {
    pub fn new(key: impl Into<SigningKey>, tag: impl Into<Tag>) -> Self {
        Self {
            key: key.into(),
            tag: tag.into(),
        }
    }

    fn sign(&self, msg: &[u8]) -> Signature {
        self.key.sign(msg).to_bytes().into()
    }

    fn verifying_key(&self) -> VerifyingKey {
        subbit_core::VerifyingKey::from(self.key.verifying_key().to_bytes())
    }

    pub fn pop(&self, ttl: Duration) -> Pop<Body> {
        let body = Body {
            key: self.verifying_key(),
            tag: self.tag.clone(),
            ttl,
        };
        let sig = self.sign(&body.tbs());
        Pop { body, sig }
    }

    fn tag_tbs(&self, amount: u64) -> TagTbs {
        TagTbs::new(self.tag.clone(), amount)
    }

    pub fn iou(&self, amount: u64) -> Iou {
        let sig = self.sign(&self.tag_tbs(amount).to_vec());
        Iou::new(amount, sig)
    }

    pub fn verify_iou(&self, iou: &Iou) -> bool {
        self.key
            .verify(
                &self.tag_tbs(iou.amount()).to_vec(),
                &<[u8; 64]>::from(*iou.signature()).into(),
            )
            .is_ok()
    }
}

mod signing_key_cbor {
    use ed25519_dalek::SigningKey;
    use minicbor::{Decoder, Encoder};

    pub fn encode<C, W: minicbor::encode::Write>(
        v: &SigningKey,
        e: &mut Encoder<W>,
        _ctx: &mut C,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        e.bytes(&v.to_bytes())?;
        Ok(())
    }

    pub fn decode<'b, C>(
        d: &mut Decoder<'b>,
        _ctx: &mut C,
    ) -> Result<SigningKey, minicbor::decode::Error> {
        let key: [u8; 32] = d
            .bytes()?
            .try_into()
            .map_err(|_| minicbor::decode::Error::message("signing key must be 32 bytes"))?;
        Ok(SigningKey::from_bytes(&key))
    }
}
