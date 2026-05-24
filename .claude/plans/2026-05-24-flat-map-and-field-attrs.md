# Gap analysis #4 — `#[arbitrary]` field attrs + dependent / recursive gen

**Status: shipped in commit `45e9d00` (M6).**

## Motivation

After M5, the library was at rough feature parity with `proptest`
plus extras (state machine, async, regression replay). The fourth gap
analysis identified five remaining items whose absence kept the
`#[derive(Arbitrary)]` workflow from being agent-friendly in practice.

## Items implemented

### Tier S — the last big blocker

1. **`#[arbitrary(strategy = ...)]` field attribute on
   `#[derive(Arbitrary)]`.** This was the single biggest remaining
   gap. Without it, derived `Arbitrary` impls generate "anything in
   the type's domain" — including invalid inputs that the property
   has to filter out via `prop_assume!`. With field attributes:

   ```rust
   #[derive(Arbitrary)]
   struct Request {
       #[arbitrary(strategy = "str::ascii_alphanumeric(1..20)")]
       user_id: String,
       #[arbitrary(strategy = int_range(1024u16..65535))]
       port: u16,
   }
   ```

   Supports both the string-literal form (proptest-style) and the
   bare-expression form. Applies to named-field structs, tuple
   structs, and to fields of enum variants (both shapes).

### Tier A — significant additions

2. **`Strategy::flat_map`** for dependent generation. "First pick a
   length, then a `Vec` of exactly that length" was previously
   impossible without a custom Strategy impl.
3. **`prop_recursive!` macro** for trees / ASTs / JSON-like values.
   Internally calls a bounded-depth `recursive` builder that stacks
   `inner` over `leaf` up to `max_depth` times.
4. **`char_range(lo..hi)`** strategy (chars in a half-open range,
   skipping surrogates).
5. **`bytes(len_range)`** strategy sugar.
6. **`f32_range` / `f64_range`** strategies with shrink toward 0.0
   when zero lies in the range, otherwise toward `lo`.
7. **`prop_assert_close!`** macro for approximate float equality.

### Other small additions while we were there

- `prop_filter!` macro (sugar over `StrategyExt::filter`).
- `propcheck::{Strategy, StrategyExt}` re-exported at the crate root
  so derive's generated paths resolve.

## Decisions

- **Field attribute parser.** Hand-rolled, no `syn`. Supports both
  `strategy = "expr"` (string literal — proptest-style) and
  `strategy = expr` (bare tokens). The string form is stripped of
  quotes and parsed by Rust at macro output time.
- **Codegen split.** Per-field generation picks between
  `Arbitrary::arbitrary` (default) and
  `{ let __strat = (EXPR); Strategy::new_value(&__strat, ...) }`
  (when an attribute is present). Shrink uses
  `Strategy::shrink_value` correspondingly. This means shrinks honor
  the strategy's lower bound — verified by a planted-failure test
  (`int_range(10..1000)` shrinks to **51**, not 0).
- **`Fields::Named` / `Fields::Unnamed` representation.** Both now
  hold `Vec<FieldInfo>` (name, ty, optional strategy) instead of the
  earlier `Vec<(String, String)>`, allowing tuple-field attrs to
  work uniformly.
- **`prop_recursive!` typing.** The `inner` closure may return any
  `Strategy<Value = T>`, not just `BoxedStrategy<T>`, so
  `prop_oneof![...]` composes cleanly without explicit `.boxed()`.
- **`proc_macro_derive` attributes(...)**. The derive macro now
  declares `attributes(arbitrary)` so rustc accepts `#[arbitrary(...)]`
  syntactically.
- **`unsafe_code = forbid` preserved.** The new code paths (FlatMap,
  recursive, CharRange, FloatRange) all stay safe-only.

## Skipped (deferred or rejected)

- Pre-canned domain strategies (`email_like`, `url_like`, etc.) —
  vague specs, low value, ~200 LOC.
- `prop_assert_panic!` / `prop_assert_no_panic!` — small impact.
- `Strategy::sample()` debug helper — deferred to the backlog.
- Coverage-guided fuzzing, regex strings, async runtime, snapshot
  testing — same rejections as previous milestones.

## Verification

153 unit + integration tests pass; clippy clean. The `derive_demo`
example exercises every new feature in a single 80-line file. The
planted-failure case shrinks a `User { age: 77, name: "e4aXBPirBVfg8",
favorite_numbers: [35, 49, 34] }` down to `User { age: 51, name: "0",
favorite_numbers: [] }`, demonstrating that field-attribute strategies
drive both generation AND shrinking.
