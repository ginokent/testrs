//! Per-case classification statistics and the [`classify!`] macro.
//!
//! The macro records a label for the current test case when its condition
//! is true. The runner clears the recorded labels before each case and
//! aggregates them across all cases into a [`Classifications`] report.

use std::cell::RefCell;
use std::collections::BTreeMap;

thread_local! {
    static CURRENT: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Aggregated label counts for a run.
#[derive(Debug, Default, Clone)]
pub struct Classifications {
    pub(crate) counts: BTreeMap<String, usize>,
    pub(crate) total: usize,
}

impl Classifications {
    /// Returns the number of cases that recorded each label.
    pub fn counts(&self) -> &BTreeMap<String, usize> {
        &self.counts
    }

    /// Total number of cases observed (including those that recorded no
    /// labels).
    pub fn total(&self) -> usize {
        self.total
    }

    /// `true` if no `classify!` calls occurred during the run.
    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// Formats the classifications as a percentage table sorted by label.
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

    /// Merges another report's counts into this one. Both `total` and per-
    /// label counts are summed.
    pub(crate) fn merge_case(&mut self, labels: Vec<String>) {
        self.total += 1;
        for label in labels {
            *self.counts.entry(label).or_insert(0) += 1;
        }
    }
}

/// Internal: called by the runner before each case to reset the buffer.
pub(crate) fn reset_current() {
    CURRENT.with(|c| c.borrow_mut().clear());
}

/// Internal: called by the runner after a case to harvest its labels.
pub(crate) fn take_current() -> Vec<String> {
    CURRENT.with(|c| std::mem::take(&mut *c.borrow_mut()))
}

/// Internal: invoked by the [`classify!`] macro. Records `label` once per
/// case even if the macro is hit multiple times with the same label.
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

/// Records `label` for the current test case when `cond` is true.
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
