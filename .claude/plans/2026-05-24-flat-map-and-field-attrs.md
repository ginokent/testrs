# Gap 分析 #4 — `#[arbitrary]` フィールド属性 + 依存 / 再帰生成

**Status: コミット `45e9d00` (M6) で着地。**

## 動機

M5 終了時点でライブラリは `proptest` とおおむね機能パリティに達し
さらに追加機能 (状態機械、async、regression replay) も備えていた。
4 回目の gap 分析では、`#[derive(Arbitrary)]` ワークフローを実運用
でエージェントに優しいものにするのを阻んでいた、残り 5 項目を特定
した。

## 実装した項目

### Tier S — 最後の大きな障壁

1. **`#[derive(Arbitrary)]` のフィールド属性
   `#[arbitrary(strategy = ...)]`。** これが唯一最大の残ギャップ
   だった。これが無いと derive 生成の `Arbitrary` impl は「型の
   ドメイン中の何でも」を生成する — つまり不正な入力も含み、
   プロパティ側で `prop_assume!` でフィルタする羽目になる。フィールド
   属性があると:

   ```rust
   #[derive(Arbitrary)]
   struct Request {
       #[arbitrary(strategy = "str::ascii_alphanumeric(1..20)")]
       user_id: String,
       #[arbitrary(strategy = int_range(1024u16..65535))]
       port: u16,
   }
   ```

   文字列リテラル形式 (proptest スタイル) と裸の式形式の両方を
   サポート。named-field 構造体・tuple 構造体・enum バリアントの
   フィールド (両形式) に適用可能。

### Tier A — 重要な追加

2. **`Strategy::flat_map`** — 依存生成のため。「先に長さを決め、
   その後にちょうどその長さの `Vec`」が、それまでは独自 Strategy
   実装無しには不可能だった。
3. tree / AST / JSON ライク値向けの **`prop_recursive!` マクロ**。
   内部では深さ制限付きの `recursive` ビルダを呼び、`inner` を
   `leaf` の上に `max_depth` 回まで積み上げる。
4. **`char_range(lo..hi)`** strategy (半開区間内の char。サロゲートは
   スキップ)。
5. **`bytes(len_range)`** strategy シュガー。
6. **`f32_range` / `f64_range`** strategy。範囲に 0 を含めば 0.0、
   それ以外は `lo` に向けて shrink。
7. 浮動小数点の近似等価アサート用 **`prop_assert_close!`** マクロ。

### ついでに入れた小ぶりな追加

- `prop_filter!` マクロ (`StrategyExt::filter` のシュガー)。
- `propcheck::{Strategy, StrategyExt}` をクレートルートで
  再エクスポート — derive の生成パスが解決するように。

## 設計判断

- **フィールド属性パーサ。** 手書き、`syn` 不使用。`strategy = "expr"`
  (文字列リテラル — proptest スタイル) と `strategy = expr`
  (裸トークン) の両方をサポート。文字列形式はクォートを剥がした上で
  マクロ出力時に Rust パーサに渡される。
- **コード生成の分岐。** フィールドごとの生成は
  `Arbitrary::arbitrary` (デフォルト) と
  `{ let __strat = (EXPR); Strategy::new_value(&__strat, ...) }`
  (属性付き) を使い分け。shrink もこれに合わせ `Strategy::shrink_value`
  を使う。これにより shrink が strategy の下限を尊重する — 仕込み
  失敗テストで `int_range(10..1000)` が 0 ではなく **51** に縮む
  ことを確認済み。
- **`Fields::Named` / `Fields::Unnamed` 表現。** 双方とも従来の
  `Vec<(String, String)>` ではなく `Vec<FieldInfo>` (name, ty,
  optional strategy) を保持するよう統一。tuple フィールドの属性も
  同じ仕組みで動くようになる。
- **`prop_recursive!` の型付け。** `inner` クロージャは
  `BoxedStrategy<T>` に限らず任意の `Strategy<Value = T>` を
  返せる。これにより `prop_oneof![...]` を明示的な `.boxed()` 無し
  でクリーンに組める。
- **`proc_macro_derive` の `attributes(...)`**。derive マクロは
  `attributes(arbitrary)` を宣言し、`#[arbitrary(...)]` 構文を rustc
  が受理するように。
- **`unsafe_code = forbid` を維持。** 新規コードパス (FlatMap,
  recursive, CharRange, FloatRange) はすべて safe-only。

## 棄却 (先送り or 却下)

- ドメイン pre-canned strategy (`email_like`, `url_like` 等) — 仕様
  曖昧、価値低、約 200 行。
- `prop_assert_panic!` / `prop_assert_no_panic!` — 影響小。
- `Strategy::sample()` デバッグヘルパー — バックログ送り。
- カバレッジ駆動ファジング、regex 文字列、async ランタイム、スナップ
  ショットテスト — 前マイルストーンと同じ理由で却下。

## 検証

ユニット + 統合テスト 153 件 pass、clippy clean。`derive_demo`
example が新機能すべてを 1 ファイル 80 行で行使。仕込み失敗ケース
では `User { age: 77, name: "e4aXBPirBVfg8", favorite_numbers:
[35, 49, 34] }` が `User { age: 51, name: "0", favorite_numbers:
[] }` まで縮む — フィールド属性 strategy が生成 **と** shrink の
両方を駆動していることを示す。
