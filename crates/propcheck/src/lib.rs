//! Property-based test runner.
//!
//! Use [`run`] from inside a `#[test]` function (or write `#[propcheck]` on
//! a free function for the same effect with less boilerplate) to assert a
//! property over many randomly generated inputs:
//!
//! ```
//! use propcheck::{run, prop_assert_eq};
//!
//! run("addition is commutative", |&(a, b): &(i32, i32)| {
//!     prop_assert_eq!(a.wrapping_add(b), b.wrapping_add(a));
//!     true
//! });
//! ```
//!
//! Inside a property body you can use:
//!
//! - [`prop_assert!`], [`prop_assert_eq!`], [`prop_assert_ne!`] for
//!   assertions with rich failure messages.
//! - [`prop_assume!`] to discard cases that don't satisfy a precondition.
//! - [`classify!`] to record per-case labels; the runner aggregates them
//!   into a percentage table reported alongside the pass/fail summary.
//!
//! Property bodies may return any type that implements [`IntoPropResult`]:
//! `bool`, `()`, `Result<(), E>`, or [`PropResult`] directly. This lets
//! you use the `?` operator inside the property body.
//!
//! Failure output includes the seed of the run so it can be reproduced
//! deterministically by setting the `PROPCHECK_SEED` env var. Failing
//! seeds are also persisted to `target/propcheck-regressions/<name>.txt`
//! and replayed on subsequent runs.

use std::any::Any;
use std::env;
use std::panic::{self, AssertUnwindSafe};

pub use propcheck_core::{Arbitrary, Rng, XorShift64};

mod assert;
pub mod classify;
mod panic_hook;
mod regression;
pub mod strategy_runner;

pub use assert::{PropAssertFailure, PropDiscard};
pub use classify::Classifications;
pub use propcheck_core::strategy;
// `Arbitrary` as both the trait (from propcheck-core, type namespace) and the
// derive macro (from propcheck-derive, macro namespace).
pub use propcheck_derive::{propcheck, Arbitrary};
pub use strategy_runner::{forall_strategy, forall_strategy_with, run_strategy, run_strategy_with};

use panic_hook::SilentPanicHook;

// ---------------------------------------------------------------------------
// IntoPropResult: lets properties return bool, (), Result, or PropResult.
// ---------------------------------------------------------------------------

/// The outcome of a single property case before the runner classifies it.
#[derive(Debug)]
pub enum PropResult {
    /// Case passed.
    Pass,
    /// Case failed; carries a human-readable reason.
    Fail(String),
    /// Case discarded (precondition not met); does not count toward `cases`.
    Discard,
}

/// Trait that lets property bodies return any of `bool`, `()`,
/// `Result<(), E>`, or [`PropResult`] without an explicit conversion.
pub trait IntoPropResult {
    /// Converts `self` into a [`PropResult`].
    fn into_prop_result(self) -> PropResult;
}

impl IntoPropResult for bool {
    fn into_prop_result(self) -> PropResult {
        if self {
            PropResult::Pass
        } else {
            PropResult::Fail("property returned false".to_string())
        }
    }
}

impl IntoPropResult for () {
    fn into_prop_result(self) -> PropResult {
        PropResult::Pass
    }
}

impl IntoPropResult for PropResult {
    fn into_prop_result(self) -> PropResult {
        self
    }
}

impl<E: std::fmt::Debug> IntoPropResult for Result<(), E> {
    fn into_prop_result(self) -> PropResult {
        match self {
            Ok(()) => PropResult::Pass,
            Err(e) => PropResult::Fail(format!("returned Err({e:?})")),
        }
    }
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Tunables for a property-based test run.
#[derive(Debug, Clone)]
pub struct Config {
    /// Target number of *passing* cases to execute.
    pub cases: usize,
    /// PRNG seed. Defaults to `PROPCHECK_SEED` env var or wall-clock entropy.
    pub seed: u64,
    /// Maximum number of shrink steps applied to a failing case.
    pub max_shrinks: usize,
    /// Upper bound on the size hint passed to `Arbitrary::arbitrary`.
    pub max_size: usize,
    /// Maximum total discards allowed before the run is aborted. Defaults
    /// to `cases * 10` — i.e. up to ten discards per kept case.
    pub max_discards: usize,
    /// If `true`, the global panic hook is silenced while the run executes
    /// so failing cases don't spam the terminal. Refcounted internally so
    /// concurrent runners share a single hook installation.
    pub silence_panic_hook: bool,
    /// If `true`, [`run`] / [`run_with`] persist failing seeds under
    /// `target/propcheck-regressions/<name>.txt` and replay them at the
    /// start of subsequent runs.
    pub regression_replay: bool,
}

impl Default for Config {
    fn default() -> Self {
        let cases = 100;
        Self {
            cases,
            seed: env_seed(),
            max_shrinks: 1024,
            max_size: 100,
            max_discards: cases * 10,
            silence_panic_hook: true,
            regression_replay: true,
        }
    }
}

fn env_seed() -> u64 {
    if let Ok(s) = env::var("PROPCHECK_SEED") {
        if let Ok(n) = s.parse::<u64>() {
            return n;
        }
    }
    XorShift64::from_entropy().state()
}

// ---------------------------------------------------------------------------
// Outcome
// ---------------------------------------------------------------------------

/// Outcome of a property test.
#[derive(Debug)]
pub enum Outcome<A> {
    /// All required cases passed (possibly after some discards).
    Passed {
        cases: usize,
        discarded: usize,
        classifications: Classifications,
    },
    /// A case failed.
    Failed {
        original: A,
        shrunk: A,
        message: String,
        seed: u64,
        attempt: usize,
        discarded: usize,
        classifications: Classifications,
    },
    /// Run aborted before completion (e.g. too many discards).
    Aborted {
        reason: String,
        cases: usize,
        discarded: usize,
        seed: u64,
        classifications: Classifications,
    },
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Runs `prop` against [`Config::default`] and returns the [`Outcome`].
pub fn forall<A, F, R>(prop: F) -> Outcome<A>
where
    A: Arbitrary,
    F: FnMut(&A) -> R,
    R: IntoPropResult,
{
    forall_with(Config::default(), prop)
}

/// Runs `prop` with a custom [`Config`] and returns the [`Outcome`].
pub fn forall_with<A, F, R>(cfg: Config, mut prop: F) -> Outcome<A>
where
    A: Arbitrary,
    F: FnMut(&A) -> R,
    R: IntoPropResult,
{
    let mut wrapped = move |val: &A| prop(val).into_prop_result();
    let _guard = if cfg.silence_panic_hook {
        Some(SilentPanicHook::install())
    } else {
        None
    };
    run_loop(&cfg, &mut wrapped, &[])
}

/// Convenience wrapper that converts [`Outcome::Failed`] into a `panic!`,
/// making it suitable for use directly inside `#[test]` functions.
pub fn run<A, F, R>(name: &str, prop: F)
where
    A: Arbitrary,
    F: FnMut(&A) -> R,
    R: IntoPropResult,
{
    run_with(name, Config::default(), prop)
}

/// Same as [`run`] but takes an explicit [`Config`].
pub fn run_with<A, F, R>(name: &str, cfg: Config, mut prop: F)
where
    A: Arbitrary,
    F: FnMut(&A) -> R,
    R: IntoPropResult,
{
    let seed = cfg.seed;
    let regression_path = if cfg.regression_replay {
        regression::regression_file_path(name)
    } else {
        None
    };
    let regression_seeds = regression_path
        .as_deref()
        .map(regression::read_seeds)
        .unwrap_or_default();

    let mut wrapped = move |val: &A| prop(val).into_prop_result();
    let _guard = if cfg.silence_panic_hook {
        Some(SilentPanicHook::install())
    } else {
        None
    };
    let outcome = run_loop(&cfg, &mut wrapped, &regression_seeds);
    drop(_guard);

    // Persist new failing seed for future runs.
    if let Outcome::Failed { seed: failed_seed, .. } = &outcome {
        if let Some(path) = &regression_path {
            let _ = regression::append_seed(path, *failed_seed);
        }
    }

    match outcome {
        Outcome::Passed {
            cases,
            discarded,
            classifications,
        } => {
            let discard_part = if discarded == 0 {
                String::new()
            } else {
                format!(", {discarded} discarded")
            };
            eprintln!("[propcheck] {name}: ok ({cases} cases{discard_part}, seed={seed})");
            if !classifications.is_empty() {
                eprint!("  classifications:\n{}", classifications.render());
            }
        }
        Outcome::Failed {
            original,
            shrunk,
            message,
            seed,
            attempt,
            discarded,
            classifications,
        } => {
            let class_part = if classifications.is_empty() {
                String::new()
            } else {
                format!("\n  classifications:\n{}", classifications.render())
            };
            panic!(
                "[propcheck] {name} FAILED at case #{attempt} (PROPCHECK_SEED={seed}, {discarded} discarded)\n  \
                 reason:   {message}\n  \
                 original: {original:?}\n  \
                 shrunk:   {shrunk:?}{class_part}",
            );
        }
        Outcome::Aborted {
            reason,
            cases,
            discarded,
            seed,
            classifications,
        } => {
            let class_part = if classifications.is_empty() {
                String::new()
            } else {
                format!("\n  classifications:\n{}", classifications.render())
            };
            panic!(
                "[propcheck] {name} ABORTED (seed={seed})\n  \
                 reason: {reason}\n  \
                 cases ran: {cases}, discarded: {discarded}{class_part}",
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Inner loop
// ---------------------------------------------------------------------------

fn run_loop<A, F>(cfg: &Config, prop: &mut F, regression_seeds: &[u64]) -> Outcome<A>
where
    A: Arbitrary,
    F: FnMut(&A) -> PropResult,
{
    // 1. Replay regression seeds first.
    for &rseed in regression_seeds {
        let mut rng = XorShift64::seed_from_u64(rseed);
        let size = (cfg.max_size / 2).max(1);
        let val: A = A::arbitrary(&mut rng, size);
        classify::reset_current();
        if let CaseOutcome::Fail(msg) = run_prop(prop, &val) {
            let _labels = classify::take_current();
            let shrunk = shrink_failure(val.clone(), prop, cfg.max_shrinks);
            return Outcome::Failed {
                original: val,
                shrunk,
                message: format!("regression seed {rseed} reproduced: {msg}"),
                seed: rseed,
                attempt: 0,
                discarded: 0,
                classifications: Classifications::default(),
            };
        }
        let _ = classify::take_current();
    }

    // 2. Main loop.
    let mut rng = XorShift64::seed_from_u64(cfg.seed);
    let target_cases = cfg.cases.max(1);
    let mut passed = 0usize;
    let mut discarded = 0usize;
    let mut classifications = Classifications::default();

    while passed < target_cases {
        if discarded > cfg.max_discards {
            return Outcome::Aborted {
                reason: format!(
                    "too many discards: {discarded} (limit {max}) before {target_cases} cases completed",
                    max = cfg.max_discards
                ),
                cases: passed,
                discarded,
                seed: cfg.seed,
                classifications,
            };
        }
        let size = 1 + (passed * cfg.max_size / target_cases).min(cfg.max_size);
        let val: A = A::arbitrary(&mut rng, size);
        classify::reset_current();
        let outcome = run_prop(prop, &val);
        let labels = classify::take_current();
        match outcome {
            CaseOutcome::Pass => {
                passed += 1;
                classifications.merge_case(labels);
            }
            CaseOutcome::Discard => {
                discarded += 1;
            }
            CaseOutcome::Fail(message) => {
                classifications.merge_case(labels);
                let shrunk = shrink_failure(val.clone(), prop, cfg.max_shrinks);
                return Outcome::Failed {
                    original: val,
                    shrunk,
                    message,
                    seed: cfg.seed,
                    attempt: passed + 1,
                    discarded,
                    classifications,
                };
            }
        }
    }
    Outcome::Passed {
        cases: passed,
        discarded,
        classifications,
    }
}

/// Result of running the property on one case.
pub(crate) enum CaseOutcome {
    Pass,
    Fail(String),
    Discard,
}

pub(crate) fn run_prop<A, F>(prop: &mut F, val: &A) -> CaseOutcome
where
    F: FnMut(&A) -> PropResult,
{
    match panic::catch_unwind(AssertUnwindSafe(|| prop(val))) {
        Ok(PropResult::Pass) => CaseOutcome::Pass,
        Ok(PropResult::Fail(m)) => CaseOutcome::Fail(m),
        Ok(PropResult::Discard) => CaseOutcome::Discard,
        Err(payload) => classify_panic(&payload),
    }
}

pub(crate) fn classify_panic(payload: &Box<dyn Any + Send>) -> CaseOutcome {
    if payload.downcast_ref::<PropDiscard>().is_some() {
        return CaseOutcome::Discard;
    }
    if let Some(f) = payload.downcast_ref::<PropAssertFailure>() {
        return CaseOutcome::Fail(f.message.clone());
    }
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        return CaseOutcome::Fail(format!("panicked: {s}"));
    }
    if let Some(s) = payload.downcast_ref::<String>() {
        return CaseOutcome::Fail(format!("panicked: {s}"));
    }
    CaseOutcome::Fail("<non-string panic payload>".to_string())
}

fn shrink_failure<A, F>(initial: A, prop: &mut F, max: usize) -> A
where
    A: Arbitrary,
    F: FnMut(&A) -> PropResult,
{
    let mut current = initial;
    let mut attempts = 0;
    'outer: loop {
        let candidates: Vec<A> = current.shrink().collect();
        if candidates.is_empty() {
            return current;
        }
        for c in candidates {
            if attempts >= max {
                return current;
            }
            attempts += 1;
            // Discard during shrinking counts as "pass" — we can't minimize
            // toward a value the property never sees.
            classify::reset_current();
            let r = run_prop(prop, &c);
            let _ = classify::take_current();
            if matches!(r, CaseOutcome::Fail(_)) {
                current = c;
                continue 'outer;
            }
        }
        return current;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(seed: u64) -> Config {
        Config {
            cases: 200,
            seed,
            max_shrinks: 512,
            max_size: 50,
            max_discards: 5000,
            silence_panic_hook: false,
            regression_replay: false,
        }
    }

    #[test]
    fn passes_for_true_property() {
        match forall_with(cfg(1), |&(a, b): &(i32, i32)| {
            a.wrapping_add(b) == b.wrapping_add(a)
        }) {
            Outcome::Passed { cases, .. } => assert_eq!(cases, 200),
            other => panic!("expected pass, got {other:?}"),
        }
    }

    #[test]
    fn unit_return_means_pass() {
        // No `prop_assert!` and no return value should be Pass.
        let out: Outcome<u32> = forall_with(cfg(5), |_n: &u32| {});
        match out {
            Outcome::Passed { cases, .. } => assert_eq!(cases, 200),
            other => panic!("expected pass, got {other:?}"),
        }
    }

    #[test]
    fn result_err_means_fail() {
        let out: Outcome<u8> = forall_with(cfg(6), |&n: &u8| -> Result<(), String> {
            if n > 50 {
                Err(format!("too big: {n}"))
            } else {
                Ok(())
            }
        });
        match out {
            Outcome::Failed { message, .. } => assert!(message.contains("too big")),
            other => panic!("expected fail, got {other:?}"),
        }
    }

    #[test]
    fn shrinks_to_small_counterexample() {
        let outcome = forall_with(cfg(42), |v: &Vec<i32>| !v.contains(&7));
        match outcome {
            Outcome::Failed { shrunk, .. } => {
                assert_eq!(shrunk, vec![7]);
            }
            other => panic!("expected failure, got {other:?}"),
        }
    }

    #[test]
    fn captures_panic_message() {
        let outcome = forall_with(cfg(7), |&n: &u8| {
            if n > 50 {
                panic!("oh no: {n}");
            }
            true
        });
        match outcome {
            Outcome::Failed {
                message, shrunk, ..
            } => {
                assert!(
                    message.starts_with("panicked: oh no"),
                    "message was {message:?}"
                );
                assert_eq!(shrunk, 51);
            }
            other => panic!("expected failure, got {other:?}"),
        }
    }

    #[test]
    fn prop_assert_carries_message() {
        let outcome = forall_with(cfg(11), |&n: &u8| {
            crate::prop_assert!(n < 100, "expected n < 100, got {n}");
            true
        });
        match outcome {
            Outcome::Failed { message, .. } => {
                assert!(message.contains("expected n < 100"));
                assert!(message.contains("prop_assert!"));
            }
            other => panic!("expected failure, got {other:?}"),
        }
    }

    #[test]
    fn prop_assert_eq_shows_both_sides() {
        let outcome = forall_with(cfg(13), |&n: &u8| {
            crate::prop_assert_eq!(n, n.wrapping_add(1));
            true
        });
        match outcome {
            Outcome::Failed { message, .. } => {
                assert!(message.contains("left:"));
                assert!(message.contains("right:"));
            }
            other => panic!("expected failure, got {other:?}"),
        }
    }

    #[test]
    fn prop_assume_discards_unwanted_cases() {
        let cfg = Config {
            cases: 50,
            ..cfg(17)
        };
        let outcome = forall_with(cfg, |&n: &i32| {
            crate::prop_assume!(n > 0);
            n > 0
        });
        match outcome {
            Outcome::Passed { cases, discarded, .. } => {
                assert_eq!(cases, 50);
                assert!(discarded > 0, "expected some discards from negative inputs");
            }
            other => panic!("expected pass, got {other:?}"),
        }
    }

    #[test]
    fn too_many_discards_aborts() {
        let cfg = Config {
            cases: 100,
            max_discards: 50,
            ..cfg(19)
        };
        let outcome = forall_with(cfg, |_: &i32| {
            crate::prop_assume!(false);
            true
        });
        match outcome {
            Outcome::Aborted { discarded, .. } => {
                assert!(discarded > 50);
            }
            other => panic!("expected abort, got {other:?}"),
        }
    }

    #[test]
    fn classify_collects_labels() {
        let outcome = forall_with(cfg(23), |&n: &i32| {
            crate::classify!(n == 0, "zero");
            crate::classify!(n > 0, "positive");
            crate::classify!(n < 0, "negative");
            true
        });
        match outcome {
            Outcome::Passed {
                classifications, ..
            } => {
                assert!(classifications.counts().contains_key("positive"));
                assert!(classifications.counts().contains_key("negative"));
                assert_eq!(classifications.total(), 200);
            }
            other => panic!("expected pass, got {other:?}"),
        }
    }

    #[test]
    fn same_seed_reproduces_failure() {
        let a = forall_with(cfg(123), |&n: &u16| n < 200);
        let b = forall_with(cfg(123), |&n: &u16| n < 200);
        match (a, b) {
            (
                Outcome::Failed {
                    original: o1,
                    attempt: a1,
                    ..
                },
                Outcome::Failed {
                    original: o2,
                    attempt: a2,
                    ..
                },
            ) => {
                assert_eq!(o1, o2);
                assert_eq!(a1, a2);
            }
            _ => panic!("both should fail at the same case"),
        }
    }
}
