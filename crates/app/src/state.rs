//! Everything the window shows, and the two ways it changes: an action from the user
//! (`update`) or an answer from the backend (`apply`). Both return the commands to send.

use std::collections::{HashMap, HashSet};

use dnm_audio::equalizer::EqualizerValues;
use dnm_audio::AudioOutputDevice;
use dnm_core::{Album, AlbumRef, Artist, ArtistRef, Found, Track};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

use limn::{RasterImage, ScalarTransition, UiAction};

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
    /// A track someone shared, as its card.
    Song(Track),
    /// What is kept on disk to listen to without a network.
    Downloads,
    /// One of the user's lists, by its id.
    Playlist(u64),
    Settings,
}

/// An album or an artist the library or the home page can lead to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Entry {
    Album(AlbumRef),
    Artist(ArtistRef),
}

impl Entry {
    pub fn title(&self) -> &str {
        match self {
            Self::Album(album) => &album.title,
            Self::Artist(artist) => &artist.name,
        }
    }

    pub fn artwork(&self) -> &Option<String> {
        match self {
            Self::Album(album) => &album.artwork_url,
            Self::Artist(artist) => &artist.artwork_url,
        }
    }

    fn target(&self) -> Target {
        match self {
            Self::Album(album) => Target::Album(album.id.clone()),
            Self::Artist(artist) => Target::Artist(artist.id.clone()),
        }
    }

    fn is(&self, other: &Self) -> bool {
        self.target() == other.target()
    }
}

/// What the user keeps and what they opened lately, newest first. It is kept with the
/// settings of this machine until accounts carry it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Collection {
    pub kept: Vec<Entry>,
    pub recent: Vec<Entry>,
    #[serde(default)]
    pub playlists: Vec<Playlist>,
    /// The id the next playlist takes.
    #[serde(default)]
    next_playlist: u64,
}

/// A list the user makes. A collaborative one is meant for others to add to as well, which
/// they can once accounts carry it; until then it is a mark on a list of this machine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Playlist {
    pub id: u64,
    pub name: String,
    pub collaborative: bool,
    pub tracks: Vec<Track>,
}

/// A playlist being made in the dialog.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Draft {
    pub name: String,
    pub collaborative: bool,
    /// The track that asked for a new list, which goes into it.
    pub first: Option<Track>,
}

/// Something that cannot be undone, waiting for a yes.
#[derive(Clone, Debug, PartialEq)]
pub enum Confirm {
    DeletePlaylist(u64),
    ClearDownloads,
    ClearRecent,
}

/// What the user set, kept between runs.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Go on with similar music when what was asked for runs out.
    pub autoplay: bool,
    /// Show changes at once instead of moving to them.
    pub reduce_motion: bool,
    /// Seconds a track that ends shares with the next; 0 for none.
    pub crossfade: f32,
    pub quality: Quality,
}

/// How much of a stream is asked for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quality {
    Low,
    Normal,
    #[default]
    High,
}

impl Quality {
    pub const ALL: [(Quality, &'static str, &'static str); 3] = [(Quality::Low, "low", "Baja"), (Quality::Normal, "normal", "Normal"), (Quality::High, "high", "Alta")];

    /// The most bits per second it takes. YouTube offers about 50, 70 and 140 thousand.
    pub fn bps(self) -> u64 {
        match self {
            Quality::Low => 64_000,
            Quality::Normal => 96_000,
            Quality::High => u64::MAX,
        }
    }
}

/// The longest crossfade on offer, as in the players people know.
pub const CROSSFADE_MAX: f32 = 12.0;

impl Default for Settings {
    fn default() -> Self {
        Self { autoplay: true, reduce_motion: false, crossfade: 0.0, quality: Quality::High }
    }
}

/// How many recent entries the home page offers.
const RECENT_KEPT: usize = 8;

impl Collection {
    pub fn albums(&self) -> impl Iterator<Item = (usize, &AlbumRef)> {
        self.kept.iter().enumerate().filter_map(|(index, entry)| match entry {
            Entry::Album(album) => Some((index, album)),
            Entry::Artist(_) => None,
        })
    }

    pub fn artists(&self) -> impl Iterator<Item = (usize, &ArtistRef)> {
        self.kept.iter().enumerate().filter_map(|(index, entry)| match entry {
            Entry::Artist(artist) => Some((index, artist)),
            Entry::Album(_) => None,
        })
    }

    fn keeps(&self, entry: &Entry) -> bool {
        self.kept.iter().any(|kept| kept.is(entry))
    }

    fn toggle(&mut self, entry: Entry) {
        if self.keeps(&entry) {
            self.kept.retain(|kept| !kept.is(&entry));
        } else {
            self.kept.insert(0, entry);
        }
    }

    pub fn playlist(&self, id: u64) -> Option<&Playlist> {
        self.playlists.iter().find(|playlist| playlist.id == id)
    }

    /// The name a playlist takes when it is given none.
    pub fn default_name(&self) -> String {
        format!("Mi playlist n.º {}", self.playlists.len() + 1)
    }

    /// Makes a playlist, newest first, and returns its id.
    fn create(&mut self, draft: Draft) -> u64 {
        self.next_playlist += 1;
        let id = self.next_playlist;
        let name = match draft.name.trim() {
            "" => self.default_name(),
            name => name.to_owned(),
        };
        self.playlists.insert(0, Playlist { id, name, collaborative: draft.collaborative, tracks: draft.first.into_iter().collect() });
        id
    }

    /// Adds a track to a list once: a list holds each track one time.
    fn add(&mut self, id: u64, track: Track) {
        if let Some(playlist) = self.playlists.iter_mut().find(|playlist| playlist.id == id) {
            if !playlist.tracks.iter().any(|held| held.id == track.id) {
                playlist.tracks.push(track);
            }
        }
    }

    fn visited(&mut self, entry: Entry) {
        self.recent.retain(|recent| !recent.is(&entry));
        self.recent.insert(0, entry);
        self.recent.truncate(RECENT_KEPT);
    }
}

/// What the library panel lists.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Filter {
    #[default]
    All,
    Lists,
    Artists,
    Albums,
}

/// "Buenos días", "Buenas tardes" or "Buenas noches" for an hour of the day.
pub fn greeting(hour: u32) -> &'static str {
    match hour {
        6..=11 => "Buenos días",
        12..=19 => "Buenas tardes",
        _ => "Buenas noches",
    }
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
    /// Whether the library shows, and how far open it is drawn, from 0 to 1, on its way there.
    pub library_open: bool,
    pub library_slide: ScalarTransition,
    /// Whether the right panel shows, and whether it shows the queue instead of what plays.
    pub panel_open: bool,
    /// How far open the right panel is drawn, from 0 to 1, on its way to `panel_open`.
    pub panel_slide: ScalarTransition,
    /// Whether what the app moves by itself travels or jumps. Off for a screenshot, which
    /// wants the final picture.
    pub motion: bool,
    pub queue_open: bool,
    pub collection: Collection,
    pub filter: Filter,
    pub settings: Settings,
    /// The playlist being made, while its dialog is open.
    pub creating: Option<Draft>,
    pub confirm: Option<Confirm>,
    /// The room the downloads take on disk.
    pub saved_bytes: u64,
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
    /// The color each cover lends to a card, by the cover's address.
    pub tints: HashMap<String, limn::Color>,
    /// The track whose share card is open, and whether its link was just copied.
    pub sharing: Option<Track>,
    pub copied: bool,
    /// Text on its way to the clipboard; the window takes it.
    pub copy: Option<String>,
    /// Covers already asked for, arrived or not.
    covers: HashSet<String>,
    /// The tracks kept on disk, newest first, and the ids on their way there.
    pub saved: Vec<Track>,
    pub saving: HashSet<String>,
    /// Why the last track asked for could not be kept.
    pub save_error: Option<String>,
    /// The devices there are to play through, the one chosen (none for the system's own),
    /// and whether their list is open.
    pub outputs: Vec<AudioOutputDevice>,
    pub output: Option<String>,
    pub output_open: bool,
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

/// Where the app's links live. It is dnmusic's own site, so a link made here opens in any
/// of its clients.
pub const SITE: &str = "https://dnmusic-production-c3f4.up.railway.app";

/// The link that shares a track.
pub fn share_link(track: &Track) -> String {
    format!("{SITE}/cancion/{}", track.id)
}

/// The id of the track a dnmusic link shares, for any of the forms such a link takes.
/// Anything else is not a link: a search.
pub fn shared_song(text: &str) -> Option<&str> {
    let text = text.trim();
    let rest = [SITE, "https://dnmusic-app.vercel.app", "dnmusic:/", "app://dnmusic"].iter().find_map(|origin| text.strip_prefix(origin))?;
    let id = rest.strip_prefix("/cancion/")?.split(['/', '?', '#']).next()?;
    (!id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')).then_some(id)
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
            library_open: true,
            library_slide: ScalarTransition::new(1.0, Instant::now()),
            panel_open: true,
            panel_slide: ScalarTransition::new(1.0, Instant::now()),
            motion: false,
            queue_open: false,
            collection: Collection::default(),
            filter: Filter::All,
            settings: Settings::default(),
            creating: None,
            confirm: None,
            saved_bytes: 0,
            left_width: Side::Left.limits().default,
            right_width: Side::Right.limits().default,
            resizing: None,
            menu: None,
            equalizer_open: false,
            equalizer: EqualizerValues::FLAT,
            artwork: HashMap::new(),
            tints: HashMap::new(),
            sharing: None,
            copied: false,
            copy: None,
            covers: HashSet::new(),
            saved: Vec::new(),
            saving: HashSet::new(),
            save_error: None,
            outputs: Vec::new(),
            output: None,
            output_open: false,
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
        // A dnmusic link is not searched for: it opens the track it shares.
        if let Some(id) = shared_song(&query) {
            return self.open(Target::Song(id.to_owned()));
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

    /// The page being shown as something the library can keep.
    fn page_entry(&self) -> Option<Entry> {
        match &self.page {
            Page::Album(album) => Some(Entry::Album(AlbumRef {
                id: album.id.clone(),
                title: album.title.clone(),
                artist: Some(album.artist.clone()),
                year: album.year.clone(),
                artwork_url: album.artwork_url.clone(),
            })),
            Page::Artist(artist) => Some(Entry::Artist(ArtistRef {
                id: artist.id.clone(),
                name: artist.name.clone(),
                subscribers: artist.subscribers.clone(),
                artwork_url: artist.artwork_url.clone(),
            })),
            _ => None,
        }
    }

    /// Whether the page being shown is in the library.
    pub fn keeps_page(&self) -> bool {
        self.page_entry().is_some_and(|entry| self.collection.keeps(&entry))
    }

    /// The covers of the tracks the page being shown lists.
    fn page_covers(&mut self) -> Vec<Command> {
        let urls: Vec<Option<String>> = self.page_tracks().iter().map(|track| track.artwork_url.clone()).collect();
        cover_requests(&mut self.covers, &urls)
    }

    /// The covers of everything the library and the home page show.
    pub fn collection_covers(&mut self) -> Vec<Command> {
        let collection = &self.collection;
        cover_requests(&mut self.covers, collection.kept.iter().chain(&collection.recent).map(Entry::artwork))
    }

    /// Opens or closes a side panel, which slides to its place.
    fn open_side(&mut self, side: Side, open: bool) {
        let (flag, slide) = match side {
            Side::Left => (&mut self.library_open, &mut self.library_slide),
            Side::Right => (&mut self.panel_open, &mut self.panel_slide),
        };
        *flag = open;
        let travel = Duration::from_millis(u64::from(limn::tokens().motion.move_ms));
        slide.retarget_with_motion(if open { 1.0 } else { 0.0 }, Instant::now(), travel, !self.motion);
    }

    /// Puts the panels where a saved session left them, with no travel.
    pub fn restore_sides(&mut self, library_open: bool, panel_open: bool) {
        let now = Instant::now();
        self.library_open = library_open;
        self.library_slide = ScalarTransition::new(if library_open { 1.0 } else { 0.0 }, now);
        self.panel_open = panel_open;
        self.panel_slide = ScalarTransition::new(if panel_open { 1.0 } else { 0.0 }, now);
    }

    /// Whether a side panel is on its way open or shut.
    pub fn is_sliding(&self, now: Instant) -> bool {
        self.library_slide.is_active(now) || self.panel_slide.is_active(now)
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
            ("kept", Some(index), _) => self.collection.kept.get(index).map(Entry::target),
            ("recent", Some(index), _) => self.collection.recent.get(index).map(Entry::target),
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
            "share" => {
                self.sharing = Some(track);
                self.copied = false;
                Vec::new()
            }
            "save" => {
                self.save_error = None;
                vec![Command::Save(track)]
            }
            "unsave" => vec![Command::Unsave(track.id)],
            "artist" => self.open(artist_of(&track)),
            "album" => track.album_id.map(Target::Album).map(|album| self.open(album)).unwrap_or_default(),
            // A track goes into a list that exists, or into one made for it.
            "add-new" => {
                self.creating = Some(Draft { first: Some(track), ..Draft::default() });
                Vec::new()
            }
            "remove" => {
                if let (Page::Playlist(id), Some(index)) = (&self.page, row.strip_prefix("page-track:").and_then(|index| index.parse::<usize>().ok())) {
                    if let Some(playlist) = self.collection.playlists.iter_mut().find(|playlist| playlist.id == *id) {
                        playlist.tracks.remove(index);
                    }
                }
                Vec::new()
            }
            add => {
                if let Some(id) = add.strip_prefix("add-").and_then(|id| id.parse::<u64>().ok()) {
                    self.collection.add(id, track);
                }
                Vec::new()
            }
        }
    }

    /// The tracks the current page plays.
    fn page_tracks(&self) -> &[Track] {
        match &self.page {
            Page::Artist(artist) => &artist.songs,
            Page::Album(album) => &album.tracks,
            Page::Song(track) => std::slice::from_ref(track),
            Page::Downloads => &self.saved,
            Page::Playlist(id) => self.collection.playlist(*id).map_or(&[], |playlist| &playlist.tracks),
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
            UiAction::InputChanged { id, value } if id == "playlist-name" => {
                if let Some(draft) = &mut self.creating {
                    draft.name = value.clone();
                }
                Vec::new()
            }
            UiAction::Submitted { id, .. } if id == "playlist-name" => self.click("playlist-create"),
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
            UiAction::SliderChanged { id, fraction, .. } if id == "setting:crossfade" => {
                self.settings.crossfade = (fraction * CROSSFADE_MAX).round();
                vec![Command::Crossfade(f64::from(self.settings.crossfade))]
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
        if let Some(playlist) = indexed("playlist:").and_then(|index| self.collection.playlists.get(index)) {
            let id = playlist.id;
            if self.page != Page::Playlist(id) {
                self.go(Page::Playlist(id));
            }
            return self.page_covers();
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
            // The queue is a face of the right panel: its button opens the panel on it, and
            // closes the panel from it.
            "queue" => {
                self.queue_open = !self.queue_open;
                self.open_side(Side::Right, self.queue_open);
                self.upcoming_covers()
            }
            // Each panel is shut from its own header and opened from the bar above.
            "toggle-panel" | "toggle-panel-open" => {
                self.open_side(Side::Right, !self.panel_open);
                self.queue_open = false;
                Vec::new()
            }
            "toggle-library" | "toggle-library-open" => {
                self.open_side(Side::Left, !self.library_open);
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
            // The sleeve of a card plays its track, or pauses it when it is the one playing.
            "share-card" | "song-card" => {
                let track = if id == "share-card" { self.sharing.clone() } else { self.page_tracks().first().cloned() };
                match track {
                    Some(track) if self.current().is_some_and(|current| current.id == track.id) => vec![Command::PlayPause],
                    Some(track) => vec![Command::PlayList { tracks: vec![track], index: 0 }],
                    None => Vec::new(),
                }
            }
            "share-copy" => {
                self.copy = self.sharing.as_ref().map(share_link);
                self.copied = self.copy.is_some();
                Vec::new()
            }
            "share-close" => {
                self.sharing = None;
                Vec::new()
            }
            "settings" => {
                if self.page != Page::Settings {
                    self.go(Page::Settings);
                }
                // The page names the chosen device, so it needs their list.
                vec![Command::Outputs]
            }
            "setting:output" => {
                self.output_open = !self.output_open;
                if self.output_open { vec![Command::Outputs] } else { Vec::new() }
            }
            "setting:output-dismiss" => {
                self.output_open = false;
                Vec::new()
            }
            _ if id.starts_with("setting:output:") => {
                self.output_open = false;
                let chosen = Some(id["setting:output:".len()..].to_owned()).filter(|device| !device.is_empty());
                if chosen == self.output {
                    return Vec::new();
                }
                self.output = chosen;
                vec![Command::Output(self.output.clone())]
            }
            "setting:autoplay" => {
                self.settings.autoplay = !self.settings.autoplay;
                vec![Command::Autoplay(self.settings.autoplay)]
            }
            "setting:motion" => {
                self.settings.reduce_motion = !self.settings.reduce_motion;
                Vec::new()
            }
            _ if id.starts_with("setting:quality:") => {
                let Some((quality, ..)) = Quality::ALL.into_iter().find(|(_, name, _)| id.ends_with(name)) else { return Vec::new() };
                self.settings.quality = quality;
                vec![Command::Quality(quality.bps())]
            }
            "playlist-new" => {
                self.creating = Some(Draft::default());
                Vec::new()
            }
            "playlist-kind:normal" | "playlist-kind:collaborative" => {
                if let Some(draft) = &mut self.creating {
                    draft.collaborative = id == "playlist-kind:collaborative";
                }
                Vec::new()
            }
            "playlist-cancel" => {
                self.creating = None;
                Vec::new()
            }
            "playlist-create" => {
                if let Some(draft) = self.creating.take() {
                    let id = self.collection.create(draft);
                    self.go(Page::Playlist(id));
                    return self.page_covers();
                }
                Vec::new()
            }
            "playlist-delete" => {
                if let Page::Playlist(id) = self.page {
                    self.confirm = Some(Confirm::DeletePlaylist(id));
                }
                Vec::new()
            }
            "downloads-clear" => {
                self.confirm = (!self.saved.is_empty()).then_some(Confirm::ClearDownloads);
                Vec::new()
            }
            "recent-clear" => {
                self.confirm = (!self.collection.recent.is_empty()).then_some(Confirm::ClearRecent);
                Vec::new()
            }
            "confirm-cancel" => {
                self.confirm = None;
                Vec::new()
            }
            "confirm-yes" => match self.confirm.take() {
                Some(Confirm::DeletePlaylist(id)) => {
                    self.collection.playlists.retain(|playlist| playlist.id != id);
                    self.history.retain(|page| *page != Page::Playlist(id));
                    if self.page == Page::Playlist(id) {
                        self.page = self.history.pop().unwrap_or(Page::Search);
                    }
                    Vec::new()
                }
                Some(Confirm::ClearDownloads) => self.saved.iter().map(|track| Command::Unsave(track.id.clone())).collect(),
                Some(Confirm::ClearRecent) => {
                    self.collection.recent.clear();
                    Vec::new()
                }
                None => Vec::new(),
            },
            "keep" => {
                if let Some(entry) = self.page_entry() {
                    self.collection.toggle(entry);
                }
                Vec::new()
            }
            "filter:all" | "filter:lists" | "filter:artists" | "filter:albums" => {
                self.filter = match id {
                    "filter:lists" => Filter::Lists,
                    "filter:artists" => Filter::Artists,
                    "filter:albums" => Filter::Albums,
                    _ => Filter::All,
                };
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
                    Ok(Content::Song(track)) => Page::Song(track),
                    Err(message) => Page::Failed(target, message),
                };
                // What was opened is what the home page offers next time.
                if let Some(entry) = self.page_entry() {
                    self.collection.visited(entry);
                }
                match &self.page {
                    Page::Artist(artist) => {
                        let shown = &artist.songs[..artist.songs.len().min(ARTIST_SONGS_SHOWN)];
                        let releases = artist.albums.iter().chain(&artist.singles).map(|album| &album.artwork_url);
                        cover_requests(&mut self.covers, std::iter::once(&artist.artwork_url).chain(covers_of(shown)).chain(releases))
                    }
                    Page::Album(album) => cover_requests(&mut self.covers, [&album.artwork_url]),
                    Page::Song(track) => cover_requests(&mut self.covers, [&track.artwork_url]),
                    _ => Vec::new(),
                }
            }
            Event::SearchFailed { query, message } if self.submitted.as_deref() == Some(&query) => {
                self.searching = false;
                self.search_error = Some(message);
                Vec::new()
            }
            Event::Library { saved, saving, bytes } => {
                self.saved = saved;
                self.saving = saving;
                self.saved_bytes = bytes;
                if self.page == Page::Downloads { cover_requests(&mut self.covers, covers_of(&self.saved)) } else { Vec::new() }
            }
            Event::Outputs(devices) => {
                self.outputs = devices;
                Vec::new()
            }
            Event::OutputLost => {
                self.output = None;
                Vec::new()
            }
            Event::SaveFailed { title, message } => {
                self.save_error = Some(format!("{title}: {message}"));
                Vec::new()
            }
            Event::Artwork { url, image, tint } => {
                if let Some([red, green, blue]) = tint {
                    self.tints.insert(url.clone(), limn::Color::rgb(red, green, blue));
                }
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
        state.apply(Event::Library { saved: Vec::new(), saving: HashSet::from(["b".to_owned()]), bytes: 0 });
        assert!(state.saving.contains("b") && !state.is_saved("b"));
        state.apply(Event::SaveFailed { title: "b".into(), message: "sin red".into() });
        assert_eq!(state.save_error.as_deref(), Some("b: sin red"));

        state.apply(Event::Library { saved: vec![track("b"), track("a")], saving: HashSet::new(), bytes: 9 });
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
        click(&mut state, "toggle-panel");
        assert!(!state.panel_open && !state.queue_open, "shut from its own header");
        click(&mut state, "toggle-panel-open");
        assert!(state.panel_open && !state.queue_open, "opened from the bar above, on what plays");
        click(&mut state, "queue");
        assert!(state.panel_open && state.queue_open);
        click(&mut state, "queue");
        assert!(!state.panel_open && !state.queue_open);

        click(&mut state, "toggle-library");
        assert!(!state.library_open);
        assert_eq!(state.library_slide.value(Instant::now()), 0.0, "without motion it is shut at once");
        click(&mut state, "toggle-library-open");
        assert!(state.library_open && !state.is_sliding(Instant::now()));
        state.motion = true;
        click(&mut state, "toggle-library");
        assert!(state.is_sliding(Instant::now()), "with motion it travels");
        state.restore_sides(false, true);
        assert!(!state.library_open && state.panel_open && !state.is_sliding(Instant::now()));
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
    fn the_library_keeps_what_the_user_keeps_and_home_remembers_what_was_opened() {
        let mut state = State::default();
        assert!(click(&mut state, "keep").is_empty() && state.collection.kept.is_empty(), "nothing on this page to keep");
        let open = |state: &mut State, target: Target, content: Content| {
            state.page = Page::Loading(target.clone());
            state.apply(Event::Opened { target, result: Ok(content) });
        };
        open(&mut state, Target::Album("lp".into()), Content::Album(album("lp", Some("who"))));
        open(&mut state, Target::Artist("who".into()), Content::Artist(artist("who")));
        open(&mut state, Target::Album("lp".into()), Content::Album(album("lp", Some("who"))));
        let titles: Vec<&str> = state.collection.recent.iter().map(Entry::title).collect();
        assert_eq!(titles, ["lp", "who"], "newest first, each once");

        assert!(!state.keeps_page());
        click(&mut state, "keep");
        assert!(state.keeps_page());
        assert_eq!(state.collection.albums().count(), 1);
        state.page = Page::Search;
        assert_eq!(click(&mut state, "kept:0"), vec![Command::Open(Target::Album("lp".into()))]);
        assert_eq!(click(&mut state, "recent:1"), vec![Command::Open(Target::Artist("who".into()))]);
        assert!(click(&mut state, "kept:5").is_empty());

        let kept = serde_json::to_string(&state.collection).unwrap();
        assert_eq!(serde_json::from_str::<Collection>(&kept).unwrap(), state.collection, "it survives being written down");
        state.page = Page::Album(album("lp", Some("who")));
        click(&mut state, "keep");
        assert!(state.collection.kept.is_empty(), "kept twice is not kept");

        click(&mut state, "filter:artists");
        assert_eq!(state.filter, Filter::Artists);
        click(&mut state, "filter:all");
        assert_eq!(state.filter, Filter::All);
        assert_eq!((greeting(7), greeting(15), greeting(23), greeting(3)), ("Buenos días", "Buenas tardes", "Buenas noches", "Buenas noches"));
    }

    #[test]
    fn sharing_a_track_opens_its_card_copies_its_link_and_plays_it() {
        let mut state = State { query: "q".into(), ..State::default() };
        state.search();
        state.apply(Event::SearchResults { query: "q".into(), found: found(&["a", "b"]) });
        click(&mut state, "do:share:track:1");
        assert_eq!(state.sharing, Some(track("b")));
        assert_eq!(click(&mut state, "share-card"), vec![Command::PlayList { tracks: vec![track("b")], index: 0 }]);
        state.show(playing("b"));
        assert_eq!(click(&mut state, "share-card"), vec![Command::PlayPause], "the one playing is paused");
        click(&mut state, "share-copy");
        assert_eq!(state.copy.take().as_deref(), Some("https://dnmusic-production-c3f4.up.railway.app/cancion/b"));
        assert!(state.copied);
        click(&mut state, "share-close");
        assert!(state.sharing.is_none());
        click(&mut state, "share-copy");
        assert!(state.copy.is_none(), "nothing to copy once closed");
    }

    #[test]
    fn a_pasted_link_opens_the_track_it_shares_and_anything_else_is_searched() {
        let link = format!("{SITE}/cancion/DdI598gKkKw");
        assert_eq!(shared_song(&link), Some("DdI598gKkKw"));
        assert_eq!(shared_song("  dnmusic://cancion/abc_-1?x=1 "), Some("abc_-1"));
        assert_eq!(shared_song("app://dnmusic/cancion/abc/extra"), Some("abc"));
        for other in ["apocalypse", "https://example.com/cancion/abc", &format!("{SITE}/lista/abc"), &format!("{SITE}/cancion/"), &format!("{SITE}/cancion/a%20b")] {
            assert_eq!(shared_song(other), None, "{other}");
        }
        let mut state = State { query: link, ..State::default() };
        assert_eq!(state.search(), vec![Command::Open(Target::Song("DdI598gKkKw".into()))]);
        assert!(state.submitted.is_none(), "no search was started");
        state.apply(Event::Opened { target: Target::Song("DdI598gKkKw".into()), result: Ok(Content::Song(track("a"))) });
        assert_eq!(state.page, Page::Song(track("a")));
        assert_eq!(click(&mut state, "song-card"), vec![Command::PlayList { tracks: vec![track("a")], index: 0 }]);
    }

    #[test]
    fn a_playlist_is_made_in_its_dialog_filled_from_the_menus_and_deleted_after_a_yes() {
        let mut state = State { query: "q".into(), ..State::default() };
        state.search();
        state.apply(Event::SearchResults { query: "q".into(), found: found(&["a", "b"]) });

        click(&mut state, "playlist-new");
        assert_eq!(state.creating, Some(Draft::default()));
        state.update(&UiAction::InputChanged { id: "playlist-name".into(), value: "  Para correr ".into() });
        click(&mut state, "playlist-kind:collaborative");
        click(&mut state, "playlist-create");
        assert!(state.creating.is_none());
        let made = state.collection.playlists[0].clone();
        assert_eq!((made.name.as_str(), made.collaborative, made.tracks.len()), ("Para correr", true, 0));
        assert_eq!(state.page, Page::Playlist(made.id), "the new list opens");

        click(&mut state, "back");
        click(&mut state, &format!("do:add-{}:track:1", made.id));
        click(&mut state, &format!("do:add-{}:track:1", made.id));
        assert_eq!(state.collection.playlist(made.id).unwrap().tracks, vec![track("b")], "a list holds a track once");

        click(&mut state, "do:add-new:track:0");
        assert_eq!(state.creating.as_ref().and_then(|draft| draft.first.clone()), Some(track("a")));
        state.update(&UiAction::Submitted { id: "playlist-name".into(), value: String::new() });
        let second = state.collection.playlists[0].clone();
        assert_eq!((second.name.as_str(), second.collaborative, second.tracks.clone()), ("Mi playlist n.º 2", false, vec![track("a")]), "Enter makes it, with a name of its own");
        assert_eq!(click(&mut state, "page-play"), vec![Command::PlayList { tracks: vec![track("a")], index: 0 }]);
        click(&mut state, "do:remove:page-track:0");
        assert!(state.collection.playlist(second.id).unwrap().tracks.is_empty());

        click(&mut state, "playlist-delete");
        assert_eq!(state.confirm, Some(Confirm::DeletePlaylist(second.id)));
        click(&mut state, "confirm-cancel");
        assert_eq!(state.collection.playlists.len(), 2, "a no deletes nothing");
        click(&mut state, "playlist-delete");
        click(&mut state, "confirm-yes");
        assert_eq!(state.collection.playlists.len(), 1);
        assert_ne!(state.page, Page::Playlist(second.id), "the page of a deleted list is left");
        click(&mut state, "playlist-cancel");

        let written = serde_json::to_string(&state.collection).unwrap();
        assert_eq!(serde_json::from_str::<Collection>(&written).unwrap(), state.collection);
        let older: Collection = serde_json::from_str(r#"{"kept":[],"recent":[]}"#).unwrap();
        assert!(older.playlists.is_empty(), "a collection saved before lists existed still loads");
    }

    #[test]
    fn settings_toggle_and_the_destructive_ones_wait_for_a_yes() {
        let mut state = State::default();
        click(&mut state, "settings");
        assert_eq!(state.page, Page::Settings);
        assert_eq!(click(&mut state, "setting:autoplay"), vec![Command::Autoplay(false)]);
        click(&mut state, "setting:motion");
        assert!(!state.settings.autoplay && state.settings.reduce_motion);
        let half = UiAction::SliderChanged { id: "setting:crossfade".into(), fraction: 0.5, committed: true };
        assert_eq!(state.update(&half), vec![Command::Crossfade(6.0)]);
        assert_eq!(click(&mut state, "setting:output"), vec![Command::Outputs]);
        assert_eq!(click(&mut state, "setting:output:alsa:pipewire"), vec![Command::Output(Some("alsa:pipewire".into()))]);
        assert!(!state.output_open && click(&mut state, "setting:output:alsa:pipewire").is_empty(), "it is already the one");
        assert_eq!(click(&mut state, "setting:output:"), vec![Command::Output(None)]);
        state.output = Some("gone".into());
        state.apply(Event::OutputLost);
        assert_eq!(state.output, None);
        assert_eq!(click(&mut state, "setting:quality:normal"), vec![Command::Quality(96_000)]);
        assert_eq!(state.settings.quality, Quality::Normal);
        assert!(click(&mut state, "downloads-clear").is_empty() && state.confirm.is_none(), "nothing to clear");
        state.apply(Event::Library { saved: vec![track("a"), track("b")], saving: HashSet::new(), bytes: 2048 });
        assert_eq!(state.saved_bytes, 2048);
        click(&mut state, "downloads-clear");
        assert_eq!(state.confirm, Some(Confirm::ClearDownloads));
        assert_eq!(click(&mut state, "confirm-yes"), vec![Command::Unsave("a".into()), Command::Unsave("b".into())]);
        state.collection.visited(Entry::Album(AlbumRef { id: "lp".into(), title: "lp".into(), artist: None, year: String::new(), artwork_url: None }));
        click(&mut state, "recent-clear");
        click(&mut state, "confirm-yes");
        assert!(state.collection.recent.is_empty() && state.confirm.is_none());
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
