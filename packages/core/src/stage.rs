use crate::{Constants, Duration, Hash28};

#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "test-utils", derive(proptest_derive::Arbitrary))]
pub enum Stage {
    Opened {
        constants: Constants,
        subbed: u64,
    },
    Closed {
        constants: Constants,
        subbed: u64,
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

    pub fn constants(&self) -> Option<&Constants> {
        match self {
            Stage::Opened { constants, .. } => Some(constants),
            Stage::Closed { constants, .. } => Some(constants),
            Stage::Settled { .. } => None,
        }
    }

    pub fn label(&self) -> &'static str {
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
            Stage::Opened { constants, subbed } => {
                e.tag(minicbor::data::Tag::new(121))?;
                e.begin_array()?;
                e.encode_with(constants, ctx)?;
                e.encode_with(subbed, ctx)?;
                e.end()?;
            }
            Stage::Closed {
                constants,
                subbed,
                elapse_at,
            } => {
                e.tag(minicbor::data::Tag::new(122))?;
                e.begin_array()?;
                e.encode_with(constants, ctx)?;
                e.encode_with(subbed, ctx)?;
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
                let subbed: u64 = d.decode_with(ctx)?;
                d.skip()?;
                Ok(Stage::Opened { constants, subbed })
            }
            122 => {
                let constants: Constants = d.decode_with(ctx)?;
                let subbed: u64 = d.decode_with(ctx)?;
                let elapse_at: Duration = d.decode_with(ctx)?;
                d.skip()?;
                Ok(Stage::Closed {
                    constants,
                    subbed,
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
