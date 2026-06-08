//! 機械可読出力 (JSON / CSV) とエスケープの検証。
//!
//! `to_json` / `to_csv_record` は時間に依存しないため、`BenchResult` を手構築
//! して決定的に検証する (計測本体は非決定的なので lib.rs 側の単体テストで扱う)。
//! エスケープの境界値は parser を持たない以上ラウンドトリップ検証できないため、
//! 代表ケースを個別に確認する (PBT ではなく単体テストが適切な領域)。

use testrs_bench::{BenchResult, Statistics};

/// 計測を伴わずに決定的な BenchResult を構築する。
fn sample_result(name: &str, bytes_per_iter: Option<u64>) -> BenchResult {
    let samples = vec![10.0, 12.0, 11.0, 13.0];
    let stats = Statistics::from_samples(&samples).expect("non-empty");
    BenchResult {
        name: name.to_string(),
        samples,
        iters_per_sample: 8,
        total_iterations: 32,
        outliers_removed: 1,
        stats,
        bytes_per_iter,
    }
}

#[test]
fn json_is_an_object_with_expected_keys() {
    let j = sample_result("enc", Some(1024)).to_json();
    assert!(j.starts_with('{') && j.ends_with('}'), "not an object: {j}");
    for key in [
        "\"name\"",
        "\"median_ns\"",
        "\"cv\"",
        "\"bytes_per_iter\"",
        "\"throughput_ops\"",
        "\"throughput_bytes\"",
        "\"samples_ns\"",
    ] {
        assert!(j.contains(key), "missing key {key} in {j}");
    }
}

#[test]
fn json_escapes_special_characters_and_emits_null_without_meta() {
    let j = sample_result("a\"b\\c\n", None).to_json();
    // 生の特殊文字ではなくエスケープ列が現れる。
    assert!(j.contains("a\\\"b\\\\c\\n"), "name not escaped: {j}");
    // bytes 未設定のときは null。
    assert!(j.contains("\"bytes_per_iter\":null"));
    assert!(j.contains("\"throughput_bytes\":null"));
}

#[test]
fn csv_header_and_record_have_matching_field_count() {
    let header_fields = BenchResult::csv_header().split(',').count();
    // 特殊文字を含まない名前なので単純分割でフィールド数を数えられる。
    let record = sample_result("plain", Some(2048)).to_csv_record();
    assert_eq!(record.split(',').count(), header_fields);
}

#[test]
fn csv_quotes_field_with_comma_and_doubles_inner_quote() {
    let record = sample_result("a,b\"c", Some(64)).to_csv_record();
    assert!(record.starts_with("\"a,b\"\"c\","), "bad quoting: {record}");
}

#[test]
fn csv_leaves_byte_columns_empty_without_meta() {
    let record = sample_result("plain", None).to_csv_record();
    let fields: Vec<&str> = record.split(',').collect();
    assert_eq!(fields.len(), 17);
    assert_eq!(fields[14], "", "bytes_per_iter should be empty");
    assert_eq!(fields[16], "", "throughput_bytes should be empty");
    assert_ne!(fields[15], "", "throughput_ops is always present");
}
