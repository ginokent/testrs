//! モデルベース / 状態機械のプロパティテストです。
//!
//! コンテナ、parser ー、キャッシュ、簡単な仮想マシンなどの、状態を持つ API
//! のテストに役立ちます。以下を定義します。
//!
//! 1. テスト対象の状態（多くの場合、タプル `(system_under_test, reference_model)`）。
//! 2. インターリーブする操作のセット。
//! 3. 1つの操作を状態に適用する `execute` 関数。
//! 4. すべての操作の後に成立しなければならない `invariant`。
//!
//! ランナーはランダムな操作シーケンスを生成して順番に適用し、invariant に
//! 違反する最初のシーケンスを報告します。shrink は失敗シーケンスから操作を
//! 取り除き、それでも invariant に違反する最短のものを保持します。
//!
//! # 例
//!
//! ```
//! use testrs_pbt::state_machine::{run_state_machine, StateMachine};
//! use testrs_pbt::Config;
//!
//! #[derive(testrs_pbt::Arbitrary, Debug, Clone)]
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
//!         // 自明に成立します。実際のチェックに変更してください。
//!         let _ = state.len();
//!         Ok(())
//!     }
//! }
//!
//! run_state_machine::<VecModel>("vec model", Config::default());
//! ```

use std::fmt::Debug;
use std::panic::{self, AssertUnwindSafe};

use testrs_core::{Arbitrary, XorShift64};

use crate::panic_hook::SilentPanicHook;
use crate::regression;
use crate::{Classifications, Config, Outcome};

/// この trait を実装して、テスト対象システムの状態、操作、invariant を
/// 記述します。
pub trait StateMachine {
    /// 状態の型。通常は (system_under_test, model) のタプルです。
    type State: Clone + Debug;
    /// ランナーが状態に適用できる1ステップです。
    type Operation: Arbitrary;

    /// 生成された各実行の初期状態を返します。
    fn initial_state() -> Self::State;

    /// `op` を `state` に適用します。
    fn execute(state: &mut Self::State, op: &Self::Operation);

    /// 事後状態が一貫していれば `Ok(())` を返します。返されたエラー
    /// 文字列は失敗レポートに含まれます。
    fn invariant(state: &Self::State) -> Result<(), String>;
}

/// [`StateMachine`] をランダムに生成された一連の操作シーケンスに対して
/// 実行し、最初の invariant 違反で panic します。
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
                "[testrs-pbt] {name}: ok ({cases} sequences{extra}, seed={seed})"
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
            "[testrs-pbt] {name} FAILED at sequence #{attempt} (TESTRS_PBT_SEED={seed}, {discarded} discarded, {skipped} skipped)\n  \
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
            "[testrs-pbt] {name} ABORTED (seed={seed})\n  reason: {reason}\n  sequences ran: {cases}"
        ),
    }
}

fn run_sm_loop<M: StateMachine + 'static>(
    cfg: &Config,
    regression_seeds: &[u64],
) -> Outcome<Vec<M::Operation>> {
    // 最初に regression 用 seed をリプレイします。
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
    R: testrs_core::Rng + ?Sized,
{
    let len = rng.gen_range_usize(1, max_len.max(2));
    (0..len)
        .map(|_| <M::Operation as Arbitrary>::arbitrary(rng, max_len))
        .collect()
}

/// シーケンスを実行し、最初に失敗したステップで invariant 違反メッセージ
/// （または任意の panic メッセージ）を Err で返します。
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

/// 貪欲な最小化: 1度に1つの操作を取り除いてみて、短くなったシーケンスが
/// 依然として invariant に違反する場合はその削除を維持します。
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
