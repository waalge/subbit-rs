use crate::{Constants, Duration, Hash28};

#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "test-utils", derive(proptest_derive::Arbitrary))]
pub enum Stage {
    Opened {
        constants: Constants,
        amount: u64,
    },
    Closed {
        constants: Constants,
        amount: u64,
        elapse_at: Duration,
    },
    Settled {
        consumer: Hash28,
    },
}

impl Stage {
    pub fn is_opened(&self) -> bool {
        matches!(self, Stage::Opened { .. })
    }

    pub fn label(&self) -> &str {
        match self {
            Stage::Opened { .. } => "Opened",
            Stage::Closed { .. } => "Closed",
            Stage::Settled { .. } => "Settled",
        }
    }
}

impl<C> minicbor::Encode<C> for Stage {
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut minicbor::Encoder<W>,
        ctx: &mut C,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        match self {
            Stage::Opened { constants, amount } => {
                e.tag(minicbor::data::Tag::new(121))?;
                e.begin_array()?;
                e.encode_with(constants, ctx)?;
                e.encode_with(amount, ctx)?;
                e.end()?;
            }
            Stage::Closed {
                constants,
                amount,
                elapse_at,
            } => {
                e.tag(minicbor::data::Tag::new(122))?;
                e.begin_array()?;
                e.encode_with(constants, ctx)?;
                e.encode_with(amount, ctx)?;
                e.encode_with(elapse_at, ctx)?;
                e.end()?;
            }
            Stage::Settled { consumer } => {
                e.tag(minicbor::data::Tag::new(123))?;
                e.begin_array()?;
                e.encode_with(consumer, ctx)?;
                e.end()?;
            }
        }
        Ok(())
    }
}

impl<'b, C> minicbor::Decode<'b, C> for Stage {
    fn decode(d: &mut minicbor::Decoder<'b>, ctx: &mut C) -> Result<Self, minicbor::decode::Error> {
        let cbor_tag = d.tag()?;
        d.array()?;
        match cbor_tag.as_u64() {
            121 => {
                let constants: Constants = d.decode_with(ctx)?;
                let amount: u64 = d.decode_with(ctx)?;
                d.skip()?;
                Ok(Stage::Opened { constants, amount })
            }
            122 => {
                let constants: Constants = d.decode_with(ctx)?;
                let amount: u64 = d.decode_with(ctx)?;
                let elapse_at: Duration = d.decode_with(ctx)?;
                d.skip()?;
                Ok(Stage::Closed {
                    constants,
                    amount,
                    elapse_at,
                })
            }
            123 => {
                let consumer: Hash28 = d.decode_with(ctx)?;
                d.skip()?;
                Ok(Stage::Settled { consumer })
            }
            _ => Err(minicbor::decode::Error::message(
                "unknown Stage CBOR tag; expected 121, 122, or 123",
            )),
        }
    }
}
