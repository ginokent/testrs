# Initial design — propcheck workspace

**Status: shipped in commit `793f067` (M1) and incrementally extended
in `27c663d` (M2) and `ef9f8b1` (M3).**

## Goal

A dev-dependency property-based testing + fuzzing library for Rust,
with zero external dependencies, that can be used as a drop-in
alternative to `proptest` + `cargo-fuzz` for the 80% case.

## Constraints

- **No external crates.** Only the standard library and the
  rustc-provided `proc_macro` crate. `unsafe_code = forbid` at the
  workspace level.
- **Workspace, multi-crate.** A monolithic crate is harder to evolve.

## Crate layout

```
crates/
├── propcheck-core/   # Rng + Arbitrary trait + Strategy combinators
├── propcheck-derive/ # proc-macro: #[derive(Arbitrary)] + #[propcheck]
├── propcheck/        # runner, assertion macros, regression replay
└── propcheck-fuzz/   # in-process mutation byte fuzzer + typed fuzz
```

The umbrella crate `propcheck` re-exports everything from `-core` and
`-derive`. Users typically only depend on `propcheck` (plus
`propcheck-fuzz` if they want fuzzing).

## Core abstractions

- `Rng`: minimal PRNG trait with `next_u64` (only) as the required
  method. Default impls for `gen_range_u64`, `gen_range_i64`,
  `gen_range_usize`, `gen_bool`, `fill_bytes`, `choose`.
- `XorShift64`: the default PRNG. Marsaglia/Vigna xorshift64* — fast,
  deterministic per seed, period 2^64−1.
- `Arbitrary`: `arbitrary(&mut R, size: usize) -> Self` plus a
  `shrink(&self) -> Box<dyn Iterator<Item=Self> + '_>` default.

## Shrinking strategy

- Per-type shrinks: ints toward zero (halve-then-walk-back),
  collections by length then per-element, strings via char vec.
- The runner shrink loop accepts the first candidate that still fails
  (Greedy). An Exhaustive mode added later in M5.

## Initial Arbitrary impls

`bool`, all integer widths, `f32`, `f64`, `char`, `String`, `Vec<T>`,
`Option<T>`, `Result<T, E>`, tuples 0/1/2/3/4.

## Runner

- `forall` / `run` for `Arbitrary`-driven properties.
- Default 100 cases, configurable via `Config`.
- `PROPCHECK_SEED` env var for reproduction.
- Panic capture via `std::panic::catch_unwind`.

## Fuzzer

- `fuzz(cfg, |&[u8]| ...)` — runs mutated byte buffers through the
  target. Panic is the crash signal. Initial corpus optional.
- Mutations: bit-flip, byte-replace, interesting-byte, insert, delete,
  splice (from corpus), swap.
- Minimization: greedy single-byte deletion + zero-replacement passes.
- `PROPCHECK_FUZZ_SEED` for reproduction.

## Examples

- `sort_props` — sort idempotence / preservation / non-decreasing.
- `fail_demo` — deliberate failure showing shrinking output.
- `find_crash` — fuzzes a planted-bug parser.

## Not yet

The initial release intentionally omitted:

- Custom strategies / combinators (added in M2).
- `#[derive(Arbitrary)]` (added in M3).
- Assertion macros, `prop_assume!`, classify, async, state machines —
  all added in later milestones.
