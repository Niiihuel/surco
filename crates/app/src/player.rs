//! The player: the queue and what the engine is doing with it. It runs on the backend's
//! thread, so the music moves on while the window is hidden and nothing repaints.

use std::time::{Duration, Instant};

use surco_core::{Queue, Repeat, Track};

use crate::backend::Command;

/// How long before a track ends the next one is fetched onto the standby deck.
const PRELOAD_AHEAD: f64 = 20.0;

/// When the music stops by itself, as asked from the sleep timer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sleep {
    /// At this moment, the volume easing away over so long before it.
    At { ends: Instant, fade: Duration },
    /// As the song that plays ends, its last seconds fading.
    EndOfSong,
}

/// What the sleep timer is asked for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SleepAfter {
    Minutes(u32),
    EndOfSong,
}

/// How long a song's end fades when the music stops with it.
const END_FADE: f64 = 15.0;

impl Sleep {
    /// From now, as asked. A timer winds down over three minutes, or a third of a short one.
    fn from(after: SleepAfter, now: Instant) -> Self {
        match after {
            SleepAfter::Minutes(minutes) => {
                let total = Duration::from_secs(u64::from(minutes) * 60);
                Sleep::At { ends: now + total, fade: (total / 3).clamp(Duration::from_secs(5), Duration::from_secs(180)) }
            }
            SleepAfter::EndOfSong => Sleep::EndOfSong,
        }
    }
}

/// The volume at `fraction` of a wind-down: barely lower at first, half at the middle, gone
/// at its end.
fn wind_down(fraction: f64) -> f32 {
    let along = (fraction.clamp(0.0, 1.0) * std::f64::consts::FRAC_PI_2).cos();
    (along * along) as f32
}

/// What the window shows of the player.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerView {
    pub current: Option<Track>,
    /// What follows, in the order it will play.
    pub upcoming: Vec<Track>,
    /// How many of `upcoming` were queued by hand. They come first.
    pub queued: usize,
    /// What the user asked for, which the engine may not have reached yet.
    pub playing: bool,
    pub position: f64,
    pub duration: f64,
    pub shuffle: bool,
    /// Shuffled, with the catalog's picks woven in.
    pub smart: bool,
    /// The ids of the tracks that came into the list from the catalog.
    pub picks: Vec<String>,
    pub repeat: Repeat,
    pub error: Option<String>,
    pub sleep: Option<Sleep>,
}

/// What the backend does for the player.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// Start this track from the beginning, replacing whatever plays.
    Play(Track),
    /// Bring this track in over so many seconds, under the end of the one that plays.
    Fade(Track, f64),
    /// Load this track again and go on from so many seconds in, sounding or not.
    Restore(Track, f64, bool),
    /// Bring this rendition of what plays in under it, from where it is so many seconds in:
    /// what plays goes on sounding until the other is ready.
    Handover(Track, f64),
    /// Get this track ready on the standby deck.
    Preload(Track),
    /// Nothing follows this track: ask the catalog what would.
    Radio(Track),
    Pause,
    Resume,
    Seek(f64),
    /// The volume, times this: 1 is the user's own, lower while the sleep timer winds down.
    Dim(f32),
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
    /// The rendition another one is taking over from: until that one sounds, this one
    /// still says where the song is.
    outgoing: Option<String>,
    /// Whether a queue that runs out goes on with its last track's radio.
    autoplay: bool,
    /// Whether the catalog's picks for what plays are woven into the shuffled list.
    smart: bool,
    /// Seconds a track that plays to its end shares with the next. 0 hands over in one step.
    crossfade: f64,
    rng: fastrand::Rng,
    /// Where a session brought back from the last time goes on from, once it is played.
    resume_at: Option<f64>,
    sleep: Option<Sleep>,
    /// Whether the volume was turned down for the sleep timer, to be given back.
    dimmed: bool,
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
            outgoing: None,
            autoplay: true,
            smart: false,
            crossfade: 0.0,
            rng: fastrand::Rng::default(),
            resume_at: None,
            sleep: None,
            dimmed: false,
        }
    }
}

impl Player {
    pub fn view(&self) -> PlayerView {
        PlayerView {
            current: self.queue.current().cloned(),
            upcoming: self.queue.upcoming().cloned().collect(),
            queued: self.queue.queued(),
            playing: self.playing,
            position: self.position,
            duration: self.duration,
            shuffle: self.queue.is_shuffled(),
            smart: self.smart,
            picks: self.queue.guests().iter().map(|track| track.id.clone()).collect(),
            repeat: self.queue.repeat,
            error: self.error.clone(),
            sleep: self.sleep,
        }
    }

    fn start(&mut self, track: Option<Track>) -> Vec<Effect> {
        self.begin(track, 0.0)
    }

    fn begin(&mut self, track: Option<Track>, fade: f64) -> Vec<Effect> {
        let Some(track) = track else { return Vec::new() };
        self.resume_at = None;
        self.playing = true;
        self.loaded = true;
        self.position = 0.0;
        self.duration = track.duration_ms as f64 / 1000.0;
        self.error = None;
        self.preloading = None;
        self.outgoing = None;
        // A lone track and the last of a list are followed by their radio, as long as the
        // user did not ask for the list to repeat. A list still to be woven asks for its picks.
        let ends = self.autoplay && self.queue.peek_next().is_none();
        let radio = (ends || (self.smart && !self.queue.is_woven())).then(|| Effect::Radio(track.clone()));
        let play = if fade > 0.0 { Effect::Fade(track, fade) } else { Effect::Play(track) };
        std::iter::once(play).chain(radio).collect()
    }

    /// The engine lost its decks, as when its output changes: the current track goes back
    /// in where it was.
    pub fn reload(&mut self) -> Vec<Effect> {
        self.preloading = None;
        let current = self.queue.current().filter(|_| self.loaded);
        current.map(|track| Effect::Restore(track.clone(), self.position, self.playing)).into_iter().collect()
    }

    /// The catalog's answer to `Effect::Radio`. One that arrives after the user moved on to
    /// something else is dropped.
    pub fn radio(&mut self, seed: &str, tracks: Vec<Track>) {
        if !self.is_current(seed) {
            return;
        }
        if self.queue.peek_next().is_none() {
            if self.autoplay {
                self.queue.extend(tracks, &mut self.rng);
            }
        } else if self.smart && !self.queue.is_woven() {
            self.queue.weave(tracks);
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
            // Held where the last time left it, there is nothing in the engine yet. As the
            // engine reloads a track it lost, it goes in where it was, and sounds.
            Command::Session { tracks, position } if self.queue.current().is_none() && !tracks.is_empty() => {
                self.queue.start(tracks, 0, &mut self.rng);
                let duration = self.queue.current().map_or(0.0, |track| track.duration_ms as f64 / 1000.0);
                self.duration = duration;
                // A track heard to its end starts again.
                self.position = if position >= duration - 2.0 { 0.0 } else { position.max(0.0) };
                self.resume_at = Some(self.position);
                Vec::new()
            }
            Command::PlayPause if !self.loaded && self.resume_at.is_some() => {
                let at = self.resume_at.unwrap_or(0.0);
                let mut effects = self.start(self.queue.current().cloned());
                if let Some(Effect::Play(track)) = effects.first().cloned() {
                    self.position = at;
                    effects[0] = Effect::Restore(track, at, true);
                }
                effects
            }
            // After the queue ended or the track failed there is nothing to resume.
            Command::PlayPause if !self.loaded => self.start(self.queue.current().cloned()),
            Command::PlayPause => {
                self.playing = !self.playing;
                // A timer that ran out while paused is done with.
                if self.playing && matches!(self.sleep, Some(Sleep::At { ends, .. }) if ends <= Instant::now()) {
                    self.sleep = None;
                }
                vec![if self.playing { Effect::Resume } else { Effect::Pause }]
            }
            Command::Sleep(after) => {
                self.sleep = after.map(|after| Sleep::from(after, Instant::now()));
                self.undim()
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
            Command::Unqueue(upcoming) => {
                self.queue.remove_upcoming(upcoming);
                Vec::new()
            }
            Command::UnqueueIfUnchanged { index, snapshot } => {
                if self.view().upcoming == snapshot { self.queue.remove_upcoming(index); }
                Vec::new()
            }
            Command::MoveQueue(from, to) => {
                self.queue.move_upcoming(from, to);
                Vec::new()
            }
            Command::ClearQueue => {
                self.queue.clear_up_next();
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
            // Not yet played, a session goes on from wherever it is moved to.
            Command::Seek(seconds) if !self.loaded && self.resume_at.is_some() => {
                self.position = seconds.clamp(0.0, self.duration);
                self.resume_at = Some(self.position);
                Vec::new()
            }
            Command::Seek(seconds) if self.loaded => {
                self.position = seconds;
                vec![Effect::Seek(seconds)]
            }
            Command::Swap(track) if self.queue.current().is_some() => {
                let sounding = self.loaded && self.playing;
                self.outgoing = self.queue.current().filter(|_| sounding).map(|current| current.id.clone());
                self.queue.replace_current(track.clone());
                self.duration = track.duration_ms as f64 / 1000.0;
                self.position = self.position.min(self.duration);
                self.loaded = true;
                self.preloading = None;
                // What sounds is not cut for the other to load; what is paused has nothing to cut.
                vec![if sounding { Effect::Handover(track, self.position) } else { Effect::Restore(track, self.position, self.playing) }]
            }
            Command::Autoplay(on) => {
                self.autoplay = on;
                Vec::new()
            }
            Command::Crossfade(seconds) => {
                self.crossfade = seconds;
                Vec::new()
            }
            // Off, shuffled, shuffled with the catalog's picks woven in, and off again.
            Command::Shuffle => {
                if !self.queue.is_shuffled() {
                    self.queue.set_shuffle(true, &mut self.rng);
                } else if !self.smart {
                    self.smart = true;
                    return self.queue.current().cloned().map(Effect::Radio).into_iter().collect();
                } else {
                    self.smart = false;
                    self.queue.unweave();
                    self.queue.set_shuffle(false, &mut self.rng);
                }
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
    pub fn engine(&mut self, event: surco_audio::Event) -> Vec<Effect> {
        use surco_audio::Event;
        match event {
            Event::Position { track_id, position_sec, .. } if self.outgoing.as_deref() == Some(track_id.as_str()) => {
                self.position = position_sec;
                Vec::new()
            }
            Event::Position { track_id, position_sec, duration_sec } if self.is_current(&track_id) => {
                self.outgoing = None;
                self.position = position_sec;
                if let Some(duration) = duration_sec.filter(|duration| *duration > 0.0) {
                    self.duration = duration;
                }
                match self.sleep {
                    Some(sleep) => self.wind(sleep, Instant::now()),
                    None => self.fade().unwrap_or_else(|| self.preload()),
                }
            }
            // The song the music was to stop with: the next waits, paused, for play.
            Event::Ended { track_id } if self.is_current(&track_id) && self.sleep == Some(Sleep::EndOfSong) => {
                self.loaded = false;
                self.playing = false;
                self.sleep = None;
                if let Some(next) = self.queue.advance().cloned() {
                    self.position = 0.0;
                    self.duration = next.duration_ms as f64 / 1000.0;
                } else {
                    self.position = self.duration;
                }
                self.undim()
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
            // The music goes on where it was, on the device the engine moved to.
            Event::Rerouted => self.reload(),
            Event::Failed { track_id, message } if self.is_current(&track_id) => {
                self.loaded = false;
                self.playing = false;
                self.error = Some(message);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// The sleep timer at `now`, on each tick of what plays: the volume eases away before its
    /// end, and at its end the music pauses. A timer that stops with the song lets nothing
    /// come in under it.
    fn wind(&mut self, sleep: Sleep, now: Instant) -> Vec<Effect> {
        let fraction = match sleep {
            Sleep::At { ends, fade } => {
                if ends <= now {
                    self.playing = false;
                    self.sleep = None;
                    let mut effects = vec![Effect::Pause];
                    effects.extend(self.undim());
                    return effects;
                }
                let left = ends - now;
                if left >= fade { return self.fade().unwrap_or_else(|| self.preload()) }
                1.0 - left.as_secs_f64() / fade.as_secs_f64()
            }
            Sleep::EndOfSong => {
                let left = self.duration - self.position;
                if self.duration <= 0.0 || left >= END_FADE { return Vec::new() }
                1.0 - left / END_FADE
            }
        };
        self.dimmed = true;
        vec![Effect::Dim(wind_down(fraction))]
    }

    /// The user's own volume back, if the sleep timer had turned it down.
    fn undim(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.dimmed).then_some(Effect::Dim(1.0)).into_iter().collect()
    }

    /// With a crossfade set, the next track comes in under the last seconds of this one.
    fn fade(&mut self) -> Option<Vec<Effect>> {
        // A track shorter than two fades would spend all of itself fading.
        if self.crossfade <= 0.0 || self.duration < self.crossfade * 2.0 || self.duration - self.position > self.crossfade {
            return None;
        }
        // Only onto what the standby deck was asked for: anything else cuts this track short.
        let asked = self.queue.peek_next().is_some_and(|next| self.preloading.as_deref() == Some(next.id.as_str()));
        if !asked {
            return None;
        }
        let next = self.queue.advance().cloned();
        Some(self.begin(next, self.crossfade))
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
    use surco_audio::Event;

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
    fn the_sleep_timer_winds_the_volume_down_and_pauses_at_its_end() {
        let mut player = playing(&["a", "b"], 0);
        player.command(Command::Sleep(Some(SleepAfter::Minutes(30))));
        let Some(Sleep::At { ends, fade }) = player.view().sleep else { panic!("a timer") };
        assert_eq!(fade, Duration::from_secs(180), "three minutes, for a long timer");
        assert!(!player.wind(Sleep::At { ends, fade }, ends - Duration::from_secs(600)).iter().any(|effect| matches!(effect, Effect::Dim(_))), "untouched before its last minutes");
        let half = player.wind(Sleep::At { ends, fade }, ends - Duration::from_secs(90));
        assert!(matches!(half.as_slice(), [Effect::Dim(dim)] if (dim - 0.5).abs() < 0.01), "half way down at the middle: {half:?}");
        assert_eq!(player.wind(Sleep::At { ends, fade }, ends), vec![Effect::Pause, Effect::Dim(1.0)], "paused, and the volume given back");
        assert!(!player.view().playing && player.view().sleep.is_none());
        // A short timer winds down over a third of itself; put away, the volume comes back.
        let mut player = playing(&["a"], 0);
        player.command(Command::Sleep(Some(SleepAfter::Minutes(6))));
        let Some(Sleep::At { ends, fade }) = player.view().sleep else { panic!("a timer") };
        assert_eq!(fade, Duration::from_secs(120));
        player.wind(Sleep::At { ends, fade }, ends - Duration::from_secs(10));
        assert_eq!(player.command(Command::Sleep(None)), vec![Effect::Dim(1.0)]);
    }

    #[test]
    fn stopping_with_the_song_fades_its_end_and_leaves_the_next_waiting() {
        let mut player = playing(&["a", "b"], 0);
        player.command(Command::Sleep(Some(SleepAfter::EndOfSong)));
        assert!(player.engine(at("a", 100.0)).is_empty(), "nothing preloaded or faded in under it");
        assert!(matches!(player.engine(at("a", 192.5)).as_slice(), [Effect::Dim(dim)] if (dim - 0.5).abs() < 0.01));
        assert_eq!(player.engine(ended("a")), vec![Effect::Dim(1.0)], "it stops there, its volume back");
        let view = player.view();
        assert!(!view.playing && view.sleep.is_none() && view.current == Some(track("b")) && view.position == 0.0);
        assert_eq!(player.command(Command::PlayPause).first(), Some(&Effect::Play(track("b"))), "play goes on with the next");
    }

    #[test]
    fn a_session_from_the_last_time_waits_paused_and_goes_on_from_where_it_was() {
        let mut player = Player::default();
        let session = Command::Session { tracks: vec![track("a"), track("b")], position: 73.0 };
        assert!(player.command(session).is_empty(), "nothing is fetched before it is played");
        let view = player.view();
        assert_eq!((view.current, view.upcoming, view.position, view.playing), (Some(track("a")), vec![track("b")], 73.0, false));
        // Moved before it plays, it goes on from there.
        assert!(player.command(Command::Seek(80.0)).is_empty());
        assert_eq!(player.command(Command::PlayPause), vec![Effect::Restore(track("a"), 80.0, true)]);
        assert!(player.view().playing && player.view().position == 80.0);
        // Only once: a second session does not replace what is in the queue now.
        assert!(player.command(Command::Session { tracks: vec![track("z")], position: 1.0 }).is_empty());
        assert_eq!(player.view().current, Some(track("a")));
        // Something else chosen meanwhile starts from its beginning.
        let mut player = Player::default();
        player.command(Command::Session { tracks: vec![track("a")], position: 50.0 });
        assert_eq!(player.command(Command::PlayList { tracks: vec![track("c")], index: 0 }).first(), Some(&Effect::Play(track("c"))));
        player.engine(ended("c"));
        assert_eq!(player.command(Command::PlayPause).first(), Some(&Effect::Play(track("c"))), "the session's place is forgotten");
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
    fn an_open_sheet_removes_the_selected_duplicate_only_from_its_original_queue() {
        let mut player = playing(&["current", "a", "b", "a"], 0);
        let snapshot = player.view().upcoming;
        player.command(Command::UnqueueIfUnchanged { index: 2, snapshot });
        assert_eq!(player.view().upcoming, vec![track("a"), track("b")]);
        let snapshot = player.view().upcoming;
        player.command(Command::Next);
        let changed = player.view().upcoming;
        player.command(Command::UnqueueIfUnchanged { index: 0, snapshot });
        assert_eq!(player.view().upcoming, changed, "late UI commands cannot remove another queue entry");
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
    fn shuffle_steps_through_off_shuffled_and_shuffled_with_picks() {
        let mut player = playing(&["a", "b", "c", "d", "e"], 0);
        assert!(player.command(Command::Shuffle).is_empty());
        assert!(player.view().shuffle && !player.view().smart);
        assert_eq!(player.command(Command::Shuffle), vec![Effect::Radio(track("a"))], "the picks for what plays are asked for");
        player.radio("a", vec![track("x"), track("y")]);
        let view = player.view();
        assert!(view.smart && view.picks == ["x"] && view.upcoming.len() == 5, "one after three of its own: {:?}", view.picks);
        // A list started while it is on asks for picks of its own.
        let list = vec![track("p"), track("q")];
        assert_eq!(player.command(Command::PlayList { tracks: list, index: 0 }), vec![Effect::Play(track("p")), Effect::Radio(track("p"))]);
        assert!(player.command(Command::Shuffle).is_empty());
        let view = player.view();
        assert!(!view.shuffle && !view.smart && view.picks.is_empty());
    }

    #[test]
    fn another_rendition_takes_the_place_of_what_plays_and_goes_on_from_there() {
        let mut player = playing(&["a", "b"], 0);
        player.engine(Event::Position { track_id: "a".into(), position_sec: 42.0, duration_sec: Some(200.0) });
        assert_eq!(player.command(Command::Swap(track("video"))), vec![Effect::Handover(track("video"), 42.0)]);
        let view = player.view();
        assert_eq!((view.current, view.upcoming), (Some(track("video")), vec![track("b")]));
        // Until the other sounds, the one on its way out still says where the song is.
        player.engine(Event::Position { track_id: "a".into(), position_sec: 43.5, duration_sec: Some(200.0) });
        assert_eq!(player.view().position, 43.5);
        player.engine(Event::Position { track_id: "video".into(), position_sec: 44.0, duration_sec: None });
        player.engine(Event::Position { track_id: "a".into(), position_sec: 99.0, duration_sec: None });
        assert_eq!(player.view().position, 44.0, "and no longer once it does");
        // Paused, there is nothing to keep sounding: the other is loaded where the song was.
        player.command(Command::PlayPause);
        assert_eq!(player.command(Command::Swap(track("a"))), vec![Effect::Restore(track("a"), 44.0, false)]);
        assert!(Player::default().command(Command::Swap(track("video"))).is_empty(), "nothing plays");
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
    fn a_crossfade_brings_the_next_track_in_before_this_one_ends() {
        let at = |position_sec| Event::Position { track_id: "a".into(), position_sec, duration_sec: Some(200.0) };
        let mut player = playing(&["a", "b"], 0);
        player.command(Command::Crossfade(5.0));
        assert_eq!(player.engine(at(194.0)), vec![Effect::Preload(track("b"))], "outside the fade it only gets ready");
        assert_eq!(player.engine(at(195.5)).first(), Some(&Effect::Fade(track("b"), 5.0)));
        assert!(player.engine(at(196.0)).is_empty(), "the outgoing track is no longer the current one");
        assert_eq!(player.view().current, Some(track("b")));

        let mut plain = playing(&["a", "b"], 0);
        plain.engine(at(194.0));
        assert!(plain.engine(at(195.5)).is_empty(), "with none set the track plays to its end");
    }

    #[test]
    fn a_reload_puts_the_current_track_back_where_it_was() {
        let mut player = playing(&["a", "b"], 0);
        assert!(Player::default().reload().is_empty(), "nothing was loaded");
        player.engine(Event::Position { track_id: "a".into(), position_sec: 42.0, duration_sec: Some(200.0) });
        player.command(Command::PlayPause);
        assert_eq!(player.reload(), vec![Effect::Restore(track("a"), 42.0, false)]);
        player.command(Command::PlayPause);
        assert_eq!(player.engine(Event::Rerouted), vec![Effect::Restore(track("a"), 42.0, true)], "a new device picks up where the old one was");
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
