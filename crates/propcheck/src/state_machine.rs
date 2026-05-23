//! Model-based / state machine property testing.
//!
//! Useful for testing stateful APIs: containers, parsers, caches, simple
//! virtual machines, etc. You define:
//!
//! 1. The state under test (often a tuple `(system_under_test, reference_model)`).
//! 2. The set of operations to interleave.
//! 3. An `execute` function that applies one operation to the state.
//! 4. An `invariant` that must hold after every operation.
//!
//! The runner generates random operation sequences, applies them in
//! order, and reports the first sequence that violates the invariant.
//! Shrinking removes operations from the failing sequence and keeps the
//! shortest one that still violates the invariant.
//!
//! # Example
//!
//! ```
//! use propcheck::state_machine::{run_state_machine, StateMachine};
//! use propcheck::Config;
//!
//! #[derive(propcheck::Arbitrary, Debug, Clone)]
//! enum VecOp {
//!     Push(i32),
//!     Pop,
//!     Clear,
//! }
//!
//! struct VecModel;
//! impl StateMachine for VecModel {
//!     type State = Vec<i32>;
//!     type Operation = VecOp;
//!     fn initial_state() -> Self::State { Vec::new() }
//!     fn execute(state: &mut Self::State, op: &Self::Operation) {
//!         match op {
//!             VecOp::Push(n) => state.push(*n),
//!             VecOp::Pop => { state.pop(); }
//!             VecOp::Clear => state.clear(),
//!         }
//!     }
//!     fn invariant(state: &Self::State) -> Result<(), String> {
//!         // Trivially holds — change to a real check.
//!         let _ = state.len();
//!         Ok(())
//!     }
//! }
//!
//! run_state_machine::<VecModel>("vec model", Config::default());
//! ```

use std::fmt::Debug;
use std::panic::{self, AssertUnwindSafe};

use propcheck_core::{Arbitrary, XorShift64};

use crate::panic_hook::SilentPanicHook;
use crate::regression;
use crate::{Classifications, Config, Outcome};

/// Implement this trait to describe the state, operations, and invariant
/// of the system under test.
pub trait StateMachine {
    /// The state type, typically a tuple of (system_under_test, model).
    type State: Clone + Debug;
    /// One step the runner can apply to the state.
    type Operation: Arbitrary;

    /// Returns the initial state for each generated run.
    fn initial_state() -> Self::State;

    /// Applies `op` to `state`.
    fn execute(state: &mut Self::State, op: &Self::Operation);

    /// Returns `Ok(())` if the post-state is consistent. The returned
    /// error string is included in the failure report.
    fn invariant(state: &Self::State) -> Result<(), String>;
}

/// Runs a [`StateMachine`] against a series of randomly generated
/// operation sequences and panics on the first invariant violation.
pub fn run_state_machine<M: StateMachine + 'static>(name: &str, cfg: Config) {
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

    let _guard = if cfg.silence_panic_hook {
        Some(SilentPanicHook::install())
    } else {
        None
    };
    let outcome = run_sm_loop::<M>(&cfg, &regression_seeds);
    drop(_guard);

    if let Outcome::Failed { seed: fseed, .. } = &outcome {
        if let Some(path) = &regression_path {
            let _ = regression::append_seed(path, *fseed);
        }
    }

    match outcome {
        Outcome::Passed {
            cases,
            discarded,
            skipped,
            classifications,
        } => {
            let mut extra = String::new();
            if discarded > 0 {
                extra.push_str(&format!(", {discarded} discarded"));
            }
            if skipped > 0 {
                extra.push_str(&format!(", {skipped} skipped"));
            }
            eprintln!(
                "[propcheck] {name}: ok ({cases} sequences{extra}, seed={seed})"
            );
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
            skipped,
            ..
        } => panic!(
            "[propcheck] {name} FAILED at sequence #{attempt} (PROPCHECK_SEED={seed}, {discarded} discarded, {skipped} skipped)\n  \
             reason:   {message}\n  \
             original: {original:?} ({} ops)\n  \
             shrunk:   {shrunk:?} ({} ops)",
            original.len(),
            shrunk.len()
        ),
        Outcome::Aborted {
            reason,
            cases,
            seed,
            ..
        } => panic!(
            "[propcheck] {name} ABORTED (seed={seed})\n  reason: {reason}\n  sequences ran: {cases}"
        ),
    }
}

fn run_sm_loop<M: StateMachine + 'static>(
    cfg: &Config,
    regression_seeds: &[u64],
) -> Outcome<Vec<M::Operation>> {
    // Replay regression seeds first.
    for &rseed in regression_seeds {
        let mut rng = XorShift64::seed_from_u64(rseed);
        let sequence: Vec<M::Operation> = generate_sequence::<M, _>(&mut rng, cfg.max_size);
        if let Err(msg) = simulate::<M>(&sequence) {
            let shrunk = shrink_sequence::<M>(&sequence, cfg.max_shrinks);
            return Outcome::Failed {
                original: sequence,
                shrunk,
                message: format!("regression seed {rseed} reproduced: {msg}"),
                seed: rseed,
                attempt: 0,
                discarded: 0,
                skipped: 0,
                classifications: Classifications::default(),
            };
        }
    }

    let mut rng = XorShift64::seed_from_u64(cfg.seed);
    let target = cfg.cases.max(1);
    for i in 0..target {
        let sequence: Vec<M::Operation> = generate_sequence::<M, _>(&mut rng, cfg.max_size);
        match simulate::<M>(&sequence) {
            Ok(()) => continue,
            Err(msg) => {
                let shrunk = shrink_sequence::<M>(&sequence, cfg.max_shrinks);
                return Outcome::Failed {
                    original: sequence,
                    shrunk,
                    message: msg,
                    seed: cfg.seed,
                    attempt: i + 1,
                    discarded: 0,
                    skipped: 0,
                    classifications: Classifications::default(),
                };
            }
        }
    }
    Outcome::Passed {
        cases: target,
        discarded: 0,
        skipped: 0,
        classifications: Classifications::default(),
    }
}

fn generate_sequence<M, R>(rng: &mut R, max_len: usize) -> Vec<M::Operation>
where
    M: StateMachine,
    R: propcheck_core::Rng + ?Sized,
{
    let len = rng.gen_range_usize(1, max_len.max(2));
    (0..len)
        .map(|_| <M::Operation as Arbitrary>::arbitrary(rng, max_len))
        .collect()
}

/// Runs the sequence and returns Err with the invariant violation message
/// (or any panic message) on the first failing step.
fn simulate<M: StateMachine>(sequence: &[M::Operation]) -> Result<(), String> {
    let result = panic::catch_unwind(AssertUnwindSafe(|| -> Result<(), String> {
        let mut state = M::initial_state();
        for (i, op) in sequence.iter().enumerate() {
            M::execute(&mut state, op);
            if let Err(why) = M::invariant(&state) {
                return Err(format!("invariant violated after op #{i} ({op:?}): {why}"));
            }
        }
        Ok(())
    }));
    match result {
        Ok(r) => r,
        Err(payload) => {
            let msg = if let Some(s) = payload.downcast_ref::<&'static str>() {
                (*s).to_string()
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "<non-string panic>".to_string()
            };
            Err(format!("panicked: {msg}"))
        }
    }
}

/// Greedy minimization: try removing one operation at a time, keep the
/// removal when the shorter sequence still violates the invariant.
fn shrink_sequence<M: StateMachine>(
    initial: &[M::Operation],
    max_steps: usize,
) -> Vec<M::Operation> {
    let mut current: Vec<M::Operation> = initial.to_vec();
    let mut steps = 0usize;
    loop {
        if current.is_empty() {
            return current;
        }
        let mut shrunk = false;
        let mut i = 0;
        while i < current.len() {
            if steps >= max_steps {
                return current;
            }
            steps += 1;
            let mut candidate = current.clone();
            candidate.remove(i);
            if simulate::<M>(&candidate).is_err() {
                current = candidate;
                shrunk = true;
                continue;
            }
            i += 1;
        }
        if !shrunk {
            break;
        }
    }
    current
}
