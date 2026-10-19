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
/// on a square of this side, or all of it at this width in a room it does not fill, which
/// gets a glow of the frame behind it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fit {
    /// Filling a box of this width and height: the frame's middle at its proportions.
    Cover { width: f32, height: f32 },
    Whole { width: f32, room: (f32, f32) },
}

/// Where the song is, as the window last knew it.
struct Clock {
    position: f64,
    at: Instant,
    playing: bool,
}

/// What the window and the thread that decodes share.
struct Shared {
    /// The newest frame the window has not shown yet, and the glow that goes behind it.
    frame: Mutex<Option<SurfaceFrame>>,
    backdrop: Mutex<Option<SurfaceFrame>>,
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
            Fit::Cover { width, height } => covered(frame, width, height),
            Fit::Whole { width, room } => {
                *self.backdrop.lock().unwrap() = glow(frame, room);
                SurfaceFrame::from_rgba([frame.width, frame.height], &frame.rgba, frame.width as f32 / width.max(1.0))
            }
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
            backdrop: Mutex::new(None),
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

    /// The newest glow for behind the frame, once.
    pub fn take_backdrop(&self) -> Option<SurfaceFrame> {
        self.shared.backdrop.lock().unwrap().take()
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

/// How many pixels across the glow is, and how much of the frame's light it keeps.
const GLOW_ACROSS: usize = 32;
const GLOW_LIGHT: f32 = 0.45;

/// The frame as a glow to stand behind it where it does not fill its room: its middle at
/// the room's proportions, a few pixels across and dimmed. Drawn as large as the room,
/// those pixels run into one another.
fn glow(frame: &Frame, room: (f32, f32)) -> Option<SurfaceFrame> {
    let (across, tall) = room;
    if across < 1.0 || tall < 1.0 || frame.width == 0 || frame.height == 0 {
        return None;
    }
    // Rounded up: it is drawn at the room's width, and must not end short of its floor.
    let down = ((GLOW_ACROSS as f32 * tall / across).ceil() as usize).max(1);
    // The part of the frame that has the room's proportions.
    let wide = frame.width as f32 / frame.height as f32 > across / tall;
    let (taken_across, taken_down) = if wide { ((frame.height as f32 * across / tall) as usize, frame.height) } else { (frame.width, (frame.width as f32 * tall / across) as usize) };
    let (left, top) = ((frame.width - taken_across.min(frame.width)) / 2, (frame.height - taken_down.min(frame.height)) / 2);
    // Where sample `nth` of five around glow pixel `at` falls among `taken` frame pixels:
    // the five reach into the pixels beside it, so that neighbours share what they show.
    let place = |at: usize, nth: usize, of: usize, taken: usize| (((at as f32 + 0.5 + (nth as f32 - 2.0) * 0.6) / of as f32).clamp(0.0, 1.0) * taken.saturating_sub(1) as f32) as usize;
    let mut rgba = Vec::with_capacity(GLOW_ACROSS * down * 4);
    for row in 0..down {
        // Its last rows thin out, down to the bar under the room.
        let shown = ((down - row) as f32 / (down as f32 * 0.12)).min(1.0);
        for column in 0..GLOW_ACROSS {
            let mut light = [0.0f32; 3];
            for sample in 0..25 {
                let x = (left + place(column, sample % 5, GLOW_ACROSS, taken_across)).min(frame.width - 1);
                let y = (top + place(row, sample / 5, down, taken_down)).min(frame.height - 1);
                let pixel = frame.rgba.get((y * frame.width + x) * 4..)?;
                for (sum, value) in light.iter_mut().zip(pixel) {
                    *sum += f32::from(*value);
                }
            }
            rgba.extend(light.map(|sum| (sum / 25.0 * GLOW_LIGHT) as u8));
            rgba.push((shown * 255.0) as u8);
        }
    }
    SurfaceFrame::from_rgba([GLOW_ACROSS, down], &rgba, GLOW_ACROSS as f32 / across)
}

/// The middle square of a frame, said to be as dense as it takes to fill `side`: the
/// backend draws pixels at their own density, so that is what makes them cover the square.
fn covered(frame: &Frame, width: f32, height: f32) -> Option<SurfaceFrame> {
    let aspect = width / height.max(1.0);
    // The whole height of a frame wider than the box, the whole width of one narrower.
    let across = ((frame.height as f32 * aspect).round() as usize).clamp(1, frame.width.max(1));
    let down = ((frame.width as f32 / aspect).round() as usize).clamp(1, frame.height.max(1));
    let (across, down) = if across < frame.width { (across, frame.height) } else { (frame.width, down) };
    let (left, top) = ((frame.width - across) / 2, (frame.height - down) / 2);
    let mut rgba = Vec::with_capacity(across * down * 4);
    for row in top..top + down {
        let start = (row * frame.width + left) * 4;
        rgba.extend_from_slice(frame.rgba.get(start..start + across * 4)?);
    }
    SurfaceFrame::from_rgba([across, down], &rgba, across as f32 / width.max(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_gives_its_middle_at_the_boxs_proportions() {
        // Four by two, each pixel carrying its column in red.
        let rgba: Vec<u8> = (0..2).flat_map(|_| (0..4u8).flat_map(|column| [column, 0, 0, 255])).collect();
        let wide = Frame { width: 4, height: 2, rgba };
        assert_eq!(covered(&wide, 100.0, 100.0).unwrap().size(), [2, 2]);
        assert_eq!(covered(&wide, 90.0, 145.0).unwrap().size(), [1, 2], "a tall box takes a narrow middle");
        assert_eq!(covered(&wide, 400.0, 100.0).unwrap().size(), [4, 1], "a wider box takes the middle rows");
        assert!(covered(&Frame { width: 4, height: 2, rgba: vec![0; 3] }, 100.0, 100.0).is_none(), "a frame with rows missing");
    }

    #[test]
    fn the_glow_behind_a_frame_is_a_few_dimmed_pixels_at_the_rooms_proportions() {
        let frame = Frame { width: 64, height: 64, rgba: [200, 100, 50, 255].repeat(64 * 64) };
        let glow = glow(&frame, (1600.0, 800.0)).unwrap();
        assert_eq!(glow.size(), [GLOW_ACROSS, GLOW_ACROSS / 2]);
        assert!(super::glow(&frame, (0.0, 800.0)).is_none() && super::glow(&Frame { width: 0, height: 0, rgba: Vec::new() }, (16.0, 9.0)).is_none());
    }
}
