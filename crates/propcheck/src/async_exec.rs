//! `std` のみで構築された、最小限の `block_on` executor です。
//!
//! 一般的なプロパティ本体が生成する `Future` を駆動するには十分です。
//! シングルタスクで、I/O reactor も work-stealing もありません。プロパティが
//! tokio 固有の機能（timer、ネットワークソケット、ファイル IO）を使う場合は、
//! 本格的なランタイムが必要になります。propcheck の executor は future が
//! 完了するまで現在のスレッドをブロックするだけです。
//!
//! これは、`#[propcheck]` 属性マクロが `async fn` プロパティをランタイム依存
//! を強制せずにラップできるように公開されています。直接呼び出すこともできます。
//!
//! ```
//! use propcheck::block_on;
//! let v: u32 = block_on(async { 1 + 2 });
//! assert_eq!(v, 3);
//! ```

use std::future::Future;
use std::pin::pin;
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Wake, Waker};

/// 現在のスレッド上で `future` を完了までポーリングします。
///
/// Wake は `Condvar` を用いた parking プリミティブで調整されます。future は
/// `std::pin::pin!` を介してスタックに pin 留めされます（安全）。
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut pinned = pin!(future);
    let parker = Arc::new(Parker::new());
    let waker: Waker = Waker::from(parker.clone());
    let mut cx = Context::from_waker(&waker);
    loop {
        match pinned.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => parker.park(),
        }
    }
}

struct Parker {
    state: Mutex<bool>,
    cv: Condvar,
}

impl Parker {
    fn new() -> Self {
        Self {
            state: Mutex::new(false),
            cv: Condvar::new(),
        }
    }

    fn park(&self) {
        let mut state = self.state.lock().expect("parker mutex poisoned");
        while !*state {
            state = self.cv.wait(state).expect("parker condvar wait failed");
        }
        *state = false;
    }

    fn unpark(&self) {
        let mut state = self.state.lock().expect("parker mutex poisoned");
        *state = true;
        self.cv.notify_one();
    }
}

impl Wake for Parker {
    fn wake(self: Arc<Self>) {
        self.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.unpark();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    #[test]
    fn block_on_immediate_value() {
        let v = block_on(async { 7u32 });
        assert_eq!(v, 7);
    }

    #[test]
    fn block_on_ready_after_yield() {
        struct YieldOnce(bool);
        impl Future for YieldOnce {
            type Output = u32;
            fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                if !self.0 {
                    self.0 = true;
                    cx.waker().wake_by_ref();
                    Poll::Pending
                } else {
                    Poll::Ready(42)
                }
            }
        }
        assert_eq!(block_on(YieldOnce(false)), 42);
    }
}
