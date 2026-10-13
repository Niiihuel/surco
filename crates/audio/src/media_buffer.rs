// SPDX-License-Identifier: Apache-2.0
//
// Adapted from noFAYZ/zuno at commit 1fa9a9669e4a, src-tauri/src/lib.rs (`MediaBuffer`),
// licensed under Apache-2.0.
// Changed: a shared handle the caller appends to in order, instead of a chunk table filled by
// parallel range requests; the total length is optional and the body can be marked complete.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/**
 * A media body, which may still be arriving.
 *
 * The downloader appends bytes as they land while the decoder reads from the start, so the
 * first sample can play before the last byte arrives. A read past what has arrived waits for it
 * rather than being answered short.
 *
 * Clones share the same body.
 */
#[derive(Clone)]
pub struct MediaBuffer(Arc<Mutex<Inner>>);

struct Inner {
    bytes: Vec<u8>,
    total: Option<usize>,
    failed: bool,
}

impl MediaBuffer {
    /**
     * An empty body. `total` is its full length when the server declared one.
     *
     * Pass it whenever it is known. The demuxers use it to locate indexes kept at the tail of a
     * file, and without it the body is forward-only: no seeking backwards, and an MP4 whose
     * `moov` atom follows its audio does not open at all.
     */
    pub fn new(total: Option<usize>) -> Self {
        Self(Arc::new(Mutex::new(Inner { bytes: Vec::new(), total, failed: false })))
    }

    pub fn append(&self, bytes: &[u8]) {
        self.lock().bytes.extend_from_slice(bytes);
    }

    /// The whole body has arrived. Its length is now what was appended, whatever was declared.
    pub fn complete(&self) {
        let mut inner = self.lock();
        inner.total = Some(inner.bytes.len());
    }

    /// The download gave up. Readers get an end of stream instead of waiting out their timeout.
    pub fn fail(&self) {
        self.lock().failed = true;
    }

    pub fn is_failed(&self) -> bool {
        self.lock().failed
    }

    pub(crate) fn total(&self) -> Option<usize> {
        self.lock().total
    }

    /// Copies what has arrived from `position` on into `out`. `Some(0)` is the end of the body,
    /// `None` means those bytes are still on their way.
    pub(crate) fn read_at(&self, position: usize, out: &mut [u8]) -> Option<usize> {
        let inner = self.lock();
        if position < inner.bytes.len() {
            let end = (position + out.len()).min(inner.bytes.len());
            out[..end - position].copy_from_slice(&inner.bytes[position..end]);
            return Some(end - position);
        }
        /*
         * A failed download reports EOF rather than an error. The decoder then finishes the
         * frames it already has and the track ends early, which is a truncated song instead
         * of a dead sink that never empties and hangs the queue.
         */
        if inner.failed || inner.total.is_some_and(|total| position >= total) {
            return Some(0);
        }
        None
    }

    // The state is plain data that no panic can leave half-written, so a poisoned lock is still
    // safe to read; failing instead would turn a panic elsewhere into a dead deck.
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
