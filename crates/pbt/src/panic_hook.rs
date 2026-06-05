//! グローバルな panic hook 用のスレッドセーフな RAII サイレンサーです。
//!
//! panic hook はプロセス全体で共有されるため、プロパティテストを並行して実行する
//! 複数のスレッドがそれぞれ独立に設定・解除を行ってはいけません。競合して元の hook
//! が失われてしまいます。このモジュールでは install/restore を参照カウント付きの
//! ガードでラップしています。最初のガードがサイレントな hook をインストールし
//! （元の hook を保存し）、最後に drop されるガードがそれを復元します。

use std::panic::{self, PanicHookInfo};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

static REFCOUNT: AtomicUsize = AtomicUsize::new(0);
#[allow(clippy::type_complexity)]
static SAVED: Mutex<Option<Box<dyn Fn(&PanicHookInfo<'_>) + Send + Sync + 'static>>> =
    Mutex::new(None);

/// 生存している間 panic 出力を抑止する RAII ガードです。
pub(crate) struct SilentPanicHook {
    _private: (),
}

impl SilentPanicHook {
    /// サイレントな hook をインストールします（外側のガードによってすでに
    /// インストールされている場合は参照カウントを増やします）。
    pub(crate) fn install() -> Self {
        if REFCOUNT.fetch_add(1, Ordering::SeqCst) == 0 {
            let prev = panic::take_hook();
            if let Ok(mut slot) = SAVED.lock() {
                *slot = Some(prev);
            }
            panic::set_hook(Box::new(|_info| {}));
        }
        SilentPanicHook { _private: () }
    }
}

impl Drop for SilentPanicHook {
    fn drop(&mut self) {
        if REFCOUNT.fetch_sub(1, Ordering::SeqCst) == 1 {
            if let Ok(mut slot) = SAVED.lock() {
                if let Some(prev) = slot.take() {
                    panic::set_hook(prev);
                }
            }
        }
    }
}
