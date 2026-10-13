//! The video of what plays, where its cover was: a stretch of it turning over without a
//! sound, or the whole of it keeping time with the song.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use surco_video::{Clip, Decoder, Film, Frame, Sample};
use limn_egui::SurfaceFrame;

use crate::backend::Footage;

/// Which of a track's footage is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Showing {
    /// A stretch of it, over and over.
    Loop,
    /// The whole of it in time with what plays, at the panel's size or the window's.
    Small,
    Large,
}

/// How a frame is laid in its place, in the window's own units: the middle square of it
/// on a square of this side, or all of it at this width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fit {
    Square(f32),
    Whole(f32),
}

/// Where the song is, as the window last knew it.
struct Clock {
    position: f64,
    at: Instant,
    playing: bool,
}

/// What the window and the thread that decodes share.
struct Shared {
    /// The newest frame the window has not shown yet.
    frame: Mutex<Option<SurfaceFrame>>,
    fit: Mutex<Fit>,
    clock: Mutex<Clock>,
    stopped: AtomicBool,
}

impl Shared {
    /// Seconds into the song, counting on from the last word of it while it plays. Only
    /// so far: a song that is still loading says nothing new, and is not moving.
    fn now(&self) -> f64 {
        let clock = self.clock.lock().unwrap();
        clock.position + if clock.playing { clock.at.elapsed().as_secs_f64().min(UNHEARD) } else { 0.0 }
    }

    fn playing(&self) -> bool {
        self.clock.lock().unwrap().playing
    }

    fn stopped(&self) -> bool {
        self.stopped.load(Ordering::Relaxed)
    }

    fn show(&self, frame: &Frame, repaint: &impl Fn()) {
        let fit = *self.fit.lock().unwrap();
        let laid = match fit {
            Fit::Square(side) => squared(frame, side),
            Fit::Whole(width) => SurfaceFrame::from_rgba([frame.width, frame.height], &frame.rgba, frame.width as f32 / width.max(1.0)),
        };
        if let Some(laid) = laid {
            *self.frame.lock().unwrap() = Some(laid);
            repaint();
        }
    }
}

/// Footage being decoded on a thread of its own. Dropping it ends the thread.
pub struct Screen {
    /// What it shows, so the window knows when to start another.
    pub key: String,
    shared: Arc<Shared>,
}

impl Screen {
    /// `repaint` wakes the window when a frame is ready.
    pub fn start(key: String, footage: Arc<Footage>, showing: Showing, fit: Fit, repaint: impl Fn() + Send + 'static) -> Self {
        let shared = Arc::new(Shared {
            frame: Mutex::new(None),
            fit: Mutex::new(fit),
            clock: Mutex::new(Clock { position: 0.0, at: Instant::now(), playing: false }),
            stopped: AtomicBool::new(false),
        });
        let theirs = shared.clone();
        // A machine that cannot spare a thread goes on showing the cover.
        let _ = std::thread::Builder::new().name("clip".into()).spawn(move || match showing {
            Showing::Loop => turn(&footage.clip, &theirs, &repaint),
            Showing::Small => keep_time(&footage.small.0, &footage.small.1, &theirs, &repaint),
            Showing::Large => keep_time(&footage.large.0, &footage.large.1, &theirs, &repaint),
        });
        Self { key, shared }
    }

    /// The newest frame, once.
    pub fn take(&self) -> Option<SurfaceFrame> {
        self.shared.frame.lock().unwrap().take()
    }

    /// Follows the music: where the song is and whether it plays, and the place the
    /// picture has, which may change size.
    pub fn follow(&self, playing: bool, position: f64, fit: Fit) {
        *self.shared.fit.lock().unwrap() = fit;
        let mut clock = self.shared.clock.lock().unwrap();
        // The same word again is no news: the clock goes on counting from when it came.
        if clock.position != position || clock.playing != playing {
            *clock = Clock { position, at: Instant::now(), playing };
        }
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        self.shared.stopped.store(true, Ordering::Relaxed);
    }
}

/// Decodes the clip from its first picture to its last, at its own pace, again and again.
fn turn(clip: &Clip, shared: &Shared, repaint: &impl Fn()) {
    while !shared.stopped() {
        // Each round starts from the picture that stands alone, on a fresh decoder.
        let Ok(mut decoder) = Decoder::new(&clip.parameters) else { return };
        let mut due = Instant::now();
        for sample in &clip.samples {
            while !shared.playing() && !shared.stopped() {
                std::thread::sleep(Duration::from_millis(50));
                due = Instant::now();
            }
            if shared.stopped() {
                return;
            }
            // A picture the decoder cannot take is skipped; the next one that stands alone mends it.
            if let Ok(Some(frame)) = decoder.decode(&sample.data) {
                shared.show(&frame, repaint);
            }
            due += Duration::from_secs_f64(sample.seconds);
            std::thread::sleep(due.saturating_duration_since(Instant::now()));
        }
    }
}

/// How long the clock counts on without a word from the song, which gives one every
/// quarter of a second while it sounds.
const UNHEARD: f64 = 0.5;
/// How far the song may be from the pictures before they are looked for again where it is.
const ADRIFT: f64 = 1.0;
/// A decoder hands a picture back a few pictures late: they are shown this much early.
const LEAD: f64 = 0.08;

/// Shows the film's pictures as the song reaches them, fetching it a fragment at a time
/// and the next one ahead. Where the song jumps, or a fragment is late, it goes on from
/// the fragment the song is in.
fn keep_time(film: &Film, url: &str, shared: &Shared, repaint: &impl Fn()) {
    let Ok(runtime) = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build() else { return };
    // A client of its own: connections belong to the runtime that opened them.
    let Ok(youtube) = surco_youtube::YouTube::new().map(Arc::new) else { return };
    let fetch = |index: usize| {
        let fragment = *film.fragments.get(index)?;
        let (youtube, url) = (youtube.clone(), url.to_owned());
        Some((index, runtime.spawn(async move { youtube.bytes(&url, fragment.start, fragment.start + fragment.size.saturating_sub(1)).await.ok() })))
    };
    // The fragment being shown, its pictures still to decode, and when the next is due.
    let mut showing: Option<(usize, Decoder, VecDeque<Sample>, f64)> = None;
    let mut ahead: Option<(usize, tokio::task::JoinHandle<Option<Vec<u8>>>)> = None;
    while !shared.stopped() {
        let now = shared.now() + LEAD;
        let adrift = showing.as_ref().is_none_or(|(index, .., due)| {
            now < due - ADRIFT || (now > due + ADRIFT && film.at(now).map(|(there, _)| there) != Some(*index))
        });
        // Out of pictures, the next fragment follows on the same decoder.
        let wanted = match &showing {
            _ if adrift => film.at(now),
            Some((index, _, pending, due)) if pending.is_empty() => Some((index + 1, *due)),
            _ => None,
        };
        if let Some((index, starts)) = wanted {
            let fetched = ahead.take().filter(|(fetching, _)| *fetching == index).or_else(|| fetch(index));
            // Past the film's end, or a fragment that did not come: the last picture stays.
            let Some(bytes) = fetched.and_then(|(_, fetching)| runtime.block_on(fetching).ok().flatten()) else {
                std::thread::sleep(Duration::from_millis(300));
                continue;
            };
            let decoder = match showing.take() {
                Some((_, decoder, ..)) if !adrift => Ok(decoder),
                _ => Decoder::new(&film.header.parameters),
            };
            let Ok(decoder) = decoder else { return };
            showing = Some((index, decoder, surco_video::samples(&bytes, &film.header).into(), starts));
            ahead = fetch(index + 1);
            continue;
        }
        let Some((_, decoder, pending, due)) = showing.as_mut().filter(|_| !adrift) else {
            // The song is somewhere the film is not.
            std::thread::sleep(Duration::from_millis(300));
            continue;
        };
        let Some(sample) = pending.pop_front() else { continue };
        let shown_at = *due;
        *due += sample.seconds;
        let frame = decoder.decode(&sample.data).ok().flatten();
        // One whose moment has passed is decoded for the ones that lean on it, and not shown.
        if now > shown_at + sample.seconds {
            continue;
        }
        // Short naps: a pause holds the picture here, and a jump is noticed.
        loop {
            let ahead_by = shown_at - (shared.now() + LEAD);
            if shared.stopped() || ahead_by <= 0.0 || ahead_by > ADRIFT {
                break;
            }
            std::thread::sleep(Duration::from_secs_f64(ahead_by.min(0.02)));
        }
        if let Some(frame) = frame {
            shared.show(&frame, repaint);
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
