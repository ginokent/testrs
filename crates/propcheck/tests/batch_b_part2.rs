//! Batch 6で追加された機能のテスト: 差分テストヘルパーと
//! `ShrinkMode::Exhaustive`です。

use propcheck::{differential_with, forall_with, Config, ShrinkMode};

fn no_replay_cfg(seed: u64) -> Config {
    Config {
        seed,
        cases: 100,
        silence_panic_hook: false,
        regression_replay: false,
        ..Config::default()
    }
}

// --- Differential ----------------------------------------------------

fn slow_sort(v: &[u8]) -> Vec<u8> {
    let mut v = v.to_vec();
    v.sort();
    v
}

fn fast_sort(v: &[u8]) -> Vec<u8> {
    // 同じ実装。結果は一致するはずです。
    let mut v = v.to_vec();
    v.sort_unstable();
    v
}

fn buggy_sort(v: &[u8]) -> Vec<u8> {
    // 意図的に最後の要素を捨て、差異が出るようにしています。
    let mut v = v.to_vec();
    v.sort();
    v.pop();
    v
}

#[test]
fn differential_passes_when_implementations_agree() {
    differential_with(
        "sort agreement",
        no_replay_cfg(1),
        |v: &Vec<u8>| slow_sort(v),
        |v: &Vec<u8>| fast_sort(v),
    );
}

#[test]
#[should_panic(expected = "FAILED")]
fn differential_catches_disagreement() {
    differential_with(
        "buggy",
        no_replay_cfg(2),
        |v: &Vec<u8>| slow_sort(v),
        |v: &Vec<u8>| buggy_sort(v),
    );
}

// --- Exhaustive shrink mode -----------------------------------------

#[test]
fn exhaustive_shrink_mode_runs_to_completion() {
    // モード設定が反映され、妥当な shrink 結果が得られることを確認するだけの
    // テストです（この自明なプロパティでは、両モードとも100に収束するため、
    // Greedy の結果と一致するはずです）。
    let cfg = Config {
        shrink_mode: ShrinkMode::Exhaustive,
        ..no_replay_cfg(3)
    };
    let outcome = forall_with(cfg, |&n: &u32| n < 100);
    assert!(outcome.is_failed());
    assert_eq!(*outcome.shrunk().unwrap(), 100);
}

#[test]
fn exhaustive_shrink_mode_for_nested_data() {
    // Vec<i32>のプロパティは、いずれかの要素が50を超えると失敗します。
    // Greedy と exhaustive のいずれも[51]に到達するはずです。整合性チェックとして、
    // 両モードが同じ結果を生成することをここで確認します。
    let outcome_greedy = forall_with(
        Config {
            shrink_mode: ShrinkMode::Greedy,
            ..no_replay_cfg(4)
        },
        |v: &Vec<u32>| v.iter().all(|n| *n <= 50),
    );
    let outcome_exhaustive = forall_with(
        Config {
            shrink_mode: ShrinkMode::Exhaustive,
            ..no_replay_cfg(4)
        },
        |v: &Vec<u32>| v.iter().all(|n| *n <= 50),
    );
    assert_eq!(*outcome_greedy.shrunk().unwrap(), vec![51]);
    assert_eq!(*outcome_exhaustive.shrunk().unwrap(), vec![51]);
}
