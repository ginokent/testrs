//! Demonstrates property-based testing of stdlib sort against several
//! invariants. Run with `cargo run --example sort_props -p propcheck`.

use propcheck::run;

fn main() {
    run("sort is idempotent", |v: &Vec<i32>| {
        let mut once = v.clone();
        once.sort();
        let mut twice = once.clone();
        twice.sort();
        once == twice
    });

    run("sort preserves length", |v: &Vec<i32>| {
        let mut s = v.clone();
        s.sort();
        s.len() == v.len()
    });

    run("sort produces non-decreasing output", |v: &Vec<i32>| {
        let mut s = v.clone();
        s.sort();
        s.windows(2).all(|w| w[0] <= w[1])
    });

    run("reverse twice is identity", |v: &Vec<String>| {
        let mut r = v.clone();
        r.reverse();
        r.reverse();
        r == *v
    });

    println!("All properties passed.");
}
