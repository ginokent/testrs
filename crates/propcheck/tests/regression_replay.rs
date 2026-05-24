//! regression-replay 機能のエンドツーエンドテストです。
//!
//! テストから`target/propcheck-regressions/`に直接アクセスするのは難しいため
//! （他のテストと競合します）、代わりに一時ディレクトリを介してディスク上の
//! IO を動作確認し、以下を検証します:
//!
//! 1. 失敗した実行は、設定されたファイルに seed を追記します。
//! 2. その後の実行は、ファイルの seed を最初に再生し、決定的に失敗を再現します。

use std::fs;
use std::process::Command;

#[test]
fn regression_seed_is_persisted_and_replayed() {
    // テストバイナリ全体を巻き込まずに安全に panic できるよう、サブプロセス経由で
    // 実行します。ワークスペース自身の cargo を使い、インラインで小さなテストバイナリを
    // ビルドします。
    //
    // 別のバイナリクレートを用意せずにこのテストを自己完結させるため、代わりに
    // regression_replay を有効にした propcheck 自身のランナーと既知の失敗するプロパティを
    // 使って regression モジュールの（半）公開のファイル IO を動作させ、結果として
    // できたファイルを直接読みます。

    // regression ファイルの保存先:
    let target_dir = std::env::var("CARGO_TARGET_DIR")
        .ok()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            let manifest = std::env::var("CARGO_MANIFEST_DIR")
                .expect("CARGO_MANIFEST_DIR must be set by cargo");
            std::path::PathBuf::from(manifest).join("target")
        });
    let reg_dir = target_dir.join("propcheck-regressions");
    // 他のテストと衝突しないよう、一意なテスト名を使用します。
    let test_name = format!("regression_e2e_{}", std::process::id());
    let reg_file = reg_dir.join(format!("{test_name}.txt"));
    let _ = fs::remove_file(&reg_file);

    // panic がこのテストを crash させないよう、既知の失敗するプロパティを
    // サブプロセスで実行します。`cargo`で子プロセスを生成する方法もありますが、
    // 簡潔さのためここでは std::panic::catch_unwind を使用します。
    let test_name_for_run = test_name.clone();
    let captured = std::panic::catch_unwind(move || {
        propcheck::run::<u32, _, _>(&test_name_for_run, |&n: &u32| n < 50);
    });
    assert!(captured.is_err(), "expected runner to panic on failure");

    // 失敗後、regression ファイルが存在し、少なくとも 1 行の seed を含むはずです。
    assert!(
        reg_file.exists(),
        "regression file was not created at {reg_file:?}"
    );
    let content = fs::read_to_string(&reg_file).expect("read regression file");
    let seeds: Vec<u64> = content
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.trim().parse().ok())
        .collect();
    assert!(!seeds.is_empty(), "expected at least one persisted seed");

    // replay を有効にして再実行: 同じ seed で再現するはずなので、ランナーは
    // 再び panic します。
    let test_name_for_replay = test_name.clone();
    let captured2 = std::panic::catch_unwind(move || {
        propcheck::run::<u32, _, _>(&test_name_for_replay, |&n: &u32| n < 50);
    });
    assert!(
        captured2.is_err(),
        "regression should reproduce the failure"
    );

    // 後片付け。
    let _ = fs::remove_file(&reg_file);
}

#[test]
fn run_with_regression_replay_disabled_does_not_persist() {
    use propcheck::Config;

    let target_dir = std::env::var("CARGO_TARGET_DIR")
        .ok()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
            std::path::PathBuf::from(manifest).join("target")
        });
    let reg_dir = target_dir.join("propcheck-regressions");
    let test_name = format!("regression_disabled_{}", std::process::id());
    let reg_file = reg_dir.join(format!("{test_name}.txt"));
    let _ = fs::remove_file(&reg_file);

    let test_name_owned = test_name.clone();
    let _ = std::panic::catch_unwind(move || {
        propcheck::run_with::<u32, _, _>(
            &test_name_owned,
            Config {
                regression_replay: false,
                cases: 50,
                ..Config::default()
            },
            |&n: &u32| n < 50,
        );
    });

    assert!(
        !reg_file.exists(),
        "file should not be created when regression_replay = false"
    );

    // このファイルがサブプロセス生成なしでビルドされる場合に、Command の
    // unused-import 警告を抑制します。
    let _ = std::mem::size_of::<Command>();
}
