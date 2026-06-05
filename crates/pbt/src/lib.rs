//! プロパティベーステストのランナーです。
//!
//! `#[test]` 関数の内部から [`run`] を使う（あるいは同じ効果を boilerplate
//! を減らして得るために自由関数に `#[pbt]` を書く）ことで、ランダムに
//! 生成された多数の入力に対してプロパティを表明できます。
//!
//! ```
//! use testrs_pbt::{run, prop_assert_eq};
//!
//! run("addition is commutative", |&(a, b): &(i32, i32)| {
//!     prop_assert_eq!(a.wrapping_add(b), b.wrapping_add(a));
//!     true
//! });
//! ```
//!
//! プロパティ本体の中では以下が使えます。
//!
//! - [`prop_assert!`]、[`prop_assert_eq!`]、[`prop_assert_ne!`] は
//!   詳細な失敗メッセージ付きの assertion を提供します。
//! - [`prop_assume!`] は前提条件を満たさないケースを破棄します。
//! - [`classify!`] はケースごとのラベルを記録します。ランナーはそれらを
//!   集計し、合格／失敗のサマリーと一緒にパーセンテージ表として報告します。
//!
//! プロパティ本体は [`IntoPropResult`] を実装する任意の型を返せます。
//! 具体的には `bool`、`()`、`Result<(), E>`、または [`PropResult`] その
//! ものです。これによりプロパティ本体の中で `?` 演算子を使えます。
//!
//! 失敗時の出力には実行で使用した seed が含まれているため、`TESTRS_PBT_SEED`
//! 環境変数を設定することで決定的に再現できます。失敗した seed は
//! `target/testrs-pbt-regressions/<name>.txt` にも永続化され、以降の実行
//! 時に再生されます。

use std::any::Any;
use std::env;
use std::panic::{self, AssertUnwindSafe};

pub use testrs_core::{Arbitrary, Rng, Strategy, StrategyExt, XorShift64};

mod assert;
mod async_exec;
pub mod classify;
pub mod differential;
mod panic_hook;
mod regression;
pub mod state_machine;
pub mod strategy_runner;

pub use async_exec::block_on;
pub use differential::{differential, differential_with};

#[doc(hidden)]
pub use assert::{__current_context, __panic_payload_str, __pop_context, __push_context};
pub use assert::{PropAssertFailure, PropDiscard, PropSkip};
pub use classify::Classifications;
pub use testrs_core::strategy;
// `Arbitrary` を trait（testrs-core から、型の名前空間）および
// derive マクロ（testrs-pbt-derive から、マクロの名前空間）の両方として公開します。
pub use testrs_pbt_derive::pbt;

#[doc(hidden)]
pub use strategy_runner::__ComposedStrategy;
pub use strategy_runner::{forall_strategy, forall_strategy_with, run_strategy, run_strategy_with};
/// `#[derive(Arbitrary)]` は各フィールド型が [`Arbitrary`] を実装していることを
/// 要求します。未実装の型をフィールドに持つ場合はコンパイルエラーとなり、診断は
/// 当該フィールドの型を指し示します（`#[arbitrary(strategy = ...)]` 付きフィールド
/// は strategy が値を供給するため除外されます）。
///
/// ```compile_fail
/// use testrs_pbt::Arbitrary;
///
/// // `Arbitrary` 未実装の型。
/// struct NotArbitrary;
///
/// #[derive(Arbitrary)]
/// struct Holder {
///     value: NotArbitrary,
/// }
/// ```
pub use testrs_pbt_derive::Arbitrary;

use panic_hook::SilentPanicHook;

// ---------------------------------------------------------------------------
// IntoPropResult: プロパティが bool、()、Result、または PropResult を返せる
// ようにします。
// ---------------------------------------------------------------------------

/// ランナーが分類する前の、単一のプロパティケースの結果です。
#[derive(Debug)]
pub enum PropResult {
    /// ケースが合格しました。
    Pass,
    /// ケースが失敗しました。人間が読める理由を伴います。
    Fail(String),
    /// ケースが破棄されました（前提条件を満たしませんでした）。`cases` には
    /// カウントされません。
    Discard,
}

/// プロパティ本体が `bool`、`()`、`Result<(), E>`、または [`PropResult`] の
/// いずれかを明示的な変換なしに返せるようにする trait です。
pub trait IntoPropResult {
    /// `self` を [`PropResult`] に変換します。
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

/// プロパティベーステスト実行の調整可能なパラメータです。
#[derive(Debug, Clone)]
pub struct Config {
    /// 実行する*合格*ケースの目標数です。
    pub cases: usize,
    /// PRNG seed です。デフォルトは `TESTRS_PBT_SEED` 環境変数、または
    /// 壁時計のエントロピーです。
    pub seed: u64,
    /// 失敗ケースに対して適用される shrink ステップの最大数です。
    pub max_shrinks: usize,
    /// `Arbitrary::arbitrary` に渡される size ヒントの上限です。
    pub max_size: usize,
    /// 実行が中止される前に許容される `prop_assume!` の破棄の合計上限です。
    /// デフォルトは `cases * 10` です。
    pub max_discards: usize,
    /// 実行が中止される前に許容される `prop_skip!` のスキップの合計上限です。
    /// 不安定な環境がノイズの多い generator のように見えないよう、破棄とは
    /// 別にカウントされます。デフォルトは `cases * 10` です。
    pub max_skips: usize,
    /// `true` の場合、実行中はグローバル panic フックを無音化し、失敗ケース
    /// が端末をスパムしないようにします。内部で参照カウントされているため、
    /// 並行するランナーは単一のフックインストールを共有します。
    pub silence_panic_hook: bool,
    /// `true` の場合、[`run`] / [`run_with`] は失敗 seed を
    /// `target/testrs-pbt-regressions/<name>.txt` に永続化し、以降の実行
    /// の最初に再生します。
    pub regression_replay: bool,
    /// 縮小戦略です。デフォルトは [`ShrinkMode::Greedy`] で、各ステップで
    /// 最初に失敗した候補を受け入れます。[`ShrinkMode::Exhaustive`] は
    /// すべての候補を評価し、最も小さい（安価なヒューリスティックとして
    /// `Debug` の長さで判断）失敗候補のみを受け入れます。コストは高くなります。
    pub shrink_mode: ShrinkMode,
}

/// shrinker が候補を探索する方法です。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShrinkMode {
    /// 依然として失敗する最初の候補を受け入れます（高速で、典型的な
    /// QuickCheck の挙動です）。
    Greedy,
    /// 各ステップで、すべての候補を評価し、依然として失敗するもののうち
    /// （`Debug` 表現の長さで）最も小さいものを受け入れます。低速ですが、
    /// ネストされたデータに対してはより厳密な反例を生み出すことが多いです。
    Exhaustive,
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
            max_skips: cases * 10,
            silence_panic_hook: true,
            regression_replay: true,
            shrink_mode: ShrinkMode::Greedy,
        }
    }
}

fn env_seed() -> u64 {
    if let Ok(s) = env::var("TESTRS_PBT_SEED") {
        if let Ok(n) = s.parse::<u64>() {
            return n;
        }
    }
    XorShift64::from_entropy().state()
}

// ---------------------------------------------------------------------------
// Outcome
// ---------------------------------------------------------------------------

/// プロパティテストの結果です。
#[derive(Debug)]
pub enum Outcome<A> {
    /// すべての必要なケースが合格しました（破棄やスキップを経た場合もあります）。
    Passed {
        cases: usize,
        discarded: usize,
        skipped: usize,
        classifications: Classifications,
    },
    /// あるケースが失敗しました。
    Failed {
        original: A,
        shrunk: A,
        message: String,
        seed: u64,
        attempt: usize,
        discarded: usize,
        skipped: usize,
        classifications: Classifications,
    },
    /// 実行が完了前に中止されました（破棄またはスキップが多すぎる）。
    Aborted {
        reason: String,
        cases: usize,
        discarded: usize,
        skipped: usize,
        seed: u64,
        classifications: Classifications,
    },
}

impl<A> Outcome<A> {
    /// 失敗を見つけずに実行が完了した場合に `true` を返します。
    pub fn is_passed(&self) -> bool {
        matches!(self, Outcome::Passed { .. })
    }

    /// 実行が失敗ケースを見つけた場合に `true` を返します。
    pub fn is_failed(&self) -> bool {
        matches!(self, Outcome::Failed { .. })
    }

    /// 実行が完了前に中止された場合に `true` を返します。
    pub fn is_aborted(&self) -> bool {
        matches!(self, Outcome::Aborted { .. })
    }

    /// 失敗メッセージがあれば返します。
    pub fn failure_message(&self) -> Option<&str> {
        match self {
            Outcome::Failed { message, .. } => Some(message),
            _ => None,
        }
    }

    /// 最小化された反例があれば返します。
    pub fn shrunk(&self) -> Option<&A> {
        match self {
            Outcome::Failed { shrunk, .. } => Some(shrunk),
            _ => None,
        }
    }

    /// 元の（縮小されていない）失敗入力があれば返します。
    pub fn original(&self) -> Option<&A> {
        match self {
            Outcome::Failed { original, .. } => Some(original),
            _ => None,
        }
    }

    /// プロパティを正常に最後まで実行したケースの数です。
    /// 失敗した実行の場合、これは失敗ケースのインデックスから 1 を引いた値です。
    pub fn cases(&self) -> usize {
        match self {
            Outcome::Passed { cases, .. } => *cases,
            Outcome::Failed { attempt, .. } => attempt.saturating_sub(1),
            Outcome::Aborted { cases, .. } => *cases,
        }
    }

    /// 発生した `prop_assume!` の破棄回数です。
    pub fn discarded(&self) -> usize {
        match self {
            Outcome::Passed { discarded, .. }
            | Outcome::Failed { discarded, .. }
            | Outcome::Aborted { discarded, .. } => *discarded,
        }
    }

    /// 発生した `prop_skip!` のスキップ回数です。
    pub fn skipped(&self) -> usize {
        match self {
            Outcome::Passed { skipped, .. }
            | Outcome::Failed { skipped, .. }
            | Outcome::Aborted { skipped, .. } => *skipped,
        }
    }

    /// 実行に使用された seed です。
    pub fn seed(&self) -> Option<u64> {
        match self {
            Outcome::Failed { seed, .. } | Outcome::Aborted { seed, .. } => Some(*seed),
            Outcome::Passed { .. } => None,
        }
    }

    /// 実行中に収集された `classify!` ラベルの集計です。
    pub fn classifications(&self) -> &Classifications {
        match self {
            Outcome::Passed {
                classifications, ..
            }
            | Outcome::Failed {
                classifications, ..
            }
            | Outcome::Aborted {
                classifications, ..
            } => classifications,
        }
    }
}

// ---------------------------------------------------------------------------
// 公開エントリーポイント
// ---------------------------------------------------------------------------

/// [`Config::default`] に対して `prop` を実行し、[`Outcome`] を返します。
pub fn forall<A, F, R>(prop: F) -> Outcome<A>
where
    A: Arbitrary,
    F: FnMut(&A) -> R,
    R: IntoPropResult,
{
    forall_with(Config::default(), prop)
}

/// カスタム [`Config`] で `prop` を実行し、[`Outcome`] を返します。
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

/// [`forall()`] の宣言的なシンタックスシュガーです。変数 + 型バインドの構文で
/// プロパティを書けます。複数の変数を与えると、対応するタプル型に対する
/// [`forall()`] 呼び出しへ展開されます。
///
/// ```ignore
/// // 単一変数: forall(|n: &u8| ...) へ展開。
/// let outcome = forall! { |n: u8| {
///     prop_assert_eq!(n.wrapping_add(0), *n);
///     true
/// }};
///
/// // 複数変数: forall(|(a, b): &(i32, i32)| ...) へ展開。
/// let outcome = forall! { |a: i32, b: i32| {
///     prop_assert_eq!(a.wrapping_add(*b), b.wrapping_add(*a));
///     true
/// }};
/// ```
///
/// クロージャ引数は match ergonomics により本体内では参照 (`&T`) として束縛
/// されます。明示的な [`Config`] が必要な場合は [`forall_with`] を直接呼んで
/// ください。
#[macro_export]
macro_rules! forall {
    (| $($name:ident : $ty:ty),+ $(,)? | $body:expr) => {
        $crate::forall(|($($name),+): &($($ty),+)| $body)
    };
}

/// [`Outcome::Failed`] を `panic!` に変換する便利なラッパーで、
/// `#[test]` 関数の中で直接使うのに適しています。
pub fn run<A, F, R>(name: &str, prop: F)
where
    A: Arbitrary,
    F: FnMut(&A) -> R,
    R: IntoPropResult,
{
    run_with(name, Config::default(), prop)
}

/// [`run`] と同じですが、明示的な [`Config`] を取ります。
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

    // 今後の実行のために新しい失敗 seed を永続化します。
    if let Outcome::Failed {
        seed: failed_seed, ..
    } = &outcome
    {
        if let Some(path) = &regression_path {
            let _ = regression::append_seed(path, *failed_seed);
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
            let class_part = if classifications.is_empty() {
                String::new()
            } else {
                format!("\n  classifications:\n{}", classifications.render())
            };
            panic!(
                "[testrs-pbt] {name} FAILED at case #{attempt} (TESTRS_PBT_SEED={seed}, {discarded} discarded, {skipped} skipped)\n  \
                 reason:   {message}\n  \
                 original: {original:?}\n  \
                 shrunk:   {shrunk:?}{class_part}",
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
            let class_part = if classifications.is_empty() {
                String::new()
            } else {
                format!("\n  classifications:\n{}", classifications.render())
            };
            panic!(
                "[testrs-pbt] {name} ABORTED (seed={seed})\n  \
                 reason: {reason}\n  \
                 cases ran: {cases}, discarded: {discarded}, skipped: {skipped}{class_part}",
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 内部ループ
// ---------------------------------------------------------------------------

fn run_loop<A, F>(cfg: &Config, prop: &mut F, regression_seeds: &[u64]) -> Outcome<A>
where
    A: Arbitrary,
    F: FnMut(&A) -> PropResult,
{
    // 1. まず回帰 seed を再生します。
    for &rseed in regression_seeds {
        let mut rng = XorShift64::seed_from_u64(rseed);
        let size = (cfg.max_size / 2).max(1);
        let val: A = A::arbitrary(&mut rng, size);
        classify::reset_current();
        if let CaseOutcome::Fail(msg) = run_prop(prop, &val) {
            let _labels = classify::take_current();
            let shrunk =
                shrink_failure_with_mode(val.clone(), prop, cfg.max_shrinks, cfg.shrink_mode);
            return Outcome::Failed {
                original: val,
                shrunk,
                message: format!("regression seed {rseed} reproduced: {msg}"),
                seed: rseed,
                attempt: 0,
                discarded: 0,
                skipped: 0,
                classifications: Classifications::default(),
            };
        }
        let _ = classify::take_current();
    }

    // 2. メインループ。
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
                    "too many discards: {discarded} (limit {max}) — your prop_assume! \
                     preconditions reject too many generated cases. Tighten the generator.",
                    max = cfg.max_discards
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
                reason: format!(
                    "too many skips: {skipped} (limit {max}) — the test environment is \
                     not providing required preconditions. Check prop_skip! call sites.",
                    max = cfg.max_skips
                ),
                cases: passed,
                discarded,
                skipped,
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
            CaseOutcome::Skip(_) => {
                skipped += 1;
            }
            CaseOutcome::Fail(message) => {
                classifications.merge_case(labels);
                let shrunk =
                    shrink_failure_with_mode(val.clone(), prop, cfg.max_shrinks, cfg.shrink_mode);
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

/// 1 つのケースに対してプロパティを実行した結果です。
#[allow(dead_code)] // `Skip` のメッセージは情報提供用で、スキップ数を通じて表面化します。
pub(crate) enum CaseOutcome {
    Pass,
    Fail(String),
    Discard,
    Skip(String),
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
    if let Some(s) = payload.downcast_ref::<PropSkip>() {
        return CaseOutcome::Skip(s.message.clone());
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

fn shrink_failure_with_mode<A, F>(initial: A, prop: &mut F, max: usize, mode: ShrinkMode) -> A
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
        match mode {
            ShrinkMode::Greedy => {
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
            ShrinkMode::Exhaustive => {
                // すべての候補を評価し（バジェットの範囲内で）、Debug の長さで
                // 最も小さい失敗候補を保持します。
                let mut best: Option<(usize, A)> = None;
                for c in candidates {
                    if attempts >= max {
                        break;
                    }
                    attempts += 1;
                    classify::reset_current();
                    let r = run_prop(prop, &c);
                    let _ = classify::take_current();
                    if matches!(r, CaseOutcome::Fail(_)) {
                        let len = format!("{c:?}").len();
                        if best.as_ref().map(|(b, _)| len < *b).unwrap_or(true) {
                            best = Some((len, c));
                        }
                    }
                }
                match best {
                    Some((_, c)) => {
                        current = c;
                        continue 'outer;
                    }
                    None => return current,
                }
            }
        }
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
            max_skips: 5000,
            silence_panic_hook: false,
            regression_replay: false,
            shrink_mode: ShrinkMode::Greedy,
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
        // `prop_assert!` がなく戻り値もない場合は Pass となるはずです。
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
            Outcome::Passed {
                cases, discarded, ..
            } => {
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
            max_skips: 5000,
            shrink_mode: ShrinkMode::Greedy,
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
