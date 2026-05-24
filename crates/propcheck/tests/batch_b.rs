//! Batch 1+2で追加された機能のテスト: 文字列strategy、prop_compose!、
//! prop_assert_matches!、prop_skip!、prop_with_context!、Outcomeのアクセサ。

use propcheck::strategy::{int_range, str};
use propcheck::{
    forall_strategy_with, forall_with, prop_assert, prop_assert_eq, prop_assert_matches,
    prop_compose, prop_skip, prop_with_context, run_strategy_with, Config, Outcome,
};

fn no_replay_cfg(seed: u64) -> Config {
    Config {
        seed,
        cases: 100,
        silence_panic_hook: false,
        regression_replay: false,
        ..Config::default()
    }
}

// --- prop_assert_matches! ---------------------------------------------

#[test]
fn prop_assert_matches_passes_when_pattern_matches() {
    let outcome = forall_with(no_replay_cfg(1), |_n: &u8| -> bool {
        let result: Result<u8, &'static str> = Ok(42);
        prop_assert_matches!(result, Ok(_));
        true
    });
    assert!(outcome.is_passed());
}

#[test]
fn prop_assert_matches_fails_with_pattern_and_value_in_message() {
    let outcome = forall_with(no_replay_cfg(2), |&n: &u8| -> bool {
        let result: Result<u8, &'static str> = if n > 100 { Err("too big") } else { Ok(n) };
        prop_assert_matches!(result, Ok(_));
        true
    });
    assert!(outcome.is_failed());
    let msg = outcome.failure_message().unwrap();
    assert!(msg.contains("prop_assert_matches"));
    assert!(msg.contains("pattern"));
    assert!(msg.contains("value"));
}

#[test]
fn prop_assert_matches_supports_guard() {
    let outcome = forall_with(no_replay_cfg(3), |&n: &u8| -> bool {
        let result: Result<u8, &'static str> = Ok(n);
        prop_assert_matches!(result, Ok(x) if x < 200);
        true
    });
    // u8の最大値は255なので、n >= 200となるケースに必ず到達するはずです。
    assert!(outcome.is_failed());
}

// --- prop_skip! --------------------------------------------------------

#[test]
fn prop_skip_increments_skip_counter_not_discard() {
    let outcome = forall_with(no_replay_cfg(4), |&n: &u8| -> bool {
        if n > 200 {
            prop_skip!("only test small inputs in CI");
        }
        true
    });
    assert!(outcome.is_passed());
    assert!(outcome.skipped() > 0, "expected some skips");
}

#[test]
fn too_many_skips_aborts_with_distinct_message() {
    let cfg = Config {
        cases: 50,
        max_skips: 20,
        max_discards: 100_000,
        silence_panic_hook: false,
        regression_replay: false,
        ..Config::default()
    };
    let outcome = forall_with(cfg, |_n: &u8| -> bool {
        prop_skip!("env not ready");
    });
    assert!(outcome.is_aborted());
    let msg = match &outcome {
        Outcome::Aborted { reason, .. } => reason.clone(),
        _ => unreachable!(),
    };
    assert!(msg.contains("skip"), "expected reason to mention skips, got: {msg}");
}

// --- prop_with_context! -----------------------------------------------

#[test]
fn prop_with_context_includes_context_in_failure_message() {
    let outcome = forall_with(no_replay_cfg(5), |&n: &u8| -> bool {
        prop_with_context!(format!("input n = {n}"), {
            prop_assert!(n < 10, "n was too large");
        });
        true
    });
    assert!(outcome.is_failed());
    let msg = outcome.failure_message().unwrap();
    assert!(msg.contains("context"), "expected context: in message, got {msg}");
    assert!(msg.contains("input n ="));
}

#[test]
fn prop_with_context_nests() {
    let outcome = forall_with(no_replay_cfg(6), |&n: &u8| -> bool {
        prop_with_context!("outer", {
            prop_with_context!("inner", {
                prop_assert!(n < 10);
            });
        });
        true
    });
    assert!(outcome.is_failed());
    let msg = outcome.failure_message().unwrap();
    assert!(msg.contains("outer / inner"), "got {msg}");
}

// --- Outcome accessors ------------------------------------------------

#[test]
fn outcome_accessors_work_for_each_variant() {
    let pass = forall_with(no_replay_cfg(7), |_: &u8| -> bool { true });
    assert!(pass.is_passed());
    assert!(!pass.is_failed());
    assert!(!pass.is_aborted());
    assert!(pass.failure_message().is_none());
    assert!(pass.shrunk().is_none());
    assert_eq!(pass.discarded(), 0);
    assert_eq!(pass.skipped(), 0);
    assert!(pass.seed().is_none());

    let fail = forall_with(no_replay_cfg(8), |&n: &u8| n < 100);
    assert!(fail.is_failed());
    assert!(fail.failure_message().is_some());
    assert!(fail.shrunk().is_some());
    assert_eq!(*fail.shrunk().unwrap(), 100);
    assert!(fail.seed().is_some());
}

// --- String strategies -----------------------------------------------

#[test]
fn ascii_alphanumeric_strategy_drives_property() {
    let outcome = forall_strategy_with(
        str::ascii_alphanumeric(1..20),
        no_replay_cfg(9),
        |s: &String| -> bool {
            prop_assert!(s.chars().all(|c| c.is_ascii_alphanumeric()));
            prop_assert!(!s.is_empty());
            true
        },
    );
    assert!(outcome.is_passed());
}

#[test]
fn string_strategy_shrinks_to_canonical_chars() {
    // プロパティ: 「生成されるどの文字列にも'5'が含まれない」 — 数字集合に
    // 5が含まれるため、ほぼ即座に失敗します。shrinkされた値は、5を含む
    // 最小の文字列、すなわち"5"となるはずです。
    let outcome = forall_strategy_with(
        str::ascii_digits(1..30),
        no_replay_cfg(10),
        |s: &String| !s.contains('5'),
    );
    assert!(outcome.is_failed());
    assert_eq!(outcome.shrunk().unwrap(), "5");
}

#[test]
fn hex_string_strategy_only_emits_hex_chars() {
    let outcome = forall_strategy_with(
        str::hex_string(0..10),
        no_replay_cfg(11),
        |s: &String| {
            prop_assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
            true
        },
    );
    assert!(outcome.is_passed());
}

// --- prop_compose! ----------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
struct User {
    name: String,
    age: u8,
}

prop_compose! {
    fn user_strategy()(
        name in str::ascii_alphanumeric(1..15),
        age in int_range(18u8..100),
    ) -> User {
        User { name, age }
    }
}

#[test]
fn prop_compose_strategy_produces_valid_users() {
    let strategy = user_strategy();
    let outcome = forall_strategy_with(strategy, no_replay_cfg(12), |u: &User| -> bool {
        prop_assert!(!u.name.is_empty());
        prop_assert!(u.name.chars().all(|c| c.is_ascii_alphanumeric()));
        prop_assert!(u.age >= 18 && u.age < 100);
        true
    });
    assert!(outcome.is_passed());
}

#[test]
fn prop_compose_can_drive_run_strategy() {
    // strategyがランナーに組み込めることを確認するだけのテストです。
    run_strategy_with(
        "compose drives runner",
        user_strategy(),
        no_replay_cfg(13),
        |u: &User| -> bool {
            prop_assert_eq!(u.name.len(), u.name.chars().count());
            true
        },
    );
}

// --- Tuples 5..=8 -----------------------------------------------------

#[test]
fn five_tuple_arbitrary_works() {
    let outcome = forall_with(no_replay_cfg(14), |&(a, b, c, d, e): &(u8, u8, u8, u8, u8)| {
        prop_assert_eq!(a, a);
        prop_assert_eq!(b, b);
        prop_assert_eq!(c, c);
        prop_assert_eq!(d, d);
        prop_assert_eq!(e, e);
        true
    });
    assert!(outcome.is_passed());
}

#[test]
fn eight_tuple_arbitrary_works() {
    let outcome = forall_with(
        no_replay_cfg(15),
        |&(a, b, c, d, e, f, g, h): &(u8, u8, u8, u8, u8, u8, u8, u8)| {
            let _ = (a, b, c, d, e, f, g, h);
            true
        },
    );
    assert!(outcome.is_passed());
}
