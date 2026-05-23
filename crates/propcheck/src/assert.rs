//! Assertion macros and the structured panic payloads they emit.
//!
//! The runner catches `panic_any` payloads of these specific types and
//! reports rich failure messages or discards instead of the default
//! "panicked: …" string.

/// Panic payload type emitted by `prop_assert!`, `prop_assert_eq!`, and
/// `prop_assert_ne!`. Caught by the runner to surface the message.
#[derive(Debug, Clone)]
pub struct PropAssertFailure {
    /// Pre-formatted failure message including file/line.
    pub message: String,
}

/// Panic payload type emitted by `prop_assume!`. Caught by the runner to
/// discard the current case rather than fail it.
#[derive(Debug, Clone, Copy)]
pub struct PropDiscard;

/// Assert that a boolean condition holds within a property body. On
/// failure, panics with a [`PropAssertFailure`] payload that the runner
/// captures and reports.
///
/// ```ignore
/// prop_assert!(v.len() <= cap, "expected len <= {cap}, got {}", v.len());
/// ```
#[macro_export]
macro_rules! prop_assert {
    ($cond:expr $(,)?) => {
        if !($cond) {
            ::std::panic::panic_any($crate::PropAssertFailure {
                message: format!(
                    "prop_assert!({}) failed at {}:{}",
                    stringify!($cond),
                    file!(),
                    line!()
                ),
            });
        }
    };
    ($cond:expr, $($arg:tt)+) => {
        if !($cond) {
            ::std::panic::panic_any($crate::PropAssertFailure {
                message: format!(
                    "prop_assert! failed at {}:{}: {}",
                    file!(),
                    line!(),
                    format_args!($($arg)+)
                ),
            });
        }
    };
}

/// Assert that two values are equal. On failure, both sides are included
/// in the failure message, just like `assert_eq!`.
#[macro_export]
macro_rules! prop_assert_eq {
    ($left:expr, $right:expr $(,)?) => {{
        match (&($left), &($right)) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    ::std::panic::panic_any($crate::PropAssertFailure {
                        message: format!(
                            "prop_assert_eq! failed at {}:{}\n  left:  {:?}\n  right: {:?}",
                            file!(),
                            line!(),
                            left_val,
                            right_val
                        ),
                    });
                }
            }
        }
    }};
}

/// Assert that two values are not equal.
#[macro_export]
macro_rules! prop_assert_ne {
    ($left:expr, $right:expr $(,)?) => {{
        match (&($left), &($right)) {
            (left_val, right_val) => {
                if *left_val == *right_val {
                    ::std::panic::panic_any($crate::PropAssertFailure {
                        message: format!(
                            "prop_assert_ne! failed at {}:{}\n  both: {:?}",
                            file!(),
                            line!(),
                            left_val
                        ),
                    });
                }
            }
        }
    }};
}

/// Discard the current case if the condition is false. The runner will
/// generate a fresh input and try again. If too many cases are discarded
/// in a row, the run is aborted.
///
/// Useful for properties that only hold under preconditions:
///
/// ```ignore
/// run("binary_search finds existing", |v: &Vec<i32>| {
///     prop_assume!(!v.is_empty() && v.windows(2).all(|w| w[0] <= w[1]));
///     let target = v[v.len() / 2];
///     prop_assert!(v.binary_search(&target).is_ok());
///     true
/// });
/// ```
#[macro_export]
macro_rules! prop_assume {
    ($cond:expr $(,)?) => {
        if !($cond) {
            ::std::panic::panic_any($crate::PropDiscard);
        }
    };
}
