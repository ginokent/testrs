# Backlog

Status snapshot for the `propcheck` workspace. Updated 2026-05-24.

- Repository state: all changes committed and pushed on
  `claude/property-based-testing-lib-V4Q5s`.
- Test suite: **153 unit + integration tests pass**, plus doc tests.
- Lint: `cargo clippy --workspace --all-targets -- -D warnings` clean.
- Dependency policy: std + the compiler-provided `proc_macro` crate only.
- `unsafe_code = "forbid"` at the workspace level (the minimal `block_on`
  executor uses `std::pin::pin!` to avoid unsafe).

## Legend

- ✅ Done — shipped on the current branch.
- 🟡 Backlog — proposed but not implemented.
- ❌ Won't do — explicitly out of scope.
- ⚠️ Known limitation — acknowledged but not a blocker.

---

## ✅ Done

Grouped by the commit / milestone in which it landed.

### M1 — Initial workspace (commit `793f067`)

- ✅ Workspace skeleton with three crates: `propcheck-core`, `propcheck`,
  `propcheck-fuzz`.
- ✅ `Rng` trait + `XorShift64` PRNG.
- ✅ `Arbitrary` trait + impls for primitives, `Vec`, `String`, `Option`,
  `Result`, tuples (2-arity through 4-arity).
- ✅ Property runner with `forall` / `run`, automatic shrinking,
  `Outcome::{Passed, Failed}`, panic-capture as failure.
- ✅ Seed-based reproduction via `PROPCHECK_SEED`.
- ✅ Mutation byte fuzzer (`fuzz`), corpus splicing, panic-driven crash
  minimization, `PROPCHECK_FUZZ_SEED`.

### M2 — Strategy combinators & more impls (commit `27c663d`)

- ✅ `Strategy` trait + `StrategyExt` extension trait.
- ✅ Combinators: `any`, `just`, `int_range`, `vec_of`, `one_of`,
  `weighted_one_of`, `tuple`, `Map`, `Filter`, `BoxedStrategy`.
- ✅ Additional `Arbitrary` impls: `HashMap`, `HashSet`, `BTreeMap`,
  `BTreeSet`, `VecDeque`, `[T; N]`, `Box<T>`, `Range<T>`, `Duration`.
- ✅ Object-safe `Rng` (`Self: Sized` bound on generic methods).
- ✅ MSRV bumped to **1.82** (for `PanicHookInfo`).

### M3 — Derive macros, assertions, typed fuzz (commit `ef9f8b1`)

- ✅ New `propcheck-derive` crate (hand-rolled, no `syn`/`quote`).
- ✅ `#[derive(Arbitrary)]` for named-field, tuple, and unit structs
  including generics.
- ✅ `#[propcheck]` attribute macro on free functions (0/1/N args).
- ✅ `prop_assert!`, `prop_assert_eq!`, `prop_assert_ne!` macros with
  rich failure messages (file, line, both sides).
- ✅ `prop_assume!` macro + structured `PropDiscard` payload.
- ✅ `fuzz_typed<T: Arbitrary>`: byte seed → `T::arbitrary` typed fuzz.
- ✅ Convenience `Outcome::Failed` panic format with reproduction hint.

### M4 — Tier-B additions (commit `59724cd`)

- ✅ `classify!` macro + `Classifications` aggregation, rendered in
  the run summary.
- ✅ `IntoPropResult` trait: properties may return `bool`, `()`,
  `Result<(), E>`, or `PropResult` directly (`?` works in property body).
- ✅ `#[propcheck(cases = N, seed = N, max_shrinks = N, max_size = N,
  max_discards = N, max_skips = N)]` attribute arguments.
- ✅ Regression auto-replay to
  `target/propcheck-regressions/<test>.txt` (replayed at next run).
- ✅ Thread-safe `SilentPanicHook` (atomic refcount + Mutex).
- ✅ Additional `Arbitrary` impls: `Rc<T>`, `Arc<T>`, `Cell<T>`,
  `RefCell<T>`, `PathBuf`, `OsString`, `Ipv4Addr`, `Ipv6Addr`, `IpAddr`,
  `SocketAddrV4`, `SocketAddrV6`, `SocketAddr`, `NonZero{U,I}*` for all
  integer widths including `{Usize, Isize}`.
- ✅ Fuzz dictionary (`FuzzConfig::dictionary`).
- ✅ Crash dedup + multi-crash collection (`continue_after_crash`,
  `dedup_by_message`, `FuzzReport::failures: Vec<Failure>`).
- ✅ Disk persistence: `FuzzConfig::{corpus_dir, crash_dir}` with
  hash-based filenames.

### M5 — Tier S/A/B (commit `48eba2e`)

- ✅ `#[derive(Arbitrary)]` for **enums** (unit, tuple, and named-field
  variants), with shrink-to-simplest-variant collapse when possible.
- ✅ `where`-clause support on `#[derive(Arbitrary)]`.
- ✅ String structured generators in `propcheck::strategy::str`:
  `ascii_digits`, `ascii_letters_{lower,upper}`, `ascii_letters`,
  `ascii_alphanumeric`, `hex_string`, `ascii_printable`, `from_char_set`.
- ✅ `prop_assert_matches!` macro (with optional `if` guard).
- ✅ `prop_compose!` macro for declarative composite strategies.
- ✅ Tuple `Arbitrary` arities 5..=8.
- ✅ State-machine framework (`propcheck::state_machine`):
  `StateMachine` trait + `run_state_machine`, with operation-sequence
  shrinking.
- ✅ Async support: built-in `block_on` (`std::pin::pin!`-based, zero
  unsafe) + `#[propcheck] async fn` wrapper.
- ✅ Failure categorisation: `PropSkip` payload + `prop_skip!` macro,
  counted separately via `Config::max_skips`.
- ✅ `prop_with_context!` macro with thread-local context stack
  appended to assertion messages.
- ✅ `Outcome` accessor methods: `is_passed`, `is_failed`, `is_aborted`,
  `failure_message`, `shrunk`, `original`, `cases`, `discarded`,
  `skipped`, `seed`, `classifications`.
- ✅ Float gen / shrink improvements (NaN, `EPSILON`, `MIN_POSITIVE`,
  `round`, one-ULP-step).
- ✅ Differential testing helper (`differential`, `differential_with`).
- ✅ `Config::shrink_mode` with `ShrinkMode::{Greedy, Exhaustive}`.

### M6 — Field attrs + dependent / recursive generation (commit `45e9d00`)

- ✅ `#[arbitrary(strategy = ...)]` field attribute on
  `#[derive(Arbitrary)]`. Supports both the string-literal
  (`"expr"`-as-string, proptest-style) and bare-expression forms.
  Applies to named-field structs, tuple structs, and enum variants
  (both shapes).
- ✅ `Strategy::flat_map` / `FlatMap` for dependent generation.
- ✅ `prop_recursive! { leaf = …, inner = …, max_depth = N }` macro
  for trees / ASTs / JSON-like values.
- ✅ `char_range(lo..hi)` strategy.
- ✅ `bytes(len_range)` strategy (sugar over `vec_of(any::<u8>(), …)`).
- ✅ `f32_range` / `f64_range` strategies with shrink-toward-0 / lo.
- ✅ `prop_assert_close!` for approximate float equality.
- ✅ `Strategy`/`StrategyExt` re-exported at `propcheck::*` so derive's
  generated paths resolve without users importing `propcheck-core`.
- ✅ `prop_filter!` macro (sugar over `Strategy::filter`).

---

## 🟡 Backlog (proposed, not implemented)

Each item carries a rough effort estimate and a one-line agent-impact
note. Roughly ordered by value.

### Polish / nice-to-have

- 🟡 **`Strategy::sample(n)`** debug helper (~15 LOC). Lets a user (or
  agent) inspect what a strategy generates without running a test.
  Impact: medium — useful while authoring custom strategies.
- 🟡 **`prop_assert_panic!` / `prop_assert_no_panic!`** (~30 LOC).
  Asserts a closure panics / doesn't panic. Useful for testing that
  specific inputs trigger expected panics. Impact: small.
- 🟡 **`Strategy::no_shrink()`** wrapper (~20 LOC). Disables shrinking
  for one strategy; useful when shrinking is expensive or distorts the
  failing case. Impact: small.

### More `Arbitrary` impls

- 🟡 `std::collections::BinaryHeap<T>` (~20 LOC).
- 🟡 `std::collections::LinkedList<T>` (~20 LOC).
- 🟡 `std::num::Wrapping<T>`, `std::num::Saturating<T>` (~30 LOC).
- 🟡 `std::cmp::Ordering` (~15 LOC).
- 🟡 `std::cmp::Reverse<T>` (~15 LOC).
- 🟡 `std::borrow::Cow<'static, str>` / `Cow<'static, [T]>` (~30 LOC).

Impact per item: small. Together they round out stdlib coverage.

### Strategy domain pack

- 🟡 **Pre-canned domain strategies**: `email_like`, `url_like`,
  `uuid_like`, `ipv4_dotted`, `iso8601_date`. ~200 LOC total. Risk:
  the specs are fuzzy and may not match user-specific notions of
  "valid". Impact: medium for parser tests.

### CI / reporting

- 🟡 **JUnit XML / JSON output** mode (~150 LOC). For dashboards.
  Impact: low — `cargo test` output suffices for typical use.
- 🟡 **`cargo propcheck` CLI subcommand** in a new bin crate (~500+
  LOC). Provides progress bars for long fuzz runs, named-seed
  reproduction, etc. Impact: low — `cargo test` already works.

### Test scaffolding

- 🟡 **Test result accumulator** — run multiple properties and report
  every failure at the end rather than panicking on the first one.
  Large refactor; impact small for typical workflows.
- 🟡 **`forall!` syntactic-sugar macro** for inline properties (~30
  LOC). Impact: minor.

### Edge cases

- 🟡 Lifetime parameters on `#[derive(Arbitrary)]` — references aren't
  trivially `Arbitrary` (no owned value to generate). Document as
  unsupported or design a `borrowed` adapter. Impact: small.
- 🟡 Generic functions under `#[propcheck]` — currently rejected.
  Adding support means parsing generic params on the test fn and
  monomorphising at call sites. Impact: small.
- 🟡 Improved compile-error messages from the derive when a field type
  doesn't implement `Arbitrary` (currently surfaces as a generic trait
  bound error). Impact: medium for newcomers; needs proc-macro spans.

---

## ❌ Won't do (explicitly out of scope)

These have been considered and rejected, primarily for the no-deps
policy or because they belong to a different library's responsibility.

- ❌ **Full regex-based string generation** (proptest's `regex`
  feature). Would require reimplementing `regex` (thousands of LOC)
  or pulling in the crate. The Tier-S string strategies cover the
  80% case.
- ❌ **Coverage-guided fuzzing** (libFuzzer / SanitizerCoverage). Needs
  LLVM instrumentation that no-deps can't reach. Recommend
  `cargo-fuzz` for that workflow.
- ❌ **Real async runtime**. The built-in `block_on` is enough for pure
  async logic. Network/timer-heavy async code should use tokio
  directly inside a sync wrapper.
- ❌ **Snapshot testing** (insta-style). Different testing concern;
  leave to `insta`.
- ❌ **Mock / DI framework**. Out of propcheck's responsibility.
- ❌ **Time / randomness injection**. Belongs to user code or a
  dedicated faking library.

---

## ⚠️ Known limitations

- ⚠️ The runner installs a process-global panic hook (refcounted, but
  still global). Concurrent property tests in the same process share
  the install. If user code separately calls `panic::set_hook`,
  ordering matters.
- ⚠️ Regression replay writes to `target/`. If `CARGO_TARGET_DIR` and
  `CARGO_MANIFEST_DIR` are both unset (e.g. running a release binary
  outside cargo), persistence is silently skipped.
- ⚠️ `#[derive(Arbitrary)]` does not parse arbitrary attribute paths
  like `#[arbitrary(strategy = some::path::Thing::new())]` differently
  from `#[arbitrary(strategy = "some::path::Thing::new()")]`; both
  work, but the string form is parsed by Rust on macro output, so
  quoting characters must follow Rust string escape rules.
- ⚠️ `prop_recursive!`'s `inner` closure can technically build
  strategies that grow exponentially with depth; `max_depth` only
  bounds nesting, not breadth.
- ⚠️ `panic = "abort"` Cargo profiles are incompatible: the runner
  relies on `std::panic::catch_unwind`.

---

## Process notes

- Plan files for future work should go in `.claude/plans/` (currently
  contains the `README.md` and the historical analyses for this
  session).
- The "Tier S / A / B / C / D" framing used throughout the conversation
  reflects an explicit agent-effort × value trade-off, not a hard
  prioritisation. Items can move between tiers as priorities change.
