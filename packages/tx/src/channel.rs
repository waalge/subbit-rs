use cardano_sdk::{Hash, Output, Value, cbor::ToCbor};
use minicbor::{Decode, Encode};
use subbit_core::{Constants, Currency, Datum, Hash28, Iou, Stage};

use crate::{
    MIN_ADA_BUFFER, VALIDATOR, Variables, iou,
    step::{self, Want, Will, WillCont, WillEol},
};

/// Data obtained from parsing a channel
#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct Channel {
    #[n(0)]
    constants: Constants,
    #[n(1)]
    variables: Variables,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum FromOutputError {
    #[error("Expect Shelley Address")]
    ShelleyAddress,
    #[error("Expect Script Payment Credential")]
    ScriptCredential,
    #[error("Expect Konduit Payment Credential")]
    KonduitCredential,
    #[error("Expect datum")]
    Datum,
    #[error("Expect Inline datum")]
    Inline,
    #[error("Failed to parse datum")]
    ParseDatum,
    #[error("Own hash is wrong")]
    OwnHash,
}

impl TryFrom<&Output> for Channel {
    type Error = FromOutputError;

    fn try_from(output: &Output) -> Result<Self, Self::Error> {
        let Some(address) = output.address().as_shelley() else {
            return Err(FromOutputError::ShelleyAddress);
        };
        let Some(hash) = address.payment().as_script() else {
            return Err(FromOutputError::ScriptCredential);
        };
        if hash != VALIDATOR.hash {
            return Err(FromOutputError::KonduitCredential);
        }
        let Some(datum) = output.datum() else {
            return Err(FromOutputError::Datum);
        };
        let cardano_sdk::Datum::Inline(data) = datum else {
            return Err(FromOutputError::Inline);
        };
        let Datum {
            own_hash,
            constants,
            stage,
        } = minicbor::decode(&data.to_cbor()).map_err(|_| FromOutputError::ParseDatum)?;
        if <[u8; 28]>::from(own_hash) != <[u8; 28]>::from(VALIDATOR.hash) {
            return Err(FromOutputError::OwnHash);
        }
        let value = output.value();
        let amount = match constants.currency() {
            Currency::Ada => value.lovelace().saturating_sub(MIN_ADA_BUFFER),
            Currency::Asset { hash, name } => value
                .assets()
                .get(&Hash::<28>::from(*hash))
                .and_then(|t| t.get(name))
                .copied()
                .unwrap_or(0),
        };
        let variables = Variables::new(amount, stage);
        Ok(Self {
            constants,
            variables,
        })
    }
}

impl Channel {
    pub fn new(constants: Constants, variables: Variables) -> Self {
        Self {
            constants,
            variables,
        }
    }

    pub fn constants(&self) -> &Constants {
        &self.constants
    }

    pub fn variables(&self) -> &Variables {
        &self.variables
    }

    /// The buffered amount, expressed as a `Value` in this channel's currency.
    pub fn buffered_value(&self) -> Value<u64> {
        let amount = self.variables().amount();
        if let Currency::Asset { hash, name } = self.constants().currency() {
            Value::new(MIN_ADA_BUFFER)
                .with_assets([(hash.clone().into(), [(name.clone(), amount)])])
        } else {
            Value::new(amount + MIN_ADA_BUFFER)
        }
    }

    pub fn datum(&self) -> Datum {
        Datum {
            own_hash: Hash28::from(<[u8; 28]>::from(VALIDATOR.hash)),
            constants: self.constants.clone(),
            stage: self.variables.stage().clone(),
        }
    }

    pub fn can(&self) -> Vec<crate::step::Can> {
        use crate::step::Can;
        match self.variables.stage() {
            Stage::Opened { .. } => {
                let available = self.variables.amount();
                vec![Can::Add, Can::Sub { available }, Can::Close]
            }
            Stage::Closed { elapse_at, .. } => {
                let available = self.variables.amount();
                vec![
                    Can::Settle {
                        before: *elapse_at,
                        available,
                    },
                    Can::Elapse { after: *elapse_at },
                ]
            }
            Stage::Settled => vec![Can::End],
        }
    }

    /// Runs `f` against current state; on success, replaces `variables` in
    /// place and returns whatever extra value `f` produced. On failure,
    /// `self` is handed back unchanged.
    fn try_step<F, T>(mut self, f: F) -> Result<(Self, T), (Self, step::Error)>
    where
        F: FnOnce(&Self) -> Result<(Variables, T), step::Error>,
    {
        match f(&self) {
            Ok((variables, extra)) => {
                self.variables = variables;
                Ok((self, extra))
            }
            Err(err) => Err((self, err)),
        }
    }

    fn try_verify(&self, iou: &Iou) -> Result<(), iou::Error> {
        iou::verify(self.constants.iou_key(), self.constants.tag(), &iou)
    }

    pub fn try_will(self, want: Want) -> Result<Will, (Self, step::Error)> {
        match want {
            Want::Add { amount } => {
                let (channel, ()) = self.try_step(|c| Ok((c.variables.add(amount)?, ())))?;
                Ok(Will::cont(channel, WillCont::add(amount)))
            }

            Want::Sub { iou } => {
                if let Err(err) = self.try_verify(&iou) {
                    return Err((self, err.into()));
                }
                let (channel, gain) =
                    self.try_step(|c| c.variables.sub(iou.amount()).map_err(step::Error::from))?;
                Ok(Will::cont(channel, WillCont::sub(iou, gain)))
            }

            Want::Close { upper } => {
                let (channel, ()) = self.try_step(|c| {
                    Ok((c.variables.close(&upper, c.constants.close_period())?, ()))
                })?;
                Ok(Will::cont(channel, WillCont::close(upper)))
            }

            Want::Settle { iou } => {
                if let Err(err) = self.try_verify(&iou) {
                    return Err((self, err.into()));
                }
                let (channel, gain) =
                    self.try_step(|c| c.variables.settle(iou.amount()).map_err(step::Error::from))?;
                Ok(Will::cont(channel, WillCont::settle(iou, gain)))
            }

            Want::Elapse { lower } => {
                if let Err(err) = self.variables.elapse(&lower) {
                    return Err((self, err.into()));
                }
                Ok(Will::eol(WillEol::elapse(lower)))
            }

            Want::End => {
                if let Err(err) = self.variables.end() {
                    return Err((self, err.into()));
                }
                Ok(Will::eol(WillEol::end()))
            }
        }
    }
}
