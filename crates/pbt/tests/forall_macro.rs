//! `forall!` シンタックスシュガーマクロのテストです。
//!
//! マクロが単一変数では `forall(|n: &T| ...)`、複数変数では対応するタプル型
//! `forall(|(a, b): &(T, U)| ...)` へ展開されることを確認します。展開結果が
//! ランナーで実際に動くことに依存するため固定 seed の単体テストとして記述します。

use testrs_pbt::{forall, prop_assert_eq, Outcome};

#[test]
fn forall_macro_single_var_expands_to_forall() {
    let outcome: Outcome<u8> = forall! { |n: u8| {
        prop_assert_eq!(n.wrapping_add(0), *n);
        true
    }};
    assert!(outcome.is_passed());
}

#[test]
fn forall_macro_two_vars_expands_to_tuple_forall() {
    let outcome: Outcome<(i32, i32)> = forall! { |a: i32, b: i32| {
        prop_assert_eq!(a.wrapping_add(*b), b.wrapping_add(*a));
        true
    }};
    assert!(outcome.is_passed());
}

#[test]
fn forall_macro_detects_failing_property() {
    // u8 を生成すれば早晩 0 以外が現れ、n < 1 は反証される。
    let outcome: Outcome<u8> = forall! { |n: u8| *n < 1 };
    assert!(outcome.is_failed());
}
