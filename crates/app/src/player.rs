//! The player: the queue and what the engine is doing with it. It runs on the backend's
//! thread, so the music moves on while the window is hidden and nothing repaints.

use dnm_core::{Queue, Repeat, Track};

use crate::backend::Command;

/// How long before a track ends the next one is fetched onto the standby deck.
const PRELOAD_AHEAD: f64 = 20.0;

/// What the window shows of the player.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerView {
    pub current: Option<Track>,
    /// What follows, in the order it will play.
    pub upcoming: Vec<Track>,
    /// What the user asked for, which the engine may not have reached yet.
    pub playing: bool,
    pub position: f64,
    pub duration: f64,
    pub shuffle: bool,
    pub repeat: Repeat,
    pub error: Option<String>,
}

/// What the backend does for the player.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// Start this track from the beginning, replacing whatever plays.
    Play(Track),
    /// Get this track ready on the standby deck.
    Preload(Track),
    /// Nothing follows this track: ask the catalog what would.
    Radio(Track),
    Pause,
    Resume,
    Seek(f64),
}

pub struct Player {
    queue: Queue,
    playing: bool,
    /// Whether the engine holds the current track. Not once it ended or failed.
    loaded: bool,
    position: f64,
    duration: f64,
    error: Option<String>,
    /// The track already asked onto the standby deck, so it is asked for once.
    preloading: Option<String>,
    /// Whether a queue that runs out goes on with its last track's radio.
    autoplay: bool,
    rng: fastrand::Rng,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            queue: Queue::default(),
            playing: false,
            loaded: false,
            position: 0.0,
            duration: 0.0,
            error: None,
            preloading: None,
            autoplay: true,
            rng: fastrand::Rng::default(),
        }
    }
}

impl Player {
    pub fn view(&self) -> PlayerView {
        PlayerView {
            current: self.queue.current().cloned(),
            upcoming: self.queue.upcoming().cloned().collect(),
            playing: self.playing,
            position: self.position,
            duration: self.duration,
            shuffle: self.queue.is_shuffled(),
            repeat: self.queue.repeat,
            error: self.error.clone(),
        }
    }

    fn start(&mut self, track: Option<Track>) -> Vec<Effect> {
        let Some(track) = track else { return Vec::new() };
        self.playing = true;
        self.loaded = true;
        self.position = 0.0;
        self.duration = track.duration_ms as f64 / 1000.0;
        self.error = None;
        self.preloading = None;
        // A lone track and the last of a list are followed by their radio, as long as the
        // user did not ask for the list to repeat.
        let radio = (self.autoplay && self.queue.peek_next().is_none()).then(|| Effect::Radio(track.clone()));
        std::iter::once(Effect::Play(track)).chain(radio).collect()
    }

    /// The catalog's answer to `Effect::Radio`. One that arrives after the user moved on to
    /// something else is dropped.
    pub fn radio(&mut self, seed: &str, tracks: Vec<Track>) {
        if self.is_current(seed) && self.queue.peek_next().is_none() {
            self.queue.extend(tracks, &mut self.rng);
        }
    }

    /// Moves forward `count` tracks at once.
    fn skip(&mut self, count: usize) -> Vec<Effect> {
        let mut reached = None;
        for _ in 0..count {
            match self.queue.skip_next() {
                Some(track) => reached = Some(track.clone()),
                None => break,
            }
        }
        self.start(reached)
    }

    pub fn command(&mut self, command: Command) -> Vec<Effect> {
        match command {
            Command::PlayList { tracks, index } if index < tracks.len() => {
                self.queue.start(tracks, index, &mut self.rng);
                self.start(self.queue.current().cloned())
            }
            // After the queue ended or the track failed there is nothing to resume.
            Command::PlayPause if !self.loaded => self.start(self.queue.current().cloned()),
            Command::PlayPause => {
                self.playing = !self.playing;
                vec![if self.playing { Effect::Resume } else { Effect::Pause }]
            }
            // With nothing playing there is no "after": the track simply starts.
            Command::PlayNext(track) | Command::Enqueue(track) if self.queue.current().is_none() => {
                self.queue.start(vec![track], 0, &mut self.rng);
                self.start(self.queue.current().cloned())
            }
            Command::PlayNext(track) => {
                self.queue.enqueue_next(track);
                Vec::new()
            }
            Command::Enqueue(track) => {
                self.queue.enqueue(track);
                Vec::new()
            }
            Command::Next => self.skip(1),
            Command::Jump(upcoming) => self.skip(upcoming + 1),
            // Like every player: a few seconds in, "previous" restarts the track.
            Command::Previous if self.loaded && self.position > 3.0 => {
                self.position = 0.0;
                vec![Effect::Seek(0.0)]
            }
            Command::Previous => {
                let previous = self.queue.previous().cloned();
                self.start(previous)
            }
            Command::Seek(seconds) if self.loaded => {
                self.position = seconds;
                vec![Effect::Seek(seconds)]
            }
            Command::Autoplay(on) => {
                self.autoplay = on;
                Vec::new()
            }
            Command::Shuffle => {
                self.queue.set_shuffle(!self.queue.is_shuffled(), &mut self.rng);
                Vec::new()
            }
            Command::Repeat => {
                self.queue.repeat = match self.queue.repeat {
                    Repeat::Off => Repeat::All,
                    Repeat::All => Repeat::One,
                    Repeat::One => Repeat::Off,
                };
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// Engine events arrive late: one for a track the user already left changes nothing.
    pub fn engine(&mut self, event: dnm_audio::Event) -> Vec<Effect> {
        use dnm_audio::Event;
        match event {
            Event::Position { track_id, position_sec, duration_sec } if self.is_current(&track_id) => {
                self.position = position_sec;
                if let Some(duration) = duration_sec.filter(|duration| *duration > 0.0) {
                    self.duration = duration;
                }
                self.preload()
            }
            Event::Ended { track_id } if self.is_current(&track_id) => {
                self.loaded = false;
                let next = self.queue.advance().cloned();
                if next.is_none() {
                    self.playing = false;
                    self.position = self.duration;
                }
                self.start(next)
            }
            Event::Failed { track_id, message } if self.is_current(&track_id) => {
                self.loaded = false;
                self.playing = false;
                self.error = Some(message);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn preload(&mut self) -> Vec<Effect> {
        if self.duration <= 0.0 || self.duration - self.position > PRELOAD_AHEAD {
            return Vec::new();
        }
        let Some(next) = self.queue.peek_next() else { return Vec::new() };
        // Repeat-one and a one-track list lead back to the track the active deck holds.
        if self.is_current(&next.id) || self.preloading.as_deref() == Some(next.id.as_str()) {
            return Vec::new();
        }
        self.preloading = Some(next.id.clone());
        vec![Effect::Preload(next.clone())]
    }

    fn is_current(&self, track_id: &str) -> bool {
        self.queue.current().is_some_and(|track| track.id == track_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dnm_audio::Event;

    fn track(id: &str) -> Track {
        Track {
            id: id.into(),
            title: id.into(),
            artist: "artist".into(),
            artist_id: None,
            album: None,
            album_id: None,
            artwork_url: None,
            duration_ms: 200_000,
        }
    }

    fn playing(ids: &[&str], index: usize) -> Player {
        let mut player = Player::default();
        player.command(Command::PlayList { tracks: ids.iter().map(|id| track(id)).collect(), index });
        player
    }

    fn at(id: &str, seconds: f64) -> Event {
        Event::Position { track_id: id.into(), position_sec: seconds, duration_sec: Some(200.0) }
    }

    fn ended(id: &str) -> Event {
        Event::Ended { track_id: id.into() }
    }

    #[test]
    fn a_list_plays_from_the_chosen_track_to_its_end() {
        let mut player = Player::default();
        let tracks = vec![track("a"), track("b"), track("c")];
        assert!(player.command(Command::PlayList { tracks: tracks.clone(), index: 9 }).is_empty());
        assert_eq!(player.command(Command::PlayList { tracks, index: 1 }), vec![Effect::Play(track("b"))]);
        assert_eq!(player.view().duration, 200.0);
        assert_eq!(player.view().upcoming, vec![track("c")]);
        assert_eq!(player.engine(ended("b")), vec![Effect::Play(track("c")), Effect::Radio(track("c"))]);
        assert!(player.engine(ended("c")).is_empty());
        let view = player.view();
        assert!(!view.playing && view.position == view.duration, "the queue ended on its last track");
        assert_eq!(player.command(Command::PlayPause).first(), Some(&Effect::Play(track("c"))), "play starts it again");
    }

    #[test]
    fn late_engine_events_for_another_track_are_ignored() {
        let mut player = playing(&["a", "b"], 0);
        player.command(Command::Next);
        player.engine(at("a", 99.0));
        assert_eq!(player.view().position, 0.0);
        assert!(player.engine(ended("a")).is_empty());
        player.engine(Event::Failed { track_id: "a".into(), message: "x".into() });
        let view = player.view();
        assert_eq!(view.current, Some(track("b")));
        assert!(view.playing && view.error.is_none());
    }

    #[test]
    fn a_failed_track_shows_why_and_play_tries_it_again() {
        let mut player = playing(&["a"], 0);
        player.engine(Event::Failed { track_id: "a".into(), message: "sin red".into() });
        let view = player.view();
        assert!(!view.playing);
        assert_eq!(view.error.as_deref(), Some("sin red"));
        assert!(player.command(Command::Seek(10.0)).is_empty(), "nothing is loaded to seek in");
        assert_eq!(player.command(Command::PlayPause).first(), Some(&Effect::Play(track("a"))));
        assert!(player.view().error.is_none());
    }

    #[test]
    fn previous_restarts_after_three_seconds_and_goes_back_before() {
        let mut player = playing(&["a", "b"], 1);
        player.engine(at("b", 12.0));
        assert_eq!(player.command(Command::Previous), vec![Effect::Seek(0.0)]);
        assert_eq!(player.command(Command::Previous), vec![Effect::Play(track("a"))]);
    }

    #[test]
    fn transport_follows_the_commands() {
        let mut player = Player::default();
        assert!(player.command(Command::PlayPause).is_empty(), "nothing to play yet");
        assert!(player.command(Command::Next).is_empty());
        let mut player = playing(&["a"], 0);
        assert_eq!(player.command(Command::PlayPause), vec![Effect::Pause]);
        assert!(!player.view().playing);
        assert_eq!(player.command(Command::PlayPause), vec![Effect::Resume]);
        assert_eq!(player.command(Command::Seek(50.0)), vec![Effect::Seek(50.0)]);
        assert_eq!(player.view().position, 50.0);
        player.command(Command::Shuffle);
        player.command(Command::Repeat);
        let view = player.view();
        assert!(view.shuffle && view.repeat == Repeat::All);
    }

    #[test]
    fn a_lone_track_is_followed_by_its_radio() {
        let mut player = Player::default();
        let effects = player.command(Command::PlayList { tracks: vec![track("a")], index: 0 });
        assert_eq!(effects, vec![Effect::Play(track("a")), Effect::Radio(track("a"))]);
        player.radio("other", vec![track("x")]);
        assert!(player.view().upcoming.is_empty(), "the answer to another seed is dropped");
        player.radio("a", vec![track("a"), track("b"), track("c")]);
        assert_eq!(player.view().upcoming, vec![track("b"), track("c")], "the seed is not repeated");
        player.radio("a", vec![track("d")]);
        assert_eq!(player.view().upcoming.len(), 2, "a second answer adds nothing");
        assert_eq!(player.command(Command::Jump(1)), vec![Effect::Play(track("c")), Effect::Radio(track("c"))]);
        assert!(player.command(Command::Jump(5)).is_empty(), "nowhere to jump to");
    }

    #[test]
    fn tracks_queued_by_hand_go_before_the_rest_of_the_list() {
        let mut player = Player::default();
        assert_eq!(player.command(Command::Enqueue(track("x"))).first(), Some(&Effect::Play(track("x"))), "nothing played, so it starts");
        let mut player = playing(&["a", "b"], 0);
        assert!(player.command(Command::Enqueue(track("x"))).is_empty());
        assert!(player.command(Command::Enqueue(track("y"))).is_empty());
        assert!(player.command(Command::PlayNext(track("w"))).is_empty());
        assert_eq!(player.view().upcoming, vec![track("w"), track("x"), track("y"), track("b")]);
        assert_eq!(player.command(Command::Next), vec![Effect::Play(track("w"))]);
    }

    #[test]
    fn with_autoplay_off_a_queue_that_runs_out_just_ends() {
        let mut player = Player::default();
        player.command(Command::Autoplay(false));
        assert_eq!(player.command(Command::PlayList { tracks: vec![track("a")], index: 0 }), vec![Effect::Play(track("a"))]);
        player.command(Command::Autoplay(true));
        assert_eq!(player.command(Command::PlayPause), vec![Effect::Pause]);
        assert_eq!(player.command(Command::PlayList { tracks: vec![track("a")], index: 0 }).len(), 2, "on again, the radio is asked for");
    }

    #[test]
    fn a_list_that_repeats_asks_for_no_radio() {
        let mut player = playing(&["a", "b"], 0);
        player.command(Command::Repeat);
        assert_eq!(player.command(Command::Next), vec![Effect::Play(track("b"))]);
    }

    #[test]
    fn the_next_track_is_preloaded_once_near_the_end() {
        let mut player = playing(&["a", "b"], 0);
        assert!(player.engine(at("a", 100.0)).is_empty(), "too early");
        assert_eq!(player.engine(at("a", 185.0)), vec![Effect::Preload(track("b"))]);
        assert!(player.engine(at("a", 186.0)).is_empty(), "asked once");
        assert_eq!(player.engine(ended("a")).first(), Some(&Effect::Play(track("b"))));
        assert!(player.engine(at("b", 190.0)).is_empty(), "nothing follows the last track");
        player.command(Command::Repeat);
        assert_eq!(player.engine(at("b", 191.0)), vec![Effect::Preload(track("a"))], "repeat-all wraps around");
        player.command(Command::Repeat);
        assert!(player.engine(at("b", 192.0)).is_empty(), "repeat-one replays what is already loaded");
    }
}
