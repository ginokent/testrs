//! 型の [`Arbitrary`](crate::Arbitrary) 実装の代わりに [`Strategy`]
//! によって駆動されるランナーの variant です。

use std::fmt::Debug;

use testrs_core::strategy::Strategy;
use testrs_core::XorShift64;

use crate::classify::{self, Classifications};
use crate::panic_hook::SilentPanicHook;
use crate::{run_prop, CaseOutcome, Config, IntoPropResult, Outcome, PropResult};

/// `strategy` が生成した値に対して `prop` を実行します。
pub fn forall_strategy<S, F, R>(strategy: S, prop: F) -> Outcome<S::Value>
where
    S: Strategy,
    S::Value: Debug,
    F: FnMut(&S::Value) -> R,
    R: IntoPropResult,
{
    forall_strategy_with(strategy, Config::default(), prop)
}

/// カスタム設定で `strategy` が生成した値に対して `prop` を実行します。
pub fn forall_strategy_with<S, F, R>(strategy: S, cfg: Config, mut prop: F) -> Outcome<S::Value>
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

/// [`forall_strategy`] と同様ですが、失敗時に panic するため `#[test]` に適しています。
pub fn run_strategy<S, F, R>(name: &str, strategy: S, prop: F)
where
    S: Strategy,
    S::Value: Debug,
    F: FnMut(&S::Value) -> R,
    R: IntoPropResult,
{
    run_strategy_with(name, strategy, Config::default(), prop)
}

/// [`forall_strategy_with`] と同様ですが、失敗時に panic します。
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
            eprintln!("[testrs-pbt] {name}: ok ({cases} cases{extra}, seed={seed})");
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
                "[testrs-pbt] {name} FAILED at case #{attempt} (TESTRS_PBT_SEED={seed}, {discarded} discarded, {skipped} skipped)\n  \
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
                "[testrs-pbt] {name} ABORTED (seed={seed})\n  reason: {reason}\n  cases ran: {cases}, discarded: {discarded}, skipped: {skipped}{cpart}"
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
                reason: format!(
                    "too many discards: {discarded} (limit {})",
                    cfg.max_discards
                ),
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

/// 可変長のサブ strategy のリストから [`one_of`](testrs_core::strategy::one_of)
/// strategy を構築します。それぞれを自動的にボックス化するため、異なる具象型を
/// 持つことができます。
///
/// 2つの形式がサポートされています。
///
/// - 一様: `prop_oneof![strat_a, strat_b, strat_c]`
/// - 重み付き: `prop_oneof![1 => strat_a, 4 => strat_b]`
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

/// `inner` を `leaf` の上に固定回数積み重ねることで、再帰的データの
/// strategy を構築します。木、AST、JSON 風の値に便利です。
///
/// ```ignore
/// use testrs_pbt::{prop_recursive, prop_oneof};
/// use testrs_pbt::strategy::{any, just, vec_of, StrategyExt};
///
/// #[derive(testrs_pbt::Arbitrary, Debug, Clone)]
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
/// `leaf` と `inner` の結果は同じ値の型を生成する必要があります。`inner`
/// closure は、1つ深いレベルを表す `BoxedStrategy<T>` を受け取り、現在の
/// レベル用の `BoxedStrategy<T>` を返します。
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

/// `Strategy::filter` の便利マクロで、フィルタがすべての候補を拒否した
/// 場合に panic メッセージへ含まれるラベルヒントを指定できます。
///
/// ```ignore
/// let s = prop_filter!("only even", any::<i32>(), |n| n % 2 == 0);
/// ```
///
/// 注意: ラベルは現在のところ情報提供のみで、候補が拒否されると標準の
/// `Filter: predicate rejected every candidate` メッセージが出力されます。
/// 将来のバージョンではラベルを表示する可能性があります。
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

/// 名前付きのサブ strategy と本体式から構築された複合 [`Strategy`] を
/// 返す関数を定義します。
///
/// ```ignore
/// use testrs_pbt::strategy::str;
/// testrs_pbt::prop_compose! {
///     pub fn valid_user()(
///         name in str::ascii_alphanumeric(1..20),
///         age in 18u8..100,
///     ) -> User {
///         User { name, age }
///     }
/// }
/// ```
///
/// 各バインディング `name in expr` は `expr` を [`Strategy`]（または `Strategy`
/// を実装する任意の値。`0..10` のような範囲は `IntRange` strategy の
/// ブランケット impl 経由で動作します）として使用します。本体はターゲットの値の
/// 型を返さなければなりません。
///
/// shrink は `prop_compose!` をまたいで保持されません。生成された strategy は
/// 各フィールドの shrink を独立に各サブ strategy から再導出して shrink します。
/// 複雑な領域でより細かい制御が必要な場合は、手動で `Strategy` 実装を
/// 書いてください。
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

/// 内部用: `new_value` がボックス化された closure で、`shrink_value` が
/// 何も返さない strategy です（複合 strategy は入力変数を保持しないと
/// 自然な shrink 経路を持ちません）。[`prop_compose!`] で使用されます。
#[doc(hidden)]
pub struct __ComposedStrategy<T> {
    pub builder: ComposedBuilder<T>,
}

#[doc(hidden)]
pub type ComposedBuilder<T> =
    std::sync::Arc<dyn Fn(&mut dyn testrs_core::Rng, usize) -> T + Send + Sync>;

impl<T: Clone + std::fmt::Debug + 'static> testrs_core::Strategy for __ComposedStrategy<T> {
    type Value = T;
    fn new_value<R: testrs_core::Rng + ?Sized>(&self, rng: &mut R, size: usize) -> T {
        // BoxedStrategy と同じ sized ラッパーのトリックを使用します。
        // `&mut dyn Rng` に強制変換できるよう rng をラップします。
        struct W<'a, R: testrs_core::Rng + ?Sized> {
            inner: &'a mut R,
        }
        impl<R: testrs_core::Rng + ?Sized> testrs_core::Rng for W<'_, R> {
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
    use testrs_core::strategy::{int_range, just};

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
