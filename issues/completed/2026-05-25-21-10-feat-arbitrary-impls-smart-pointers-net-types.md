# スマートポインタ・ネットワーク型・NonZero 系の Arbitrary 実装を追加する

- Priority: -
- Created: -
- Completed: 2026-05-25 20:47 JST (BACKLOG.md から移行)
- Model: -
- Branch: - (該当コミット: `59724cd`)

## 完了内容

追加 `Arbitrary` 実装: `Rc<T>`, `Arc<T>`, `Cell<T>`,
`RefCell<T>`, `PathBuf`, `OsString`, `Ipv4Addr`, `Ipv6Addr`, `IpAddr`,
`SocketAddrV4`, `SocketAddrV6`, `SocketAddr`、全整数幅
(`{Usize, Isize}` 含む) の `NonZero{U,I}*`。

## 関連マイルストーン

M4 — Tier-B 追加 (コミット `59724cd`)
