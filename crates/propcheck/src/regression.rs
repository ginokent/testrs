//! regression seed の永続化を行います。
//!
//! プロパティテストが失敗すると、その seed は
//! `<target>/propcheck-regressions/<sanitized_name>.txt` に追記されます。次回以降の
//! 実行では、runner はランダムなケースを生成する前にまずこれらの seed を再生します。
//! これにより、ユーザーが `PROPCHECK_SEED` を設定することを覚えていなくても、
//! 「バグが再発した」という regression を決定論的に捕捉できます。

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// 1 ファイルあたり保持する seed の最大数です。ファイルサイズを抑えるため、
/// 超過分（最も古い seed）は破棄されます。
const MAX_SEEDS_PER_FILE: usize = 64;

/// `test_name` に対応する regression ファイルを返します。`CARGO_TARGET_DIR` と
/// `CARGO_MANIFEST_DIR` のいずれも設定されていない場合（`cargo test` ではなく
/// 公開済みのバイナリから実行された場合に典型的）は `None` を返します。
pub(crate) fn regression_file_path(test_name: &str) -> Option<PathBuf> {
    let target_dir = if let Ok(t) = env::var("CARGO_TARGET_DIR") {
        PathBuf::from(t)
    } else if let Ok(m) = env::var("CARGO_MANIFEST_DIR") {
        PathBuf::from(m).join("target")
    } else {
        return None;
    };
    let sanitized = sanitize(test_name);
    Some(
        target_dir
            .join("propcheck-regressions")
            .join(format!("{sanitized}.txt")),
    )
}

fn sanitize(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push('_');
    }
    out
}

/// このテストについて永続化されている regression seed を読み込みます。
/// ファイルが存在しない、または読み込めない場合は空の vec を返します。
pub(crate) fn read_seeds(path: &Path) -> Vec<u64> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut seen: BTreeSet<u64> = BTreeSet::new();
    let mut out: Vec<u64> = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Ok(seed) = trimmed.parse::<u64>() {
            if seen.insert(seed) {
                out.push(seed);
            }
        }
    }
    out
}

/// このテストの regression ファイルに `seed` を追記します。必要に応じて親
/// ディレクトリを作成します。直近の [`MAX_SEEDS_PER_FILE`] 件までに制限されます。
/// ベストエフォートで動作し、エラーは黙って無視されます。
pub(crate) fn append_seed(path: &Path, seed: u64) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut seeds = read_seeds(path);
    if seeds.contains(&seed) {
        return Ok(());
    }
    seeds.push(seed);
    if seeds.len() > MAX_SEEDS_PER_FILE {
        let excess = seeds.len() - MAX_SEEDS_PER_FILE;
        seeds.drain(..excess);
    }
    let mut content = String::new();
    content.push_str("# Regression seeds replayed by propcheck. Add or remove freely.\n");
    for s in &seeds {
        content.push_str(&format!("{s}\n"));
    }
    fs::write(path, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_keeps_safe_chars() {
        assert_eq!(sanitize("ok_test-1"), "ok_test-1");
        assert_eq!(sanitize("a::b/c d"), "a__b_c_d");
        assert_eq!(sanitize(""), "_");
    }

    #[test]
    fn append_and_read_roundtrip() {
        let dir = std::env::temp_dir().join(format!("propcheck-regtest-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("foo.txt");
        append_seed(&path, 1).unwrap();
        append_seed(&path, 2).unwrap();
        append_seed(&path, 1).unwrap(); // 重複
        let read = read_seeds(&path);
        assert_eq!(read, vec![1, 2]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn appends_bounded_to_max() {
        let dir =
            std::env::temp_dir().join(format!("propcheck-regtest-bound-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("bound.txt");
        for s in 0..(MAX_SEEDS_PER_FILE as u64 + 10) {
            append_seed(&path, s).unwrap();
        }
        let read = read_seeds(&path);
        assert_eq!(read.len(), MAX_SEEDS_PER_FILE);
        // 最も古いものは破棄され、最も新しいものが保持されます。
        assert!(read.contains(&(MAX_SEEDS_PER_FILE as u64 + 9)));
        assert!(!read.contains(&0));
        fs::remove_dir_all(&dir).unwrap();
    }
}
