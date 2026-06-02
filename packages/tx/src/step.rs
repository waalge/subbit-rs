use std::process::Output;

use crate::channel::Channel;
use cardano_sdk::{Input, Signature, VerificationKey};
use subbit_core::{Duration, Hash28, Iou, Redeemer, Stage, Tbs};

// ---------------------------------------------------------------------------
// Can: advisory, derived from channel state, shown to user
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Can {
    Add,
    Sub { available: u64 },
    Close,
    Settle { before: Duration, available: u64 },
    End,
    Elapse { after: Duration },
}

pub fn can(channel: &Channel) -> Vec<Can> {
    match &channel.stage() {
        Stage::Opened { subbed, .. } => {
            let available = channel.amount() - subbed;
            vec![Can::Add, Can::Sub { available }, Can::Close]
        }
        Stage::Closed {
            subbed, elapse_at, ..
        } => {
            let available = channel.amount() - subbed;
            vec![
                Can::Settle {
                    before: *elapse_at,
                    available,
                },
                Can::Elapse { after: *elapse_at },
            ]
        }
        Stage::Settled { .. } => {
            vec![Can::End]
        }
    }
}

// ---------------------------------------------------------------------------
// Want: user expression, flat, unconstrained
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Want {
    Add { amount: u64 },
    Sub { iou: Iou },
    Close { upper: Duration },
    Settle { iou: Iou },
    End,
    Elapse,
}

impl Want {
    pub fn label(&self) -> &'static str {
        match self {
            Want::Add { .. } => "Add",
            Want::Sub { .. } => "Sub",
            Want::Close { .. } => "Close",
            Want::Settle { .. } => "Settle",
            Want::End => "End",
            Want::Elapse => "Elapse",
        }
    }
}

// ---------------------------------------------------------------------------
// Will: validated step, partitioned by output (Cont produces utxo, Eol does not)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Will {
    Cont { output: Channel, step: WillCont },
    Eol(WillEol),
}

#[derive(Debug, Clone)]
pub enum WillCont {
    Add,
    Sub { iou: Iou },
    Close { upper: Duration },
    Settle { iou: Iou },
}

#[derive(Debug, Clone)]
pub enum WillEol {
    End,
    Elapse { lower: Duration },
}

// ---------------------------------------------------------------------------
// Validate: per-channel correctness (Want + Channel → Will)
// ---------------------------------------------------------------------------

pub fn validate(channel: &Channel, want: Want) -> Result<Will, StepError> {
    match (&channel.stage(), want) {
        // Opened → Add, Sub, Close (all Cont)
        (Stage::Opened { constants, subbed }, Want::Add { amount }) => {
            if amount <= 1 {
                return Err(StepError::AddAmount);
            }
            let output = Channel::new(
                channel.amount() + amount,
                Stage::Opened {
                    constants: constants.clone(),
                    subbed: *subbed,
                },
            );
            let step = WillCont::Add;
            Ok(Will::Cont { output, step })
        }
        (Stage::Opened { constants, subbed }, Want::Sub { iou }) => {
            if !verify_iou(&constants.iou_key(), constants.tag(), &iou) {
                return Err(StepError::IouForm);
            }
            if iou.amount() < *subbed {
                return Err(StepError::IouStale);
            }
            let rel_owed = iou.amount() - subbed;
            if rel_owed == 0 {
                return Err(StepError::IouUsed);
            }
            let available = channel.amount();
            if available == 0 {
                return Err(StepError::NoFunds);
            }
            let sub_delta = std::cmp::min(rel_owed, available);
            let output = Channel::new(
                channel.amount() - sub_delta,
                Stage::Opened {
                    constants: constants.clone(),
                    subbed: *subbed + sub_delta,
                },
            );
            let step = WillCont::Sub { iou };
            Ok(Will::Cont { output, step })
        }
        (Stage::Opened { constants, subbed }, Want::Close { upper }) => {
            let elapse_at = upper + *constants.close_period();
            let output = Channel::new(
                channel.amount(),
                Stage::Closed {
                    constants: constants.clone(),
                    subbed: *subbed,
                    elapse_at,
                },
            );
            let step = WillCont::Close { upper };
            Ok(Will::Cont { output, step })
        }
        (
            Stage::Closed {
                constants, subbed, ..
            },
            Want::Settle { iou },
        ) => {
            if !verify_iou(&constants.iou_key(), constants.tag(), &iou) {
                return Err(StepError::IouForm);
            }
            if iou.amount() < *subbed {
                return Err(StepError::IouStale);
            }
            // Permit IouUsed & NoFunds. This is legitimate.
            let rel_owed = iou.amount() - subbed;
            let available = channel.amount();
            if available == 0 {
                return Err(StepError::NoFunds);
            }
            let sub_delta = std::cmp::min(rel_owed, available);
            let output = Channel::new(
                channel.amount() - sub_delta,
                Stage::Settled {
                    consumer: constants.consumer().clone(),
                },
            );
            let step = WillCont::Settle { iou };
            Ok(Will::Cont { output, step })
        }
        (Stage::Closed { .. }, Want::Elapse) => {
            todo!()
        }

        // Settled → End (Eol)
        (Stage::Settled { .. }, Want::End) => {
            todo!()
        }

        // Wrong stage
        (_, want) => Err(StepError::WrongStage {
            want: want.label(),
            stage: channel.stage().label(),
        }),
    }
}

fn verify_iou(iou_key: &subbit_core::VerifyingKey, tag: &subbit_core::Tag, iou: &Iou) -> bool {
    let message = Tbs::new(tag.clone(), iou.amount()).to_vec();
    let vk_bytes: &[u8; 32] = iou_key.as_ref();
    let sig_bytes: &[u8; 64] = iou.signature().as_ref();
    VerificationKey::from(*vk_bytes).verify(&message, &Signature::from(*sig_bytes))
}

// ---------------------------------------------------------------------------
// Build: cross-channel assembly, domain-aware (batch → tx)
// ---------------------------------------------------------------------------

pub fn build(_spends: Vec<(Input, Will)>) -> Result<Tx, BuildError> {
    todo!()
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Per-channel: Want is invalid for this Channel
#[derive(Debug)]
pub enum StepError {
    /// Step not valid for current stage (e.g. Add on Closed)
    WrongStage {
        want: &'static str,
        stage: &'static str,
    },
    /// Time bound is not feasible
    Bound { reason: &'static str },
    /// IOU failed verification
    IouForm,
    /// IOU is stale
    IouStale,
    /// IOU is current, but already used
    IouUsed,
    /// Zero funds available. Step will have no effect.
    NoFunds,
    /// Add amount is too small
    AddAmount,
}

/// Cross-channel: batch cannot be assembled into a tx
#[derive(Debug)]
pub enum BuildError {
    /// Time bounds across steps are contradictory
    ConflictingBounds,
    /// Redeemer construction failed
    Redeemer,
    /// Tx exceeds size limit
    TxTooLarge,
    /// Cannot balance tx (fees, min utxo, etc.)
    Balancing,
}

/// Intermediary transaction. Just the fields relevant to subbit-spends.
#[derive(Debug, Clone, Default)]
pub struct Tx {
    pub inputs: Vec<(Input, Redeemer)>,
    pub outputs: Vec<Output>,
    pub signers: Vec<Hash28>,
    pub upper_bound: Option<Duration>,
    pub lower_bound: Option<Duration>,
}
