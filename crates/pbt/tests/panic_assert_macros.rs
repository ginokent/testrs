//! `prop_assert_panic!` / `prop_assert_no_panic!` マクロのテストです。
//!
//! マクロ展開とランナーへの統合（panic payload の捕捉、SilentPanicHook 下での
//! 無出力動作）を確認するもので、PBT runner 自体の振る舞いに依存するため固定
//! seed の単体テストとして記述します。

use testrs_pbt::{forall_with, prop_assert_no_panic, prop_assert_panic, Config, Outcome};

/// SilentPanicHook を有効化し（マクロ内 catch_unwind の panic 出力を抑止）、
/// regression ファイル IO を避ける Config を返します。
fn cfg(seed: u64) -> Config {
    Config {
        seed,
        cases: 50,
        silence_panic_hook: true,
        regression_replay: false,
        ..Config::default()
    }
}

// --- prop_assert_panic! -----------------------------------------------

#[test]
fn prop_assert_panic_passes_when_expr_panics() {
    let outcome: Outcome<u8> = forall_with(cfg(1), |&_n: &u8| -> bool {
        prop_assert_panic!(panic!("boom"));
        true
    });
    assert!(
        outcome.is_passed(),
        "panic する式は assertion を通過するはず"
    );
}

#[test]
fn prop_assert_panic_fails_when_expr_does_not_panic() {
    let outcome: Outcome<u8> = forall_with(cfg(2), |&_n: &u8| -> bool {
        prop_assert_panic!(1 + 1);
        true
    });
    assert!(outcome.is_failed());
    let msg = outcome.failure_message().unwrap();
    assert!(msg.contains("did not panic"), "actual: {msg}");
}

// --- prop_assert_no_panic! --------------------------------------------

#[test]
fn prop_assert_no_panic_passes_and_returns_value() {
    let outcome: Outcome<u8> = forall_with(cfg(3), |&n: &u8| -> bool {
        // マクロは式の値を返すため、束縛して利用できる。
        let v = prop_assert_no_panic!(n.wrapping_add(1));
        v == n.wrapping_add(1)
    });
    assert!(outcome.is_passed());
}

#[test]
fn prop_assert_no_panic_fails_with_panic_message() {
    let outcome: Outcome<u8> = forall_with(cfg(4), |&_n: &u8| -> bool {
        prop_assert_no_panic!(panic!("kaboom"));
        true
    });
    assert!(outcome.is_failed());
    let msg = outcome.failure_message().unwrap();
    assert!(
        msg.contains("kaboom"),
        "panic メッセージを含むはず, actual: {msg}"
    );
}
