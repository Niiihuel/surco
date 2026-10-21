//! Everything the window shows, and the two ways it changes: an action from the user
//! (`update`) or an answer from the backend (`apply`). Both return the commands to send.

pub(crate) mod mobile;
pub(crate) mod onboarding;

use std::collections::{HashMap, HashSet, VecDeque};

use surco_audio::equalizer::EqualizerValues;
use surco_audio::AudioOutputDevice;
use surco_core::discover;
use surco_core::transfer;
use surco_core::lyrics::{active, Line};
use std::sync::Arc;
use mobile::SearchFilter;
use surco_core::{Album, AlbumRef, Artist, ArtistRef, Found, Track};
use serde::{Deserialize, Serialize};

pub use crate::i18n::Language;
use crate::i18n::t;
use std::borrow::Cow;
use std::time::{Duration, Instant};

use limn::{RasterImage, ScalarSpring, ScalarTransition, SpringMotion, TableSort, UiAction};

use crate::backend::{Command, Content, Event, Footage, Known, Patch, Picks, PictureFor, Release, Target, Taste};
use crate::handoff::{self, Bundle, Offer, Parts};
use crate::update::Note;
use crate::clip::Showing;
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
    /// The tracks the user marked as liked.
    Liked,
    /// One of the user's lists, by its id.
    Playlist(u64),
    /// The saved collection as a full page on a phone.
    Library,
    /// The phone's full player, backed by the same playback state.
    Playing,
    Settings,
    /// A part of the settings, a page of its own under their list.
    SettingsPart(SettingsPart),
    /// The lines of what plays, in the middle of the window.
    Lyrics,
    /// What the artists the user follows have released, and a later version of the app.
    News,
    /// What each version of the app brought, as its release tells it.
    Patches,
    /// The equaliser as a page of its own on a phone.
    Equalizer,
    /// Where a search starts on the desktop: what was searched lately, and genres to browse.
    Browse,
    /// One of the lists the home makes out of what the user plays, as it was when opened.
    Mix(Mix),
    /// The local account: who uses this Surco, and what they play most.
    Profile,
}

/// A list the home makes out of what the user plays, opened and played as a playlist is.
#[derive(Clone, Debug, PartialEq)]
pub struct Mix {
    pub kind: MixKind,
    pub tracks: Vec<Track>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MixKind {
    /// What goes with everything played lately, named after the time of day.
    Moment,
    /// What goes with this track, the latest of one of the artists played most.
    Artist(Track),
    /// What was played again and again lately.
    Repeat,
}

impl Mix {
    /// Which mix it is, the same while the picks are asked for again.
    pub fn key(&self) -> String {
        match &self.kind {
            MixKind::Moment => "moment".to_owned(),
            MixKind::Artist(seed) => format!("artist-{}", seed.id),
            MixKind::Repeat => "repeat".to_owned(),
        }
    }

    /// The artists it plays most, after the one it is named after: what its card says it is.
    pub fn artists(&self, count: usize) -> Vec<&str> {
        let named = match &self.kind { MixKind::Artist(seed) => discover::plain(&seed.artist), _ => String::new() };
        let mut artists: Vec<&str> = Vec::new();
        for track in &self.tracks {
            let plain = discover::plain(&track.artist);
            if !plain.is_empty() && plain != named && artists.iter().all(|known| discover::plain(known) != plain) {
                artists.push(&track.artist);
            }
        }
        artists.truncate(count);
        artists
    }

    /// Up to four covers, each of a different release, for a picture made of them.
    pub fn covers(&self) -> Vec<&String> {
        let mut covers: Vec<&String> = Vec::new();
        for url in self.tracks.iter().filter_map(|track| track.artwork_url.as_ref()) {
            if covers.len() < 4 && !covers.contains(&url) {
                covers.push(url);
            }
        }
        covers
    }
}

/// The settings in parts, each a page of its own under their list, as Spotify's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsPart {
    Playback,
    Audio,
    Devices,
    Display,
    Library,
    About,
    Storage,
}

impl SettingsPart {
    pub const ALL: [SettingsPart; 7] = [Self::Playback, Self::Audio, Self::Devices, Self::Display, Self::Library, Self::About, Self::Storage];

    /// How its row's id names it.
    pub fn slug(self) -> &'static str {
        match self {
            Self::Playback => "playback",
            Self::Audio => "audio",
            Self::Devices => "devices",
            Self::Display => "display",
            Self::Library => "library",
            Self::About => "about",
            Self::Storage => "storage",
        }
    }
}

/// What played when the app was last closed, to go on from the next time: the list from its
/// current track on, and how far into that track it was.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub tracks: Vec<Track>,
    pub position: f64,
}

/// The minutes the sleep timer offers, as Opentune's and the phones' own clocks do.
pub const SLEEP_MINUTES: [u32; 5] = [15, 30, 45, 60, 90];

/// The most of what was still to play a session keeps.
const SESSION_KEPT: usize = 500;

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

/// A search remembered, as Spotify's: what was asked for, or a result chosen from it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Searched {
    /// All a list kept before results were remembered holds.
    Query(String),
    Track { track: Track },
    Entry(Entry),
}

impl Searched {
    fn is(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Query(one), Self::Query(other)) => one.to_lowercase() == other.to_lowercase(),
            (Self::Track { track: one }, Self::Track { track: other }) => one.id == other.id,
            (Self::Entry(one), Self::Entry(other)) => one.is(other),
            _ => false,
        }
    }
}

#[cfg(test)]
impl From<&str> for Searched {
    fn from(query: &str) -> Self { Self::Query(query.to_owned()) }
}

#[cfg(test)]
impl PartialEq<&str> for Searched {
    fn eq(&self, query: &&str) -> bool { matches!(self, Self::Query(held) if held == query) }
}

/// What the user keeps and what they opened lately, newest first. It is kept with the
/// settings of this machine until accounts carry it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Collection {
    pub kept: Vec<Entry>,
    pub recent: Vec<Entry>,
    /// Explicit searches and the results chosen from them, newest first. Typing alone is not
    /// history. Kept under a key of their own: a build from before results were remembered
    /// fails on them and would drop the whole collection, so it finds none.
    #[serde(default, rename = "searched", alias = "searches")]
    pub searches: Vec<Searched>,
    #[serde(default)]
    pub playlists: Vec<Playlist>,
    /// The tracks last played, newest first.
    #[serde(default)]
    pub heard: Vec<Track>,
    /// The tracks marked as liked, the last one first.
    #[serde(default)]
    pub liked: Vec<Track>,
    /// What the artists followed have released since they were followed, the latest
    /// found first, each with whether it is still to be looked at.
    #[serde(default)]
    pub news: Vec<(AlbumRef, bool)>,
    /// The releases already known of each artist followed, by the artist's id: one that
    /// is not among them is news.
    #[serde(default)]
    known: HashMap<String, Vec<String>>,
    /// The id the next playlist takes.
    #[serde(default)]
    next_playlist: u64,
    /// What the library lists first, in the order it was pinned, by what each row is.
    #[serde(default)]
    pub pinned: Vec<String>,
}

/// Songs that go with the playlist on screen, to add to it or to play.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Suggested {
    pub playlist: u64,
    /// What is still to be offered, the ones shown first.
    pub tracks: Vec<Track>,
    /// Every song offered so far, which a later answer leaves out.
    offered: HashSet<String>,
    /// How many times they were asked for: each time other songs of the list lead.
    round: usize,
    asking: bool,
}

/// A list the user makes. A collaborative one is meant for others to add to as well, which
/// they can once accounts carry it; until then it is a mark on a list of this machine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Playlist {
    pub id: u64,
    pub name: String,
    pub collaborative: bool,
    /// The picture chosen for it, kept with the covers on disk under this name.
    #[serde(default)]
    pub picture: Option<String>,
    pub tracks: Vec<Track>,
}

impl Playlist {
    /// What shows for the list: the picture chosen for it, or its first song's cover.
    pub fn cover(&self) -> Option<&String> {
        self.picture.as_ref().or_else(|| self.tracks.first().and_then(|track| track.artwork_url.as_ref()))
    }
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
    ClearQueue,
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
    /// Show a stretch of a song's own video where its cover is while it plays.
    pub clips: bool,
    pub library_sort: Sort,
    pub library_view: LibraryView,
    /// Play the same in both speakers.
    pub mono: bool,
    /// Turn the tracks louder than the reference down to it.
    pub normalize: bool,
    /// The library's offer to bring music in from another app was put away.
    pub import_dismissed: bool,
    /// The language chosen for the app; none follows the system's.
    pub language: Option<Language>,
    /// Each line of the lyrics comes with its translation into the app's language.
    pub translate_lyrics: bool,
    /// The offer to bring music in that greets a first start was answered.
    pub welcomed: bool,
    /// Whether the first run's steps were gone through, or the account brought in.
    pub onboarded: bool,
    /// The SponsorBlock categories jumped over in a video, a bit each; none, it is off.
    pub sponsorblock: u8,
}

/// The order the library lists what is not pinned in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sort {
    /// As it was kept, the newest first.
    #[default]
    Added,
    Alphabetical,
    /// By whose it is.
    Creator,
}

/// How the library lists what it holds: a line each, a row with its picture, or tiles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LibraryView {
    Compact,
    #[default]
    List,
    Grid,
}

impl Sort {
    pub fn label(self) -> &'static str {
        crate::i18n::fixed(match self {
            Sort::Added => t!("state-sort-added"),
            Sort::Alphabetical => t!("state-sort-alphabetical"),
            Sort::Creator => t!("common-creator"),
        })
    }
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
    pub const ALL: [(Quality, &'static str, Label); 3] = [(Quality::Low, "low", Label("state-quality-low")), (Quality::Normal, "normal", Label("state-quality-normal")), (Quality::High, "high", Label("state-quality-high"))];

    /// The most bits per second it takes. YouTube offers about 50, 70 and 140 thousand.
    pub fn bps(self) -> u64 {
        match self {
            Quality::Low => 64_000,
            Quality::Normal => 96_000,
            Quality::High => u64::MAX,
        }
    }
}

/// What is known of a track's lyrics.
#[derive(Clone, Debug, PartialEq)]
pub enum Lyrics {
    Loading,
    Lines(Vec<Line>),
    /// Available words without timing; they never seek or follow the playback clock.
    Plain { lines: Vec<String>, source: String },
    /// The request completed but the provider has no usable lyrics.
    Missing,
    /// A provider/network failure can be requested again after its rate-limit cooldown.
    Failed { message: String, retry_at: Instant },
}

/// The lyrics of a track in another language, a line for each of theirs.
#[derive(Clone, Debug, PartialEq)]
pub enum Translation {
    Asking,
    Lines(Vec<String>),
    /// They already are in the language asked for.
    Same,
    Failed,
}

/// How many tracks a shelf of the home page holds.
pub const SHELF: usize = 12;

/// The longest crossfade on offer, as in the players people know.
pub const CROSSFADE_MAX: f32 = 12.0;

impl Default for Settings {
    fn default() -> Self {
        Self { autoplay: true, reduce_motion: false, crossfade: 0.0, quality: Quality::High, clips: true, library_sort: Sort::Added, library_view: LibraryView::List, mono: false, normalize: true, import_dismissed: false, language: None, translate_lyrics: false, welcomed: false, onboarded: false, sponsorblock: crate::sponsorblock::DEFAULT }
    }
}

/// The dialog that takes the account to another device: sending it, with the parts that go
/// and the address and code it is served at, or receiving one, from the address typed.
#[derive(Default)]
pub struct Handoff {
    pub receiving: bool,
    pub parts: Parts,
    /// The address another device reads it from, and the code of it, or why there is none.
    pub offer: Option<Result<(Offer, Option<limn::RasterImage>), String>>,
    pub address: String,
    pub fetching: bool,
    /// What came from the other device, or why nothing did.
    pub fetched: Option<Result<Bundle, String>>,
    /// Whether what comes takes the place of what is here rather than joining it.
    pub replace: bool,
}

/// An app a list is brought in from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImportSource {
    #[default]
    Spotify,
    Apple,
    YouTube,
}

impl ImportSource {
    pub const ALL: [ImportSource; 3] = [ImportSource::Spotify, ImportSource::Apple, ImportSource::YouTube];

    /// The id after `import-source:`.
    pub fn slug(self) -> &'static str {
        match self { Self::Spotify => "spotify", Self::Apple => "apple", Self::YouTube => "youtube" }
    }

    pub fn name(self) -> &'static str {
        match self { Self::Spotify => "Spotify", Self::Apple => "Apple Music", Self::YouTube => "YouTube Music" }
    }

    /// The app a link is of.
    fn of(link: &str) -> Option<Self> {
        Some(match transfer::source(link.trim())? {
            transfer::Source::Spotify(..) => Self::Spotify,
            transfer::Source::Apple(_) => Self::Apple,
            transfer::Source::YouTube(_) => Self::YouTube,
        })
    }
}

/// Who uses this Surco: an account that lives on the device, named as its user likes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub name: String,
    /// The picture chosen for its face, kept with the covers on disk under this name.
    pub picture: Option<String>,
}

impl Profile {
    /// What the account is called: its name, or the app's while it has none.
    pub fn shown(&self) -> &str {
        let name = self.name.trim();
        if name.is_empty() { "Surco" } else { name }
    }

    /// The letter its face shows.
    pub fn initial(&self) -> String {
        self.shown().chars().next().map(|first| first.to_uppercase().collect()).unwrap_or_default()
    }
}

/// How far taking a later version in has got.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Updating {
    #[default]
    Idle,
    /// How much of its program has arrived, 0 to 1.
    Fetching(f32),
    /// In place: the next start is it.
    Ready,
    Failed(String),
}

/// How many explicit searches are remembered.
const SEARCH_HISTORY: usize = 12;
/// How many news are kept, and how many of an artist's releases of the year are listed
/// when they are first followed.
const NEWS_KEPT: usize = 60;
const FIRST_NEWS: usize = 2;

/// The narrowest and the widest the album column of a page's tracks is dragged to.
const ALBUM_WIDTH: (f32, f32) = (80.0, 360.0);

/// A song and its video this close in length are taken to be the same recording.
const SAME_LENGTH_MS: u64 = 2000;

/// How many played tracks are remembered, and for how many of the artists played most
/// the home page asks the catalog what goes with them.
const HEARD_KEPT: usize = 100;
const SEEDS: usize = 5;
/// The most tracks the mix of what was played again holds, and the fewest it takes to be one.
const REPEAT: usize = 20;
/// How many artists and songs the profile names as the most played, and the longest name.
pub const TOP_ARTISTS: usize = 10;
const TOP_TRACKS: usize = 10;
const PROFILE_NAME_MOST: usize = 40;
/// How many of the latest plays, each by another artist, today's picks grow from.
const RECENT_SEEDS: usize = 3;
const REPEAT_FEWEST: usize = 4;

/// How many songs a playlist's recommendations show at a time, and how few of them still to
/// come make it ask for more, as Spotify's.
pub const SUGGESTED_SHOWN: usize = 10;
const SUGGESTED_LOW: usize = 20;

/// How many recent entries the home page offers.
const RECENT_KEPT: usize = 8;
/// How long the rows of the lyrics take to settle after the list jumps to a new line.
const LYRIC_GLIDE: Duration = Duration::from_millis(560);
/// How much later each row sets off than the one nearer the line being sung, up to this many rows out.
const LYRIC_RIPPLE: Duration = Duration::from_millis(34);
const LYRIC_RIPPLE_ROWS: usize = 6;

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

    fn playlist_mut(&mut self, id: u64) -> Option<&mut Playlist> {
        self.playlists.iter_mut().find(|playlist| playlist.id == id)
    }

    /// The name a playlist takes when it is given none.
    pub fn default_name(&self) -> String {
        t!("state-default-playlist-name", number = self.playlists.len() + 1).into_owned()
    }

    /// Takes in a list from another device under an id of this one's, newest first, unless
    /// one of its name and songs is here already.
    fn adopt(&mut self, playlist: Playlist) {
        if self.playlists.iter().any(|held| held.name == playlist.name && held.tracks == playlist.tracks) { return; }
        // Past every id here, whatever the count says.
        self.next_playlist = self.playlists.iter().map(|held| held.id).max().unwrap_or(0).max(self.next_playlist) + 1;
        self.playlists.insert(0, Playlist { id: self.next_playlist, ..playlist });
    }

    /// Makes a playlist, newest first, and returns its id.
    fn create(&mut self, draft: Draft) -> u64 {
        self.next_playlist += 1;
        let id = self.next_playlist;
        let name = match draft.name.trim() {
            "" => self.default_name(),
            name => name.to_owned(),
        };
        self.playlists.insert(0, Playlist { id, name, collaborative: draft.collaborative, picture: None, tracks: draft.first.into_iter().collect() });
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

    /// Marks a track as liked, or takes the mark off one that had it.
    /// Pins a row of the library after the ones already pinned, or lets it go.
    fn pin(&mut self, key: &str) {
        let before = self.pinned.len();
        self.pinned.retain(|pinned| pinned != key);
        if self.pinned.len() == before {
            self.pinned.push(key.to_owned());
        }
    }

    fn like(&mut self, track: &Track) {
        let before = self.liked.len();
        self.liked.retain(|liked| liked.id != track.id);
        if self.liked.len() == before {
            self.liked.insert(0, track.clone());
        }
    }

    /// Takes in what an artist followed has out. The first time, all of it is known and
    /// only what came out in `year` is listed, as already seen; after that, whatever was
    /// not known is news.
    fn learn(&mut self, artist: String, releases: Vec<AlbumRef>, year: i32) {
        let first = !self.known.contains_key(&artist);
        let known = self.known.entry(artist).or_default();
        let fresh: Vec<AlbumRef> = releases.into_iter().filter(|release| !known.contains(&release.id)).collect();
        known.extend(fresh.iter().map(|release| release.id.clone()));
        if first {
            let of_the_year = fresh.into_iter().filter(|release| release.year == year.to_string());
            self.news.extend(of_the_year.take(FIRST_NEWS).map(|release| (release, false)));
        } else {
            // They come the latest first, and so they stay.
            for release in fresh.into_iter().rev() {
                self.news.insert(0, (release, true));
            }
        }
        self.news.truncate(NEWS_KEPT);
    }

    fn heard(&mut self, track: &Track) {
        self.heard.retain(|heard| heard.id != track.id);
        self.heard.insert(0, track.clone());
        self.heard.truncate(HEARD_KEPT);
    }

    /// Puts a search first among those remembered, once.
    fn searched(&mut self, searched: Searched) {
        self.searches.retain(|held| !held.is(&searched));
        self.searches.insert(0, searched);
        self.searches.truncate(SEARCH_HISTORY);
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
    crate::i18n::fixed(match hour {
        6..=11 => t!("state-greeting-morning"),
        12..=19 => t!("state-greeting-afternoon"),
        _ => t!("state-greeting-evening"),
    })
}

/// A catalog text a constant names by its id, said in the language of the moment it is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Label(&'static str);

impl Label {
    pub fn text(self) -> &'static str {
        crate::i18n::fixed(crate::i18n::text(crate::i18n::current(), self.0, None))
    }
}

impl From<Label> for String {
    fn from(label: Label) -> String {
        label.text().to_owned()
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
    /// Use the phone layout; desktop previews can opt in without a second player.
    pub mobile: bool,
    /// The room the window's own buttons take at the top bar's left and right ends.
    pub caption: (f32, f32),
    /// The search tab is open even before a query has been submitted.
    pub mobile_search: bool,
    pub mobile_ui: mobile::MobileUi,
    pub query: String,
    /// The query whose results are shown or awaited. Answers for any other are stale.
    pub submitted: Option<String>,
    pub searching: bool,
    /// What is typed, and when it is searched for if nothing more is typed by then.
    pub search_due: Option<(String, Instant)>,
    /// Which of the results show: all of them, or one kind.
    pub search_filter: mobile::SearchFilter,
    pub found: Found,
    pub search_error: Option<String>,
    pub page: Page,
    /// The pages behind the current one, the search first.
    history: Vec<Page>,
    /// The pages gone back from, the latest last: where forward leads, until a page is opened.
    ahead: Vec<Page>,
    /// The player as the backend last described it.
    pub player: PlayerView,
    /// Where the seek thumb is while it is being dragged.
    pub seek_preview: Option<f32>,
    pub volume: f32,
    /// What the tracks of the page are searched for, whether that search is open, and how
    /// far open its field is drawn, from 0 to 1, on its way there.
    pub page_search: String,
    pub page_searching: bool,
    pub page_search_slide: ScalarTransition,
    /// How far open the library's search is, the same way.
    pub library_search_slide: ScalarTransition,
    /// A field the window is to put the keyboard in, once.
    pub focus: Option<String>,
    /// The desktop's search field, or one of the recent searches it drops down, holds the
    /// keyboard: with nothing typed, they show.
    pub search_focused: bool,
    /// The panel of what plays is scrolled from its top: its header takes the panel's floor.
    pub panel_scrolled: bool,
    /// Whether the library shows, and how far open it is drawn, from 0 to 1, on its way there.
    pub library_open: bool,
    pub library_slide: ScalarTransition,
    /// Whether the right panel shows, and whether it shows the queue instead of what plays.
    pub panel_open: bool,
    /// How far open the right panel is drawn, from 0 to 1, on its way to `panel_open`.
    pub panel_slide: ScalarTransition,
    /// The quick-access entry of the home under the pointer, whose colour the page's wash takes.
    pub quick_hovered: Option<String>,
    /// The wash's colour when it last set off for another, and how far it has gone.
    pub wash_from: limn::Color,
    pub wash: ScalarTransition,
    /// Whether what the app moves by itself travels or jumps. Off for a screenshot, which
    /// wants the final picture.
    pub motion: bool,
    pub queue_open: bool,
    pub collection: Collection,
    pub profile: Profile,
    /// The account's menu, at the end of the top bar.
    pub account_open: bool,
    /// Whether the menu behind a page's dots is open.
    pub page_more_open: bool,
    /// The footage whose frames are on screen already, by the name `screening` gives it.
    pub clip_ready: Option<String>,
    /// How far the name of the song in the right panel reaches past its room, by the song's
    /// id, and since when it shows: a long one slides along it.
    pub title_reach: Option<(String, f32, Instant)>,
    /// The dialog that takes the account to another device, while it is open.
    pub handoff: Option<Handoff>,
    /// The first run's steps, while they are open.
    pub onboarding: Option<onboarding::Onboarding>,
    /// Whether the window has room for the right panel, open or as the strip it leaves shut.
    /// The window says so each frame.
    pub wide: bool,
    /// Whether the menu behind the dots of the panel of what plays is open.
    pub panel_more_open: bool,
    /// The account's name while it is being changed.
    pub profile_draft: Option<String>,
    pub filter: Filter,
    /// What the library's rows are searched for.
    pub library_search: String,
    /// The library's search is open, and so is the menu of its orders and views.
    pub library_searching: bool,
    pub library_menu: bool,
    pub settings: Settings,
    /// The playlist being made, while its dialog is open.
    pub creating: Option<Draft>,
    /// The link of another app's list being typed, while its dialog is open, and whether
    /// what was typed is no such link.
    pub importing: Option<String>,
    pub import_unknown: bool,
    /// The app the list being brought in comes from, whose way of copying its link is shown.
    pub import_source: ImportSource,
    /// The playlist just brought in and the songs of it that were not found, while its page
    /// says so, and whether the list of them is open.
    pub imported: Option<(u64, Vec<String>)>,
    /// What a tile's play button asked for, to play once it arrives without leaving the page.
    pub play_when: Option<Target>,
    pub missed_open: bool,
    /// How many songs of the list being brought in have been looked for, out of how many.
    pub import_progress: Option<(usize, usize)>,
    pub confirm: Option<Confirm>,
    /// The room the downloads take on disk.
    pub saved_bytes: u64,
    /// The widths the user gave the side panels.
    pub left_width: f32,
    pub right_width: f32,
    /// The handle being dragged: which one, where the pointer took it and the width it had.
    resizing: Option<(Side, f32, f32)>,
    /// What the page's tracks are ordered by; their own order while it is nothing.
    pub page_sort: Option<TableSort>,
    /// The menu of the orders a page's tracks can take is open.
    pub sort_open: bool,
    /// The width of the album column of a page's tracks, and whether it and the lengths
    /// are among the columns shown.
    pub album_width: f32,
    pub album_shown: bool,
    pub duration_shown: bool,
    /// The album column being dragged: where the drag started, and its width then.
    column_drag: Option<(f32, f32)>,
    /// The row whose menu of actions is open.
    pub menu: Option<String>,
    pub equalizer_open: bool,
    pub equalizer: EqualizerValues,
    /// The band last moved: its name is lit and its point marked on the curve.
    pub equalizer_band: Option<usize>,
    pub artwork: HashMap<String, RasterImage>,
    /// Where the page's song list began against its scroll area at the last layout, and the
    /// area's height: a long list builds only the rows around what is in view.
    pub track_window: Option<(f32, f32)>,
    /// The page's actions have scrolled away under its top, where its bar now stands.
    pub page_stuck: bool,
    /// The covers held, oldest first, so a long session lets the ones it saw first go.
    artwork_order: VecDeque<String>,
    /// The color each cover lends to a card, by the cover's address.
    pub tints: HashMap<String, limn::Color>,
    /// Each picture's width and height in pixels, by its address, while it is held.
    pub picture_sizes: HashMap<String, (u32, u32)>,
    /// The track whose share card is open, and whether its link was just copied.
    pub sharing: Option<Track>,
    pub copied: bool,
    /// The playlist whose link was just copied, while its page says so, and the one whose
    /// songs were, as text for another app.
    pub list_copied: Option<u64>,
    pub list_exported: Option<u64>,
    /// Text on its way to the clipboard; the window takes it.
    pub copy: Option<String>,
    /// A field targeted by an explicit user paste request; never read on a timer.
    pub paste: Option<String>,
    /// Covers already asked for, arrived or not.
    covers: HashSet<String>,
    /// The covers asked for that could not be had.
    missing_covers: HashSet<String>,
    /// The tracks kept on disk, newest first, and the ids on their way there.
    pub saved: Vec<Track>,
    pub saving: HashSet<String>,
    /// Why the last track asked for could not be kept.
    pub save_error: Option<String>,
    /// The devices there are to play through, the one chosen (none for the system's own),
    /// and whether their list is open.
    /// What sounds right now, in the engine's four bands; silence while nothing plays.
    pub levels: [f32; surco_audio::meter::BANDS],
    /// What the catalog offers after a track that was heard, by that track's id; empty
    /// while the answer is on its way.
    /// The catalog's picks for the home page: a mix out of what goes with what the user
    /// plays, what goes with each of the first seeds, and artists and albums to open.
    pub mix: Vec<Track>,
    pub because: Vec<(Track, Vec<Track>)>,
    pub picked_artists: Vec<ArtistRef>,
    pub picked_albums: Vec<AlbumRef>,
    /// What goes with the latest plays: today's picks.
    pub recent_picks: Vec<Track>,
    /// For the fans of each of the artists played most, albums of artists like them.
    pub fans_of: Vec<(ArtistRef, Vec<AlbumRef>)>,
    /// The artist played most, with the releases of theirs the user does not keep.
    pub more_of: Option<(ArtistRef, Vec<AlbumRef>)>,
    /// The artists the picks were last asked for.
    picked_for: Option<Vec<String>>,
    /// Which of the picks, the tracks, the artists and the albums, are on their way.
    pub picks_coming: [bool; 3],
    /// The offer to bring music in stands over the window, as it does at a first start.
    pub welcome: bool,
    /// The lyrics of what plays in the app's language: whose, into which language, and them.
    pub translation: Option<(String, String, Translation)>,
    /// What goes with the playlist last opened.
    pub suggested: Option<Suggested>,
    /// The versions published and what each says of itself; none while they are asked for.
    pub patches: Option<Result<Vec<Patch>, String>>,
    patches_asked: bool,
    /// The footage asked for, with the song it belongs to; none yet while it is fetched,
    /// and still none when the song has no video of its own.
    pub clip: Option<(String, Option<Arc<Footage>>)>,
    /// Who the artist of what plays is, asked by their name as the track writes it:
    /// nobody yet while they are looked for, and still nobody when the catalog knows none.
    pub about: Option<(String, Option<Known>)>,
    /// A later version of the app, and how far taking it in has got.
    pub update: Option<Release>,
    pub updating: Updating,
    /// The later version is still to be said over the window: its card there has been
    /// neither closed nor followed.
    pub update_toast: bool,
    /// The later version is in place and is to be started in this one's stead; the window
    /// takes it.
    pub restart: bool,
    /// How many of the news were still to be looked at when their page was opened.
    pub news_fresh: usize,
    /// An address on its way to the browser; the window takes it.
    pub open_url: Option<String>,
    /// What plays fills the window: its video, or its cover.
    pub cinema: bool,
    /// The song whose video's own sound plays in its place.
    swapped: Option<Track>,
    /// The lyrics asked for, with the track they belong to.
    pub lyrics: Option<(String, Lyrics)>,
    /// The line the list was last scrolled to.
    lyric_shown: Option<usize>,
    /// How far the rows of the lyrics still trail the list's last jump to the line being sung.
    lyric_glide: ScalarSpring,
    /// How far the player's lyric card has slid its lines up, so the one being sung rests second.
    lyric_card: ScalarSpring,
    /// Whether the card was on screen at the last layout; one that appears is in place at once.
    lyric_card_shown: bool,
    /// Consumed by the Host before following the new track's first timed line.
    pub(crate) lyrics_scroll_reset: bool,
    pub outputs: Vec<AudioOutputDevice>,
    pub outputs_loading: bool,
    pub outputs_error: Option<String>,
    pub output_notice: Option<String>,
    pub output: Option<String>,
    pub output_open: bool,
    /// The playback bar's menu of where it sounds.
    pub device_open: bool,
    /// The sleep timer's menu.
    pub sleep_open: bool,
    /// The category of the SponsorBlock segment last jumped over, and when.
    pub skip_notice: Option<(String, Instant)>,
}

/// How many covers a session holds, a few tens of megabytes of them.
const ARTWORK_KEPT: usize = 400;

/// One request per cover, however many rows show it and however often it comes up.
// shortcut: covers come at `COVER_SIDE` whatever their place; ask for the size on screen
// when the bytes matter.
fn cover_requests<'a>(asked: &mut HashSet<String>, urls: impl IntoIterator<Item = &'a Option<String>>) -> Vec<Command> {
    let urls = urls.into_iter().flatten();
    urls.filter(|url| asked.insert((*url).clone())).map(|url| Command::FetchArtwork(url.clone())).collect()
}

/// The action a control of limn's top bar or of the closed panel's strip stands for, under
/// the id the rest of the app knows it by: `nav:search` is the search, `nav:home` home.
fn desktop_alias(action: &UiAction) -> Option<UiAction> {
    let alias = |id: &str| match id {
        "nav:search" => Some("search"),
        "nav:search:clear" => Some("search-clear"),
        "nav:home" => Some("home"),
        "nav:browse" => Some("browse"),
        "panel-peek" => Some("toggle-panel-open"),
        "library-rail:expand" => Some("toggle-library-open"),
        "library-rail:create" => Some("playlist-new"),
        // The panel of what plays has the bar's own controls; its ids are its own.
        "panel-like" => Some("like"),
        "panel-cinema" => Some("cinema"),
        "panel-share" | "panel-go:share" => Some("now-share"),
        "panel-go:album" => Some("now-album"),
        "panel-go:artist" => Some("now-artist"),
        _ => None,
    };
    match action {
        UiAction::Clicked { id } => alias(id).map(|id| UiAction::Clicked { id: id.into() }),
        UiAction::InputChanged { id, value } => alias(id).map(|id| UiAction::InputChanged { id: id.into(), value: value.clone() }),
        UiAction::Submitted { id, value } => alias(id).map(|id| UiAction::Submitted { id: id.into(), value: value.clone() }),
        _ => None,
    }
}

/// Whether the keyboard at `id` keeps the recent searches dropped down: the desktop's field,
/// a row, or its options' dots and menu, whose ids all carry the row's.
pub fn keeps_recent_searches(id: &str) -> bool {
    id == "nav:search" || id.contains("recent-search")
}

/// Where a name that reaches `reach` past its room stands `elapsed` after it showed, as
/// Spotify's: still a while, along to its end at an even pace, still again, and back.
pub fn marquee(elapsed: Duration, reach: f32) -> f32 {
    let travel = reach / MARQUEE_SPEED;
    let pause = MARQUEE_PAUSE.as_secs_f32();
    let at = elapsed.as_secs_f32() % (2.0 * (pause + travel));
    if at < pause { 0.0 }
    else if at < pause + travel { (at - pause) * MARQUEE_SPEED }
    else if at < 2.0 * pause + travel { reach }
    else { reach - (at - 2.0 * pause - travel) * MARQUEE_SPEED }
}

/// How fast a long name slides, in points a second, and how long it rests at either end.
const MARQUEE_SPEED: f32 = 30.0;
const MARQUEE_PAUSE: Duration = Duration::from_millis(2500);

/// How long the home's wash takes to change colour, and how strong it is at the top.
const WASH_FADE: Duration = Duration::from_millis(1000);
const WASH_ALPHA: u8 = 150;

/// `a` taken `share` of the way to `b`, alpha and all.
pub fn blend(a: limn::Color, b: limn::Color, share: f32) -> limn::Color {
    let share = share.clamp(0.0, 1.0);
    let channel = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * share).round() as u8;
    limn::Color::rgba(channel(a.r, b.r), channel(a.g, b.g), channel(a.b, b.b), channel(a.a, b.a))
}

fn covers_of(tracks: &[Track]) -> impl Iterator<Item = &Option<String>> {
    tracks.iter().map(|track| &track.artwork_url)
}

/// The link that shares a track: the catalog's own address for it, which opens for
/// anyone, and which the search box here takes for the track itself.
pub fn share_link(track: &Track) -> String {
    format!("https://music.youtube.com/watch?v={}", track.id)
}

/// The id of the track a link shares: the catalog's address for it in any of its forms,
/// or the app's own `surco://cancion/<id>`. Anything else is not a link: a search.
pub fn shared_song(text: &str) -> Option<&str> {
    let text = text.trim();
    let watched = ["https://music.youtube.com/watch?", "https://www.youtube.com/watch?", "https://youtube.com/watch?"];
    let id = match watched.iter().find_map(|address| text.strip_prefix(address)) {
        Some(asked) => asked.split('&').find_map(|pair| pair.strip_prefix("v="))?,
        None => ["surco://cancion/", "https://youtu.be/"].iter().find_map(|address| text.strip_prefix(address))?,
    };
    let id = id.split(['/', '?', '&', '#']).next()?;
    (!id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')).then_some(id)
}

/// The most tracks a shared playlist brings.
const SHARED_LIST_MOST: usize = 300;
/// What a playlist's name keeps as it is inside a link.
const LINK_NAME: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC.remove(b'-').remove(b'_').remove(b'.');

/// The link that shares a playlist. It carries the playlist itself, its name and the ids
/// of its tracks, so opening it needs no account and no server: whoever pastes it into the
/// search box gets a copy.
pub fn list_link(playlist: &Playlist) -> String {
    let ids: Vec<&str> = playlist.tracks.iter().map(|track| track.id.as_str()).collect();
    format!("surco://lista/{}?nombre={}", ids.join(","), percent_encoding::utf8_percent_encode(&playlist.name, LINK_NAME))
}

/// What the id of a link in a release's notes begins with.
pub const RELEASE_LINK: &str = "release-link:";

/// The name and the track ids a playlist's link carries. Anything else is not such a link.
pub fn shared_list(text: &str) -> Option<(String, Vec<String>)> {
    let rest = text.trim().strip_prefix("surco://lista/")?;
    let (ids, query) = rest.split_once('?').unwrap_or((rest, ""));
    let valid = |id: &&str| !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    let ids: Vec<String> = ids.split(',').filter(valid).take(SHARED_LIST_MOST).map(str::to_owned).collect();
    let name = query.split('&').find_map(|pair| pair.strip_prefix("nombre="))?;
    let name = percent_encoding::percent_decode_str(name).decode_utf8().ok()?.trim().to_owned();
    (!ids.is_empty() && !name.is_empty()).then_some((name, ids))
}

/// The side every cover is fetched at: a phone draws its rows and shelves at three pixels a
/// point, and the catalog lists most covers at 60 or 120. limn decodes each at the size it is drawn.
pub const COVER_SIDE: u32 = 544;

/// A cover's address at the size a large picture needs: a phone's player is 380 points wide.
pub fn large_cover(url: &str) -> String {
    cover_at(url, 1200)
}

/// A cover's address at `side` pixels or more. The catalog's addresses end in the size they were
/// listed at (`=w120-h120-…`, `=s120-…`): a smaller one is scaled up to `side`, keeping its
/// shape. Any other address is returned as it is.
pub fn cover_at(url: &str, side: u32) -> String {
    fn number(text: &str) -> Option<(u32, &str)> {
        let end = text.find(|c: char| !c.is_ascii_digit()).unwrap_or(text.len());
        text[..end].parse().ok().filter(|number| *number > 0).map(|number| (number, &text[end..]))
    }
    let sized = || {
        let (head, tail) = url.rsplit_once('=')?;
        // The size closes the path; an `=` in a query or before another segment is something else.
        if head.contains('?') || tail.contains(['/', '?', '#']) {
            return None;
        }
        if let Some((listed, rest)) = tail.strip_prefix('s').and_then(number) {
            return Some(format!("{head}=s{}{rest}", listed.max(side)));
        }
        let (width, rest) = number(tail.strip_prefix('w')?)?;
        let (height, rest) = number(rest.strip_prefix("-h")?)?;
        let scale = (f64::from(side) / f64::from(width.max(height))).max(1.0);
        let grown = |listed: u32| (f64::from(listed) * scale).round() as u32;
        Some(format!("{head}=w{}-h{}{rest}", grown(width), grown(height)))
    };
    sized().unwrap_or_else(|| url.to_owned())
}

/// Where a track's artist lives: by link when the catalog gave one, by name otherwise.
fn artist_of(track: &Track) -> Target {
    track.artist_id.clone().map_or_else(|| Target::ArtistNamed(track.artist.clone()), Target::Artist)
}

/// The equaliser's range, each side of zero.
pub const EQUALIZER_DB: f32 = 12.0;

/// The equaliser's ready-made curves: the id after `equalizer-preset:`, the name, and the bands.
/// Each leaves the preamp as it is.
pub const EQUALIZER_PRESETS: [(&str, Label, [f32; 10]); 6] = [
    ("flat", Label("state-preset-flat"), [0.0; 10]),
    ("bass", Label("state-preset-bass"), [6.0, 5.0, 4.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
    ("voice", Label("state-preset-voice"), [-3.0, -2.0, -1.0, 1.0, 3.0, 4.0, 4.0, 2.0, 0.0, -1.0]),
    ("treble", Label("state-preset-treble"), [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 4.0, 5.0, 6.0]),
    ("acoustic", Label("state-preset-acoustic"), [4.0, 3.0, 2.0, 1.0, 1.0, 1.0, 2.0, 3.0, 3.0, 2.0]),
    ("electronic", Label("state-preset-electronic"), [5.0, 4.0, 1.0, 0.0, -2.0, 1.0, 0.0, 2.0, 4.0, 5.0]),
];

/// The ready-made curve the bands are set to, by its id and name, if they are one.
pub fn equalizer_preset(values: EqualizerValues) -> Option<(&'static str, &'static str)> {
    EQUALIZER_PRESETS.iter().find(|(_, _, bands)| *bands == values.bands_db).map(|(key, name, _)| (*key, name.text()))
}

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
            mobile: cfg!(target_os = "ios"),
            caption: (0.0, 0.0),
            mobile_search: false,
            mobile_ui: mobile::MobileUi::default(),
            query: String::new(),
            submitted: None,
            searching: false,
            search_due: None,
            search_filter: mobile::SearchFilter::All,
            found: Found::default(),
            search_error: None,
            page: Page::Search,
            history: Vec::new(),
            ahead: Vec::new(),
            player: PlayerView::default(),
            seek_preview: None,
            volume: 0.8,
            page_search: String::new(),
            page_searching: false,
            page_search_slide: ScalarTransition::new(0.0, Instant::now()),
            library_search_slide: ScalarTransition::new(0.0, Instant::now()),
            focus: None,
            library_open: true,
            library_slide: ScalarTransition::new(1.0, Instant::now()),
            panel_open: true,
            panel_slide: ScalarTransition::new(1.0, Instant::now()),
            quick_hovered: None,
            wash_from: limn::Color::rgba(0, 0, 0, 0),
            wash: ScalarTransition::new(1.0, Instant::now()),
            motion: false,
            queue_open: false,
            collection: Collection::default(),
            profile: Profile::default(),
            account_open: false,
            page_more_open: false,
            clip_ready: None,
            title_reach: None,
            handoff: None,
            onboarding: None,
            wide: false,
            panel_more_open: false,
            profile_draft: None,
            filter: Filter::All,
            library_search: String::new(),
            library_searching: false,
            library_menu: false,
            settings: Settings::default(),
            creating: None,
            importing: None,
            import_unknown: false,
            import_source: ImportSource::default(),
            imported: None,
            play_when: None,
            missed_open: false,
            import_progress: None,
            confirm: None,
            saved_bytes: 0,
            left_width: Side::Left.limits().default,
            right_width: Side::Right.limits().default,
            resizing: None,
            menu: None,
            equalizer_open: false,
            equalizer: EqualizerValues::FLAT,
            equalizer_band: None,
            artwork: HashMap::new(),
            track_window: None,
            page_stuck: false,
            artwork_order: VecDeque::new(),
            tints: HashMap::new(),
            picture_sizes: HashMap::new(),
            sharing: None,
            copied: false,
            list_copied: None,
            list_exported: None,
            copy: None,
            paste: None,
            covers: HashSet::new(),
            missing_covers: HashSet::new(),
            saved: Vec::new(),
            saving: HashSet::new(),
            save_error: None,
            levels: [0.0; surco_audio::meter::BANDS],
            mix: Vec::new(),
            because: Vec::new(),
            picked_artists: Vec::new(),
            picked_albums: Vec::new(),
            recent_picks: Vec::new(),
            fans_of: Vec::new(),
            more_of: None,
            picked_for: None,
            picks_coming: [false; 3],
            welcome: false,
            search_focused: false,
            panel_scrolled: false,
            translation: None,
            suggested: None,
            patches: None,
            patches_asked: false,
            clip: None,
            about: None,
            update: None,
            updating: Updating::Idle,
            update_toast: false,
            restart: false,
            news_fresh: 0,
            open_url: None,
            cinema: false,
            swapped: None,
            page_sort: None,
            sort_open: false,
            album_width: 180.0,
            album_shown: true,
            duration_shown: true,
            column_drag: None,
            lyrics: None,
            lyric_shown: None,
            lyric_glide: ScalarSpring::new(0.0, Instant::now()).motion(SpringMotion::settling_in(LYRIC_GLIDE)),
            lyric_card: ScalarSpring::new(0.0, Instant::now()).motion(SpringMotion::settling_in(LYRIC_GLIDE)),
            lyric_card_shown: false,
            lyrics_scroll_reset: false,
            outputs: Vec::new(),
            outputs_loading: false,
            outputs_error: None,
            output_notice: None,
            output: None,
            output_open: false,
            device_open: false,
            sleep_open: false,
            skip_notice: None,
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
        self.search_due = None;
        if self.mobile {
            self.mobile_search = true;
        }
        // A link to a track is not searched for: it opens the track it shares.
        if let Some(id) = shared_song(&query) {
            return self.open(Target::Song(id.to_owned()));
        }
        // Nor is a playlist's: it is brought in as a copy.
        if let Some((name, ids)) = shared_list(&query) {
            return self.open(Target::List { name, ids });
        }
        // And so is a list of another app's.
        if transfer::source(&query).is_some() {
            return self.open(Target::Import(query));
        }
        if !query.contains("://") {
            self.collection.searched(Searched::Query(query.clone()));
        }
        self.find(query)
    }

    /// Asks the catalog for `query`, whose answer the search page waits for.
    fn find(&mut self, query: String) -> Vec<Command> {
        self.submitted = Some(query.clone());
        self.searching = true;
        self.search_error = None;
        if self.page != Page::Search {
            // The order and the search of the page left are not the results'.
            if self.page_searching { self.search_page(false); }
            (self.page_sort, self.sort_open, self.track_window) = (None, false, None);
        }
        self.page = Page::Search;
        self.history.clear();
        self.ahead.clear();
        vec![Command::Search(query)]
    }

    pub fn can_go_back(&self) -> bool {
        !self.history.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.ahead.is_empty()
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
        let pictures = collection.playlists.iter().map(|playlist| &playlist.picture).chain([&self.profile.picture]);
        let searched = collection.searches.iter().filter_map(|searched| match searched {
            Searched::Query(_) => None,
            Searched::Track { track } => Some(&track.artwork_url),
            Searched::Entry(entry) => Some(entry.artwork()),
        });
        cover_requests(&mut self.covers, collection.kept.iter().chain(&collection.recent).map(Entry::artwork).chain(pictures).chain(searched))
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

    /// What of the account travels: the parts chosen of it, as they are here.
    pub fn bundle(&self, parts: Parts) -> Bundle {
        let collection = &self.collection;
        let part = |sent: bool, tracks: &Vec<Track>| if sent { tracks.clone() } else { Vec::new() };
        Bundle {
            version: handoff::VERSION,
            profile: parts.profile.then(|| self.profile.clone()),
            playlists: if parts.playlists { collection.playlists.clone() } else { Vec::new() },
            liked: part(parts.liked, &collection.liked),
            kept: if parts.library { collection.kept.clone() } else { Vec::new() },
            heard: part(parts.history, &collection.heard),
            settings: parts.settings.then_some(self.settings),
            equalizer: parts.equalizer.then(|| equalizer_text(self.equalizer)),
            pictures: Vec::new(),
        }
    }

    /// Joins an account from another device to this one: each part chosen is added to what
    /// is here, or takes its place when `replace`. The profile, the settings and the equaliser
    /// take the place of these.
    fn take_in(&mut self, bundle: Bundle, parts: Parts, replace: bool) -> Vec<Command> {
        let collection = &mut self.collection;
        if parts.playlists {
            if replace { collection.playlists.clear(); }
            // Each goes first: from the last, they keep their order.
            bundle.playlists.into_iter().rev().for_each(|playlist| collection.adopt(playlist));
        }
        if parts.liked {
            if replace { collection.liked.clear(); }
            for track in bundle.liked {
                if !collection.liked.iter().any(|held| held.id == track.id) { collection.liked.push(track); }
            }
        }
        if parts.library {
            if replace { collection.kept.clear(); }
            for entry in bundle.kept {
                if !collection.keeps(&entry) { collection.kept.push(entry); }
            }
        }
        if parts.history {
            if replace { collection.heard.clear(); }
            collection.heard.extend(bundle.heard);
            collection.heard.truncate(HEARD_KEPT);
        }
        let mut commands = Vec::new();
        if let Some(profile) = bundle.profile.filter(|_| parts.profile) { self.profile = profile; }
        if let Some(settings) = bundle.settings.filter(|_| parts.settings) {
            self.settings = settings;
            if let Some(language) = settings.language { crate::i18n::set(language); }
            commands.extend([Command::Autoplay(settings.autoplay), Command::Crossfade(f64::from(settings.crossfade)), Command::Quality(settings.quality.bps()), Command::Mono(settings.mono), Command::Normalize(settings.normalize), Command::SponsorBlock(settings.sponsorblock)]);
        }
        if let Some(equalizer) = bundle.equalizer.as_deref().filter(|_| parts.equalizer).and_then(equalizer_from) {
            self.equalizer = equalizer;
            commands.push(Command::Equalizer(equalizer));
        }
        commands.extend(self.collection_covers());
        commands
    }

    /// Opens the handoff on what another device serves at `link`, as the code a phone's camera
    /// reads hands it over, and asks for it.
    #[cfg(any(target_os = "ios", test))]
    pub fn receive_from(&mut self, link: &str) -> Vec<Command> {
        self.handoff = Some(Handoff { receiving: true, address: link.to_owned(), replace: self.onboarding.is_some(), ..Handoff::default() });
        // On the first run it is the step that brings an account.
        if let Some(onboarding) = self.onboarding.as_mut() { onboarding.step = onboarding::Step::Bring; }
        self.click("handoff-fetch")
    }

    /// The parts a bundle that came has something of.
    fn parts_of(bundle: &Bundle) -> Parts {
        Parts {
            profile: bundle.profile.is_some(),
            playlists: !bundle.playlists.is_empty(),
            liked: !bundle.liked.is_empty(),
            library: !bundle.kept.is_empty(),
            history: !bundle.heard.is_empty(),
            settings: bundle.settings.is_some(),
            equalizer: bundle.equalizer.is_some(),
        }
    }

    /// What the user plays most: what was played again, the most played first.
    pub fn top_tracks(&self) -> Vec<Track> {
        discover::on_repeat(&self.collection.heard, TOP_TRACKS)
    }

    /// Whether the home is shown, with nothing searched.
    pub fn at_home(&self) -> bool {
        self.page == Page::Search && self.submitted.is_none() && !self.searching
    }

    /// Whether something on the page moves by itself, slowly, for as long as it is shown: the
    /// colours of the time of day's mix. Never in a screenshot or under reduced motion.
    pub fn ambient(&self) -> bool {
        self.motion && !self.settings.reduce_motion && self.at_home() && !self.mix.is_empty()
    }

    /// Follows the pointer over the home's quick access: the page's wash takes the colour of
    /// the entry under it, over a second as Spotify's, and gives it back when it leaves. Says
    /// whether that changed.
    pub fn point_at(&mut self, hovered: Option<&str>) -> bool {
        let entry = hovered.map(|id| id.strip_prefix("tile-play:").unwrap_or(id)).filter(|id| id.starts_with("recent:")).map(str::to_owned);
        if entry == self.quick_hovered {
            return false;
        }
        let now = Instant::now();
        self.wash_from = self.home_wash(now);
        self.quick_hovered = entry;
        self.wash = ScalarTransition::new(0.0, now);
        self.wash.retarget_with_motion(1.0, now, WASH_FADE, !self.motion || self.settings.reduce_motion);
        true
    }

    /// The colour washing down from the top of the home at `now`: the cover of the entry
    /// under the pointer, or the first one's while none is.
    pub fn home_wash(&self, now: Instant) -> limn::Color {
        let index = self.quick_hovered.as_deref().and_then(|id| id.strip_prefix("recent:")?.parse::<usize>().ok()).unwrap_or(0);
        let tint = self.collection.recent.get(index).and_then(|entry| entry.artwork().as_ref()).and_then(|url| self.tints.get(url));
        let to = tint.map_or(limn::Color::rgba(0, 0, 0, 0), |tint| limn::Color { a: WASH_ALPHA, ..*tint });
        blend(self.wash_from, to, self.wash.value(now))
    }

    /// Puts the panels where a saved session left them, with no travel.
    pub fn restore_sides(&mut self, library_open: bool, panel_open: bool) {
        let now = Instant::now();
        self.library_open = library_open;
        self.library_slide = ScalarTransition::new(if library_open { 1.0 } else { 0.0 }, now);
        self.panel_open = panel_open;
        self.panel_slide = ScalarTransition::new(if panel_open { 1.0 } else { 0.0 }, now);
    }

    /// Whether a side panel, or the page's search, is on its way open or shut.
    pub fn is_sliding(&self, now: Instant) -> bool {
        self.library_slide.is_active(now) || self.panel_slide.is_active(now) || self.page_search_slide.is_active(now) || self.library_search_slide.is_active(now)
            || self.wash.is_active(now)
            || self.lyric_glide.is_active(now.checked_sub(LYRIC_RIPPLE * LYRIC_RIPPLE_ROWS as u32).unwrap_or(now))
            || self.lyric_card.is_active(now)
    }

    /// Opens the search of the page's tracks, with the keyboard in it, or shuts it and
    /// forgets what it was looking for. Its field grows out of its magnifier and back.
    fn search_page(&mut self, open: bool) {
        self.page_searching = open;
        self.page_search.clear();
        self.focus = open.then(|| "page-query".to_owned());
        let travel = Duration::from_millis(u64::from(limn::tokens().motion.move_ms));
        self.page_search_slide.retarget_with_motion(if open { 1.0 } else { 0.0 }, Instant::now(), travel, !self.motion);
    }

    /// Whether a track is one the page's search is looking for: every track while nothing
    /// is searched, and otherwise those whose title, artist or album holds what was typed,
    /// whatever its case.
    pub fn page_has(&self, track: &Track) -> bool {
        let wanted = self.page_search.trim().to_lowercase();
        let holds = |text: &str| text.to_lowercase().contains(&wanted);
        wanted.is_empty() || holds(&track.title) || holds(&track.artist) || track.album.as_deref().is_some_and(holds)
    }

    pub fn is_liked(&self, track_id: &str) -> bool {
        self.collection.liked.iter().any(|track| track.id == track_id)
    }

    pub fn is_saved(&self, track_id: &str) -> bool {
        self.saved.iter().any(|track| track.id == track_id)
    }

    /// Whether every song of a list shown is kept for listening without a network, where the
    /// page is a list that can be: an album, a playlist, a mix or the liked songs.
    pub fn page_downloaded(&self) -> Option<bool> {
        let list = matches!(self.page, Page::Album(_) | Page::Playlist(_) | Page::Mix(_) | Page::Liked);
        let tracks = self.page_tracks();
        (list && !tracks.is_empty()).then(|| tracks.iter().all(|track| self.is_saved(&track.id)))
    }

    /// Leaves the current page for another. A page that never loaded is not one to return to.
    fn go(&mut self, page: Page) {
        self.list_copied = None;
        self.list_exported = None;
        self.imported = None;
        self.missed_open = false;
        // A search and an order belong to the page they were made on.
        if self.page_searching {
            self.search_page(false);
        }
        (self.page_sort, self.sort_open) = (None, false);
        self.track_window = None;
        self.page_stuck = false;
        let leaving = std::mem::replace(&mut self.page, page);
        if !matches!(leaving, Page::Loading(_) | Page::Failed(..)) {
            self.history.push(leaving);
        }
        self.ahead.clear();
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
            ("about-artist", None, _) => self.about_artist().map(|(artist, _)| Target::Artist(artist.id.clone())),
            ("news", Some(index), _) => self.collection.news.get(index).map(|(release, _)| album(release)),
            ("pick-artist", Some(index), _) => self.picked_artists.get(index).map(|artist| Target::Artist(artist.id.clone())),
            ("pick-album", Some(index), _) => self.picked_albums.get(index).map(album),
            ("more-album", Some(index), _) => self.more_of.as_ref().and_then(|(_, albums)| albums.get(index)).map(album),
            ("more-artist", None, _) => self.more_of.as_ref().map(|(artist, _)| Target::Artist(artist.id.clone())),
            ("top-artist", Some(index), _) => discover::top_artists(&self.collection.heard, TOP_ARTISTS).get(index).map(artist_of),
            (fans, index, _) if fans.starts_with("fans-") => {
                // fans-2-album:4 is the fifth album for the fans of the third artist; fans-2 is that artist.
                let (shelf, part) = fans["fans-".len()..].split_once('-').unwrap_or((&fans["fans-".len()..], ""));
                let (artist, albums) = self.fans_of.get(shelf.parse::<usize>().ok()?)?;
                match (part, index) {
                    ("album", Some(index)) => albums.get(index).map(album),
                    ("", None) => Some(Target::Artist(artist.id.clone())),
                    _ => None,
                }
            }
            ("now-artist", None, _) => self.current().map(artist_of),
            ("now-album", None, _) => self.current().and_then(|track| track.album_id.clone()).map(Target::Album),
            _ => None,
        }
    }

    /// The result of the search a row or tile shows, from its id.
    fn result_at(&self, row: &str) -> Option<Searched> {
        let (list, index) = row.split_once(':')?;
        let index = index.parse::<usize>().ok()?;
        match list {
            "track" | "page-track" => self.found.tracks.get(index).map(|track| Searched::Track { track: track.clone() }),
            "artist" => self.found.artists.get(index).map(|artist| Searched::Entry(Entry::Artist(artist.clone()))),
            "album" => self.found.albums.get(index).map(|album| Searched::Entry(Entry::Album(album.clone()))),
            _ => None,
        }
    }

    /// The track a row shows, from the row's id.
    pub fn track_at(&self, row: &str) -> Option<&Track> {
        if row == "current" {
            return self.current();
        }
        let (list, index) = row.split_once(':')?;
        let index = index.parse::<usize>().ok()?;
        match list {
            "track" => self.found.tracks.get(index),
            "page-track" => self.page_tracks().get(index),
            "upcoming" => self.player.upcoming.get(index),
            "heard" => self.collection.heard.get(index),
            "today" => self.recent_picks.get(index),
            "liked-track" => self.collection.liked.get(index),
            "recent-search" => match self.collection.searches.get(index) {
                Some(Searched::Track { track }) => Some(track),
                _ => None,
            },
            _ => None,
        }
    }

    /// The lists the home makes: what goes with the time of day, one for each of the artists
    /// played most, opening with the song it grew from, and what was played again and again.
    pub fn mixes(&self) -> Vec<Mix> {
        let moment = (!self.mix.is_empty()).then(|| Mix { kind: MixKind::Moment, tracks: self.mix.clone() });
        let artists = self.because.iter().filter(|(_, tracks)| !tracks.is_empty()).map(|(seed, tracks)| {
            Mix { kind: MixKind::Artist(seed.clone()), tracks: std::iter::once(seed).chain(tracks).cloned().collect() }
        });
        let repeat = discover::on_repeat(&self.collection.heard, REPEAT);
        let repeat = (repeat.len() >= REPEAT_FEWEST).then_some(Mix { kind: MixKind::Repeat, tracks: repeat });
        moment.into_iter().chain(artists).chain(repeat).collect()
    }

    /// The home's mix a card's id names, as `made:2`.
    pub fn mix_at(&self, row: &str) -> Option<Mix> {
        let index = row.strip_prefix("made:")?.parse::<usize>().ok()?;
        self.mixes().into_iter().nth(index)
    }

    /// Whether what plays came from `tracks` as a list: the song is one of them, and so is the
    /// one due after it. A song that is in it by chance, played alone, is not the list playing.
    // shortcut: the player keeps no record of the list it was handed, so its last song reads as
    // not from it; keep that list in PlayerView if a card's button must know it there too.
    pub fn plays_from(&self, tracks: &[Track]) -> bool {
        let has = |track: &Track| tracks.iter().any(|known| known.id == track.id);
        self.current().is_some_and(has) && self.player.upcoming.get(self.player.queued).is_some_and(has)
    }

    /// One of the actions of a row's menu.
    fn act(&mut self, action: &str, row: &str) -> Vec<Command> {
        let Some(track) = self.mobile_ui.context.as_ref().filter(|context| context.row == row)
            .map(|context| &context.track).or_else(|| self.track_at(row)).cloned() else { return Vec::new() };
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
            "like" => {
                self.collection.like(&track);
                Vec::new()
            }
            "unsave" => vec![Command::Unsave(track.id)],
            "unqueue" => {
                if self.mobile_ui.context.as_ref().and_then(|context| context.upcoming.as_ref()).is_some_and(|held| held != &self.player.upcoming) {
                    self.mobile_ui.notice = Some(t!("state-queue-changed").into_owned());
                    return Vec::new();
                }
                // Queue entries may contain the same song more than once. Keep
                // the selected occurrence, and reject a changed snapshot above.
                let index = row.strip_prefix("upcoming:").and_then(|index| index.parse().ok());
                index.map(|index| self.mobile_ui.context.as_ref().and_then(|context| context.upcoming.clone())
                    .map_or(Command::Unqueue(index), |snapshot| Command::UnqueueIfUnchanged { index, snapshot })).into_iter().collect()
            }
            "artist" => self.open(artist_of(&track)),
            "album" => track.album_id.map(Target::Album).map(|album| self.open(album)).unwrap_or_default(),
            // A track goes into a list that exists, or into one made for it.
            "add-new" => {
                self.creating = Some(Draft { first: Some(track), ..Draft::default() });
                Vec::new()
            }
            "remove" => {
                if let Some(id) = self.mobile_ui.context.as_ref().and_then(|context| context.playlist) {
                    if let Some(playlist) = self.collection.playlists.iter_mut().find(|playlist| playlist.id == id) {
                        playlist.tracks.retain(|held| held.id != track.id);
                    }
                    return Vec::new();
                }
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
            Page::Liked => &self.collection.liked,
            Page::Playlist(id) => self.collection.playlist(*id).map_or(&[], |playlist| &playlist.tracks),
            Page::Mix(mix) => &mix.tracks,
            // A song found plays the others after it, as a list's.
            Page::Search => &self.found.tracks,
            _ => &[],
        }
    }

    fn play_page(&self, index: usize) -> Vec<Command> {
        let tracks = self.page_tracks();
        if index >= tracks.len() {
            return Vec::new();
        }
        // Ordered, the page plays as it reads.
        let rows = self.page_rows();
        match rows.iter().position(|(place, _)| *place == index).filter(|_| self.page_sort.is_some()) {
            Some(index) => vec![Command::PlayList { tracks: rows.into_iter().map(|(_, track)| track.clone()).collect(), index }],
            None => vec![Command::PlayList { tracks: tracks.to_vec(), index }],
        }
    }

    /// The page's tracks as its table lists them, each with its place in the page's own
    /// order: the ones its search is looking for, in the order asked for.
    pub fn page_rows(&self) -> Vec<(usize, &Track)> {
        let tracks = self.page_tracks();
        // An artist's page shows the first of their songs, and all of them to its search.
        let shown = if matches!(self.page, Page::Artist(_)) && self.page_search.trim().is_empty() { ARTIST_SONGS_SHOWN } else { tracks.len() };
        let mut rows: Vec<(usize, &Track)> = tracks.iter().take(shown).enumerate().filter(|(_, track)| self.page_has(track)).collect();
        if let Some(sort) = &self.page_sort {
            match sort.key.as_str() {
                "title" => rows.sort_by_cached_key(|(_, track)| track.title.to_lowercase()),
                "album" => rows.sort_by_cached_key(|(_, track)| track.album.as_deref().unwrap_or_default().to_lowercase()),
                "duration" => rows.sort_by_key(|(_, track)| track.duration_ms),
                _ => {}
            }
            if sort.descending {
                rows.reverse();
            }
        }
        rows
    }

    pub fn update(&mut self, action: &UiAction) -> Vec<Command> {
        if self.mobile {
            if let Some(commands) = self.mobile_action(action) {
                return commands;
            }
        }
        // A choice in the account's menu closes it.
        if let UiAction::Clicked { id } = action {
            if id.starts_with("account-") || id == "settings" { self.account_open = false; }
            if id.starts_with("playlist-") { self.page_more_open = false; }
            if id.starts_with("panel-go:") { self.panel_more_open = false; }
        }
        // limn's top bar and the closed panel's strip name their controls after themselves;
        // here they are the controls they stand for.
        let renamed = desktop_alias(action);
        let action = renamed.as_ref().unwrap_or(action);
        match action {
            UiAction::CopyRequested { text } => {
                self.copy = Some(text.clone());
                Vec::new()
            }
            UiAction::PasteRequested { id } => {
                self.paste = Some(id.clone());
                Vec::new()
            }
            UiAction::InputChanged { id, value } if id == "search" => {
                self.type_query(value, Instant::now());
                Vec::new()
            }
            UiAction::InputChanged { id, value } if id == "library-query" => {
                self.library_search.clone_from(value);
                Vec::new()
            }
            UiAction::InputChanged { id, value } if id == "page-query" => {
                // Typing into a search that was shut meanwhile changes nothing.
                if self.page_searching {
                    self.page_search.clone_from(value);
                }
                Vec::new()
            }
            UiAction::InputChanged { id, value } if id == "onboarding-name" => {
                if let Some(onboarding) = self.onboarding.as_mut() { onboarding.name = value.chars().take(PROFILE_NAME_MOST).collect(); }
                Vec::new()
            }
            UiAction::Submitted { id, .. } if id == "onboarding-name" => self.click("onboarding-next"),
            UiAction::InputChanged { id, value } if id == "onboarding-search" => {
                self.onboarding_type(value, Instant::now());
                Vec::new()
            }
            UiAction::InputChanged { id, value } if id == "handoff-address" => {
                if let Some(handoff) = self.handoff.as_mut() { handoff.address = value.clone(); }
                Vec::new()
            }
            UiAction::Submitted { id, .. } if id == "handoff-address" => self.click("handoff-fetch"),
            UiAction::InputChanged { id, value } if id == "profile-name" => {
                self.profile_draft = Some(value.clone());
                Vec::new()
            }
            UiAction::Submitted { id, .. } if id == "profile-name" => self.click("profile-save"),
            UiAction::InputChanged { id, value } if id == "playlist-name" => {
                if let Some(draft) = &mut self.creating {
                    draft.name = value.clone();
                }
                Vec::new()
            }
            UiAction::Submitted { id, .. } if id == "playlist-name" => self.click("playlist-create"),
            UiAction::InputChanged { id, value } if id == "import-link" => {
                self.importing = Some(value.clone());
                self.import_unknown = false;
                // A link pasted says which app it is of.
                if let Some(source) = ImportSource::of(value) { self.import_source = source; }
                Vec::new()
            }
            UiAction::Submitted { id, .. } if id == "import-link" => self.click("import-go"),
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
                    Ok(band) if band < self.equalizer.bands_db.len() => {
                        self.equalizer.bands_db[band] = gain;
                        self.equalizer_band = Some(band);
                    }
                    Ok(_) => return Vec::new(),
                    Err(_) => self.equalizer.preamp_db = gain,
                }
                vec![Command::Equalizer(self.equalizer)]
            }
            UiAction::DragStarted { id, position } => {
                self.column_drag = (id == "tracks:resize:album").then_some((position.x, self.album_width));
                // A folded library is dragged out from its rail.
                self.resizing = [Side::Left, Side::Right].into_iter().find(|side| side.handle() == id).map(|side| {
                    let width = match side { Side::Left if !self.library_open => limn::SideRail::WIDTH, Side::Left => self.left_width, Side::Right => self.right_width };
                    (side, position.x, width)
                });
                Vec::new()
            }
            UiAction::DragMoved { position, .. } => {
                // The left panel grows as its handle goes right; the right one as its handle goes left.
                match self.resizing {
                    Some((Side::Left, from, width)) => {
                        // Dragged narrow enough, the library folds into its rail, and out of it, opens.
                        let wanted = width + position.x - from;
                        let open = wanted >= (limn::SideRail::WIDTH + Side::Left.limits().min) / 2.0;
                        if open != self.library_open {
                            self.open_side(Side::Left, open);
                        }
                        if open {
                            self.left_width = Side::Left.width(wanted);
                        }
                    }
                    Some((Side::Right, from, width)) => self.right_width = Side::Right.width(width - (position.x - from)),
                    None => {}
                }
                // The album column follows the title, which gives way: wider as its edge goes left.
                if let Some((from, width)) = self.column_drag {
                    self.album_width = (width - (position.x - from)).clamp(ALBUM_WIDTH.0, ALBUM_WIDTH.1);
                }
                Vec::new()
            }
            UiAction::Dropped { id, target, .. } => {
                (self.resizing, self.column_drag) = (None, None);
                // A row of the queue let go over another takes its place.
                let place = |row: &str| row.strip_prefix("upcoming:").and_then(|index| index.parse::<usize>().ok());
                match (place(id), target.as_deref().and_then(place)) {
                    (Some(from), Some(to)) if from != to => vec![Command::MoveQueue(from, to)],
                    _ => Vec::new(),
                }
            }
            UiAction::DragCancelled { .. } => {
                if let Some((_, width)) = self.column_drag.take() {
                    self.album_width = width;
                }
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
        if let Some(commands) = self.onboarding_click(id) {
            return commands;
        }
        // A link in what a release says of itself: its address comes last in its id.
        if let Some(link) = id.strip_prefix(RELEASE_LINK) {
            self.open_url = link.splitn(3, ':').nth(2).filter(|address| address.starts_with("https://") || address.starts_with("http://")).map(str::to_owned);
            return Vec::new();
        }
        if let Some(index) = id.strip_prefix("recent-search-remove:").and_then(|value| value.parse::<usize>().ok()) {
            if index < self.collection.searches.len() { self.collection.searches.remove(index); }
            return Vec::new();
        }
        if let Some(searched) = id.strip_prefix("recent-search:").and_then(|value| value.parse::<usize>().ok()).and_then(|index| self.collection.searches.get(index)).cloned() {
            return match searched {
                Searched::Query(query) => {
                    self.query = query;
                    self.search()
                }
                Searched::Track { track } => {
                    self.collection.searched(Searched::Track { track: track.clone() });
                    if self.current().is_some_and(|current| current.id == track.id) { vec![Command::PlayPause] } else { vec![Command::PlayList { tracks: vec![track], index: 0 }] }
                }
                Searched::Entry(entry) => {
                    let target = entry.target();
                    self.collection.searched(Searched::Entry(entry));
                    self.open(target)
                }
            };
        }
        if let Some(filter) = id.strip_prefix("search-filter:") {
            self.search_filter = match filter { "tracks" => SearchFilter::Tracks, "artists" => SearchFilter::Artists, "albums" => SearchFilter::Albums, _ => SearchFilter::All };
            return Vec::new();
        }
        // A result chosen is remembered among the searches, as Spotify's.
        if self.page == Page::Search {
            if let Some(searched) = self.result_at(id.strip_prefix("tile-play:").unwrap_or(id)) {
                self.collection.searched(searched);
            }
        }
        if let Some(query) = id.strip_prefix("browse:") {
            self.query = query.to_owned();
            return self.search();
        }
        // A tile's play button: a track does what its tile does, anything else is fetched and
        // played where the user is, as Spotify's cards do.
        if let Some(tile) = id.strip_prefix("tile-play:") {
            let tile = tile.strip_prefix("home-").unwrap_or(tile);
            // A mix plays from its start, or pauses if it is what plays.
            if let Some(mix) = self.mix_at(tile) {
                return if self.plays_from(&mix.tracks) { vec![Command::PlayPause] } else { vec![Command::PlayList { tracks: mix.tracks, index: 0 }] };
            }
            let top = || tile.strip_prefix("top-track:")?.parse::<usize>().ok().and_then(|index| self.top_tracks().into_iter().nth(index));
            if let Some(track) = self.track_at(tile).cloned().or_else(top) {
                if self.current().is_some_and(|current| current.id == track.id) { return vec![Command::PlayPause]; }
                return self.click(tile);
            }
            let Some(target) = self.target_of(tile) else { return Vec::new() };
            self.play_when = Some(target.clone());
            return vec![Command::Open(target)];
        }
        if let Some(key) = id.strip_prefix("pin:") {
            self.collection.pin(key);
            return Vec::new();
        }
        if let Some((_, _, bands)) = id.strip_prefix("equalizer-preset:").and_then(|key| EQUALIZER_PRESETS.iter().find(|preset| preset.0 == key)) {
            self.equalizer.bands_db = *bands;
            return vec![Command::Equalizer(self.equalizer)];
        }
        if let Some(row) = id.strip_prefix("more:") {
            self.menu = (self.menu.as_deref() != Some(row)).then(|| row.to_owned());
            return Vec::new();
        }
        if let Some((action, row)) = id.strip_prefix("do:").and_then(|rest| rest.split_once(':')) {
            self.menu = None;
            return self.act(action, row);
        }
        if id.starts_with("suggested") {
            return self.suggestion_action(id);
        }
        let indexed = |prefix: &str| id.strip_prefix(prefix).and_then(|index| index.parse::<usize>().ok());
        // The row of what plays pauses it and takes it up again; any other row starts its track.
        let row = indexed("track:").or(indexed("page-track:")).and_then(|_| self.track_at(id));
        if row.is_some_and(|track| self.current().is_some_and(|current| current.id == track.id)) {
            return vec![Command::PlayPause];
        }
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
            "liked" => {
                if self.page != Page::Liked {
                    self.go(Page::Liked);
                }
                cover_requests(&mut self.covers, covers_of(&self.collection.liked))
            }
            // The heart of the playback bar marks what plays.
            "like" => {
                if let Some(track) = self.player.current.clone() {
                    self.collection.like(&track);
                }
                Vec::new()
            }
            // As a browser's: back keeps the page left for forward, which a new page forgets.
            "back" => {
                if let Some(page) = self.history.pop() {
                    let left = std::mem::replace(&mut self.page, page);
                    if !matches!(left, Page::Loading(_) | Page::Failed(..)) {
                        self.ahead.push(left);
                    }
                }
                Vec::new()
            }
            "forward" => {
                if let Some(page) = self.ahead.pop() {
                    let left = std::mem::replace(&mut self.page, page);
                    self.history.push(left);
                }
                Vec::new()
            }
            // Shuffled, a page starts anywhere in itself; otherwise, at the top of what it lists.
            "page-play" | "stuck-play" => {
                let top = self.page_rows().first().map_or(0, |(place, _)| *place);
                self.play_page(if self.player.shuffle { fastrand::usize(..self.page_tracks().len().max(1)) } else { top })
            }
            "page-sort" | "page-sort-close" => {
                self.sort_open = id == "page-sort" && !self.sort_open;
                Vec::new()
            }
            // From the menu beside the search: the page's own order, or one of its columns.
            _ if id.starts_with("page-sort:") => {
                let key = &id["page-sort:".len()..];
                self.page_sort = (key != "own").then(|| TableSort { key: key.to_owned(), descending: false });
                self.sort_open = false;
                Vec::new()
            }
            // From the head of a column: up, down, and the page's own order again.
            _ if id.starts_with("tracks:sort:") => {
                self.page_sort = TableSort::toggled(self.page_sort.take(), &id["tracks:sort:".len()..]);
                Vec::new()
            }
            "tracks:column:album" => {
                self.album_shown = !self.album_shown;
                Vec::new()
            }
            "tracks:column:duration" => {
                self.duration_shown = !self.duration_shown;
                Vec::new()
            }
            // The queue is a face of the right panel: its button opens the panel on it, and
            // closes the panel from it.
            "page-search" => {
                self.search_page(!self.page_searching);
                Vec::new()
            }
            // Its button in the bar, and the link under what comes next in the panel.
            "queue" | "next-queue" => {
                self.queue_open = !self.queue_open;
                self.open_side(Side::Right, self.queue_open);
                let mut commands = self.upcoming_covers();
                commands.extend(self.ask_clip());
                commands.extend(self.ask_about());
                commands
            }
            // The lyrics are a page: their button opens it, and leaves it for the one before.
            "lyrics" => {
                if self.lyrics_open() {
                    self.page = self.history.pop().unwrap_or(Page::Search);
                } else {
                    self.go(Page::Lyrics);
                }
                self.ask_lyrics()
            }
            "retry-lyrics" => {
                let retry = self.current().zip(self.lyrics.as_ref()).is_some_and(|(track, (id, lyrics))| {
                    *id == track.id && match lyrics {
                        Lyrics::Failed { retry_at, .. } => Instant::now() >= *retry_at,
                        Lyrics::Missing => true,
                        _ => false,
                    }
                });
                if retry { self.lyrics = None; self.ask_lyrics() } else { Vec::new() }
            }
            _ if id.starts_with("heard:") => {
                let index = id["heard:".len()..].parse::<usize>().ok().filter(|index| *index < self.collection.heard.len());
                index.map(|index| Command::PlayList { tracks: self.collection.heard.clone(), index }).into_iter().collect()
            }
            _ if id.starts_with("today:") => {
                let index = id["today:".len()..].parse::<usize>().ok().filter(|index| *index < self.recent_picks.len());
                index.map(|index| Command::PlayList { tracks: self.recent_picks.clone(), index }).into_iter().collect()
            }
            _ if id.starts_with("liked-track:") => {
                let index = id["liked-track:".len()..].parse::<usize>().ok().filter(|index| *index < self.collection.liked.len());
                index.map(|index| Command::PlayList { tracks: self.collection.liked.clone(), index }).into_iter().collect()
            }
            _ if id.starts_with("made:") => {
                if let Some(mix) = self.mix_at(id) { self.go(Page::Mix(mix)); }
                Vec::new()
            }
            _ if id.starts_with("lyric:") => {
                // A line that is clicked is sung from its start.
                let line = id["lyric:".len()..].parse::<usize>().ok().and_then(|index| self.lines().get(index));
                line.map(|line| Command::Seek(line.at_ms as f64 / 1000.0)).into_iter().collect()
            }
            // The bell opens what is new, and leaves it for the page before. Opened, it has
            // all been looked at; what was still to be stays apart while the page shows.
            "lyrics-translate" => {
                self.settings.translate_lyrics = !self.settings.translate_lyrics;
                // Turned on again after a failure, it is asked for anew.
                if matches!(self.translation, Some((_, _, Translation::Failed))) {
                    self.translation = None;
                }
                self.ask_translation()
            }
            "patches" | "patches-retry" => {
                if self.page != Page::Patches {
                    self.go(Page::Patches);
                }
                // Asked once while they come, and again only after they could not be had.
                if self.patches_asked && !matches!(self.patches, Some(Err(_))) {
                    return Vec::new();
                }
                (self.patches, self.patches_asked) = (None, true);
                vec![Command::Patches]
            }
            "news" => {
                self.update_toast = false;
                if self.page == Page::News {
                    self.page = self.history.pop().unwrap_or(Page::Search);
                    return Vec::new();
                }
                self.go(Page::News);
                self.news_fresh = self.collection.news.iter().filter(|(_, unseen)| *unseen).count();
                self.collection.news.iter_mut().for_each(|(_, unseen)| *unseen = false);
                let covers: Vec<Option<String>> = self.collection.news.iter().map(|(album, _)| album.artwork_url.clone()).collect();
                cover_requests(&mut self.covers, &covers)
            }
            // The card of a later version leads to what is new, where what it brings is said.
            "update-toast" if self.page != Page::News => self.click("news"),
            "update-toast" | "update-toast-close" => {
                self.update_toast = false;
                Vec::new()
            }
            // The later version is taken in by the app itself where its release has this
            // system's program; where not, its page is opened for the user to fetch it.
            "update-get" => match (&self.update, &self.updating) {
                (Some(release), Updating::Idle | Updating::Failed(_)) if release.program.is_some() => {
                    self.updating = Updating::Fetching(0.0);
                    vec![Command::InstallUpdate(release.clone())]
                }
                (Some(release), Updating::Idle | Updating::Failed(_)) => {
                    self.open_url = Some(release.page.clone());
                    Vec::new()
                }
                _ => Vec::new(),
            },
            "update-page" => {
                self.open_url = self.update.as_ref().map(|release| release.page.clone());
                Vec::new()
            }
            "update-restart" => {
                self.restart = self.updating == Updating::Ready;
                Vec::new()
            }
            // What plays over the whole window, and back.
            "cinema" => {
                self.cinema = !self.cinema;
                let mut commands = self.ask_clip();
                commands.extend(self.resound());
                commands
            }
            // Each panel is shut from its own header and opened from the bar above.
            "toggle-panel" | "toggle-panel-open" | "setting:player-panel" => {
                self.open_side(Side::Right, !self.panel_open);
                self.queue_open = false;
                let mut commands = self.ask_clip();
                commands.extend(self.ask_about());
                commands
            }
            "toggle-library" | "toggle-library-open" | "setting:library-panel" => {
                self.open_side(Side::Left, !self.library_open);
                Vec::new()
            }
            "home" => {
                self.mobile_search = false;
                self.page = Page::Search;
                self.history.clear();
                self.ahead.clear();
                self.query.clear();
                self.submitted = None;
                self.searching = false;
                self.search_due = None;
                self.search_error = None;
                self.found = Found::default();
                self.ask_picks()
            }
            "mobile-search" => {
                self.mobile_search = true;
                if self.page != Page::Search {
                    self.go(Page::Search);
                }
                self.focus = Some("search".into());
                Vec::new()
            }
            "search-submit" => self.search(),
            "mobile-library" => {
                self.mobile_search = false;
                if self.page != Page::Library {
                    self.go(Page::Library);
                }
                Vec::new()
            }
            "mobile-playing" => {
                if self.current().is_some() && self.page != Page::Playing {
                    self.go(Page::Playing);
                }
                let mut commands = self.ask_lyrics();
                commands.extend(self.ask_about());
                commands
            }
            // A phone gives it a page; a window, a popover.
            "equalizer" if self.mobile => {
                if self.page != Page::Equalizer {
                    self.go(Page::Equalizer);
                }
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
            part if part.starts_with("settings-part:") => {
                if let Some(part) = SettingsPart::ALL.into_iter().find(|known| part.strip_prefix("settings-part:") == Some(known.slug())) {
                    self.go(Page::SettingsPart(part));
                }
                Vec::new()
            }
            "settings" => {
                if self.page != Page::Settings {
                    self.go(Page::Settings);
                }
                // The page names the chosen device, so it needs their list.
                self.outputs_loading = true;
                self.outputs_error = None;
                vec![Command::Outputs]
            }
            "setting:outputs-refresh" => {
                if self.outputs_loading {
                    return Vec::new();
                }
                self.outputs_loading = true;
                self.outputs_error = None;
                vec![Command::Outputs]
            }
            "setting:output" => {
                self.output_open = !self.output_open;
                if self.output_open {
                    self.outputs_loading = true;
                    self.outputs_error = None;
                    vec![Command::Outputs]
                } else { Vec::new() }
            }
            // The bar's own way to where it sounds, as Spotify's "Connect to a device".
            "device" => {
                self.device_open = !self.device_open;
                if self.device_open {
                    self.outputs_loading = true;
                    self.outputs_error = None;
                    vec![Command::Outputs]
                } else { Vec::new() }
            }
            "device-dismiss" => {
                self.device_open = false;
                Vec::new()
            }
            "sleep" => {
                self.sleep_open = !self.sleep_open;
                Vec::new()
            }
            "sleep-dismiss" => {
                self.sleep_open = false;
                Vec::new()
            }
            _ if id.starts_with("sleep:") => {
                use crate::player::SleepAfter;
                self.sleep_open = false;
                let after = match &id["sleep:".len()..] {
                    "end" => Some(SleepAfter::EndOfSong),
                    "off" => None,
                    minutes => match minutes.parse() { Ok(minutes) => Some(SleepAfter::Minutes(minutes)), Err(_) => return Vec::new() },
                };
                vec![Command::Sleep(after)]
            }
            // What plays, in the right panel: shown if it was not, or the panel shut.
            // What plays, handed around from its own panel.
            "now-share" => {
                self.sharing = self.current().cloned();
                self.copied = false;
                Vec::new()
            }
            // The account's menu: whose Surco this is, and its ways out.
            "account" => {
                self.account_open = !self.account_open;
                Vec::new()
            }
            "account-dismiss" => {
                self.account_open = false;
                Vec::new()
            }
            "panel-more" => {
                self.panel_more_open = !self.panel_more_open;
                Vec::new()
            }
            "panel-more-dismiss" => {
                self.panel_more_open = false;
                Vec::new()
            }
            "onboarding-taste" => self.begin_onboarding(true),
            "account-handoff" => {
                let handoff = Handoff::default();
                let serve = Command::Serve(self.bundle(handoff.parts));
                self.handoff = Some(handoff);
                vec![serve]
            }
            "handoff-close" => {
                self.handoff = None;
                vec![Command::StopServing]
            }
            "handoff-tab:send" | "handoff-tab:receive" => {
                let Some(handoff) = self.handoff.as_mut() else { return Vec::new() };
                handoff.receiving = id == "handoff-tab:receive";
                handoff.parts = Parts::default();
                handoff.offer = None;
                if handoff.receiving { return vec![Command::StopServing]; }
                let parts = handoff.parts;
                vec![Command::Serve(self.bundle(parts))]
            }
            _ if id.starts_with("handoff-part:") => {
                let Some(handoff) = self.handoff.as_mut() else { return Vec::new() };
                if let Some(part) = handoff.parts.by_name(&id["handoff-part:".len()..]) { *part = !*part; }
                if handoff.receiving { return Vec::new(); }
                let parts = handoff.parts;
                vec![Command::Serve(self.bundle(parts))]
            }
            "handoff-mode:add" | "handoff-mode:replace" => {
                if let Some(handoff) = self.handoff.as_mut() { handoff.replace = id == "handoff-mode:replace"; }
                Vec::new()
            }
            "handoff-scan" => vec![Command::ScanCode],
            "handoff-copy" => {
                self.copy = self.handoff.as_ref().and_then(|handoff| handoff.offer.as_ref()?.as_ref().ok()).map(|(offer, _)| offer.code.clone());
                Vec::new()
            }
            "handoff-fetch" => {
                let Some(handoff) = self.handoff.as_mut() else { return Vec::new() };
                match handoff::locate(&handoff.address) {
                    Some(url) => {
                        handoff.fetching = true;
                        handoff.fetched = None;
                        vec![Command::Fetch(url)]
                    }
                    None => {
                        handoff.fetched = Some(Err(t!("state-handoff-bad-address").into_owned()));
                        Vec::new()
                    }
                }
            }
            "handoff-take" => {
                let Some(handoff) = self.handoff.take() else { return Vec::new() };
                match handoff.fetched {
                    Some(Ok(bundle)) => {
                        let commands = self.take_in(bundle, handoff.parts, handoff.replace);
                        // An account brought in on the first run is the end of its steps.
                        if self.onboarding.is_some() { self.end_first_run(); }
                        commands
                    }
                    _ => Vec::new(),
                }
            }
            // A whole list kept for listening without a network, or let go once all of it is.
            "page-download" => match self.page_downloaded() {
                Some(true) => self.page_tracks().iter().map(|track| Command::Unsave(track.id.clone())).collect(),
                Some(false) => {
                    self.save_error = None;
                    self.page_tracks().iter().filter(|track| !self.is_saved(&track.id) && !self.saving.contains(&track.id)).cloned().map(Command::Save).collect()
                }
                None => Vec::new(),
            },
            "page-more" => {
                self.page_more_open = !self.page_more_open;
                Vec::new()
            }
            "page-more-dismiss" => {
                self.page_more_open = false;
                Vec::new()
            }
            "account-profile" => {
                if self.page != Page::Profile { self.go(Page::Profile); }
                Vec::new()
            }
            // A picture of its own for the list shown, or for the account, chosen from the
            // computer's files.
            "playlist-picture" => match self.page {
                Page::Playlist(id) => vec![Command::PickPicture(PictureFor::Playlist(id))],
                _ => Vec::new(),
            },
            "playlist-picture-remove" => {
                if let Page::Playlist(id) = self.page {
                    if let Some(playlist) = self.collection.playlist_mut(id) { playlist.picture = None; }
                }
                Vec::new()
            }
            "profile-picture" => vec![Command::PickPicture(PictureFor::Profile)],
            "profile-picture-remove" => {
                self.profile.picture = None;
                Vec::new()
            }
            "profile-edit" => {
                self.profile_draft = Some(self.profile.name.clone());
                Vec::new()
            }
            "profile-cancel" => {
                self.profile_draft = None;
                Vec::new()
            }
            "profile-save" => {
                if let Some(name) = self.profile_draft.take() {
                    self.profile.name = name.trim().chars().take(PROFILE_NAME_MOST).collect();
                }
                Vec::new()
            }
            _ if id.starts_with("top-track:") => {
                let tracks = self.top_tracks();
                let index = id["top-track:".len()..].parse::<usize>().ok().filter(|index| *index < tracks.len());
                index.map(|index| Command::PlayList { tracks, index }).into_iter().collect()
            }
            "now-playing-view" => {
                let showing = self.panel_open && !self.queue_open;
                self.queue_open = false;
                self.open_side(Side::Right, !showing);
                if showing { return Vec::new(); }
                let mut commands = self.ask_clip();
                commands.extend(self.ask_about());
                commands
            }
            "setting:output-dismiss" => {
                self.output_open = false;
                Vec::new()
            }
            _ if id.starts_with("setting:output:") => {
                (self.output_open, self.device_open) = (false, false);
                let chosen = Some(id["setting:output:".len()..].to_owned()).filter(|device| !device.is_empty());
                let retry = self.output_notice.take().is_some();
                if chosen == self.output && !retry {
                    return Vec::new();
                }
                self.output = chosen;
                vec![Command::Output(self.output.clone())]
            }
            "setting:autoplay" => {
                self.settings.autoplay = !self.settings.autoplay;
                vec![Command::Autoplay(self.settings.autoplay)]
            }
            "setting:mono" => {
                self.settings.mono = !self.settings.mono;
                vec![Command::Mono(self.settings.mono)]
            }
            "setting:normalize" => {
                self.settings.normalize = !self.settings.normalize;
                vec![Command::Normalize(self.settings.normalize)]
            }
            // On, it skips what most listeners want skipped; off, nothing is asked.
            "setting:sponsorblock" => {
                self.settings.sponsorblock = if self.settings.sponsorblock == 0 { crate::sponsorblock::DEFAULT } else { 0 };
                vec![Command::SponsorBlock(self.settings.sponsorblock)]
            }
            _ if id.starts_with("setting:sponsorblock:") => {
                let name = &id["setting:sponsorblock:".len()..];
                let Some(bit) = crate::sponsorblock::CATEGORIES.iter().position(|(category, _)| *category == name) else { return Vec::new() };
                self.settings.sponsorblock ^= 1 << bit;
                vec![Command::SponsorBlock(self.settings.sponsorblock)]
            }
            "setting:motion" => {
                self.settings.reduce_motion = !self.settings.reduce_motion;
                Vec::new()
            }
            // From the settings, and from the button over the cover.
            "setting:clips" | "cover-clips" => {
                self.settings.clips = !self.settings.clips;
                self.ask_clip()
            }
            _ if id.starts_with("setting:language:") => {
                if let Some(language) = Language::from_tag(&id["setting:language:".len()..]) {
                    self.settings.language = Some(language);
                    crate::i18n::set(language);
                }
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
            "playlist-share" => {
                if let Page::Playlist(id) = self.page {
                    self.copy = self.collection.playlist(id).map(list_link);
                    self.list_copied = self.copy.is_some().then_some(id);
                }
                Vec::new()
            }
            // Its songs as text, which is what carries a list to another app.
            "playlist-export" => {
                if let Page::Playlist(id) = self.page {
                    self.copy = self.collection.playlist(id).map(|playlist| transfer::as_text(&playlist.tracks));
                    self.list_exported = self.copy.is_some().then_some(id);
                }
                Vec::new()
            }
            "browse" => {
                if self.page != Page::Browse {
                    self.go(Page::Browse);
                }
                Vec::new()
            }
            "recent-search-clear" => {
                self.collection.searches.clear();
                Vec::new()
            }
            "search-clear" => {
                self.type_query("", Instant::now());
                self.focus = Some("search".to_owned());
                Vec::new()
            }
            "import-missed" | "import-missed-close" => {
                self.missed_open = id == "import-missed" && self.imported.as_ref().is_some_and(|(_, missed)| !missed.is_empty());
                Vec::new()
            }
            "import-missed-copy" => {
                self.copy = self.imported.as_ref().map(|(_, missed)| missed.join("\n"));
                Vec::new()
            }
            "welcome-close" | "welcome-import" => {
                (self.welcome, self.settings.welcomed) = (false, true);
                if id == "welcome-import" { self.click("import-open") } else { Vec::new() }
            }
            "import-open" => {
                self.importing = Some(String::new());
                self.import_unknown = false;
                Vec::new()
            }
            "import-cancel" => {
                self.importing = None;
                Vec::new()
            }
            _ if id.starts_with("import-source:") => {
                if let Some(source) = ImportSource::ALL.into_iter().find(|source| id == format!("import-source:{}", source.slug())) { self.import_source = source; }
                Vec::new()
            }
            "import-dismiss" => {
                self.settings.import_dismissed = true;
                Vec::new()
            }
            "import-go" => {
                let link = self.importing.clone().unwrap_or_default().trim().to_owned();
                self.import_unknown = transfer::source(&link).is_none();
                if self.import_unknown {
                    return Vec::new();
                }
                self.importing = None;
                self.import_progress = None;
                // Used once, the offer has done its work.
                self.settings.import_dismissed = true;
                self.open(Target::Import(link))
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
                Some(Confirm::ClearQueue) => vec![Command::ClearQueue],
                None => Vec::new(),
            },
            // Following from the panel that presents the artist of what plays.
            "about-follow" => {
                if let Some((artist, _)) = self.about_artist().cloned() {
                    self.collection.toggle(Entry::Artist(artist));
                }
                Vec::new()
            }
            "keep" => {
                if let Some(entry) = self.page_entry() {
                    self.collection.toggle(entry);
                }
                Vec::new()
            }
            "library-sort" | "library-menu-close" => {
                self.library_menu = id == "library-sort" && !self.library_menu;
                Vec::new()
            }
            _ if id.starts_with("library-sort:") || id.starts_with("library-view:") => {
                match &id["library-".len()..] {
                    "sort:added" => self.settings.library_sort = Sort::Added,
                    "sort:alphabetical" => self.settings.library_sort = Sort::Alphabetical,
                    "sort:creator" => self.settings.library_sort = Sort::Creator,
                    "view:compact" => self.settings.library_view = LibraryView::Compact,
                    "view:list" => self.settings.library_view = LibraryView::List,
                    "view:grid" => self.settings.library_view = LibraryView::Grid,
                    _ => {}
                }
                self.library_menu = false;
                Vec::new()
            }
            // The magnifier opens the library's search; shut, it forgets what was typed.
            "library-search" => {
                self.library_searching = !self.library_searching;
                self.library_search.clear();
                self.focus = self.library_searching.then(|| "library-query".to_owned());
                // Its field grows out of its magnifier and back, as a page's does.
                let travel = Duration::from_millis(u64::from(limn::tokens().motion.move_ms));
                self.library_search_slide.retarget_with_motion(if self.library_searching { 1.0 } else { 0.0 }, Instant::now(), travel, !self.motion);
                Vec::new()
            }
            "queue-clear" => {
                self.confirm = Some(Confirm::ClearQueue);
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
            "shuffle" | "page-shuffle" => vec![Command::Shuffle],
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
            // A list brought in becomes one of the user's own, wherever they have walked to
            // meanwhile; where they waited for it, that is the page shown.
            Event::Importing { done, of } => {
                self.import_progress = Some((done, of));
                Vec::new()
            }
            Event::Opened { target, result: Ok(Content::List { name, tracks, missed }) } => {
                self.import_progress = None;
                let id = self.collection.create(Draft { name, collaborative: false, first: None });
                for track in tracks {
                    self.collection.add(id, track);
                }
                if self.page != Page::Loading(target) {
                    return Vec::new();
                }
                self.query.clear();
                self.page = Page::Playlist(id);
                self.imported = Some((id, missed));
                self.page_covers()
            }
            Event::Opened { target, result } if self.play_when.as_ref() == Some(&target) && self.page != Page::Loading(target.clone()) => {
                self.play_when = None;
                let tracks = match result {
                    Ok(Content::Artist(artist)) => artist.songs,
                    Ok(Content::Album(album)) => album.tracks,
                    Ok(Content::Song(track)) => vec![track],
                    Ok(Content::List { .. }) | Err(_) => Vec::new(),
                };
                if tracks.is_empty() { Vec::new() } else { vec![Command::PlayList { tracks, index: 0 }] }
            }
            Event::Opened { target, result } if self.page == Page::Loading(target.clone()) => {
                self.page = match result {
                    Ok(Content::Artist(artist)) => Page::Artist(artist),
                    Ok(Content::Album(album)) => Page::Album(album),
                    Ok(Content::Song(track)) => Page::Song(track),
                    Ok(Content::List { .. }) => unreachable!("a list brought in was taken above"),
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
            Event::Releases { artist, releases } => {
                self.collection.learn(artist, releases, chrono::Datelike::year(&chrono::Local::now()));
                Vec::new()
            }
            Event::Update(release) => {
                // The pictures of what it says of itself are fetched as covers are.
                let pictures: Vec<Option<String>> = release.notes.iter().filter_map(|note| if let Note::Picture(address) = note { Some(Some(address.clone())) } else { None }).collect();
                // It is found again for as long as the app stays open, and said once.
                self.update_toast |= self.update.as_ref().is_none_or(|known| known.version != release.version);
                self.update = Some(release);
                cover_requests(&mut self.covers, &pictures)
            }
            Event::UpdateProgress(done) => {
                self.updating = Updating::Fetching(done);
                Vec::new()
            }
            // One that had to be taken starts as soon as it is in place.
            Event::UpdateInstalled => {
                self.updating = Updating::Ready;
                self.restart = self.update.as_ref().is_some_and(|release| release.mandatory);
                Vec::new()
            }
            Event::UpdateFailed(why) => {
                self.updating = Updating::Failed(why);
                Vec::new()
            }
            Event::About { artist, found } => {
                let covers = cover_requests(&mut self.covers, found.iter().map(|(artist, _)| &artist.artwork_url));
                if let Some((asked, known)) = &mut self.about {
                    if *asked == artist {
                        *known = found;
                    }
                }
                covers
            }
            Event::Clip { track_id, footage } => {
                if let Some((asked, found)) = &mut self.clip {
                    if *asked == track_id {
                        *found = footage;
                    }
                }
                self.resound()
            }
            Event::Discovered(Picks::Tracks { mix, because }) => {
                self.picks_coming[0] = false;
                // A seed's cover is its mix's picture.
                let seeds = because.iter().map(|(seed, _)| &seed.artwork_url);
                let covers = cover_requests(&mut self.covers, covers_of(&mix).chain(seeds).chain(because.iter().flat_map(|(_, tracks)| covers_of(tracks))));
                (self.mix, self.because) = (mix, because);
                covers
            }
            Event::ArtistsFound { key, artists } => self.onboarding_found(&key, artists),
            Event::Offered(offer) => {
                if let Some(handoff) = self.handoff.as_mut().filter(|handoff| !handoff.receiving) { handoff.offer = Some(offer); }
                Vec::new()
            }
            Event::Fetched(fetched) => {
                if let Some(handoff) = self.handoff.as_mut().filter(|handoff| handoff.receiving) {
                    handoff.fetching = false;
                    if let Ok(bundle) = &fetched { handoff.parts = Self::parts_of(bundle); }
                    handoff.fetched = Some(fetched);
                }
                Vec::new()
            }
            Event::Picture { of, name } => {
                match of {
                    PictureFor::Playlist(id) => if let Some(playlist) = self.collection.playlist_mut(id) { playlist.picture = Some(name.clone()) },
                    PictureFor::Profile => self.profile.picture = Some(name.clone()),
                }
                cover_requests(&mut self.covers, [&Some(name)])
            }
            Event::ArtworkMissing(url) => {
                self.missing_covers.insert(url);
                Vec::new()
            }
            Event::Patches(patches) => {
                let pictures: Vec<Option<String>> = patches.iter().flatten().flat_map(|patch| &patch.notes)
                    .filter_map(|note| if let Note::Picture(address) = note { Some(Some(address.clone())) } else { None }).collect();
                self.patches = Some(patches);
                cover_requests(&mut self.covers, &pictures)
            }
            Event::Suggested { playlist, tracks } => {
                let Some(suggested) = self.suggested.as_mut().filter(|suggested| suggested.playlist == playlist) else { return Vec::new() };
                suggested.asking = false;
                let fresh: Vec<Track> = tracks.into_iter().filter(|track| suggested.offered.insert(track.id.clone())).collect();
                // Out of new songs: what was offered may come round again, as on Spotify.
                if fresh.is_empty() { suggested.offered.clear(); }
                suggested.tracks.extend(fresh);
                self.suggested_covers()
            }
            Event::Discovered(Picks::Artists(artists)) => {
                self.picks_coming[1] = false;
                let covers = cover_requests(&mut self.covers, artists.iter().map(|artist| &artist.artwork_url));
                self.picked_artists = artists;
                covers
            }
            Event::Discovered(Picks::Albums(albums)) => {
                self.picks_coming[2] = false;
                let covers = cover_requests(&mut self.covers, albums.iter().map(|album| &album.artwork_url));
                self.picked_albums = albums;
                covers
            }
            Event::Discovered(Picks::Recent(tracks)) => {
                let covers = cover_requests(&mut self.covers, covers_of(&tracks));
                self.recent_picks = tracks;
                covers
            }
            Event::Discovered(Picks::FansOf(shelves)) => {
                let pictures = shelves.iter().flat_map(|(artist, albums)| std::iter::once(&artist.artwork_url).chain(albums.iter().map(|album| &album.artwork_url)));
                let covers = cover_requests(&mut self.covers, pictures);
                self.fans_of = shelves;
                covers
            }
            Event::Discovered(Picks::MoreOf(artist, albums)) => {
                let covers = cover_requests(&mut self.covers, std::iter::once(&artist.artwork_url).chain(albums.iter().map(|album| &album.artwork_url)));
                self.more_of = Some((artist, albums));
                covers
            }
            Event::Lyrics { track_id, lines } => {
                if let Some((asked, lyrics)) = &mut self.lyrics {
                    if *asked == track_id && (lines.is_some() || !matches!(lyrics, Lyrics::Lines(_) | Lyrics::Plain { .. })) {
                        *lyrics = lines.map_or(Lyrics::Missing, Lyrics::Lines);
                    }
                }
                Vec::new()
            }
            Event::Translated { track_id, to, lines } => {
                if let Some((asked, into, translation)) = &mut self.translation {
                    if *asked == track_id && *into == to {
                        *translation = match lines { Ok(Some(lines)) => Translation::Lines(lines), Ok(None) => Translation::Same, Err(_) => Translation::Failed };
                    }
                }
                Vec::new()
            }
            Event::LyricsPlain { track_id, lines, source } => {
                if let Some((asked, lyrics)) = &mut self.lyrics {
                    if *asked == track_id && !matches!(lyrics, Lyrics::Lines(_)) { *lyrics = Lyrics::Plain { lines, source }; }
                }
                Vec::new()
            }
            Event::LyricsFailed { track_id, message, retry_after } => {
                if let Some((asked, lyrics)) = &mut self.lyrics {
                    if *asked == track_id && !matches!(lyrics, Lyrics::Lines(_) | Lyrics::Plain { .. }) {
                        let now = Instant::now();
                        let retry_at = now.checked_add(retry_after).unwrap_or(now + Duration::from_secs(86_400));
                        *lyrics = Lyrics::Failed { message, retry_at };
                    }
                }
                Vec::new()
            }
            Event::Outputs(devices) => {
                self.outputs = devices;
                self.outputs_loading = false;
                self.outputs_error = None;
                Vec::new()
            }
            Event::OutputsFailed(message) => {
                self.outputs_loading = false;
                self.outputs_error = Some(message);
                Vec::new()
            }
            Event::OutputLost => {
                self.output = None;
                self.output_notice = Some(t!("state-output-lost").into_owned());
                Vec::new()
            }
            Event::OutputFailed(message) => {
                self.output_notice = Some(message);
                Vec::new()
            }
            Event::SaveFailed { title, message } => {
                self.save_error = Some(format!("{title}: {message}"));
                Vec::new()
            }
            Event::Artwork { url, image, tint, size } => {
                if let Some([red, green, blue]) = tint {
                    self.tints.insert(url.clone(), limn::Color::rgb(red, green, blue));
                }
                if let Some(size) = size.filter(|(wide, high)| *wide > 0 && *high > 0) {
                    self.picture_sizes.insert(url.clone(), size);
                }
                if self.artwork.insert(url.clone(), image).is_none() { self.artwork_order.push_back(url); }
                self.forget_old_covers();
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// Takes the player as the backend now describes it.
    pub fn show(&mut self, player: PlayerView) -> Vec<Command> {
        // A SponsorBlock segment just jumped over is said, for a moment.
        let jumped = |view: &PlayerView| view.skipped.as_ref().map_or(0, |(count, _)| *count);
        if jumped(&player) > jumped(&self.player) {
            self.skip_notice = player.skipped.as_ref().map(|(_, category)| (category.clone(), Instant::now()));
        }
        let before = self.player.current.as_ref().map(|track| track.id.as_str());
        let now = player.current.as_ref().map(|track| track.id.as_str());
        if before != now {
            self.lyrics_scroll_reset = true;
            self.lyric_shown = None;
            self.lyric_glide.snap_to(0.0, Instant::now());
        }
        // The video's sound in a song's place is that song still: until the song is back
        // with the window, or something else plays.
        let video = self.footage().map(|footage| footage.video.id.clone());
        if self.swapped.as_ref().is_some_and(|song| if now == Some(song.id.as_str()) { !self.cinema } else { now != video.as_deref() }) {
            self.swapped = None;
        }
        if let Some(track) = player.current.as_ref().filter(|track| before != Some(track.id.as_str()) && self.swapped.is_none()) {
            self.collection.heard(track);
        }
        self.player = player;
        // The playing track's cover is also shown large, in the right panel.
        let large = self.player.current.as_ref().and_then(|track| track.artwork_url.as_deref()).map(large_cover);
        // And the panel of what plays shows what comes next.
        let next = self.player.upcoming.first().filter(|_| self.shows_playing()).map(|track| &track.artwork_url);
        let mut commands = cover_requests(&mut self.covers, self.player.current.iter().map(|track| &track.artwork_url).chain([&large]).chain(next));
        commands.extend(self.upcoming_covers());
        commands.extend(self.ask_lyrics());
        commands.extend(self.ask_clip());
        commands.extend(self.ask_about());
        commands
    }

    /// Asks what the artists followed have out and whether the app has a later version:
    /// once when the window opens.
    pub fn ask_news(&mut self) -> Vec<Command> {
        let followed: Vec<String> = self.collection.artists().map(|(_, artist)| artist.id.clone()).collect();
        let news = (!followed.is_empty()).then_some(Command::News(followed));
        news.into_iter().chain([Command::CheckUpdate]).collect()
    }

    /// Whether the bell has something to say: news not looked at yet, or a later version.
    pub fn has_news(&self) -> bool {
        self.news_count() > 0
    }

    /// How many things the bell has to say.
    pub fn news_count(&self) -> usize {
        usize::from(self.update.is_some()) + self.collection.news.iter().filter(|(_, unseen)| *unseen).count()
    }

    /// The artist of what plays is looked for once, while the panel that presents them
    /// shows: by their name, so the next track of theirs asks nothing.
    fn ask_about(&mut self) -> Vec<Command> {
        let shown = self.shows_playing() || (self.mobile && self.page == Page::Playing);
        let Some(track) = self.player.current.as_ref().filter(|_| shown) else { return Vec::new() };
        if self.about.as_ref().is_some_and(|(asked, _)| *asked == track.artist) {
            return Vec::new();
        }
        self.about = Some((track.artist.clone(), None));
        vec![Command::About(track.clone())]
    }

    pub fn is_followed(&self, artist: &ArtistRef) -> bool {
        self.collection.artists().any(|(_, followed)| followed.id == artist.id)
    }

    /// The artist of what plays as the catalog presents them, once that is known.
    pub fn about_artist(&self) -> Option<&Known> {
        let (asked, found) = self.about.as_ref()?;
        (*asked == self.player.current.as_ref()?.artist).then_some(found.as_ref()?)
    }

    /// Whether the right panel shows what plays, the one place a clip is seen.
    fn shows_playing(&self) -> bool {
        !self.mobile && self.panel_open && !self.queue_open
    }

    /// The song that plays, also while its video's own sound has its place.
    fn song(&self) -> Option<&Track> {
        self.swapped.as_ref().or(self.player.current.as_ref())
    }

    /// The song whose name the right panel shows large, while it does.
    pub fn panel_title(&self) -> Option<&str> {
        self.current().filter(|_| self.shows_playing()).map(|track| track.id.as_str())
    }

    /// Whether the name in the right panel is long and slides along its room.
    pub fn title_moves(&self) -> bool {
        self.motion && !self.settings.reduce_motion
            && self.title_reach.as_ref().is_some_and(|(id, reach, _)| *reach > 1.0 && self.panel_title() == Some(id))
    }

    /// Whether the right panel is shut to the strip that peeks at what plays, as Spotify's,
    /// where its video goes on showing.
    pub fn peeks(&self) -> bool {
        !self.mobile && self.wide && !self.panel_open
    }

    /// Whether what plays is somewhere its video is seen, and the user wants videos.
    fn wants_footage(&self) -> bool {
        self.settings.clips && !self.settings.reduce_motion && (self.cinema || self.shows_playing() || self.peeks())
    }

    /// A song's footage is looked for once, while it would be seen and the user wants it.
    fn ask_clip(&mut self) -> Vec<Command> {
        let wanted = self.wants_footage();
        let Some(track) = self.song().filter(|_| wanted) else { return Vec::new() };
        if self.clip.as_ref().is_some_and(|(asked, _)| *asked == track.id) {
            return Vec::new();
        }
        let track = track.clone();
        self.clip = Some((track.id.clone(), None));
        vec![Command::Clip(track)]
    }

    /// The footage of the song that plays, once it has arrived.
    fn footage(&self) -> Option<&Arc<Footage>> {
        let (asked, footage) = self.clip.as_ref()?;
        (Some(asked) == self.song().map(|song| &song.id)).then_some(footage.as_ref()?)
    }

    /// Whether the whole video can keep time with what sounds: it is the video's own
    /// sound, or the song lasts what the video does.
    fn follows(&self, footage: &Footage) -> bool {
        let video = &footage.video;
        self.player.current.as_ref().is_some_and(|track| track.id == video.id || track.duration_ms.abs_diff(video.duration_ms) <= SAME_LENGTH_MS)
    }

    /// What to show of the song's footage now, named so that another showing is another
    /// name: the whole video where it keeps time, and a stretch of it in the panel where
    /// it does not.
    pub fn screening(&self) -> Option<(String, &Arc<Footage>, Showing)> {
        let footage = self.footage().filter(|_| self.wants_footage())?;
        let showing = match (self.follows(footage), self.cinema) {
            (true, true) => Showing::Large,
            (true, false) => Showing::Small,
            (false, false) => Showing::Loop,
            (false, true) => return None,
        };
        Some((format!("{}:{showing:?}", self.song()?.id), footage, showing))
    }

    /// The sound that goes with what is seen. A video that fills the window and does not
    /// last what its song does plays with its own; out of the window, the song is back.
    fn resound(&mut self) -> Vec<Command> {
        match (&self.swapped, self.cinema) {
            (None, true) => {
                let Some(footage) = self.footage().filter(|footage| self.settings.clips && !self.follows(footage)).cloned() else { return Vec::new() };
                self.swapped = self.player.current.clone();
                vec![Command::Swap(footage.video.clone())]
            }
            (Some(song), false) => vec![Command::Swap(song.clone())],
            _ => Vec::new(),
        }
    }

    /// What the user plays and already has, for the catalog to pick from.
    fn taste(&self) -> Taste {
        let collection = &self.collection;
        let seeds = discover::seeds(&collection.heard, &collection.liked, SEEDS);
        let own = || collection.heard.iter().chain(&collection.liked).chain(&self.saved).chain(collection.playlists.iter().flat_map(|playlist| &playlist.tracks));
        // A track by several is a track of each of them.
        let credited = own().flat_map(|track| std::iter::once(track.artist.as_str()).chain(track.artist.split([',', '&'])));
        let followed = collection.artists().map(|(_, artist)| artist.name.as_str());
        let visited = collection.recent.iter().filter_map(|entry| match entry {
            Entry::Artist(artist) => Some(artist.id.clone()),
            Entry::Album(_) => None,
        });
        let mut artist_ids: Vec<String> = Vec::new();
        for id in seeds.iter().filter_map(|seed| seed.artist_id.clone()).chain(collection.artists().map(|(_, artist)| artist.id.clone())).chain(visited) {
            if !artist_ids.contains(&id) {
                artist_ids.push(id);
            }
        }
        Taste {
            tracks: own().map(|track| track.id.clone()).collect(),
            artists: credited.chain(followed).map(discover::plain).collect(),
            artist_ids,
            albums: collection.albums().map(|(_, album)| album.id.clone()).collect(),
            recent: discover::latest_by_artist(&collection.heard, RECENT_SEEDS),
            seeds,
        }
    }

    /// Asks for the home page's picks, once for each set of artists the user plays most,
    /// and for the covers of what was heard and of what was heard again, the picture of its mix.
    pub fn ask_picks(&mut self) -> Vec<Command> {
        let heard = &self.collection.heard;
        let repeat = discover::on_repeat(heard, REPEAT);
        let mut commands = cover_requests(&mut self.covers, covers_of(&heard[..heard.len().min(SHELF)]).chain(covers_of(&repeat)));
        let taste = self.taste();
        let mut artists: Vec<String> = taste.seeds.iter().map(|seed| discover::plain(&seed.artist)).collect();
        artists.sort();
        let anything = !taste.seeds.is_empty() || !taste.artist_ids.is_empty();
        if anything && self.picked_for.as_ref() != Some(&artists) {
            self.picked_for = Some(artists);
            self.picks_coming = [true; 3];
            commands.push(Command::Discover(taste));
        }
        commands
    }

    /// Whether a cover is on its way: asked for, and neither in nor known to be missing.
    pub fn cover_coming(&self, url: &str) -> bool {
        self.covers.contains(url) && !self.artwork.contains_key(url) && !self.missing_covers.contains(url)
    }

    /// Whether a dialog of the app's own stands over the window, the first run's included:
    /// what offers itself over the window waits for it to close.
    pub fn dialog_open(&self) -> bool {
        self.welcome || self.onboarding.is_some() || self.sharing.is_some() || self.creating.is_some() || self.importing.is_some()
            || self.missed_open || self.confirm.is_some() || self.handoff.is_some() || self.profile_draft.is_some()
    }

    /// What plays and what follows it, to be gone on with the next time the app opens.
    pub fn session(&self) -> Option<Session> {
        let current = self.player.current.clone()?;
        let tracks = std::iter::once(current).chain(self.player.upcoming.iter().take(SESSION_KEPT).cloned()).collect();
        Some(Session { tracks, position: self.player.position })
    }

    /// Whether the recent searches drop down from the desktop's search field.
    pub fn recent_searches_shown(&self) -> bool {
        !self.mobile && self.search_focused && self.query.is_empty() && !self.collection.searches.is_empty()
    }

    /// Asks for the translation of what plays once its lyrics are in, while translating is on.
    pub fn ask_translation(&mut self) -> Vec<Command> {
        let Some(track) = self.current().filter(|_| self.settings.translate_lyrics) else { return Vec::new() };
        let to = crate::i18n::current().tag().to_owned();
        if self.translation.as_ref().is_some_and(|(asked, into, _)| *asked == track.id && *into == to) {
            return Vec::new();
        }
        let lines: Vec<String> = match self.lines() {
            [] => self.plain_lyrics().to_vec(),
            lines => lines.iter().map(|line| line.text.clone()).collect(),
        };
        if lines.is_empty() {
            return Vec::new();
        }
        let track_id = track.id.clone();
        self.translation = Some((track_id.clone(), to.clone(), Translation::Asking));
        vec![Command::Translate { track_id, lines, to }]
    }

    /// The translation of the `index`-th line of what plays, while translating is on; none for
    /// a blank line or one that reads the same.
    pub fn translated(&self, index: usize, line: &str) -> Option<&str> {
        let track = self.current().filter(|_| self.settings.translate_lyrics)?;
        match &self.translation {
            Some((asked, _, Translation::Lines(lines))) if *asked == track.id => lines.get(index).map(String::as_str).filter(|said| !said.is_empty() && *said != line.trim()),
            _ => None,
        }
    }

    /// Whether the lyrics of what plays can be translated: they are in, and in another language
    /// as far as is known.
    pub fn translatable(&self) -> bool {
        let known_same = self.current().is_some_and(|track| matches!(&self.translation, Some((asked, _, Translation::Same)) if *asked == track.id));
        !known_same && (!self.lines().is_empty() || !self.plain_lyrics().is_empty())
    }

    /// The songs the playlist on screen is offered, the ones shown first; none on any other page.
    pub fn suggestions(&self) -> Vec<&Track> {
        let Page::Playlist(id) = self.page else { return Vec::new() };
        let (Some(playlist), Some(suggested)) = (self.collection.playlist(id), self.suggested.as_ref().filter(|suggested| suggested.playlist == id)) else { return Vec::new() };
        suggested.tracks.iter().filter(|track| !playlist.tracks.iter().any(|held| held.id == track.id)).collect()
    }

    /// Asks for what goes with a playlist the first time its page is on screen.
    pub fn ask_suggested(&mut self) -> Vec<Command> {
        let Page::Playlist(id) = self.page else { return Vec::new() };
        if self.suggested.as_ref().is_some_and(|suggested| suggested.playlist == id) || self.collection.playlist(id).is_none() {
            return Vec::new();
        }
        self.suggested = Some(Suggested { playlist: id, ..Suggested::default() });
        self.ask_more_suggested()
    }

    /// Asks for more of what goes with the playlist once few are left to show. Each time
    /// other songs of it lead, its latest first; an empty one goes by what the user plays.
    fn ask_more_suggested(&mut self) -> Vec<Command> {
        let left = self.suggestions().len();
        let Some(suggested) = self.suggested.as_ref().filter(|suggested| !suggested.asking && left < SUGGESTED_LOW) else { return Vec::new() };
        let Some(playlist) = self.collection.playlist(suggested.playlist) else { return Vec::new() };
        let tracks = &playlist.tracks;
        let seeds: Vec<Track> = if tracks.is_empty() {
            discover::seeds(&self.collection.heard, &self.collection.liked, SEEDS)
        } else {
            (0..SEEDS.min(tracks.len())).map(|k| tracks[tracks.len() - 1 - (suggested.round * SEEDS + k) % tracks.len()].clone()).collect()
        };
        if seeds.is_empty() {
            return Vec::new();
        }
        let mut known = suggested.offered.clone();
        known.extend(tracks.iter().map(|track| track.id.clone()));
        let playlist = suggested.playlist;
        let suggested = self.suggested.as_mut().unwrap();
        suggested.round += 1;
        suggested.asking = true;
        vec![Command::Suggest { playlist, seeds, known }]
    }

    fn suggested_covers(&mut self) -> Vec<Command> {
        let urls: Vec<Option<String>> = self.suggestions().into_iter().take(SUGGESTED_SHOWN).map(|track| track.artwork_url.clone()).collect();
        cover_requests(&mut self.covers, &urls)
    }

    /// A recommendation played, added to the playlist, or the ones shown traded for the next.
    fn suggestion_action(&mut self, id: &str) -> Vec<Command> {
        let shown: Vec<Track> = self.suggestions().into_iter().take(SUGGESTED_SHOWN).cloned().collect();
        let picked = |prefix: &str| id.strip_prefix(prefix).and_then(|index| index.parse::<usize>().ok()).and_then(|index| shown.get(index));
        // A row plays the ten shown from it, as Spotify's.
        if let Some(index) = id.strip_prefix("suggested:").and_then(|index| index.parse::<usize>().ok()).filter(|index| *index < shown.len()) {
            if self.current().is_some_and(|current| current.id == shown[index].id) {
                return vec![Command::PlayPause];
            }
            return vec![Command::PlayList { tracks: shown, index }];
        }
        let Some(suggested) = self.suggested.as_mut() else { return Vec::new() };
        if let Some(track) = picked("suggested-add:") {
            // It goes to the end of the list, and the next one takes its place on the shelf.
            self.collection.add(suggested.playlist, track.clone());
        } else if id == "suggested-refresh" {
            suggested.tracks.retain(|track| !shown.iter().any(|gone| gone.id == track.id));
        } else {
            return Vec::new();
        }
        let mut commands = self.ask_more_suggested();
        commands.extend(self.suggested_covers());
        commands
    }

    /// The lyrics are only looked for while they are on screen, once for each track.
    fn ask_lyrics(&mut self) -> Vec<Command> {
        // Their page shows them all, and the panel of what plays a few of them.
        let shown = self.lyrics_open() || self.shows_playing() || (self.mobile && self.page == Page::Playing);
        let Some(track) = self.player.current.as_ref().filter(|_| shown) else { return Vec::new() };
        if self.lyrics.as_ref().is_some_and(|(asked, _)| *asked == track.id) {
            return Vec::new();
        }
        self.lyrics = Some((track.id.clone(), Lyrics::Loading));
        self.lyric_shown = None;
        vec![Command::Lyrics(track.clone())]
    }

    /// Lets the covers seen longest ago go once there are more than `ARTWORK_KEPT`, and
    /// forgets they were asked for, so a page that shows one again asks again. The cover of
    /// what plays stays.
    fn forget_old_covers(&mut self) {
        let playing = self.player.current.as_ref().and_then(|track| track.artwork_url.clone());
        for _ in 0..self.artwork_order.len().saturating_sub(ARTWORK_KEPT) {
            let Some(url) = self.artwork_order.pop_front() else { break };
            if Some(&url) == playing.as_ref() {
                self.artwork_order.push_back(url);
                continue;
            }
            self.artwork.remove(&url);
            self.picture_sizes.remove(&url);
            self.covers.remove(&url);
        }
    }

    /// The lines of what plays, when they are known.
    pub fn lines(&self) -> &[Line] {
        match (&self.lyrics, &self.player.current) {
            (Some((asked, Lyrics::Lines(lines))), Some(track)) if *asked == track.id => lines,
            _ => &[],
        }
    }

    /// The un-timed words for the current track only.
    pub fn plain_lyrics(&self) -> &[String] {
        match (&self.lyrics, &self.player.current) {
            (Some((asked, Lyrics::Plain { lines, .. })), Some(track)) if *asked == track.id => lines,
            _ => &[],
        }
    }

    /// The line being sung.
    pub fn active_line(&self) -> Option<usize> {
        active(self.lines(), (self.player.position.max(0.0) * 1000.0) as u64)
    }

    pub fn lyrics_open(&self) -> bool {
        self.page == Page::Lyrics
    }

    /// The line to bring into view, once each time the song moves on to another.
    pub fn line_to_reveal(&mut self) -> Option<String> {
        let line = self.active_line().filter(|_| self.lyrics_open());
        if line == self.lyric_shown {
            return None;
        }
        self.lyric_shown = line;
        line.map(|index| format!("lyric:{index}"))
    }

    /// Holds the rows of the lyrics where they were before the list jumped `by` to the line
    /// being sung, and lets them spring after it from where, and as fast as, they still moved.
    /// Without motion they are in place at once.
    pub(crate) fn glide_lyrics(&mut self, by: f32, now: Instant) {
        let from = self.lyric_glide.value(now) + by;
        if self.motion {
            self.lyric_glide.release(from, self.lyric_glide.velocity(now), 0.0, now);
        } else {
            self.lyric_glide.snap_to(0.0, now);
        }
    }

    /// Slides the player's lyric card `to` this far up, by spring when it was already on
    /// screen and moving is allowed; `None` when the card is not laid out. Whether it moved.
    pub(crate) fn follow_lyric_card(&mut self, to: Option<f32>, now: Instant) -> bool {
        let Some(to) = to else {
            self.lyric_card_shown = false;
            return false;
        };
        if (self.lyric_card.target() - to).abs() < 0.5 && self.lyric_card_shown {
            return false;
        }
        if self.lyric_card_shown && self.motion {
            self.lyric_card.release(self.lyric_card.value(now), self.lyric_card.velocity(now), to, now);
        } else {
            self.lyric_card.snap_to(to, now);
        }
        self.lyric_card_shown = true;
        true
    }

    pub fn lyric_card_offset(&self, now: Instant) -> f32 {
        self.lyric_card.value(now)
    }

    /// How far below its place the row of line `index` is painted at `now`. A row sets off a
    /// little after the one before it, counting out from the line being sung, so the list
    /// ripples into place instead of moving as one slab.
    pub fn lyric_lag(&self, index: usize, now: Instant) -> f32 {
        let distance = self.active_line().map_or(index, |line| line.abs_diff(index)).min(LYRIC_RIPPLE_ROWS);
        self.lyric_glide.value(now.checked_sub(LYRIC_RIPPLE * distance as u32).unwrap_or(now))
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

    #[test]
    fn typing_searches_live_and_a_result_chosen_is_remembered_as_spotify_does() {
        let mut state = State::default();
        let now = Instant::now();
        state.type_query("cer", now);
        assert!(state.search_tick(now + Duration::from_millis(100)).is_empty());
        assert_eq!(state.search_tick(now + Duration::from_millis(200)), vec![Command::Search("cer".into())]);
        assert!(state.collection.searches.is_empty(), "typing alone is not history");
        state.apply(Event::SearchResults { query: "cer".into(), found: found(&["a", "b"]) });
        // Its songs play as a list's from the chosen one, and that one is remembered.
        assert_eq!(click(&mut state, "page-track:1"), vec![Command::PlayList { tracks: vec![track("a"), track("b")], index: 1 }]);
        assert_eq!(state.collection.searches, vec![Searched::Track { track: track("b") }]);
        // A remembered song plays again from the recent searches, and goes first once more.
        state.collection.searches.insert(0, "otra".into());
        assert_eq!(click(&mut state, "recent-search:1"), vec![Command::PlayList { tracks: vec![track("b")], index: 0 }]);
        assert_eq!(state.collection.searches[0], Searched::Track { track: track("b") });
        // Emptied, the field goes back to where a search starts.
        state.type_query("", now);
        assert!(state.at_home() && state.search_due.is_none());
    }

    #[test]
    fn searches_remembered_before_results_were_still_read() {
        let old: Collection = serde_json::from_str(r#"{"kept":[],"recent":[],"searches":["Cerati"]}"#).unwrap();
        assert_eq!(old.searches, vec!["Cerati"]);
        let mut collection = old.clone();
        collection.searched(Searched::Entry(Entry::Artist(ArtistRef { id: "x".into(), name: "Soda".into(), subscribers: None, artwork_url: None })));
        collection.searched(Searched::Track { track: track("t") });
        let text = serde_json::to_string(&collection).unwrap();
        let again: Collection = serde_json::from_str(&text).unwrap();
        assert_eq!(again, collection);
        // A build from before reads what it knows and does not lose the collection over them.
        assert!(!text.contains("\"searches\""), "the old key is left to the old builds");
    }

    /// The covers among what was asked for.
    fn covers(commands: Vec<Command>) -> Vec<Command> {
        commands.into_iter().filter(|command| matches!(command, Command::FetchArtwork(_))).collect()
    }

    /// A state that looks for no clips, for the tests that count what a new track asks for.
    fn without_clips() -> State {
        State { settings: Settings { clips: false, ..Settings::default() }, ..State::default() }
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
    fn a_tiles_play_button_plays_what_it_shows_without_leaving_the_page() {
        let mut state = State { query: "q".into(), ..State::default() };
        state.search();
        let mut results = found(&["a"]);
        results.albums = vec![AlbumRef { id: "lp".into(), title: "lp".into(), artist: Some("artist".into()), year: String::new(), artwork_url: None }];
        state.apply(Event::SearchResults { query: "q".into(), found: results });
        let page = state.page.clone();
        assert_eq!(click(&mut state, "tile-play:album:0"), vec![Command::Open(Target::Album("lp".into()))]);
        assert_eq!(state.page, page, "the page stays");
        let played = state.apply(Event::Opened { target: Target::Album("lp".into()), result: Ok(Content::Album(album("lp", None))) });
        assert_eq!(played, vec![Command::PlayList { tracks: vec![track("t1"), track("t2")], index: 0 }]);
        assert_eq!(state.page, page, "what arrived plays where the user is");
        assert!(state.apply(Event::Opened { target: Target::Album("lp".into()), result: Ok(Content::Album(album("lp", None))) }).is_empty(), "once");
        assert!(click(&mut state, "tile-play:album:9").is_empty());
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
    fn an_account_from_another_device_joins_this_one_or_takes_its_place() {
        let list = |id: u64, name: &str, tracks: Vec<Track>| Playlist { id, name: name.into(), collaborative: false, picture: None, tracks };
        let mut there = State::default();
        there.profile.name = "Nihuel".into();
        there.collection.playlists = vec![list(1, "Viaje", vec![track("a")]), list(2, "Noche", vec![track("b")])];
        there.collection.liked = vec![track("a"), track("c")];
        there.settings.autoplay = false;
        let bundle = there.bundle(Parts::default());
        assert!(bundle.heard.is_empty(), "what was played stays unless asked for");

        let mut here = State::default();
        here.collection.playlists = vec![list(1, "Mía", vec![track("z")]), list(9, "Viaje", vec![track("a")])];
        here.collection.liked = vec![track("c")];
        let commands = here.take_in(bundle.clone(), Parts::default(), false);
        let names: Vec<&str> = here.collection.playlists.iter().map(|playlist| playlist.name.as_str()).collect();
        assert_eq!(names, ["Noche", "Mía", "Viaje"], "a list already here is not taken twice, and another keeps its order");
        let ids: HashSet<u64> = here.collection.playlists.iter().map(|playlist| playlist.id).collect();
        assert_eq!(ids.len(), 3, "a list that came takes an id of this device's");
        assert_eq!(here.collection.liked.iter().map(|track| track.id.as_str()).collect::<Vec<_>>(), ["c", "a"]);
        assert_eq!(here.profile.name, "Nihuel");
        assert!(!here.settings.autoplay && commands.contains(&Command::Autoplay(false)), "the settings take their place and reach the player");

        let mut replaced = State::default();
        replaced.collection.liked = vec![track("z")];
        replaced.take_in(bundle, Parts { liked: true, ..Parts { profile: false, playlists: false, liked: false, library: false, history: false, settings: false, equalizer: false } }, true);
        assert_eq!(replaced.collection.liked.iter().map(|track| track.id.as_str()).collect::<Vec<_>>(), ["a", "c"]);
        assert!(replaced.profile.name.is_empty() && replaced.collection.playlists.is_empty(), "only what was chosen");
    }

    #[test]
    fn the_handoff_dialog_serves_what_is_chosen_and_reads_what_is_typed() {
        let mut state = State::default();
        assert!(matches!(click(&mut state, "account-handoff").as_slice(), [Command::Serve(bundle)] if bundle.profile.is_some()));
        assert!(matches!(click(&mut state, "handoff-part:profile").as_slice(), [Command::Serve(bundle)] if bundle.profile.is_none()), "another part, another bundle");
        assert_eq!(click(&mut state, "handoff-tab:receive"), vec![Command::StopServing]);
        state.update(&UiAction::InputChanged { id: "handoff-address".into(), value: "10.0.0.2:4000/abc1".into() });
        assert_eq!(click(&mut state, "handoff-fetch"), vec![Command::Fetch("http://10.0.0.2:4000/abc1".into())]);
        state.apply(Event::Fetched(Ok(Bundle { version: 1, liked: vec![track("a")], ..Bundle::default() })));
        let handoff = state.handoff.as_ref().unwrap();
        assert!(handoff.parts.liked && !handoff.parts.playlists && !handoff.fetching, "what came is chosen");
        click(&mut state, "handoff-take");
        assert!(state.handoff.is_none() && state.collection.liked.len() == 1);
        assert_eq!(click(&mut state, "handoff-close"), vec![Command::StopServing]);
    }

    #[test]
    fn a_code_read_by_the_camera_opens_the_handoff_on_what_it_names() {
        let mut state = State::default();
        let commands = state.receive_from("surco://receive?from=http%3A%2F%2F10.0.0.2%3A4000%2Fabc1");
        assert_eq!(commands, vec![Command::Fetch("http://10.0.0.2:4000/abc1".into())]);
        assert!(state.handoff.as_ref().is_some_and(|handoff| handoff.receiving && handoff.fetching));
    }

    #[test]
    fn a_list_is_downloaded_whole_and_let_go_once_all_of_it_is() {
        let mut state = State::default();
        let list = Playlist { id: 1, name: "L".into(), collaborative: false, picture: None, tracks: vec![track("a"), track("b")] };
        state.collection.playlists.push(list);
        state.page = Page::Playlist(1);
        state.saved = vec![track("a")];
        assert_eq!(state.page_downloaded(), Some(false));
        assert_eq!(click(&mut state, "page-download"), vec![Command::Save(track("b"))], "only what is not kept yet");
        state.saved.push(track("b"));
        assert_eq!(state.page_downloaded(), Some(true));
        assert_eq!(click(&mut state, "page-download"), vec![Command::Unsave("a".into()), Command::Unsave("b".into())]);
        state.page = Page::Settings;
        assert_eq!(state.page_downloaded(), None, "only a list can be kept whole");
    }

    #[test]
    fn the_shut_panels_strip_keeps_the_video_where_the_window_has_room_for_it() {
        let mut state = State { panel_open: false, ..State::default() };
        assert!(!state.peeks() && !state.wants_footage(), "a narrow window has no strip");
        state.wide = true;
        assert!(state.peeks() && state.wants_footage());
        state.settings.clips = false;
        assert!(!state.wants_footage(), "nor when videos are off");
    }

    #[test]
    fn a_picture_chosen_is_the_lists_cover_and_the_accounts_face_until_taken_off() {
        let mut state = State::default();
        state.collection.playlists.push(Playlist { id: 3, name: "L".into(), collaborative: false, picture: None, tracks: vec![track("a")] });
        state.page = Page::Playlist(3);
        assert_eq!(click(&mut state, "playlist-picture"), vec![Command::PickPicture(PictureFor::Playlist(3))]);
        let asked = state.apply(Event::Picture { of: PictureFor::Playlist(3), name: "surco:picture:1".into() });
        assert_eq!(asked, vec![Command::FetchArtwork("surco:picture:1".into())], "it is fetched as a cover");
        assert_eq!(state.collection.playlist(3).and_then(Playlist::cover).map(String::as_str), Some("surco:picture:1"));
        click(&mut state, "playlist-picture-remove");
        assert_eq!(state.collection.playlist(3).and_then(Playlist::cover), track("a").artwork_url.as_ref(), "back to its first song's");
        state.apply(Event::Picture { of: PictureFor::Profile, name: "surco:picture:2".into() });
        assert_eq!(state.profile.picture.as_deref(), Some("surco:picture:2"));
        click(&mut state, "profile-picture-remove");
        assert!(state.profile.picture.is_none());
    }

    #[test]
    fn the_profile_opens_from_the_account_and_its_name_is_saved_trimmed() {
        let mut state = State::default();
        click(&mut state, "account-profile");
        assert_eq!(state.page, Page::Profile);
        click(&mut state, "profile-edit");
        state.update(&UiAction::InputChanged { id: "profile-name".into(), value: format!("  Nihuel{}  ", "x".repeat(60)) });
        click(&mut state, "profile-save");
        assert!(state.profile_draft.is_none());
        assert!(state.profile.name.starts_with("Nihuel") && state.profile.name.chars().count() == PROFILE_NAME_MOST);
        click(&mut state, "profile-edit");
        click(&mut state, "profile-cancel");
        assert!(state.profile_draft.is_none() && state.profile.name.starts_with("Nihuel"), "cancelling keeps the name");
    }

    #[test]
    fn back_keeps_the_page_left_for_forward_until_another_is_opened() {
        let mut state = State { page: Page::Liked, ..State::default() };
        click(&mut state, "downloads");
        assert_eq!(state.page, Page::Downloads);
        assert!(!state.can_go_forward());
        click(&mut state, "back");
        assert_eq!(state.page, Page::Liked);
        assert!(state.can_go_forward());
        click(&mut state, "forward");
        assert_eq!(state.page, Page::Downloads);
        assert!(state.can_go_back() && !state.can_go_forward());
        click(&mut state, "back");
        click(&mut state, "settings");
        assert!(!state.can_go_forward(), "a page opened forgets where forward led");
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

        click(&mut state, "pin:liked");
        click(&mut state, "pin:playlist:2");
        click(&mut state, "pin:liked");
        assert_eq!(state.collection.pinned, ["playlist:2"], "pinned twice is let go");
        click(&mut state, "library-sort");
        assert!(state.library_menu);
        click(&mut state, "library-sort:alphabetical");
        assert!(state.settings.library_sort == Sort::Alphabetical && !state.library_menu, "choosing shuts the menu");
        click(&mut state, "library-view:grid");
        assert_eq!(state.settings.library_view, LibraryView::Grid);
        click(&mut state, "library-search");
        assert!(state.library_searching && state.focus.as_deref() == Some("library-query"));
        state.update(&UiAction::InputChanged { id: "library-query".into(), value: "lu".into() });
        assert_eq!(state.library_search, "lu");
        assert!(click(&mut state, "queue-clear").is_empty() && state.confirm == Some(Confirm::ClearQueue), "it asks first");
        assert_eq!(click(&mut state, "confirm-yes"), vec![Command::ClearQueue]);
        let dropped = |id: &str, target: Option<&str>| UiAction::Dropped { id: id.into(), position: limn::Point::new(0.0, 0.0), target: target.map(str::to_owned) };
        assert_eq!(state.update(&dropped("upcoming:2", Some("upcoming:0"))), vec![Command::MoveQueue(2, 0)]);
        assert!(state.update(&dropped("upcoming:2", Some("upcoming:2"))).is_empty() && state.update(&dropped("upcoming:2", None)).is_empty());
        assert_eq!(click(&mut state, "setting:mono"), vec![Command::Mono(true)]);
        assert_eq!(click(&mut state, "setting:normalize"), vec![Command::Normalize(false)]);
        state.show(PlayerView { upcoming: vec![track("a"), track("b")], ..PlayerView::default() });
        assert_eq!(click(&mut state, "do:unqueue:upcoming:1"), vec![Command::Unqueue(1)]);
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
        assert_eq!(state.copy.take().as_deref(), Some("https://music.youtube.com/watch?v=b"));
        assert!(state.copied);
        click(&mut state, "share-close");
        assert!(state.sharing.is_none());
        click(&mut state, "share-copy");
        assert!(state.copy.is_none(), "nothing to copy once closed");
    }

    #[test]
    fn a_pasted_link_opens_the_track_it_shares_and_anything_else_is_searched() {
        let link = "https://music.youtube.com/watch?v=DdI598gKkKw";
        assert_eq!(shared_song(link), Some("DdI598gKkKw"));
        assert_eq!(shared_song("https://www.youtube.com/watch?list=x&v=abc_-1&t=4"), Some("abc_-1"));
        assert_eq!(shared_song("  surco://cancion/abc_-1?x=1 "), Some("abc_-1"));
        assert_eq!(shared_song("https://youtu.be/abc/extra"), Some("abc"));
        for other in ["apocalypse", "https://example.com/watch?v=abc", "https://music.youtube.com/playlist?list=abc", "surco://cancion/", "https://music.youtube.com/watch?v=a%20b"] {
            assert_eq!(shared_song(other), None, "{other}");
        }
        let mut state = State { query: link.to_owned(), ..State::default() };
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
    fn the_row_of_what_plays_pauses_it_and_any_other_starts_its_own() {
        let mut state = State { query: "q".into(), ..State::default() };
        state.search();
        state.apply(Event::SearchResults { query: "q".into(), found: found(&["a", "b"]) });
        state.show(PlayerView { current: Some(track("a")), playing: true, ..PlayerView::default() });
        assert_eq!(click(&mut state, "track:0"), vec![Command::PlayPause]);
        assert_eq!(click(&mut state, "track:1"), vec![Command::PlayList { tracks: vec![track("b")], index: 0 }]);
    }

    #[test]
    fn refreshing_outputs_reports_loading_failure_and_recovery_without_losing_the_selection() {
        let mut state = State { output: Some("headphones".into()), ..State::default() };
        assert_eq!(click(&mut state, "setting:outputs-refresh"), vec![Command::Outputs]);
        assert!(state.outputs_loading);
        assert!(click(&mut state, "setting:outputs-refresh").is_empty(), "one refresh at a time");
        state.apply(Event::OutputsFailed("Audio service unavailable".into()));
        assert!(!state.outputs_loading);
        assert_eq!(state.outputs_error.as_deref(), Some("Audio service unavailable"));
        assert_eq!(state.output.as_deref(), Some("headphones"));
        assert_eq!(click(&mut state, "setting:outputs-refresh"), vec![Command::Outputs]);
        assert!(state.outputs_loading && state.outputs_error.is_none());
        state.apply(Event::Outputs(vec![AudioOutputDevice { id: "headphones".into(), name: "Mis auriculares".into(), is_default: true }]));
        assert!(!state.outputs_loading && state.outputs_error.is_none());
        assert_eq!(state.outputs[0].name, "Mis auriculares");
        assert_eq!(state.output.as_deref(), Some("headphones"));
    }

    #[test]
    fn losing_an_output_explains_the_fallback_and_a_new_choice_clears_the_notice() {
        let mut state = State { output: Some("headphones".into()), ..State::default() };
        state.apply(Event::OutputLost);
        assert!(state.output.is_none() && state.output_notice.is_some());
        assert_eq!(click(&mut state, "setting:output:speakers"), vec![Command::Output(Some("speakers".into()))]);
        assert!(state.output_notice.is_none());
        state.apply(Event::OutputFailed("No audio output".into()));
        assert_eq!(state.output_notice.as_deref(), Some("No audio output"));
        assert_eq!(click(&mut state, "setting:output:speakers"), vec![Command::Output(Some("speakers".into()))], "a failed output can be retried");
    }

    #[test]
    fn settings_toggle_and_the_destructive_ones_wait_for_a_yes() {
        let mut state = State::default();
        click(&mut state, "settings");
        assert_eq!(state.page, Page::Settings);
        assert_eq!(click(&mut state, "setting:autoplay"), vec![Command::Autoplay(false)]);
        assert_eq!(click(&mut state, "setting:sponsorblock:intro"), vec![Command::SponsorBlock(crate::sponsorblock::DEFAULT | 0b1_0000)], "a category added");
        assert_eq!(click(&mut state, "setting:sponsorblock"), vec![Command::SponsorBlock(0)], "off: nothing is asked");
        assert_eq!(click(&mut state, "setting:sponsorblock"), vec![Command::SponsorBlock(crate::sponsorblock::DEFAULT)], "on again, as most want it");
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
    fn a_pages_search_opens_with_the_keyboard_in_it_narrows_its_tracks_and_ends_with_the_page() {
        let mut state = State::default();
        let named = |title: &str, album: Option<&str>| Track { title: title.into(), album: album.map(str::to_owned), ..track("x") };
        assert!(state.page_has(&named("Luna", None)), "nothing is searched yet");
        click(&mut state, "page-search");
        assert!(state.page_searching && state.focus.as_deref() == Some("page-query"));
        state.update(&UiAction::InputChanged { id: "page-query".into(), value: " NADA ".into() });
        assert!(state.page_has(&named("Nada (feat. Enrique Bunbury)", None)) && !state.page_has(&named("Luna", None)));
        assert!(state.page_has(&named("Luna", Some("Nada personal"))), "its album counts");
        assert!(state.page_has(&Track { artist: "Nadadores".into(), ..named("Luna", None) }), "and its artist");

        click(&mut state, "downloads");
        assert!(!state.page_searching && state.page_search.is_empty() && state.page_has(&named("Luna", None)), "another page starts unsearched");
        state.update(&UiAction::InputChanged { id: "page-query".into(), value: "x".into() });
        assert!(state.page_search.is_empty(), "a shut search takes no typing");
        click(&mut state, "page-search");
        click(&mut state, "page-search");
        assert!(!state.page_searching);
    }

    #[test]
    fn footage_is_asked_once_for_what_plays_while_it_would_be_seen() {
        let playing = |id: &str| PlayerView { current: Some(track(id)), playing: true, ..PlayerView::default() };
        let asks = |commands: &[Command]| commands.iter().any(|command| matches!(command, Command::Clip(_)));
        let shown = |state: &State| state.screening().map(|(name, _, showing)| (name, showing));
        let mut state = State::default();
        assert!(asks(&state.show(playing("a"))));
        assert!(!asks(&state.show(playing("a"))), "already asked");
        assert!(state.screening().is_none(), "not fetched yet");
        let footage = footage_of(&track("a"), 0);
        state.apply(Event::Clip { track_id: "other".into(), footage: Some(footage.clone()) });
        assert!(state.screening().is_none(), "an answer for another track changes nothing");
        state.apply(Event::Clip { track_id: "a".into(), footage: Some(footage) });
        assert_eq!(shown(&state), Some(("a:Small".into(), Showing::Small)), "as long as the song, the whole video keeps time with it");

        click(&mut state, "queue");
        assert!(state.screening().is_none() && !asks(&state.show(playing("b"))), "the queue covers what plays");
        // Its button shuts the panel; opened again, it shows what plays and its footage is looked for.
        click(&mut state, "queue");
        assert!(asks(&click(&mut state, "toggle-panel-open")));
        click(&mut state, "setting:clips");
        assert!(!asks(&state.show(playing("c"))) && state.screening().is_none(), "switched off");
        assert!(asks(&click(&mut state, "setting:clips")), "and back on, it is looked for");
    }

    #[test]
    fn the_artist_of_what_plays_is_asked_once_by_name_and_followed_from_their_card() {
        let playing = |id: &str| PlayerView { current: Some(track(id)), playing: true, ..PlayerView::default() };
        let asks = |commands: &[Command]| commands.iter().any(|command| matches!(command, Command::About(_)));
        let mut state = without_clips();
        assert!(asks(&state.show(playing("a"))));
        assert!(!asks(&state.show(playing("b"))), "another track of the same artist");
        assert!(state.about_artist().is_none(), "not known yet");
        let who = ArtistRef { id: "who".into(), name: "artist".into(), subscribers: Some("4.36M".into()), artwork_url: None };
        state.apply(Event::About { artist: "Someone else".into(), found: Some((who.clone(), None)) });
        assert!(state.about_artist().is_none(), "an answer for another artist changes nothing");
        state.apply(Event::About { artist: "artist".into(), found: Some((who.clone(), Some("Una banda.".into()))) });
        assert_eq!(state.about_artist().map(|(artist, _)| &artist.id), Some(&who.id));

        assert!(!state.is_followed(&who));
        click(&mut state, "about-follow");
        assert!(state.is_followed(&who), "kept as the artist's own page keeps them");
        click(&mut state, "about-follow");
        assert!(!state.is_followed(&who));
        assert_eq!(click(&mut state, "about-artist"), vec![Command::Open(Target::Artist("who".into()))]);
    }

    #[test]
    fn a_pages_tracks_take_the_order_of_a_column_and_play_as_they_read() {
        let named = |id: &str, title: &str, seconds: u64| Track { title: title.into(), duration_ms: seconds * 1000, ..track(id) };
        let tracks = vec![named("x", "Luna", 200), named("y", "arroz", 300), named("z", "Mar", 100)];
        let album = Album { id: "lp".into(), title: "LP".into(), artist: "artist".into(), artist_id: None, year: String::new(), artwork_url: None, tracks: tracks.clone() };
        let mut state = State { page: Page::Album(album), ..State::default() };
        let listed = |state: &State| state.page_rows().iter().map(|(place, track)| (*place, track.id.clone())).collect::<Vec<_>>();
        let ids = |ids: [(usize, &str); 3]| ids.map(|(place, id)| (place, id.to_owned())).to_vec();
        assert_eq!(listed(&state), ids([(0, "x"), (1, "y"), (2, "z")]), "its own order");

        click(&mut state, "tracks:sort:title");
        assert_eq!(listed(&state), ids([(1, "y"), (0, "x"), (2, "z")]), "whatever the case of its letters");
        // The row keeps the number of its place in the list; the list plays as it reads.
        let ordered = vec![tracks[1].clone(), tracks[0].clone(), tracks[2].clone()];
        assert_eq!(click(&mut state, "page-track:0"), vec![Command::PlayList { tracks: ordered.clone(), index: 1 }]);
        assert_eq!(click(&mut state, "page-play"), vec![Command::PlayList { tracks: ordered, index: 0 }]);
        click(&mut state, "tracks:sort:title");
        assert_eq!(listed(&state), ids([(2, "z"), (0, "x"), (1, "y")]), "a second press turns it over");
        click(&mut state, "tracks:sort:title");
        assert!(state.page_sort.is_none(), "and a third gives the list its own order back");

        click(&mut state, "page-sort");
        assert!(state.sort_open);
        click(&mut state, "page-sort:duration");
        assert_eq!(listed(&state), ids([(2, "z"), (0, "x"), (1, "y")]));
        assert!(!state.sort_open, "choosing shuts the menu");
        click(&mut state, "page-sort:own");
        assert!(state.page_sort.is_none());
        click(&mut state, "page-sort:title");
        click(&mut state, "downloads");
        assert!(state.page_sort.is_none(), "another page starts in its own order");

        // The album column is as wide as it is dragged, within its limits, and can be put away.
        let at = |x: f32| limn::Point::new(x, 0.0);
        state.update(&UiAction::DragStarted { id: "tracks:resize:album".into(), position: at(500.0) });
        state.update(&UiAction::DragMoved { id: "tracks:resize:album".into(), position: at(460.0), over: None });
        assert_eq!(state.album_width, 220.0, "40 to the left is 40 wider");
        state.update(&UiAction::DragMoved { id: "tracks:resize:album".into(), position: at(2000.0), over: None });
        assert_eq!(state.album_width, ALBUM_WIDTH.0);
        state.update(&UiAction::DragCancelled { id: "tracks:resize:album".into() });
        assert_eq!(state.album_width, 180.0, "a drag that is called off changes nothing");
        click(&mut state, "tracks:column:album");
        assert!(!state.album_shown);
    }

    #[test]
    fn what_the_artists_followed_release_is_news_once_and_the_bell_says_so() {
        let release = |id: &str, year: &str| AlbumRef { id: id.into(), title: id.into(), artist: Some("Who".into()), year: year.into(), artwork_url: None };
        let year = chrono::Datelike::year(&chrono::Local::now()).to_string();
        let mut state = State::default();
        state.collection.toggle(Entry::Artist(ArtistRef { id: "who".into(), name: "Who".into(), subscribers: None, artwork_url: None }));
        assert_eq!(state.ask_news(), vec![Command::News(vec!["who".into()]), Command::CheckUpdate]);
        // What they already had out is not news: of this year's, a couple are listed as seen.
        state.apply(Event::Releases { artist: "who".into(), releases: vec![release("new", &year), release("old", "1999")] });
        assert_eq!(state.collection.news, [(release("new", &year), false)]);
        assert!(!state.has_news());
        state.apply(Event::Releases { artist: "who".into(), releases: vec![release("newer", &year), release("new", &year), release("old", "1999")] });
        assert_eq!(state.collection.news.first(), Some(&(release("newer", &year), true)), "what was not known is news, first");
        assert!(state.has_news());

        click(&mut state, "news");
        assert!(state.page == Page::News && state.news_fresh == 1 && !state.has_news(), "opened, it has been looked at");
        assert_eq!(click(&mut state, "news:0"), vec![Command::Open(Target::Album("newer".into()))]);
        // A later version of the app rings the bell too. One the app cannot take in by
        // itself is fetched from its page.
        let release = Release { version: "9.0.0".into(), page: "https://example/releases/9".into(), program: None, mandatory: false, notes: Vec::new() };
        state.apply(Event::Update(release.clone()));
        assert!(state.has_news() && state.update_toast, "and is said over the window");
        click(&mut state, "update-toast-close");
        state.apply(Event::Update(release.clone()));
        assert!(state.has_news() && !state.update_toast, "found again, it has been said");
        state.apply(Event::Update(Release { version: "9.1.0".into(), ..release.clone() }));
        assert!(state.update_toast, "a later one still is news");
        click(&mut state, "update-toast");
        assert!(state.page == Page::News && !state.update_toast, "its card leads to what is new");
        click(&mut state, "update-get");
        assert_eq!(state.open_url.take().as_deref(), Some("https://example/releases/9"));

        // One that has this system's program is taken in when the user says, and started when they say.
        let own = Release { program: Some(("https://example/program".into(), "https://example/sums".into())), ..release };
        state.apply(Event::Update(own.clone()));
        assert_eq!(click(&mut state, "update-get"), vec![Command::InstallUpdate(own.clone())]);
        assert!(click(&mut state, "update-get").is_empty(), "once");
        state.apply(Event::UpdateProgress(0.5));
        assert_eq!(state.updating, Updating::Fetching(0.5));
        click(&mut state, "update-restart");
        assert!(!state.restart, "there is nothing to start yet");
        state.apply(Event::UpdateInstalled);
        assert!(state.updating == Updating::Ready && !state.restart);
        click(&mut state, "update-restart");
        assert!(state.restart);

        // One that fails can be tried again; one that must be taken starts by itself.
        let mut state = State::default();
        state.apply(Event::Update(Release { mandatory: true, ..own.clone() }));
        click(&mut state, "update-get");
        state.apply(Event::UpdateFailed("sin red".into()));
        assert_eq!(click(&mut state, "update-get").len(), 1);
        state.apply(Event::UpdateInstalled);
        assert!(state.restart);
    }

    /// The footage of a song whose video lasts `longer_ms` more than it does.
    fn footage_of(song: &Track, longer_ms: u64) -> Arc<Footage> {
        let video = Track { id: format!("{}-video", song.id), duration_ms: song.duration_ms + longer_ms, ..song.clone() };
        Arc::new(Footage { clip: surco_video::Clip { parameters: vec![0], samples: Vec::new() }, small: Default::default(), large: Default::default(), video })
    }

    #[test]
    fn a_video_that_fills_the_window_plays_with_its_own_sound_when_it_is_not_as_long_as_its_song() {
        let playing = |track: &Track| PlayerView { current: Some(track.clone()), playing: true, ..PlayerView::default() };
        let shown = |state: &State| state.screening().map(|(_, _, showing)| showing);
        let song = track("a");
        let mut state = State::default();
        state.show(playing(&song));
        let footage = footage_of(&song, 30_000);
        state.apply(Event::Clip { track_id: "a".into(), footage: Some(footage.clone()) });
        assert_eq!(shown(&state), Some(Showing::Loop), "beside the song it only turns over");

        assert_eq!(click(&mut state, "cinema"), vec![Command::Swap(footage.video.clone())]);
        assert_eq!(shown(&state), None, "until its own sound plays");
        state.show(playing(&footage.video));
        assert_eq!(shown(&state), Some(Showing::Large));
        assert_eq!(state.collection.heard, std::slice::from_ref(&song), "the video is the same song, not another one heard");

        assert_eq!(click(&mut state, "cinema"), vec![Command::Swap(song.clone())]);
        state.show(playing(&song));
        assert_eq!(shown(&state), Some(Showing::Loop));
        // Back in the window, the song is swapped again: it was not left swapped.
        assert_eq!(click(&mut state, "cinema"), vec![Command::Swap(footage.video.clone())]);

        // A video as long as its song keeps time with it and changes nothing of the sound.
        let mut state = State::default();
        state.show(playing(&song));
        state.apply(Event::Clip { track_id: "a".into(), footage: Some(footage_of(&song, 0)) });
        assert!(click(&mut state, "cinema").is_empty());
        assert_eq!(shown(&state), Some(Showing::Large));
    }

    #[test]
    fn a_playlist_travels_in_its_link_and_arrives_as_a_copy() {
        let list = Playlist { id: 7, name: "Para el viaje & más".into(), collaborative: false, picture: None, tracks: vec![track("DdI598gKkKw"), track("abc_-1")] };
        let link = list_link(&list);
        assert_eq!(link, "surco://lista/DdI598gKkKw,abc_-1?nombre=Para%20el%20viaje%20%26%20m%C3%A1s");
        assert_eq!(shared_list(&format!("  {link} ")), Some(("Para el viaje & más".into(), vec!["DdI598gKkKw".into(), "abc_-1".into()])));
        for not_one in ["surco://lista/?nombre=x", "surco://lista/a,b", "surco://cancion/abc", "viaje"] {
            assert_eq!(shared_list(not_one), None, "{not_one}");
        }

        let mut state = State { query: link, ..State::default() };
        let target = Target::List { name: "Para el viaje & más".into(), ids: vec!["DdI598gKkKw".into(), "abc_-1".into()] };
        assert_eq!(state.search(), vec![Command::Open(target.clone())]);
        state.apply(Event::Opened { target, result: Ok(Content::List { name: "Para el viaje & más".into(), tracks: vec![track("a"), track("b")], missed: Vec::new() }) });
        let copy = &state.collection.playlists[0];
        assert_eq!((copy.name.as_str(), copy.tracks.len(), state.page.clone()), ("Para el viaje & más", 2, Page::Playlist(copy.id)));

        click(&mut state, "playlist-share");
        assert_eq!(state.copy.as_deref(), Some("surco://lista/a,b?nombre=Para%20el%20viaje%20%26%20m%C3%A1s"));
        assert_eq!(state.list_copied, Some(state.collection.playlists[0].id));
    }

    #[test]
    fn a_list_of_another_app_comes_in_by_its_link_and_goes_out_as_text() {
        let mut state = State::default();
        let link = "https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M";
        click(&mut state, "import-open");
        state.update(&UiAction::InputChanged { id: "import-link".into(), value: "viaje".into() });
        assert!(click(&mut state, "import-go").is_empty() && state.import_unknown && state.importing.is_some(), "not a link of a list");
        state.update(&UiAction::InputChanged { id: "import-link".into(), value: format!(" {link} ") });
        let target = Target::Import(link.into());
        assert_eq!(state.update(&UiAction::Submitted { id: "import-link".into(), value: String::new() }), vec![Command::Open(target.clone())]);
        assert!(state.importing.is_none() && state.settings.import_dismissed, "the offer has done its work");

        state.apply(Event::Opened { target: target.clone(), result: Ok(Content::List { name: "Viaje".into(), tracks: vec![track("a"), track("b")], missed: vec!["Zoé — Soñé".into(), "Zoé — Azul".into()] }) });
        let id = state.collection.playlists[0].id;
        assert_eq!((state.page.clone(), state.imported.as_ref().map(|(list, missed)| (*list, missed.len()))), (Page::Playlist(id), Some((id, 2))));
        click(&mut state, "import-missed");
        click(&mut state, "import-missed-copy");
        assert!(state.missed_open && state.copy.take().as_deref() == Some("Zoé — Soñé\nZoé — Azul"));
        click(&mut state, "playlist-export");
        assert_eq!((state.copy.as_deref(), state.list_exported), (Some("artist - a\nartist - b"), Some(id)));

        // Pasted where songs are searched for, it is brought in all the same; and it is
        // kept even when its user has walked off meanwhile.
        state.query = link.into();
        assert_eq!(state.search(), vec![Command::Open(target.clone())]);
        click(&mut state, "settings");
        state.apply(Event::Opened { target, result: Ok(Content::List { name: "Otra".into(), tracks: vec![track("c")], missed: Vec::new() }) });
        assert_eq!((state.collection.playlists.len(), state.page.clone(), state.imported), (2, Page::Settings, None));
    }

    #[test]
    fn a_liked_track_is_kept_first_and_the_same_heart_takes_it_off() {
        let mut state = State { query: "q".into(), ..State::default() };
        state.search();
        state.apply(Event::SearchResults { query: "q".into(), found: found(&["a", "b"]) });
        click(&mut state, "do:like:track:0");
        click(&mut state, "do:like:track:1");
        assert_eq!(state.collection.liked.iter().map(|track| track.id.as_str()).collect::<Vec<_>>(), ["b", "a"]);
        state.show(PlayerView { current: Some(track("a")), playing: true, ..PlayerView::default() });
        click(&mut state, "like");
        assert!(!state.is_liked("a") && state.is_liked("b"));
        click(&mut state, "liked");
        assert_eq!(state.page, Page::Liked);
        assert_eq!(click(&mut state, "page-track:0"), vec![Command::PlayList { tracks: vec![track("b")], index: 0 }]);
    }

    #[test]
    fn a_playlist_is_offered_ten_songs_at_a_time_to_add_or_trade_for_others() {
        let mut state = State::default();
        state.collection.playlists.push(Playlist { id: 1, name: "Lista".into(), collaborative: false, picture: None, tracks: vec![track("a"), track("b")] });
        assert!(state.ask_suggested().is_empty(), "only on the playlist's page");
        state.page = Page::Playlist(1);
        let Some(Command::Suggest { playlist, seeds, known }) = state.ask_suggested().pop() else { panic!("they are asked for") };
        assert_eq!((playlist, seeds.iter().map(|seed| seed.id.as_str()).collect::<Vec<_>>()), (1, vec!["b", "a"]), "its latest songs lead");
        assert!(known.contains("a") && known.contains("b"));
        assert!(state.ask_suggested().is_empty(), "asked once");
        state.apply(Event::Suggested { playlist: 7, tracks: vec![track("x")] });
        assert!(state.suggestions().is_empty(), "an answer for another list is not this one's");
        state.apply(Event::Suggested { playlist: 1, tracks: (0..25).map(|index| track(&format!("s{index}"))).collect() });
        assert_eq!(state.suggestions().len(), 25);

        assert!(click(&mut state, "suggested-add:0").iter().all(|command| !matches!(command, Command::Suggest { .. })), "enough are left");
        assert_eq!(state.collection.playlist(1).unwrap().tracks.last().unwrap().id, "s0", "added at the end");
        assert_eq!(state.suggestions()[0].id, "s1", "and the next takes its place");

        let asked = click(&mut state, "suggested-refresh");
        assert_eq!(state.suggestions()[0].id, "s11", "the ten shown give way to the next ten");
        let Some(Command::Suggest { seeds, known, .. }) = asked.into_iter().find(|command| matches!(command, Command::Suggest { .. })) else { panic!("few are left: more are asked for") };
        assert!(known.contains("s1") && known.contains("s24") && known.contains("s0"), "nothing offered is offered again");
        assert_ne!(seeds[0].id, "s0", "other songs of the list lead");
        let ten: Vec<Track> = (11..21).map(|index| track(&format!("s{index}"))).collect();
        assert_eq!(click(&mut state, "suggested:1"), vec![Command::PlayList { tracks: ten, index: 1 }], "a row plays the ten shown from it");

        state.apply(Event::Suggested { playlist: 1, tracks: vec![track("s5"), track("n")] });
        assert_eq!(state.suggestions().last().unwrap().id, "n", "what was offered before is left out");
        state.page = Page::Library;
        assert!(state.suggestions().is_empty());
    }

    #[test]
    fn what_was_heard_is_offered_again_and_seeds_what_the_catalog_suggests() {
        let playing = |id: &str| PlayerView { current: Some(track(id)), playing: true, ..PlayerView::default() };
        let mut state = State::default();
        state.show(playing("a"));
        state.show(playing("a"));
        state.show(playing("b"));
        assert_eq!(state.collection.heard.iter().map(|track| track.id.as_str()).collect::<Vec<_>>(), ["b", "a"], "newest first, each once");

        let asked = click(&mut state, "home");
        let Some(Command::Discover(taste)) = asked.last() else { panic!("the picks are asked for: {asked:?}") };
        assert_eq!(taste.seeds, [track("b")], "one seed: both are by the same artist");
        assert!(taste.tracks.contains("a") && taste.artists.contains("artist"));
        assert!(click(&mut state, "home").iter().all(|command| !matches!(command, Command::Discover(_))), "asked once");

        state.apply(Event::Discovered(Picks::Tracks { mix: vec![track("m")], because: vec![(track("b"), vec![track("x"), track("y")])] }));
        // The picks come as lists: the time of day's, then one for each seed, opening with it.
        assert_eq!(state.mixes().iter().map(Mix::key).collect::<Vec<_>>(), ["moment", "artist-b"]);
        let seeded = vec![track("b"), track("x"), track("y")];
        assert_eq!(click(&mut state, "tile-play:home-made:1"), vec![Command::PlayList { tracks: seeded.clone(), index: 0 }]);
        assert!(click(&mut state, "made:1").is_empty() && matches!(&state.page, Page::Mix(mix) if mix.key() == "artist-b"), "a card opens its list");
        assert_eq!(click(&mut state, "page-track:2"), vec![Command::PlayList { tracks: seeded, index: 2 }]);
        assert!(click(&mut state, "made:9").is_empty() && click(&mut state, "tile-play:made:9").is_empty());
        let artist = ArtistRef { id: "who".into(), name: "Who".into(), subscribers: None, artwork_url: None };
        let album = AlbumRef { id: "lp".into(), title: "LP".into(), artist: Some("Who".into()), year: String::new(), artwork_url: None };
        state.apply(Event::Discovered(Picks::Artists(vec![artist])));
        state.apply(Event::Discovered(Picks::Albums(vec![album])));
        assert_eq!(click(&mut state, "pick-album:0"), vec![Command::Open(Target::Album("lp".into()))]);
        assert_eq!(click(&mut state, "pick-artist:0"), vec![Command::Open(Target::Artist("who".into()))]);
        // Today's picks play as a list; the fans' shelves open their albums and, from the face, the artist.
        state.apply(Event::Discovered(Picks::Recent(vec![track("t"), track("u")])));
        assert_eq!(click(&mut state, "today:1"), vec![Command::PlayList { tracks: vec![track("t"), track("u")], index: 1 }]);
        let fan = ArtistRef { id: "fan".into(), name: "Fan".into(), subscribers: None, artwork_url: None };
        let theirs = AlbumRef { id: "theirs".into(), title: "Theirs".into(), artist: Some("Like".into()), year: String::new(), artwork_url: None };
        state.apply(Event::Discovered(Picks::FansOf(vec![(fan.clone(), vec![theirs.clone()])])));
        assert_eq!(click(&mut state, "fans-0-album:0"), vec![Command::Open(Target::Album("theirs".into()))]);
        assert_eq!(click(&mut state, "fans-0"), vec![Command::Open(Target::Artist("fan".into()))]);
        assert!(click(&mut state, "fans-1").is_empty() && click(&mut state, "fans-0-album:3").is_empty() && click(&mut state, "fans-x").is_empty());
        state.apply(Event::Discovered(Picks::MoreOf(fan, vec![theirs])));
        assert_eq!(click(&mut state, "more-album:0"), vec![Command::Open(Target::Album("theirs".into()))]);
        assert_eq!(click(&mut state, "more-artist"), vec![Command::Open(Target::Artist("fan".into()))]);
        assert_eq!(click(&mut state, "heard:1"), vec![Command::PlayList { tracks: vec![track("b"), track("a")], index: 1 }]);
    }

    #[test]
    fn the_mobile_player_requests_its_own_lyrics_and_artist_once() {
        let mut state = State { mobile: true, panel_open: false,
            ..without_clips() };
        let playing = PlayerView { current: Some(track("a")), playing: false,
            duration: 200.0, ..PlayerView::default() };
        let commands = state.show(playing.clone());
        assert!(!commands.iter().any(|command| matches!(command, Command::Lyrics(_) | Command::About(_))));
        let commands = click(&mut state, "mobile-playing");
        assert!(commands.contains(&Command::Lyrics(track("a"))));
        assert!(commands.iter().any(|command| matches!(command, Command::About(_))));
        let commands = state.show(PlayerView { position: 1.0, ..playing });
        assert!(!commands.iter().any(|command| matches!(command, Command::Lyrics(_) | Command::About(_))));
    }

    #[test]
    fn a_cover_pulses_only_while_it_is_on_its_way() {
        let mut state = State::default();
        let url = "https://img/a".to_owned();
        assert!(!state.cover_coming(&url), "not asked for, it is not coming");
        cover_requests(&mut state.covers, std::iter::once(&Some(url.clone())));
        assert!(state.cover_coming(&url));
        state.apply(Event::ArtworkMissing(url.clone()));
        assert!(!state.cover_coming(&url), "one that could not be had leaves its placeholder");
    }

    #[test]
    fn lyrics_are_translated_a_line_each_once_for_what_plays_while_it_is_on() {
        let mut state = State::default();
        state.show(PlayerView { current: Some(track("a")), playing: true, ..PlayerView::default() });
        state.lyrics = Some(("a".into(), Lyrics::Lines(vec![Line { at_ms: 0, text: "I still see you".into() }, Line { at_ms: 900, text: String::new() }, Line { at_ms: 1800, text: "OK".into() }])));
        assert!(state.ask_translation().is_empty(), "off until it is asked for");
        assert!(state.translatable());
        let asked = click(&mut state, "lyrics-translate");
        assert_eq!(asked, vec![Command::Translate { track_id: "a".into(), lines: vec!["I still see you".into(), String::new(), "OK".into()], to: "es".into() }]);
        assert!(state.ask_translation().is_empty(), "asked once");
        state.apply(Event::Translated { track_id: "b".into(), to: "es".into(), lines: Ok(Some(vec!["otra".into()])) });
        assert_eq!(state.translated(0, "I still see you"), None, "an answer for another song is not this one's");
        state.apply(Event::Translated { track_id: "a".into(), to: "es".into(), lines: Ok(Some(vec!["Todavía te veo".into(), String::new(), "OK".into()])) });
        assert_eq!(state.translated(0, "I still see you"), Some("Todavía te veo"));
        assert_eq!((state.translated(1, ""), state.translated(2, "OK")), (None, None), "a blank line, and one that reads the same, have none");
        click(&mut state, "lyrics-translate");
        assert_eq!(state.translated(0, "I still see you"), None, "turned off, the lines stand alone");
        assert!(click(&mut state, "lyrics-translate").is_empty(), "turned on again, what was had is kept");
        state.translation = None;
        state.ask_translation();
        state.apply(Event::Translated { track_id: "a".into(), to: "es".into(), lines: Ok(None) });
        assert!(!state.translatable(), "lyrics already in the app's language have nothing to translate");
    }

    #[test]
    fn lyrics_are_asked_once_for_what_plays_and_a_line_seeks_to_itself() {
        let playing = |id: &str, position| PlayerView { current: Some(track(id)), position, duration: 200.0, playing: true, ..PlayerView::default() };
        // Neither their page nor the panel that shows a few of them is on screen.
        let mut state = State { panel_open: false, ..State::default() };
        assert!(state.show(playing("a", 0.0)).iter().all(|command| !matches!(command, Command::Lyrics(_))), "not while they are not on screen");
        assert_eq!(click(&mut state, "lyrics"), vec![Command::Lyrics(track("a"))]);
        assert!(state.lyrics_open(), "they take the page");
        assert!(state.show(playing("a", 1.0)).is_empty(), "already asked");

        let lines = vec![Line { at_ms: 5_000, text: "one".into() }, Line { at_ms: 9_000, text: "two".into() }];
        state.apply(Event::Lyrics { track_id: "other".into(), lines: None });
        assert_eq!(state.lyrics, Some(("a".into(), Lyrics::Loading)), "an answer for another track changes nothing");
        state.apply(Event::Lyrics { track_id: "a".into(), lines: Some(lines) });
        assert_eq!((state.active_line(), state.line_to_reveal()), (None, None));
        state.show(playing("a", 9.5));
        assert_eq!(state.line_to_reveal().as_deref(), Some("lyric:1"));
        assert_eq!(state.line_to_reveal(), None, "once for each line");
        assert_eq!(click(&mut state, "lyric:0"), vec![Command::Seek(5.0)]);

        assert!(state.show(playing("b", 0.0)).contains(&Command::Lyrics(track("b"))), "the next track brings its own");
        state.apply(Event::Lyrics { track_id: "b".into(), lines: None });
        assert_eq!(state.lyrics, Some(("b".into(), Lyrics::Missing)));
        assert!(state.lines().is_empty());

        // Their button leaves them for the page they were opened over, and so does any other page.
        click(&mut state, "lyrics");
        assert_eq!(state.page, Page::Search);
        click(&mut state, "lyrics");
        click(&mut state, "downloads");
        assert_eq!(state.page, Page::Downloads);
        click(&mut state, "back");
        assert!(state.lyrics_open(), "and the way back passes through them");
    }

    #[test]
    fn a_failed_lyrics_lookup_can_be_requested_again_without_duplicate_inflight_work() {
        let mut state = State { mobile: true, panel_open: false, ..without_clips() };
        state.show(PlayerView { current: Some(track("a")), ..PlayerView::default() });
        assert!(click(&mut state, "mobile-playing").contains(&Command::Lyrics(track("a"))));
        assert!(click(&mut state, "retry-lyrics").is_empty(), "an in-flight lookup is already working");
        state.apply(Event::Lyrics { track_id: "a".into(), lines: None });
        assert_eq!(click(&mut state, "retry-lyrics"), vec![Command::Lyrics(track("a"))],
            "a failed lookup must not poison this song for the rest of its playback");
        assert_eq!(state.lyrics, Some(("a".into(), Lyrics::Loading)));
    }

    #[test]
    fn lyric_provider_failures_honor_cooldown_and_plain_results_never_seek() {
        let mut state = State { mobile: true, panel_open: false, ..without_clips() };
        state.show(PlayerView { current: Some(track("a")), position: 30.0, ..PlayerView::default() });
        click(&mut state, "lyrics");
        state.apply(Event::LyricsFailed { track_id: "old".into(), message: "rate limited".into(), retry_after: Duration::from_secs(60) });
        assert_eq!(state.lyrics, Some(("a".into(), Lyrics::Loading)), "unrelated errors are ignored");
        state.apply(Event::LyricsFailed { track_id: "a".into(), message: "rate limited".into(), retry_after: Duration::from_secs(60) });
        assert!(matches!(&state.lyrics, Some((_, Lyrics::Failed { message, .. })) if message == "rate limited"));
        assert!(click(&mut state, "retry-lyrics").is_empty(), "respect the provider's Retry-After");
        if let Some((_, Lyrics::Failed { retry_at, .. })) = &mut state.lyrics {
            *retry_at = Instant::now() - Duration::from_secs(1);
        }
        assert_eq!(click(&mut state, "retry-lyrics"), vec![Command::Lyrics(track("a"))]);
        state.apply(Event::LyricsPlain { track_id: "old".into(), lines: vec!["stale".into()], source: "Test".into() });
        assert!(state.plain_lyrics().is_empty());
        state.apply(Event::LyricsPlain { track_id: "a".into(), lines: vec!["First line".into(), "Second line".into()], source: "Test".into() });
        assert_eq!(state.plain_lyrics(), ["First line", "Second line"]);
        assert!(state.lines().is_empty() && state.active_line().is_none() && state.line_to_reveal().is_none());
        assert!(click(&mut state, "lyric:0").is_empty(), "plain words have no seek time");
    }

    #[test]
    fn late_lyrics_failures_cannot_erase_words_already_loaded_for_the_returned_track() {
        let mut state = State { mobile: true, panel_open: false, ..without_clips() };
        state.show(PlayerView { current: Some(track("a")), ..PlayerView::default() });
        click(&mut state, "lyrics");
        let lines = vec![Line { at_ms: 1000, text: "Available words".into() }];
        state.apply(Event::Lyrics { track_id: "a".into(), lines: Some(lines.clone()) });
        // A→B→A can leave an older request for A in flight. Its failure is not newer knowledge.
        state.apply(Event::LyricsFailed { track_id: "a".into(), message: "late timeout".into(), retry_after: Duration::ZERO });
        state.apply(Event::Lyrics { track_id: "a".into(), lines: None });
        assert_eq!(state.lines(), lines);
        state.apply(Event::LyricsPlain { track_id: "a".into(), lines: vec!["untimed".into()], source: "Test".into() });
        assert_eq!(state.lines(), lines, "a late plain result cannot downgrade synchronized words");
    }

    #[test]
    fn a_page_steps_the_shuffle_and_plays_from_its_top_until_shuffled() {
        let mut state = State { page: Page::Album(album("lp", None)), ..State::default() };
        assert_eq!(click(&mut state, "page-shuffle"), vec![Command::Shuffle]);
        assert!(matches!(click(&mut state, "page-play").as_slice(), [Command::PlayList { tracks, index: 0 }] if tracks.len() == 2));
        state.show(PlayerView { shuffle: true, ..PlayerView::default() });
        assert!(matches!(click(&mut state, "page-play").as_slice(), [Command::PlayList { index, .. }] if *index < 2));
    }

    #[test]
    fn a_cover_is_asked_for_large_where_its_address_names_a_size() {
        assert_eq!(large_cover("https://lh3.example/abc=w120-h120-l90-rj"), "https://lh3.example/abc=w1200-h1200-l90-rj");
        assert_eq!(cover_at("https://lh3.example/abc=w60-h60", 544), "https://lh3.example/abc=w544-h544");
        assert_eq!(cover_at("https://yt3.example/abc=s88-c-k-no-rj", 544), "https://yt3.example/abc=s544-c-k-no-rj");
        assert_eq!(cover_at("https://lh3.example/abc=w540-h225-p", 1080), "https://lh3.example/abc=w1080-h450-p", "a banner keeps its shape");
        assert_eq!(cover_at("https://lh3.example/abc=w2880-h1200-p", 544), "https://lh3.example/abc=w2880-h1200-p", "never smaller");
        let others = ["https://i.ytimg.com/vi/x/hqdefault.jpg", "https://i.ytimg.com/vi/x/sddefault.jpg?sqp=a&rs=w12-h3", "https://img/a", "https://x/=w-h1", "https://x/=w12", "https://x/=sz", "https://x/a=w9-h9/b", ""];
        for other in others {
            assert_eq!(cover_at(other, 544), other);
        }
        let mut state = without_clips();
        let mut track = track("a");
        track.artwork_url = Some("https://lh3.example/a=w120-h120-rj".into());
        let asked = covers(state.show(PlayerView { current: Some(track), ..PlayerView::default() }));
        assert_eq!(asked.len(), 2, "the row's size and the panel's: {asked:?}");
    }

    #[test]
    fn a_new_track_asks_for_its_cover_once() {
        let mut state = without_clips();
        assert_eq!(covers(state.show(playing("a"))), vec![Command::FetchArtwork("https://img/a".into())]);
        assert!(state.show(playing("a")).is_empty(), "the same track again is only a position");
        assert_eq!(state.current(), Some(&track("a")));
    }

    #[test]
    fn the_queue_fetches_its_covers_only_while_it_is_open() {
        let mut state = without_clips();
        let view = PlayerView { upcoming: vec![track("b"), track("c")], ..playing("a") };
        // The panel of what plays shows what comes next, and no further.
        assert_eq!(covers(state.show(view.clone())), vec![Command::FetchArtwork("https://img/a".into()), Command::FetchArtwork("https://img/b".into())]);
        let opened = click(&mut state, "queue");
        assert_eq!(opened, vec![Command::FetchArtwork("https://img/c".into())]);
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
        // A phone opens it as a page, and back leaves it.
        let mut phone = State { mobile: true, ..State::default() };
        click(&mut phone, "equalizer");
        assert_eq!((phone.page.clone(), phone.equalizer_open), (Page::Equalizer, false));
        click(&mut phone, "back");
        assert_ne!(phone.page, Page::Equalizer);
    }

    #[test]
    fn a_preset_sets_every_band_and_moving_one_makes_it_custom() {
        let mut state = State::default();
        state.equalizer.preamp_db = -2.0;
        assert_eq!(equalizer_preset(state.equalizer), Some(("flat", "Plano")));
        let commands = click(&mut state, "equalizer-preset:bass");
        assert_eq!(state.equalizer.bands_db, EQUALIZER_PRESETS[1].2);
        assert_eq!(state.equalizer.preamp_db, -2.0, "the preamp stays");
        assert_eq!(commands, vec![Command::Equalizer(state.equalizer)]);
        assert_eq!(equalizer_preset(state.equalizer).map(|preset| preset.1), Some("Más graves"));
        state.update(&UiAction::SliderChanged { id: "equalizer:9".into(), fraction: 1.0, committed: true });
        assert_eq!((equalizer_preset(state.equalizer), state.equalizer_band), (None, Some(9)));
        assert!(click(&mut state, "equalizer-preset:nope").is_empty());
        // Each preset is a different curve, in whole decibels within the range.
        for (index, (_, _, bands)) in EQUALIZER_PRESETS.iter().enumerate() {
            assert!(bands.iter().all(|gain| gain.fract() == 0.0 && gain.abs() <= EQUALIZER_DB));
            assert!(EQUALIZER_PRESETS[..index].iter().all(|other| other.2 != *bands));
        }
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

    #[test]
    fn a_long_session_lets_its_oldest_covers_go_but_keeps_what_plays() {
        let mut state = State::default();
        let cover = |index: usize| format!("https://covers/{index}");
        state.show(PlayerView { current: Some(Track { artwork_url: Some(cover(0)), ..track("a") }), ..PlayerView::default() });
        for index in 0..ARTWORK_KEPT + 2 {
            state.covers.insert(cover(index));
            let image = RasterImage::new(format!("{index:064x}"), limn::RasterImageFormat::Png, 1, "AA==".into(), "Portada").unwrap();
            state.apply(Event::Artwork { url: cover(index), image, tint: None, size: None });
        }
        assert_eq!(state.artwork.len(), ARTWORK_KEPT);
        assert!(state.artwork.contains_key(&cover(0)), "the cover of what plays stays");
        assert!(!state.artwork.contains_key(&cover(1)) && !state.artwork.contains_key(&cover(2)));
        assert!(!state.covers.contains(&cover(1)), "and may be asked for again");
        assert!(state.artwork.contains_key(&cover(ARTWORK_KEPT + 1)));
    }

    #[test]
    fn lyric_card_appears_in_place_then_springs_to_each_new_line() {
        let mut state = State { motion: true, ..State::default() };
        let now = Instant::now();
        assert!(state.follow_lyric_card(Some(48.0), now), "a card that appears is placed");
        assert_eq!(state.lyric_card_offset(now), 48.0);
        assert!(!state.follow_lyric_card(Some(48.0), now), "nothing to do while the line holds");
        assert!(state.follow_lyric_card(Some(96.0), now));
        assert_eq!(state.lyric_card_offset(now), 48.0, "the next line slides in rather than jumps");
        assert!(state.is_sliding(now + Duration::from_millis(100)));
        let later = now + Duration::from_secs(2);
        assert!((state.lyric_card_offset(later) - 96.0).abs() < 0.01 && !state.is_sliding(later));
        assert!(!state.follow_lyric_card(None, later));
        assert!(state.follow_lyric_card(Some(0.0), later));
        assert_eq!(state.lyric_card_offset(later), 0.0, "reopened, it is in place at once");
    }

    #[test]
    fn lyric_rows_ripple_out_from_the_line_being_sung_and_settle_in_place() {
        let mut state = State::default();
        state.show(PlayerView { current: Some(track("a")), position: 10.5, ..PlayerView::default() });
        state.apply(Event::Lyrics { track_id: "a".into(), lines: Some((0..20)
            .map(|index| Line { at_ms: index * 1_000, text: format!("Verse {index}") }).collect()) });
        assert_eq!(state.active_line(), Some(10));
        state.motion = true;
        let now = Instant::now();
        state.glide_lyrics(80.0, now);
        assert_eq!(state.lyric_lag(10, now), 80.0, "every row starts where it was painted");
        let soon = now + Duration::from_millis(200);
        let (sung, near, far) = (state.lyric_lag(10, soon), state.lyric_lag(12, soon), state.lyric_lag(15, soon));
        assert!(sung < near && near < far && far < 80.0, "the sung line leads: {sung} {near} {far}");
        assert_eq!(state.lyric_lag(8, soon), near, "the ripple spreads both ways");
        assert_eq!(state.lyric_lag(19, soon), state.lyric_lag(16, soon), "and stops spreading a few rows out");
        // A second jump keeps the way still to go and the speed it has.
        state.glide_lyrics(80.0, soon);
        assert!((state.lyric_lag(10, soon) - (sung + 80.0)).abs() < 0.01);
        let later = soon + Duration::from_secs(2);
        assert!(!state.is_sliding(later));
        assert!((0..20).all(|index| state.lyric_lag(index, later) == 0.0));
        state.motion = false;
        state.glide_lyrics(80.0, later);
        assert_eq!(state.lyric_lag(10, later), 0.0, "without motion the rows are in place at once");
    }

    #[test]
    fn labels_named_in_constants_are_in_every_catalog() {
        let labels = EQUALIZER_PRESETS.iter().map(|preset| preset.1).chain(Quality::ALL.iter().map(|quality| quality.2));
        for label in labels {
            for language in Language::ALL {
                assert_ne!(crate::i18n::text(language, label.0, None), label.0, "{} lacks {}", language.tag(), label.0);
            }
        }
        crate::i18n::set(Language::English);
        assert_eq!((greeting(7), String::from(Quality::ALL[0].2)), ("Good morning", "Low".to_owned()));
        crate::i18n::set(Language::Spanish);
    }
}
