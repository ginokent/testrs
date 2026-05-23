//! Tests for the state-machine framework.

use propcheck::state_machine::{run_state_machine, StateMachine};
use propcheck::{Arbitrary, Config};

#[derive(Arbitrary, Debug, Clone)]
enum VecOp {
    Push(u8),
    Pop,
    Clear,
}

/// Reference model: a system-under-test (real Vec) paired with an
/// independent reference (also a Vec, but managed separately so a bug in
/// either would surface as divergence).
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
    // std::Vec matches our trivial reference (which is also std::Vec).
    run_state_machine::<VecAgainstReference>("vec-against-itself", no_replay_cfg(1));
}

// --- A deliberately buggy SUT to verify the runner reports + shrinks ---

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
                // BUG: forgot to decrement the real counter.
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
    // The runner should panic with the failure message; the test passes
    // by virtue of #[should_panic]. The shrunk sequence should be just
    // [Decrement] (the minimal failing op).
    run_state_machine::<BuggyCounter>("buggy-counter", no_replay_cfg(2));
}
