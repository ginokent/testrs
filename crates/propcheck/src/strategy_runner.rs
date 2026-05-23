//! Runner variants driven by a [`Strategy`] instead of the type's
//! [`Arbitrary`] impl.

use std::fmt::Debug;

use propcheck_core::strategy::Strategy;
use propcheck_core::XorShift64;

use crate::classify::{self, Classifications};
use crate::panic_hook::SilentPanicHook;
use crate::{run_prop, CaseOutcome, Config, IntoPropResult, Outcome, PropResult};

/// Runs `prop` against values produced by `strategy`.
pub fn forall_strategy<S, F, R>(strategy: S, prop: F) -> Outcome<S::Value>
where
    S: Strategy,
    S::Value: Debug,
    F: FnMut(&S::Value) -> R,
    R: IntoPropResult,
{
    forall_strategy_with(strategy, Config::default(), prop)
}

/// Runs `prop` against values produced by `strategy` with a custom config.
pub fn forall_strategy_with<S, F, R>(
    strategy: S,
    cfg: Config,
    mut prop: F,
) -> Outcome<S::Value>
where
    S: Strategy,
    S::Value: Debug,
    F: FnMut(&S::Value) -> R,
    R: IntoPropResult,
{
    let mut wrapped = move |val: &S::Value| prop(val).into_prop_result();
    let _guard = if cfg.silence_panic_hook {
        Some(SilentPanicHook::install())
    } else {
        None
    };
    run_strategy_loop(&strategy, &cfg, &mut wrapped)
}

/// Like [`forall_strategy`] but panics on failure, suitable for `#[test]`.
pub fn run_strategy<S, F, R>(name: &str, strategy: S, prop: F)
where
    S: Strategy,
    S::Value: Debug,
    F: FnMut(&S::Value) -> R,
    R: IntoPropResult,
{
    run_strategy_with(name, strategy, Config::default(), prop)
}

/// Like [`forall_strategy_with`] but panics on failure.
pub fn run_strategy_with<S, F, R>(name: &str, strategy: S, cfg: Config, prop: F)
where
    S: Strategy,
    S::Value: Debug,
    F: FnMut(&S::Value) -> R,
    R: IntoPropResult,
{
    let seed = cfg.seed;
    match forall_strategy_with(strategy, cfg, prop) {
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
            eprintln!("[propcheck] {name}: ok ({cases} cases{extra}, seed={seed})");
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
            classifications,
        } => {
            let cpart = if classifications.is_empty() {
                String::new()
            } else {
                format!("\n  classifications:\n{}", classifications.render())
            };
            panic!(
                "[propcheck] {name} FAILED at case #{attempt} (PROPCHECK_SEED={seed}, {discarded} discarded, {skipped} skipped)\n  \
                 reason:   {message}\n  \
                 original: {original:?}\n  \
                 shrunk:   {shrunk:?}{cpart}"
            );
        }
        Outcome::Aborted {
            reason,
            cases,
            discarded,
            skipped,
            seed,
            classifications,
        } => {
            let cpart = if classifications.is_empty() {
                String::new()
            } else {
                format!("\n  classifications:\n{}", classifications.render())
            };
            panic!(
                "[propcheck] {name} ABORTED (seed={seed})\n  reason: {reason}\n  cases ran: {cases}, discarded: {discarded}, skipped: {skipped}{cpart}"
            );
        }
    }
}

fn run_strategy_loop<S, F>(strategy: &S, cfg: &Config, prop: &mut F) -> Outcome<S::Value>
where
    S: Strategy,
    S::Value: Debug,
    F: FnMut(&S::Value) -> PropResult,
{
    let mut rng = XorShift64::seed_from_u64(cfg.seed);
    let target_cases = cfg.cases.max(1);
    let mut passed = 0usize;
    let mut discarded = 0usize;
    let mut skipped = 0usize;
    let mut classifications = Classifications::default();

    while passed < target_cases {
        if discarded > cfg.max_discards {
            return Outcome::Aborted {
                reason: format!("too many discards: {discarded} (limit {})", cfg.max_discards),
                cases: passed,
                discarded,
                skipped,
                seed: cfg.seed,
                classifications,
            };
        }
        if skipped > cfg.max_skips {
            return Outcome::Aborted {
                reason: format!("too many skips: {skipped} (limit {})", cfg.max_skips),
                cases: passed,
                discarded,
                skipped,
                seed: cfg.seed,
                classifications,
            };
        }
        let size = 1 + (passed * cfg.max_size / target_cases).min(cfg.max_size);
        let val = strategy.new_value(&mut rng, size);
        classify::reset_current();
        let outcome = run_prop(prop, &val);
        let labels = classify::take_current();
        match outcome {
            CaseOutcome::Pass => {
                passed += 1;
                classifications.merge_case(labels);
            }
            CaseOutcome::Discard => discarded += 1,
            CaseOutcome::Skip(_) => skipped += 1,
            CaseOutcome::Fail(message) => {
                classifications.merge_case(labels);
                let shrunk = shrink_with_strategy(strategy, val.clone(), prop, cfg.max_shrinks);
                return Outcome::Failed {
                    original: val,
                    shrunk,
                    message,
                    seed: cfg.seed,
                    attempt: passed + 1,
                    discarded,
                    skipped,
                    classifications,
                };
            }
        }
    }
    Outcome::Passed {
        cases: passed,
        discarded,
        skipped,
        classifications,
    }
}

fn shrink_with_strategy<S, F>(strategy: &S, initial: S::Value, prop: &mut F, max: usize) -> S::Value
where
    S: Strategy,
    F: FnMut(&S::Value) -> PropResult,
{
    let mut current = initial;
    let mut attempts = 0;
    'outer: loop {
        let candidates = strategy.shrink_value(&current);
        if candidates.is_empty() {
            return current;
        }
        for c in candidates {
            if attempts >= max {
                return current;
            }
            attempts += 1;
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

/// Builds a [`one_of`](propcheck_core::strategy::one_of) strategy from a
/// variadic list of sub-strategies, automatically boxing each so they may
/// have different concrete types.
///
/// Two forms are supported:
///
/// - Uniform: `prop_oneof![strat_a, strat_b, strat_c]`
/// - Weighted: `prop_oneof![1 => strat_a, 4 => strat_b]`
#[macro_export]
macro_rules! prop_oneof {
    ($($w:literal => $strat:expr),+ $(,)?) => {
        $crate::strategy::weighted_one_of(vec![
            $( ($w as u32, $crate::strategy::StrategyExt::boxed($strat)), )+
        ])
    };
    ($($strat:expr),+ $(,)?) => {
        $crate::strategy::one_of(vec![
            $( $crate::strategy::StrategyExt::boxed($strat), )+
        ])
    };
}

/// Builds a recursive-data strategy by stacking `inner` over `leaf` a
/// fixed number of times. Useful for trees, ASTs, JSON-like values.
///
/// ```ignore
/// use propcheck::{prop_recursive, prop_oneof};
/// use propcheck::strategy::{any, just, vec_of, StrategyExt};
///
/// #[derive(propcheck::Arbitrary, Debug, Clone)]
/// enum Json { Null, Bool(bool), Num(i32), Array(Vec<Json>) }
///
/// let s = prop_recursive! {
///     leaf = prop_oneof![
///         just(Json::Null),
///         any::<bool>().map(Json::Bool),
///         any::<i32>().map(Json::Num),
///     ],
///     inner = |child| prop_oneof![
///         just(Json::Null),
///         any::<bool>().map(Json::Bool),
///         any::<i32>().map(Json::Num),
///         vec_of(child, 0..4).map(Json::Array),
///     ],
///     max_depth = 3,
/// };
/// ```
///
/// `leaf` and the result of `inner` must yield the same value type. The
/// `inner` closure takes a `BoxedStrategy<T>` representing the next-deeper
/// level and returns a `BoxedStrategy<T>` for the current level.
#[macro_export]
macro_rules! prop_recursive {
    (
        leaf = $leaf:expr,
        inner = $inner:expr,
        max_depth = $depth:expr $(,)?
    ) => {
        $crate::strategy::recursive($leaf, $inner, $depth)
    };
}

/// Convenience macro for `Strategy::filter` with a label hint included
/// in the panic message if the filter rejects every candidate.
///
/// ```ignore
/// let s = prop_filter!("only even", any::<i32>(), |n| n % 2 == 0);
/// ```
///
/// Note: the label is currently informational only; rejecting candidates
/// produce the standard `Filter: predicate rejected every candidate`
/// message. Future versions may surface the label.
#[macro_export]
macro_rules! prop_filter {
    ($label:expr, $strat:expr, $pred:expr $(,)?) => {{
        let _ = $label;
        $crate::strategy::StrategyExt::filter($strat, $pred)
    }};
    ($strat:expr, $pred:expr $(,)?) => {
        $crate::strategy::StrategyExt::filter($strat, $pred)
    };
}

/// Defines a function returning a composite [`Strategy`] built from named
/// sub-strategies and a body expression.
///
/// ```ignore
/// use propcheck::strategy::str;
/// propcheck::prop_compose! {
///     pub fn valid_user()(
///         name in str::ascii_alphanumeric(1..20),
///         age in 18u8..100,
///     ) -> User {
///         User { name, age }
///     }
/// }
/// ```
///
/// Each binding `name in expr` uses `expr` as a [`Strategy`] (or any value
/// implementing `Strategy`; ranges like `0..10` work via the `IntRange`
/// strategy's blanket impls). The body must return the target value type.
///
/// Shrinking is not preserved across `prop_compose!` — the generated
/// strategy shrinks by re-deriving from each sub-strategy's shrinks for
/// each field independently. For complex domains write a manual `Strategy`
/// impl if you need finer-grained control.
#[macro_export]
macro_rules! prop_compose {
    (
        $(#[$attr:meta])*
        $vis:vis fn $name:ident ( $($extra:tt)* ) ( $($field:ident in $strat:expr),* $(,)? ) -> $ret:ty
        $body:block
    ) => {
        $(#[$attr])*
        $vis fn $name ( $($extra)* ) -> impl $crate::strategy::Strategy<Value = $ret> {
            $crate::__ComposedStrategy {
                builder: ::std::sync::Arc::new(move |__rng: &mut dyn $crate::Rng, __size: usize| -> $ret {
                    $(
                        let $field = $crate::strategy::Strategy::new_value(&$strat, __rng, __size);
                    )*
                    $body
                }),
            }
        }
    };
}

/// Internal: a strategy whose `new_value` is a boxed closure and whose
/// `shrink_value` returns nothing (composed strategies don't have a
/// natural shrink path without preserving the input variables). Used by
/// [`prop_compose!`].
#[doc(hidden)]
pub struct __ComposedStrategy<T> {
    pub builder: ComposedBuilder<T>,
}

#[doc(hidden)]
pub type ComposedBuilder<T> =
    std::sync::Arc<dyn Fn(&mut dyn propcheck_core::Rng, usize) -> T + Send + Sync>;

impl<T: Clone + std::fmt::Debug + 'static> propcheck_core::Strategy for __ComposedStrategy<T> {
    type Value = T;
    fn new_value<R: propcheck_core::Rng + ?Sized>(&self, rng: &mut R, size: usize) -> T {
        // Use the same sized-wrapper trick as BoxedStrategy: wrap rng so
        // it can be coerced to `&mut dyn Rng`.
        struct W<'a, R: propcheck_core::Rng + ?Sized> {
            inner: &'a mut R,
        }
        impl<R: propcheck_core::Rng + ?Sized> propcheck_core::Rng for W<'_, R> {
            fn next_u64(&mut self) -> u64 {
                self.inner.next_u64()
            }
        }
        let mut wrap = W { inner: rng };
        (self.builder)(&mut wrap, size)
    }
    fn shrink_value(&self, _value: &T) -> Vec<T> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use propcheck_core::strategy::{int_range, just};

    fn cfg(seed: u64) -> Config {
        Config {
            cases: 100,
            seed,
            max_shrinks: 256,
            max_size: 50,
            max_discards: 1000,
            max_skips: 1000,
            silence_panic_hook: false,
            regression_replay: false,
            shrink_mode: crate::ShrinkMode::Greedy,
        }
    }

    #[test]
    fn forall_strategy_runs_with_constrained_generator() {
        let s = int_range(0i32..100);
        match forall_strategy_with(s, cfg(1), |&n: &i32| (0..100).contains(&n)) {
            Outcome::Passed { cases, .. } => assert_eq!(cases, 100),
            other => panic!("expected pass, got {other:?}"),
        }
    }

    #[test]
    fn forall_strategy_shrinks_via_strategy() {
        let s = int_range(0i32..1000);
        let outcome = forall_strategy_with(s, cfg(2), |&n: &i32| n < 500);
        match outcome {
            Outcome::Failed { shrunk, .. } => assert_eq!(shrunk, 500),
            other => panic!("expected failure, got {other:?}"),
        }
    }

    #[test]
    fn prop_oneof_uniform_compiles_and_runs() {
        let s = crate::prop_oneof![just(1i32), just(2), just(3)];
        let mut seen = std::collections::HashSet::new();
        match forall_strategy_with(s, cfg(3), |&n: &i32| {
            seen.insert(n);
            (1..=3).contains(&n)
        }) {
            Outcome::Passed { .. } => {}
            other => panic!("expected pass, got {other:?}"),
        }
        assert!(seen.len() > 1, "expected variety");
    }

    #[test]
    fn prop_oneof_weighted_compiles_and_runs() {
        let s = crate::prop_oneof![99 => just(0i32), 1 => just(1)];
        let mut zeros = 0;
        let mut ones = 0;
        let _ = forall_strategy_with(s, cfg(4), |&n: &i32| {
            if n == 0 {
                zeros += 1
            } else {
                ones += 1
            }
            true
        });
        assert!(zeros > ones * 5, "expected heavy bias toward 0");
    }
}
