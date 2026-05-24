//! インプロセスの mutation fuzzer です。
//!
//! ミューテートされたバイトバッファのストリームでターゲット関数を駆動し、
//! panic を crash として捕捉し、panic を引き起こし続けるバイトを繰り返し
//! 削除することで crash 入力を最小化します。
//!
//! 機能:
//!
//! - **crash の重複排除と継続**: crash を発見した後、
//!   [`FuzzConfig::continue_after_crash`] を設定すると、追加の異なる crash
//!   を探し続けます。重複排除はデフォルトで panic メッセージに基づきます。
//! - **辞書**: 事前に与えられたバイトスライス（マジックナンバー、キーワード、
//!   プロトコルトークン）が高い確率でミューテートされた入力にスプライス
//!   され、カバレッジフィードバックなしでも素朴な mutator ーが興味深い
//!   状態に到達するのを助けます。
//! - **corpus 永続化**: [`FuzzConfig::corpus_dir`] をディレクトリに指定すると、
//!   fuzzer は起動時にエントリを読み込み、新たに発見された興味深い入力を
//!   時間とともに追記していきます。
//! - **crash 永続化**: [`FuzzConfig::crash_dir`] は各ユニークな crash を
//!   メッセージのハッシュに基づく名前のファイルに保存するため、再現用入力が
//!   実行をまたいで蓄積されます。
//!
//! # 例
//!
//! ```no_run
//! use propcheck_fuzz::{fuzz, FuzzConfig};
//!
//! let report = fuzz(FuzzConfig::default(), |data: &[u8]| {
//!     if data.len() >= 3 && &data[..3] == b"BAD" {
//!         panic!("found magic bytes");
//!     }
//! });
//!
//! if let Some(f) = report.failure() {
//!     eprintln!("crash after {} iters: {:?}", report.iterations, f.input);
//! }
//! ```

use std::any::Any;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;

pub use propcheck_core::{Arbitrary, Rng, XorShift64};

mod typed;
pub use typed::{fuzz_typed, TypedFuzzConfig};

/// fuzzing 実行向けの調整可能パラメータです。
#[derive(Debug, Clone)]
pub struct FuzzConfig {
    /// 諦めるまでに実行する入力の最大数です。
    pub iterations: usize,
    /// 生成される任意の入力の最大サイズ（バイト単位）です。
    pub max_input_len: usize,
    /// PRNG の seed です。デフォルトは `PROPCHECK_FUZZ_SEED` または時計エントロピーです。
    pub seed: u64,
    /// 初期 corpus です。これと `corpus_dir` の両方が空である場合は、
    /// 空の入力が追加されます。
    pub initial_corpus: Vec<Vec<u8>>,
    /// crash 入力の最小化に費やす試行の最大数です。
    pub minimize_steps: usize,
    /// `true` の場合、fuzzing 中の panic フックを無音化し、crash の
    /// バックトレースが端末に大量出力されないようにします。
    pub silence_panic_hook: bool,
    /// `true` の場合、fuzzer は crash 発見後も実行を継続し、追加の
    /// 異なる crash を発見します（重複排除の対象となります）。
    pub continue_after_crash: bool,
    /// `true` の場合、同一の panic メッセージを持つ crash は 1 度だけ報告されます。
    pub dedup_by_message: bool,
    /// 辞書 mutation によって入力にスプライスされるバイトスライスです。
    /// プロトコル / フォーマットのマジックナンバーやキーワードに便利です。
    pub dictionary: Vec<Vec<u8>>,
    /// 起動時に corpus エントリを読み込み、実行中に新しい「興味深い」入力を
    /// 追記するディレクトリです。ファイル名はその内容のハッシュに基づきます。
    /// `None` は corpus 永続化を無効にします。
    pub corpus_dir: Option<PathBuf>,
    /// 各ユニークな crash の再現用ファイルを保存するディレクトリです。
    /// ファイル名は panic メッセージのハッシュから導出されます。
    /// `None` は crash 永続化を無効にします。
    pub crash_dir: Option<PathBuf>,
}

impl Default for FuzzConfig {
    fn default() -> Self {
        Self {
            iterations: 10_000,
            max_input_len: 4096,
            seed: env_seed(),
            initial_corpus: Vec::new(),
            minimize_steps: 500,
            silence_panic_hook: true,
            continue_after_crash: false,
            dedup_by_message: true,
            dictionary: Vec::new(),
            corpus_dir: None,
            crash_dir: None,
        }
    }
}

fn env_seed() -> u64 {
    if let Ok(s) = env::var("PROPCHECK_FUZZ_SEED") {
        if let Ok(n) = s.parse::<u64>() {
            return n;
        }
    }
    XorShift64::from_entropy().state()
}

/// fuzzing 実行の結果です。
#[derive(Debug, Clone)]
pub struct FuzzReport {
    /// 完了したターゲット呼び出しの回数です。
    pub iterations: usize,
    /// 使用された PRNG の seed で、再現に適しています。
    pub seed: u64,
    /// 発見されたすべてのユニークな crash を、最初に発見された順に並べたものです。
    pub failures: Vec<Failure>,
}

impl FuzzReport {
    /// 最初に発見された crash、または存在しない場合は `None` です。
    pub fn failure(&self) -> Option<&Failure> {
        self.failures.first()
    }
}

/// fuzzer が発見した crash 入力です。
#[derive(Debug, Clone)]
pub struct Failure {
    /// 最小化された crash 入力です。
    pub input: Vec<u8>,
    /// panic メッセージ（または `<non-string panic payload>`）です。
    pub message: String,
    /// crash が発見された 1 始まりのイテレーションインデックスです。
    pub iteration: usize,
}

/// `cfg` に基づいて `target` を実行します。
pub fn fuzz<F>(cfg: FuzzConfig, target: F) -> FuzzReport
where
    F: FnMut(&[u8]),
{
    if cfg.silence_panic_hook {
        let prev = panic::take_hook();
        panic::set_hook(Box::new(|_| {}));
        let report = run_fuzz(cfg, target);
        panic::set_hook(prev);
        report
    } else {
        run_fuzz(cfg, target)
    }
}

fn run_fuzz<F>(cfg: FuzzConfig, mut target: F) -> FuzzReport
where
    F: FnMut(&[u8]),
{
    let mut rng = XorShift64::seed_from_u64(cfg.seed);

    // corpus を読み込みます: initial_corpus と corpus_dir の内容の和集合です。
    let mut corpus: Vec<Vec<u8>> = cfg.initial_corpus.clone();
    if let Some(dir) = &cfg.corpus_dir {
        corpus.extend(load_corpus_dir(dir));
    }
    if corpus.is_empty() {
        corpus.push(Vec::new());
    }
    // 既にディスクへ保存した corpus の内容を（ハッシュで）追跡します。
    let mut persisted_hashes: BTreeSet<u64> = corpus.iter().map(|e| fnv_hash(e)).collect();

    let mut failures: Vec<Failure> = Vec::new();
    let mut seen_crash_messages: BTreeSet<String> = BTreeSet::new();

    for i in 0..cfg.iterations {
        let idx = rng.gen_range_usize(0, corpus.len());
        let base = corpus[idx].clone();
        let mut input = base;
        let n_mutations = rng.gen_range_u64(1, 5);
        for _ in 0..n_mutations {
            mutate(
                &mut rng,
                &mut input,
                &corpus,
                &cfg.dictionary,
                cfg.max_input_len,
            );
        }

        match invoke(&mut target, &input) {
            Ok(()) => {
                // ヒューリスティック: 将来のスプライス用に、実行された入力の
                // 一部を残しておきます。カバレッジフィードバックがない以上、
                // これが利用可能な唯一の「新規性」シグナルです。
                if corpus.len() < 1024 && i % 16 == 0 {
                    let h = fnv_hash(&input);
                    if persisted_hashes.insert(h) {
                        if let Some(dir) = &cfg.corpus_dir {
                            let _ = save_input(dir, &input, h);
                        }
                        corpus.push(input);
                    }
                }
            }
            Err(message) => {
                if cfg.dedup_by_message && !seen_crash_messages.insert(message.clone()) {
                    continue; // 重複 crash なので無視します
                } else if !cfg.dedup_by_message {
                    // それでも、この実行中に同じ crash を何度も再保存
                    // するのは避けたいので、いずれにせよメッセージで追跡します。
                    seen_crash_messages.insert(message.clone());
                }
                let minimized = minimize(&input, &mut target, cfg.minimize_steps);
                let failure = Failure {
                    input: minimized,
                    message: message.clone(),
                    iteration: i + 1,
                };
                if let Some(dir) = &cfg.crash_dir {
                    let _ = save_crash(dir, &failure);
                }
                failures.push(failure);
                if !cfg.continue_after_crash {
                    break;
                }
            }
        }
    }

    FuzzReport {
        iterations: failures
            .last()
            .map(|f| f.iteration)
            .unwrap_or(cfg.iterations),
        seed: cfg.seed,
        failures,
    }
}

fn invoke<F: FnMut(&[u8])>(target: &mut F, input: &[u8]) -> Result<(), String> {
    match panic::catch_unwind(AssertUnwindSafe(|| target(input))) {
        Ok(()) => Ok(()),
        Err(payload) => Err(extract_panic_message(&payload)),
    }
}

fn extract_panic_message(payload: &Box<dyn Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

fn minimize<F: FnMut(&[u8])>(crash: &[u8], target: &mut F, max_steps: usize) -> Vec<u8> {
    let mut current = crash.to_vec();
    let mut steps = 0;
    loop {
        if current.is_empty() {
            return current;
        }
        let mut shrunk = false;
        let mut i = 0;
        while i < current.len() {
            if steps >= max_steps {
                return current;
            }
            steps += 1;
            let mut candidate = current.clone();
            candidate.remove(i);
            if invoke(target, &candidate).is_err() {
                current = candidate;
                shrunk = true;
                continue;
            }
            i += 1;
        }
        if !shrunk {
            break;
        }
    }
    for i in 0..current.len() {
        if steps >= max_steps {
            return current;
        }
        steps += 1;
        if current[i] == 0 {
            continue;
        }
        let mut candidate = current.clone();
        candidate[i] = 0;
        if invoke(target, &candidate).is_err() {
            current = candidate;
        }
    }
    current
}

fn mutate<R: Rng + ?Sized>(
    rng: &mut R,
    input: &mut Vec<u8>,
    corpus: &[Vec<u8>],
    dictionary: &[Vec<u8>],
    max_len: usize,
) {
    let has_dict = !dictionary.is_empty();
    // 辞書が空でない場合、辞書スプライスに少しだけバイアスを掛けます。
    let strategy_count = if has_dict { 9 } else { 7 };
    for _ in 0..3 {
        let choice = rng.gen_range_u64(0, strategy_count);
        let did = match choice {
            0 if !input.is_empty() => {
                let i = rng.gen_range_usize(0, input.len());
                let bit = rng.gen_range_u64(0, 8) as u8;
                input[i] ^= 1 << bit;
                true
            }
            1 if !input.is_empty() => {
                let i = rng.gen_range_usize(0, input.len());
                input[i] = rng.next_u64() as u8;
                true
            }
            2 if !input.is_empty() => {
                let i = rng.gen_range_usize(0, input.len());
                const INTERESTING: [u8; 7] = [0x00, 0x01, 0x10, 0x7f, 0x80, 0xfe, 0xff];
                let idx = rng.gen_range_usize(0, INTERESTING.len());
                input[i] = INTERESTING[idx];
                true
            }
            3 if input.len() < max_len => {
                let i = rng.gen_range_usize(0, input.len() + 1);
                input.insert(i, rng.next_u64() as u8);
                true
            }
            4 if !input.is_empty() => {
                let i = rng.gen_range_usize(0, input.len());
                input.remove(i);
                true
            }
            5 if !corpus.is_empty() && !input.is_empty() => {
                let idx = rng.gen_range_usize(0, corpus.len());
                let other = corpus[idx].clone();
                if other.is_empty() {
                    false
                } else {
                    let cut_self = rng.gen_range_usize(0, input.len() + 1);
                    let cut_other = rng.gen_range_usize(0, other.len());
                    let mut new = input[..cut_self].to_vec();
                    new.extend_from_slice(&other[cut_other..]);
                    new.truncate(max_len);
                    *input = new;
                    true
                }
            }
            6 if !input.is_empty() && input.len() > 1 => {
                let i = rng.gen_range_usize(0, input.len());
                let j = rng.gen_range_usize(0, input.len());
                input.swap(i, j);
                true
            }
            // 辞書 mutation: 挿入と上書きです。
            7 if has_dict && input.len() < max_len => {
                let dict_idx = rng.gen_range_usize(0, dictionary.len());
                let entry = &dictionary[dict_idx];
                let pos = rng.gen_range_usize(0, input.len() + 1);
                let mut new = input[..pos].to_vec();
                new.extend_from_slice(entry);
                new.extend_from_slice(&input[pos..]);
                new.truncate(max_len);
                *input = new;
                true
            }
            8 if has_dict && !input.is_empty() => {
                let dict_idx = rng.gen_range_usize(0, dictionary.len());
                let entry = &dictionary[dict_idx];
                if entry.is_empty() {
                    false
                } else {
                    let pos = rng.gen_range_usize(0, input.len());
                    let n = entry.len().min(input.len() - pos);
                    input[pos..pos + n].copy_from_slice(&entry[..n]);
                    true
                }
            }
            _ => false,
        };
        if did {
            return;
        }
    }
    if input.len() < max_len {
        input.push(rng.next_u64() as u8);
    }
}

// --- 永続化ヘルパー -----------------------------------------------

fn load_corpus_dir(dir: &std::path::Path) -> Vec<Vec<u8>> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<Vec<u8>> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Ok(bytes) = fs::read(&path) {
            out.push(bytes);
        }
    }
    out
}

fn save_input(dir: &std::path::Path, input: &[u8], hash: u64) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let path = dir.join(format!("input_{hash:016x}.bin"));
    if !path.exists() {
        fs::write(&path, input)?;
    }
    Ok(())
}

fn save_crash(dir: &std::path::Path, failure: &Failure) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let h = fnv_hash(failure.message.as_bytes());
    let path = dir.join(format!("crash_{h:016x}.bin"));
    if !path.exists() {
        fs::write(&path, &failure.input)?;
        let meta = dir.join(format!("crash_{h:016x}.txt"));
        let msg = format!(
            "iteration: {}\nbytes_len: {}\nmessage:\n{}\n",
            failure.iteration,
            failure.input.len(),
            failure.message
        );
        fs::write(&meta, msg)?;
    }
    Ok(())
}

fn fnv_hash(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(seed: u64, iters: usize) -> FuzzConfig {
        FuzzConfig {
            iterations: iters,
            max_input_len: 64,
            seed,
            initial_corpus: vec![b"hello world".to_vec()],
            minimize_steps: 200,
            silence_panic_hook: false,
            ..FuzzConfig::default()
        }
    }

    #[test]
    fn finds_simple_crash_and_minimizes_it() {
        let report = fuzz(cfg(0xDEAD_BEEF, 50_000), |data: &[u8]| {
            if data.contains(&0xCC) {
                panic!("trigger");
            }
        });
        let failure = report.failure().expect("should have found a crash");
        assert!(failure.input.contains(&0xCC));
        assert_eq!(failure.input, vec![0xCC]);
        assert!(failure.message.contains("trigger"));
    }

    #[test]
    fn no_crash_means_no_failure() {
        let report = fuzz(cfg(1, 200), |_data: &[u8]| {});
        assert!(report.failure().is_none());
        assert_eq!(report.iterations, 200);
    }

    #[test]
    fn run_is_reproducible_for_same_seed() {
        let a = fuzz(cfg(0xABC, 5_000), |data: &[u8]| {
            if data.len() >= 2 && data[0] == b'!' && data[1] == b'?' {
                panic!("match");
            }
        });
        let b = fuzz(cfg(0xABC, 5_000), |data: &[u8]| {
            if data.len() >= 2 && data[0] == b'!' && data[1] == b'?' {
                panic!("match");
            }
        });
        match (a.failure(), b.failure()) {
            (Some(fa), Some(fb)) => {
                assert_eq!(fa.iteration, fb.iteration);
                assert_eq!(fa.input, fb.input);
            }
            (None, None) => {}
            _ => panic!("non-deterministic outcome"),
        }
    }

    #[test]
    fn dictionary_helps_find_multi_byte_signature() {
        // 辞書がないと、4 バイトのマジックは 5 万イテレーションでは統計的に
        // 手の届かない範囲です。辞書があれば、最初の数百回で発見できます。
        let cfg = FuzzConfig {
            iterations: 50_000,
            max_input_len: 32,
            seed: 1,
            initial_corpus: vec![b"x".to_vec()],
            minimize_steps: 100,
            silence_panic_hook: false,
            dictionary: vec![b"MAGIC".to_vec()],
            ..FuzzConfig::default()
        };
        let report = fuzz(cfg, |data: &[u8]| {
            if data.windows(5).any(|w| w == b"MAGIC") {
                panic!("found magic");
            }
        });
        assert!(
            report.failure().is_some(),
            "dictionary mutation should find MAGIC"
        );
    }

    #[test]
    fn continue_after_crash_collects_multiple_unique_failures() {
        let cfg = FuzzConfig {
            iterations: 20_000,
            max_input_len: 32,
            seed: 7,
            initial_corpus: vec![b"hello".to_vec()],
            minimize_steps: 50,
            silence_panic_hook: false,
            continue_after_crash: true,
            dedup_by_message: true,
            ..FuzzConfig::default()
        };
        let report = fuzz(cfg, |data: &[u8]| {
            if data.contains(&0xAA) {
                panic!("alpha");
            }
            if data.contains(&0xBB) {
                panic!("beta");
            }
        });
        // alpha と beta の両方の panic を、別個の failure として発見できるはずです。
        let msgs: Vec<&str> = report.failures.iter().map(|f| f.message.as_str()).collect();
        assert!(msgs.iter().any(|m| m.contains("alpha")));
        assert!(msgs.iter().any(|m| m.contains("beta")));
    }

    #[test]
    fn dedup_keeps_only_unique_crash_messages() {
        let cfg = FuzzConfig {
            iterations: 5_000,
            max_input_len: 32,
            seed: 11,
            initial_corpus: vec![b"hi".to_vec()],
            minimize_steps: 50,
            silence_panic_hook: false,
            continue_after_crash: true,
            dedup_by_message: true,
            ..FuzzConfig::default()
        };
        let report = fuzz(cfg, |data: &[u8]| {
            if data.contains(&0xCC) {
                panic!("same message every time");
            }
        });
        // 何度トリガーされても、ユニークな crash は 1 つだけです。
        assert_eq!(report.failures.len(), 1);
    }

    #[test]
    fn corpus_and_crash_persistence_roundtrip() {
        let temp = std::env::temp_dir().join(format!("propcheck-fuzz-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp);
        let corpus_dir = temp.join("corpus");
        let crash_dir = temp.join("crashes");

        // 1 回目の実行: crash を発見し、それを永続化させます。
        let cfg1 = FuzzConfig {
            iterations: 5_000,
            max_input_len: 16,
            seed: 0xCAFE,
            initial_corpus: vec![b"x".to_vec()],
            minimize_steps: 50,
            silence_panic_hook: false,
            corpus_dir: Some(corpus_dir.clone()),
            crash_dir: Some(crash_dir.clone()),
            ..FuzzConfig::default()
        };
        let r1 = fuzz(cfg1, |data: &[u8]| {
            if data.contains(&0xCC) {
                panic!("p");
            }
        });
        assert!(r1.failure().is_some());

        // crash ファイルが存在しているはずです。
        let crashes: Vec<_> = std::fs::read_dir(&crash_dir).unwrap().collect();
        assert!(!crashes.is_empty(), "no crash files written");

        // 2 回目の実行: 明示的な initial_corpus なしでディスクから corpus を読み込みます。
        let cfg2 = FuzzConfig {
            iterations: 1_000,
            max_input_len: 16,
            seed: 0xBEEF,
            initial_corpus: Vec::new(),
            minimize_steps: 50,
            silence_panic_hook: false,
            corpus_dir: Some(corpus_dir.clone()),
            crash_dir: None,
            ..FuzzConfig::default()
        };
        let r2 = fuzz(cfg2, |_data: &[u8]| { /* 無害 */ });
        assert!(r2.failure().is_none());

        std::fs::remove_dir_all(&temp).unwrap();
    }
}
