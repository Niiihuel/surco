//! Everything the window shows, and the two ways it changes: an action from the user
//! (`update`) or an answer from the backend (`apply`). Both return the commands to send.

use std::collections::{HashMap, HashSet};

use dnm_audio::equalizer::EqualizerValues;
use dnm_core::Track;
use limn::{RasterImage, UiAction};

use crate::backend::{Command, Event};
use crate::player::PlayerView;

pub struct State {
    pub query: String,
    /// The query whose results are shown or awaited. Answers for any other are stale.
    pub submitted: Option<String>,
    pub searching: bool,
    pub results: Vec<Track>,
    pub search_error: Option<String>,
    /// The player as the backend last described it.
    pub player: PlayerView,
    /// Where the seek thumb is while it is being dragged.
    pub seek_preview: Option<f32>,
    pub volume: f32,
    pub queue_open: bool,
    pub equalizer_open: bool,
    pub equalizer: EqualizerValues,
    pub artwork: HashMap<String, RasterImage>,
    /// Covers already asked for, arrived or not.
    covers: HashSet<String>,
}

/// One request per cover, however many rows show it and however often it comes up.
fn cover_requests<'a>(asked: &mut HashSet<String>, tracks: impl IntoIterator<Item = &'a Track>) -> Vec<Command> {
    let urls = tracks.into_iter().filter_map(|track| track.artwork_url.as_ref());
    urls.filter(|url| asked.insert((*url).clone())).map(|url| Command::FetchArtwork(url.clone())).collect()
}

/// The equaliser's range, each side of zero.
pub const EQUALIZER_DB: f32 = 12.0;

impl Default for State {
    fn default() -> Self {
        Self {
            query: String::new(),
            submitted: None,
            searching: false,
            results: Vec::new(),
            search_error: None,
            player: PlayerView::default(),
            seek_preview: None,
            volume: 0.8,
            queue_open: false,
            equalizer_open: false,
            equalizer: EqualizerValues::FLAT,
            artwork: HashMap::new(),
            covers: HashSet::new(),
        }
    }
}

impl State {
    pub fn current(&self) -> Option<&Track> {
        self.player.current.as_ref()
    }

    /// Share of the track that has played, following the thumb while it is dragged.
    pub fn progress(&self) -> f32 {
        let player = &self.player;
        self.seek_preview.unwrap_or_else(|| {
            if player.duration > 0.0 { (player.position / player.duration).clamp(0.0, 1.0) as f32 } else { 0.0 }
        })
    }

    pub fn search(&mut self) -> Vec<Command> {
        let query = self.query.trim().to_owned();
        if query.is_empty() {
            return Vec::new();
        }
        self.submitted = Some(query.clone());
        self.searching = true;
        self.search_error = None;
        vec![Command::Search(query)]
    }

    pub fn update(&mut self, action: &UiAction) -> Vec<Command> {
        match action {
            UiAction::InputChanged { id, value } if id == "search" => {
                self.query = value.clone();
                Vec::new()
            }
            UiAction::Submitted { id, .. } if id == "search" => self.search(),
            UiAction::SliderChanged { id, fraction, committed } if id == "seek" => {
                if !*committed {
                    self.seek_preview = Some(*fraction);
                    return Vec::new();
                }
                self.seek_preview = None;
                if self.current().is_none() || self.player.duration <= 0.0 {
                    return Vec::new();
                }
                // Ahead of the backend's answer, so the thumb does not jump back for a frame.
                self.player.position = f64::from(*fraction) * self.player.duration;
                vec![Command::Seek(self.player.position)]
            }
            UiAction::SliderChanged { id, fraction, .. } if id == "volume" => {
                self.volume = *fraction;
                vec![Command::Volume(self.volume)]
            }
            UiAction::SliderChanged { id, fraction, .. } if id.starts_with("equalizer:") => {
                let gain = ((fraction * 2.0 - 1.0) * EQUALIZER_DB).round();
                match id["equalizer:".len()..].parse::<usize>() {
                    Ok(band) if band < self.equalizer.bands_db.len() => self.equalizer.bands_db[band] = gain,
                    Ok(_) => return Vec::new(),
                    Err(_) => self.equalizer.preamp_db = gain,
                }
                vec![Command::Equalizer(self.equalizer)]
            }
            UiAction::Clicked { id } => self.click(id),
            _ => Vec::new(),
        }
    }

    fn click(&mut self, id: &str) -> Vec<Command> {
        if let Some(index) = id.strip_prefix("track:").and_then(|i| i.parse::<usize>().ok()) {
            // A result plays on its own and the player follows it with its radio.
            return self.results.get(index).map(|track| Command::PlayList { tracks: vec![track.clone()], index: 0 }).into_iter().collect();
        }
        if let Some(upcoming) = id.strip_prefix("upcoming:").and_then(|i| i.parse::<usize>().ok()) {
            return vec![Command::Jump(upcoming)];
        }
        match id {
            "queue" => {
                self.queue_open = !self.queue_open;
                self.upcoming_covers()
            }
            "equalizer" => {
                self.equalizer_open = !self.equalizer_open;
                Vec::new()
            }
            "equalizer-close" => {
                self.equalizer_open = false;
                Vec::new()
            }
            "equalizer-reset" => {
                self.equalizer = EqualizerValues::FLAT;
                vec![Command::Equalizer(self.equalizer)]
            }
            "play-pause" => vec![Command::PlayPause],
            "next" => vec![Command::Next],
            "previous" => vec![Command::Previous],
            "shuffle" => vec![Command::Shuffle],
            "repeat" => vec![Command::Repeat],
            "mute" => {
                self.volume = if self.volume > 0.0 { 0.0 } else { 0.8 };
                vec![Command::Volume(self.volume)]
            }
            _ => Vec::new(),
        }
    }

    pub fn apply(&mut self, event: Event) -> Vec<Command> {
        match event {
            Event::SearchResults { query, tracks } if self.submitted.as_deref() == Some(&query) => {
                self.searching = false;
                self.results = tracks;
                cover_requests(&mut self.covers, &self.results)
            }
            Event::SearchFailed { query, message } if self.submitted.as_deref() == Some(&query) => {
                self.searching = false;
                self.search_error = Some(message);
                Vec::new()
            }
            Event::Artwork { url, image } => {
                self.artwork.insert(url, image);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// Takes the player as the backend now describes it.
    pub fn show(&mut self, player: PlayerView) -> Vec<Command> {
        self.player = player;
        let mut commands = cover_requests(&mut self.covers, &self.player.current);
        commands.extend(self.upcoming_covers());
        commands
    }

    /// The queue's covers are only fetched while the queue is on screen.
    fn upcoming_covers(&mut self) -> Vec<Command> {
        if !self.queue_open {
            return Vec::new();
        }
        cover_requests(&mut self.covers, &self.player.upcoming)
    }
}

/// `m:ss`, the way a player writes a time.
pub fn clock(seconds: f64) -> String {
    let total = if seconds.is_finite() { seconds.max(0.0) as u64 } else { 0 };
    format!("{}:{:02}", total / 60, total % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str) -> Track {
        Track {
            id: id.into(),
            title: id.into(),
            artist: "artist".into(),
            artist_id: None,
            album: None,
            album_id: None,
            artwork_url: Some(format!("https://img/{id}")),
            duration_ms: 200_000,
        }
    }

    fn click(state: &mut State, id: &str) -> Vec<Command> {
        state.update(&UiAction::Clicked { id: id.into() })
    }

    fn playing(id: &str) -> PlayerView {
        PlayerView { current: Some(track(id)), playing: true, duration: 200.0, ..PlayerView::default() }
    }

    #[test]
    fn a_search_asks_once_and_ignores_the_answer_to_an_older_query() {
        let mut state = State { query: "  first ".into(), ..State::default() };
        assert_eq!(state.search(), vec![Command::Search("first".into())]);
        assert!(state.searching);
        state.query = "second".into();
        state.search();
        state.apply(Event::SearchResults { query: "first".into(), tracks: vec![track("old")] });
        assert!(state.results.is_empty() && state.searching, "the stale answer changes nothing");
        let commands = state.apply(Event::SearchResults { query: "second".into(), tracks: vec![track("a"), track("a")] });
        assert!(!state.searching);
        assert_eq!(commands, vec![Command::FetchArtwork("https://img/a".into())], "one request per cover");
        state.query = "   ".into();
        assert!(state.search().is_empty());
    }

    #[test]
    fn clicks_become_player_commands() {
        let mut state = State { query: "q".into(), ..State::default() };
        state.search();
        state.apply(Event::SearchResults { query: "q".into(), tracks: vec![track("a"), track("b")] });
        assert_eq!(click(&mut state, "track:1"), vec![Command::PlayList { tracks: vec![track("b")], index: 0 }]);
        assert!(click(&mut state, "track:9").is_empty());
        assert!(state.current().is_none(), "the player says what plays, not the click");
        for (id, command) in [
            ("upcoming:3", Command::Jump(3)),
            ("play-pause", Command::PlayPause),
            ("next", Command::Next),
            ("previous", Command::Previous),
            ("shuffle", Command::Shuffle),
            ("repeat", Command::Repeat),
        ] {
            assert_eq!(click(&mut state, id), vec![command]);
        }
        let volume = UiAction::SliderChanged { id: "volume".into(), fraction: 0.3, committed: false };
        assert_eq!(state.update(&volume), vec![Command::Volume(0.3)]);
        assert_eq!(click(&mut state, "mute"), vec![Command::Volume(0.0)]);
        assert_eq!(click(&mut state, "mute"), vec![Command::Volume(0.8)]);
    }

    #[test]
    fn a_new_track_asks_for_its_cover_once() {
        let mut state = State::default();
        assert_eq!(state.show(playing("a")), vec![Command::FetchArtwork("https://img/a".into())]);
        assert!(state.show(playing("a")).is_empty(), "the same track again is only a position");
        assert_eq!(state.current(), Some(&track("a")));
    }

    #[test]
    fn the_queue_fetches_its_covers_only_while_it_is_open() {
        let mut state = State::default();
        let view = PlayerView { upcoming: vec![track("b"), track("c")], ..playing("a") };
        assert_eq!(state.show(view.clone()), vec![Command::FetchArtwork("https://img/a".into())]);
        let opened = click(&mut state, "queue");
        assert_eq!(opened, vec![Command::FetchArtwork("https://img/b".into()), Command::FetchArtwork("https://img/c".into())]);
        assert!(state.show(view).is_empty(), "nothing is asked for twice");
        click(&mut state, "queue");
        assert!(!state.queue_open);
    }

    #[test]
    fn the_equalizer_sliders_set_whole_decibels() {
        let mut state = State::default();
        let slide = |id: &str, fraction| UiAction::SliderChanged { id: id.into(), fraction, committed: false };
        let mut expected = EqualizerValues::FLAT;
        expected.bands_db[5] = 6.0;
        assert_eq!(state.update(&slide("equalizer:5", 0.76)), vec![Command::Equalizer(expected)]);
        expected.preamp_db = -12.0;
        assert_eq!(state.update(&slide("equalizer:preamp", 0.0)), vec![Command::Equalizer(expected)]);
        assert!(state.update(&slide("equalizer:42", 1.0)).is_empty(), "there is no such band");
        assert_eq!(click(&mut state, "equalizer-reset"), vec![Command::Equalizer(EqualizerValues::FLAT)]);
        click(&mut state, "equalizer");
        assert!(state.equalizer_open);
        click(&mut state, "equalizer-close");
        assert!(!state.equalizer_open);
    }

    #[test]
    fn the_seek_bar_previews_while_dragged_and_seeks_on_release() {
        let mut state = State::default();
        let drag = |fraction, committed| UiAction::SliderChanged { id: "seek".into(), fraction, committed };
        assert!(state.update(&drag(0.5, true)).is_empty(), "nothing to seek in");
        state.show(playing("a"));
        assert!(state.update(&drag(0.5, false)).is_empty());
        assert_eq!(state.progress(), 0.5);
        state.show(PlayerView { position: 10.0, ..playing("a") });
        assert_eq!(state.progress(), 0.5, "the thumb stays under the pointer");
        assert_eq!(state.update(&drag(0.25, true)), vec![Command::Seek(50.0)]);
        assert_eq!(state.progress(), 0.25);
    }

    #[test]
    fn the_clock_reads_like_a_player() {
        assert_eq!(clock(0.0), "0:00");
        assert_eq!(clock(59.9), "0:59");
        assert_eq!(clock(291.0), "4:51");
        assert_eq!(clock(-3.0), "0:00");
        assert_eq!(clock(f64::NAN), "0:00");
    }
}
