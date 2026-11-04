//! A television's player, as BitChord's TV: the screen is the song, and its transport rises on
//! the remote's presses and steps away after a few idle seconds.

use std::time::{Duration, Instant};

use super::{Page, State};
use crate::backend::Command;

/// How long the transport stays up after the remote's last press.
pub const TRANSPORT_STAYS: Duration = Duration::from_secs(5);
/// How far a press on the scrubber seeks, in seconds.
const SEEK_STEP: f64 = 10.0;
/// The transport's scrubber, which the focus goes to when the transport comes up.
pub const SCRUBBER: &str = "tv-scrubber";

impl State {
    /// Raise the transport on the way into the player, and send it away 5 s after the last
    /// press while the music plays; paused, it stays. Says whether it just went.
    pub fn tv_tick(&mut self, now: Instant) -> bool {
        let in_player = self.tv && matches!(self.page, Page::Playing | Page::Lyrics);
        if in_player && !self.tv_in_player {
            self.tv_transport = Some(now);
        }
        self.tv_in_player = in_player;
        if !in_player {
            self.tv_transport = None;
            return false;
        }
        // Paused, the 5 s start over once the music does.
        if !self.player.playing && self.tv_transport.is_some() {
            self.tv_transport = Some(now);
        }
        let gone = self.tv_transport.is_some_and(|at| now >= at + TRANSPORT_STAYS);
        if gone {
            self.tv_transport = None;
        }
        gone
    }

    /// When the transport is to step away, for a frame to be asked for then.
    pub fn tv_transport_due(&self) -> Option<Instant> {
        self.tv_transport.filter(|_| self.player.playing).map(|at| at + TRANSPORT_STAYS)
    }

    /// A press of the remote in the player. Each keeps the transport up; while it is away, left
    /// and right skip and up, down and select bring it back; on the scrubber left and right seek
    /// 10 s. None for a press the tree is to take, as one that moves the focus.
    pub fn tv_press(&mut self, key: limn::Key, now: Instant) -> Option<Vec<Command>> {
        use limn::Key;
        if !self.tv_in_player || key == Key::Escape {
            return None;
        }
        if self.tv_transport.is_none() {
            return Some(match key {
                Key::ArrowLeft => self.click("previous"),
                Key::ArrowRight => self.click("next"),
                Key::ArrowUp | Key::ArrowDown | Key::Enter => {
                    self.tv_transport = Some(now);
                    self.focus = Some(SCRUBBER.into());
                    Vec::new()
                }
                _ => return None,
            });
        }
        self.tv_transport = Some(now);
        let step = match key {
            Key::ArrowLeft => -SEEK_STEP,
            Key::ArrowRight => SEEK_STEP,
            _ => return None,
        };
        if self.tv_focused.as_deref() != Some(SCRUBBER) || self.player.duration <= 0.0 {
            return None;
        }
        // shortcut: each press seeks at once; BitChord gathers presses for 650 ms first, worth it if streams stall on quick seeks.
        self.player.position = (self.player.position + step).clamp(0.0, self.player.duration);
        Some(vec![Command::Seek(self.player.position)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::PlayerView;
    use limn::{Key, UiAction};
    use surco_core::Track;

    fn playing() -> State {
        let mut state = State { tv: true, ..State::default() };
        let track = Track { id: "song".into(), title: "Canción".into(), artist: "Banda".into(), artist_id: None,
            album: None, album_id: None, artwork_url: None, duration_ms: 200_000 };
        state.show(PlayerView { current: Some(track), playing: true, position: 30.0, duration: 200.0, ..PlayerView::default() });
        state.update(&UiAction::Clicked { id: "mobile-playing".into() });
        state
    }

    #[test]
    fn the_transport_steps_away_after_five_idle_seconds_while_the_music_plays() {
        let mut state = playing();
        let start = Instant::now();
        assert!(!state.tv_tick(start));
        assert_eq!(state.tv_transport, Some(start), "up on the way in");
        assert_eq!(state.tv_transport_due(), Some(start + TRANSPORT_STAYS));
        assert_eq!(state.tv_press(Key::ArrowUp, start + Duration::from_secs(3)), None, "a press up the transport is the tree's");
        assert!(!state.tv_tick(start + Duration::from_secs(6)), "a press keeps it up");
        assert!(state.tv_tick(start + Duration::from_secs(8)));
        assert_eq!(state.tv_transport, None);
        state.player.playing = false;
        state.tv_press(Key::Enter, start + Duration::from_secs(9));
        assert!(!state.tv_tick(start + Duration::from_secs(60)), "paused, it stays");
    }

    #[test]
    fn with_the_transport_away_the_arrows_skip_and_the_rest_bring_it_back() {
        let mut state = playing();
        let start = Instant::now();
        state.tv_tick(start);
        state.tv_tick(start + TRANSPORT_STAYS);
        assert_eq!(state.tv_press(Key::ArrowRight, start + TRANSPORT_STAYS), Some(vec![Command::Next]));
        assert_eq!(state.tv_press(Key::ArrowLeft, start + TRANSPORT_STAYS), Some(vec![Command::Previous]));
        assert_eq!(state.tv_transport, None, "skipping leaves it away");
        assert_eq!(state.tv_press(Key::ArrowDown, start + TRANSPORT_STAYS), Some(Vec::new()));
        assert!(state.tv_transport.is_some());
        assert_eq!(state.focus.as_deref(), Some(SCRUBBER), "the focus comes up on the scrubber");
    }

    #[test]
    fn on_the_scrubber_left_and_right_seek_ten_seconds() {
        let mut state = playing();
        let start = Instant::now();
        state.tv_tick(start);
        state.tv_focused = Some(SCRUBBER.into());
        assert_eq!(state.tv_press(Key::ArrowRight, start), Some(vec![Command::Seek(40.0)]));
        assert_eq!(state.tv_press(Key::ArrowLeft, start), Some(vec![Command::Seek(30.0)]));
        state.tv_focused = Some("like".into());
        assert_eq!(state.tv_press(Key::ArrowLeft, start), None, "elsewhere the arrows move the focus");
    }
}
