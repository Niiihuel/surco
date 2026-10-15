//! iOS system playback. Queue and decoder ownership stay with the Rust backend.

use crate::player::PlayerView;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SessionEvent {
    Interrupted,
    InterruptionEnded(bool),
    RouteLost,
    RouteChanged,
    MediaReset,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SessionAction {
    None,
    Pause,
    Resume,
    Rebuild,
    Reset,
}

#[derive(Default)]
pub struct Lifecycle {
    interrupted: bool,
    resume_track: Option<String>,
}

impl Lifecycle {
    pub fn interrupted(&self) -> bool {
        self.interrupted
    }

    pub fn user_decision(&mut self) {
        self.resume_track = None;
    }

    pub fn event(
        &mut self,
        event: SessionEvent,
        track: Option<&str>,
        playing: bool,
    ) -> SessionAction {
        match event {
            SessionEvent::Interrupted => {
                if !self.interrupted {
                    self.resume_track = track.filter(|_| playing).map(str::to_owned);
                }
                self.interrupted = true;
                SessionAction::Pause
            }
            SessionEvent::InterruptionEnded(should_resume) => {
                self.interrupted = false;
                let resume = self.resume_track.take();
                if should_resume
                    && resume.as_deref().is_some_and(|saved| Some(saved) == track)
                    && !playing
                {
                    SessionAction::Resume
                } else {
                    SessionAction::None
                }
            }
            SessionEvent::RouteLost => {
                self.user_decision();
                SessionAction::Pause
            }
            SessionEvent::RouteChanged => {
                if self.interrupted {
                    SessionAction::None
                } else {
                    SessionAction::Rebuild
                }
            }
            SessionEvent::MediaReset => {
                self.interrupted = false;
                self.user_decision();
                SessionAction::Reset
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct TrackMetadata {
    id: String,
    title: String,
    artist: String,
    album: Option<String>,
    artwork_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Snapshot {
    track: Option<TrackMetadata>,
    duration: f64,
    elapsed: f64,
    rate: f64,
}

#[derive(Default)]
pub struct MetadataDiff {
    previous: Option<(Snapshot, f64)>,
}

impl MetadataDiff {
    /// iOS extrapolates elapsed time from the last publication and its playback rate.
    /// https://developer.apple.com/documentation/mediaplayer/mpnowplayinginfopropertyelapsedplaybacktime
    pub fn changed(&mut self, view: &PlayerView, now: f64) -> Option<Snapshot> {
        let finite = |value: f64| {
            if value.is_finite() {
                value.max(0.0)
            } else {
                0.0
            }
        };
        let snapshot = Snapshot {
            track: view.current.as_ref().map(|track| TrackMetadata {
                id: track.id.clone(),
                title: track.title.clone(),
                artist: track.artist.clone(),
                album: track.album.clone(),
                artwork_url: track.artwork_url.clone(),
            }),
            duration: finite(view.duration),
            elapsed: finite(view.position),
            rate: if view.current.is_some() && view.playing {
                1.0
            } else {
                0.0
            },
        };
        if let Some((previous, published)) = &self.previous {
            let expected = (previous.elapsed + (now - published).max(0.0) * previous.rate)
                .min(previous.duration);
            if previous.track == snapshot.track
                && previous.duration == snapshot.duration
                && previous.rate == snapshot.rate
                && (expected - snapshot.elapsed).abs() < 0.75
            {
                return None;
            }
        }
        self.previous = Some((snapshot.clone(), now));
        Some(snapshot)
    }
}

#[cfg(target_os = "ios")]
mod native {
    use super::*;
    use crate::media_keys::{Key, MediaEvent};
    use std::ffi::{c_char, CStr, CString};
    use std::sync::Mutex;
    use std::time::Instant;

    type Handler = Box<dyn Fn(MediaEvent) + Send>;
    static HANDLER: Mutex<Option<Handler>> = Mutex::new(None);

    unsafe extern "C" {
        fn surco_media_start(callback: extern "C" fn(i32, f64) -> i32);
        fn surco_media_stop();
        fn surco_media_publish(json: *const c_char);
        fn surco_media_artwork(
            track: *const c_char,
            url: *const c_char,
            bytes: *const u8,
            length: usize,
        );
        fn surco_media_configure(message: *mut c_char, capacity: usize) -> i32;
        fn surco_media_activate(message: *mut c_char, capacity: usize) -> i32;
        fn surco_media_deactivate(message: *mut c_char, capacity: usize) -> i32;
    }

    fn deliver(event: MediaEvent) -> i32 {
        let handler = HANDLER
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(handler) = &*handler {
            handler(event);
            1
        } else {
            0
        }
    }

    extern "C" fn receive(kind: i32, value: f64) -> i32 {
        let event = match kind {
            1 => MediaEvent::Key(Key::Play),
            2 => MediaEvent::Key(Key::Pause),
            3 => MediaEvent::Key(Key::Toggle),
            4 => MediaEvent::Key(Key::Next),
            5 => MediaEvent::Key(Key::Previous),
            6 if value.is_finite() => MediaEvent::Key(Key::Seek(value)),
            10 => MediaEvent::Session(SessionEvent::Interrupted),
            11 => MediaEvent::Session(SessionEvent::InterruptionEnded(value != 0.0)),
            12 => MediaEvent::Session(SessionEvent::RouteLost),
            13 => MediaEvent::Session(SessionEvent::RouteChanged),
            14 => MediaEvent::Session(SessionEvent::MediaReset),
            _ => return 0,
        };
        deliver(event)
    }

    fn session_call(call: unsafe extern "C" fn(*mut c_char, usize) -> i32) -> Result<(), String> {
        let mut message = [0 as c_char; 1024];
        if unsafe { call(message.as_mut_ptr(), message.len()) } != 0 {
            Ok(())
        } else {
            // The bridge always leaves a bounded, NUL-terminated error buffer.
            Err(unsafe { CStr::from_ptr(message.as_ptr()) }
                .to_string_lossy()
                .into_owned())
        }
    }

    pub fn configure() -> Result<(), String> {
        session_call(surco_media_configure)
    }
    pub fn activate() -> Result<(), String> {
        session_call(surco_media_activate)
    }
    pub fn deactivate() -> Result<(), String> {
        session_call(surco_media_deactivate)
    }

    pub struct Controls {
        metadata: MetadataDiff,
        artwork: Option<(String, String)>,
        started: Instant,
    }

    impl Controls {
        pub fn start(handler: impl Fn(MediaEvent) + Send + 'static) -> Self {
            *HANDLER
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Box::new(handler));
            unsafe { surco_media_start(receive) };
            Self {
                metadata: MetadataDiff::default(),
                artwork: None,
                started: Instant::now(),
            }
        }

        pub fn show(&mut self, view: &PlayerView) {
            let Some(snapshot) = self
                .metadata
                .changed(view, self.started.elapsed().as_secs_f64())
            else {
                return;
            };
            match serde_json::to_vec(&snapshot)
                .map_err(|error| error.to_string())
                .and_then(|json| CString::new(json).map_err(|error| error.to_string()))
            {
                Ok(json) => unsafe { surco_media_publish(json.as_ptr()) },
                Err(error) => eprintln!("Surco could not publish Now Playing: {error}"),
            }
            let artwork = snapshot
                .track
                .and_then(|track| track.artwork_url.map(|url| (track.id, url)));
            if artwork != self.artwork {
                self.artwork = artwork.clone();
                if let Some((track_id, url)) = artwork {
                    deliver(MediaEvent::Artwork { track_id, url });
                }
            }
        }
    }

    impl Drop for Controls {
        fn drop(&mut self) {
            *HANDLER
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
            unsafe { surco_media_stop() };
        }
    }

    pub fn artwork(track: &str, url: &str, bytes: &[u8]) {
        let (Ok(track), Ok(url)) = (CString::new(track), CString::new(url)) else {
            return;
        };
        // NSData copies the bytes before the native call returns; queued UIKit work owns them.
        unsafe { surco_media_artwork(track.as_ptr(), url.as_ptr(), bytes.as_ptr(), bytes.len()) };
    }
}

#[cfg(target_os = "ios")]
pub use native::{activate, artwork, configure, deactivate, Controls};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_interruption_resumes_only_the_unmodified_playing_track() {
        let mut state = Lifecycle::default();
        assert_eq!(
            state.event(SessionEvent::Interrupted, Some("a"), true),
            SessionAction::Pause
        );
        assert!(state.interrupted());
        assert_eq!(
            state.event(SessionEvent::InterruptionEnded(true), Some("a"), false),
            SessionAction::Resume
        );
        assert!(!state.interrupted());

        state.event(SessionEvent::Interrupted, Some("a"), true);
        state.user_decision();
        assert_eq!(
            state.event(SessionEvent::InterruptionEnded(true), Some("a"), false),
            SessionAction::None
        );
        state.event(SessionEvent::Interrupted, Some("a"), true);
        assert_eq!(
            state.event(SessionEvent::InterruptionEnded(true), Some("b"), false),
            SessionAction::None
        );
        state.event(SessionEvent::Interrupted, Some("a"), true);
        assert_eq!(
            state.event(SessionEvent::InterruptionEnded(false), Some("a"), false),
            SessionAction::None
        );
        state.event(SessionEvent::Interrupted, Some("a"), false);
        assert_eq!(
            state.event(SessionEvent::InterruptionEnded(true), Some("a"), false),
            SessionAction::None
        );
    }

    #[test]
    fn route_loss_and_media_reset_never_resume_unasked() {
        let mut state = Lifecycle::default();
        state.event(SessionEvent::Interrupted, Some("a"), true);
        assert_eq!(
            state.event(SessionEvent::RouteLost, Some("a"), false),
            SessionAction::Pause
        );
        assert_eq!(
            state.event(SessionEvent::InterruptionEnded(true), Some("a"), false),
            SessionAction::None
        );
        assert_eq!(
            state.event(SessionEvent::MediaReset, Some("a"), true),
            SessionAction::Reset
        );
        assert_eq!(
            state.event(SessionEvent::RouteChanged, Some("a"), false),
            SessionAction::Rebuild
        );
    }

    #[test]
    fn now_playing_uses_extrapolation_but_publishes_seek_pause_and_new_metadata() {
        let mut diff = MetadataDiff::default();
        let mut view = crate::player::PlayerView::default();
        view.current = Some(surco_core::Track {
            id: "a".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            artist_id: None,
            album: Some("Album".into()),
            album_id: None,
            artwork_url: Some("https://cover/a".into()),
            duration_ms: 200_000,
        });
        view.playing = true;
        view.duration = 200.0;
        assert!(diff.changed(&view, 0.0).is_some());
        view.position = 1.0;
        assert!(diff.changed(&view, 1.0).is_none());
        view.position = 30.0;
        assert!(diff.changed(&view, 1.1).is_some());
        view.playing = false;
        assert_eq!(diff.changed(&view, 1.2).unwrap().rate, 0.0);
        view.current.as_mut().unwrap().album = None;
        view.current.as_mut().unwrap().artwork_url = None;
        assert!(
            diff.changed(&view, 1.3).is_some(),
            "missing new fields must clear old values"
        );
        view.current = None;
        assert!(diff.changed(&view, 1.4).unwrap().track.is_none());
        assert!(diff.changed(&view, 1.5).is_none());
    }
}
