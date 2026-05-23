//! Tests for `#[derive(Arbitrary)]` on enums and structs with `where`
//! clauses.

use propcheck::{forall_with, run, Arbitrary, Config, XorShift64};

#[derive(Arbitrary, Debug, Clone, PartialEq)]
enum SimpleEnum {
    Alpha,
    Beta,
    Gamma,
}

#[derive(Arbitrary, Debug, Clone, PartialEq)]
enum WithFields {
    Empty,
    Single(u32),
    Pair(u8, String),
    Named { x: i32, y: i32 },
}

#[derive(Arbitrary, Debug, Clone, PartialEq)]
enum Generic<T> {
    Nothing,
    Just(T),
    List(Vec<T>),
}

fn no_replay_cfg(seed: u64) -> Config {
    Config {
        seed,
        cases: 200,
        silence_panic_hook: false,
        regression_replay: false,
        ..Config::default()
    }
}

#[test]
fn simple_enum_generates_all_variants() {
    let mut rng = XorShift64::seed_from_u64(1);
    let mut seen_alpha = false;
    let mut seen_beta = false;
    let mut seen_gamma = false;
    for _ in 0..200 {
        match SimpleEnum::arbitrary(&mut rng, 10) {
            SimpleEnum::Alpha => seen_alpha = true,
            SimpleEnum::Beta => seen_beta = true,
            SimpleEnum::Gamma => seen_gamma = true,
        }
    }
    assert!(seen_alpha && seen_beta && seen_gamma);
}

#[test]
fn enum_with_fields_arbitrary_works() {
    let mut rng = XorShift64::seed_from_u64(2);
    let mut seen_empty = false;
    let mut seen_single = false;
    let mut seen_pair = false;
    let mut seen_named = false;
    for _ in 0..400 {
        match WithFields::arbitrary(&mut rng, 10) {
            WithFields::Empty => seen_empty = true,
            WithFields::Single(_) => seen_single = true,
            WithFields::Pair(_, _) => seen_pair = true,
            WithFields::Named { .. } => seen_named = true,
        }
    }
    assert!(seen_empty);
    assert!(seen_single);
    assert!(seen_pair);
    assert!(seen_named);
}

#[test]
fn enum_shrinks_to_simplest_variant_when_property_universally_fails() {
    // An always-false property: every variant fails, so the shrinker can
    // collapse non-unit variants into the simplest (Empty) and accept it.
    let outcome = forall_with(no_replay_cfg(3), |_: &WithFields| false);
    assert!(outcome.is_failed());
    assert_eq!(*outcome.shrunk().unwrap(), WithFields::Empty);
}

#[test]
fn enum_shrinks_within_variant_when_collapse_doesnt_help() {
    // Property fails *only* for the Pair variant. The collapse to Empty
    // would make the property pass (so collapse is rejected); the shrinker
    // stays inside Pair and reduces its fields toward defaults.
    let outcome = forall_with(no_replay_cfg(3), |v: &WithFields| {
        !matches!(v, WithFields::Pair(_, _))
    });
    assert!(outcome.is_failed());
    assert_eq!(
        *outcome.shrunk().unwrap(),
        WithFields::Pair(0, String::new())
    );
}

#[test]
fn enum_shrinks_fields_within_variant_when_no_unit_collapse_applies() {
    // Property: any WithFields::Pair has its first u8 <= 200.
    // Shrink should converge on Pair(201, "") because the simplest variant
    // (Empty) makes the matches!() guard skip and the property pass; so
    // the shrinker stays within Pair and reduces the fields.
    //
    // We test this via a property that *only* fires on Pair and checks a
    // field invariant.
    let outcome = forall_with(no_replay_cfg(4), |v: &WithFields| match v {
        WithFields::Pair(n, _) => *n <= 200,
        _ => true,
    });
    assert!(outcome.is_failed());
    match outcome.shrunk().unwrap() {
        WithFields::Pair(n, s) => {
            assert_eq!(*n, 201);
            assert_eq!(s, "");
        }
        other => panic!("expected shrunk to be Pair, got {other:?}"),
    }
}

#[test]
fn generic_enum_works_with_concrete_type() {
    let mut rng = XorShift64::seed_from_u64(5);
    let mut seen_nothing = false;
    let mut seen_just = false;
    let mut seen_list = false;
    for _ in 0..200 {
        match Generic::<u32>::arbitrary(&mut rng, 10) {
            Generic::Nothing => seen_nothing = true,
            Generic::Just(_) => seen_just = true,
            Generic::List(_) => seen_list = true,
        }
    }
    assert!(seen_nothing && seen_just && seen_list);
}

#[test]
fn generic_enum_drives_run() {
    run("Generic round-trip", |g: &Generic<u8>| match g {
        Generic::Nothing => true,
        Generic::Just(_) => true,
        Generic::List(v) => v.len() == v.len(),
    });
}

// --- where clauses ----------------------------------------------------

#[derive(Arbitrary, Debug, Clone)]
struct WhereOwned<T>
where
    T: Send + 'static,
{
    inner: T,
    list: Vec<T>,
}

#[test]
fn struct_with_where_clause_compiles_and_generates() {
    let mut rng = XorShift64::seed_from_u64(6);
    let v: WhereOwned<u32> = Arbitrary::arbitrary(&mut rng, 10);
    let _ = v.inner;
    let _ = v.list;
}

#[derive(Arbitrary, Debug, Clone)]
enum WhereEnum<T>
where
    T: std::fmt::Debug + Send + 'static,
{
    Empty,
    Value(T),
}

#[test]
fn enum_with_where_clause_works() {
    let mut rng = XorShift64::seed_from_u64(7);
    for _ in 0..50 {
        let _: WhereEnum<u32> = Arbitrary::arbitrary(&mut rng, 10);
    }
}

// --- attribute macro + derive enum combination -----------------------

#[propcheck::propcheck]
fn enum_attribute_round_trip(e: SimpleEnum) {
    propcheck::prop_assert_eq!(e.clone(), e);
}

#[propcheck::propcheck(cases = 50)]
fn with_fields_attribute_works(e: WithFields) {
    propcheck::prop_assert_eq!(e.clone(), e);
}
