//! Property-based test runner.
//!
//! # Example
//!
//! ```
//! use propcheck::{run, Config};
//!
//! run("addition is commutative", |&(a, b): &(i32, i32)| {
//!     a.wrapping_add(b) == b.wrapping_add(a)
//! });
//! ```
//!
//! On failure, the runner shrinks the counterexample and panics with both
//! the original and minimized inputs plus the seed, so the run is
//! reproducible by setting `PROPCHECK_SEED`.

use std::any::Any;
use std::env;
use std::panic::{self, AssertUnwindSafe};

pub use propcheck_core::{Arbitrary, Rng, XorShift64};

/// Tunables for a property-based test run.
#[derive(Debug, Clone)]
pub struct Config {
    /// Number of randomly generated cases to try.
    pub cases: usize,
    /// PRNG seed. Defaults to `PROPCHECK_SEED` env var or wall-clock entropy.
    pub seed: u64,
    /// Maximum number of shrink steps applied to a failing case.
    pub max_shrinks: usize,
    /// Upper bound on the size hint passed to `Arbitrary::arbitrary`.
    pub max_size: usize,
    /// If `true`, the global panic hook is silenced while the run executes
    /// so failing cases don't spam the terminal. Note that the panic hook
    /// is global state, so concurrent runs in different threads may race.
    pub silence_panic_hook: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cases: 100,
            seed: env_seed(),
            max_shrinks: 1024,
            max_size: 100,
            silence_panic_hook: true,
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

/// Outcome of a property test.
#[derive(Debug)]
pub enum Outcome<A> {
    /// All cases passed.
    Passed {
        /// Number of cases executed.
        cases: usize,
    },
    /// A case failed. `original` is the first failing value, `shrunk` is the
    /// minimum the runner could reach within `max_shrinks` steps.
    Failed {
        /// The first generated value that failed the property.
        original: A,
        /// The minimized counterexample after shrinking.
        shrunk: A,
        /// Human-readable failure reason ("property returned false" or panic message).
        message: String,
        /// PRNG seed for this run, suitable for reproduction.
        seed: u64,
        /// 1-based index of the failing case.
        attempt: usize,
    },
}

/// Runs `prop` against [`Config::default`] and returns the [`Outcome`].
pub fn forall<A, F>(prop: F) -> Outcome<A>
where
    A: Arbitrary,
    F: FnMut(&A) -> bool,
{
    forall_with(Config::default(), prop)
}

/// Runs `prop` with a custom [`Config`] and returns the [`Outcome`].
pub fn forall_with<A, F>(cfg: Config, mut prop: F) -> Outcome<A>
where
    A: Arbitrary,
    F: FnMut(&A) -> bool,
{
    if cfg.silence_panic_hook {
        let prev = panic::take_hook();
        panic::set_hook(Box::new(|_| {}));
        let outcome = run_loop(&cfg, &mut prop);
        panic::set_hook(prev);
        outcome
    } else {
        run_loop(&cfg, &mut prop)
    }
}

/// Convenience wrapper that converts [`Outcome::Failed`] into a `panic!`,
/// making it suitable for use directly inside `#[test]` functions.
///
/// On success, logs a one-line summary to stderr.
pub fn run<A, F>(name: &str, prop: F)
where
    A: Arbitrary,
    F: FnMut(&A) -> bool,
{
    run_with(name, Config::default(), prop)
}

/// Same as [`run`] but takes an explicit [`Config`].
pub fn run_with<A, F>(name: &str, cfg: Config, prop: F)
where
    A: Arbitrary,
    F: FnMut(&A) -> bool,
{
    let seed = cfg.seed;
    match forall_with(cfg, prop) {
        Outcome::Passed { cases } => {
            eprintln!("[propcheck] {name}: ok ({cases} cases, seed={seed})");
        }
        Outcome::Failed {
            original,
            shrunk,
            message,
            seed,
            attempt,
        } => {
            panic!(
                "[propcheck] {name} FAILED at case #{attempt} (PROPCHECK_SEED={seed})\n  \
                 reason:   {message}\n  \
                 original: {original:?}\n  \
                 shrunk:   {shrunk:?}",
            );
        }
    }
}

fn run_loop<A, F>(cfg: &Config, prop: &mut F) -> Outcome<A>
where
    A: Arbitrary,
    F: FnMut(&A) -> bool,
{
    let mut rng = XorShift64::seed_from_u64(cfg.seed);
    let cases = cfg.cases.max(1);
    for i in 0..cases {
        // Grow size linearly: small cases first, big cases later.
        let size = 1 + (i * cfg.max_size / cases).min(cfg.max_size);
        let val: A = A::arbitrary(&mut rng, size);
        match run_prop(prop, &val) {
            Ok(true) => continue,
            other => {
                let message = match other {
                    Ok(false) => "property returned false".to_string(),
                    Err(msg) => format!("panicked: {msg}"),
                    Ok(true) => unreachable!(),
                };
                let shrunk = shrink_failure(val.clone(), prop, cfg.max_shrinks);
                return Outcome::Failed {
                    original: val,
                    shrunk,
                    message,
                    seed: cfg.seed,
                    attempt: i + 1,
                };
            }
        }
    }
    Outcome::Passed { cases }
}

fn run_prop<A, F: FnMut(&A) -> bool>(prop: &mut F, val: &A) -> Result<bool, String> {
    match panic::catch_unwind(AssertUnwindSafe(|| prop(val))) {
        Ok(b) => Ok(b),
        Err(payload) => Err(extract_panic_message(&payload)),
    }
}

fn extract_panic_message(payload: &Box<dyn Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

fn shrink_failure<A, F>(initial: A, prop: &mut F, max: usize) -> A
where
    A: Arbitrary,
    F: FnMut(&A) -> bool,
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
            // Any non-true result (false or panic) counts as "still failing".
            if !matches!(run_prop(prop, &c), Ok(true)) {
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
            // Don't fight with the test harness over the panic hook in tests.
            silence_panic_hook: false,
        }
    }

    #[test]
    fn passes_for_true_property() {
        match forall_with(cfg(1), |&(a, b): &(i32, i32)| {
            a.wrapping_add(b) == b.wrapping_add(a)
        }) {
            Outcome::Passed { cases } => assert_eq!(cases, 200),
            Outcome::Failed { .. } => panic!("commutativity should hold"),
        }
    }

    #[test]
    fn shrinks_to_small_counterexample() {
        // Intentionally false property: "no vec contains 7".
        let outcome = forall_with(cfg(42), |v: &Vec<i32>| !v.contains(&7));
        match outcome {
            Outcome::Failed { shrunk, .. } => {
                assert!(shrunk.contains(&7));
                // Shrinking should have removed unrelated elements.
                assert_eq!(shrunk, vec![7]);
            }
            Outcome::Passed { .. } => panic!("expected failure"),
        }
    }

    #[test]
    fn shrinks_integer_counterexample() {
        // Property: integer is < 10. Should shrink to exactly 10.
        let outcome = forall_with(cfg(99), |&n: &u32| n < 10);
        match outcome {
            Outcome::Failed { shrunk, .. } => assert_eq!(shrunk, 10),
            Outcome::Passed { .. } => panic!("expected failure"),
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
            Outcome::Failed { message, shrunk, .. } => {
                assert!(message.starts_with("panicked: oh no"), "message was {message:?}");
                assert_eq!(shrunk, 51);
            }
            Outcome::Passed { .. } => panic!("expected failure"),
        }
    }

    #[test]
    fn same_seed_reproduces_failure() {
        let a = forall_with(cfg(123), |&n: &u16| n < 200);
        let b = forall_with(cfg(123), |&n: &u16| n < 200);
        match (a, b) {
            (
                Outcome::Failed { original: o1, attempt: a1, .. },
                Outcome::Failed { original: o2, attempt: a2, .. },
            ) => {
                assert_eq!(o1, o2);
                assert_eq!(a1, a2);
            }
            _ => panic!("both should fail at the same case"),
        }
    }
}
