# Gap analysis #3 — Tier S/A/B for agent test-authoring

**Status: shipped in commit `48eba2e` (M5).**

## Motivation

After M4, the library had a solid backbone. The third gap analysis
focused specifically on what an *agent* (LLM) needs to write good
tests with the library. The remaining gaps were grouped by tier:

## Items implemented

### Tier S — biggest agent blockers

1. **`#[derive(Arbitrary)]` for enums** (unit / tuple / named-field
   variants, with generics). Without this, ~50% of real Rust crates
   are unreachable via the derive.
2. **String structured strategies** (`str::ascii_digits`,
   `ascii_alphanumeric`, `hex_string`, `ascii_letters_*`,
   `ascii_printable`, `from_char_set`). Without these, parser tests
   waste cases on random Unicode.
3. **`prop_assert_matches!`** with optional `if` guard. Required for
   testing parsers/state machines that return enum-typed results.

### Tier A — significant quality of life

4. **`prop_compose!`** for declarative composite generators (proptest
   syntax-compat).
5. **Tuple arities 5..=8** so wide-arity test functions work.
6. **State-machine framework** — a `StateMachine` trait + runner that
   generates random operation sequences, applies them, and reports the
   first invariant violation. Shrinks by greedy operation-removal.
7. **Async support** — built-in `block_on` (Condvar-parker, `std::pin::pin!`-
   based, zero unsafe). The `#[propcheck]` macro detects `async fn`
   and wraps the body in `block_on`.
8. **`PropSkip` payload + `prop_skip!` macro**, counted separately
   from discards so a flaky environment doesn't mask a noisy
   generator.
9. **`prop_with_context!`** with thread-local context stack appended
   to assertion messages.
10. **`Outcome` accessors** (`is_passed`, `failure_message`, `shrunk`,
    `seed`, etc.) for programmatic inspection.

### Tier B — polish

11. **where-clause support** on `#[derive(Arbitrary)]`.
12. **Float improvements** — more interesting values (`EPSILON`,
    `MIN_POSITIVE`, NaN), ULP-step + `round` in shrink.
13. **`differential` / `differential_with`** helper for comparing two
    implementations.
14. **`Config::shrink_mode` (`Greedy` / `Exhaustive`)**.

## Decisions

- **Enum shrink-to-simplest-variant.** Only fires if a unit variant
  exists. Otherwise stay within the variant and shrink fields. The
  collapse is rejected if it makes the property pass (good — we keep
  drilling into the failing variant).
- **Async without runtime deps.** A minimal `block_on` with a
  Mutex+Condvar parker is enough for pure async logic. Networked /
  timer-heavy code needs tokio anyway, and users can wrap that
  themselves.
- **Skip vs discard.** Distinct counters; both have abort thresholds.
- **Differential testing.** Trivial wrapper over `run_with` that
  calls both implementations and `prop_assert_eq!`s the outputs.

## Implementation order

1. Smaller independent items first (tuples 5..=8, more std impls,
   accessor methods).
2. `IntoPropResult` and `prop_skip!` plumbing.
3. derive macro extensions: enum support, where-clauses, attribute args.
4. State-machine framework.
5. Async (block_on + macro wrapper).
6. Differential + ShrinkMode.

## Skipped

- CLI subcommand (out of scope).
- JUnit XML (low value, easy to add later).
- Coverage-guided fuzzing (impossible without LLVM bindings).

## Verification

134 unit + integration tests pass; clippy clean. A buggy counter SM
test verifies the state machine reduces a 22-op failing sequence
down to `[Decrement]`.
