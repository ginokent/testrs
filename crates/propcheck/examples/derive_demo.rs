//! Demonstrates the full propcheck workflow: `#[derive(Arbitrary)]`,
//! `#[propcheck]` with attribute args, the assertion macros, `classify!`
//! for distribution diagnostics, `Result<(), E>` returns with `?`, and
//! finally a deliberately failing property that the runner shrinks.
//!
//! Run with: `cargo run --example derive_demo -p propcheck`

use propcheck::{classify, prop_assert, prop_assert_eq, prop_assume, run, run_with, Arbitrary, Config};

#[derive(Arbitrary, Debug, Clone, PartialEq)]
struct User {
    name: String,
    age: u8,
    favorite_numbers: Vec<i32>,
}

fn main() {
    // 1. Passing property with classify! to inspect the generator's
    //    distribution. The output below the "ok" line is the percentage
    //    table.
    run("User clone round-trip", |u: &User| {
        classify!(u.name.is_empty(), "empty-name");
        classify!(u.age == 0, "age-zero");
        classify!(u.age >= 18, "adult");
        classify!(u.favorite_numbers.is_empty(), "no-favorites");
        prop_assert_eq!(u.clone(), u.clone());
        true
    });

    // 2. Property with a precondition via prop_assume!. Discarded cases
    //    are counted separately from passes.
    run("adults can vote", |u: &User| {
        prop_assume!(u.age >= 18);
        prop_assert!(u.age >= 18);
        true
    });

    // 3. Result return + the ? operator inside the property body. We
    //    take a `u64` directly so the generator hits the "valid input"
    //    space every time (no discards needed).
    run_with(
        "u64 → string → u64 round-trip",
        Config {
            cases: 200,
            ..Config::default()
        },
        |n: &u64| -> Result<(), std::num::ParseIntError> {
            let parsed: u64 = n.to_string().parse()?;
            prop_assert_eq!(parsed, *n);
            Ok(())
        },
    );

    // 4. Intentionally false property to demonstrate failure output +
    //    struct-field-level shrinking + the seed for reproduction.
    //    The age field shrinks toward the failure boundary while the
    //    name and favorites collapse toward their empty defaults.
    run("user.age is never above 200", |u: &User| {
        prop_assert!(u.age <= 200, "expected age <= 200, got {}", u.age);
        true
    });
}
