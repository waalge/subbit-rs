//! Tx resolver.
//!
//! On-chain txs don't self-describe channel evolution clearly enough to
//! reconstruct state without external lookups (UTxO set). This module
//! does a best-effort resolution: collect what the tx tells us, ask the
//! caller to look up what it doesn't, propagate until fully resolved.
//!
//! Normal ops: resolves in one pass. Unusual topologies (multi-script,
//! mutual close) may need several propagation rounds or stay partially
//! unresolved.

use cardano_sdk::{
    Credential, Hash, Input, PlutusData, RedeemerPointer, Transaction,
    cbor::{self, ToCbor},
    transaction::state::ReadyForSigning,
};
use subbit_core::{Redeemer, Stage, Step};

// ----------------------------------------------------------------------------
// Constants
// ----------------------------------------------------------------------------

// FIXME: derive from plutus.json at build time
fn subbit_hash() -> [u8; 28] {
    let hex = std::env::var("SUBBIT_HASH")
        .unwrap_or_else(|_| "f745370aed2791109b453e81ced6b818395bca21fc8ee55fd21bad95".to_string());
    hex::decode(hex)
        .expect("SUBBIT_HASH: invalid hex")
        .try_into()
        .expect("SUBBIT_HASH: must be 28 bytes")
}

// FIXME: derive from tx builder or core
const MIN_ADA_BUFFER: u64 = 2_000_000;

// ----------------------------------------------------------------------------
// Public API types
// ----------------------------------------------------------------------------

/// A fully resolved input/output pair (or unilateral close).
/// Constructed by [`Resolver::to_io`]; safe to call on partially resolved state.
#[derive(Debug, Clone)]
pub enum Io {
    /// A new channel funded in this tx - no corresponding input.
    Init { output: ParsedOutput },
    /// A channel step: input spent, output produced.
    Cont {
        step: Step,
        input: Input,
        output: ParsedOutput,
    },
    /// Channel closed - input spent, no continuing output.
    Done { step: Step, input: Input },
}

impl std::fmt::Display for Io {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Io::Init { output } => {
                write!(f, "Init(index={}, amount={})", output.index, output.amount)
            }
            Io::Cont {
                step,
                input,
                output,
            } => write!(f, "Cont(step={step:?}, output={})", output.index),
            Io::Done { step, input } => write!(f, "Done(step={step:?})"),
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ResolverError {
    #[error("Cannot parse tx bytes")]
    Parse,
    #[error("Input not found in parsed tx")]
    Input,
    #[error("Cont redeemer has no corresponding output")]
    Output,
    #[error("Other")]
    Other,
}

#[derive(Debug, Clone)]
pub enum PropagateResult {
    /// Fully resolved; call [`Resolver::to_io`].
    Resolved,
    /// Made progress; call [`Resolver::propagate`] again.
    Unresolved,
    /// Stuck - caller must supply more UTxO lookups via [`Resolver::looked_up`].
    Unchanged,
}

// ----------------------------------------------------------------------------
// Resolver
// ----------------------------------------------------------------------------

pub struct Resolver {
    parsed: ParsedTx,
    /// Incrementally-resolved working state, parallel to `parsed`.
    wip: WipTx,
}

impl Resolver {
    pub fn new(raw: &[u8]) -> Result<Self, ResolverError> {
        let parsed = ParsedTx::new(raw).map_err(|_| ResolverError::Parse)?;
        let wip = WipTx::new(&parsed);
        Ok(Self { parsed, wip })
    }

    /// Next input whose UTxO is still unknown; `None` if all are resolved.
    pub fn next_unresolved_input(&self) -> Option<&Input> {
        self.wip
            .next_unresolved_input()
            .map(|i| &self.parsed.inputs[i])
    }

    /// Supply a UTxO lookup result for a given input.
    /// `output: None` means the input is not a subbit UTxO.
    pub fn looked_up(
        &mut self,
        input: Input,
        output: Option<ParsedOutput>,
    ) -> Result<(), ResolverError> {
        let index = self
            .parsed
            .inputs
            .iter()
            .position(|i| i == &input)
            .ok_or(ResolverError::Input)?;
        self.wip.looked_up(index, output);
        Ok(())
    }

    /// Run one propagation round. Repeat until [`PropagateResult::Resolved`]
    /// or [`PropagateResult::Unchanged`].
    pub fn propagate(&mut self) -> Result<PropagateResult, ResolverError> {
        let changed = self.resolve_main()? | self.resolve_mutual()? | self.resolve_outputs()?;

        if self.is_resolved() {
            return Ok(PropagateResult::Resolved);
        }
        if changed {
            return Ok(PropagateResult::Unresolved);
        }
        Ok(PropagateResult::Unchanged)
    }

    pub fn is_resolved(&self) -> bool {
        self.wip
            .inputs
            .iter()
            .all(|i| !matches!(i, WipInput::Unresolved))
            && self
                .wip
                .redeemers
                .iter()
                .all(|r| !matches!(r, WipRedeemer::Unresolved))
            && self
                .wip
                .outputs
                .iter()
                .all(|o| !matches!(o, WipOutput::Unresolved))
    }

    /// Reconstruct `Io` vec from current (possibly partial) wip state.
    /// Unresolved redeemers fall back to `Done`; unresolved outputs to `Init`.
    pub fn to_io(&self) -> Vec<Io> {
        let mut ios: Vec<Io> = self
            .wip
            .inputs
            .iter()
            .zip(self.wip.redeemers.iter())
            .enumerate()
            .map(|(i, (_, redeemer))| {
                let input = self.parsed.inputs[i].clone();
                match redeemer {
                    WipRedeemer::Cont(step) => {
                        let output_index = self
                            .wip
                            .outputs
                            .iter()
                            .position(|o| matches!(o, WipOutput::Cont { input } if *input == i))
                            .expect("Cont redeemer without output; resolve_outputs ensures this");
                        Io::Cont {
                            step: step.clone(),
                            input,
                            output: self.parsed.outputs[output_index].clone(),
                        }
                    }
                    WipRedeemer::Eol(step) => Io::Done {
                        step: step.clone(),
                        input,
                    },
                    // Unresolved/Mutual: best-effort fallback
                    // FIXME :: We actually don't know why its ended here!
                    _ => Io::Done {
                        step: Step::Eol(subbit_core::Eol::End),
                        input,
                    },
                }
            })
            .collect();

        // Init outputs: those not claimed by any Cont
        for (wip, parsed) in self.wip.outputs.iter().zip(self.parsed.outputs.iter()) {
            if matches!(wip, WipOutput::Init) {
                ios.push(Io::Init {
                    output: parsed.clone(),
                });
            }
        }

        ios
    }

    // -- Private resolution passes --------------------------------------------

    // If there is a validated Mutual redeemer:
    // - There should be exactly one input (the mutual close input).
    // - All outputs are Init (no channel continuation).
    // FIXME: not yet implemented
    fn resolve_mutual(&mut self) -> Result<bool, ResolverError> {
        Ok(false)
    }

    // A validated Main redeemer encodes the full step sequence.
    // Two sufficient conditions to assign steps to inputs:
    //   Bound above: #steps == #inputs  (common case)
    //   Bound below: #steps == #validated inputs  (multi-script edge case)
    fn resolve_main(&mut self) -> Result<bool, ResolverError> {
        let Some((Redeemer::Main(steps), _)) = self
            .parsed
            .redeemers
            .iter()
            .zip(self.wip.inputs.iter())
            .find(|(r, i)| matches!(r, Redeemer::Main(_)) && i.is_validated())
        else {
            return Ok(false);
        };

        let mut changed = false;

        // Bound above
        if steps.len() == self.wip.inputs.len() {
            for (i, (input, redeemer)) in self
                .wip
                .inputs
                .iter_mut()
                .zip(self.wip.redeemers.iter_mut())
                .enumerate()
            {
                if !input.is_validated() {
                    *input = WipInput::Inferred;
                    changed = true;
                }
                if matches!(redeemer, WipRedeemer::Unresolved) {
                    *redeemer = match &steps[i] {
                        Step::Cont(c) => WipRedeemer::Cont(Step::Cont(c.clone())),
                        Step::Eol(e) => WipRedeemer::Eol(Step::Eol(e.clone())),
                    };
                    changed = true;
                }
            }
            return Ok(changed);
        }

        // Bound below - FIXME: not yet implemented
        if steps.len() == self.wip.inputs.iter().filter(|i| i.is_validated()).count() {}

        Ok(false)
    }

    // Cont redeemers claim outputs left-to-right; any remaining outputs are Init.
    // Breaks early on Unresolved - can't safely assign past an unknown redeemer.
    fn resolve_outputs(&mut self) -> Result<bool, ResolverError> {
        let mut changed = false;
        let mut outputs = self.wip.outputs.iter_mut();

        for (i, redeemer) in self.wip.redeemers.iter().enumerate() {
            match redeemer {
                WipRedeemer::Unresolved => return Ok(changed),
                WipRedeemer::Cont(_) => {
                    let output = outputs.next().ok_or(ResolverError::Output)?;
                    if matches!(output, WipOutput::Unresolved) {
                        *output = WipOutput::Cont { input: i };
                        changed = true;
                    }
                }
                _ => {}
            }
        }

        // Redeemers exhausted normally -- remaining outputs are Init.
        for output in outputs {
            if matches!(output, WipOutput::Unresolved) {
                *output = WipOutput::Init;
                changed = true;
            }
        }

        Ok(changed)
    }
}

// ----------------------------------------------------------------------------
// Parsed tx - decoded once from raw bytes, immutable thereafter
// ----------------------------------------------------------------------------

/// A script output at this validator address.
#[derive(Debug, Clone)]
pub struct ParsedOutput {
    pub index: usize,
    pub delegation: Option<Credential>,
    pub amount: u64,
    pub stage: Stage,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ParseError {
    #[error("Decode: {0}")]
    Decode(String),
}

impl From<cardano_sdk::cbor::decode::Error> for ParseError {
    fn from(e: cardano_sdk::cbor::decode::Error) -> Self {
        Self::Decode(e.to_string())
    }
}

struct ParsedTx {
    id: Hash<32>,
    inputs: Vec<Input>,
    redeemers: Vec<Redeemer>,
    outputs: Vec<ParsedOutput>,
}

impl ParsedTx {
    fn new(tx: &[u8]) -> Result<Self, ParseError> {
        let tx: Transaction<ReadyForSigning> = cbor::decode(tx)?;
        let id = tx.id();

        // Only keep outputs at the subbit address with a valid inline datum.
        let outputs = tx
            .outputs()
            .enumerate()
            .filter_map(|(index, output)| {
                let address = output.address().as_shelley()?;
                let payment = address.payment().as_script()?;
                if <[u8; 28]>::from(payment) != subbit_hash() {
                    return None;
                }
                let delegation = address.delegation();
                let cardano_sdk::Datum::Inline(datum) = output.datum()? else {
                    return None;
                };
                let datum = minicbor::decode::<subbit_core::Datum>(&datum.to_cbor())
                    .or_else(|e| {
                        tracing::debug!(index, error = %e, "output filtered: datum decode failed");
                        Err(e)
                    })
                    .ok()?;
                if <[u8; 28]>::from(datum.own_hash) != subbit_hash() {
                    tracing::debug!(index, "output filtered: datum own_hash mismatch");
                    return None;
                }
                let stage = datum.stage;
                let amount = compute_amount(&output.value(), &stage);
                Some(ParsedOutput {
                    index,
                    delegation,
                    amount,
                    stage,
                })
            })
            .collect();

        // Only keep redeemers that decode as subbit redeemers; pair with their input.
        let (inputs, redeemers) = tx
            .redeemers()
            .filter_map(|(ptr, data): (RedeemerPointer, PlutusData<'_>)| {
                let redeemer: subbit_core::Redeemer = minicbor::decode(&data.to_cbor()).ok()?;
                let input = tx
                    .inputs()
                    .nth(ptr.index() as usize)
                    .expect("ptr index out of range");
                Some((input, redeemer))
            })
            .collect::<(Vec<Input>, Vec<Redeemer>)>();

        Ok(Self {
            id,
            inputs,
            redeemers,
            outputs,
        })
    }
}

/// Derive the channel balance from the UTxO value given the stage's currency rules.
fn compute_amount(val: &cardano_sdk::Value<u64>, stage: &Stage) -> u64 {
    if let Some(constants) = stage.constants() {
        match constants.currency() {
            subbit_core::Currency::Ada => val.lovelace().saturating_sub(MIN_ADA_BUFFER),
            subbit_core::Currency::ByHash { hash } => val
                .assets()
                .get(&Hash::<28>::from(<[u8; 28]>::from(*hash)))
                .and_then(|x| x.values().next().copied())
                .unwrap_or(0),
            subbit_core::Currency::ByClass { hash, name } => val
                .assets()
                .get(&Hash::<28>::from(<[u8; 28]>::from(*hash)))
                .and_then(|x| x.get(name))
                .copied()
                .unwrap_or(0),
        }
    } else {
        // Settled/unknown currency - lovelace is a reasonable proxy.
        val.lovelace().saturating_sub(MIN_ADA_BUFFER)
    }
}

// ----------------------------------------------------------------------------
// Wip ("work in progress") - parallel to parsed, mutated during resolution
// ----------------------------------------------------------------------------

/// What we know about a script input's provenance.
#[derive(Debug, Clone, Default)]
enum WipInput {
    #[default]
    Unresolved,
    /// Looked up; not a subbit UTxO (but the validator still ran on it).
    Unknown,
    /// Inferred from Main redeemer step count - no UTxO lookup needed.
    Inferred,
    /// Looked up; confirmed subbit UTxO with full state.
    Known(WipKnownInput),
}

impl WipInput {
    /// True if this input is confirmed to have been validated by the script.
    fn is_validated(&self) -> bool {
        matches!(self, WipInput::Inferred | WipInput::Known(_))
    }
}

/// Full resolved state for a Known input.
#[derive(Debug, Clone)]
struct WipKnownInput {
    delegation: Option<Credential>,
    amount: u64,
    stage: Stage,
}

impl From<ParsedOutput> for WipKnownInput {
    fn from(o: ParsedOutput) -> Self {
        Self {
            delegation: o.delegation,
            amount: o.amount,
            stage: o.stage,
        }
    }
}

/// What we know about a script output's role.
#[derive(Debug, Clone, Default)]
enum WipOutput {
    #[default]
    Unresolved,
    /// Continuing output for input at `input` index.
    Cont { input: usize },
    /// New channel initialisation - no corresponding input.
    Init,
}

/// The resolved redeemer for a script input, carrying the decoded step.
#[derive(Debug, Clone, Default)]
enum WipRedeemer {
    #[default]
    Unresolved,
    Cont(Step),
    Eol(Step),
    Mutual,
}

/// Working state - structurally parallel to [`ParsedTx`] inputs/outputs.
struct WipTx {
    inputs: Vec<WipInput>,
    redeemers: Vec<WipRedeemer>,
    outputs: Vec<WipOutput>,
}

impl WipTx {
    fn new(parsed: &ParsedTx) -> Self {
        Self {
            inputs: vec![WipInput::default(); parsed.inputs.len()],
            redeemers: vec![WipRedeemer::default(); parsed.inputs.len()],
            outputs: vec![WipOutput::default(); parsed.outputs.len()],
        }
    }

    fn next_unresolved_input(&self) -> Option<usize> {
        self.inputs
            .iter()
            .position(|x| matches!(x, WipInput::Unresolved))
    }

    fn looked_up(&mut self, index: usize, output: Option<ParsedOutput>) {
        self.inputs[index] = match output {
            Some(o) => WipInput::Known(WipKnownInput::from(o)),
            None => WipInput::Unknown,
        };
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use cardano_sdk::input;

    use super::*;

    // -- Helpers --------------------------------------------------------------

    fn make_wip(n_inputs: usize, n_outputs: usize) -> WipTx {
        WipTx {
            inputs: vec![WipInput::default(); n_inputs],
            redeemers: vec![WipRedeemer::default(); n_inputs],
            outputs: vec![WipOutput::default(); n_outputs],
        }
    }

    // -- WipInput::is_validated -----------------------------------------------

    #[test]
    fn unresolved_is_not_validated() {
        assert!(!WipInput::Unresolved.is_validated());
    }

    #[test]
    fn unknown_is_not_validated() {
        assert!(!WipInput::Unknown.is_validated());
    }

    #[test]
    fn inferred_is_validated() {
        assert!(WipInput::Inferred.is_validated());
    }

    // -- resolve_outputs ------------------------------------------------------

    /// Helper: build a minimal Resolver shell with controlled wip state,
    /// bypassing the real tx parse.  We do this by constructing the fields
    /// directly - the resolution logic only touches `self.wip`.
    fn resolver_with_wip(wip: WipTx) -> Resolver {
        // ParsedTx with empty vecs - resolution passes only read parsed for
        // inputs/outputs when building Io, not during resolve_* passes.
        let input = input!(
            "0000000000000000000000000000000000000000000000000000000000000000",
            0
        );
        let parsed = ParsedTx {
            id: Hash::from([0u8; 32]),
            inputs: vec![input; wip.inputs.len()],
            redeemers: vec![],
            outputs: vec![],
        };
        Resolver { parsed, wip }
    }

    #[test]
    fn resolve_outputs_cont_claims_output() {
        let mut wip = make_wip(1, 1);
        wip.redeemers[0] = WipRedeemer::Cont(Step::Cont(subbit_core::Cont::Add));

        let mut r = resolver_with_wip(wip);
        let changed = r.resolve_outputs().unwrap();

        assert!(changed);
        assert!(matches!(r.wip.outputs[0], WipOutput::Cont { input: 0 }));
    }

    #[test]
    fn resolve_outputs_remaining_become_init() {
        let mut wip = make_wip(1, 2);
        wip.redeemers[0] = WipRedeemer::Cont(Step::Cont(subbit_core::Cont::Add));

        let mut r = resolver_with_wip(wip);
        r.resolve_outputs().unwrap();

        assert!(matches!(r.wip.outputs[0], WipOutput::Cont { input: 0 }));
        assert!(matches!(r.wip.outputs[1], WipOutput::Init));
    }

    #[test]
    fn resolve_outputs_eol_leaves_all_outputs_as_init() {
        let mut wip = make_wip(1, 1);
        wip.redeemers[0] = WipRedeemer::Eol(Step::Eol(subbit_core::Eol::End));

        let mut r = resolver_with_wip(wip);
        r.resolve_outputs().unwrap();

        assert!(matches!(r.wip.outputs[0], WipOutput::Init));
    }

    #[test]
    fn resolve_outputs_breaks_on_unresolved() {
        // Unresolved redeemer at index 0 - nothing should be assigned.
        let mut wip = make_wip(1, 1);
        // redeemers[0] stays Unresolved

        let mut r = resolver_with_wip(wip);
        let changed = r.resolve_outputs().unwrap();

        assert!(!changed);
        assert!(matches!(r.wip.outputs[0], WipOutput::Unresolved));
    }

    #[test]
    fn resolve_outputs_missing_output_is_error() {
        let mut wip = make_wip(1, 0); // Cont redeemer but no outputs
        wip.redeemers[0] = WipRedeemer::Cont(Step::Cont(subbit_core::Cont::Add));

        let mut r = resolver_with_wip(wip);
        assert!(matches!(r.resolve_outputs(), Err(ResolverError::Output)));
    }

    #[test]
    fn resolve_outputs_idempotent() {
        let mut wip = make_wip(1, 1);
        wip.redeemers[0] = WipRedeemer::Cont(Step::Cont(subbit_core::Cont::Add));

        let mut r = resolver_with_wip(wip);
        r.resolve_outputs().unwrap();
        let changed = r.resolve_outputs().unwrap();

        // Second pass: already assigned, should report no change.
        assert!(!changed);
    }

    // -- is_resolved ----------------------------------------------------------

    #[test]
    fn is_resolved_requires_no_unresolved() {
        let mut wip = make_wip(1, 1);
        wip.inputs[0] = WipInput::Inferred;
        wip.redeemers[0] = WipRedeemer::Eol(Step::Eol(subbit_core::Eol::End));
        wip.outputs[0] = WipOutput::Init;

        let r = resolver_with_wip(wip);
        assert!(r.is_resolved());
    }

    #[test]
    fn is_resolved_false_when_any_unresolved() {
        let wip = make_wip(1, 1); // all Unresolved by default
        let r = resolver_with_wip(wip);
        assert!(!r.is_resolved());
    }
}
