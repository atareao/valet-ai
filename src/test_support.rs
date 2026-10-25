//! Test-only helpers shared by the crate's unit-test modules.
//!
//! [`capture_traces`] installs a **single global** `tracing` subscriber whose
//! `MakeWriter` routes each record to the **emitting thread**'s buffer. A global
//! subscriber is required because `tracing` caches per-callsite interest the
//! first time a callsite is hit: a thread-scoped `set_default` subscriber would
//! never see callsites already visited without one. Routing per thread keeps
//! tests running in parallel isolated — each thread captures only its own
//! records.

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

thread_local! {
    /// Buffer of the current thread that the global subscriber dumps records
    /// into while a capture guard is active.
    static ACTIVE_CAPTURE: RefCell<Option<Arc<Mutex<Vec<u8>>>>> = const { RefCell::new(None) };
}

/// `MakeWriter` that creates a writer routing every record to the buffer of the
/// **emitting** thread.
struct ThreadWriters;

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for ThreadWriters {
    type Writer = ThreadWriter;

    fn make_writer(&'a self) -> Self::Writer {
        ThreadWriter
    }
}

/// A record's writer: dumps it into the emitting thread's buffer, if active.
struct ThreadWriter;

impl std::io::Write for ThreadWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        ACTIVE_CAPTURE.with(|cell| {
            if let Some(shared) = cell.borrow().as_ref() {
                shared.lock().unwrap().extend_from_slice(buf);
            }
        });
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Capture guard: activates the current thread's buffer and deactivates it on
/// drop. Exposes the records captured by the thread.
pub struct CaptureGuard {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl CaptureGuard {
    /// The records captured so far, as text.
    pub fn contents(&self) -> String {
        String::from_utf8_lossy(&self.buffer.lock().unwrap()).into_owned()
    }
}

impl Drop for CaptureGuard {
    fn drop(&mut self) {
        ACTIVE_CAPTURE.with(|cell| *cell.borrow_mut() = None);
    }
}

/// Activate trace capture for the current thread and return its guard.
///
/// The subscriber is **global** and installed a single time: `set_global_default`
/// rebuilds `tracing`'s interest cache, whereas a thread-scoped `set_default`
/// subscriber does not, so a callsite first visited without a subscriber would
/// be cached as "never" and later captures would come out empty at random. The
/// `MakeWriter` routes each record to the emitting thread's buffer, so the
/// capture stays deterministic even with tests running in parallel.
pub fn capture_traces() -> CaptureGuard {
    use std::sync::Once;

    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let subscriber = tracing_subscriber::fmt()
            .with_writer(ThreadWriters)
            .with_ansi(false)
            .with_env_filter(tracing_subscriber::EnvFilter::new("valet=trace"))
            .finish();
        // If another global subscriber already existed the capture would not
        // apply; in this crate's test binary there is none.
        let _ = tracing::subscriber::set_global_default(subscriber);
    });

    let buffer = Arc::new(Mutex::new(Vec::new()));
    ACTIVE_CAPTURE.with(|cell| *cell.borrow_mut() = Some(buffer.clone()));
    CaptureGuard { buffer }
}
