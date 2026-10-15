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
