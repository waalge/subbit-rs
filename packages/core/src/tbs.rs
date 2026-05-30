use crate::Tag;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "test-utils", derive(proptest_derive::Arbitrary))]
pub struct Tbs {
    tag: Tag,
    amount: u64,
}

impl Tbs {
    pub fn new(tag: Tag, amount: u64) -> Self {
        Self { tag, amount }
    }

    pub fn tag(&self) -> &Tag {
        &self.tag
    }

    pub fn amount(&self) -> u64 {
        self.amount
    }
}

impl<C> minicbor::Encode<C> for Tbs {
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut minicbor::Encoder<W>,
        ctx: &mut C,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        e.begin_array()?;
        e.encode_with(&self.tag, ctx)?;
        e.encode_with(self.amount, ctx)?;
        e.end()?;
        Ok(())
    }
}

impl<'b, C> minicbor::Decode<'b, C> for Tbs {
    fn decode(d: &mut minicbor::Decoder<'b>, ctx: &mut C) -> Result<Self, minicbor::decode::Error> {
        d.array()?;
        let tag: Tag = d.decode_with(ctx)?;
        let amount: u64 = d.decode_with(ctx)?;
        if d.datatype()? != minicbor::data::Type::Break {
            return Err(minicbor::decode::Error::message("expected end of array"));
        }
        d.skip()?;
        Ok(Self { tag, amount })
    }
}

#[cfg(feature = "test-utils")]
mod tests {
    #[allow(unused_imports)]
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn tbs_roundtrip(tbs in any::<Tbs>()) {
            let encoded = minicbor::to_vec(&tbs).unwrap();
            let decoded: Tbs = minicbor::decode(&encoded).unwrap();
            prop_assert_eq!(decoded.tag().as_ref(), tbs.tag().as_ref());
            prop_assert_eq!(decoded.amount(), tbs.amount());
        }

        #[test]
        fn tbs_encoding_is_deterministic(tbs in any::<Tbs>()) {
            let a = minicbor::to_vec(&tbs).unwrap();
            let b = minicbor::to_vec(&tbs).unwrap();
            prop_assert_eq!(a, b);
        }

        #[test]
        fn tbs_encodes_as_indefinite_array(tbs in any::<Tbs>()) {
            let encoded = minicbor::to_vec(&tbs).unwrap();
            prop_assert_eq!(encoded[0], 0x9f);
            prop_assert_eq!(*encoded.last().unwrap(), 0xff);
        }
    }
}
