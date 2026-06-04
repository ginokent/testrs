# FuzzReport.iterations のセマンティクス不一致を解消する

- Priority: Low
- Created: 2026-06-03 05:51 JST
- Completed: 2026-06-04
- Model: -
- Branch: feature/fix-fuzz-report-iterations-semantics

## 目的

`FuzzReport.iterations` の doc と実装の不一致を解消し、フィールドが
表す意味を一意に定める。

## 経緯

コードベースの批判的レビュー中に、`continue_after_crash = true` の
ケースで `iterations` が実態とかけ離れた値を返すことが判明したため
起票する。

## 優先度根拠

crash 入力自体は各 `Failure.iteration` に正しく格納されており、
誤った値が出るのは report の集計フィールド表示のみで実害は限定的。
ただし「完了回数」を信じて統計に使うコードは誤った値を得るため
放置すべきでないとして Low とする。

## 現状・問題

- フィールド doc (`crates/propcheck-fuzz/src/lib.rs:55` 付近) は
  `iterations` を「完了したターゲット呼び出しの回数」と説明し、
  モジュール例 (`crates/propcheck-fuzz/src/lib.rs:35` 付近) も
  `"crash after {} iters"` と表示している。
- 実装 (`crates/propcheck-fuzz/src/lib.rs:233-238` 付近) は
  `iterations = failures.last().map(|f| f.iteration).unwrap_or(cfg.iterations)`。
- `continue_after_crash = false` のときは初回 crash で break するため
  crash の 1-based index ≒ 完了回数で概ね一致する。しかし
  `continue_after_crash = true` のときはループが `cfg.iterations` 回まで
  回り続けるのに、`iterations` は最後に見つかった crash の iteration
  index になり、実際の完了回数より大幅に小さい値を返す。

## 完了条件

- `FuzzReport.iterations` の doc と実装が一致する
- `continue_after_crash = true` で複数 crash が発生するケースを含む
  単体テストで、`iterations` が定義どおりの値を返すことを検証する

## 解決方法

以下のいずれかを採る (要ユーザー確認)。後方互換不要の方針なら (a) が
素直。

- (a) 実際に完了したターゲット呼び出し回数を別途カウントして格納し、
  doc の「完了したターゲット呼び出しの回数」と一致させる
- (b) フィールドを「最後の crash の iteration / crash が無ければ総数」と
  再定義し、doc・モジュール例を実装に合わせて修正する

## 設計方針

- no-deps 方針を維持する
- `unsafe_code = "forbid"` を維持する
- セマンティクスのクリーンさを優先し、後方互換は考慮しない
