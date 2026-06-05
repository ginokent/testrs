# #[derive(Arbitrary)] の lifetime パラメータ方針を決める

- Priority: Low
- Created: 2026-05-25 21:01 JST
- Completed: 2026-06-05
- Model: -
- Branch: feature/add-derive-arbitrary-lifetime-decision

## 目的

`#[derive(Arbitrary)]` 対象型に lifetime パラメータが含まれる
ケースについて、明示的な方針を決め (非サポートとしてドキュメント化
するか、`borrowed` アダプタを設計するか) て対応する。

## 経緯

`BACKLOG.md` の 🟡 バックログ項目「エッジケース」から
`issues/` 配下へ移行する。

## 優先度根拠

BACKLOG.md 上の影響度は「小」。Low とする。

## 現状・問題

参照型はそのままでは `Arbitrary` にならない (生成すべき所有値が
無い)。現状は曖昧なコンパイルエラーになる。

## 完了条件

- 方針を決定する (非サポート明示 / アダプタ提供のいずれか)
- 非サポートの場合: derive で lifetime パラメータを検出した時に
  明確なエラーメッセージを出すこと、README / docs に記載すること
- アダプタの場合: `borrowed` アダプタの API と PBT を追加すること

## 解決方法

BACKLOG.md 記載の通り、まず方針決定が必要。非サポート明示が
最小コストで、`borrowed` アダプタは別 issue とする選択もあり得る。

## 設計方針

- no-deps 方針を維持する
- proc-macro での lifetime parameter 検出は既存 parser を拡張する
- 方針決定後、README の「制約」セクションに反映する

## 決定 (2026-06-05)

**(A) 非サポート明示** を採用する。

(B) borrowed アダプタを退けた根拠 (批判的検討):

- `arbitrary` クレートのように `Unstructured<'a>` 入力バッファから借用を
  切り出すモデルなら borrowed 生成は一般には coherent。よって「PBT 一般で
  無意味」とは言えない
- ただし propcheck の `Arbitrary::arbitrary(&mut Rng)` は乱数源しか取らず、
  lifetime を帯びた入力供給元を持たない。呼び出し側が選ぶ任意の `'a` に
  対し `&'a T` を生成する手段が無い (`&'static` を除く)
- 成立させるには中核 trait を lifetime 入力前提に再設計する必要があり、
  `Rng` 駆動 + `shrink` という SPEC の中核と衝突する破壊的変更になる。
  かつテスト入力は所有値で持ち本体内で借用すれば足り PBT 上の便益が無い

→ 以上より A。

## 実装内容

- `parse_generics` が lifetime 名 (`'a` 形式) を収集して返すよう拡張
- `parse_input` が `reject_lifetimes` で型名と併せて拒否
- `reject_lifetimes` は原因の型名・全 lifetime・非サポートである旨を含む
  `compile_error!` 文言を生成する純粋関数。エラーパスを単体テストで検証
  (proc_macro token 走査は実行文脈が必要で単体テスト不可のため、文言生成を
  純粋関数に分離)
- README「制約」セクションと derive クレートのモジュール doc に非対応を明記
