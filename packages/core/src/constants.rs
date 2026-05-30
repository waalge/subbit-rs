use crate::{Currency, Duration, Hash28, Tag, VerifyingKey};

// pub type Constants {
//   tag: Tag,
//   currency: Currency,
//   iou_key: VerificationKey,
//   consumer: VerificationKeyHash,
//   provider: VerificationKeyHash,
//   close_period: Int,
// }


#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub struct Constants {
    pub tag: Tag,
    pub currency: Currency,
    pub iou_key: VerifyingKey,
    pub consumer: Hash28,
    pub provider: Hash28,
    pub close_period: Duration,
}

impl Constants {
    pub fn verify(&self, max_tag_length: usize, min_close_period: u64) -> bool {
        self.tag.len() <= max_tag_length
            && self.close_period.as_millis() >= min_close_period as u128
    }
}

impl<C> minicbor::Encode<C> for Constants {
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut minicbor::Encoder<W>,
        ctx: &mut C,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        e.tag(minicbor::data::Tag::new(121))?;
        e.begin_array()?;
        e.encode_with(&self.tag, ctx)?;
        e.encode_with(&self.currency, ctx)?;
        e.encode_with(&self.iou_key, ctx)?;
        e.encode_with(&self.consumer, ctx)?;
        e.encode_with(&self.provider, ctx)?;
        e.encode_with(&self.close_period, ctx)?;
        e.end()?;
        Ok(())
    }
}

impl<'b, C> minicbor::Decode<'b, C> for Constants {
    fn decode(d: &mut minicbor::Decoder<'b>, ctx: &mut C) -> Result<Self, minicbor::decode::Error> {
        let tag = d.tag()?;
        if tag.as_u64() != 121 {
            return Err(minicbor::decode::Error::message(
                "expected CBOR tag 121 for Constants",
            ));
        }
        d.array()?;
        let tag: _ = d.decode_with(ctx)?;
        let currency: _ = d.decode_with(ctx)?;
        let iou_key: _ = d.decode_with(ctx)?;
        let consumer: _ = d.decode_with(ctx)?;
        let provider: _ = d.decode_with(ctx)?;
        let close_period: _ = d.decode_with(ctx)?;
        d.skip()?;
        Ok(Self {
            tag,
            currency,
            iou_key,
            consumer,
            provider,
            close_period,
        })
    }
}
