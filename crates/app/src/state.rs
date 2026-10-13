//! Everything the window shows, and the two ways it changes: an action from the user
//! (`update`) or an answer from the backend (`apply`). Both return the commands to send.

use std::collections::HashMap;

use dnm_core::{Queue, Repeat, Track};
use limn::{RasterImage, UiAction};

use crate::backend::{Command, Event};

pub struct State {
    pub query: String,
    /// The query whose results are shown or awaited. Answers for any other are stale.
    pub submitted: Option<String>,
    pub searching: bool,
    pub results: Vec<Track>,
    pub search_error: Option<String>,
    pub queue: Queue,
    /// What the user asked for, which the engine may not have reached yet.
    pub playing: bool,
    pub position: f64,
    pub duration: f64,
    /// Where the seek thumb is while it is being dragged.
    pub seek_preview: Option<f32>,
    pub volume: f32,
    pub artwork: HashMap<String, RasterImage>,
    pub playback_error: Option<String>,
    rng: fastrand::Rng,
}

impl Default for State {
    fn default() -> Self {
        Self {
            query: String::new(),
            submitted: None,
            searching: false,
            results: Vec::new(),
            search_error: None,
            queue: Queue::default(),
            playing: false,
            position: 0.0,
            duration: 0.0,
            seek_preview: None,
            volume: 0.8,
            artwork: HashMap::new(),
            playback_error: None,
            rng: fastrand::Rng::new(),
        }
    }
}

impl State {
    pub fn current(&self) -> Option<&Track> {
        self.queue.current()
    }

    /// Share of the track that has played, following the thumb while it is dragged.
    pub fn progress(&self) -> f32 {
        self.seek_preview.unwrap_or_else(|| {
            if self.duration > 0.0 { (self.position / self.duration).clamp(0.0, 1.0) as f32 } else { 0.0 }
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

    fn start(&mut self, track: Option<Track>) -> Vec<Command> {
        let Some(track) = track else { return Vec::new() };
        self.playing = true;
        self.position = 0.0;
        self.duration = track.duration_ms as f64 / 1000.0;
        self.seek_preview = None;
        self.playback_error = None;
        let mut commands = Vec::new();
        commands.extend(self.artwork_request(&track));
        commands.push(Command::Play(track));
        commands
    }

    fn artwork_request(&self, track: &Track) -> Option<Command> {
        let url = track.artwork_url.as_ref()?;
        (!self.artwork.contains_key(url)).then(|| Command::FetchArtwork(url.clone()))
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
                if self.current().is_none() || self.duration <= 0.0 {
                    return Vec::new();
                }
                self.position = f64::from(*fraction) * self.duration;
                vec![Command::Seek(self.position)]
            }
            UiAction::SliderChanged { id, fraction, .. } if id == "volume" => {
                self.volume = *fraction;
                vec![Command::Volume(self.volume)]
            }
            UiAction::Clicked { id } => self.click(id),
            _ => Vec::new(),
        }
    }

    fn click(&mut self, id: &str) -> Vec<Command> {
        if let Some(index) = id.strip_prefix("track:").and_then(|i| i.parse::<usize>().ok()) {
            if index >= self.results.len() {
                return Vec::new();
            }
            self.queue.start(self.results.clone(), index, &mut self.rng);
            return self.start(self.current().cloned());
        }
        match id {
            "play-pause" => {
                if self.current().is_none() {
                    return Vec::new();
                }
                self.playing = !self.playing;
                vec![if self.playing { Command::Resume } else { Command::Pause }]
            }
            "next" => {
                let next = self.queue.skip_next().cloned();
                self.start(next)
            }
            "previous" => {
                // Like every player: a few seconds in, "previous" restarts the track.
                if self.position > 3.0 {
                    self.position = 0.0;
                    return vec![Command::Seek(0.0)];
                }
                let previous = self.queue.previous().cloned();
                self.start(previous)
            }
            "shuffle" => {
                let on = !self.queue.is_shuffled();
                self.queue.set_shuffle(on, &mut self.rng);
                Vec::new()
            }
            "repeat" => {
                self.queue.repeat = match self.queue.repeat {
                    Repeat::Off => Repeat::All,
                    Repeat::All => Repeat::One,
                    Repeat::One => Repeat::Off,
                };
                Vec::new()
            }
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
                let mut urls: Vec<&String> =
                    self.results.iter().filter_map(|t| t.artwork_url.as_ref()).filter(|u| !self.artwork.contains_key(*u)).collect();
                urls.sort();
                urls.dedup();
                urls.into_iter().map(|url| Command::FetchArtwork(url.clone())).collect()
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
            Event::Position { track_id, seconds, duration } if self.is_current(&track_id) => {
                self.position = seconds;
                if duration > 0.0 {
                    self.duration = duration;
                }
                Vec::new()
            }
            Event::Ended { track_id } if self.is_current(&track_id) => {
                let next = self.queue.advance().cloned();
                if next.is_none() {
                    self.playing = false;
                    self.position = self.duration;
                }
                self.start(next)
            }
            Event::PlaybackFailed { track_id, message } if self.is_current(&track_id) => {
                self.playing = false;
                self.playback_error = Some(message);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn is_current(&self, track_id: &str) -> bool {
        self.current().is_some_and(|t| t.id == track_id)
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

    fn searched(ids: &[&str]) -> State {
        let mut state = State { query: "q".into(), ..State::default() };
        state.search();
        state.apply(Event::SearchResults { query: "q".into(), tracks: ids.iter().map(|id| track(id)).collect() });
        state
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
    fn clicking_a_result_plays_it_and_the_rest_follow() {
        let mut state = searched(&["a", "b", "c"]);
        let commands = click(&mut state, "track:1");
        assert_eq!(commands.last(), Some(&Command::Play(track("b"))));
        assert!(state.playing);
        assert_eq!(state.duration, 200.0);
        let commands = state.apply(Event::Ended { track_id: "b".into() });
        assert_eq!(commands.last(), Some(&Command::Play(track("c"))));
        assert!(state.apply(Event::Ended { track_id: "c".into() }).is_empty());
        assert!(!state.playing, "the queue ended");
        assert!(click(&mut state, "track:9").is_empty());
    }

    #[test]
    fn late_engine_events_for_another_track_are_ignored() {
        let mut state = searched(&["a", "b"]);
        click(&mut state, "track:0");
        click(&mut state, "next");
        state.apply(Event::Position { track_id: "a".into(), seconds: 99.0, duration: 100.0 });
        assert_eq!(state.position, 0.0);
        assert!(state.apply(Event::Ended { track_id: "a".into() }).is_empty());
        assert_eq!(state.current().map(|t| t.id.as_str()), Some("b"));
        state.apply(Event::PlaybackFailed { track_id: "a".into(), message: "x".into() });
        assert!(state.playback_error.is_none() && state.playing);
    }

    #[test]
    fn the_seek_bar_previews_while_dragged_and_seeks_on_release() {
        let mut state = searched(&["a"]);
        click(&mut state, "track:0");
        let drag = |fraction, committed| UiAction::SliderChanged { id: "seek".into(), fraction, committed };
        assert!(state.update(&drag(0.5, false)).is_empty());
        assert_eq!(state.progress(), 0.5);
        state.apply(Event::Position { track_id: "a".into(), seconds: 10.0, duration: 200.0 });
        assert_eq!(state.progress(), 0.5, "the thumb stays under the pointer");
        assert_eq!(state.update(&drag(0.25, true)), vec![Command::Seek(50.0)]);
        assert_eq!(state.progress(), 0.25);
    }

    #[test]
    fn previous_restarts_after_three_seconds_and_goes_back_before() {
        let mut state = searched(&["a", "b"]);
        click(&mut state, "track:1");
        state.apply(Event::Position { track_id: "b".into(), seconds: 12.0, duration: 200.0 });
        assert_eq!(click(&mut state, "previous"), vec![Command::Seek(0.0)]);
        assert_eq!(click(&mut state, "previous").last(), Some(&Command::Play(track("a"))));
    }

    #[test]
    fn transport_and_volume_send_the_matching_commands() {
        let mut state = State::default();
        assert!(click(&mut state, "play-pause").is_empty(), "nothing to play yet");
        let mut state = searched(&["a"]);
        click(&mut state, "track:0");
        assert_eq!(click(&mut state, "play-pause"), vec![Command::Pause]);
        assert_eq!(click(&mut state, "play-pause"), vec![Command::Resume]);
        let volume = UiAction::SliderChanged { id: "volume".into(), fraction: 0.3, committed: false };
        assert_eq!(state.update(&volume), vec![Command::Volume(0.3)]);
        assert_eq!(click(&mut state, "mute"), vec![Command::Volume(0.0)]);
        assert_eq!(click(&mut state, "mute"), vec![Command::Volume(0.8)]);
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
