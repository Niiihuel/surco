//! Everything the window shows, and the two ways it changes: an action from the user
//! (`update`) or an answer from the backend (`apply`). Both return the commands to send.

use std::collections::{HashMap, HashSet};

use dnm_audio::equalizer::EqualizerValues;
use dnm_core::{Album, AlbumRef, Artist, Found, Track};
use limn::{RasterImage, UiAction};

use crate::backend::{Command, Content, Event, Target};
use crate::player::PlayerView;

/// What the main pane shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Page {
    Search,
    Loading(Target),
    Failed(Target, String),
    Artist(Artist),
    Album(Album),
    /// What is kept on disk to listen to without a network.
    Downloads,
}

/// A side panel the user can resize.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Side {
    Left,
    Right,
}

pub struct Limits {
    pub min: f32,
    pub default: f32,
    pub max: f32,
}

impl Side {
    pub const fn limits(self) -> Limits {
        match self {
            Self::Left => Limits { min: 220.0, default: 280.0, max: 420.0 },
            Self::Right => Limits { min: 280.0, default: 320.0, max: 460.0 },
        }
    }

    /// The id of the handle that resizes it.
    pub const fn handle(self) -> &'static str {
        match self {
            Self::Left => "resize:left",
            Self::Right => "resize:right",
        }
    }

    /// A width for this panel from anything a setting or a drag may hold.
    pub fn width(self, wanted: f32) -> f32 {
        let limits = self.limits();
        if wanted.is_finite() { wanted.clamp(limits.min, limits.max) } else { limits.default }
    }
}

/// How many of an artist's songs their page lists; playing one still queues them all.
pub const ARTIST_SONGS_SHOWN: usize = 10;

pub struct State {
    pub query: String,
    /// The query whose results are shown or awaited. Answers for any other are stale.
    pub submitted: Option<String>,
    pub searching: bool,
    pub found: Found,
    pub search_error: Option<String>,
    pub page: Page,
    /// The pages behind the current one, the search first.
    history: Vec<Page>,
    /// The player as the backend last described it.
    pub player: PlayerView,
    /// Where the seek thumb is while it is being dragged.
    pub seek_preview: Option<f32>,
    pub volume: f32,
    /// Whether the right panel shows, and whether it shows the queue instead of what plays.
    pub panel_open: bool,
    pub queue_open: bool,
    /// The widths the user gave the side panels.
    pub left_width: f32,
    pub right_width: f32,
    /// The handle being dragged: which one, where the pointer took it and the width it had.
    resizing: Option<(Side, f32, f32)>,
    /// The row whose menu of actions is open.
    pub menu: Option<String>,
    pub equalizer_open: bool,
    pub equalizer: EqualizerValues,
    pub artwork: HashMap<String, RasterImage>,
    /// Covers already asked for, arrived or not.
    covers: HashSet<String>,
    /// The tracks kept on disk, newest first, and the ids on their way there.
    pub saved: Vec<Track>,
    pub saving: HashSet<String>,
    /// Why the last track asked for could not be kept.
    pub save_error: Option<String>,
}

/// One request per cover, however many rows show it and however often it comes up.
// shortcut: covers come at the catalog's largest size and stay for the whole run; ask for the
// size on screen and evict when a long session's memory matters.
fn cover_requests<'a>(asked: &mut HashSet<String>, urls: impl IntoIterator<Item = &'a Option<String>>) -> Vec<Command> {
    let urls = urls.into_iter().flatten();
    urls.filter(|url| asked.insert((*url).clone())).map(|url| Command::FetchArtwork(url.clone())).collect()
}

fn covers_of(tracks: &[Track]) -> impl Iterator<Item = &Option<String>> {
    tracks.iter().map(|track| &track.artwork_url)
}

/// A cover's address at the size a large picture needs. The catalog's addresses end in the size
/// they were listed at (`=w120-h120-…`); any other address is returned as it is.
pub fn large_cover(url: &str) -> String {
    const SIDE: &str = "544";
    let digits = |text: &str| text.len() - text.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let Some(start) = url.rfind("=w") else { return url.to_owned() };
    let width = digits(&url[start + 2..]);
    let rest = &url[start + 2 + width..];
    let height = rest.strip_prefix("-h").map(digits).unwrap_or(0);
    if width == 0 || height == 0 {
        return url.to_owned();
    }
    format!("{}=w{SIDE}-h{SIDE}{}", &url[..start], &rest[2 + height..])
}

/// Where a track's artist lives: by link when the catalog gave one, by name otherwise.
fn artist_of(track: &Track) -> Target {
    track.artist_id.clone().map_or_else(|| Target::ArtistNamed(track.artist.clone()), Target::Artist)
}

/// The equaliser's range, each side of zero.
pub const EQUALIZER_DB: f32 = 12.0;

/// The equaliser as it is kept between runs: the preamp and then each band.
pub fn equalizer_text(values: EqualizerValues) -> String {
    let gains = std::iter::once(values.preamp_db).chain(values.bands_db);
    gains.map(|gain| gain.to_string()).collect::<Vec<_>>().join(",")
}

pub fn equalizer_from(text: &str) -> Option<EqualizerValues> {
    let gains: Vec<f32> = text.split(',').map(|gain| gain.parse().ok().filter(|gain: &f32| gain.is_finite())).collect::<Option<_>>()?;
    let (preamp_db, bands) = gains.split_first()?;
    Some(EqualizerValues { preamp_db: *preamp_db, bands_db: bands.try_into().ok()? })
}

impl Default for State {
    fn default() -> Self {
        Self {
            query: String::new(),
            submitted: None,
            searching: false,
            found: Found::default(),
            search_error: None,
            page: Page::Search,
            history: Vec::new(),
            player: PlayerView::default(),
            seek_preview: None,
            volume: 0.8,
            panel_open: true,
            queue_open: false,
            left_width: Side::Left.limits().default,
            right_width: Side::Right.limits().default,
            resizing: None,
            menu: None,
            equalizer_open: false,
            equalizer: EqualizerValues::FLAT,
            artwork: HashMap::new(),
            covers: HashSet::new(),
            saved: Vec::new(),
            saving: HashSet::new(),
            save_error: None,
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
        self.page = Page::Search;
        self.history.clear();
        vec![Command::Search(query)]
    }

    pub fn can_go_back(&self) -> bool {
        !self.history.is_empty()
    }

    pub fn is_saved(&self, track_id: &str) -> bool {
        self.saved.iter().any(|track| track.id == track_id)
    }

    /// Leaves the current page for another. A page that never loaded is not one to return to.
    fn go(&mut self, page: Page) {
        let leaving = std::mem::replace(&mut self.page, page);
        if !matches!(leaving, Page::Loading(_) | Page::Failed(..)) {
            self.history.push(leaving);
        }
    }

    fn open(&mut self, target: Target) -> Vec<Command> {
        self.go(Page::Loading(target.clone()));
        vec![Command::Open(target)]
    }

    /// The page a click leads to, if it leads to one.
    fn target_of(&self, id: &str) -> Option<Target> {
        let (name, index) = match id.split_once(':') {
            Some((name, index)) => (name, index.parse::<usize>().ok()),
            None => (id, None),
        };
        let album = |album: &AlbumRef| Target::Album(album.id.clone());
        match (name, index, &self.page) {
            ("artist", Some(index), _) => self.found.artists.get(index).map(|artist| Target::Artist(artist.id.clone())),
            ("album", Some(index), _) => self.found.albums.get(index).map(album),
            ("page-album", Some(index), Page::Artist(artist)) => artist.albums.get(index).map(album),
            ("page-single", Some(index), Page::Artist(artist)) => artist.singles.get(index).map(album),
            ("page-artist", None, Page::Album(album)) => {
                Some(album.artist_id.clone().map_or_else(|| Target::ArtistNamed(album.artist.clone()), Target::Artist))
            }
            ("retry", None, Page::Failed(target, _)) => Some(target.clone()),
            ("now-artist", None, _) => self.current().map(artist_of),
            ("now-album", None, _) => self.current().and_then(|track| track.album_id.clone()).map(Target::Album),
            _ => None,
        }
    }

    /// The track a row shows, from the row's id.
    pub fn track_at(&self, row: &str) -> Option<&Track> {
        let (list, index) = row.split_once(':')?;
        let index = index.parse::<usize>().ok()?;
        match list {
            "track" => self.found.tracks.get(index),
            "page-track" => self.page_tracks().get(index),
            "upcoming" => self.player.upcoming.get(index),
            _ => None,
        }
    }

    /// One of the actions of a row's menu.
    fn act(&mut self, action: &str, row: &str) -> Vec<Command> {
        let Some(track) = self.track_at(row).cloned() else { return Vec::new() };
        match action {
            "queue" => vec![Command::Enqueue(track)],
            "next" => vec![Command::PlayNext(track)],
            "save" => {
                self.save_error = None;
                vec![Command::Save(track)]
            }
            "unsave" => vec![Command::Unsave(track.id)],
            "artist" => self.open(artist_of(&track)),
            "album" => track.album_id.map(Target::Album).map(|album| self.open(album)).unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    /// The tracks the current page plays.
    fn page_tracks(&self) -> &[Track] {
        match &self.page {
            Page::Artist(artist) => &artist.songs,
            Page::Album(album) => &album.tracks,
            Page::Downloads => &self.saved,
            _ => &[],
        }
    }

    fn play_page(&self, index: usize) -> Vec<Command> {
        let tracks = self.page_tracks();
        if index >= tracks.len() {
            return Vec::new();
        }
        vec![Command::PlayList { tracks: tracks.to_vec(), index }]
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
            UiAction::DragStarted { id, position } => {
                self.resizing = [Side::Left, Side::Right].into_iter().find(|side| side.handle() == id).map(|side| {
                    (side, position.x, if side == Side::Left { self.left_width } else { self.right_width })
                });
                Vec::new()
            }
            UiAction::DragMoved { position, .. } => {
                // The left panel grows as its handle goes right; the right one as its handle goes left.
                match self.resizing {
                    Some((Side::Left, from, width)) => self.left_width = Side::Left.width(width + position.x - from),
                    Some((Side::Right, from, width)) => self.right_width = Side::Right.width(width - (position.x - from)),
                    None => {}
                }
                Vec::new()
            }
            UiAction::Dropped { .. } => {
                self.resizing = None;
                Vec::new()
            }
            UiAction::DragCancelled { .. } => {
                match self.resizing.take() {
                    Some((Side::Left, _, width)) => self.left_width = width,
                    Some((Side::Right, _, width)) => self.right_width = width,
                    None => {}
                }
                Vec::new()
            }
            UiAction::Clicked { id } => self.click(id),
            _ => Vec::new(),
        }
    }

    fn click(&mut self, id: &str) -> Vec<Command> {
        if let Some(row) = id.strip_prefix("more:") {
            self.menu = (self.menu.as_deref() != Some(row)).then(|| row.to_owned());
            return Vec::new();
        }
        if let Some((action, row)) = id.strip_prefix("do:").and_then(|rest| rest.split_once(':')) {
            self.menu = None;
            return self.act(action, row);
        }
        let indexed = |prefix: &str| id.strip_prefix(prefix).and_then(|index| index.parse::<usize>().ok());
        if let Some(index) = indexed("track:") {
            // A result plays on its own and the player follows it with its radio.
            let track = self.found.tracks.get(index);
            return track.map(|track| Command::PlayList { tracks: vec![track.clone()], index: 0 }).into_iter().collect();
        }
        if let Some(upcoming) = indexed("upcoming:") {
            return vec![Command::Jump(upcoming)];
        }
        if let Some(index) = indexed("page-track:") {
            return self.play_page(index);
        }
        let target = self.target_of(id);
        if let Some(target) = target {
            return self.open(target);
        }
        match id {
            "downloads" => {
                if self.page != Page::Downloads {
                    self.go(Page::Downloads);
                }
                cover_requests(&mut self.covers, covers_of(&self.saved))
            }
            "back" => {
                if let Some(page) = self.history.pop() {
                    self.page = page;
                }
                Vec::new()
            }
            "page-play" => self.play_page(0),
            "page-shuffle" => {
                let tracks = self.page_tracks();
                if tracks.is_empty() {
                    return Vec::new();
                }
                let start = Command::PlayList { tracks: tracks.to_vec(), index: fastrand::usize(..tracks.len()) };
                // The player keeps shuffle on across lists, so it is only switched on.
                (!self.player.shuffle).then_some(Command::Shuffle).into_iter().chain([start]).collect()
            }
            // Each button shows its face of the right panel, and closes the panel from that face.
            "queue" => {
                self.queue_open = !self.queue_open;
                self.panel_open = self.queue_open;
                self.upcoming_covers()
            }
            "now-playing-view" => {
                self.panel_open = !self.panel_open || self.queue_open;
                self.queue_open = false;
                Vec::new()
            }
            "home" => {
                self.page = Page::Search;
                self.history.clear();
                self.query.clear();
                self.submitted = None;
                self.searching = false;
                self.search_error = None;
                self.found = Found::default();
                Vec::new()
            }
            "equalizer" => {
                self.equalizer_open = !self.equalizer_open;
                Vec::new()
            }
            "menu-close" => {
                self.menu = None;
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
            Event::SearchResults { query, found } if self.submitted.as_deref() == Some(&query) => {
                self.searching = false;
                self.found = found;
                let found = &self.found;
                let covers = covers_of(&found.tracks)
                    .chain(found.artists.iter().map(|artist| &artist.artwork_url))
                    .chain(found.albums.iter().map(|album| &album.artwork_url));
                cover_requests(&mut self.covers, covers)
            }
            Event::Opened { target, result } if self.page == Page::Loading(target.clone()) => {
                self.page = match result {
                    Ok(Content::Artist(artist)) => Page::Artist(artist),
                    Ok(Content::Album(album)) => Page::Album(album),
                    Err(message) => Page::Failed(target, message),
                };
                match &self.page {
                    Page::Artist(artist) => {
                        let shown = &artist.songs[..artist.songs.len().min(ARTIST_SONGS_SHOWN)];
                        let releases = artist.albums.iter().chain(&artist.singles).map(|album| &album.artwork_url);
                        cover_requests(&mut self.covers, std::iter::once(&artist.artwork_url).chain(covers_of(shown)).chain(releases))
                    }
                    Page::Album(album) => cover_requests(&mut self.covers, [&album.artwork_url]),
                    _ => Vec::new(),
                }
            }
            Event::SearchFailed { query, message } if self.submitted.as_deref() == Some(&query) => {
                self.searching = false;
                self.search_error = Some(message);
                Vec::new()
            }
            Event::Library { saved, saving } => {
                self.saved = saved;
                self.saving = saving;
                if self.page == Page::Downloads { cover_requests(&mut self.covers, covers_of(&self.saved)) } else { Vec::new() }
            }
            Event::SaveFailed { title, message } => {
                self.save_error = Some(format!("{title}: {message}"));
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
        // The playing track's cover is also shown large, in the right panel.
        let large = self.player.current.as_ref().and_then(|track| track.artwork_url.as_deref()).map(large_cover);
        let mut commands = cover_requests(&mut self.covers, self.player.current.iter().map(|track| &track.artwork_url).chain([&large]));
        commands.extend(self.upcoming_covers());
        commands
    }

    /// The queue's covers are only fetched while the queue is on screen.
    fn upcoming_covers(&mut self) -> Vec<Command> {
        if !self.queue_open {
            return Vec::new();
        }
        cover_requests(&mut self.covers, covers_of(&self.player.upcoming))
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

    fn found(ids: &[&str]) -> Found {
        Found { tracks: ids.iter().map(|id| track(id)).collect(), ..Found::default() }
    }

    fn album(id: &str, artist_id: Option<&str>) -> Album {
        Album {
            id: id.into(),
            title: id.into(),
            artist: "artist".into(),
            artist_id: artist_id.map(Into::into),
            year: "2017".into(),
            artwork_url: Some(format!("https://img/{id}")),
            tracks: vec![track("t1"), track("t2")],
        }
    }

    fn artist(id: &str) -> Artist {
        let release = |id: &str| AlbumRef { id: id.into(), title: id.into(), artist: None, year: "2019".into(), artwork_url: None };
        Artist {
            id: id.into(),
            name: id.into(),
            subscribers: None,
            artwork_url: None,
            songs: vec![track("s1"), track("s2")],
            albums: vec![release("lp")],
            singles: vec![release("single")],
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
        state.apply(Event::SearchResults { query: "first".into(), found: found(&["old"]) });
        assert!(state.found.tracks.is_empty() && state.searching, "the stale answer changes nothing");
        let commands = state.apply(Event::SearchResults { query: "second".into(), found: found(&["a", "a"]) });
        assert!(!state.searching);
        assert_eq!(commands, vec![Command::FetchArtwork("https://img/a".into())], "one request per cover");
        state.query = "   ".into();
        assert!(state.search().is_empty());
    }

    #[test]
    fn clicks_become_player_commands() {
        let mut state = State { query: "q".into(), ..State::default() };
        state.search();
        state.apply(Event::SearchResults { query: "q".into(), found: found(&["a", "b"]) });
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
    fn pages_open_load_and_lead_back_the_way_they_came() {
        let mut state = State { query: "q".into(), ..State::default() };
        state.search();
        let mut results = found(&["a"]);
        results.albums = vec![AlbumRef { id: "lp".into(), title: "lp".into(), artist: Some("artist".into()), year: String::new(), artwork_url: None }];
        state.apply(Event::SearchResults { query: "q".into(), found: results });

        assert_eq!(click(&mut state, "album:0"), vec![Command::Open(Target::Album("lp".into()))]);
        assert_eq!(state.page, Page::Loading(Target::Album("lp".into())));
        state.apply(Event::Opened { target: Target::Album("other".into()), result: Ok(Content::Album(album("other", None))) });
        assert_eq!(state.page, Page::Loading(Target::Album("lp".into())), "the answer for another page is dropped");
        let covers = state.apply(Event::Opened { target: Target::Album("lp".into()), result: Ok(Content::Album(album("lp", Some("who")))) });
        assert_eq!(covers, vec![Command::FetchArtwork("https://img/lp".into())]);
        assert_eq!(click(&mut state, "page-track:1"), vec![Command::PlayList { tracks: vec![track("t1"), track("t2")], index: 1 }]);
        assert_eq!(click(&mut state, "page-play"), vec![Command::PlayList { tracks: vec![track("t1"), track("t2")], index: 0 }]);
        assert!(click(&mut state, "page-track:7").is_empty());

        assert_eq!(click(&mut state, "page-artist"), vec![Command::Open(Target::Artist("who".into()))]);
        state.apply(Event::Opened { target: Target::Artist("who".into()), result: Ok(Content::Artist(artist("who"))) });
        assert_eq!(click(&mut state, "page-single:0"), vec![Command::Open(Target::Album("single".into()))]);
        state.apply(Event::Opened { target: Target::Album("single".into()), result: Err("sin red".into()) });
        assert_eq!(state.page, Page::Failed(Target::Album("single".into()), "sin red".into()));
        assert_eq!(click(&mut state, "retry"), vec![Command::Open(Target::Album("single".into()))]);

        click(&mut state, "back");
        assert_eq!(state.page, Page::Artist(artist("who")), "a page that never loaded is not one to return to");
        click(&mut state, "back");
        assert!(matches!(state.page, Page::Album(_)));
        click(&mut state, "back");
        assert!(state.page == Page::Search && !state.can_go_back());
    }

    #[test]
    fn the_playing_track_leads_to_its_album_and_its_artist() {
        let mut state = State::default();
        assert!(click(&mut state, "now-artist").is_empty(), "nothing plays yet");
        let mut playing_track = track("a");
        playing_track.album_id = Some("lp".into());
        state.show(PlayerView { current: Some(playing_track), ..PlayerView::default() });
        assert_eq!(click(&mut state, "now-album"), vec![Command::Open(Target::Album("lp".into()))]);
        assert_eq!(click(&mut state, "now-artist"), vec![Command::Open(Target::ArtistNamed("artist".into()))], "found by name without a link");
        state.query = "x".into();
        state.search();
        assert!(state.page == Page::Search && !state.can_go_back(), "a search starts over");
    }

    #[test]
    fn a_row_menu_queues_the_track_or_leads_to_its_album_and_artist() {
        let mut state = State { query: "q".into(), ..State::default() };
        state.search();
        let mut results = found(&["a", "b"]);
        results.tracks[1].album_id = Some("lp".into());
        state.apply(Event::SearchResults { query: "q".into(), found: results.clone() });
        let b = results.tracks[1].clone();

        click(&mut state, "more:track:1");
        assert_eq!(state.menu.as_deref(), Some("track:1"));
        click(&mut state, "more:track:0");
        assert_eq!(state.menu.as_deref(), Some("track:0"), "one menu at a time");
        click(&mut state, "more:track:0");
        assert!(state.menu.is_none(), "the dots close their own menu");

        click(&mut state, "more:track:1");
        assert_eq!(click(&mut state, "do:queue:track:1"), vec![Command::Enqueue(b.clone())]);
        assert!(state.menu.is_none(), "an action closes the menu");
        assert_eq!(click(&mut state, "do:next:track:1"), vec![Command::PlayNext(b)]);
        assert!(click(&mut state, "do:album:track:0").is_empty(), "that track names no album");
        assert_eq!(click(&mut state, "do:album:track:1"), vec![Command::Open(Target::Album("lp".into()))]);
        click(&mut state, "back");
        assert_eq!(click(&mut state, "do:artist:track:0"), vec![Command::Open(Target::ArtistNamed("artist".into()))]);
        assert!(click(&mut state, "do:queue:track:9").is_empty());
    }

    #[test]
    fn saved_tracks_have_a_page_that_plays_them_and_a_menu_that_drops_them() {
        let mut state = State { query: "q".into(), ..State::default() };
        state.search();
        state.apply(Event::SearchResults { query: "q".into(), found: found(&["a", "b"]) });
        assert_eq!(click(&mut state, "do:save:track:1"), vec![Command::Save(track("b"))]);
        state.apply(Event::Library { saved: Vec::new(), saving: HashSet::from(["b".to_owned()]) });
        assert!(state.saving.contains("b") && !state.is_saved("b"));
        state.apply(Event::SaveFailed { title: "b".into(), message: "sin red".into() });
        assert_eq!(state.save_error.as_deref(), Some("b: sin red"));

        state.apply(Event::Library { saved: vec![track("b"), track("a")], saving: HashSet::new() });
        assert!(state.is_saved("b"));
        assert!(click(&mut state, "downloads").is_empty(), "both covers were asked for with the search");
        assert_eq!(state.page, Page::Downloads);
        click(&mut state, "downloads");
        assert_eq!(click(&mut state, "page-track:1"), vec![Command::PlayList { tracks: vec![track("b"), track("a")], index: 1 }]);
        assert_eq!(click(&mut state, "do:unsave:page-track:0"), vec![Command::Unsave("b".into())]);
        click(&mut state, "back");
        assert!(state.page == Page::Search && !state.can_go_back(), "opening the page twice is one step back");
    }

    #[test]
    fn dragging_a_handle_resizes_its_panel_within_limits_and_escape_puts_it_back() {
        let mut state = State::default();
        let at = |x: f32| limn::Point::new(x, 100.0);
        let started = |id: &str, x| UiAction::DragStarted { id: id.into(), position: at(x) };
        let moved = |id: &str, x| UiAction::DragMoved { id: id.into(), position: at(x), over: None };
        let dropped = |id: &str, x| UiAction::Dropped { id: id.into(), position: at(x), target: None };

        state.update(&started("resize:left", 290.0));
        state.update(&moved("resize:left", 340.0));
        assert_eq!(state.left_width, 330.0, "50 to the right is 50 wider");
        state.update(&moved("resize:left", 2000.0));
        assert_eq!(state.left_width, Side::Left.limits().max);
        state.update(&dropped("resize:left", 2000.0));
        state.update(&moved("resize:left", 0.0));
        assert_eq!(state.left_width, Side::Left.limits().max, "nothing is held any more");

        state.update(&started("resize:right", 900.0));
        state.update(&moved("resize:right", 860.0));
        assert_eq!(state.right_width, 360.0, "the right panel grows towards the left");
        state.update(&UiAction::DragCancelled { id: "resize:right".into() });
        assert_eq!(state.right_width, 320.0);

        state.update(&started("some-row", 10.0));
        state.update(&moved("some-row", 500.0));
        assert_eq!((state.left_width, state.right_width), (Side::Left.limits().max, 320.0), "other drags resize nothing");
        assert_eq!(Side::Left.width(f32::NAN), 280.0);
    }

    #[test]
    fn the_right_panel_has_two_faces_and_each_button_closes_its_own() {
        let mut state = State::default();
        assert!(state.panel_open && !state.queue_open, "what plays shows at first");
        click(&mut state, "queue");
        assert!(state.panel_open && state.queue_open);
        click(&mut state, "now-playing-view");
        assert!(state.panel_open && !state.queue_open, "from the queue to what plays");
        click(&mut state, "now-playing-view");
        assert!(!state.panel_open);
        click(&mut state, "queue");
        assert!(state.panel_open && state.queue_open);
        click(&mut state, "queue");
        assert!(!state.panel_open && !state.queue_open);
    }

    #[test]
    fn home_clears_the_search() {
        let mut state = State { query: "q".into(), ..State::default() };
        state.search();
        state.apply(Event::SearchResults { query: "q".into(), found: found(&["a"]) });
        click(&mut state, "downloads");
        click(&mut state, "home");
        assert!(state.page == Page::Search && state.submitted.is_none() && state.found.tracks.is_empty() && state.query.is_empty());
        assert!(!state.can_go_back());
    }

    #[test]
    fn shuffling_a_page_turns_shuffle_on_once() {
        let mut state = State { page: Page::Album(album("lp", None)), ..State::default() };
        let commands = click(&mut state, "page-shuffle");
        assert!(matches!(commands.as_slice(), [Command::Shuffle, Command::PlayList { tracks, .. }] if tracks.len() == 2));
        state.show(PlayerView { shuffle: true, ..PlayerView::default() });
        assert!(matches!(click(&mut state, "page-shuffle").as_slice(), [Command::PlayList { .. }]));
    }

    #[test]
    fn a_cover_is_asked_for_large_where_its_address_names_a_size() {
        assert_eq!(large_cover("https://lh3.example/abc=w120-h120-l90-rj"), "https://lh3.example/abc=w544-h544-l90-rj");
        assert_eq!(large_cover("https://lh3.example/abc=w60-h60"), "https://lh3.example/abc=w544-h544");
        for other in ["https://i.ytimg.com/vi/x/hqdefault.jpg", "https://img/a", "https://x/=w-h1", "https://x/=w12", ""] {
            assert_eq!(large_cover(other), other);
        }
        let mut state = State::default();
        let mut track = track("a");
        track.artwork_url = Some("https://lh3.example/a=w120-h120-rj".into());
        let asked = state.show(PlayerView { current: Some(track), ..PlayerView::default() });
        assert_eq!(asked.len(), 2, "the row's size and the panel's: {asked:?}");
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
    fn the_equalizer_survives_being_written_down() {
        let mut values = EqualizerValues::FLAT;
        values.preamp_db = -3.0;
        values.bands_db[9] = 12.0;
        assert_eq!(equalizer_from(&equalizer_text(values)), Some(values));
        for broken in ["", "1,2,3", "0,0,0,0,0,0,0,0,0,0,x", "NaN,0,0,0,0,0,0,0,0,0,0"] {
            assert_eq!(equalizer_from(broken), None, "{broken:?} is not a setting");
        }
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
