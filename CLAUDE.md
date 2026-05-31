# CLAUDE.md

[Claude Code](https://claude.ai/code) およびその他のコーディングエージェントへの
**本リポジトリ局所の行動指針**。global な指示 (`~/.claude/CLAUDE.md`) を
上書きするものではなく補足する。

開発フロー (mise install / `mise run ci` 等)、リポ局所の規約、設計判断
(特に `ci-publish-check-dangerously` の危険性) は
[`CONTRIBUTING.md`](CONTRIBUTING.md) を参照すること。

## リポ固有の前提

`propcheck` (testrs) は外部依存ゼロを核とした PBT + in-process fuzzer
ライブラリ。エージェントは以下の制約を **暗黙に破らないこと**:

- **直接依存は std と `proc_macro` のみ**。`syn` / `quote` / `proc-macro2`
  を含む proc-macro 補助 crate も使用禁止 (`SPEC.md` 依存方針)。新しい依存
  追加を提案する前に `SPEC.md` を再読すること
- **`unsafe_code = "forbid"`** が workspace 全体で強制されている。`unsafe`
  ブロックは書かない。`std::pin::pin!` 等で代替する
- **`propcheck-fuzz/examples/find_crash.rs` は意図的に panic する** デモ
  example。CI / test job に混ぜないこと (現状の `cargo test --workspace
  --all-targets` は example の binary build はするが run しないので OK)
- **issue は `issues/` 配下管理** (global `~/.claude/CLAUDE.md` 参照)。完了
  したら `issues/completed/` に移動し、同じ commit に含める
- **PBT で記述可能なものは単体テストで書かない** (`~/.claude/CLAUDE.md` の
  「テストの役割分担」参照)。ただし `propcheck-core` 内のテストは PBT runner
  自体が使えない (循環依存) ため、ループ + 固定 seed の単体テストで書く

## エージェントが PR を作る前にやること

1. `mise run ci` がローカルで緑になっていること (詳細手順は `CONTRIBUTING.md`
   の「開発手順」セクション)
2. 変更点と `README.md` / `SPEC.md` / 本 `CLAUDE.md` / `CONTRIBUTING.md` の
   整合性を確認
3. 関連 issue を `issues/completed/` に移動し、同じ commit に含める
4. commit message が global `~/.claude/CLAUDE.md` の規約 (動詞終止形 /
   体言止め本文) に従う

## `mise run ci-publish-check-dangerously` の利用条件

本 task は CI と完全同名の check run を PR HEAD に投影することで、Branch
protection の Required check を local 1 回で満たせる構造を持つ。**コスト
削減目的のユーザー責任設計** で、ローカル環境差で本来 CI が落ちる check が
local で緑になりうる。

**エージェントが本 task を呼ぶ前に、必ず [`CONTRIBUTING.md`](CONTRIBUTING.md)
の「`ci-publish-check-dangerously` — コスト削減目的のユーザー責任設計」
セクションを読み、ユーザーが「危険性を理解して実行する」旨を明示している
ことを確認すること**。ユーザーからの明示的指示なしに本 task を自発的に呼ば
ないこと。
