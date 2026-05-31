use proptest::prelude::*;

use crate::AikenFn;
use subbit_core::{
    Constants, Currency, Datum, Duration, Hash28, Iou, Redeemer, Stage, Tag, VerifyingKey,
};

fn currency_fn() -> AikenFn {
    AikenFn::from_shortcut("currency")
}

#[test]
fn currency_ada_conforms() {
    assert!(currency_fn().eval_true(&Currency::Ada));
}

#[test]
fn currency_by_hash_conforms() {
    assert!(currency_fn().eval_true(&Currency::ByHash { hash: [1u8; 28] }));
}

#[test]
fn currency_by_class_conforms() {
    assert!(currency_fn().eval_true(&Currency::ByClass {
        hash: [1u8; 28],
        name: vec![1, 2, 3]
    }));
}

// Sanity check: verifies the framework can detect rejections,
// not just pass everything through.
#[test]
fn currency_long_name_rejected() {
    assert!(currency_fn().eval_false(&Currency::ByClass {
        hash: [1u8; 28],
        name: vec![1u8; 33]
    }));
}

#[test]
fn duration_zero() {
    let duration = Duration::from_millis(0);
    let aiken_fn = AikenFn::from_shortcut("duration");
    assert!(aiken_fn.eval_true(&duration));
}

proptest! {
    #[test]
    fn prop_duration_conforms(duration: Duration) {
        assert!(AikenFn::from_shortcut("duration").eval_true(&duration));
    }

    #[test]
    fn prop_currency_conforms(currency: Currency) {
        assert!(currency_fn().eval_true(&currency));
    }

    #[test]
    fn prop_iou_conforms(iou: Iou) {
        assert!(AikenFn::from_shortcut("iou").eval_true(&iou));
    }

    #[test]
    fn prop_constants_conforms(constants: Constants) {
        assert!(AikenFn::from_shortcut("constants").eval_true(&constants));
    }

    #[test]
    fn prop_stage_conforms(stage: Stage) {
        assert!(AikenFn::from_shortcut("stage").eval_true(&stage));
    }

    #[test]
    fn prop_datum_conforms(datum: Datum) {
        assert!(AikenFn::from_shortcut("datum").eval_true(&datum));
    }

    #[test]
    fn prop_redeemer_conforms(redeemer: Redeemer) {
        assert!(AikenFn::from_shortcut("redeemer").eval_true(&redeemer));
    }
}
