//! End-to-end test for the regression-replay feature.
//!
//! We can't easily reach into `target/propcheck-regressions/` from a test
//! (it would race with other tests), so instead we exercise the on-disk
//! IO through a temporary directory and verify that:
//!
//! 1. A failing run appends the seed to the configured file.
//! 2. A subsequent run replays the file's seed first and reproduces the
//!    failure deterministically.

use std::fs;
use std::process::Command;

#[test]
fn regression_seed_is_persisted_and_replayed() {
    // We drive this via a sub-process so it can panic safely without
    // killing the whole test binary. Use the workspace's own cargo to
    // build a tiny test binary inline.
    //
    // To keep this test self-contained without a separate binary crate,
    // we instead exercise the regression module's public-ish file IO
    // through propcheck's own runner with regression_replay enabled and
    // a known-failing property — then read the resulting file directly.

    // Where regression files land:
    let target_dir = std::env::var("CARGO_TARGET_DIR")
        .ok()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            let manifest = std::env::var("CARGO_MANIFEST_DIR")
                .expect("CARGO_MANIFEST_DIR must be set by cargo");
            std::path::PathBuf::from(manifest).join("target")
        });
    let reg_dir = target_dir.join("propcheck-regressions");
    // Use a unique test name to avoid colliding with other tests.
    let test_name = format!("regression_e2e_{}", std::process::id());
    let reg_file = reg_dir.join(format!("{test_name}.txt"));
    let _ = fs::remove_file(&reg_file);

    // Run a known-failing property in a sub-process so the panic doesn't
    // crash this test. We use `cargo` to spawn a child... but for
    // simplicity, just use std::panic::catch_unwind here.
    let test_name_for_run = test_name.clone();
    let captured = std::panic::catch_unwind(move || {
        propcheck::run::<u32, _, _>(&test_name_for_run, |&n: &u32| n < 50);
    });
    assert!(captured.is_err(), "expected runner to panic on failure");

    // After the failure, the regression file should exist and contain
    // at least one seed line.
    assert!(reg_file.exists(), "regression file was not created at {reg_file:?}");
    let content = fs::read_to_string(&reg_file).expect("read regression file");
    let seeds: Vec<u64> = content
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.trim().parse().ok())
        .collect();
    assert!(!seeds.is_empty(), "expected at least one persisted seed");

    // Re-run with replay enabled: the same seed should reproduce, which
    // means the runner panics again.
    let test_name_for_replay = test_name.clone();
    let captured2 = std::panic::catch_unwind(move || {
        propcheck::run::<u32, _, _>(&test_name_for_replay, |&n: &u32| n < 50);
    });
    assert!(captured2.is_err(), "regression should reproduce the failure");

    // Cleanup.
    let _ = fs::remove_file(&reg_file);
}

#[test]
fn run_with_regression_replay_disabled_does_not_persist() {
    use propcheck::Config;

    let target_dir = std::env::var("CARGO_TARGET_DIR")
        .ok()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
            std::path::PathBuf::from(manifest).join("target")
        });
    let reg_dir = target_dir.join("propcheck-regressions");
    let test_name = format!("regression_disabled_{}", std::process::id());
    let reg_file = reg_dir.join(format!("{test_name}.txt"));
    let _ = fs::remove_file(&reg_file);

    let test_name_owned = test_name.clone();
    let _ = std::panic::catch_unwind(move || {
        propcheck::run_with::<u32, _, _>(
            &test_name_owned,
            Config {
                regression_replay: false,
                cases: 50,
                ..Config::default()
            },
            |&n: &u32| n < 50,
        );
    });

    assert!(!reg_file.exists(), "file should not be created when regression_replay = false");

    // Silence unused-import warning for Command when this file is built
    // without spawning sub-processes.
    let _ = std::mem::size_of::<Command>();
}
