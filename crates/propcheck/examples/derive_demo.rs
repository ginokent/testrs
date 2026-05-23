//! Demonstrates `#[derive(Arbitrary)]`, `#[propcheck]`, and the assertion
//! macros all together. The intentionally buggy property fails; the runner
//! shrinks the struct fields independently and prints both sides of the
//! mismatching `prop_assert_eq!`.
//!
//! Run with: `cargo run --example derive_demo -p propcheck`

use propcheck::{prop_assert, prop_assert_eq, prop_assume, run, Arbitrary};

#[derive(Arbitrary, Debug, Clone, PartialEq)]
struct User {
    name: String,
    age: u8,
    favorite_numbers: Vec<i32>,
}

fn main() {
    // Passing property: clone is identity.
    run("User clone round-trip", |u: &User| {
        prop_assert_eq!(u.clone(), u.clone());
        true
    });

    // Passing with discards: only test users with ages we consider "adult".
    run("adults can vote", |u: &User| {
        prop_assume!(u.age >= 18);
        prop_assert!(u.age >= 18);
        true
    });

    // Intentionally false property to demonstrate failure output + shrinking.
    // u8 max is 255, so "age <= 200" fails for many generated cases. The
    // shrinker should reduce `age` to 201 while emptying the other fields.
    run("user.age is never above 200", |u: &User| {
        prop_assert!(u.age <= 200, "expected age <= 200, got {}", u.age);
        true
    });
}
