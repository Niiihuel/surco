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
/// The tab bar's controls.
const BAR: [&str; 6] = ["home", "mobile-library", "mobile-playing", "mobile-search", "settings", "account-profile"];
/// How long the focus rests on Home's or Library's tab before it opens, as tvOS's tabs.
const TAB_REST: Duration = Duration::from_millis(140);
/// How far a page scrolls before the tabs step aside.
pub const SCROLLED: f32 = 32.0;

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

    /// When the transport is to step away or a tab to open, for a frame to be asked for then.
    pub fn tv_due(&self) -> Option<Instant> {
        let transport = self.tv_transport.filter(|_| self.player.playing).map(|at| at + TRANSPORT_STAYS);
        let tab = self.tv_resting.as_ref().filter(|(tab, _)| !self.tv_shows(tab)).map(|(_, since)| *since + TAB_REST);
        transport.into_iter().chain(tab).min()
    }

    /// Open Home or Library once the focus has rested on its tab; the other tabs wait for
    /// select. Some once it opened one.
    pub fn tv_rest(&mut self, now: Instant) -> Option<Vec<Command>> {
        let tab = self.tv_focused.as_deref().filter(|id| ["home", "mobile-library"].contains(id));
        if self.tv_resting.as_ref().map(|(id, _)| id.as_str()) != tab {
            self.tv_resting = tab.map(|id| (id.to_owned(), now));
        }
        let due = self.tv_resting.as_ref().filter(|(tab, since)| now >= *since + TAB_REST && !self.tv_shows(tab)).map(|(tab, _)| tab.clone())?;
        Some(self.click(&due))
    }

    /// Whether the page shown is the one of the tab `id`.
    fn tv_shows(&self, id: &str) -> bool {
        self.tv_tab() == Some(id)
    }

    /// The tab of the page shown, when it is a tab's own and not one pushed over it.
    pub fn tv_tab(&self) -> Option<&'static str> {
        match self.page {
            Page::Search if self.mobile_search => Some("mobile-search"),
            Page::Search => Some("home"),
            Page::Library => Some("mobile-library"),
            Page::Settings => Some("settings"),
            _ => None,
        }
    }

    /// Where the remote's focus goes when it is on nothing: the page's tab, a song list's Play,
    /// the player's scrubber; never another tab, which resting there would open.
    pub fn tv_rest_focus(&self) -> Option<&'static str> {
        match self.page {
            // With the transport away, the player itself takes the presses.
            Page::Playing | Page::Lyrics => self.tv_transport.map(|_| SCRUBBER),
            Page::Album(_) | Page::Artist(_) | Page::Playlist(_) | Page::Mix(_) => Some("page-play"),
            Page::Search if self.mobile_search => Some("search"),
            // Elsewhere the first press of an arrow focuses the page's first control.
            _ => self.tv_tab(),
        }
    }

    /// Whether the remote's focus is up on the tab bar.
    pub fn tv_on_bar(&self) -> bool {
        self.tv_focused.as_deref().is_some_and(|id| BAR.contains(&id))
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
        assert_eq!(state.tv_due(), Some(start + TRANSPORT_STAYS));
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
    fn home_and_library_open_as_the_focus_rests_on_their_tabs() {
        let mut state = State { tv: true, ..State::default() };
        let start = Instant::now();
        state.tv_focused = Some("mobile-library".into());
        assert_eq!(state.tv_rest(start), None);
        assert_eq!(state.tv_due(), Some(start + TAB_REST));
        state.tv_focused = Some("mobile-playing".into());
        assert_eq!(state.tv_rest(start + TAB_REST), None, "passing over it opens nothing");
        state.tv_focused = Some("mobile-library".into());
        state.tv_rest(start + TAB_REST);
        assert!(state.tv_rest(start + TAB_REST * 2).is_some());
        assert_eq!(state.page, Page::Library);
        assert_eq!(state.tv_rest(start + TAB_REST * 3), None, "once");
        state.tv_focused = Some("settings".into());
        assert_eq!(state.tv_rest(start + TAB_REST * 9), None, "the rest wait for select");
    }

    #[test]
    fn the_focus_lost_on_a_pushed_page_never_rests_on_a_tab_that_would_close_it() {
        let mut state = State { tv: true, ..State::default() };
        assert_eq!(state.tv_rest_focus(), Some("home"));
        state.update(&UiAction::Clicked { id: "settings".into() });
        assert_eq!(state.tv_rest_focus(), Some("settings"));
        state.page = Page::Liked;
        assert_eq!(state.tv_rest_focus(), None);
    }

    #[test]
    fn back_goes_up_to_the_tabs_then_home_then_out() {
        let mut state = State { tv: true, ..State::default() };
        state.update(&UiAction::Clicked { id: "mobile-library".into() });
        state.tv_focused = Some("playlist:0".into());
        assert_eq!(crate::back_target(&state), Some("tv-tabs"));
        state.update(&UiAction::Clicked { id: "tv-tabs".into() });
        assert_eq!(state.focus.as_deref(), Some("mobile-library"), "up to the page's tab");
        state.tv_focused = state.focus.take();
        assert_eq!(crate::back_target(&state), Some("home"));
        state.update(&UiAction::Clicked { id: "home".into() });
        state.tv_focused = Some("home".into());
        assert_eq!(crate::back_target(&state), None, "from Home's tab, out of the app");
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
