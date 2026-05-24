//! Tier-A以降に追加された機能を含む、propcheckの完全なワークフローを実演します。
//! 具体的には、`#[arbitrary(strategy = ...)]`フィールド属性、`flat_map`による
//! 依存生成、`prop_recursive!`による再帰データ、`prop_assert_close!`、
//! そしてその他のツール群が連携して動作することを示します。
//!
//! 実行方法: `cargo run --example derive_demo -p propcheck`

use propcheck::strategy::{any, int_range, just, str, vec_of, StrategyExt};
use propcheck::{
    classify, prop_assert, prop_assert_close, prop_assert_eq, prop_assert_matches, prop_assume,
    prop_oneof, prop_recursive, run, run_strategy_with, Arbitrary, Config,
};

#[derive(Arbitrary, Debug, Clone, PartialEq)]
struct User {
    // フィールド属性によりジェネレータの値域を制約するため、
    // テストが無意味な入力に試行を浪費することはありません。
    #[arbitrary(strategy = "str::ascii_alphanumeric(1..16)")]
    name: String,
    #[arbitrary(strategy = int_range(18u8..100))]
    age: u8,
    #[arbitrary(strategy = vec_of(int_range(-50i32..50), 0..5))]
    favorite_numbers: Vec<i32>,
}

#[derive(Arbitrary, Debug, Clone, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Num(i32),
    Array(Vec<Json>),
}

fn main() {
    // 1. フィールド属性がジェネレータを値域内に保つため、discard は不要で、
    //    classifications は実際の分布を反映します。
    run(
        "User invariants hold under derived generator",
        |u: &User| {
            classify!(u.favorite_numbers.is_empty(), "no-favorites");
            classify!(u.favorite_numbers.len() >= 3, "many-favorites");
            prop_assert!(!u.name.is_empty());
            prop_assert!((18..100).contains(&u.age));
            true
        },
    );

    // 2. prop_assert_matches!はResult/Option/enumのアサーションを読みやすくします。
    run("name parses as identifier-like", |u: &User| {
        let first = u.name.chars().next();
        prop_assert_matches!(first, Some(c) if c.is_ascii_alphanumeric());
        true
    });

    // 3. prop_recursive!による再帰データ。
    let json_strategy = prop_recursive! {
        leaf = prop_oneof![
            just(Json::Null),
            any::<bool>().map(Json::Bool),
            int_range(-100i32..100).map(Json::Num),
        ],
        inner = |child| prop_oneof![
            just(Json::Null),
            int_range(-100i32..100).map(Json::Num),
            vec_of(child, 0..4).map(Json::Array),
        ],
        max_depth = 3,
    };
    run_strategy_with(
        "json values can be cloned",
        json_strategy,
        Config {
            cases: 200,
            ..Config::default()
        },
        |j: &Json| {
            prop_assert_eq!(j.clone(), j.clone());
            true
        },
    );

    // 4. flat_mapによる依存生成: まず長さを選び、その長さちょうどのVecを生成します。
    let dependent = int_range(1usize..6).flat_map(|len| vec_of(any::<i32>(), len..len + 1));
    run_strategy_with(
        "length-dependent Vec has expected size",
        dependent,
        Config::default(),
        |v: &Vec<i32>| {
            prop_assert!((1..=5).contains(&v.len()));
            true
        },
    );

    // 5. prop_assert_close!による浮動小数点の近似等価判定。
    //    sin(2x) ≈ 2 * sin(x) * cos(x)。
    run("trig double-angle identity", |&x: &f64| {
        prop_assume!(x.is_finite() && x.abs() < 1e6);
        prop_assert_close!((2.0 * x).sin(), 2.0 * x.sin() * x.cos(), epsilon = 1e-9);
        true
    });

    // 6. 失敗時の出力と、フィールド属性のstrategyを介したフィールド単位の
    //    shrinkを実演するため、意図的に偽となるプロパティです。
    //    `age`は0ではなく18（int_rangeの下限）に向かってshrinkされます。
    run("user.age is never above 50", |u: &User| {
        prop_assert!(u.age <= 50, "expected age <= 50, got {}", u.age);
        true
    });
}
