use std::collections::VecDeque;

use crate::Track;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Repeat {
    #[default]
    Off,
    /// After the last track of the list comes the first.
    All,
    /// The current track restarts until this is turned off.
    One,
}

/// What is playing and what follows.
///
/// Two sources feed it: the `list` the user started (an album, a playlist, a search) and
/// `up_next`, tracks queued by hand. Hand-queued tracks always go first, and while one of
/// them plays the position in the list does not move, so the list resumes where it was.
#[derive(Clone, Debug, Default)]
pub struct Queue {
    list: Vec<Track>,
    /// Position in `list`. `None` before anything was started.
    index: Option<usize>,
    /// The hand-queued track that is playing instead of `list[index]`.
    manual: Option<Track>,
    up_next: VecDeque<Track>,
    /// Play order over `list` when shuffling: a permutation of its indices.
    shuffle: Option<Vec<usize>>,
    /// What played before each hand-queued track, so `previous` can walk back through them.
    history: Vec<Track>,
    pub repeat: Repeat,
}

impl Queue {
    /// Replaces the list and starts at `index`. Hand-queued tracks and history belong to the
    /// old list and are dropped; shuffle stays on as a preference and is re-dealt.
    pub fn start(&mut self, list: Vec<Track>, index: usize, rng: &mut fastrand::Rng) {
        if index >= list.len() {
            return;
        }
        let shuffled = self.shuffle.is_some();
        self.list = list;
        self.index = Some(index);
        self.manual = None;
        self.up_next.clear();
        self.history.clear();
        self.shuffle = shuffled.then(|| deal(self.list.len(), index, rng));
    }

    pub fn current(&self) -> Option<&Track> {
        self.manual.as_ref().or_else(|| self.index.and_then(|i| self.list.get(i)))
    }

    pub fn list(&self) -> &[Track] {
        &self.list
    }

    pub fn up_next(&self) -> impl Iterator<Item = &Track> {
        self.up_next.iter()
    }

    pub fn is_shuffled(&self) -> bool {
        self.shuffle.is_some()
    }

    /// The current track stays first; the rest of the list is dealt after it.
    pub fn set_shuffle(&mut self, on: bool, rng: &mut fastrand::Rng) {
        self.shuffle = match (on, self.index) {
            (true, Some(index)) => Some(deal(self.list.len(), index, rng)),
            (true, None) => Some(Vec::new()),
            (false, _) => None,
        };
    }

    /// Appends to the list being played, keeping the shuffle a permutation.
    pub fn extend(&mut self, tracks: Vec<Track>, rng: &mut fastrand::Rng) {
        let from = self.list.len();
        let fresh: Vec<Track> =
            tracks.into_iter().filter(|t| !self.list.iter().any(|known| known.id == t.id)).collect();
        if let Some(order) = &mut self.shuffle {
            let mut added: Vec<usize> = (from..from + fresh.len()).collect();
            rng.shuffle(&mut added);
            order.extend(added);
        }
        self.list.extend(fresh);
    }

    pub fn enqueue(&mut self, track: Track) {
        self.up_next.push_back(track);
    }

    pub fn enqueue_next(&mut self, track: Track) {
        self.up_next.push_front(track);
    }

    /// What `advance` would play, without moving. The engine preloads this.
    pub fn peek_next(&self) -> Option<&Track> {
        if self.repeat == Repeat::One {
            return self.current();
        }
        if let Some(track) = self.up_next.front() {
            return Some(track);
        }
        self.next_index().and_then(|i| self.list.get(i))
    }

    /// Moves to the next track and returns it. `None` means the queue ended and nothing changed.
    pub fn advance(&mut self) -> Option<&Track> {
        if self.repeat == Repeat::One && self.current().is_some() {
            return self.current();
        }
        self.skip_next()
    }

    /// The "next" button: like `advance`, but never held back by repeat-one.
    pub fn skip_next(&mut self) -> Option<&Track> {
        if let Some(track) = self.up_next.pop_front() {
            if let Some(previous) = self.current().cloned() {
                self.history.push(previous);
            }
            self.manual = Some(track);
            return self.current();
        }
        let next = self.next_index()?;
        self.manual = None;
        self.index = Some(next);
        self.current()
    }

    /// Moves to the previous track and returns it. `None` when there is nowhere to go back to.
    pub fn previous(&mut self) -> Option<&Track> {
        if let Some(manual) = self.manual.take() {
            // The way back mirrors the way in: the track left behind is next again.
            self.up_next.push_front(manual);
            match self.history.pop() {
                Some(before) if self.index.and_then(|i| self.list.get(i)) != Some(&before) => {
                    self.manual = Some(before);
                }
                _ => {}
            }
            return self.current();
        }
        let previous = self.previous_index()?;
        self.index = Some(previous);
        self.current()
    }

    fn position(&self) -> Option<usize> {
        let index = self.index?;
        match &self.shuffle {
            Some(order) => order.iter().position(|&i| i == index),
            None => Some(index),
        }
    }

    fn at(&self, position: usize) -> Option<usize> {
        match &self.shuffle {
            Some(order) => order.get(position).copied(),
            None => (position < self.list.len()).then_some(position),
        }
    }

    fn next_index(&self) -> Option<usize> {
        let position = self.position()?;
        self.at(position + 1).or_else(|| (self.repeat == Repeat::All).then(|| self.at(0)).flatten())
    }

    fn previous_index(&self) -> Option<usize> {
        let position = self.position()?;
        position.checked_sub(1).and_then(|p| self.at(p))
    }
}

/// A random order over `len` indices that starts at `first`.
fn deal(len: usize, first: usize, rng: &mut fastrand::Rng) -> Vec<usize> {
    let mut rest: Vec<usize> = (0..len).filter(|&i| i != first).collect();
    rng.shuffle(&mut rest);
    std::iter::once(first).chain(rest).collect()
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
            artwork_url: None,
            duration_ms: 180_000,
        }
    }

    fn tracks(ids: &[&str]) -> Vec<Track> {
        ids.iter().map(|id| track(id)).collect()
    }

    fn id(track: Option<&Track>) -> Option<&str> {
        track.map(|t| t.id.as_str())
    }

    fn rng() -> fastrand::Rng {
        fastrand::Rng::with_seed(7)
    }

    #[test]
    fn plays_the_list_in_order_and_stops_at_the_end() {
        let mut q = Queue::default();
        q.start(tracks(&["a", "b", "c"]), 1, &mut rng());
        assert_eq!(id(q.current()), Some("b"));
        assert_eq!(id(q.peek_next()), Some("c"));
        assert_eq!(id(q.advance()), Some("c"));
        assert_eq!(id(q.advance()), None);
        assert_eq!(id(q.current()), Some("c"), "ending leaves the last track in place");
        assert_eq!(id(q.previous()), Some("b"));
        assert_eq!(id(q.previous()), Some("a"));
        assert_eq!(id(q.previous()), None);
    }

    #[test]
    fn hand_queued_tracks_go_first_and_the_list_resumes_where_it_was() {
        let mut q = Queue::default();
        q.start(tracks(&["a", "b", "c"]), 0, &mut rng());
        q.enqueue(track("x"));
        q.enqueue(track("y"));
        q.enqueue_next(track("w"));
        assert_eq!(id(q.peek_next()), Some("w"));
        assert_eq!(id(q.advance()), Some("w"));
        assert_eq!(id(q.advance()), Some("x"));
        assert_eq!(id(q.advance()), Some("y"));
        assert_eq!(id(q.advance()), Some("b"), "the list continues after the track it was on");
    }

    #[test]
    fn previous_walks_back_through_hand_queued_tracks_the_way_it_came() {
        let mut q = Queue::default();
        q.start(tracks(&["a", "b"]), 0, &mut rng());
        q.enqueue(track("x"));
        q.enqueue(track("y"));
        q.advance();
        q.advance();
        assert_eq!(id(q.current()), Some("y"));
        assert_eq!(id(q.previous()), Some("x"));
        assert_eq!(id(q.previous()), Some("a"));
        assert_eq!(id(q.advance()), Some("x"), "going forward again replays the same path");
        assert_eq!(id(q.advance()), Some("y"));
        assert_eq!(id(q.advance()), Some("b"));
    }

    #[test]
    fn repeat_all_wraps_and_repeat_one_holds_until_skipped() {
        let mut q = Queue::default();
        q.start(tracks(&["a", "b"]), 1, &mut rng());
        q.repeat = Repeat::All;
        assert_eq!(id(q.advance()), Some("a"));
        q.repeat = Repeat::One;
        assert_eq!(id(q.peek_next()), Some("a"));
        assert_eq!(id(q.advance()), Some("a"));
        assert_eq!(id(q.skip_next()), Some("b"), "the next button is not held by repeat-one");
    }

    #[test]
    fn shuffle_keeps_the_current_track_first_and_visits_every_track_once() {
        let mut q = Queue::default();
        let ids = ["a", "b", "c", "d", "e", "f"];
        q.start(tracks(&ids), 3, &mut rng());
        q.set_shuffle(true, &mut rng());
        assert_eq!(id(q.current()), Some("d"));
        let mut seen = vec!["d".to_string()];
        while let Some(t) = q.advance() {
            seen.push(t.id.clone());
        }
        assert_eq!(seen.len(), ids.len());
        seen.sort();
        assert_eq!(seen, ids);
        q.set_shuffle(false, &mut rng());
        assert!(!q.is_shuffled());
    }

    #[test]
    fn starting_a_new_list_drops_hand_queued_tracks_but_keeps_shuffle_on() {
        let mut q = Queue::default();
        q.start(tracks(&["a", "b"]), 0, &mut rng());
        q.set_shuffle(true, &mut rng());
        q.enqueue(track("x"));
        q.start(tracks(&["m", "n", "o"]), 2, &mut rng());
        assert!(q.is_shuffled());
        assert_eq!(id(q.current()), Some("o"));
        assert_eq!(q.up_next().count(), 0);
        let mut seen = vec![];
        while let Some(t) = q.advance() {
            seen.push(t.id.clone());
        }
        seen.sort();
        assert_eq!(seen, ["m", "n"]);
    }

    #[test]
    fn extending_a_playing_list_skips_duplicates_and_keeps_shuffle_complete() {
        let mut q = Queue::default();
        q.start(tracks(&["a"]), 0, &mut rng());
        q.set_shuffle(true, &mut rng());
        q.extend(tracks(&["a", "b", "c"]), &mut rng());
        assert_eq!(q.list().len(), 3);
        let mut seen = vec![];
        while let Some(t) = q.advance() {
            seen.push(t.id.clone());
        }
        seen.sort();
        assert_eq!(seen, ["b", "c"]);
    }

    #[test]
    fn an_out_of_range_start_changes_nothing() {
        let mut q = Queue::default();
        q.start(tracks(&["a"]), 0, &mut rng());
        q.start(tracks(&["b"]), 5, &mut rng());
        assert_eq!(id(q.current()), Some("a"));
    }
}
