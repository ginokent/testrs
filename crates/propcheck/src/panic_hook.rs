//! Thread-safe RAII silencer for the global panic hook.
//!
//! The panic hook is process-wide, so multiple threads running property
//! tests concurrently must not each set then unset it independently —
//! they'd race and lose the original. This module wraps install/restore in
//! a reference-counted guard: the first guard installs a silent hook (and
//! saves the previous one), the last guard to drop restores it.

use std::panic::{self, PanicHookInfo};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

static REFCOUNT: AtomicUsize = AtomicUsize::new(0);
#[allow(clippy::type_complexity)]
static SAVED: Mutex<Option<Box<dyn Fn(&PanicHookInfo<'_>) + Send + Sync + 'static>>> =
    Mutex::new(None);

/// RAII guard that silences panic output for as long as it is alive.
pub(crate) struct SilentPanicHook {
    _private: (),
}

impl SilentPanicHook {
    /// Installs a silent hook (or bumps the reference count if one is
    /// already installed by an outer guard).
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
