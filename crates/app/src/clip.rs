//! The short video of what plays, turning over without a sound where its cover was.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dnm_video::{Clip, Decoder, Frame};
use limn_egui::SurfaceFrame;

/// What the window and the thread that decodes share.
struct Shared {
    /// The newest frame the window has not shown yet.
    frame: Mutex<Option<SurfaceFrame>>,
    /// The side, in the window's own units, of the square the clip fills.
    side: AtomicU32,
    paused: AtomicBool,
    stopped: AtomicBool,
}

/// A clip being decoded over and over on a thread of its own. Dropping it ends the thread.
pub struct ClipPlayer {
    pub track_id: String,
    shared: Arc<Shared>,
}

impl ClipPlayer {
    /// `repaint` wakes the window when a frame is ready.
    pub fn start(track_id: String, clip: Arc<Clip>, side: f32, repaint: impl Fn() + Send + 'static) -> Self {
        let shared = Arc::new(Shared {
            frame: Mutex::new(None),
            side: AtomicU32::new(side.to_bits()),
            paused: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
        });
        let theirs = shared.clone();
        // A machine that cannot spare a thread goes on showing the cover.
        let _ = std::thread::Builder::new().name("clip".into()).spawn(move || turn(&clip, &theirs, &repaint));
        Self { track_id, shared }
    }

    /// The newest frame, once.
    pub fn take(&self) -> Option<SurfaceFrame> {
        self.shared.frame.lock().unwrap().take()
    }

    /// Follows the music: a paused song holds its picture, and the square may change size.
    pub fn follow(&self, playing: bool, side: f32) {
        self.shared.paused.store(!playing, Ordering::Relaxed);
        self.shared.side.store(side.to_bits(), Ordering::Relaxed);
    }
}

impl Drop for ClipPlayer {
    fn drop(&mut self) {
        self.shared.stopped.store(true, Ordering::Relaxed);
    }
}

/// Decodes the clip from its first picture to its last, at its own pace, again and again.
fn turn(clip: &Clip, shared: &Shared, repaint: &impl Fn()) {
    let stopped = || shared.stopped.load(Ordering::Relaxed);
    while !stopped() {
        // Each round starts from the picture that stands alone, on a fresh decoder.
        let Ok(mut decoder) = Decoder::new(&clip.parameters) else { return };
        let mut due = Instant::now();
        for sample in &clip.samples {
            while shared.paused.load(Ordering::Relaxed) && !stopped() {
                std::thread::sleep(Duration::from_millis(50));
                due = Instant::now();
            }
            if stopped() {
                return;
            }
            // A picture the decoder cannot take is skipped; the next one that stands alone mends it.
            if let Ok(Some(frame)) = decoder.decode(&sample.data) {
                let side = f32::from_bits(shared.side.load(Ordering::Relaxed));
                if let Some(frame) = squared(&frame, side) {
                    *shared.frame.lock().unwrap() = Some(frame);
                    repaint();
                }
            }
            due += Duration::from_secs_f64(sample.seconds);
            std::thread::sleep(due.saturating_duration_since(Instant::now()));
        }
    }
}

/// The middle square of a frame, said to be as dense as it takes to fill `side`: the
/// backend draws pixels at their own density, so that is what makes them cover the square.
fn squared(frame: &Frame, side: f32) -> Option<SurfaceFrame> {
    let edge = frame.width.min(frame.height);
    let (left, top) = ((frame.width - edge) / 2, (frame.height - edge) / 2);
    let mut rgba = Vec::with_capacity(edge * edge * 4);
    for row in top..top + edge {
        let start = (row * frame.width + left) * 4;
        rgba.extend_from_slice(frame.rgba.get(start..start + edge * 4)?);
    }
    SurfaceFrame::from_rgba([edge, edge], &rgba, edge as f32 / side.max(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_frame_gives_its_middle_square_at_the_density_that_fills_the_side() {
        // Four by two, each pixel carrying its column in red.
        let rgba: Vec<u8> = (0..2).flat_map(|_| (0..4u8).flat_map(|column| [column, 0, 0, 255])).collect();
        let frame = squared(&Frame { width: 4, height: 2, rgba }, 100.0).unwrap();
        assert_eq!(frame.size(), [2, 2]);
        assert!(squared(&Frame { width: 4, height: 2, rgba: vec![0; 3] }, 100.0).is_none(), "a frame with rows missing");
    }
}
