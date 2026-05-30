use crate::Signature;

#[derive(Debug, Clone, PartialEq, PartialOrd, Ord, Eq)]
#[cfg_attr(feature = "test-utils", derive(proptest_derive::Arbitrary))]
pub struct Iou {
    amount: u64,
    signature: Signature,
}

impl Iou {
    pub fn new(amount: u64, signature: Signature) -> Self {
        Self { amount, signature }
    }

    pub fn amount(&self) -> u64 {
        self.amount
    }

    pub fn signature(&self) -> &Signature {
        &self.signature
    }
}

impl<C> minicbor::Encode<C> for Iou {
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut minicbor::Encoder<W>,
        ctx: &mut C,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        e.tag(minicbor::data::Tag::new(121))?;
        e.begin_array()?;
        e.encode_with(self.amount, ctx)?;
        e.encode_with(&self.signature, ctx)?;
        e.end()?;
        Ok(())
    }
}

impl<'b, C> minicbor::Decode<'b, C> for Iou {
    fn decode(d: &mut minicbor::Decoder<'b>, ctx: &mut C) -> Result<Self, minicbor::decode::Error> {
        let tag = d.tag()?;
        if tag.as_u64() != 121 {
            return Err(minicbor::decode::Error::message(
                "expected CBOR tag 121 for Iou",
            ));
        }
        d.array()?;
        let amount: u64 = d.decode_with(ctx)?;
        let signature: Signature = d.decode_with(ctx)?;
        if d.datatype()? != minicbor::data::Type::Break {
            return Err(minicbor::decode::Error::message("expected end of array"));
        }
        d.skip()?;
        Ok(Self { amount, signature })
    }
}
