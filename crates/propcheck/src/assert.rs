//! assertion マクロと、それらが発行する構造化された panic payload です。
//!
//! ランナーはこれらの特定の型の `panic_any` payload を捕捉し、デフォルトの
//! "panicked: …" 文字列の代わりに、詳細な失敗メッセージ、discard、skip を
//! 報告します。
//!
//! 3つのカテゴリが区別されます。
//!
//! - [`PropAssertFailure`] — プロパティの invariant が違反されました。これは
//!   実際のバグです。assertion メッセージと、周囲に `prop_with_context!`
//!   スタックがあればそれを伴う失敗として報告されます。
//! - [`PropDiscard`] — 入力が前提条件を満たしませんでした。discard として
//!   報告されます。ランナーはこれらを `Config::max_discards` にカウントします。
//!   なぜなら、過剰な場合は generator が有用な入力を生成していないことを
//!   意味するからです。
//! - [`PropSkip`] — 環境が準備できていない（環境変数がない、IO 失敗、機能が
//!   有効化されていない）ためにテストを実行できませんでした。skip として
//!   報告されます。`Config::max_skips` に別途カウントされ、不安定な環境を
//!   generator の不良に偽装させません。

use std::cell::RefCell;

/// `prop_assert!`、`prop_assert_eq!`、`prop_assert_ne!`、`prop_assert_matches!`
/// が発行する panic payload 型です。ランナーがメッセージを表示するために
/// 捕捉します。
#[derive(Debug, Clone)]
pub struct PropAssertFailure {
    /// ファイル/行番号と、アクティブな `prop_with_context!` スタックを
    /// 含む、フォーマット済みの失敗メッセージです。
    pub message: String,
}

/// `prop_assume!` が発行する panic payload 型です。ランナーが現在の
/// ケースを失敗させる代わりに discard するために捕捉します。
#[derive(Debug, Clone, Copy)]
pub struct PropDiscard;

/// `prop_skip!` が発行する panic payload 型です。discard と似ていますが、
/// （入力ではなく）*環境* がテストの実行を妨げる場合に使用されます。discard
/// とは別途カウントされるため、ランナーは壊れた環境とノイズの多い
/// generator を区別できます。
#[derive(Debug, Clone)]
pub struct PropSkip {
    /// テストがスキップされた理由です。
    pub message: String,
}

// --- with_context! サポート -------------------------------------------

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

/// `body` の実行中、コンテキストフレームをプッシュします。`body` 内で失敗した
/// 任意の assertion は、報告されるメッセージにフォーマットされたコンテキスト
/// 文字列を含めます。
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
        let __result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| $body));
        $crate::__pop_context();
        match __result {
            ::std::result::Result::Ok(v) => v,
            ::std::result::Result::Err(payload) => ::std::panic::resume_unwind(payload),
        }
    }};
}

// --- prop_assert! ファミリ --------------------------------------------

/// プロパティ本体内で真偽条件が成立することを assert します。失敗時には、
/// ランナーが捕捉して報告する [`PropAssertFailure`] payload で panic
/// します。
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

/// 2つの値が等しいことを assert します。失敗時には、`assert_eq!` と同様に
/// 両辺が失敗メッセージに含まれます。
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

/// 2つの値が等しくないことを assert します。
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

/// 2つの浮動小数点数値が互いに `epsilon` の範囲内であることを assert します。
/// 完全な等価性が不適切な場合（丸め誤差、超越関数など）に、`prop_assert_eq!`
/// の代わりに使用してください。
///
/// 両辺は `Sub`、`Output: Into<f64>` スタイルの絶対値計算をサポートする必要が
/// あります。実用上、このマクロは `-` と比較可能な値を返す `.abs()` を持つ
/// 任意の `T` で動作します。
///
/// ```ignore
/// prop_assert_close!(my_sin(x), x.sin(), epsilon = 1e-9);
/// ```
#[macro_export]
macro_rules! prop_assert_close {
    ($left:expr, $right:expr, epsilon = $eps:expr $(,)?) => {{
        let __left = $left;
        let __right = $right;
        let __eps = $eps;
        let __diff = (__left - __right).abs();
        if !(__diff <= __eps) {
            ::std::panic::panic_any($crate::PropAssertFailure {
                message: ::std::format!(
                    "prop_assert_close! failed at {}:{}{}\n  left:    {:?}\n  right:   {:?}\n  |diff|:  {:?}\n  epsilon: {:?}",
                    ::std::file!(),
                    ::std::line!(),
                    $crate::__current_context(),
                    __left,
                    __right,
                    __diff,
                    __eps
                ),
            });
        }
    }};
}

/// 値がパターンにマッチすることを assert します。`std::matches!` と同様に
/// オプションの `if` ガードをサポートします。
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

/// 条件が false の場合に現在のケースを discard します。ランナーは新しい
/// 入力を生成して再試行します。連続して discard されるケースが多すぎる
/// 場合、実行は「ノイズの多い generator」診断とともに中止されます。
///
/// これは *入力* の前提条件（「リストがソートされている場合のみ実行する」）
/// に使用してください。*環境* の前提条件（「$FOO が設定されている場合のみ
/// 実行する」）には [`prop_skip!`] を推奨します。
#[macro_export]
macro_rules! prop_assume {
    ($cond:expr $(,)?) => {
        if !($cond) {
            ::std::panic::panic_any($crate::PropDiscard);
        }
    };
}

/// *環境* が条件を満たせない（設定がない、オプション機能が無効、一時的な
/// IO 失敗）ために現在のケースをスキップします。`prop_assume!` の discard
/// とは別途カウントされます。
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
