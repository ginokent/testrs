# propcheck

Property-based testing and fuzzing for Rust, **with zero external
dependencies** — only the standard library and the compiler-provided
`proc_macro` crate.

The workspace is split into four crates:

| Crate              | Purpose                                                              |
|--------------------|----------------------------------------------------------------------|
| `propcheck-core`   | `Rng`, `XorShift64`, `Arbitrary` trait, `strategy::*` combinators    |
| `propcheck-derive` | `#[derive(Arbitrary)]` and `#[propcheck]` proc-macros                |
| `propcheck`        | Test runner, assertion macros, regression shrinking                  |
| `propcheck-fuzz`   | In-process mutation fuzzer (`fuzz` + `fuzz_typed`)                   |

Most users only need `propcheck`. It re-exports everything from
`propcheck-core` and `propcheck-derive`.

```toml
[dev-dependencies]
propcheck = { path = "crates/propcheck" }
propcheck-fuzz = { path = "crates/propcheck-fuzz" }   # optional, only for fuzzing
```

## Quick start

```rust
use propcheck::{propcheck, prop_assert_eq};

#[propcheck]
fn addition_is_commutative(a: i32, b: i32) {
    prop_assert_eq!(a.wrapping_add(b), b.wrapping_add(a));
}

// Override defaults with attribute args:
#[propcheck(cases = 10_000, seed = 42, max_size = 200)]
fn stress_test(v: Vec<u32>) {
    prop_assert_eq!(v.len(), v.iter().count());
}
```

`cargo test` runs the property 100 times by default. On failure, the runner
shrinks the inputs to a minimal counterexample, prints both sides of the
assertion, and emits a `PROPCHECK_SEED` so the run can be reproduced. The
same seed is appended to `target/propcheck-regressions/<test>.txt` so it
will be replayed automatically at the start of the next run.

## What's in the box

| Feature                                | Where it lives                                |
|----------------------------------------|-----------------------------------------------|
| `#[derive(Arbitrary)]` (struct & enum) | `propcheck::Arbitrary` (macro namespace)      |
| `#[propcheck]` attribute               | `propcheck::propcheck`                        |
| `#[propcheck(cases = N, seed = N, ..)]`| Same, with `key = literal` args               |
| `#[propcheck] async fn ...`            | Uses built-in `block_on` — no runtime needed  |
| `prop_assert!{,_eq,_ne,_matches}!`     | `propcheck::prop_assert*!`                    |
| `prop_assume!` / `prop_skip!`          | Discard (bad input) vs skip (bad env)         |
| `prop_with_context!`                   | Scoped context strings in failure messages    |
| `classify!`                            | Per-case label distribution report            |
| `IntoPropResult` (bool/`()`/`Result`)  | `?` operator + `Result<(), E>` in properties  |
| `prop_oneof!` / `prop_compose!`        | Strategy combinator macros                    |
| Strategy combinators                   | `propcheck::strategy::*`                      |
| String generators                      | `propcheck::strategy::str::*` (ascii, hex, …) |
| State-machine testing                  | `propcheck::state_machine::run_state_machine` |
| Differential testing                   | `propcheck::{differential, differential_with}`|
| Greedy / Exhaustive shrink             | `Config::shrink_mode`                         |
| Regression auto-replay                 | On by default; toggle `Config::regression_replay` |
| Outcome accessors                      | `.is_passed()`, `.failure_message()`, `.shrunk()`, … |
| Mutation byte fuzzer                   | `propcheck_fuzz::fuzz`                        |
| Typed fuzzer (`Arbitrary`-driven)      | `propcheck_fuzz::fuzz_typed`                  |
| Fuzz dictionary                        | `FuzzConfig::dictionary`                      |
| Continue after crash + dedup           | `FuzzConfig::{continue_after_crash, dedup_by_message}` |
| Corpus / crash persistence             | `FuzzConfig::{corpus_dir, crash_dir}`         |

## Patterns

### 1. Round-trip a serializer

```rust
use propcheck::{propcheck, prop_assert_eq, Arbitrary};

#[derive(Arbitrary, Debug, Clone, PartialEq)]
struct Config {
    name: String,
    port: u16,
    flags: Vec<bool>,
}

fn to_bytes(c: &Config) -> Vec<u8> { /* ... */ }
fn from_bytes(b: &[u8]) -> Result<Config, Error> { /* ... */ }

#[propcheck]
fn config_round_trips(c: Config) {
    let bytes = to_bytes(&c);
    let back = from_bytes(&bytes).expect("our own serializer should parse");
    prop_assert_eq!(c, back);
}
```

### 2. Test a sort against its specification

```rust
use propcheck::{propcheck, prop_assert};

#[propcheck]
fn sort_is_sorted(mut v: Vec<i32>) {
    let original_len = v.len();
    v.sort();
    prop_assert!(v.len() == original_len);
    prop_assert!(v.windows(2).all(|w| w[0] <= w[1]));
}

#[propcheck]
fn sort_is_idempotent(v: Vec<i32>) {
    let mut once = v.clone();
    once.sort();
    let mut twice = once.clone();
    twice.sort();
    propcheck::prop_assert_eq!(once, twice);
}
```

### 3. Properties with preconditions

```rust
use propcheck::{propcheck, prop_assume, prop_assert};

#[propcheck]
fn binary_search_finds_existing(v: Vec<u32>, idx: usize) {
    let mut sorted = v.clone();
    sorted.sort();
    prop_assume!(!sorted.is_empty());
    let i = idx % sorted.len();
    let target = sorted[i];
    prop_assert!(sorted.binary_search(&target).is_ok());
}
```

`prop_assume!` discards the case and generates a fresh one. Excessive
discards (more than `Config::max_discards`, default 10× the case count) abort
the run with a clear message — so a noisy `prop_assume!` is hard to miss.

### 4. Constrain generated values with a `Strategy`

```rust
use propcheck::{run_strategy, prop_assert};
use propcheck::strategy::{int_range, vec_of, StrategyExt};

#[test]
fn percentage_stays_in_range() {
    let strategy = vec_of(int_range(0i32..101), 1..50);
    run_strategy("percentages add to <= 100*n", strategy, |v: &Vec<i32>| {
        let n = v.len() as i32;
        prop_assert!(v.iter().sum::<i32>() <= 100 * n);
        true
    });
}
```

Available combinators in `propcheck::strategy`:

- `any::<T>()` — defer to `T::Arbitrary`
- `just(v)` — constant
- `int_range(lo..hi)` — integer in `[lo, hi)`, shrinks toward 0 or `lo`
- `vec_of(elem, len_range)` — variable-length `Vec<T>` respecting `min_len`
- `one_of(vec![...])` — uniform choice; `weighted_one_of` for biased
- `tuple(a, b)` — product
- `.map(f)` / `.filter(pred)` / `.boxed()` on any strategy
- `prop_oneof![a, b]` or `prop_oneof![1 => a, 4 => b]` — convenience macro

### 5. Fuzz a byte-oriented target

```rust
use propcheck_fuzz::{fuzz, FuzzConfig};

#[test]
fn parser_does_not_panic() {
    let report = fuzz(FuzzConfig::default(), |bytes: &[u8]| {
        let _ = my_parser::parse(bytes);
    });
    assert!(report.failure.is_none(), "found crash: {:?}", report.failure);
}
```

### 6. Fuzz a typed API

```rust
use propcheck_fuzz::{fuzz_typed, TypedFuzzConfig};

#[test]
fn json_query_never_panics() {
    let report = fuzz_typed::<String, _>(TypedFuzzConfig::default(), |s: &String| {
        let _ = json::query(s);
    });
    assert!(report.failure().is_none());
}
```

Any type with an `Arbitrary` impl can drive `fuzz_typed`. The mutator
operates on the byte seed, so it explores a wide variety of inputs without
needing a hand-written `&[u8] → T` decoder.

### 7. Diagnose generator distribution with `classify!`

```rust
use propcheck::{run, classify};

run("sort handles every input", |v: &Vec<i32>| {
    classify!(v.is_empty(), "empty");
    classify!(v.len() > 100, "large");
    classify!(v.windows(2).any(|w| w[0] == w[1]), "has-duplicates");
    let mut s = v.clone();
    s.sort();
    s.windows(2).all(|w| w[0] <= w[1])
});
// Output ends with:
//   classifications:
//      40.0%  empty            (40/100)
//      20.0%  has-duplicates   (20/100)
//      10.0%  large            (10/100)
```

If "large" is 0% you know your test isn't actually exercising the long-input
path and need to bump `max_size` or use a custom strategy.

### 8. Collect every distinct crash from a fuzz run

```rust
use propcheck_fuzz::{fuzz, FuzzConfig};
use std::path::PathBuf;

let report = fuzz(
    FuzzConfig {
        iterations: 100_000,
        dictionary: vec![b"GET ".to_vec(), b"POST ".to_vec(), b"HTTP/1.1".to_vec()],
        continue_after_crash: true,
        dedup_by_message: true,
        corpus_dir: Some(PathBuf::from("target/fuzz-corpus")),
        crash_dir: Some(PathBuf::from("target/fuzz-crashes")),
        ..FuzzConfig::default()
    },
    |bytes: &[u8]| { let _ = http::parse(bytes); },
);
for f in &report.failures {
    eprintln!("crash: {} (input: {} bytes)", f.message, f.input.len());
}
```

`dictionary` lifts the fuzzer past multi-byte gates a dumb mutator would
take eons to discover. `continue_after_crash` + `dedup_by_message` collect
every unique panic. `crash_dir` saves a `.bin` reproducer and matching
`.txt` metadata for each one.

### 9. State-machine / model-based testing

```rust
use propcheck::state_machine::{run_state_machine, StateMachine};
use propcheck::{Arbitrary, Config};

#[derive(Arbitrary, Debug, Clone)]
enum Op {
    Push(u8),
    Pop,
    Clear,
}

struct VecModel;
impl StateMachine for VecModel {
    type State = (Vec<u8>, Vec<u8>); // (sut, reference)
    type Operation = Op;
    fn initial_state() -> Self::State { (Vec::new(), Vec::new()) }
    fn execute(s: &mut Self::State, op: &Op) {
        match op {
            Op::Push(n) => { s.0.push(*n); s.1.push(*n); }
            Op::Pop     => { s.0.pop();    s.1.pop(); }
            Op::Clear   => { s.0.clear();  s.1.clear(); }
        }
    }
    fn invariant(s: &Self::State) -> Result<(), String> {
        if s.0 == s.1 { Ok(()) } else { Err(format!("{:?} != {:?}", s.0, s.1)) }
    }
}

#[test]
fn vec_matches_reference() {
    run_state_machine::<VecModel>("vec model", Config::default());
}
```

The runner generates sequences of operations, applies them in order, and
checks the invariant after each step. Failing sequences are shrunk by
greedy operation-removal until no more ops can be deleted while keeping
the invariant violated.

### 10. Async property tests

```rust
use propcheck::{propcheck, prop_assert_eq};

#[propcheck]
async fn http_parse_round_trips(req: Request) -> Result<(), Error> {
    let bytes = req.encode().await;
    let back = Request::decode(&bytes).await?;
    prop_assert_eq!(back, req);
    Ok(())
}
```

The attribute macro detects `async fn` and drives the body with a built-in
single-threaded executor (`propcheck::block_on`). No tokio / async-std
dependency is introduced. Real I/O is not supported by the built-in
executor; for tokio code, write a non-async wrapper that calls
`tokio::runtime::Runtime::new()?.block_on(...)` instead.

### 11. Differential testing

```rust
propcheck::differential(
    "fast_sort matches slow_sort",
    |v: &Vec<i32>| slow_sort(v),
    |v: &Vec<i32>| fast_sort(v),
);
```

On disagreement, both outputs and the shrunk input are reported.

## Reproducing a failure

Failures print a seed:

```
[propcheck] my_test FAILED at case #4 (PROPCHECK_SEED=12345, 0 discarded)
  reason:   prop_assert_eq! failed at src/lib.rs:42
            left:  42
            right: 43
  original: ...
  shrunk:   ...
```

Three ways to reproduce:

1. **Automatic** — failing seeds are appended to
   `target/propcheck-regressions/<test>.txt` and replayed first on the next
   run. No manual step required; just run `cargo test` again.
2. **Env var** — `PROPCHECK_SEED=12345 cargo test my_test`.
3. **Config override** — `#[propcheck(seed = 12345)]` on the test function,
   or `Config { seed: 12345, ..Config::default() }` for `run_with`.

The fuzz crate uses `PROPCHECK_FUZZ_SEED` in the same way.

## Tuning a run

`run` and `forall` use `Config::default()`. To customize, use `run_with`:

```rust
use propcheck::{run_with, Config};

run_with(
    "stress test",
    Config {
        cases: 10_000,
        max_shrinks: 4_096,
        max_size: 500,
        ..Config::default()
    },
    |v: &Vec<i32>| /* property */,
);
```

## Limitations

- `#[derive(Arbitrary)]` supports structs (named, tuple, unit) and generic
  structs whose type parameters all need `Arbitrary`. Enums and structs with
  a custom `where` clause must implement `Arbitrary` by hand.
- The fuzzer has no coverage feedback. It's a "smoke fuzzer" for catching
  panics on random and mutated inputs, not a replacement for libFuzzer or
  cargo-fuzz when those are available.
- The runner installs a process-global panic hook (refcounted for safety).
  Property tests in different threads share that hook installation; mixing
  with code that takes/sets the panic hook concurrently can race.
- No async-fn support.

## Running the test suite

```
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run --example sort_props   -p propcheck
cargo run --example derive_demo  -p propcheck   # demonstrates a failing property
cargo run --release --example find_crash -p propcheck-fuzz
```
