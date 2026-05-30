use crate::prelude::Vec;

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub enum Currency {
    Ada,
    ByHash { hash: [u8; 28] },
    ByClass { hash: [u8; 28], name: Vec<u8> },
}

impl Currency {
    pub fn is_ada(&self) -> bool {
        matches!(self, Currency::Ada)
    }

    pub fn label(&self) -> &str {
        match self {
            Currency::Ada => "Ada",
            Currency::ByHash { .. } => "ByHash",
            Currency::ByClass { .. } => "ByClass",
        }
    }
}

impl<C> minicbor::Encode<C> for Currency {
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut minicbor::Encoder<W>,
        _ctx: &mut C,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        match self {
            Currency::Ada => {
                e.tag(minicbor::data::Tag::new(121))?;
                e.begin_array()?;
                e.end()?;
            }
            Currency::ByHash { hash } => {
                e.tag(minicbor::data::Tag::new(122))?;
                e.begin_array()?;
                e.bytes(hash)?;
                e.end()?;
            }
            Currency::ByClass { hash, name } => {
                e.tag(minicbor::data::Tag::new(123))?;
                e.begin_array()?;
                e.bytes(hash)?;
                e.bytes(name)?;
                e.end()?;
            }
        }
        Ok(())
    }
}

impl<'b, C> minicbor::Decode<'b, C> for Currency {
    fn decode(
        d: &mut minicbor::Decoder<'b>,
        _ctx: &mut C,
    ) -> Result<Self, minicbor::decode::Error> {
        let cbor_tag = d.tag()?;
        d.array()?;

        match cbor_tag.as_u64() {
            121 => {
                d.skip()?;
                Ok(Currency::Ada)
            }
            122 => {
                let hash = d
                    .bytes()?
                    .try_into()
                    .map_err(|_| minicbor::decode::Error::message("expected 28-byte hash"))?;
                d.skip()?;
                Ok(Currency::ByHash { hash })
            }
            123 => {
                let hash = d
                    .bytes()?
                    .try_into()
                    .map_err(|_| minicbor::decode::Error::message("expected 28-byte hash"))?;
                let name = d.bytes()?.to_vec();
                d.skip()?;
                Ok(Currency::ByClass { hash, name })
            }
            _ => Err(minicbor::decode::Error::message("unknown Currency tag")),
        }
    }
}

#[cfg(feature = "test-utils")]
impl proptest::arbitrary::Arbitrary for Currency {
    type Parameters = ();
    type Strategy = proptest::strategy::BoxedStrategy<Self>;

    fn arbitrary_with(_: ()) -> Self::Strategy {
        use proptest::prelude::*;
        prop_oneof![
            Just(Currency::Ada),
            any::<[u8; 28]>().prop_map(|hash| Currency::ByHash { hash }),
            (
                any::<[u8; 28]>(),
                proptest::collection::vec(any::<u8>(), 1..=32)
            )
                .prop_map(|(hash, name)| Currency::ByClass { hash, name }),
        ]
        .boxed()
    }
}
