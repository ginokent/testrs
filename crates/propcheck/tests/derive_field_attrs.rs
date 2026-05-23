//! Tests for `#[arbitrary(strategy = ...)]` field attributes on derive.
//!
//! Both the string-literal and bare-expression forms are exercised, plus
//! shrink-through-strategy verification.

use propcheck::strategy::{int_range, str, vec_of, Strategy, StrategyExt};
use propcheck::{forall_with, run, Arbitrary, Config, XorShift64};

fn cfg(seed: u64) -> Config {
    Config {
        seed,
        cases: 200,
        silence_panic_hook: false,
        regression_replay: false,
        ..Config::default()
    }
}

// --- string-literal form ---------------------------------------------

#[derive(Arbitrary, Debug, Clone, PartialEq)]
struct ConfigStr {
    #[arbitrary(strategy = "str::ascii_alphanumeric(1..15)")]
    name: String,
    #[arbitrary(strategy = "int_range(1024u16..65535)")]
    port: u16,
    // Default Arbitrary for this one.
    body: Vec<u8>,
}

#[test]
fn string_literal_strategy_constrains_field_values() {
    run("config string-strategy", |c: &ConfigStr| {
        propcheck::prop_assert!(!c.name.is_empty());
        propcheck::prop_assert!(c.name.chars().all(|c| c.is_ascii_alphanumeric()));
        propcheck::prop_assert!(c.port >= 1024);
        // body is unconstrained — just check it's a Vec.
        let _ = c.body.len();
        true
    });
}

// --- bare-expression form -------------------------------------------

#[derive(Arbitrary, Debug, Clone, PartialEq)]
struct ConfigExpr {
    #[arbitrary(strategy = str::ascii_letters_lower(3..10))]
    user: String,
    #[arbitrary(strategy = int_range(1i32..100))]
    age: i32,
}

#[test]
fn bare_expression_strategy_works() {
    run("config bare-expr", |c: &ConfigExpr| {
        propcheck::prop_assert!(c.user.len() >= 3 && c.user.len() < 10);
        propcheck::prop_assert!(c.user.chars().all(|c| c.is_ascii_lowercase()));
        propcheck::prop_assert!(c.age >= 1 && c.age < 100);
        true
    });
}

// --- tuple struct with field attrs ----------------------------------

#[derive(Arbitrary, Debug, Clone, PartialEq)]
struct Pair(
    #[arbitrary(strategy = int_range(0i32..10))] i32,
    #[arbitrary(strategy = int_range(100i32..200))] i32,
);

#[test]
fn tuple_struct_field_attrs_work() {
    let mut rng = XorShift64::seed_from_u64(7);
    for _ in 0..200 {
        let p: Pair = Arbitrary::arbitrary(&mut rng, 32);
        assert!((0..10).contains(&p.0));
        assert!((100..200).contains(&p.1));
    }
}

// --- enum variants with field attrs ---------------------------------

#[derive(Arbitrary, Debug, Clone, PartialEq)]
enum Message {
    Heartbeat,
    Text {
        #[arbitrary(strategy = "str::ascii_printable(1..20)")]
        content: String,
    },
    Code(#[arbitrary(strategy = int_range(100u16..600))] u16),
}

#[test]
fn enum_variant_field_attrs_constrain_payload() {
    run("message variants", |m: &Message| match m {
        Message::Heartbeat => true,
        Message::Text { content } => {
            propcheck::prop_assert!(!content.is_empty() && content.len() < 20);
            propcheck::prop_assert!(content.chars().all(|c| c.is_ascii() && !c.is_control()));
            true
        }
        Message::Code(c) => {
            propcheck::prop_assert!(*c >= 100 && *c < 600);
            true
        }
    });
}

// --- shrink propagates through strategy ----------------------------

#[derive(Arbitrary, Debug, Clone, PartialEq)]
struct Constrained {
    #[arbitrary(strategy = int_range(10i32..1000))]
    value: i32,
}

#[test]
fn shrink_uses_strategy_not_default_arbitrary() {
    // Property: value never exceeds 50. Fails for many cases; shrunk
    // should be 50 (the int_range strategy shrinks toward 10, and the
    // boundary where the property first fails is 50).
    let outcome = forall_with(cfg(11), |c: &Constrained| c.value <= 50);
    assert!(outcome.is_failed());
    let shrunk = outcome.shrunk().unwrap();
    // The strategy shrinks toward 10, never going below. So shrunk.value
    // is at the failure boundary 50.
    assert_eq!(shrunk.value, 51);
}

// --- strategy expression can call user functions -------------------

fn small_evens() -> impl Strategy<Value = i32> {
    int_range(0i32..1000).filter(|n| n % 2 == 0)
}

#[derive(Arbitrary, Debug, Clone, PartialEq)]
struct EvenWrapper {
    #[arbitrary(strategy = small_evens())]
    n: i32,
}

#[test]
fn field_strategy_can_call_user_fns() {
    run("even wrapper", |e: &EvenWrapper| {
        propcheck::prop_assert!(e.n % 2 == 0);
        propcheck::prop_assert!((0..1000).contains(&e.n));
        true
    });
}

// --- combined with vec_of ------------------------------------------

#[derive(Arbitrary, Debug, Clone, PartialEq)]
struct Bag {
    #[arbitrary(strategy = vec_of(int_range(0i32..10), 1..5))]
    items: Vec<i32>,
}

#[test]
fn vec_of_in_field_attr_works() {
    run("bag", |b: &Bag| {
        propcheck::prop_assert!(!b.items.is_empty() && b.items.len() < 5);
        propcheck::prop_assert!(b.items.iter().all(|&n| (0..10).contains(&n)));
        true
    });
}
