//! 後から追加された Tier-S/A combinator のテスト: flat_map、char_range、
//! bytes、float の range、prop_assert_close!、prop_recursive! です。
//!
//! `char_range`の`Range<char>`シグネチャに合わせるため、意図的に
//! `'a'..'z'`（exclusive）を使用しています。このテストにおける
//! clippy の「almost-complete range」lint は誤検知です。
#![allow(clippy::almost_complete_range)]

use propcheck::strategy::{
    any, bytes, char_range, f64_range, int_range, just, vec_of, Strategy, StrategyExt,
};
use propcheck::{
    forall_strategy_with, prop_assert, prop_assert_close, prop_oneof, prop_recursive, Config,
};

fn cfg(seed: u64) -> Config {
    Config {
        seed,
        cases: 100,
        silence_panic_hook: false,
        regression_replay: false,
        ..Config::default()
    }
}

// --- flat_map --------------------------------------------------------

#[test]
fn flat_map_generates_dependent_length() {
    // まず長さ1..=5を選び、その長さちょうどの Vec を生成します。
    let s = int_range(1usize..6).flat_map(|len| vec_of(any::<u8>(), len..len + 1));
    let mut rng = propcheck::XorShift64::seed_from_u64(7);
    for _ in 0..100 {
        let v = s.new_value(&mut rng, 32);
        assert!((1..=5).contains(&v.len()));
    }
}

#[test]
fn flat_map_drives_property() {
    let s = int_range(1usize..5).flat_map(|len| vec_of(int_range(0i32..10), len..len + 1));
    let outcome = forall_strategy_with(s, cfg(11), |v: &Vec<i32>| {
        prop_assert!(!v.is_empty());
        prop_assert!(v.iter().all(|&n| (0..10).contains(&n)));
        true
    });
    assert!(outcome.is_passed());
}

// --- char_range / bytes ---------------------------------------------

#[test]
fn char_range_stays_in_range_and_skips_surrogates() {
    let s = char_range('a'..'z');
    let mut rng = propcheck::XorShift64::seed_from_u64(13);
    for _ in 0..200 {
        let c = s.new_value(&mut rng, 16);
        assert!(('a'..'z').contains(&c));
    }
}

#[test]
fn char_range_shrinks_toward_lo() {
    let s = char_range('a'..'z');
    let shrinks = s.shrink_value(&'m');
    assert_eq!(shrinks[0], 'a');
    assert!(shrinks.iter().all(|c| ('a'..'z').contains(c)));
}

#[test]
fn bytes_strategy_respects_length() {
    let s = bytes(4..8);
    let mut rng = propcheck::XorShift64::seed_from_u64(17);
    for _ in 0..50 {
        let v = s.new_value(&mut rng, 16);
        assert!(v.len() >= 4 && v.len() < 8);
    }
}

// --- f32_range / f64_range -----------------------------------------

#[test]
fn f64_range_stays_in_bounds() {
    let s = f64_range(-1.0..1.0);
    let mut rng = propcheck::XorShift64::seed_from_u64(19);
    for _ in 0..200 {
        let v = s.new_value(&mut rng, 16);
        assert!((-1.0..1.0).contains(&v));
    }
}

#[test]
fn f64_range_shrinks_toward_zero_when_range_contains_it() {
    let s = f64_range(-10.0..10.0);
    let shrinks = s.shrink_value(&5.0);
    assert!(shrinks.contains(&0.0));
}

#[test]
fn f64_range_shrinks_toward_lo_when_zero_not_in_range() {
    let s = f64_range(10.0..20.0);
    let shrinks = s.shrink_value(&15.0);
    assert_eq!(shrinks[0], 10.0);
    assert!(shrinks.iter().all(|v| (10.0..20.0).contains(v)));
}

// --- prop_assert_close! --------------------------------------------

#[test]
fn prop_assert_close_passes_within_epsilon() {
    let outcome = propcheck::forall_with(cfg(23), |&n: &u8| -> bool {
        // 自明に近い値: 同一の値はどんな epsilon に対しても範囲内です。
        let x = n as f64;
        prop_assert_close!(x, x + 1e-12, epsilon = 1e-9);
        true
    });
    assert!(outcome.is_passed());
}

#[test]
fn prop_assert_close_fails_outside_epsilon() {
    let outcome = propcheck::forall_with(cfg(29), |_: &u8| -> bool {
        prop_assert_close!(1.0_f64, 2.0_f64, epsilon = 0.1);
        true
    });
    assert!(outcome.is_failed());
    let msg = outcome.failure_message().unwrap();
    assert!(msg.contains("|diff|"));
    assert!(msg.contains("epsilon"));
}

// --- prop_recursive! -----------------------------------------------

#[derive(propcheck::Arbitrary, Debug, Clone, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Num(i32),
    Array(Vec<Json>),
}

fn json_strategy() -> propcheck::strategy::BoxedStrategy<Json> {
    prop_recursive! {
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
    }
}

#[test]
fn prop_recursive_builds_tree_strategy() {
    let s = json_strategy();
    let mut rng = propcheck::XorShift64::seed_from_u64(31);
    let mut max_depth_seen = 0;
    for _ in 0..200 {
        let v = s.new_value(&mut rng, 8);
        max_depth_seen = max_depth_seen.max(depth_of(&v));
    }
    // depth = 3 を指定したので、実行時のツリーはその上限を守る必要があります。
    // (葉が 1 を寄与し、その上に 3 層の inner がそれらを包みます。)
    assert!(
        max_depth_seen <= 4,
        "expected depth <= 4, saw {max_depth_seen}"
    );
    // 再帰が発火することを確認するため、Array が少なくとも 1 つは現れるはずです。
    let mut rng = propcheck::XorShift64::seed_from_u64(31);
    let any_array = (0..200).any(|_| matches!(s.new_value(&mut rng, 8), Json::Array(_)));
    assert!(any_array, "expected at least one Array variant");
}

fn depth_of(j: &Json) -> usize {
    match j {
        Json::Null | Json::Bool(_) | Json::Num(_) => 1,
        Json::Array(xs) => 1 + xs.iter().map(depth_of).max().unwrap_or(0),
    }
}

#[test]
fn prop_recursive_strategy_drives_property() {
    let s = json_strategy();
    let outcome = forall_strategy_with(s, cfg(37), |j: &Json| {
        prop_assert!(depth_of(j) >= 1);
        true
    });
    assert!(outcome.is_passed());
}
