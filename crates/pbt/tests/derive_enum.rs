//! enum および`where`句つきの構造体に対する`#[derive(Arbitrary)]`のテストです。

use testrs_pbt::{forall_with, run, Arbitrary, Config, XorShift64};

#[derive(Arbitrary, Debug, Clone, PartialEq)]
enum SimpleEnum {
    Alpha,
    Beta,
    Gamma,
}

#[derive(Arbitrary, Debug, Clone, PartialEq)]
enum WithFields {
    Empty,
    Single(u32),
    Pair(u8, String),
    Named { x: i32, y: i32 },
}

#[derive(Arbitrary, Debug, Clone, PartialEq)]
enum Generic<T> {
    Nothing,
    Just(T),
    List(Vec<T>),
}

fn no_replay_cfg(seed: u64) -> Config {
    Config {
        seed,
        cases: 200,
        silence_panic_hook: false,
        regression_replay: false,
        ..Config::default()
    }
}

#[test]
fn simple_enum_generates_all_variants() {
    let mut rng = XorShift64::seed_from_u64(1);
    let mut seen_alpha = false;
    let mut seen_beta = false;
    let mut seen_gamma = false;
    for _ in 0..200 {
        match SimpleEnum::arbitrary(&mut rng, 10) {
            SimpleEnum::Alpha => seen_alpha = true,
            SimpleEnum::Beta => seen_beta = true,
            SimpleEnum::Gamma => seen_gamma = true,
        }
    }
    assert!(seen_alpha && seen_beta && seen_gamma);
}

#[test]
fn enum_with_fields_arbitrary_works() {
    let mut rng = XorShift64::seed_from_u64(2);
    let mut seen_empty = false;
    let mut seen_single = false;
    let mut seen_pair = false;
    let mut seen_named = false;
    for _ in 0..400 {
        match WithFields::arbitrary(&mut rng, 10) {
            WithFields::Empty => seen_empty = true,
            WithFields::Single(_) => seen_single = true,
            WithFields::Pair(_, _) => seen_pair = true,
            WithFields::Named { .. } => seen_named = true,
        }
    }
    assert!(seen_empty);
    assert!(seen_single);
    assert!(seen_pair);
    assert!(seen_named);
}

#[test]
fn enum_shrinks_to_simplest_variant_when_property_universally_fails() {
    // 常に偽となるプロパティ。すべての variant が失敗するため、shrinker は
    // unit 以外の variant を最もシンプルなもの（Empty）に潰し、それを受け入れる
    // ことができます。
    let outcome = forall_with(no_replay_cfg(3), |_: &WithFields| false);
    assert!(outcome.is_failed());
    assert_eq!(*outcome.shrunk().unwrap(), WithFields::Empty);
}

#[test]
fn enum_shrinks_within_variant_when_collapse_doesnt_help() {
    // プロパティは Pairvariant の場合*のみ*失敗します。Empty への潰しは
    // プロパティを成功させてしまうため拒否され、shrinker は Pair 内に留まり、
    // フィールドをデフォルト値に向けて縮小します。
    let outcome = forall_with(no_replay_cfg(3), |v: &WithFields| {
        !matches!(v, WithFields::Pair(_, _))
    });
    assert!(outcome.is_failed());
    assert_eq!(
        *outcome.shrunk().unwrap(),
        WithFields::Pair(0, String::new())
    );
}

#[test]
fn enum_shrinks_fields_within_variant_when_no_unit_collapse_applies() {
    // プロパティ: 任意の WithFields::Pair は最初の u8が200以下である。
    // 最もシンプルな variant（Empty）では matches!()のガードによってスキップされ
    // プロパティが成功するため、shrink は Pair(201, "")に収束するはずです。
    // したがって shrinker は Pair 内に留まり、フィールドを縮小します。
    //
    // このテストは、Pair 上で*のみ*発火しフィールドの不変条件をチェックする
    // プロパティを用いて検証します。
    let outcome = forall_with(no_replay_cfg(4), |v: &WithFields| match v {
        WithFields::Pair(n, _) => *n <= 200,
        _ => true,
    });
    assert!(outcome.is_failed());
    match outcome.shrunk().unwrap() {
        WithFields::Pair(n, s) => {
            assert_eq!(*n, 201);
            assert_eq!(s, "");
        }
        other => panic!("expected shrunk to be Pair, got {other:?}"),
    }
}

#[test]
fn generic_enum_works_with_concrete_type() {
    let mut rng = XorShift64::seed_from_u64(5);
    let mut seen_nothing = false;
    let mut seen_just = false;
    let mut seen_list = false;
    for _ in 0..200 {
        match Generic::<u32>::arbitrary(&mut rng, 10) {
            Generic::Nothing => seen_nothing = true,
            Generic::Just(_) => seen_just = true,
            Generic::List(_) => seen_list = true,
        }
    }
    assert!(seen_nothing && seen_just && seen_list);
}

#[test]
fn generic_enum_drives_run() {
    run("Generic round-trip", |g: &Generic<u8>| match g {
        Generic::Nothing => true,
        Generic::Just(_) => true,
        Generic::List(v) => v.len() == v.len(),
    });
}

// --- where 句 ----------------------------------------------------

#[derive(Arbitrary, Debug, Clone)]
struct WhereOwned<T>
where
    T: Send + 'static,
{
    inner: T,
    list: Vec<T>,
}

#[test]
fn struct_with_where_clause_compiles_and_generates() {
    let mut rng = XorShift64::seed_from_u64(6);
    let v: WhereOwned<u32> = Arbitrary::arbitrary(&mut rng, 10);
    let _ = v.inner;
    let _ = v.list;
}

#[derive(Arbitrary, Debug, Clone)]
enum WhereEnum<T>
where
    T: std::fmt::Debug + Send + 'static,
{
    Empty,
    Value(T),
}

#[test]
fn enum_with_where_clause_works() {
    let mut rng = XorShift64::seed_from_u64(7);
    for _ in 0..50 {
        let _: WhereEnum<u32> = Arbitrary::arbitrary(&mut rng, 10);
    }
}

// --- 属性マクロ + derive enum の組み合わせ -----------------------

#[testrs_pbt::pbt]
fn enum_attribute_round_trip(e: SimpleEnum) {
    testrs_pbt::prop_assert_eq!(e.clone(), e);
}

#[testrs_pbt::pbt(cases = 50)]
fn with_fields_attribute_works(e: WithFields) {
    testrs_pbt::prop_assert_eq!(e.clone(), e);
}
