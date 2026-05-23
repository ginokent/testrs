//! Tests for the Batch 6 additions: differential testing helper and
//! `ShrinkMode::Exhaustive`.

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
    // Same impl; should agree.
    let mut v = v.to_vec();
    v.sort_unstable();
    v
}

fn buggy_sort(v: &[u8]) -> Vec<u8> {
    // Deliberately drops the last element to force divergence.
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
    // Just verify the mode setting is observed and produces a valid
    // shrunk value (it should equal Greedy's result for this trivial
    // property, since both reduce to 100).
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
    // Vec<i32> property fails when any element exceeds 50. Greedy and
    // exhaustive should both reach [51]. We assert both modes produce
    // the same answer here as a consistency check.
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
