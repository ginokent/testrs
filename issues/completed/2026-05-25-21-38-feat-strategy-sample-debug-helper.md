# Strategy::sample(n) デバッグヘルパーを追加する

- Priority: Medium
- Created: 2026-05-25 20:47 JST
- Completed: 2026-05-29 JST (feature/add-strategy-sample-and-no-shrink で実装)
- Model: -
- Branch: feature/add-strategy-sample-and-no-shrink

## 目的

ユーザ (やエージェント) が、テストを走らせずに strategy が
どんな値を生成するか確認できるようにする。

## 経緯

`BACKLOG.md` の 🟡 バックログ項目「Polish / nice-to-have」から
`issues/` 配下へ移行する。

## 優先度根拠

BACKLOG.md 上の影響度は「中 — カスタム strategy を書く時に便利」と
記載されている。カスタム strategy 開発時のフィードバックループを
短縮する効果が見込めるため Medium とする。

## 現状・問題

現状、strategy がどのような値を生成するか確かめるには
プロパティテストを書いてランナーを起動する必要がある。
strategy 単体での挙動確認手段がないため、開発体験が悪い。

## 完了条件

- `Strategy` (もしくは `StrategyExt`) に `sample(n: usize) -> Vec<Self::Value>`
  を追加する
- 既存 strategy で動作する単体テストと、生成数・型の関係を確認する
  PBT を追加する

## 解決方法

BACKLOG.md 記載の概算は約 15 行。`StrategyExt` に追加し、内部で
固定 seed (もしくは `Config::default()` 相当の seed) の `Rng` を
作って `n` 回 `value_tree` を回し、その root 値を集める。

## 設計方針

- no-deps 方針 (std と `proc_macro` のみ) を維持する
- 既存 `Strategy` / `Rng` trait を再利用し、新規 trait は追加しない
- `Rng` 固定 seed を内部生成することで、テスト不要なデバッグ用途で
  あっても再現性のある出力を返す

## 完了内容

- `crates/propcheck-core/src/strategy.rs` の `StrategyExt` trait に `sample(&self, n: usize) -> Vec<Self::Value>` を default method として追加
- 固定 seed (`0xC0DE_BABE_C0DE_BABE`、PBT 本体の `env_seed()` とは独立) と固定 size (`32`) で `n` 回 `new_value` を呼び、戻り値を `Vec` に集める実装
- 同じ strategy への複数回呼び出しが同じ列を返すこと (deterministic) と、生成値が strategy の値域に収まることを確認する単体テストを追加 (`sample_returns_requested_count` / `sample_is_deterministic_across_calls` / `sample_values_lie_in_strategy_range`)
- `propcheck::StrategyExt` 経由で外部から `s.sample(n)` が呼び出せる (`propcheck-core::strategy` モジュールが `propcheck` から再エクスポート済みのため追加作業なし)
