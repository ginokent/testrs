//! Assertion macros and the structured panic payloads they emit.
//!
//! The runner catches `panic_any` payloads of these specific types and
//! reports rich failure messages, discards, or skips instead of the
//! default "panicked: …" string.
//!
//! Three categories are distinguished:
//!
//! - [`PropAssertFailure`] — the property's invariant was violated. This
//!   is a real bug. Reported as a failure with the assertion message,
//!   plus the surrounding `prop_with_context!` stack if any.
//! - [`PropDiscard`] — the input didn't satisfy a precondition. Reported
//!   as a discard; the runner counts these against `Config::max_discards`
//!   because too many means your generator isn't producing useful inputs.
//! - [`PropSkip`] — the test couldn't run because the environment isn't
//!   ready (missing env var, IO failure, feature not enabled). Reported
//!   as a skip; counted against `Config::max_skips` separately so a flaky
//!   env doesn't masquerade as a bad generator.

use std::cell::RefCell;

/// Panic payload type emitted by `prop_assert!`, `prop_assert_eq!`,
/// `prop_assert_ne!`, and `prop_assert_matches!`. Caught by the runner to
/// surface the message.
#[derive(Debug, Clone)]
pub struct PropAssertFailure {
    /// Pre-formatted failure message including file/line and any active
    /// `prop_with_context!` stack.
    pub message: String,
}

/// Panic payload type emitted by `prop_assume!`. Caught by the runner to
/// discard the current case rather than fail it.
#[derive(Debug, Clone, Copy)]
pub struct PropDiscard;

/// Panic payload type emitted by `prop_skip!`. Like discard but used
/// when the *environment* (not the input) prevents the test from running.
/// Counted separately from discards so the runner can tell a broken
/// environment from a noisy generator.
#[derive(Debug, Clone)]
pub struct PropSkip {
    /// Reason the test was skipped.
    pub message: String,
}

// --- with_context! support --------------------------------------------

thread_local! {
    static CONTEXT_STACK: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

#[doc(hidden)]
pub fn __push_context(s: String) {
    CONTEXT_STACK.with(|c| c.borrow_mut().push(s));
}

#[doc(hidden)]
pub fn __pop_context() {
    CONTEXT_STACK.with(|c| {
        c.borrow_mut().pop();
    });
}

#[doc(hidden)]
pub fn __current_context() -> String {
    CONTEXT_STACK.with(|c| {
        let v = c.borrow();
        if v.is_empty() {
            String::new()
        } else {
            format!(" [context: {}]", v.join(" / "))
        }
    })
}

/// Pushes a context frame for the duration of `body`. Any assertion that
/// fails inside `body` includes the formatted context string in its
/// reported message.
///
/// ```ignore
/// run("complex flow", |state: &State| {
///     prop_with_context!(format!("step 1, state={state:?}"), {
///         prop_assert!(state.precheck());
///     });
///     prop_with_context!("step 2", {
///         prop_assert_eq!(state.advance(), Ok(()));
///     });
///     true
/// });
/// ```
#[macro_export]
macro_rules! prop_with_context {
    ($ctx:expr, $body:block) => {{
        $crate::__push_context(::std::format!("{}", $ctx));
        let __result =
            ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| $body));
        $crate::__pop_context();
        match __result {
            ::std::result::Result::Ok(v) => v,
            ::std::result::Result::Err(payload) => ::std::panic::resume_unwind(payload),
        }
    }};
}

// --- prop_assert! family ----------------------------------------------

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
                message: ::std::format!(
                    "prop_assert!({}) failed at {}:{}{}",
                    ::std::stringify!($cond),
                    ::std::file!(),
                    ::std::line!(),
                    $crate::__current_context()
                ),
            });
        }
    };
    ($cond:expr, $($arg:tt)+) => {
        if !($cond) {
            ::std::panic::panic_any($crate::PropAssertFailure {
                message: ::std::format!(
                    "prop_assert! failed at {}:{}: {}{}",
                    ::std::file!(),
                    ::std::line!(),
                    ::std::format_args!($($arg)+),
                    $crate::__current_context()
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
                        message: ::std::format!(
                            "prop_assert_eq! failed at {}:{}{}\n  left:  {:?}\n  right: {:?}",
                            ::std::file!(),
                            ::std::line!(),
                            $crate::__current_context(),
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
                        message: ::std::format!(
                            "prop_assert_ne! failed at {}:{}{}\n  both: {:?}",
                            ::std::file!(),
                            ::std::line!(),
                            $crate::__current_context(),
                            left_val
                        ),
                    });
                }
            }
        }
    }};
}

/// Assert that a value matches a pattern. Supports optional `if` guards
/// just like `std::matches!`.
///
/// ```ignore
/// prop_assert_matches!(result, Ok(Token::Number(n)) if n > 0);
/// ```
#[macro_export]
macro_rules! prop_assert_matches {
    ($expr:expr, $pat:pat $(,)?) => {
        match $expr {
            $pat => {}
            ref __other => {
                ::std::panic::panic_any($crate::PropAssertFailure {
                    message: ::std::format!(
                        "prop_assert_matches! failed at {}:{}{}\n  pattern: {}\n  value:   {:?}",
                        ::std::file!(),
                        ::std::line!(),
                        $crate::__current_context(),
                        ::std::stringify!($pat),
                        __other
                    ),
                });
            }
        }
    };
    ($expr:expr, $pat:pat if $guard:expr $(,)?) => {
        match $expr {
            $pat if $guard => {}
            ref __other => {
                ::std::panic::panic_any($crate::PropAssertFailure {
                    message: ::std::format!(
                        "prop_assert_matches! failed at {}:{}{}\n  pattern: {} if {}\n  value:   {:?}",
                        ::std::file!(),
                        ::std::line!(),
                        $crate::__current_context(),
                        ::std::stringify!($pat),
                        ::std::stringify!($guard),
                        __other
                    ),
                });
            }
        }
    };
}

// --- prop_assume! / prop_skip! ----------------------------------------

/// Discard the current case if the condition is false. The runner will
/// generate a fresh input and try again. If too many cases are discarded
/// in a row, the run is aborted with a "noisy generator" diagnostic.
///
/// Use this for *input* preconditions ("only run when the list is
/// sorted"). For *environment* preconditions ("only run when $FOO is set"),
/// prefer [`prop_skip!`].
#[macro_export]
macro_rules! prop_assume {
    ($cond:expr $(,)?) => {
        if !($cond) {
            ::std::panic::panic_any($crate::PropDiscard);
        }
    };
}

/// Skip the current case because the *environment* can't satisfy it
/// (missing config, optional feature off, transient IO failure). Counted
/// separately from `prop_assume!` discards.
///
/// ```ignore
/// let db_url = match std::env::var("DB_URL") {
///     Ok(v) => v,
///     Err(_) => prop_skip!("DB_URL not set"),
/// };
/// ```
#[macro_export]
macro_rules! prop_skip {
    ($($arg:tt)+) => {
        ::std::panic::panic_any($crate::PropSkip {
            message: ::std::format!($($arg)+),
        })
    };
}
