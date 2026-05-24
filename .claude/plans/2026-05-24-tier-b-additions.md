# Gap 分析 #2 — Tier B+ 追加

**Status: コミット `59724cd` (M4) で着地。**

## 動機

M1〜M3 終了時点でワークスペースには以下が揃っていた:

- 3 クレート (`-core`, `-derive`, `propcheck`)。
- `#[derive(Arbitrary)]` (struct 対応)、`#[propcheck]` 属性マクロ。
- `prop_assert*!` ファミリ、`prop_assume!`。
- 基本的な strategy コンビネータ。

2 回目の gap 分析の問い: 実プロジェクトでプロパティテストを書く
エージェントが、なお詰まる箇所はどこか?

## 特定したギャップ

### Tier B — 必須追加

1. ケースごとの分布診断用 `classify!`。
2. プロパティが `Result<(), E>` を返せて `?` を直接使えるための
   `IntoPropResult`。
3. `#[propcheck(cases = N, seed = N, …)]` 属性引数。
4. 失敗 seed の自動再現: 保存して次ラン冒頭で再生。
5. thread-safe な panic-hook silencer。
6. 追加 std `Arbitrary` 実装: `Rc`, `Arc`, `Cell`, `RefCell`,
   `PathBuf`, `OsString`, `Ipv*`, `SocketAddr*`, `NonZero*`。

### Tier C — ついでに含めたもの

7. Fuzz dictionary (`FuzzConfig::dictionary`)。
8. Crash 重複排除 + 最初の crash 後も継続。
9. コーパス / crash のディスク永続化。

### Tier D — 棄却

- CLI サブコマンド、JUnit XML、フル regex 文字列、スナップショット
  テスト。

## 実装順

1. 軽量な追加から: `Outcome` フィールド拡張、アクセサヘルパー、
   panic-hook RAII。
2. `IntoPropResult` (ランナー署名は変えるが、blanket
   `impl IntoPropResult for bool` で互換性を保ったまま追加可能)。
3. `classify!` を `run_loop` 経由で配線 (各ケース前にリセット、
   後に収穫)。
4. Regression replay: `target/propcheck-regressions/` 下のファイル
   IO、seed の sanitization + 保管数上限。
5. `propcheck-derive` 側の属性引数パース (整数リテラル値の key/value)。
6. Fuzz アップグレード: dictionary mutation, `dedup_by_message`,
   `continue_after_crash`、on-disk corpus + crash dirs。

## 設計判断

- **Discard と skip。** 別々のカウンタと abort 閾値を持つ —
  `prop_assume!` (入力の前提条件未充足) は `PropDiscard` 経由、
  `prop_skip!` (環境が未整備) は M5 で `PropSkip` 経由・別の
  `max_skips` 予算として追加。
- **Regression ファイル配置。** `$HOME` ではなく `target/` 配下。
  ビルドディレクトリと一緒に動き、デフォルトで gitignore 対象。
- **Panic-hook 参照カウント。** atomic `usize` カウンタ +
  `Mutex<Option<…>>` で旧 hook を保持。並行する install は単一を
  共有。

## 検証

ユニット + 統合テスト 86 件 pass、clippy clean。E2E の regression
replay テスト (`tests/regression_replay.rs`) は temp ディレクトリへ
書き込み、2 回目の起動で失敗が再現することを検証。
