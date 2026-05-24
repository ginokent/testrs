# Gap analysis #2 — Tier B+ additions

**Status: shipped in commit `59724cd` (M4).**

## Motivation

After M1–M3, the workspace had:

- Three crates (`-core`, `-derive`, `propcheck`).
- `#[derive(Arbitrary)]` for structs, `#[propcheck]` attribute macro.
- `prop_assert*!` family, `prop_assume!`.
- Basic strategy combinators.

The second gap analysis asked: what does an agent still hit when
writing real-world property tests?

## Gaps identified

### Tier B — must add

1. `classify!` for per-case distribution diagnostics.
2. `IntoPropResult` so properties can return `Result<(), E>` and use
   `?` directly.
3. `#[propcheck(cases = N, seed = N, …)]` attribute arguments.
4. Regression auto-replay: save failing seeds, replay first next run.
5. Thread-safe panic-hook silencer.
6. More std `Arbitrary` impls: `Rc`, `Arc`, `Cell`, `RefCell`,
   `PathBuf`, `OsString`, `Ipv*`, `SocketAddr*`, `NonZero*`.

### Tier C — pulled in opportunistically

7. Fuzz dictionary (`FuzzConfig::dictionary`).
8. Crash deduplication + continue past first crash.
9. Corpus / crash persistence to disk.

### Tier D — skipped

- CLI subcommand, JUnit XML, full regex strings, snapshot testing.

## Implementation order

1. Lightweight additions first: `Outcome` field expansion, accessor
   helpers, panic-hook RAII.
2. `IntoPropResult` (changes runner signature but additive via blanket
   `impl IntoPropResult for bool`).
3. `classify!` wired through `run_loop` (reset before each case,
   harvest after).
4. Regression replay: file IO under `target/propcheck-regressions/`
   with seed sanitisation and a bounded retention policy.
5. Attribute-arg parsing in `propcheck-derive` (key/value with integer
   literal values).
6. Fuzz upgrades: dictionary mutation, `dedup_by_message`,
   `continue_after_crash`, on-disk corpus + crash dirs.

## Decisions

- **Discard vs skip.** Two separate counters and abort thresholds —
  `prop_assume!` (input precondition not met) goes through
  `PropDiscard`; `prop_skip!` (environment not ready) is added later
  in M5 via `PropSkip` with a separate `max_skips` budget.
- **Regression file location.** `target/` rather than `$HOME` so it
  rides along with the build directory and is gitignored by default.
- **Panic-hook refcounting.** Atomic `usize` counter + `Mutex<Option<…>>`
  to hold the previous hook. Concurrent installs share one.

## Verification

86 unit + integration tests pass; clippy clean. End-to-end regression
replay test (`tests/regression_replay.rs`) writes to a temp directory
and verifies the failure is reproduced on the second invocation.
