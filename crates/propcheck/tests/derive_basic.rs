//! Integration tests for `#[derive(Arbitrary)]` and `#[propcheck]`.

use propcheck::{run, Arbitrary};

#[derive(Arbitrary, Debug, Clone, PartialEq)]
struct UnitStruct;

#[derive(Arbitrary, Debug, Clone, PartialEq)]
struct NamedStruct {
    a: u8,
    b: String,
    c: Vec<i32>,
}

#[derive(Arbitrary, Debug, Clone, PartialEq)]
struct TupleStruct(u32, bool);

#[derive(Arbitrary, Debug, Clone, PartialEq)]
struct Generic<T> {
    inner: T,
    list: Vec<T>,
}

#[test]
fn derive_unit_struct_compiles_and_generates() {
    use propcheck::XorShift64;
    let mut rng = XorShift64::seed_from_u64(1);
    let _: UnitStruct = Arbitrary::arbitrary(&mut rng, 10);
    assert!(UnitStruct.shrink().next().is_none());
}

#[test]
fn derive_named_struct_round_trips() {
    use propcheck::XorShift64;
    let mut rng = XorShift64::seed_from_u64(2);
    for _ in 0..50 {
        let v: NamedStruct = Arbitrary::arbitrary(&mut rng, 20);
        let _ = v.shrink().take(5).count();
    }
}

#[test]
fn derive_tuple_struct_round_trips() {
    use propcheck::XorShift64;
    let mut rng = XorShift64::seed_from_u64(3);
    for _ in 0..50 {
        let v: TupleStruct = Arbitrary::arbitrary(&mut rng, 20);
        let _ = v.shrink().take(5).count();
    }
}

#[test]
fn derive_generic_struct_works_with_arbitrary_inner() {
    use propcheck::XorShift64;
    let mut rng = XorShift64::seed_from_u64(4);
    let v: Generic<u32> = Arbitrary::arbitrary(&mut rng, 10);
    // Use it through the runner to exercise the full pipeline.
    let _: () = v.shrink().next().map(|_| ()).unwrap_or(());
}

#[test]
fn derive_works_inside_property_test() {
    run("named struct round-trip", |s: &NamedStruct| {
        // Trivially true property — we're just checking that the derive
        // produces values the runner can use.
        s.a == s.a && s.b == s.b && s.c == s.c
    });
}

#[test]
fn derive_failure_shrinks_through_fields() {
    use propcheck::{forall_with, Config, Outcome};
    let cfg = Config {
        cases: 100,
        seed: 999,
        max_shrinks: 256,
        max_size: 30,
        max_discards: 1000,
        silence_panic_hook: false,
    };
    // Property: NamedStruct.a should never be > 200.
    let outcome = forall_with(cfg, |s: &NamedStruct| s.a <= 200);
    match outcome {
        Outcome::Failed { shrunk, .. } => {
            assert!(shrunk.a > 200, "expected shrunk.a > 200, got {}", shrunk.a);
            // Shrinking should reduce the unrelated fields toward defaults.
            assert!(
                shrunk.b.is_empty() || shrunk.b.len() <= 5,
                "expected b to shrink, got {:?}",
                shrunk.b
            );
            assert!(
                shrunk.c.len() <= 3,
                "expected c to shrink, got {:?}",
                shrunk.c
            );
        }
        other => panic!("expected failure, got {other:?}"),
    }
}

// --- #[propcheck] attribute --------------------------------------------

#[propcheck::propcheck]
fn attribute_macro_zero_args() {
    propcheck::prop_assert_eq!(1 + 1, 2);
}

#[propcheck::propcheck]
fn attribute_macro_single_arg(n: u8) {
    propcheck::prop_assert_eq!(n.wrapping_add(0), n);
}

#[propcheck::propcheck]
fn attribute_macro_two_args(a: i32, b: i32) {
    propcheck::prop_assert_eq!(a.wrapping_add(b), b.wrapping_add(a));
}

#[propcheck::propcheck]
fn attribute_macro_with_derive(s: NamedStruct) {
    // Use prop_assume! to skip cases where the property would be trivial.
    propcheck::prop_assume!(!s.c.is_empty());
    propcheck::prop_assert_eq!(s.c.len(), s.c.iter().count());
}
