# propcheck

Rust 向けのプロパティベーステスト + fuzzing ライブラリ。
**外部依存ゼロ** — std とコンパイラ提供の `proc_macro` クレートのみ。

大方針は [`SPEC.md`](SPEC.md) を参照してください。計画中の項目・
明示的な非ゴール・完了履歴は [`issues/`](issues/) 配下で管理します。
主要な設計判断と gap 分析は [`.claude/plans/`](.claude/plans/) 配下にあります。

ワークスペースは 4 クレートに分かれています:

| クレート           | 目的                                                                  |
|--------------------|-----------------------------------------------------------------------|
| `propcheck-core`   | `Rng`, `XorShift64`, `Arbitrary` trait, `strategy::*` combinator    |
| `propcheck-derive` | `#[derive(Arbitrary)]` と `#[propcheck]` proc-macro                   |
| `propcheck`        | テストランナー、assertion マクロ、regression shrinking              |
| `propcheck-fuzz`   | in-process mutation・fuzzer (`fuzz` + `fuzz_typed`)         |

通常は `propcheck` だけで足ります。`propcheck-core` と
`propcheck-derive` の内容をすべて再エクスポートしています。

```toml
[dev-dependencies]
propcheck = { path = "crates/propcheck" }
propcheck-fuzz = { path = "crates/propcheck-fuzz" }   # オプション、fuzzing 用
```

## 配布方針

本ライブラリは **crates.io には公開しない**。git dependency として
参照される運用を前提とする。したがって:

- インストールは `git = "..."` 形式の dependency 指定で行う。
- 内部依存も crates.io 公開要件に縛られない。必要であれば依存先が
  git のみで配布されているクレートでも構わない (no-deps 方針自体は
  「実利用クレートに不要な依存を入れない」目的なので維持する)。
- `readme` / `keywords` / `categories` 等、crates.io ページ向けの
  メタデータ整備、および公開順序の運用ドキュメント等は対応しない。

```toml
[dev-dependencies]
propcheck = { git = "https://github.com/ginokent/testrs", package = "propcheck" }
```

## クイックスタート

```rust
use propcheck::{propcheck, prop_assert_eq};

#[propcheck]
fn addition_is_commutative(a: i32, b: i32) {
    prop_assert_eq!(a.wrapping_add(b), b.wrapping_add(a));
}

// 属性引数でデフォルトを上書き:
#[propcheck(cases = 10_000, seed = 42, max_size = 200)]
fn stress_test(v: Vec<u32>) {
    prop_assert_eq!(v.len(), v.iter().count());
}
```

`cargo test` でプロパティをデフォルト 100 回実行します。失敗時は
ランナーが入力を最小反例まで shrink し、assertion の両辺を表示し、
再現用の `PROPCHECK_SEED` を出力します。同じ seed は
`target/propcheck-regressions/<test>.txt` に追記され、次回ラン冒頭で
自動再生されます。

## 同梱機能

| 機能                                       | 場所                                                |
|--------------------------------------------|-----------------------------------------------------|
| `#[derive(Arbitrary)]` (struct & enum)     | `propcheck::Arbitrary` (マクロ名前空間)             |
| `#[arbitrary(strategy = ...)]`             | derive のフィールド単位 strategy 上書き             |
| `#[propcheck]` 属性                        | `propcheck::propcheck`                              |
| `#[propcheck(cases = N, seed = N, ..)]`    | 同上、`key = literal` 引数付き                      |
| `#[propcheck] async fn ...`                | 組み込み `block_on` を使う — ランタイム不要         |
| `prop_assert!{,_eq,_ne,_matches,_close}!`  | `propcheck::prop_assert*!`                          |
| `prop_assume!` / `prop_skip!`              | 不適切な入力 / 不適切な環境を切り分け               |
| `prop_with_context!`                       | 失敗メッセージ内のスコープ付きコンテキスト文字列    |
| `classify!`                                | ケースごとのラベル分布レポート                      |
| `IntoPropResult` (bool/`()`/`Result`)      | プロパティ内で `?` 演算子 + `Result<(), E>` 戻り    |
| `prop_oneof!` / `prop_compose!`            | Strategy combinator マクロ                         |
| `prop_recursive!` / `prop_filter!`         | 再帰木 / フィルタ付き strategy                      |
| `Strategy::flat_map`                       | 依存生成                                            |
| Strategy combinator                      | `propcheck::strategy::*` (`.sample(n)` / `.no_shrink()` 含む) |
| 文字列 generator                         | `propcheck::strategy::str::*` (ascii, hex, …)       |
| ドメイン strategy パック                 | `propcheck::strategy::domain::*` (`email_like` / `url_like` / `uuid_like` / `ipv4_dotted` / `iso8601_date`) |
| `char_range` / `bytes` / `f64_range`       | `propcheck::strategy::{char_range, bytes, f32_range, f64_range}` |
| 状態機械テスト                             | `propcheck::state_machine::run_state_machine`       |
| Differential テスト                        | `propcheck::{differential, differential_with}`      |
| Greedy / Exhaustive shrink                 | `Config::shrink_mode`                               |
| Regression 自動再生                        | デフォルト ON。`Config::regression_replay` で切替   |
| Outcome accessor                           | `.is_passed()`, `.failure_message()`, `.shrunk()`, … |
| mutation・バイト fuzzer           | `propcheck_fuzz::fuzz`                              |
| 型付き fuzzer (`Arbitrary` 駆動)          | `propcheck_fuzz::fuzz_typed`                        |
| Fuzz dictionary                            | `FuzzConfig::dictionary`                            |
| Crash 後継続 + 重複排除                    | `FuzzConfig::{continue_after_crash, dedup_by_message}` |
| corpus / crash 永続化                    | `FuzzConfig::{corpus_dir, crash_dir}`               |

## パターン集

### 1. serializer のラウンドトリップ

```rust
use propcheck::{propcheck, prop_assert_eq, Arbitrary};

#[derive(Arbitrary, Debug, Clone, PartialEq)]
struct Config {
    name: String,
    port: u16,
    flags: Vec<bool>,
}

fn to_bytes(c: &Config) -> Vec<u8> { /* ... */ }
fn from_bytes(b: &[u8]) -> Result<Config, Error> { /* ... */ }

#[propcheck]
fn config_round_trips(c: Config) {
    let bytes = to_bytes(&c);
    let back = from_bytes(&bytes).expect("自前の serializer はパースできるはず");
    prop_assert_eq!(c, back);
}
```

### 2. ソートを仕様に対してテスト

```rust
use propcheck::{propcheck, prop_assert};

#[propcheck]
fn sort_is_sorted(mut v: Vec<i32>) {
    let original_len = v.len();
    v.sort();
    prop_assert!(v.len() == original_len);
    prop_assert!(v.windows(2).all(|w| w[0] <= w[1]));
}

#[propcheck]
fn sort_is_idempotent(v: Vec<i32>) {
    let mut once = v.clone();
    once.sort();
    let mut twice = once.clone();
    twice.sort();
    propcheck::prop_assert_eq!(once, twice);
}
```

### 3. 前提条件付きプロパティ

```rust
use propcheck::{propcheck, prop_assume, prop_assert};

#[propcheck]
fn binary_search_finds_existing(v: Vec<u32>, idx: usize) {
    let mut sorted = v.clone();
    sorted.sort();
    prop_assume!(!sorted.is_empty());
    let i = idx % sorted.len();
    let target = sorted[i];
    prop_assert!(sorted.binary_search(&target).is_ok());
}
```

`prop_assume!` はそのケースを破棄して新しいケースを生成します。
破棄が多すぎる場合 (`Config::max_discards`、デフォルトはケース数の
10 倍) は明確なメッセージとともにランを中断します — ノイジーな
`prop_assume!` に気付きやすくなっています。

### 4. `Strategy` で生成値を制約する

```rust
use propcheck::{run_strategy, prop_assert};
use propcheck::strategy::{int_range, vec_of, StrategyExt};

#[test]
fn percentage_stays_in_range() {
    let strategy = vec_of(int_range(0i32..101), 1..50);
    run_strategy("percentages add to <= 100*n", strategy, |v: &Vec<i32>| {
        let n = v.len() as i32;
        prop_assert!(v.iter().sum::<i32>() <= 100 * n);
        true
    });
}
```

`propcheck::strategy` で使える主な combinator:

- `any::<T>()` — `T::Arbitrary` に委譲
- `just(v)` — 定数
- `int_range(lo..hi)` — `[lo, hi)` の整数。0 か `lo` に向けて shrink
- `vec_of(elem, len_range)` — 可変長 `Vec<T>`。`min_len` を尊重
- `one_of(vec![...])` — 一様選択。`weighted_one_of` でバイアス可
- `tuple(a, b)` — 直積
- 任意の strategy に `.map(f)` / `.filter(pred)` / `.boxed()`
- `.no_shrink()` — shrink を無効化するラッパー (高コストな strategy や、shrink で意味が崩れる構造化値の救済)
- `.sample(n) -> Vec<Value>` — 固定 seed + size でテストを起動せず `n` 個生成 (デバッグ確認用)
- `prop_oneof![a, b]` や `prop_oneof![1 => a, 4 => b]` — 便利マクロ

### 5. バイト指向ターゲットをファズ

```rust
use propcheck_fuzz::{fuzz, FuzzConfig};

#[test]
fn parser_does_not_panic() {
    let report = fuzz(FuzzConfig::default(), |bytes: &[u8]| {
        let _ = my_parser::parse(bytes);
    });
    assert!(report.failure.is_none(), "found crash: {:?}", report.failure);
}
```

### 6. 型付き API をファズ

```rust
use propcheck_fuzz::{fuzz_typed, TypedFuzzConfig};

#[test]
fn json_query_never_panics() {
    let report = fuzz_typed::<String, _>(TypedFuzzConfig::default(), |s: &String| {
        let _ = json::query(s);
    });
    assert!(report.failure().is_none());
}
```

`Arbitrary` を実装した任意の型を `fuzz_typed` で駆動できます。
mutator はバイト seed 上で動くため、手書きの `&[u8] → T`
decoder 無しで多様な入力を探索します。

### 7. `classify!` で generator の分布を診断

```rust
use propcheck::{run, classify};

run("sort handles every input", |v: &Vec<i32>| {
    classify!(v.is_empty(), "empty");
    classify!(v.len() > 100, "large");
    classify!(v.windows(2).any(|w| w[0] == w[1]), "has-duplicates");
    let mut s = v.clone();
    s.sort();
    s.windows(2).all(|w| w[0] <= w[1])
});
// 出力末尾:
//   classifications:
//      40.0%  empty            (40/100)
//      20.0%  has-duplicates   (20/100)
//      10.0%  large            (10/100)
```

"large" が 0% なら長い入力経路を実際には行使できていない、と分かる
ので `max_size` を上げるかカスタム strategy を使う必要があります。

### 8. ファズランから全 distinct crash を回収

```rust
use propcheck_fuzz::{fuzz, FuzzConfig};
use std::path::PathBuf;

let report = fuzz(
    FuzzConfig {
        iterations: 100_000,
        dictionary: vec![b"GET ".to_vec(), b"POST ".to_vec(), b"HTTP/1.1".to_vec()],
        continue_after_crash: true,
        dedup_by_message: true,
        corpus_dir: Some(PathBuf::from("target/fuzz-corpus")),
        crash_dir: Some(PathBuf::from("target/fuzz-crashes")),
        ..FuzzConfig::default()
    },
    |bytes: &[u8]| { let _ = http::parse(bytes); },
);
for f in &report.failures {
    eprintln!("crash: {} (input: {} bytes)", f.message, f.input.len());
}
```

`dictionary` は素朴な mutator では到達に永遠を要する複数バイトの
ゲートを越えさせます。`continue_after_crash` + `dedup_by_message` で
ユニークな panic をすべて収集。`crash_dir` には `.bin` 再現ファイルと
対応する `.txt` メタデータが保存されます。

### 9. 状態機械 / モデルベーステスト

```rust
use propcheck::state_machine::{run_state_machine, StateMachine};
use propcheck::{Arbitrary, Config};

#[derive(Arbitrary, Debug, Clone)]
enum Op {
    Push(u8),
    Pop,
    Clear,
}

struct VecModel;
impl StateMachine for VecModel {
    type State = (Vec<u8>, Vec<u8>); // (sut, reference)
    type Operation = Op;
    fn initial_state() -> Self::State { (Vec::new(), Vec::new()) }
    fn execute(s: &mut Self::State, op: &Op) {
        match op {
            Op::Push(n) => { s.0.push(*n); s.1.push(*n); }
            Op::Pop     => { s.0.pop();    s.1.pop(); }
            Op::Clear   => { s.0.clear();  s.1.clear(); }
        }
    }
    fn invariant(s: &Self::State) -> Result<(), String> {
        if s.0 == s.1 { Ok(()) } else { Err(format!("{:?} != {:?}", s.0, s.1)) }
    }
}

#[test]
fn vec_matches_reference() {
    run_state_machine::<VecModel>("vec model", Config::default());
}
```

ランナーは操作列を生成して順に適用し、各ステップ後に不変条件を確認
します。失敗列は、不変条件違反を保ったまま削除できる操作が無くなる
まで greedy な操作削除で shrink されます。

### 10. Async プロパティテスト

```rust
use propcheck::{propcheck, prop_assert_eq};

#[propcheck]
async fn http_parse_round_trips(req: Request) -> Result<(), Error> {
    let bytes = req.encode().await;
    let back = Request::decode(&bytes).await?;
    prop_assert_eq!(back, req);
    Ok(())
}
```

属性マクロは `async fn` を検出し、本体を組み込みのシングルスレッド
executor (`propcheck::block_on`) で駆動します。tokio や async-std
への依存は導入されません。組み込み executor は実 I/O はサポート
しません — tokio コードを使うなら、`tokio::runtime::Runtime::new()?.block_on(...)`
を呼ぶ非 async なラッパを書いてください。

### 11. Differential テスト

```rust
propcheck::differential(
    "fast_sort matches slow_sort",
    |v: &Vec<i32>| slow_sort(v),
    |v: &Vec<i32>| fast_sort(v),
);
```

不一致時には両出力と shrink された入力が報告されます。

### 12. `#[derive(Arbitrary)]` をフィールド単位で制約

```rust
use propcheck::{Arbitrary, propcheck, prop_assert};
use propcheck::strategy::{int_range, str, vec_of};

#[derive(Arbitrary, Debug, Clone)]
struct Request {
    #[arbitrary(strategy = "str::ascii_alphanumeric(1..20)")]
    user_id: String,
    #[arbitrary(strategy = int_range(1024u16..65535))]
    port: u16,
    #[arbitrary(strategy = vec_of(int_range(0u8..200), 0..1024))]
    payload: Vec<u8>,
}

#[propcheck]
fn request_is_valid(r: Request) {
    prop_assert!(!r.user_id.is_empty());
    prop_assert!(r.port >= 1024);
    prop_assert!(r.payload.iter().all(|b| *b < 200));
}
```

文字列リテラル形式 `"expr"` (proptest スタイル) と裸の式形式の
両方をサポート。strategy 式は `#[derive]` を書いた場所から見えて
いる必要があります (典型的にはファイル冒頭に
`use propcheck::strategy::*;`)。フィールド単位の shrink も strategy
を通ります — 上の例では `port` フィールドは `1024` まで縮みますが
それを下回ることはありません。

### 13. `flat_map` による依存生成

```rust
use propcheck::strategy::{any, int_range, vec_of, StrategyExt};
// まず長さを決め、その後にちょうどその長さの Vec を生成:
let s = int_range(1usize..10).flat_map(|len| vec_of(any::<i32>(), len..len + 1));
```

### 14. `prop_recursive!` による再帰データ

```rust
use propcheck::{prop_oneof, prop_recursive};
use propcheck::strategy::{any, just, vec_of, StrategyExt};

#[derive(propcheck::Arbitrary, Debug, Clone)]
enum Json { Null, Bool(bool), Num(i32), Array(Vec<Json>) }

let json = prop_recursive! {
    leaf = prop_oneof![
        just(Json::Null),
        any::<bool>().map(Json::Bool),
        any::<i32>().map(Json::Num),
    ],
    inner = |child| prop_oneof![
        just(Json::Null),
        any::<i32>().map(Json::Num),
        vec_of(child, 0..4).map(Json::Array),
    ],
    max_depth = 3,
};
```

### 15. 浮動小数点の近似比較

```rust
use propcheck::{propcheck, prop_assert_close, prop_assume};
#[propcheck]
fn double_angle_identity(x: f64) {
    prop_assume!(x.is_finite() && x.abs() < 1e6);
    prop_assert_close!((2.0 * x).sin(), 2.0 * x.sin() * x.cos(), epsilon = 1e-9);
}
```

### 16. ドメイン strategy で parser をファズ

```rust
use propcheck::run_strategy;
use propcheck::strategy::domain;

#[test]
fn url_parser_handles_arbitrary_urls() {
    // デフォルトの url_like は http/https、port なし、50% で path 付与。
    run_strategy("url parser does not panic", domain::url_like(), |u: &String| {
        let _ = my_url::parse(u);
        true
    });
}

#[test]
fn websocket_url_parser_with_port() {
    // builder で scheme / port をカスタマイズ。
    let strategy = domain::url_like()
        .with_schemes(&["ws", "wss"])
        .with_port_range(8000..9000);
    run_strategy("websocket urls", strategy, |u: &String| {
        let _ = my_url::parse(u);
        true
    });
}

#[test]
fn uuid_parser_does_not_panic() {
    // uuid_like は version/variant bit を強制しないため、厳格な v4 parser
    // からは reject されえる。ここでは「panic しない」ことのみ保証する。
    run_strategy("uuid parser robustness", domain::uuid_like(), |s: &String| {
        let _ = my_uuid::parse(s);
        true
    });
}
```

`domain::*` の各 strategy は `*_like` 命名で「仕様準拠ではない近似」を
示します。RFC 厳格準拠の検証ではなく、parser のクラッシュ耐性や
ラウンドトリップを叩くための「形が似た値」を提供します。
他の選択肢: `email_like` / `ipv4_dotted` / `iso8601_date`。

### 17. `Strategy::sample(n)` でデバッグ確認

```rust
use propcheck::strategy::{domain, int_range, vec_of, StrategyExt};

// テストを書かずに strategy の生成例を覗き見る。固定 seed のため
// 何度呼んでも同じ列が返り、Diff 検証にも使える。
let examples: Vec<String> = domain::email_like().sample(3);
dbg!(&examples);
// → ["abc@example.com", ...]

// 自作 strategy の動作確認:
let s = vec_of(int_range(0..100), 0..5);
assert_eq!(s.sample(3).len(), 3);
```

`sample(n)` はランナーの size スケジュールは再現せず、視覚的な動作
確認用です。網羅的な検証は `run_strategy` / `forall_strategy` を
使ってください。

### 18. `Strategy::no_shrink()` で shrink を抑制

```rust
use propcheck::run_strategy;
use propcheck::strategy::{any, vec_of, StrategyExt};

// 16 byte 固定長の鍵を生成。shrink すると鍵長が崩れて別 panic に化けるため
// no_shrink で停止し、最初の反例をそのまま観察する。
let key_strategy = vec_of(any::<u8>(), 16..17).no_shrink();
run_strategy("aes round trip", key_strategy, |k: &Vec<u8>| {
    let cipher = aes::encrypt(k, b"plaintext");
    aes::decrypt(k, &cipher) == b"plaintext"
});
```

ユースケース:

- shrink 自体が高コスト (暗号鍵生成など、再 generate に時間がかかる)
- shrink で意味が崩れる (固定長 / 構造化 token の一部を削ると別種の panic に化ける)
- shrink 経路のバグ調査 (shrink を切って「最初の反例」を見たい)

`new_value` は内部 strategy にそのまま委譲されるため、値生成の分布は
変わりません。

## 失敗の再現

失敗時は seed が出力されます:

```
[propcheck] my_test FAILED at case #4 (PROPCHECK_SEED=12345, 0 discarded)
  reason:   prop_assert_eq! failed at src/lib.rs:42
            left:  42
            right: 43
  original: ...
  shrunk:   ...
```

再現方法は 3 通り:

1. **自動** — 失敗 seed は
   `target/propcheck-regressions/<test>.txt` に追記され、次回ラン
   冒頭で再生されます。手動操作は不要 — `cargo test` を再実行する
   だけ。
2. **環境変数** — `PROPCHECK_SEED=12345 cargo test my_test`。
3. **Config 上書き** — テスト関数に `#[propcheck(seed = 12345)]`、
   または `run_with` 用に `Config { seed: 12345, ..Config::default() }`。

fuzz クレートは同じ仕組みで `PROPCHECK_FUZZ_SEED` を使います。

## ラン設定の調整

`run` と `forall` は `Config::default()` を使います。カスタマイズには
`run_with` を使います:

```rust
use propcheck::{run_with, Config};

run_with(
    "stress test",
    Config {
        cases: 10_000,
        max_shrinks: 4_096,
        max_size: 500,
        ..Config::default()
    },
    |v: &Vec<i32>| /* property */,
);
```

## 制約

- `#[derive(Arbitrary)]` は struct (named, tuple, unit) と、すべての
  型パラメータが `Arbitrary` を必要とするジェネリック struct を
  サポートします。enum および独自の `where` 句を持つ struct も
  扱えますが、複雑なケースでは手書きで `Arbitrary` を実装する方が
  分かりやすい場合があります。
- fuzzer にはカバレッジフィードバックがありません。ランダム / 変異
  入力で panic を捕まえる「スモーク fuzzer」であり、libFuzzer や
  cargo-fuzz が使える場面ではそれらの代替にはなりません。
- ランナーはプロセスグローバルな panic hook をインストールします
  (安全のため参照カウント済み)。別スレッドで動くプロパティテストは
  この install を共有するため、panic hook を別途取得 / 設定する
  コードと並行すると競合し得ます。
- `panic = "abort"` プロファイルは非対応です。
- Regression replay は `target/` 配下に書き込みます。書き込み先は
  `CARGO_TARGET_DIR` を優先し、未設定なら `CARGO_MANIFEST_DIR` を
  起点とします。両方とも未設定の場合 (例: cargo の外でリリース
  バイナリを直接実行する場合) は永続化が黙ってスキップされます。
- `#[derive(Arbitrary)]` のフィールド属性 `#[arbitrary(strategy = ...)]`
  は、裸の式形式 (`#[arbitrary(strategy = some::path::Thing::new())]`) と
  文字列形式 (`#[arbitrary(strategy = "some::path::Thing::new()")]`) を
  区別せず解釈します。両方動作しますが、文字列形式は Rust の文字列
  エスケープ規則に従う必要があります。
- `prop_recursive! { leaf = …, inner = …, max_depth = N }` の `inner`
  closure は技術的には深さに対して指数的に肥大化する strategy を組めます。
  `max_depth` はネスト深さのみを制約し、幅は制約しません。

## テストスイートの実行

```
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run --example sort_props   -p propcheck
cargo run --example derive_demo  -p propcheck   # 失敗するプロパティをデモ
cargo run --release --example find_crash -p propcheck-fuzz
```

## Development

ローカル開発では [mise](https://mise.jdx.dev/) のタスクランナーで CI 相当
の check を並列実行する運用。

### Setup (初回のみ)

```bash
# mise 本体: https://mise.jdx.dev/getting-started.html に従って install
mise install        # mise.toml の [tools] (cargo-deny 等) を一括 install

# (任意) ci-publish-check-dangerously を使うなら gh CLI も:
#   https://cli.github.com/
#   gh auth login
```

### 日常使用

```bash
mise run ci         # 開発 loop 用: dirty check + fmt / clippy / doc / test / deny を並列実行

# 個別:
mise run fmt
mise run clippy
mise run doc
mise run test
mise run deny
```

`mise run ci` は **modified / staged の差分があれば fail** (untracked は許容)。
commit してから回す前提。各 task は `scripts/*.sh` を呼ぶ単純な bash wrapper
で、mise を入れていない環境では `./scripts/<name>.sh` を直接実行しても同じ
結果を得られる。

### local / CI 対応表

| local             | 実コマンド                                                              | CI job          |
|-------------------|-------------------------------------------------------------------------|-----------------|
| `mise run fmt`    | `cargo fmt --all -- --check`                                            | `fmt`           |
| `mise run clippy` | `cargo clippy --workspace --all-targets -- -D warnings`                 | `clippy`        |
| `mise run doc`    | `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps`              | `doc`           |
| `mise run test`   | `cargo test --workspace --all-targets` + `cargo test --workspace --doc` | `test-linux` 等 |
| `mise run deny`   | `cargo deny check all`                                                  | `deny`          |
| `mise run ci`     | preflight + 上記 5 つを並列実行 (check run なし)                        | (なし)          |
| `mise run ci-publish-check-dangerously` | (⚠️ 危険) ci 相当 + 結果を **CI と同名** check run として PR HEAD に投影。[CONTRIBUTING.md](CONTRIBUTING.md) を読んでから使うこと | (なし) |

CI は GitHub Actions のコスト最適化のため PR コメント (`!run ci` / `!run ci
matrix`) で trigger する設計。multi-OS (ubuntu / macos / windows) を確認
したいときは PR 上で `!run ci matrix` を投稿する。詳細は
[`.github/workflows/ci.yml`](.github/workflows/ci.yml) 冒頭参照。MSRV (1.82)
検証はローカル task からは外しており、CI 側の `msrv` job に任せる方針。

### Contributing

設計判断のトレードオフ、特に `ci-publish-check-dangerously` の危険性に
ついては [`CONTRIBUTING.md`](CONTRIBUTING.md) を参照。

