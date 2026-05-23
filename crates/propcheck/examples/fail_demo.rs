//! Demonstrates a failing property and the shrinking report. Expect this
//! program to panic with a minimized counterexample.

use propcheck::run;

fn main() {
    // Intentionally false: "the sum of two u8s never overflows the type",
    // which obviously fails on any pair whose sum exceeds 255. The shrinker
    // should converge to the smallest such pair.
    run("u8 addition never overflows", |&(a, b): &(u8, u8)| {
        a.checked_add(b).is_some()
    });
}
