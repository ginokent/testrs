//! `#[derive(Arbitrary)]`および`#[pbt]`のインテグレーションテストです。

use testrs_pbt::{run, Arbitrary};

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
    use testrs_pbt::XorShift64;
    let mut rng = XorShift64::seed_from_u64(1);
    let _: UnitStruct = Arbitrary::arbitrary(&mut rng, 10);
    assert!(UnitStruct.shrink().next().is_none());
}

#[test]
fn derive_named_struct_round_trips() {
    use testrs_pbt::XorShift64;
    let mut rng = XorShift64::seed_from_u64(2);
    for _ in 0..50 {
        let v: NamedStruct = Arbitrary::arbitrary(&mut rng, 20);
        let _ = v.shrink().take(5).count();
    }
}

#[test]
fn derive_tuple_struct_round_trips() {
    use testrs_pbt::XorShift64;
    let mut rng = XorShift64::seed_from_u64(3);
    for _ in 0..50 {
        let v: TupleStruct = Arbitrary::arbitrary(&mut rng, 20);
        let _ = v.shrink().take(5).count();
    }
}

#[test]
fn derive_generic_struct_works_with_arbitrary_inner() {
    use testrs_pbt::XorShift64;
    let mut rng = XorShift64::seed_from_u64(4);
    let v: Generic<u32> = Arbitrary::arbitrary(&mut rng, 10);
    // ランナーを介して利用し、全体のパイプラインを動作確認します。
    let _: () = v.shrink().next().map(|_| ()).unwrap_or(());
}

#[test]
fn derive_works_inside_property_test() {
    run("named struct round-trip", |s: &NamedStruct| {
        // 自明に真のプロパティ。derive がランナーで利用できる値を生成することを
        // 確認するだけのテストです。
        s.a == s.a && s.b == s.b && s.c == s.c
    });
}

#[test]
fn derive_failure_shrinks_through_fields() {
    use testrs_pbt::{forall_with, Config, Outcome};
    let cfg = Config {
        cases: 100,
        seed: 999,
        max_shrinks: 256,
        max_size: 30,
        max_discards: 1000,
        max_skips: 1000,
        silence_panic_hook: false,
        regression_replay: false,
        shrink_mode: testrs_pbt::ShrinkMode::Greedy,
    };
    // プロパティ: NamedStruct.a は決して200を超えない。
    let outcome = forall_with(cfg, |s: &NamedStruct| s.a <= 200);
    match outcome {
        Outcome::Failed { shrunk, .. } => {
            assert!(shrunk.a > 200, "expected shrunk.a > 200, got {}", shrunk.a);
            // shrinking は無関係なフィールドをデフォルト値に向けて縮小するはずです。
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

// --- #[pbt]属性 --------------------------------------------

#[testrs_pbt::pbt]
fn attribute_macro_zero_args() {
    testrs_pbt::prop_assert_eq!(1 + 1, 2);
}

#[testrs_pbt::pbt]
fn attribute_macro_single_arg(n: u8) {
    testrs_pbt::prop_assert_eq!(n.wrapping_add(0), n);
}

#[testrs_pbt::pbt]
fn attribute_macro_two_args(a: i32, b: i32) {
    testrs_pbt::prop_assert_eq!(a.wrapping_add(b), b.wrapping_add(a));
}

#[testrs_pbt::pbt]
fn attribute_macro_with_derive(s: NamedStruct) {
    // プロパティが自明になるケースをスキップするために prop_assume!を使います。
    testrs_pbt::prop_assume!(!s.c.is_empty());
    testrs_pbt::prop_assert_eq!(s.c.len(), s.c.iter().count());
}

#[testrs_pbt::pbt(cases = 50, max_size = 20)]
fn attribute_macro_with_args(n: u8) {
    // ランナーはちょうど50ケースを実行するはずです（ここから直接観測することは
    // できませんが、このオーバーライドでテストがコンパイル・成功することで、
    // parser がそれを受け付けることが確認できます）。
    testrs_pbt::prop_assert_eq!(n.wrapping_add(0), n);
}

#[testrs_pbt::pbt(seed = 12345)]
fn attribute_macro_with_seed(n: u32) {
    // 固定 seed -> 決定的な実行。ここで seed を直接観測するのは難しいですが、
    // この関数は seed オーバーライドつきでコンパイル・成功する必要があります。
    testrs_pbt::prop_assert!(n == n);
}

// `?`演算子を使った Result 戻り値型。
#[testrs_pbt::pbt]
fn attribute_macro_with_result(s: String) -> Result<(), std::num::ParseIntError> {
    testrs_pbt::prop_assume!(!s.is_empty());
    if s.chars().all(|c| c.is_ascii_digit()) {
        let _n: u128 = s.parse()?;
    }
    Ok(())
}
