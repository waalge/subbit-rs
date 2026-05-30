use crate::{Hash28, Stage};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Datum {
    pub own_hash: Hash28,
    pub stage: Stage,
}

impl Datum {
    pub fn new(own_hash: Hash28, stage: Stage) -> Self {
        Self {
            own_hash,
            stage,
        }
    }
}

impl<C> minicbor::Encode<C> for Datum
{
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut minicbor::Encoder<W>,
        ctx: &mut C,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        e.begin_array()?;
        e.encode_with(&self.own_hash, ctx)?;
        e.encode_with(&self.stage, ctx)?;
        e.end()?;
        Ok(())
    }
}

impl<'b, C> minicbor::Decode<'b, C> for Datum
{
    fn decode(d: &mut minicbor::Decoder<'b>, ctx: &mut C) -> Result<Self, minicbor::decode::Error> {
        d.array()?;
        let own_hash: Hash28 = d.decode_with(ctx)?;
        let stage: Stage = d.decode_with(ctx)?;
        d.skip()?;
        Ok(Self { own_hash, stage })
    }
}
