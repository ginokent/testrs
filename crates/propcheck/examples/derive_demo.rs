//! Demonstrates the full propcheck workflow including the post-Tier-A
//! additions: `#[arbitrary(strategy = ...)]` field attributes, dependent
//! generation via `flat_map`, recursive data via `prop_recursive!`,
//! `prop_assert_close!`, and the rest of the toolkit working together.
//!
//! Run with: `cargo run --example derive_demo -p propcheck`

use propcheck::strategy::{any, int_range, just, str, vec_of, StrategyExt};
use propcheck::{
    classify, prop_assert, prop_assert_close, prop_assert_eq, prop_assert_matches, prop_assume,
    prop_oneof, prop_recursive, run, run_strategy_with, Arbitrary, Config,
};

#[derive(Arbitrary, Debug, Clone, PartialEq)]
struct User {
    // Field attributes constrain the generator domain so the test never
    // wastes cases on nonsensical inputs.
    #[arbitrary(strategy = "str::ascii_alphanumeric(1..16)")]
    name: String,
    #[arbitrary(strategy = int_range(18u8..100))]
    age: u8,
    #[arbitrary(strategy = vec_of(int_range(-50i32..50), 0..5))]
    favorite_numbers: Vec<i32>,
}

#[derive(Arbitrary, Debug, Clone, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Num(i32),
    Array(Vec<Json>),
}

fn main() {
    // 1. Field attributes keep the generator inside the domain — no
    //    discards needed, classifications reflect the real distribution.
    run("User invariants hold under derived generator", |u: &User| {
        classify!(u.favorite_numbers.is_empty(), "no-favorites");
        classify!(u.favorite_numbers.len() >= 3, "many-favorites");
        prop_assert!(!u.name.is_empty());
        prop_assert!((18..100).contains(&u.age));
        true
    });

    // 2. prop_assert_matches! makes Result/Option/enum assertions readable.
    run("name parses as identifier-like", |u: &User| {
        let first = u.name.chars().next();
        prop_assert_matches!(first, Some(c) if c.is_ascii_alphanumeric());
        true
    });

    // 3. Recursive data via prop_recursive!.
    let json_strategy = prop_recursive! {
        leaf = prop_oneof![
            just(Json::Null),
            any::<bool>().map(Json::Bool),
            int_range(-100i32..100).map(Json::Num),
        ],
        inner = |child| prop_oneof![
            just(Json::Null),
            int_range(-100i32..100).map(Json::Num),
            vec_of(child, 0..4).map(Json::Array),
        ],
        max_depth = 3,
    };
    run_strategy_with(
        "json values can be cloned",
        json_strategy,
        Config {
            cases: 200,
            ..Config::default()
        },
        |j: &Json| {
            prop_assert_eq!(j.clone(), j.clone());
            true
        },
    );

    // 4. Dependent generation via flat_map: pick a length, then a Vec of
    //    that exact length.
    let dependent = int_range(1usize..6).flat_map(|len| vec_of(any::<i32>(), len..len + 1));
    run_strategy_with(
        "length-dependent Vec has expected size",
        dependent,
        Config::default(),
        |v: &Vec<i32>| {
            prop_assert!((1..=5).contains(&v.len()));
            true
        },
    );

    // 5. Floating-point approximate equality via prop_assert_close!.
    //    sin(2x) ≈ 2 * sin(x) * cos(x).
    run("trig double-angle identity", |&x: &f64| {
        prop_assume!(x.is_finite() && x.abs() < 1e6);
        prop_assert_close!((2.0 * x).sin(), 2.0 * x.sin() * x.cos(), epsilon = 1e-9);
        true
    });

    // 6. Intentionally false property to demonstrate failure output +
    //    field-level shrinking through the field-attribute strategy.
    //    `age` shrinks toward 18 (the int_range lower bound), not 0.
    run("user.age is never above 50", |u: &User| {
        prop_assert!(u.age <= 50, "expected age <= 50, got {}", u.age);
        true
    });
}
