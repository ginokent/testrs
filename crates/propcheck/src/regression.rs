//! Regression-seed persistence.
//!
//! When a property test fails, its seed is appended to
//! `<target>/propcheck-regressions/<sanitized_name>.txt`. On subsequent
//! runs the runner replays those seeds first, before generating any random
//! cases. This catches "the bug is back" regressions deterministically
//! without the user remembering to set `PROPCHECK_SEED`.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Maximum number of seeds retained per file. Excess (oldest) seeds are
/// dropped to keep the file size bounded.
const MAX_SEEDS_PER_FILE: usize = 64;

/// Returns the regression file for `test_name`, or `None` if neither
/// `CARGO_TARGET_DIR` nor `CARGO_MANIFEST_DIR` is set (typical when
/// running from a published binary instead of `cargo test`).
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

/// Reads any persisted regression seeds for this test. Returns an empty
/// vec if the file does not exist or cannot be read.
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

/// Appends `seed` to the regression file for this test, creating parent
/// directories as needed. Bounded to [`MAX_SEEDS_PER_FILE`] most recent
/// entries. Best-effort: errors are silently ignored.
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
        append_seed(&path, 1).unwrap(); // duplicate
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
        // Oldest were dropped; newest are retained.
        assert!(read.contains(&(MAX_SEEDS_PER_FILE as u64 + 9)));
        assert!(!read.contains(&0));
        fs::remove_dir_all(&dir).unwrap();
    }
}
