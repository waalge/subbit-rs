use amaru_uplc::{arena::Arena, binder::DeBruijn, term::Term};
use proptest::prelude::*;

use crate::{AikenFn, aiken_fn::try_into_plutus_data};
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

fn valid_constants() -> Constants {
    Constants {
        tag: Tag::from(vec![1, 2, 3]),
        currency: Currency::Ada,
        iou_key: VerifyingKey::new([1u8; 32]),
        consumer: Hash28::new([1u8; 28]),
        provider: Hash28::new([2u8; 28]),
        close_period: Duration::from_secs(86400),
    }
}

#[test]
fn duration_boundary_conforms() {
    let aiken_fn = AikenFn::from_shortcut("stage");
    // just below i64::MAX millis
    let duration = Duration::from_millis(i64::MAX as u64);
    let stage = Stage::Closed {
        constants: valid_constants(),
        amount: 0,
        elapse_at: duration,
    };
    assert!(aiken_fn.eval_true(&stage));
}

#[test]
fn duration_above_i64_max_conforms() {
    let aiken_fn = AikenFn::from_shortcut("stage");
    // just above i64::MAX millis
    let duration = Duration::from_millis(i64::MAX as u64 + 1);
    let stage = Stage::Closed {
        constants: valid_constants(),
        amount: 0,
        elapse_at: duration,
    };
    assert!(aiken_fn.eval_true(&stage));
}

#[test]
fn duration_zero() {
    let stage = Stage::Closed {
        constants: valid_constants(),
        amount: 0,
        elapse_at: Duration::from_millis(0),
    };
    assert!(AikenFn::from_shortcut("stage").eval_true(&stage));
}

#[test]
fn duration_one_sec() {
    let stage = Stage::Closed {
        constants: valid_constants(),
        amount: 0,
        elapse_at: Duration::from_secs(1),
    };
    assert!(AikenFn::from_shortcut("stage").eval_true(&stage));
}

#[test]
fn duration_one_day() {
    let stage = Stage::Closed {
        constants: valid_constants(),
        amount: 0,
        elapse_at: Duration::from_secs(86400),
    };
    assert!(AikenFn::from_shortcut("stage").eval_true(&stage));
}

#[test]
fn stage_opened_works() {
    let stage = Stage::Opened {
        constants: valid_constants(),
        amount: 0,
    };
    assert!(AikenFn::from_shortcut("stage").eval_true(&stage));
}

#[test]
fn debug_stage_opened_cbor() {
    let stage = Stage::Opened {
        constants: valid_constants(),
        amount: 0,
    };
    let mut buf = Vec::new();
    minicbor::encode(&stage, &mut buf).unwrap();
    println!("{}", hex::encode(&buf));
    let arena = Arena::new();
    let program = AikenFn::from_shortcut("stage").program::<DeBruijn>(&arena);
    let arg = Term::data(&arena, try_into_plutus_data(&arena, &stage).unwrap());
    let result = program.apply(&arena, arg).eval(&arena);
    println!("{:?}", &result.term);
}

#[test]
fn debug_stage_opened_eval() {
    let stage = Stage::Opened {
        constants: valid_constants(),
        amount: 0,
    };
    let mut buf = Vec::new();
    minicbor::encode(&stage, &mut buf).unwrap();
    dbg!(hex::encode(&buf));
    let arena = Arena::new();
    let aiken_fn = AikenFn::from_shortcut("stage");
    let program = aiken_fn.program::<DeBruijn>(&arena);
    let arg = Term::data(&arena, try_into_plutus_data(&arena, &stage).unwrap());
    let result = program.apply(&arena, arg).eval(&arena);
    dbg!(&result.term);
    dbg!(&result.info);
}

#[test]
fn debug_duration_cbor() {
    let d = Duration::from_secs(86400);
    let arena = Arena::new();
    let program = AikenFn::from_shortcut("duration").program::<DeBruijn>(&arena);
    let arg = Term::integer_from(&arena, d.0.as_millis() as i128);
    let result = program.apply(&arena, arg).eval(&arena);
    dbg!(&result.term);
}

proptest! {
    // #[test]
    // fn prop_duration_conforms(duration: Duration) {
    //     assert!(AikenFn::from_shortcut("duration").eval_true(&duration));
    // }

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

    // #[test]
    // fn prop_stage_conforms(stage: Stage) {
    //     assert!(AikenFn::from_shortcut("stage").eval_true(&stage));
    // }

    // #[test]
    // fn prop_datum_conforms(datum: Datum) {
    //     assert!(AikenFn::from_shortcut("datum").eval_true(&datum));
    // }

    // #[test]
    // fn prop_redeemer_conforms(redeemer: Redeemer) {
    //     assert!(AikenFn::from_shortcut("redeemer").eval_true(&redeemer));
    // }
}
