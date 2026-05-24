# バックログ

`propcheck` ワークスペースの状況 snapshot。更新日: 2026-05-24。

- リポジトリ状態: M1〜M6 の機能は `main` にマージ済み。本ファイルの
  整備は `claude/sub-agent-backlog-update-Sq350` ブランチで行っている。
- テストスイート: **ユニット + 統合テスト 146 件 + doc テスト 7 件 =
  合計 153 件 pass** (doc テスト 10 件は意図的に `ignore` 指定)。
- Lint: `cargo clippy --workspace --all-targets -- -D warnings` clean。
- 依存方針: std とコンパイラ組み込みの `proc_macro` クレートのみ。
- ワークスペース全体で `unsafe_code = "forbid"` (最小 `block_on` executor
  も `std::pin::pin!` を使い unsafe 回避)。

## 凡例

- ✅ 完了 — 現ブランチに反映済み。
- 🟡 バックログ — 提案済みだが未実装。
- ❌ やらない — 明示的にスコープ外と判断。
- ⚠️ 既知の制約 — 認識しているが、現時点ではブロッカーではない。

---

## ✅ 完了

着地したコミット / マイルストーン別に分類。

### M1 — 初期ワークスペース (コミット `793f067`)

- ✅ 3 クレート構成のワークスペース骨格: `propcheck-core`, `propcheck`,
  `propcheck-fuzz`。
- ✅ `Rng` trait + `XorShift64` PRNG。
- ✅ `Arbitrary` trait + プリミティブ・`Vec`・`String`・`Option`・
  `Result`・タプル (2〜4 アリティ) の実装。
- ✅ プロパティランナー: `forall` / `run`、自動 shrink、
  `Outcome::{Passed, Failed}`、panic を失敗として捕捉。
- ✅ `PROPCHECK_SEED` による seed ベース再現。
- ✅ mutation・バイト fuzzer (`fuzz`)、corpus splicing、
  panic 駆動の crash minimization、`PROPCHECK_FUZZ_SEED`。

### M2 — Strategy combinator と追加 impl (コミット `27c663d`)

- ✅ `Strategy` trait + `StrategyExt` 拡張 trait。
- ✅ combinator: `any`, `just`, `int_range`, `vec_of`, `one_of`,
  `weighted_one_of`, `tuple`, `Map`, `Filter`, `BoxedStrategy`。
- ✅ 追加 `Arbitrary` 実装: `HashMap`, `HashSet`, `BTreeMap`,
  `BTreeSet`, `VecDeque`, `[T; N]`, `Box<T>`, `Range<T>`, `Duration`。
- ✅ Object-safe な `Rng` (ジェネリックメソッドに `Self: Sized` 制約)。
- ✅ MSRV を **1.82** へ引き上げ (`PanicHookInfo` のため)。

### M3 — Derive マクロ、assertion、型付きファズ (コミット `ef9f8b1`)

- ✅ 新クレート `propcheck-derive` (手書き parser、`syn`/`quote` 不使用)。
- ✅ `#[derive(Arbitrary)]` を named-field / tuple / unit 構造体に対応、
  ジェネリクスも含む。
- ✅ `#[propcheck]` 属性マクロ (引数 0/1/N 全対応)。
- ✅ `prop_assert!`, `prop_assert_eq!`, `prop_assert_ne!` マクロ
  (ファイル名・行番号・両辺の値を含む詳細な失敗メッセージ)。
- ✅ `prop_assume!` マクロ + 構造化された `PropDiscard` payload。
- ✅ `fuzz_typed<T: Arbitrary>`: バイト seed を `T::arbitrary` に渡す
  型付き fuzzing。
- ✅ `Outcome::Failed` の panic 表示に再現情報を含める便利出力。

### M4 — Tier-B 追加 (コミット `59724cd`)

- ✅ `classify!` マクロ + `Classifications` 集計、ラン結果サマリに描画。
- ✅ `IntoPropResult` trait: プロパティが `bool`, `()`, `Result<(), E>`,
  `PropResult` のいずれかを返せる (`?` 演算子もプロパティ本体で使える)。
- ✅ `#[propcheck(cases = N, seed = N, max_shrinks = N, max_size = N,
  max_discards = N, max_skips = N)]` 属性引数。
- ✅ 失敗 seed の自動再現: `target/propcheck-regressions/<test>.txt`
  に保存し次回ラン冒頭で再生。
- ✅ thread-safe な `SilentPanicHook` (アトミック参照カウント + Mutex)。
- ✅ 追加 `Arbitrary` 実装: `Rc<T>`, `Arc<T>`, `Cell<T>`,
  `RefCell<T>`, `PathBuf`, `OsString`, `Ipv4Addr`, `Ipv6Addr`, `IpAddr`,
  `SocketAddrV4`, `SocketAddrV6`, `SocketAddr`、全整数幅
  (`{Usize, Isize}` 含む) の `NonZero{U,I}*`。
- ✅ Fuzz dictionary (`FuzzConfig::dictionary`)。
- ✅ Crash 重複排除 + 複数 crash 蓄積 (`continue_after_crash`,
  `dedup_by_message`, `FuzzReport::failures: Vec<Failure>`)。
- ✅ ディスク永続化: `FuzzConfig::{corpus_dir, crash_dir}`、
  ハッシュベースのファイル名。

### M5 — Tier S/A/B (コミット `48eba2e`)

- ✅ `#[derive(Arbitrary)]` の **enum** 対応 (unit / tuple / named-field
  variant)、可能なら simplest-variant への shrink collapse。
- ✅ `#[derive(Arbitrary)]` の `where` 句サポート。
- ✅ `propcheck::strategy::str` モジュールに文字列構造化 generator 群:
  `ascii_digits`, `ascii_letters_{lower,upper}`, `ascii_letters`,
  `ascii_alphanumeric`, `hex_string`, `ascii_printable`, `from_char_set`。
- ✅ `prop_assert_matches!` マクロ (オプションで `if` ガード対応)。
- ✅ `prop_compose!` マクロによる宣言的な複合 generator。
- ✅ タプル `Arbitrary` のアリティを 5〜8 まで拡張。
- ✅ 状態機械フレームワーク (`propcheck::state_machine`):
  `StateMachine` trait + `run_state_machine`、操作列 shrink 付き。
- ✅ Async サポート: 組み込み `block_on` (`std::pin::pin!` ベース、
  unsafe ゼロ) + `#[propcheck] async fn` のラッパー。
- ✅ 失敗カテゴリ: `PropSkip` payload + `prop_skip!` マクロ、
  `Config::max_skips` で別カウント。
- ✅ `prop_with_context!` マクロ。thread-local コンテキストスタックを
  assertion メッセージに付加。
- ✅ `Outcome` の accessor メソッド群: `is_passed`, `is_failed`,
  `is_aborted`, `failure_message`, `shrunk`, `original`, `cases`,
  `discarded`, `skipped`, `seed`, `classifications`。
- ✅ Float 生成 / shrink の改善 (NaN, `EPSILON`, `MIN_POSITIVE`,
  `round`, 1-ULP step)。
- ✅ Differential testing ヘルパー (`differential`, `differential_with`)。
- ✅ `Config::shrink_mode` (`ShrinkMode::{Greedy, Exhaustive}`)。

### M6 — フィールド属性 + 依存 / 再帰生成 (コミット `45e9d00`)

- ✅ `#[derive(Arbitrary)]` のフィールド属性
  `#[arbitrary(strategy = ...)]`。文字列リテラル形式
  (`"expr"` 文字列、proptest スタイル) と裸の式形式の両方をサポート。
  named-field 構造体・tuple 構造体・enum variant (両形式) に適用可。
- ✅ `Strategy::flat_map` / `FlatMap`、依存生成のための。
- ✅ `prop_recursive! { leaf = …, inner = …, max_depth = N }` マクロ。
  tree / AST / JSON-like 値の生成に使用。
- ✅ `char_range(lo..hi)` Strategy。
- ✅ `bytes(len_range)` Strategy (`vec_of(any::<u8>(), …)` のシンタックスシュガー)。
- ✅ `f32_range` / `f64_range`、0.0 / lo への shrink 付き。
- ✅ `prop_assert_close!` マクロ (浮動小数点の近似等価 assert)。
- ✅ `propcheck::*` から `Strategy` / `StrategyExt` を再エクスポートし、
  derive 出力のパスが `propcheck-core` の import 無しで解決するように。
- ✅ `prop_filter!` マクロ (`Strategy::filter` のシンタックスシュガー)。

---

## 🟡 バックログ (提案済み、未実装)

各項目には実装コスト見積りと、エージェント影響度の 1 行コメントを
付与。価値順におおまかに並べてあります。

### Polish / nice-to-have

- 🟡 **`Strategy::sample(n)`** デバッグヘルパー (約 15 行)。
  ユーザ (やエージェント) が、テストを走らせずに strategy が
  どんな値を生成するか確認できる。影響: 中 — カスタム strategy を
  書く時に便利。
- 🟡 **`prop_assert_panic!` / `prop_assert_no_panic!`** (約 30 行)。
  closure が panic する / しないを assert。特定入力で想定通り
  panic することのテストに有用。影響: 小。
- 🟡 **`Strategy::no_shrink()`** ラッパー (約 20 行)。1 つの strategy
  だけ shrink を無効化。shrink が高コストだったり、失敗ケースが
  歪んだりする場合に有用。影響: 小。

### 追加 `Arbitrary` 実装

- 🟡 `std::collections::BinaryHeap<T>` (約 20 行)。
- 🟡 `std::collections::LinkedList<T>` (約 20 行)。
- 🟡 `std::num::Wrapping<T>`, `std::num::Saturating<T>` (約 30 行)。
- 🟡 `std::cmp::Ordering` (約 15 行)。
- 🟡 `std::cmp::Reverse<T>` (約 15 行)。
- 🟡 `std::borrow::Cow<'static, str>` / `Cow<'static, [T]>` (約 30 行)。

各々の影響: 小。合わせて stdlib カバレッジを底上げする位置付け。

### ドメイン Strategy パック

- 🟡 **既製ドメイン strategy**: `email_like`, `url_like`,
  `uuid_like`, `ipv4_dotted`, `iso8601_date`。合計約 200 行。
  リスク: 仕様が曖昧で、ユーザの想定する「正しい」と必ずしも一致
  しない。影響: parser テストでは中。

### CI / レポーティング

- 🟡 **JUnit XML / JSON 出力** モード (約 150 行)。ダッシュボード用。
  影響: 低 — 通常用途では `cargo test` の出力で十分。
- 🟡 **`cargo propcheck` CLI サブコマンド** (新規 bin クレート、
  500 行以上)。長時間ファズランの progress bar、名前付き seed
  での再現等を提供。影響: 低 — 既に `cargo test` で動く。

### テスト足回り

- 🟡 **テスト結果アキュムレータ** — 複数プロパティを走らせて、
  最初の失敗で panic させずに全失敗をまとめてレポート。
  大規模な refactor が必要。典型ワークフローでの影響は小。
- 🟡 **`forall!` シンタックスシュガーマクロ** (約 30 行)。影響: 軽微。

### エッジケース

- 🟡 `#[derive(Arbitrary)]` の lifetime パラメータ — 参照は
  そのままでは `Arbitrary` にならない (生成すべき所有値が無い)。
  非サポートとしてドキュメント化するか、`borrowed` アダプタを
  設計する。影響: 小。
- 🟡 `#[propcheck]` のジェネリック関数 — 現状はリジェクト。
  対応するにはテスト関数のジェネリックパラメータをパースし、
  呼出側で単相化する必要がある。影響: 小。
- 🟡 derive のコンパイルエラーメッセージ改善 — フィールド型が
  `Arbitrary` を実装しない場合、現状は generic な trait 境界
  エラーになる。新参ユーザには影響: 中。proc-macro span が必要。

---

## ❌ やらない (明示的にスコープ外)

検討の上で棄却した項目。主な理由は no-deps 方針か、別ライブラリの
責務範囲外であること。

- ❌ **フル正規表現ベース文字列生成** (proptest の `regex` 機能)。
  数千行規模の `regex` 再実装か、クレート依存を必要とする。
  Tier-S の文字列 strategy で 80% のケースをカバー。
- ❌ **カバレッジ駆動 fuzzing** (libFuzzer / SanitizerCoverage)。
  no-deps では到達できない LLVM 計装が必要。このワークフローには
  `cargo-fuzz` を推奨。
- ❌ **本物の async ランタイム**。組み込み `block_on` で純粋な
  async ロジックには十分。ネットワーク / タイマー重量の async
  コードは tokio を sync ラッパーの中で直接使うべき。
- ❌ **snapshot テスト** (insta スタイル)。別領域のテスト
  関心事。`insta` に任せる。
- ❌ **Mock / DI フレームワーク**。propcheck の責務外。
- ❌ **時刻 / 乱数の注入**。ユーザコードや専用 faking ライブラリの
  領分。

---

## ⚠️ 既知の制約

- ⚠️ ランナーはプロセスグローバルな panic hook をインストールする
  (参照カウントはしているが、依然グローバル)。同一プロセス内で
  並行に走るプロパティテストは install を共有する。ユーザコードが
  別途 `panic::set_hook` を呼ぶと順序が問題になる。
- ⚠️ Regression replay は `target/` に書き込む。`CARGO_TARGET_DIR`
  と `CARGO_MANIFEST_DIR` がいずれも未設定だと (例: cargo の外で
  リリースバイナリを実行する場合)、永続化は黙ってスキップされる。
- ⚠️ `#[derive(Arbitrary)]` は属性パスとして
  `#[arbitrary(strategy = some::path::Thing::new())]` のような
  記述と、文字列形式
  `#[arbitrary(strategy = "some::path::Thing::new()")]` を
  区別して解釈しない。両方動くが、文字列形式は Rust 文字列
  エスケープ規則に従う必要がある。
- ⚠️ `prop_recursive!` の `inner` closure は技術的には深さに
  対して指数的に肥大化する strategy を組める。`max_depth` は
  ネスト深さのみを制約し、幅は制約しない。
- ⚠️ `panic = "abort"` の Cargo プロファイルは非互換。ランナーは
  `std::panic::catch_unwind` に依存している。

---

## プロセスメモ

- 今後の作業のプランファイルは `.claude/plans/` に置く (現在は
  本セッションの `README.md` と歴史的分析を格納)。
- 会話で頻出した「Tier S / A / B / C / D」分類は、明示的な
  エージェント工数 × 価値のトレードオフを表すもので、絶対的な
  優先順位ではない。優先度の変化に応じて項目はティア間を移動できる。
