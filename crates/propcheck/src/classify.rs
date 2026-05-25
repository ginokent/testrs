//! ケースごとの classification 統計と [`classify!`](macro@crate::classify) マクロを提供します。
//!
//! このマクロは、条件が真である場合に現在のテストケースに対してラベルを記録します。
//! runner は各ケースの前に記録済みのラベルをクリアし、全ケースにわたって集計して
//! [`Classifications`] レポートにまとめます。

use std::cell::RefCell;
use std::collections::BTreeMap;

thread_local! {
    static CURRENT: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// 1 回の実行における集計されたラベルのカウントです。
#[derive(Debug, Default, Clone)]
pub struct Classifications {
    pub(crate) counts: BTreeMap<String, usize>,
    pub(crate) total: usize,
}

impl Classifications {
    /// 各ラベルを記録したケースの数を返します。
    pub fn counts(&self) -> &BTreeMap<String, usize> {
        &self.counts
    }

    /// 観測されたケースの総数（ラベルを記録しなかったケースを含む）です。
    pub fn total(&self) -> usize {
        self.total
    }

    /// 実行中に `classify!` の呼び出しが一度も発生しなかった場合に `true` を返します。
    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// classification をラベル順にソートした割合の表として整形します。
    pub fn render(&self) -> String {
        if self.counts.is_empty() {
            return String::new();
        }
        let mut out = String::new();
        for (label, count) in &self.counts {
            let pct = if self.total == 0 {
                0.0
            } else {
                100.0 * (*count as f64) / (self.total as f64)
            };
            out.push_str(&format!(
                "    {pct:5.1}%  {label}  ({count}/{total})\n",
                total = self.total
            ));
        }
        out
    }

    /// 別のレポートのカウントをこのレポートにマージします。`total` とラベル
    /// ごとのカウントの両方が合算されます。
    pub(crate) fn merge_case(&mut self, labels: Vec<String>) {
        self.total += 1;
        for label in labels {
            *self.counts.entry(label).or_insert(0) += 1;
        }
    }
}

/// 内部用: 各ケースの前に runner から呼び出され、バッファをリセットします。
pub(crate) fn reset_current() {
    CURRENT.with(|c| c.borrow_mut().clear());
}

/// 内部用: ケース実行後に runner から呼び出され、そのケースのラベルを回収します。
pub(crate) fn take_current() -> Vec<String> {
    CURRENT.with(|c| std::mem::take(&mut *c.borrow_mut()))
}

/// 内部用: [`classify!`] マクロから呼び出されます。同じラベルでマクロが複数回
/// ヒットしても、`label` はケースごとに 1 回だけ記録されます。
#[doc(hidden)]
pub fn record_label(label: impl Into<String>) {
    let label = label.into();
    CURRENT.with(|c| {
        let mut buf = c.borrow_mut();
        if !buf.iter().any(|l| l == &label) {
            buf.push(label);
        }
    });
}

/// `cond` が真のときに、現在のテストケースに対して `label` を記録します。
///
/// ```ignore
/// use propcheck::{run, classify};
///
/// run("addition", |&(a, b): &(i32, i32)| {
///     classify!(a == 0 || b == 0, "has-zero");
///     classify!(a < 0 || b < 0, "has-negative");
///     a.wrapping_add(b) == b.wrapping_add(a)
/// });
/// // Output includes:
/// //   classifications:
/// //      35.0%  has-negative  (35/100)
/// //      18.0%  has-zero  (18/100)
/// ```
#[macro_export]
macro_rules! classify {
    ($cond:expr, $label:expr) => {
        if $cond {
            $crate::classify::record_label($label);
        }
    };
}
