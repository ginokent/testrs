//! ステートマシンフレームワークのテストです。

use propcheck::state_machine::{run_state_machine, StateMachine};
use propcheck::{Arbitrary, Config};

#[derive(Arbitrary, Debug, Clone)]
enum VecOp {
    Push(u8),
    Pop,
    Clear,
}

/// 参照モデル: テスト対象システム（実際の Vec）と独立した参照（こちらも Vec ですが
/// 別途管理されており、どちらかのバグがあれば不一致として顕在化します）の組合せ。
struct VecAgainstReference;

impl StateMachine for VecAgainstReference {
    type State = (Vec<u8>, Vec<u8>); // (sut, ref)
    type Operation = VecOp;

    fn initial_state() -> Self::State {
        (Vec::new(), Vec::new())
    }

    fn execute(state: &mut Self::State, op: &Self::Operation) {
        let (sut, model) = state;
        match op {
            VecOp::Push(n) => {
                sut.push(*n);
                model.push(*n);
            }
            VecOp::Pop => {
                sut.pop();
                model.pop();
            }
            VecOp::Clear => {
                sut.clear();
                model.clear();
            }
        }
    }

    fn invariant(state: &Self::State) -> Result<(), String> {
        let (sut, model) = state;
        if sut == model {
            Ok(())
        } else {
            Err(format!("divergence: sut={sut:?}, model={model:?}"))
        }
    }
}

fn no_replay_cfg(seed: u64) -> Config {
    Config {
        seed,
        cases: 100,
        max_size: 30,
        max_shrinks: 200,
        silence_panic_hook: false,
        regression_replay: false,
        ..Config::default()
    }
}

#[test]
fn correct_implementation_passes_state_machine_test() {
    // std::Vec は、自明な参照（こちらも std::Vec）と一致します。
    run_state_machine::<VecAgainstReference>("vec-against-itself", no_replay_cfg(1));
}

// --- ランナーがレポートし shrink することを検証するための、意図的にバグのある SUT ---

#[derive(Arbitrary, Debug, Clone)]
enum CounterOp {
    Increment,
    Decrement,
    Reset,
}

struct BuggyCounter;

impl StateMachine for BuggyCounter {
    type State = (i32, i32); // (real, expected)
    type Operation = CounterOp;

    fn initial_state() -> Self::State {
        (0, 0)
    }

    fn execute(state: &mut Self::State, op: &Self::Operation) {
        let (real, expected) = state;
        match op {
            CounterOp::Increment => {
                *real += 1;
                *expected += 1;
            }
            CounterOp::Decrement => {
                // バグ: 実カウンタのデクリメントを忘れています。
                *expected -= 1;
            }
            CounterOp::Reset => {
                *real = 0;
                *expected = 0;
            }
        }
    }

    fn invariant(state: &Self::State) -> Result<(), String> {
        if state.0 == state.1 {
            Ok(())
        } else {
            Err(format!("real={} expected={}", state.0, state.1))
        }
    }
}

#[test]
#[should_panic(expected = "FAILED")]
fn buggy_counter_is_caught_and_shrunk() {
    // ランナーは失敗メッセージとともに panic するはずです。このテストは
    // #[should_panic]によって成功します。shrink されたシーケンスは、
    // 失敗する最小の操作である[Decrement]だけになるはずです。
    run_state_machine::<BuggyCounter>("buggy-counter", no_replay_cfg(2));
}
