# Gap 分析 #3 — エージェントによるテスト記述向けの Tier S/A/B

**Status: コミット `48eba2e` (M5) で着地。**

## 動機

M4 終了時点でライブラリは堅実な骨格を備えていた。3 回目の gap
分析では、**エージェント (LLM) がこのライブラリで良いテストを書く**
ために必要なものは何か、に的を絞った。残ったギャップをティア別に
整理した:

## 実装した項目

### Tier S — エージェントの最大ブロッカー

1. **enum 向け `#[derive(Arbitrary)]`** (unit / tuple / named-field
   バリアント、ジェネリクス対応)。これが無いと実際の Rust クレートの
   ~50% が derive 経由で扱えない。
2. **構造化文字列 strategy** (`str::ascii_digits`,
   `ascii_alphanumeric`, `hex_string`, `ascii_letters_*`,
   `ascii_printable`, `from_char_set`)。これが無いとパーサテストが
   ランダム Unicode でケースを浪費する。
3. **`prop_assert_matches!`** (オプションの `if` ガード付き)。
   enum を返すパーサや状態機械のテストに必須。

### Tier A — 大きな quality-of-life

4. 宣言的な複合ジェネレータのための **`prop_compose!`** (proptest
   のシンタックス互換)。
5. **タプル arity 5..=8** — 引数の多いテスト関数を動かせるように。
6. **状態機械フレームワーク** — `StateMachine` trait + ランナー。
   ランダムな操作列を生成・適用し、最初の不変条件違反を報告。
   操作の greedy 削除で shrink する。
7. **Async サポート** — 組み込み `block_on` (Condvar parker、
   `std::pin::pin!` ベース、unsafe ゼロ)。`#[propcheck]` マクロは
   `async fn` を検出し、本体を `block_on` でラップする。
8. **`PropSkip` ペイロード + `prop_skip!` マクロ**。discard とは別
   集計のため、フレーキーな環境がノイジーなジェネレータを覆い隠さない。
9. アサーションメッセージに付加される thread-local コンテキスト
   スタック付き **`prop_with_context!`**。
10. **`Outcome` アクセサ** (`is_passed`, `failure_message`,
    `shrunk`, `seed` 他) — プログラム的な検査用。

### Tier B — 仕上げ

11. `#[derive(Arbitrary)]` の **where 句サポート**。
12. **Float 改善** — より興味深い値 (`EPSILON`, `MIN_POSITIVE`, NaN)、
    shrink での ULP-step + `round`。
13. 2 つの実装を比較する **`differential` / `differential_with`** ヘルパー。
14. **`Config::shrink_mode` (`Greedy` / `Exhaustive`)**。

## 設計判断

- **simplest-variant への enum shrink collapse。** unit バリアントが
  存在する場合のみ発火。それ以外はバリアント内に留まりフィールドを
  shrink。collapse がプロパティを成功させてしまう場合は採用しない
  (失敗バリアントを掘り続ける方が望ましいため)。
- **ランタイム依存ゼロでの async。** Mutex+Condvar parker による
  最小 `block_on` で純粋な async ロジックには十分。ネットワーク /
  タイマー重量のコードはどのみち tokio が必要で、ユーザ側で
  ラップすればよい。
- **Skip と discard。** カウンタを分け、両方に abort 閾値を持つ。
- **Differential testing。** `run_with` 上の薄いラッパで、両実装を
  呼び `prop_assert_eq!` で出力を比較するだけ。

## 実装順

1. 小規模で独立な項目を先に (タプル 5..=8、追加 std impl、
   アクセサメソッド)。
2. `IntoPropResult` と `prop_skip!` の配線。
3. derive マクロ拡張: enum サポート、where 句、属性引数。
4. 状態機械フレームワーク。
5. Async (block_on + マクロラッパ)。
6. Differential + ShrinkMode。

## 棄却

- CLI サブコマンド (スコープ外)。
- JUnit XML (価値低、後付け容易)。
- カバレッジ駆動ファジング (LLVM バインディング無しでは不可能)。

## 検証

ユニット + 統合テスト 134 件 pass、clippy clean。バグ入りカウンタ
の状態機械テストで、22 操作の失敗列を `[Decrement]` まで縮められる
ことを確認。
