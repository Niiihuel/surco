//! The contract between the window and everything that takes time: the catalog, the network,
//! the player and the audio engine live on the other side of these two channels.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine as _;
use surco_audio::equalizer::{self, EqualizerValues};
use surco_audio::{AudioOutputDevice, Load, MediaBuffer, NativeAudio};
use surco_core::discover;
use surco_core::lyrics::Line;

mod lyrics;
use surco_video::{Clip, Film};
use futures_util::future::join_all;
use surco_core::{Album, AlbumRef, Artist, ArtistRef, Found, Track};
use surco_youtube::{Audio, YouTube};
use eframe::egui;
use limn::{RasterImage, RasterImageFormat};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::i18n::t;
use crate::handoff::{self, Bundle, Offer};
use crate::library::Library;
use crate::state::{cover_at, COVER_SIDE};
use crate::update;
use crate::media_keys::{MediaEvent, MediaKeys};
#[cfg(target_os = "ios")]
use crate::media_keys::Key;
use crate::player::{Effect, Player, PlayerView};

/// A page of the catalog.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Artist(String),
    /// A track that names its artist without linking to it.
    ArtistNamed(String),
    Album(String),
    /// A track someone shared, by its id.
    Song(String),
    /// A playlist someone shared, as its link carries it: its name and its tracks' ids.
    List { name: String, ids: Vec<String> },
    /// A list of another app's, by its link there.
    Import(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Content {
    Artist(Artist),
    Album(Album),
    Song(Track),
    /// A list brought in, and the songs of it the catalog does not have, each as it is
    /// to be read.
    List { name: String, tracks: Vec<Track>, missed: Vec<String> },
}

/// What the window asks for.
/// What a picture of the user's own is for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PictureFor {
    Playlist(u64),
    Profile,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Search(String),
    Open(Target),
    FetchArtwork(String),
    /// Ask the user for a picture of their own, from their files, for a list or the account.
    PickPicture(PictureFor),
    /// Serve the account, or this part of it, to another device on the local network; one
    /// served already is replaced at the same address.
    Serve(Bundle),
    StopServing,
    /// Open the phone's camera on the code of an account on another device.
    ScanCode,
    /// Read an account served by another device, at this address.
    Fetch(String),
    /// Look for artists for the first run's picker: what these searches find, a turn each.
    /// `key` comes back with them.
    FindArtists { key: String, queries: Vec<String> },
    /// Replace the queue with these tracks and play the one at `index`.
    PlayList { tracks: Vec<Track>, index: usize },
    /// What played when the app was last closed: the list from its current track on, held
    /// paused so many seconds in until it is played. Nothing is fetched before then.
    Session { tracks: Vec<Track>, position: f64 },
    PlayPause,
    Next,
    /// Play the n-th of the upcoming tracks, skipping the ones before it.
    Jump(usize),
    /// Queue a track by hand: right after the current one, or after the ones already queued.
    PlayNext(Track),
    Enqueue(Track),
    /// Take the n-th of the upcoming tracks out of what follows.
    Unqueue(usize),
    /// A mobile sheet may outlive a queue update; validate at the backend boundary too.
    UnqueueIfUnchanged { index: usize, snapshot: Vec<Track> },
    /// Drop the tracks queued by hand.
    ClearQueue,
    /// Move the n-th of the upcoming tracks to where the m-th is.
    MoveQueue(usize, usize),
    /// Pass by the silence a song opens and closes with.
    SkipSilence(bool),
    /// Listen through the microphone for the song that plays nearby, until it is named.
    Recognize,
    StopRecognizing,
    /// Sign in to Last.fm or ListenBrainz.
    ScrobbleSignIn(crate::scrobble::SignIn),
    /// Say on the listener's profiles what is playing.
    NowPlaying { accounts: crate::scrobble::Accounts, listen: crate::scrobble::Listen },
    /// Send what waits for each service signed in to.
    Scrobble(crate::scrobble::Accounts),
    /// Which SponsorBlock categories are jumped over, a bit each; none, nothing is asked.
    SponsorBlock(u8),
    /// Stop the music after so long, or with the song that plays, or not.
    Sleep(Option<crate::player::SleepAfter>),
    /// Play the same in both speakers.
    Mono(bool),
    /// Turn the tracks louder than the reference down to it.
    Normalize(bool),
    Previous,
    /// Seconds from the start of the current track.
    Seek(f64),
    /// 0 to 1.
    Volume(f32),
    /// Turns shuffle on or off.
    Shuffle,
    /// Steps repeat: off, the list, the track.
    Repeat,
    Equalizer(EqualizerValues),
    /// Whether a queue that runs out goes on with its last track's radio.
    Autoplay(bool),
    /// Seconds a track that ends shares with the next; 0 for none.
    Crossfade(f64),
    /// The most bits per second a stream may have, from the next track on.
    Quality(u64),
    /// Play through this device, or through the system's own when there is none.
    Output(Option<String>),
    /// Ask which devices there are.
    Outputs,
    /// Look for the synced lyrics of a track.
    Lyrics(Track),
    /// Put a track's lyrics, a line each, into the language `to` names (`es`, `en`).
    Translate { track_id: String, lines: Vec<String>, to: String },
    /// Ask the catalog for the home page's picks.
    Discover(Taste),
    /// Ask for songs that go with a playlist: what the catalog plays after these of its
    /// songs, without the ones in `known`.
    Suggest { playlist: u64, seeds: Vec<Track>, known: HashSet<String> },
    /// Look for a track's own video and fetch a few seconds of it.
    Clip(Track),
    /// Go on from where the track that plays is with another rendition of it in its place.
    Swap(Track),
    /// Ask who the artist of a track is.
    About(Track),
    /// Ask what each of these artists, by their ids, has released.
    News(Vec<String>),
    /// Ask whether a later version of the app has been published.
    CheckUpdate,
    /// Ask for every version published and what each says of itself.
    Patches,
    /// Fetch this system's program of a release and put it where the running one is.
    InstallUpdate(Release),
    /// Keep a track on disk to play it without a network, or stop keeping it.
    Save(Track),
    Unsave(String),
}

/// A later version of the app, as its release presents it.
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: String,
    /// Its page, for whoever fetches it by hand.
    pub page: String,
    /// Where this system's program is and where the sums it is checked against are, when
    /// the release has them and this build takes versions in by itself.
    pub program: Option<(String, String)>,
    /// The release says versions as old as this one may not keep running.
    pub mandatory: bool,
    /// What it says of itself, as whoever published it wrote it.
    pub notes: Vec<update::Note>,
}

/// A version as its release tells of it, for the page of what each one brought.
#[derive(Clone, Debug, PartialEq)]
pub struct Patch {
    pub version: String,
    /// The day it came out, as `2026-10-17`.
    pub date: String,
    pub notes: Vec<update::Note>,
}

/// An artist as their own page presents them, and what it says of them.
pub type Known = (ArtistRef, Option<String>);

/// What the home page's picks are made from: what the user plays and what they already have.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Taste {
    /// A track for each of the artists played most, the most played first.
    pub seeds: Vec<Track>,
    /// The ids of the tracks the user already has.
    pub tracks: HashSet<String>,
    /// The artists already played or followed, as `discover::plain` writes them.
    pub artists: HashSet<String>,
    /// The ids of artists the user plays or follows, for more of their albums.
    pub artist_ids: Vec<String>,
    /// The ids of the albums already kept.
    pub albums: HashSet<String>,
    /// The latest plays, one for each artist: what goes with them is today's.
    pub recent: Vec<Track>,
}

/// What the catalog picked for the home page, a part at a time as each is ready.
#[derive(Clone, Debug, PartialEq)]
pub enum Picks {
    /// A mix out of every seed's radio, and what goes with each of the first seeds.
    Tracks { mix: Vec<Track>, because: Vec<(Track, Vec<Track>)> },
    /// Artists the user does not play yet.
    Artists(Vec<ArtistRef>),
    /// Albums of those artists and of the ones the user plays.
    Albums(Vec<AlbumRef>),
    /// What goes with the latest plays: today's.
    Recent(Vec<Track>),
    /// For each of the artists played most, the albums of the ones their radio plays.
    FansOf(Vec<(ArtistRef, Vec<AlbumRef>)>),
    /// The artist played most, with their releases.
    MoreOf(ArtistRef, Vec<AlbumRef>),
}

/// What comes back. Every answer names what it answers, because answers arrive late and out of
/// order: a result for a query or a page the user already left must be ignored.
#[derive(Clone, Debug)]
pub enum Event {
    /// A picture chosen, kept on disk with the covers under `name`, which fetches it as one.
    Picture { of: PictureFor, name: String },
    /// How loud what the microphone hears is, 0 to 1, and for how long it has listened.
    Hearing { level: f32, seconds: f32 },
    /// The song heard, none when Shazam did not know it, or why it could not be asked.
    Recognized(Result<Option<crate::recognize::Recognized>, String>),
    /// Signed in to a service, or why not.
    SignedIn(Result<crate::scrobble::Account, String>),
    /// The first of what waited for a service went.
    Scrobbled { service: crate::scrobble::Service, sent: usize },
    /// Where the account is served, and the code a phone reads that from.
    /// What is served, with its QR code when one could be made: the code to type is enough.
    Offered(Result<(Offer, Option<RasterImage>), String>),
    /// An account read from another device, its pictures kept with the covers already.
    Fetched(Result<Bundle, String>),
    /// The artists found for the picker, under the key they were asked for with.
    ArtistsFound { key: String, artists: Vec<ArtistRef> },
    SearchResults { query: String, found: Found },
    SearchFailed { query: String, message: String },
    Opened { target: Target, result: Result<Content, String> },
    /// What is kept on disk, newest first, the ids on their way there, and the room it takes.
    Library { saved: Vec<Track>, saving: HashSet<String>, bytes: u64 },
    SaveFailed { title: String, message: String },
    /// A cover, and the color it lends to a card of its track when it could be read.
    /// A picture, the colour it lends to a card and its width and height in pixels.
    Artwork { url: String, image: RasterImage, tint: Option<[u8; 3]>, size: Option<(u32, u32)> },
    /// A cover that could not be had.
    ArtworkMissing(String),
    Outputs(Vec<AudioOutputDevice>),
    OutputsFailed(String),
    OutputFailed(String),
    /// A track's lines and when each starts, or none when nobody has synced them.
    Lyrics { track_id: String, lines: Option<Vec<Line>> },
    /// Available text without synchronization; these lines have no seek position.
    LyricsPlain { track_id: String, lines: Vec<String>, source: String },
    /// A transient provider/network failure is not evidence that the song has no lyrics.
    LyricsFailed { track_id: String, message: String, retry_after: Duration },
    /// A track's lyrics in the language asked for, a line for each of theirs; none when
    /// they already are in it.
    Translated { track_id: String, to: String, lines: Result<Option<Vec<String>>, String> },
    /// A track's own video, or none when it has no such video.
    Clip { track_id: String, footage: Option<Arc<Footage>> },
    /// A part of the home page's picks.
    Discovered(Picks),
    /// Songs that go with a playlist, the closest first; none when the catalog had none.
    Suggested { playlist: u64, tracks: Vec<Track> },
    /// Who the artist a track names is, or nobody when the catalog knows no artist of
    /// that name.
    About { artist: String, found: Option<Known> },
    /// What an artist has out, the albums and then the singles, the latest first.
    Releases { artist: String, releases: Vec<AlbumRef> },
    /// A later version of the app.
    Update(Release),
    /// The versions published, the latest first, or why they could not be had.
    Patches(Result<Vec<Patch>, String>),
    /// How much of a later version's program has arrived, 0 to 1.
    UpdateProgress(f32),
    /// How many of the songs of a list being brought in have been looked for, out of how many.
    Importing { done: usize, of: usize },
    /// The later version is in place: the next start is it. Or why it is not.
    UpdateInstalled,
    UpdateFailed(String),
    /// The device that was asked for is gone: the system's own plays instead.
    OutputLost,
}

/// How many times a broken download resumes from where it stopped before the track fails.
const DOWNLOAD_ATTEMPTS: u32 = 4;

/// What reaches the player from the worker's own side.
enum Input {
    Engine(surco_audio::Event),
    Media(MediaEvent),
    Radio { seed: String, tracks: Vec<Track> },
    /// The SponsorBlock segments of a video, to be jumped over while it plays.
    Segments { track_id: String, segments: Vec<crate::sponsorblock::Segment> },
}

/// A track on its way into a deck.
struct Download {
    track_id: String,
    task: JoinHandle<()>,
    buffer: MediaBuffer,
}

impl Download {
    fn cancel(self) {
        self.task.abort();
        // Whoever still reads the buffer gets an end of stream instead of waiting for bytes.
        self.buffer.fail();
    }
}

#[derive(Default)]
struct Downloads {
    active: Option<Download>,
    standby: Option<Download>,
}

/// Everything a task needs, shared between them.
struct Shared {
    reply: Box<dyn Fn(Event) + Send + Sync>,
    youtube: YouTube,
    lyrics: lyrics::Provider,
    audio: NativeAudio,
    /// Bumped by every `Effect::Play`. A task that finds another number was overtaken and stops.
    generation: AtomicU64,
    /// Counts the handovers asked for: only the last one asked may take the sound.
    handovers: AtomicU64,
    /// The most bits per second a stream is fetched at.
    quality: AtomicU64,
    /// The user's volume, and what the sleep timer leaves of it, as the engine is given their product.
    volume: Mutex<(f32, f32)>,
    /// The SponsorBlock categories skipped, and the segments of the videos asked for lately.
    sponsor: std::sync::atomic::AtomicU8,
    /// The microphone while it listens for a song, and which listening it is: an older one's
    /// answer is not said.
    microphone: Mutex<Option<surco_audio::microphone::Microphone>>,
    listening: AtomicU64,
    segments: Mutex<HashMap<String, Vec<crate::sponsorblock::Segment>>>,
    downloads: Mutex<Downloads>,
    /// Held while a task changes what the active deck holds, so the newest `Play` has the last word.
    decks: tokio::sync::Mutex<()>,
    /// The way back into the player.
    inputs: UnboundedSender<Input>,
    library: Mutex<Library>,
    saving: Mutex<HashSet<String>>,
    /// How many covers are fetched at once: a library opening asks for all of its own, and
    /// the stream of a song asked for meanwhile should not queue behind them.
    covers: tokio::sync::Semaphore,
    /// The account being served to another device, while its dialog is open. Held across
    /// starting one, so that two asked for at once make one.
    serving: tokio::sync::Mutex<Option<handoff::Serving>>,
    #[cfg(target_os = "ios")]
    cached_loads: Mutex<Vec<CachedLoad>>,
}

#[cfg(target_os = "ios")]
#[derive(Clone)]
struct CachedLoad {
    track_id: String,
    buffer: MediaBuffer,
    mime_type: String,
    duration_sec: Option<f64>,
    gain: f32,
}

impl Shared {
    fn is_current(&self, generation: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == generation
    }

    #[cfg(target_os = "ios")]
    fn remember(&self, load: &Load) {
        let mut cached = self.cached_loads.lock().unwrap();
        cached.retain(|saved| saved.track_id != load.track_id);
        cached.push(CachedLoad {
            track_id: load.track_id.clone(), buffer: load.buffer.clone(), mime_type: load.mime_type.clone(),
            duration_sec: load.duration_sec, gain: load.gain,
        });
        if cached.len() > 2 { cached.remove(0); }
    }

    #[cfg(target_os = "ios")]
    fn restored_load(&self, track_id: &str, at: f64) -> Option<Load> {
        self.cached_loads.lock().unwrap().iter().find(|load| load.track_id == track_id && !load.buffer.is_failed()).map(|load| Load {
            track_id: load.track_id.clone(), buffer: load.buffer.clone(), mime_type: load.mime_type.clone(),
            duration_sec: load.duration_sec, start_sec: Some(at), standby: false, gain: load.gain,
        })
    }

    /// Tells the window what the library holds now.
    fn announce(&self) {
        let (saved, bytes) = {
            let library = self.library.lock().unwrap();
            (library.tracks(), library.bytes())
        };
        let saving = self.saving.lock().unwrap().clone();
        (self.reply)(Event::Library { saved, saving, bytes });
    }
}

#[cfg(target_os = "ios")]
fn transport_decision(command: &Command) -> bool {
    matches!(command, Command::PlayList { .. } | Command::PlayPause | Command::Next | Command::Previous
        | Command::Jump(_) | Command::Seek(_) | Command::Swap(_) | Command::PlayNext(_) | Command::Enqueue(_)
        | Command::Output(_))
}

#[cfg(target_os = "ios")]
struct IosSession {
    lifecycle: crate::ios_media::Lifecycle,
    active: bool,
    released: bool,
}

#[cfg(target_os = "ios")]
impl Default for IosSession {
    fn default() -> Self {
        Self { lifecycle: crate::ios_media::Lifecycle::default(), active: false, released: true }
    }
}

#[cfg(target_os = "ios")]
impl IosSession {
    fn release(&mut self, shared: &Result<Arc<Shared>, String>, deactivate: bool) {
        if let Ok(shared) = shared {
            if !self.released {
                // A load already decoding must not start playing after a pause or interruption.
                shared.generation.fetch_add(1, Ordering::SeqCst);
                shared.handovers.fetch_add(1, Ordering::SeqCst);
                if let Err(error) = shared.audio.release_output() { (shared.reply)(Event::OutputFailed(error)); }
            }
            if deactivate && self.active {
                if let Err(error) = crate::ios_media::deactivate() { (shared.reply)(Event::OutputFailed(error)); }
            }
        }
        if deactivate { self.active = false; }
        self.released = true;
    }

    fn event(&mut self, shared: &Result<Arc<Shared>, String>, player: &mut Player, event: crate::ios_media::SessionEvent) -> Vec<Effect> {
        use crate::ios_media::{SessionAction, SessionEvent};
        let view = player.view();
        let action = self.lifecycle.event(event, view.current.as_ref().map(|track| track.id.as_str()), view.playing);
        match action {
            SessionAction::None => Vec::new(),
            SessionAction::Pause | SessionAction::Reset => {
                let interrupted = event == SessionEvent::Interrupted;
                self.release(shared, !interrupted);
                if interrupted { self.active = false; }
                if action == SessionAction::Reset {
                    if let Err(error) = crate::ios_media::configure() {
                        if let Ok(shared) = shared { (shared.reply)(Event::OutputFailed(error)); }
                    }
                }
                Key::Pause.command(&view).map(|command| player.command(command)).unwrap_or_default()
            }
            SessionAction::Resume => Key::Play.command(&view).map(|command| player.command(command)).unwrap_or_default(),
            SessionAction::Rebuild => {
                self.release(shared, false);
                if view.playing { player.reload() } else { Vec::new() }
            }
        }
    }

    fn effect(&mut self, shared: &Result<Arc<Shared>, String>, player: &mut Player, effect: Effect, explicit: bool) -> Option<Effect> {
        let effect = match effect {
            Effect::Pause | Effect::Restore(_, _, false) => {
                self.release(shared, true);
                return None;
            }
            Effect::Resume if self.released => {
                let view = player.view();
                Effect::Restore(view.current?, view.position, true)
            }
            other => other,
        };
        if matches!(effect, Effect::Play(_) | Effect::Fade(_, _) | Effect::Restore(_, _, true) | Effect::Resume | Effect::Handover(_, _)) {
            let activated = if !self.lifecycle.can_activate(explicit) {
                Err(t!("backend-playback-interrupted").into_owned())
            } else if self.active { Ok(()) } else { crate::ios_media::activate() };
            if let Err(message) = activated {
                self.release(shared, true);
                if let Some(track) = player.view().current {
                    player.engine(surco_audio::Event::Failed { track_id: track.id, message });
                } else if let Ok(shared) = shared { (shared.reply)(Event::OutputFailed(message)); }
                return None;
            }
            self.active = true;
            self.lifecycle.activated();
            if let Ok(shared) = shared { shared.audio.prepare_output(); }
            self.released = false;
        }
        Some(effect)
    }
}

/// How soon the window is asked to draw when something outside it changed. Asked for at
/// once, egui draws two frames for each asking; asked for a moment on, the one that shows
/// the change.
pub const SOON: Duration = Duration::from_millis(1);

/// Starts the worker and returns its ends. The worker wakes the window whenever it has
/// something to say, so the window can sleep.
///
/// Draws the window again soon, unless a phone has it off screen: behind the lock screen the
/// music plays on in this thread and nobody sees a frame. Events wait in their channel, and
/// the window reads them and the player's latest view once it is back.
fn wake(ctx: &egui::Context) {
    #[cfg(target_os = "ios")]
    if !crate::ios_ui::on_screen() { return; }
    ctx.request_repaint_after(SOON);
}

/// The player is a value to read, not a stream of events: a hidden window stops repainting,
/// and positions would pile up for as long as it stays hidden.
pub fn spawn(ctx: egui::Context, library: PathBuf) -> (UnboundedSender<Command>, Receiver<Event>, watch::Receiver<PlayerView>) {
    let (commands, mut inbox) = unbounded_channel::<Command>();
    let (outbox, events) = channel::<Event>();
    let (views, player_view) = watch::channel(PlayerView::default());
    let window = ctx.clone();
    let reply = move |event: Event| {
        // The window is gone when this fails, and the worker goes with its channel.
        let _ = outbox.send(event);
        wake(&ctx);
    };
    std::thread::Builder::new()
        .name("backend".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().expect("a tokio runtime");
            runtime.block_on(async move {
                let (inputs, mut inbox_own) = unbounded_channel();
                let shared = start(reply.clone(), inputs.clone(), library).map(Arc::new);
                if let Ok(shared) = &shared {
                    shared.announce();
                }
                let mut player = Player::default();
                let pressed = inputs.clone();
                let mut keys = MediaKeys::start(move |event| drop(pressed.send(Input::Media(event))));
                #[cfg(target_os = "ios")]
                let mut session = IosSession::default();
                loop {
                    #[cfg(target_os = "ios")]
                    let mut explicit_playback = false;
                    let effects = tokio::select! {
                        command = inbox.recv() => match command {
                            Some(command) => {
                                #[cfg(target_os = "ios")]
                                if transport_decision(&command) {
                                    session.lifecycle.user_decision();
                                    explicit_playback = true;
                                }
                                handle(&shared, &reply, &mut player, command)
                            },
                            None => return,
                        },
                        Some(input) = inbox_own.recv() => match input {
                            Input::Engine(event) => player.engine(event),
                            Input::Media(MediaEvent::Key(key)) => {
                                #[cfg(target_os = "ios")]
                                {
                                    session.lifecycle.user_decision();
                                    explicit_playback = true;
                                }
                                key.command(&player.view()).map(|command| player.command(command)).unwrap_or_default()
                            },
                            #[cfg(target_os = "ios")]
                            Input::Media(MediaEvent::Session(event)) => session.event(&shared, &mut player, event),
                            #[cfg(target_os = "ios")]
                            Input::Media(MediaEvent::Artwork { track_id, url }) => {
                                if let Ok(shared) = &shared {
                                    let shared = shared.clone();
                                    tokio::spawn(async move {
                                        if let Some(bytes) = cover_bytes(&shared, &url).await.filter(|bytes| bytes.len() <= 16 * 1024 * 1024) {
                                            crate::ios_media::artwork(&track_id, &url, &bytes);
                                        }
                                    });
                                }
                                Vec::new()
                            },
                            Input::Radio { seed, tracks } => {
                                player.radio(&seed, tracks);
                                Vec::new()
                            }
                            Input::Segments { track_id, segments } => {
                                player.segments(&track_id, segments);
                                Vec::new()
                            }
                        },
                    };
                    for effect in effects {
                        #[cfg(target_os = "ios")]
                        let effect = match session.effect(&shared, &mut player, effect, explicit_playback) {
                            Some(effect) => effect,
                            None => continue,
                        };
                        match (&shared, effect) {
                            (Ok(shared), effect) => run(shared, effect),
                            // Nothing can play: the track fails with the reason the backend did not start.
                            (Err(message), Effect::Play(track)) => {
                                let failed = surco_audio::Event::Failed { track_id: track.id, message: message.clone() };
                                let _ = inputs.send(Input::Engine(failed));
                            }
                            _ => {}
                        }
                    }
                    let view = player.view();
                    #[cfg(target_os = "ios")]
                    if !view.playing { session.release(&shared, true); }
                    keys.show(&view);
                    let changed = views.send_if_modified(|shown| {
                        let changed = *shown != view;
                        *shown = view;
                        changed
                    });
                    if changed {
                        wake(&window);
                    }
                }
            });
        })
        .expect("the backend thread starts");
    (commands, events, player_view)
}

fn start(reply: impl Fn(Event) + Send + Sync + 'static, inputs: UnboundedSender<Input>, library: PathBuf) -> Result<Shared, String> {
    let youtube = YouTube::new().map_err(|error| error.to_string())?;
    let events = inputs.clone();
    let audio = NativeAudio::start(None, move |event| drop(events.send(Input::Engine(event))))
        .map_err(|error| t!("backend-output-open-failed", error = error.to_string()).into_owned())?;
    Ok(Shared {
        reply: Box::new(reply),
        youtube,
        lyrics: lyrics::Provider::default(),
        audio,
        generation: AtomicU64::new(0),
        handovers: AtomicU64::new(0),
        quality: AtomicU64::new(u64::MAX),
        downloads: Mutex::default(),
        decks: tokio::sync::Mutex::default(),
        inputs,
        library: Mutex::new(Library::open(library)),
        saving: Mutex::default(),
        covers: tokio::sync::Semaphore::new(COVERS_AT_ONCE),
        volume: Mutex::new((1.0, 1.0)),
        sponsor: std::sync::atomic::AtomicU8::new(crate::sponsorblock::DEFAULT),
        microphone: Mutex::new(None),
        listening: AtomicU64::new(0),
        segments: Mutex::new(HashMap::new()),
        serving: tokio::sync::Mutex::new(None),
        #[cfg(target_os = "ios")]
        cached_loads: Mutex::default(),
    })
}

/// Answers what needs no player and hands the rest to it.
fn handle(shared: &Result<Arc<Shared>, String>, reply: &impl Fn(Event), player: &mut Player, command: Command) -> Vec<Effect> {
    match (command, shared) {
        (Command::Search(query), Ok(shared)) => {
            let shared = shared.clone();
            tokio::spawn(async move {
                let event = match shared.youtube.search(&query).await {
                    Ok(found) => Event::SearchResults { query, found },
                    Err(error) => Event::SearchFailed { query, message: error.to_string() },
                };
                (shared.reply)(event);
            });
        }
        (Command::Search(query), Err(message)) => reply(Event::SearchFailed { query, message: message.clone() }),
        (Command::Open(target), Ok(shared)) => {
            let shared = shared.clone();
            tokio::spawn(async move {
                let result = open(&shared, &target).await;
                (shared.reply)(Event::Opened { target, result });
            });
        }
        (Command::Open(target), Err(message)) => reply(Event::Opened { target, result: Err(message.clone()) }),
        (Command::PickPicture(of), Ok(shared)) => {
            let shared = shared.clone();
            tokio::spawn(async move {
                let Some(bytes) = pick_picture().await else { return };
                // Decoding and writing hold the library: off the runtime's two workers.
                let kept = tokio::task::block_in_place(|| {
                    let jpeg = square_jpeg(&bytes)?;
                    let name = format!("{PICTURE_NAME}{:016x}", fastrand::u64(..));
                    shared.library.lock().unwrap().keep_cover(&name, &jpeg).ok()?;
                    Some(name)
                });
                if let Some(name) = kept { (shared.reply)(Event::Picture { of, name }); }
            });
        }
        (Command::Serve(mut bundle), Ok(shared)) => {
            let shared = shared.clone();
            tokio::spawn(async move {
                // The pictures of its own go along, from where they are kept.
                bundle.pictures = tokio::task::block_in_place(|| {
                    let library = shared.library.lock().unwrap();
                    bundle.picture_names().into_iter().filter_map(|name| {
                        let bytes = library.cover(&name)?;
                        Some((name, base64::engine::general_purpose::STANDARD.encode(bytes)))
                    }).collect()
                });
                let Ok(body) = serde_json::to_vec(&bundle) else { return };
                let mut serving = shared.serving.lock().await;
                if let Some(current) = serving.as_ref() {
                    current.replace(body);
                    return;
                }
                // Guessed at too often, the bundle stops being served: the dialog says so, and
                // the next part switched serves it anew under another code.
                let watcher = shared.clone();
                let stopped = move |offer: handoff::Offer| {
                    tokio::spawn(async move {
                        let mut serving = watcher.serving.lock().await;
                        if serving.as_ref().is_some_and(|current| current.offer == offer) {
                            *serving = None;
                            (watcher.reply)(Event::Offered(Err(t!("backend-handoff-stopped").into_owned())));
                        }
                    });
                };
                let offered = match handoff::serve(body, stopped).await {
                    Ok(started) => {
                        let offer = started.offer.clone();
                        *serving = Some(started);
                        let qr = code_image(&offer.link);
                        Ok((offer, qr))
                    }
                    Err(message) => Err(message),
                };
                drop(serving);
                (shared.reply)(Event::Offered(offered));
            });
        }
        (Command::FindArtists { key, queries }, Ok(shared)) => {
            let shared = shared.clone();
            tokio::spawn(async move {
                let found = join_all(queries.iter().map(|query| shared.youtube.search(query))).await;
                // shortcut: the catalog lists its own mixes and playlists among artists; told
                // apart by name until it says what each is.
                let artist = |artist: &ArtistRef| { let name = artist.name.to_lowercase(); !name.ends_with(" mix") && !name.contains("playlist") };
                let lists: Vec<Vec<ArtistRef>> = found.into_iter().map(|found| found.map(|found| found.artists.into_iter().filter(artist).collect()).unwrap_or_default()).collect();
                // A turn each, so that no genre leads the picker.
                let mut artists: Vec<ArtistRef> = Vec::new();
                for rank in 0..lists.iter().map(Vec::len).max().unwrap_or(0) {
                    for artist in lists.iter().filter_map(|list| list.get(rank)) {
                        if artists.len() < PICKER_MOST && !artists.iter().any(|known| known.id == artist.id) { artists.push(artist.clone()); }
                    }
                }
                (shared.reply)(Event::ArtistsFound { key, artists });
            });
        }
        (Command::FindArtists { key, .. }, Err(_)) => reply(Event::ArtistsFound { key, artists: Vec::new() }),
        (Command::StopServing, Ok(shared)) => {
            let shared = shared.clone();
            tokio::spawn(async move { shared.serving.lock().await.take(); });
        }
        (Command::Fetch(url), Ok(shared)) => {
            let shared = shared.clone();
            tokio::spawn(async move {
                let fetched = fetch_bundle(&shared, &url).await;
                (shared.reply)(Event::Fetched(fetched));
            });
        }
        (Command::Serve(_), Err(message)) => reply(Event::Offered(Err(message.clone()))),
        (Command::Fetch(_), Err(message)) => reply(Event::Fetched(Err(message.clone()))),
        (Command::StopServing, Err(_)) => {}
        (Command::ScanCode, _) => {
            #[cfg(target_os = "ios")]
            crate::ios_ui::scan_code(&t!("native-scan-hint"), &t!("common-close"), &t!("native-camera-denied"), &t!("native-camera-denied-detail"), &t!("native-open-settings"), &t!("common-cancel"));
        }
        (Command::FetchArtwork(url), Ok(shared)) => {
            let shared = shared.clone();
            tokio::spawn(async move {
                let Ok(_turn) = shared.covers.acquire().await else { return };
                // A cover that does not arrive leaves its placeholder instead of the pulse.
                (shared.reply)(match artwork(&shared, &url).await {
                    Some((image, tint, size)) => Event::Artwork { url, image, tint, size },
                    None => Event::ArtworkMissing(url),
                });
            });
        }
        // The slider is evenly spaced to the ear, not to the amplitude: half way is far quieter
        // than half the samples. The engine only fails this when its thread is gone, which the
        // next `Play` reports.
        (Command::Volume(volume), Ok(shared)) => {
            let dim = { let mut held = shared.volume.lock().unwrap(); held.0 = volume; held.1 };
            drop(shared.audio.set_volume(volume.powf(2.5) * dim, false));
        }
        (Command::FetchArtwork(_) | Command::PickPicture(_) | Command::Volume(_), Err(_)) => {}
        (Command::Equalizer(values), _) => equalizer::set_values(values),
        (Command::Mono(on), _) => equalizer::set_mono(on),
        (Command::Normalize(on), _) => equalizer::set_normalize(on),
        (Command::SkipSilence(on), _) => surco_audio::silence::set_skip_silence(on),
        (Command::Recognize, Ok(shared)) => recognize(shared),
        (Command::StopRecognizing, Ok(shared)) => {
            shared.listening.fetch_add(1, Ordering::SeqCst);
            shared.microphone.lock().unwrap().take();
        }
        (Command::Recognize, Err(message)) => reply(Event::Recognized(Err(message.clone()))),
        (Command::StopRecognizing, Err(_)) => {}
        (Command::ScrobbleSignIn(sign_in), Ok(shared)) => {
            let shared = shared.clone();
            tokio::spawn(async move {
                let account = crate::scrobble::sign_in(&shared.youtube, sign_in).await;
                (shared.reply)(Event::SignedIn(account));
            });
        }
        (Command::NowPlaying { accounts, listen }, Ok(shared)) => {
            let shared = shared.clone();
            tokio::spawn(async move { crate::scrobble::playing_now(&shared.youtube, &accounts, &listen).await });
        }
        (Command::Scrobble(accounts), Ok(shared)) => {
            let shared = shared.clone();
            tokio::spawn(async move {
                for service in [crate::scrobble::Service::ListenBrainz, crate::scrobble::Service::LastFm] {
                    let sent = crate::scrobble::send(&shared.youtube, &accounts, service).await;
                    if sent > 0 { (shared.reply)(Event::Scrobbled { service, sent }); }
                }
            });
        }
        (Command::ScrobbleSignIn(_), Err(message)) => reply(Event::SignedIn(Err(message.clone()))),
        (Command::NowPlaying { .. } | Command::Scrobble(_), Err(_)) => {}
        (Command::SponsorBlock(mask), Ok(shared)) => {
            shared.sponsor.store(mask, Ordering::SeqCst);
            shared.segments.lock().unwrap().clear();
        }
        (Command::SponsorBlock(_), Err(_)) => {}
        (Command::Quality(bps), Ok(shared)) => shared.quality.store(bps, Ordering::SeqCst),
        (Command::Quality(_), Err(_)) => {}
        (Command::Output(id), Ok(shared)) => {
            #[cfg(target_os = "ios")]
            {
                // iOS chooses the route through AVAudioSession; never open inactive I/O here.
                let _ = id;
                if let Err(error) = shared.audio.release_output() { reply(Event::OutputFailed(error)); }
                return player.reload();
            }
            #[cfg(not(target_os = "ios"))]
            {
            // The engine reopens its stream and both decks go with the old one. A device
            // that is gone gives way to the system's.
            if let Err(error) = shared.audio.set_output_device(id.clone()) {
                if id.is_some() {
                    match shared.audio.set_output_device(None) {
                        Ok(()) => reply(Event::OutputLost),
                        Err(fallback) => reply(Event::OutputFailed(t!("backend-output-both-failed", error = error.to_string(), fallback = fallback.to_string()).into_owned())),
                    }
                } else {
                    reply(Event::OutputFailed(t!("backend-output-default-failed", error = error.to_string()).into_owned()));
                }
            }
            return player.reload();
            }
        }
        (Command::Outputs, Ok(shared)) => {
            let shared = shared.clone();
            tokio::spawn(async move {
                // Listing may ask the sound server, which takes a moment.
                (shared.reply)(match blocking(surco_audio::list_output_devices).await {
                    Ok(devices) => Event::Outputs(devices),
                    Err(message) => Event::OutputsFailed(message),
                });
            });
        }
        (Command::Outputs, Err(message)) => reply(Event::OutputsFailed(message.clone())),
        (Command::Output(_), Err(message)) => reply(Event::OutputFailed(t!("backend-engine-unavailable", message = message.as_str()).into_owned())),
        (Command::Lyrics(track), Ok(shared)) => {
            let shared = shared.clone();
            tokio::spawn(async move {
                let event = match shared.lyrics.fetch(&shared.youtube, &track).await {
                    Ok(Some(lyrics::Found::Synced(lines))) => Event::Lyrics { track_id: track.id, lines: Some(lines) },
                    Ok(Some(lyrics::Found::Plain { lines, source })) => Event::LyricsPlain { track_id: track.id, lines, source },
                    Ok(None) => Event::Lyrics { track_id: track.id, lines: None },
                    Err(failure) => {
                        // Deliberately no track names, request URLs, headers or provider body.
                        eprintln!("[surco::lyrics] {}", failure.message);
                        Event::LyricsFailed { track_id: track.id, message: failure.message, retry_after: failure.retry_after }
                    }
                };
                (shared.reply)(event);
            });
        }
        (Command::Lyrics(track), Err(_)) => reply(Event::LyricsFailed { track_id: track.id, message: t!("backend-lyrics-start-failed").into_owned(), retry_after: Duration::ZERO }),
        (Command::Translate { track_id, lines, to }, Ok(shared)) => {
            let shared = shared.clone();
            tokio::spawn(async move {
                let lines = translate(&shared, &lines, &to).await;
                (shared.reply)(Event::Translated { track_id, to, lines });
            });
        }
        (Command::Translate { track_id, to, .. }, Err(message)) => reply(Event::Translated { track_id, to, lines: Err(message.clone()) }),
        (Command::Discover(taste), Ok(shared)) => {
            let shared = shared.clone();
            tokio::spawn(async move { discover(&shared, taste).await });
        }
        (Command::Discover(_), Err(_)) => {}
        (Command::Suggest { playlist, seeds, known }, Ok(shared)) => {
            let shared = shared.clone();
            tokio::spawn(async move {
                let radios: Vec<Vec<Track>> = join_all(seeds.iter().map(|seed| async { shared.youtube.radio(&seed.id).await.unwrap_or_default() })).await;
                (shared.reply)(Event::Suggested { playlist, tracks: discover::blend(&radios, &known, SUGGESTIONS) });
            });
        }
        (Command::Suggest { playlist, .. }, Err(_)) => reply(Event::Suggested { playlist, tracks: Vec::new() }),
        (Command::Patches, Ok(shared)) => {
            let shared = shared.clone();
            tokio::spawn(async move { (shared.reply)(Event::Patches(patches(&shared).await)) });
        }
        (Command::Patches, Err(message)) => reply(Event::Patches(Err(message.clone()))),
        (Command::About(track), Ok(shared)) => {
            let shared = shared.clone();
            tokio::spawn(async move {
                // A track by several names them all; the first is whose it is.
                let name = track.artist.split(',').next().unwrap_or_default().trim();
                let id = match &track.artist_id {
                    Some(id) => Some(id.clone()),
                    None => shared.youtube.artist_named(name).await.ok().flatten().map(|artist| artist.id),
                };
                let found = match id {
                    Some(id) => shared.youtube.about(&id).await.ok(),
                    None => None,
                };
                (shared.reply)(Event::About { artist: track.artist, found });
            });
        }
        (Command::About(track), Err(_)) => reply(Event::About { artist: track.artist, found: None }),
        (Command::News(artists), Ok(shared)) => {
            for artist in artists {
                let shared = shared.clone();
                tokio::spawn(async move {
                    if let Ok((albums, singles)) = shared.youtube.releases(&artist).await {
                        (shared.reply)(Event::Releases { artist, releases: albums.into_iter().chain(singles).collect() });
                    }
                });
            }
        }
        // Asked once, as the app opens: it is looked for again for as long as the app stays open.
        (Command::CheckUpdate, Ok(shared)) => {
            // iOS distributes signed application bundles; the desktop updater replaces an executable.
            if cfg!(target_os = "ios") {
                return Vec::new();
            }
            let shared = shared.clone();
            tokio::spawn(async move {
                let mut announced = None;
                loop {
                    if let Some(release) = latest_release(&shared).await {
                        if announced.as_ref() != Some(&release.version) {
                            announced = Some(release.version.clone());
                            tokio::task::spawn_blocking({ let version = release.version.clone(); move || announce_update(&version) });
                        }
                        (shared.reply)(Event::Update(release));
                    }
                    tokio::time::sleep(UPDATE_EVERY).await;
                }
            });
        }
        (Command::InstallUpdate(release), Ok(shared)) => {
            let shared = shared.clone();
            tokio::spawn(async move {
                let done = install_update(&shared, &release).await;
                (shared.reply)(done.map_or_else(Event::UpdateFailed, |()| Event::UpdateInstalled));
            });
        }
        (Command::InstallUpdate(_), Err(message)) => reply(Event::UpdateFailed(message.clone())),
        (Command::News(_) | Command::CheckUpdate, Err(_)) => {}
        (Command::Clip(track), Ok(shared)) => {
            let shared = shared.clone();
            tokio::spawn(async move {
                let footage = footage(&shared, &track).await.map(Arc::new);
                (shared.reply)(Event::Clip { track_id: track.id, footage });
            });
        }
        (Command::Clip(track), Err(_)) => reply(Event::Clip { track_id: track.id, footage: None }),
        (Command::Save(track), Ok(shared)) => {
            let known = shared.library.lock().unwrap().audio(&track.id).is_some();
            if known || !shared.saving.lock().unwrap().insert(track.id.clone()) {
                return Vec::new();
            }
            shared.announce();
            let shared = shared.clone();
            tokio::spawn(async move {
                let result = save(&shared, &track).await;
                shared.saving.lock().unwrap().remove(&track.id);
                shared.announce();
                if let Err(message) = result {
                    (shared.reply)(Event::SaveFailed { title: track.title, message });
                }
            });
        }
        (Command::Unsave(id), Ok(shared)) => {
            // An index that cannot be rewritten still lists the track; the window shows what is true.
            let _ = shared.library.lock().unwrap().remove(&id);
            shared.announce();
        }
        (Command::Save(_) | Command::Unsave(_), Err(_)) => {}
        (command, _) => return player.command(command),
    }
    Vec::new()
}

async fn open(shared: &Shared, target: &Target) -> Result<Content, String> {
    let youtube = &shared.youtube;
    let content = match target {
        Target::Artist(id) => youtube.artist(id).await.map(Content::Artist),
        Target::ArtistNamed(name) => match youtube.find_artist(name).await {
            Ok(Some(id)) => youtube.artist(&id).await.map(Content::Artist),
            Ok(None) => return Err(t!("backend-artist-not-found", name = name.as_str()).into_owned()),
            Err(error) => Err(error),
        },
        Target::Album(id) => youtube.album(id).await.map(Content::Album),
        Target::Song(id) => youtube.song(id).await.map(Content::Song),
        Target::List { name, ids } => {
            // A few at a time, in the order of the list. A track the catalog no longer has is left out.
            let mut tracks = Vec::new();
            for some in ids.chunks(8) {
                tracks.extend(join_all(some.iter().map(|id| youtube.song(id))).await.into_iter().flatten());
            }
            if tracks.is_empty() {
                return Err(t!("backend-playlist-empty").into_owned());
            }
            let missed = ids.iter().filter(|id| tracks.iter().all(|track| track.id != **id)).cloned().collect();
            Ok(Content::List { name: name.clone(), tracks, missed })
        }
        Target::Import(link) => {
            let told = |done, of| (shared.reply)(Event::Importing { done, of });
            return crate::import::list(youtube, link, &told).await.map(|(name, tracks, missed)| Content::List { name, tracks, missed });
        }
    };
    content.map_err(|error| error.to_string())
}

/// Listens for the song that plays nearby: the fingerprint grows as the sound comes in, and
/// Shazam is asked about it as it does, until it names the song or the last time is asked.
fn recognize(shared: &Arc<Shared>) {
    use crate::recognize::{lookup, Signature, ASK_AT};
    let this = shared.listening.fetch_add(1, Ordering::SeqCst) + 1;
    let (heard, mut sound) = tokio::sync::mpsc::unbounded_channel::<Vec<i16>>();
    // An iPhone's audio session plays and does not record; and it asks before the microphone.
    let microphone = if cfg!(target_os = "ios") { Err(t!("backend-recognize-unsupported").into_owned()) }
        else { surco_audio::microphone::listen(move |pcm| { let _ = heard.send(pcm.to_vec()); }) };
    let microphone = match microphone {
        Ok(microphone) => microphone,
        Err(why) => return (shared.reply)(Event::Recognized(Err(why))),
    };
    *shared.microphone.lock().unwrap() = Some(microphone);
    let shared = shared.clone();
    tokio::spawn(async move {
        let current = || shared.listening.load(Ordering::SeqCst) == this;
        let (mut signature, mut asked, mut said) = (Signature::default(), 0, Instant::now());
        while let Some(pcm) = sound.recv().await {
            if !current() { return; }
            signature.add(&pcm);
            let seconds = signature.duration_ms() as f32 / 1000.0;
            if said.elapsed() >= Duration::from_millis(80) {
                said = Instant::now();
                let loud = (pcm.iter().map(|sample| f64::from(*sample).powi(2)).sum::<f64>() / pcm.len().max(1) as f64).sqrt();
                (shared.reply)(Event::Hearing { level: (loud / 6_000.0).min(1.0) as f32, seconds });
            }
            if asked < ASK_AT.len() && f64::from(seconds) >= ASK_AT[asked] {
                asked += 1;
                // Named, it is over; not, the last time asked says how it went.
                match lookup(&shared.youtube, &signature.encode(), signature.duration_ms(), chrono::Utc::now().timestamp()).await {
                    Ok(Some(found)) => {
                        if current() { (shared.reply)(Event::Recognized(Ok(Some(found)))); }
                        break;
                    }
                    answer if asked == ASK_AT.len() => {
                        if current() { (shared.reply)(Event::Recognized(answer)); }
                        break;
                    }
                    _ => {}
                }
            }
        }
        if current() { shared.microphone.lock().unwrap().take(); }
    });
}

/// The most videos whose segments are held.
const SEGMENTS_KEPT: usize = 200;

/// Asks SponsorBlock what of `track` to jump over, and hands it to the player. A video asked
/// for before is not asked again; one that could not be asked is, next time.
fn look_for_segments(shared: &Arc<Shared>, track: &Track) {
    let mask = shared.sponsor.load(Ordering::SeqCst);
    if mask == 0 { return; }
    let id = track.id.clone();
    if let Some(segments) = shared.segments.lock().unwrap().get(&id).cloned() {
        let _ = shared.inputs.send(Input::Segments { track_id: id, segments });
        return;
    }
    let shared = shared.clone();
    tokio::spawn(async move {
        let Ok(segments) = crate::sponsorblock::segments(&shared.youtube, &id, mask).await else { return };
        {
            let mut held = shared.segments.lock().unwrap();
            if held.len() >= SEGMENTS_KEPT { held.clear(); }
            held.insert(id.clone(), segments.clone());
        }
        if !segments.is_empty() { let _ = shared.inputs.send(Input::Segments { track_id: id, segments }); }
    });
}

fn run(shared: &Arc<Shared>, effect: Effect) {
    if let Effect::Play(track) | Effect::Fade(track, _) | Effect::Restore(track, ..) | Effect::Handover(track, _) = &effect {
        look_for_segments(shared, track);
    }
    match effect {
        Effect::Play(track) => begin(shared, track, Duration::ZERO, None),
        Effect::Fade(track, seconds) => begin(shared, track, Duration::from_secs_f64(seconds), None),
        Effect::Restore(track, at, playing) => begin(shared, track, Duration::ZERO, Some((at, playing))),
        Effect::Handover(track, at) => {
            let shared = shared.clone();
            tokio::spawn(async move { handover(&shared, track, at).await });
        }
        Effect::Preload(track) => {
            let shared = shared.clone();
            tokio::spawn(async move { preload(&shared, &track).await });
        }
        Effect::Radio(track) => {
            let shared = shared.clone();
            tokio::spawn(async move {
                // Without an answer the track simply ends the queue.
                if let Ok(tracks) = shared.youtube.radio(&track.id).await {
                    let _ = shared.inputs.send(Input::Radio { seed: track.id, tracks });
                }
            });
        }
        // As with the volume: a dead engine is reported by the next `Play`.
        Effect::Pause => drop(shared.audio.pause()),
        Effect::Resume => drop(shared.audio.play()),
        Effect::Seek(seconds) => drop(shared.audio.seek(seconds)),
        Effect::Dim(dim) => {
            let volume = { let mut held = shared.volume.lock().unwrap(); held.1 = dim; held.0 };
            drop(shared.audio.set_volume(volume.powf(2.5) * dim, false));
        }
    }
}

/// Starts a track: at once, or under the end of the one that plays when `fade` is not zero.
/// With `resume` it goes on from so many seconds in, sounding or not.
fn begin(shared: &Arc<Shared>, track: Track, fade: Duration, resume: Option<(f64, bool)>) {
    let generation = shared.generation.fetch_add(1, Ordering::SeqCst) + 1;
    // The old track stops now, not when the new one has loaded. One that fades out plays on.
    if fade.is_zero() {
        drop(shared.audio.pause());
    }
    #[cfg(target_os = "ios")]
    let reuse = resume.is_some_and(|(at, _)| shared.restored_load(&track.id, at).is_some());
    #[cfg(not(target_os = "ios"))]
    let reuse = false;
    {
        let mut downloads = shared.downloads.lock().unwrap();
        // The track that fades out still reads its download: cancelling it would end it there.
        if fade.is_zero() && !reuse {
            if let Some(replaced) = downloads.active.take() { replaced.cancel(); }
        }
        if downloads.standby.as_ref().is_some_and(|download| download.track_id == track.id) {
            downloads.active = downloads.standby.take();
        }
    }
    let shared = shared.clone();
    tokio::spawn(async move {
        if let Err(message) = play(&shared, &track, generation, fade, resume).await {
            if shared.is_current(generation) {
                let failed = surco_audio::Event::Failed { track_id: track.id, message };
                let _ = shared.inputs.send(Input::Engine(failed));
            }
        }
    });
}

/// The engine answers these on its own thread, and `load` waits for the container header to
/// arrive, so neither can run on a runtime thread.
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tokio::task::spawn_blocking(work).await.map_err(|error| error.to_string())?
}

async fn play(shared: &Arc<Shared>, track: &Track, generation: u64, fade: Duration, resume: Option<(f64, bool)>) -> Result<(), String> {
    #[cfg(target_os = "ios")]
    if let Some((at, playing)) = resume {
        if let Some(load) = shared.restored_load(&track.id, at) {
            let _decks = shared.decks.lock().await;
            if !shared.is_current(generation) { return Ok(()); }
            let engine = shared.clone();
            return blocking(move || {
                engine.audio.load(load)?;
                if !engine.is_current(generation) { return engine.audio.drop_active(); }
                if playing { engine.audio.play() } else { Ok(()) }
            }).await;
        }
    }
    {
        let _decks = shared.decks.lock().await;
        if !shared.is_current(generation) {
            return Ok(());
        }
        let (engine, id) = (shared.clone(), track.id.clone());
        // A track that was preloaded starts in one step, or fades in.
        // shortcut: one that is not ready by its fade cuts in when it loads; have the player wait for the standby deck if preloads prove slow.
        if blocking(move || engine.audio.transition(&id, fade)).await? {
            return Ok(());
        }
    }
    let (mut load, download) = fetch(shared, track, false).await?;
    load.start_sec = resume.map(|(at, _)| at);
    let _decks = shared.decks.lock().await;
    if !shared.is_current(generation) {
        download.cancel();
        return Ok(());
    }
    let replaced = shared.downloads.lock().unwrap().active.replace(download);
    if let Some(replaced) = replaced {
        replaced.cancel();
    }
    let engine = shared.clone();
    blocking(move || {
        #[cfg(target_os = "ios")]
        engine.remember(&load);
        engine.audio.load(load)?;
        if !engine.is_current(generation) { return engine.audio.drop_active(); }
        // What was paused comes back paused.
        if resume.is_some_and(|(_, playing)| !playing) {
            return Ok(());
        }
        engine.audio.play()
    })
    .await
}

/// Where the app's releases are published.
const RELEASES: &str = "https://api.github.com/repos/Niiihuel/surco/releases/latest";

/// How long an open app waits before it looks for a later version again.
// shortcut: each look is one of the sixty an hour the service answers an address, shared by
// all behind it; look at where the releases' page leads instead when users share addresses.
const UPDATE_EVERY: Duration = Duration::from_secs(15 * 60);

/// The service answers nobody who does not say who they are.
const AGENT: &str = concat!("surco/", env!("CARGO_PKG_VERSION"));

/// The latest published release, when it is later than this build.
/// A later version, said once by the system's notifications, so it is seen with the window shut.
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
fn announce_update(version: &str) {
    let mut notification = notify_rust::Notification::new();
    notification.appname("Surco").summary(&t!("backend-update-title")).body(&t!("backend-update-body", version = version));
    #[cfg(target_os = "windows")]
    notification.app_id(APP_USER_MODEL_ID);
    // Nobody to tell is no failure: a desktop without a notification daemon still has the toast.
    let _ = notification.show();
}

/// The name Windows files the app's notifications under. Without one that is registered,
/// they come from PowerShell.
#[cfg(target_os = "windows")]
const APP_USER_MODEL_ID: &str = "Niiihuel.Surco";

/// Introduces the app to Windows for this user, as a program without an installer has to:
/// its notifications are filed under its name and mark, and a Start menu entry is how
/// search finds it by its name and how it is pinned to the taskbar. `data` is where the mark
/// is kept.
// https://learn.microsoft.com/windows/apps/develop/notifications/app-notifications/send-local-toast-other-apps
#[cfg(target_os = "windows")]
pub fn register_with_windows(data: &std::path::Path) {
    use windows_sys::Win32::System::Registry::{RegCloseKey, RegCreateKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ};
    let wide = |text: &str| text.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let icon = data.join("notifications.png");
    // Without its mark a notification still says whose it is.
    let _ = std::fs::write(&icon, include_bytes!("../assets/icon-rounded.png"));
    let key = wide(&format!(r"Software\Classes\AppUserModelId\{APP_USER_MODEL_ID}"));
    let mut handle: HKEY = std::ptr::null_mut();
    // SAFETY: every string is NUL-terminated UTF-16 that outlives the call, and the key is
    // closed once its two values are written.
    unsafe {
        if RegCreateKeyExW(HKEY_CURRENT_USER, key.as_ptr(), 0, std::ptr::null(), REG_OPTION_NON_VOLATILE, KEY_SET_VALUE, std::ptr::null(), &mut handle, std::ptr::null_mut()) == 0 {
            for (name, value) in [("DisplayName", "Surco".to_owned()), ("IconUri", icon.display().to_string())] {
                let value = wide(&value);
                RegSetValueExW(handle, wide(name).as_ptr(), 0, REG_SZ, value.as_ptr().cast(), (value.len() * 2) as u32);
            }
            RegCloseKey(handle);
        }
    }
    // A program opened from inside its zip runs from a folder that goes away: no entry for it.
    if let Some(program) = update::running().filter(|program| !program.starts_with(std::env::temp_dir())) {
        let program = program.to_owned();
        std::thread::spawn(move || {
            if let Err(error) = start_menu_entry(&program) {
                eprintln!("[surco] the Start menu entry could not be written: {error}");
            }
        });
    }
}

/// The Start menu's entry for the program, written each start so it follows the program
/// wherever it is kept. Its mark is the program's own.
#[cfg(target_os = "windows")]
fn start_menu_entry(program: &std::path::Path) -> windows::core::Result<()> {
    use windows::core::{Interface, HSTRING};
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    let Some(appdata) = std::env::var_os("APPDATA") else { return Ok(()) };
    let entry = std::path::Path::new(&appdata).join(r"Microsoft\Windows\Start Menu\Programs\Surco.lnk");
    // SAFETY: COM is started on this thread before the link is made, and every string handed
    // to it outlives the call that takes it.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        link.SetPath(&HSTRING::from(program))?;
        link.SetDescription(&HSTRING::from("Surco"))?;
        if let Some(folder) = program.parent() {
            link.SetWorkingDirectory(&HSTRING::from(folder))?;
        }
        link.cast::<IPersistFile>()?.Save(&HSTRING::from(entry.as_path()), true)?;
    }
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn announce_update(_version: &str) {}

async fn latest_release(shared: &Shared) -> Option<Release> {
    let request = shared.youtube.http().get(RELEASES).header("User-Agent", AGENT);
    let release: serde_json::Value = request.send().await.ok()?.error_for_status().ok()?.json().await.ok()?;
    let tag = release["tag_name"].as_str()?;
    if !discover::newer(tag, env!("CARGO_PKG_VERSION")) {
        return None;
    }
    let version = tag.trim_start_matches('v').to_owned();
    let file = |name: &str| {
        let files = release["assets"].as_array()?;
        files.iter().find(|file| file["name"].as_str() == Some(name))?["browser_download_url"].as_str().map(str::to_owned)
    };
    // A build made to be worked on is not replaced by a published one, unless it is asked to.
    let takes = !cfg!(debug_assertions) || std::env::var_os("SURCO_SELF_UPDATE").is_some();
    let program = update::program_name(&version).filter(|_| takes).and_then(|name| Some((file(&name)?, file("SHA256SUMS")?)));
    let minimum = release["body"].as_str().and_then(update::minimum);
    let mandatory = minimum.is_some_and(|minimum| discover::newer(minimum, env!("CARGO_PKG_VERSION")));
    let notes = update::notes(release["body"].as_str().unwrap_or_default());
    Some(Release { version, page: release["html_url"].as_str()?.to_owned(), program, mandatory, notes })
}

/// Every version published, the latest first; one asked for only when its page opens.
const ALL_RELEASES: &str = "https://api.github.com/repos/Niiihuel/surco/releases?per_page=30";

async fn patches(shared: &Shared) -> Result<Vec<Patch>, String> {
    let said = |error: &dyn std::fmt::Display| error.to_string();
    let response = shared.youtube.http().get(ALL_RELEASES).header("User-Agent", AGENT).send().await.map_err(|error| said(&error))?;
    let releases: Vec<serde_json::Value> = response.error_for_status().map_err(|error| said(&error))?.json().await.map_err(|error| said(&error))?;
    let published = releases.iter().filter(|release| release["draft"].as_bool() != Some(true) && release["prerelease"].as_bool() != Some(true));
    Ok(published.filter_map(|release| Some(Patch {
        version: release["tag_name"].as_str()?.trim_start_matches('v').to_owned(),
        date: release["published_at"].as_str().unwrap_or_default().chars().take(10).collect(),
        notes: update::notes(release["body"].as_str().unwrap_or_default()),
    })).collect())
}

/// Fetches the release's program for this system, checks it is the one the release
/// lists, and puts it where the program that runs is.
async fn install_update(shared: &Shared, release: &Release) -> Result<(), String> {
    let said = |error: &dyn std::fmt::Display| error.to_string();
    let (program, sums) = release.program.as_ref().ok_or_else(|| t!("backend-update-no-program"))?;
    let http = shared.youtube.http();
    let listed = http.get(sums).header("User-Agent", AGENT).send().await.map_err(|error| said(&error))?;
    let listed = listed.error_for_status().map_err(|error| said(&error))?.text().await.map_err(|error| said(&error))?;
    let name = program.rsplit('/').next().unwrap_or_default();
    let sum = update::sum_for(&listed, name).ok_or_else(|| t!("backend-update-no-sum"))?.to_owned();

    let fetched = http.get(program).header("User-Agent", AGENT).send().await.map_err(|error| said(&error))?;
    let mut fetched = fetched.error_for_status().map_err(|error| said(&error))?;
    let whole = fetched.content_length().filter(|whole| *whole > 0);
    let (mut bytes, mut told) = (Vec::new(), 0);
    while let Some(chunk) = fetched.chunk().await.map_err(|error| said(&error))? {
        bytes.extend_from_slice(&chunk);
        // A word for each hundredth, not for each piece.
        let hundredths = whole.map_or(0, |whole| bytes.len() as u64 * 100 / whole);
        if hundredths > told {
            told = hundredths;
            (shared.reply)(Event::UpdateProgress(hundredths as f32 / 100.0));
        }
    }
    if !update::matches(&bytes, &sum) {
        return Err(t!("backend-update-mismatch").into_owned());
    }
    let running = update::running().ok_or_else(|| t!("backend-update-no-location"))?;
    tokio::task::spawn_blocking(move || update::install(&bytes, running)).await.map_err(|error| said(&error))?.map_err(|error| t!("backend-update-replace-failed", error = error.to_string()).into_owned())
}

/// How far ahead of where the song is its other rendition is made ready, and how long the
/// two sound together when it comes in.
const HANDOVER_AHEAD: f64 = 2.5;
const HANDOVER_FADE: Duration = Duration::from_millis(250);

/// Hands what plays over to another rendition of it without a gap. The other is made ready
/// on the standby deck from a little ahead of where the song is, the song goes on until it
/// gets there, and then the other comes in under it.
async fn handover(shared: &Arc<Shared>, track: Track, at: f64) {
    let generation = shared.generation.load(Ordering::SeqCst);
    // A handover asked for after this one overrules it: going to the video and straight
    // back must end on the song, whichever of the two is ready first.
    let turn = shared.handovers.fetch_add(1, Ordering::SeqCst) + 1;
    let last = || shared.handovers.load(Ordering::SeqCst) == turn && shared.is_current(generation);
    let asked = std::time::Instant::now();
    let ready = async {
        let (mut load, download) = fetch(shared, &track, true).await.ok()?;
        load.start_sec = Some(at + HANDOVER_AHEAD);
        if !last() {
            download.cancel();
            return None;
        }
        let replaced = shared.downloads.lock().unwrap().standby.replace(download);
        if let Some(replaced) = replaced {
            replaced.cancel();
        }
        let engine = shared.clone();
        blocking(move || {
            #[cfg(target_os = "ios")]
            engine.remember(&load);
            engine.audio.load(load)
        }).await.ok()
    };
    if ready.await.is_some() {
        tokio::time::sleep(Duration::from_secs_f64(HANDOVER_AHEAD).saturating_sub(asked.elapsed())).await;
    }
    // Something else was asked to play meanwhile: this is no longer what follows.
    if last() {
        // Not ready, it is loaded the plain way from where the song has got to.
        begin(shared, track, HANDOVER_FADE, Some((at + asked.elapsed().as_secs_f64(), true)));
    }
}

/// A track that cannot be preloaded is loaded when its turn comes, and fails then if it must.
async fn preload(shared: &Arc<Shared>, track: &Track) {
    let generation = shared.generation.load(Ordering::SeqCst);
    let (engine, id) = (shared.clone(), track.id.clone());
    if blocking(move || engine.audio.has_standby(&id)).await != Ok(false) {
        return;
    }
    let Ok((load, download)) = fetch(shared, track, true).await else { return };
    // A `Play` in the meantime means the queue moved and this may not be what follows any more.
    if !shared.is_current(generation) {
        download.cancel();
        return;
    }
    let replaced = shared.downloads.lock().unwrap().standby.replace(download);
    if let Some(replaced) = replaced {
        replaced.cancel();
    }
    let engine = shared.clone();
    let _ = blocking(move || {
        #[cfg(target_os = "ios")]
        engine.remember(&load);
        engine.audio.load(load)
    }).await;
}

/// The rendition a clip is cut from, by its height, and how long the clip is.
const CLIP_HEIGHT: u64 = 360;
/// The lines of the video that fills the window.
const FILM_HEIGHT: u64 = 720;
const CLIP_SECONDS: f64 = 8.0;

/// A track's own video, as far as it was found.
#[derive(Clone, Debug, PartialEq)]
pub struct Footage {
    /// A stretch of it, to turn over where the cover was.
    pub clip: Clip,
    /// The whole of it, small for the panel and large for the window, each with the address
    /// it is fetched from.
    pub small: (Film, String),
    pub large: (Film, String),
    /// The video as a track: its sound takes over where it does not last what the song does.
    pub video: Track,
}

/// What is known of the track's own video: its header and its index in one request for
/// each size, then the fragments of a stretch of it. None when the track has no video of
/// its own, or none in H.264, the one kind this decodes.
async fn footage(shared: &Shared, track: &Track) -> Option<Footage> {
    let id = shared.youtube.video_for(&track.artist, &track.title).await.ok()??;
    let stream = shared.youtube.stream(&id).await.ok()?;
    let stream = &stream;
    let film = |height: u64| async move {
        // Of two renditions as tall, the one with fewer pictures a second: it is decoded on a core.
        let video = stream.video.iter().filter(|video| video.mime.contains("avc1")).min_by_key(|video| (video.height.abs_diff(height), video.fps))?;
        let head = shared.youtube.bytes(&video.url, 0, video.index.1).await.ok()?;
        let header = surco_video::header(head.get(..=usize::try_from(video.init.1).ok()?)?)?;
        let fragments = surco_video::fragments(head.get(usize::try_from(video.index.0).ok()?..)?, video.index.1);
        let (width, height) = (usize::try_from(video.width).ok()?, usize::try_from(video.height).ok()?);
        Some((Film { header, fragments, width, height }, video.url.clone()))
    };
    let (small, large) = tokio::join!(film(CLIP_HEIGHT), film(FILM_HEIGHT));
    let (small, large) = (small?, large?);
    let (from, to) = surco_video::stretch(&small.0.fragments, CLIP_SECONDS)?;
    let samples = surco_video::samples(&shared.youtube.bytes(&small.1, from, to).await.ok()?, &small.0.header);
    let video = Track { id, duration_ms: (stream.duration_seconds * 1000.0) as u64, ..track.clone() };
    (!samples.is_empty()).then(|| Footage { clip: Clip { parameters: small.0.header.parameters.clone(), samples }, small, large, video })
}

/// Where lyrics are translated.
// shortcut: the endpoint Chrome's dictionary uses, which takes no key and keeps the lines, one
// request a song in its address (lyrics up to some 6000 characters); move to a keyed service if
// it starts refusing.
const TRANSLATE: &str = "https://clients5.google.com/translate_a/t";

/// `lines` in the language `to`, each to its own line; none when they already are in it. Blank
/// lines stay blank and are not sent.
async fn translate(shared: &Shared, lines: &[String], to: &str) -> Result<Option<Vec<String>>, String> {
    let said: Vec<&str> = lines.iter().map(|line| line.trim()).filter(|line| !line.is_empty()).collect();
    if said.is_empty() {
        return Ok(None);
    }
    let query = [("client", "dict-chrome-ex"), ("sl", "auto"), ("tl", to), ("q", &said.join("\n"))];
    let response = shared.youtube.http().get(TRANSLATE).query(&query).send().await.map_err(|error| error.to_string())?;
    let answer: serde_json::Value = response.error_for_status().map_err(|error| error.to_string())?.json().await.map_err(|error| error.to_string())?;
    let (Some(text), from) = (answer[0][0].as_str(), answer[0][1].as_str().unwrap_or_default()) else { return Err(t!("backend-translation-unreadable").into_owned()) };
    if from.split('-').next() == to.split('-').next() {
        return Ok(None);
    }
    let mut translated = text.split('\n').map(str::trim);
    let all: Vec<String> = lines.iter().map(|line| if line.trim().is_empty() { Some(String::new()) } else { translated.next().map(str::to_owned) }).collect::<Option<_>>()
        .ok_or_else(|| t!("backend-translation-unreadable").into_owned())?;
    Ok(Some(all))
}

/// How many songs a playlist's recommendations hold, a few refreshes' worth.
const SUGGESTIONS: usize = 50;

/// How many tracks the mix has, how many seeds get a mix of their own, and how many
/// artists and albums are picked.
const MIX: usize = 20;
const BECAUSE: usize = 5;
const PICKED_ARTISTS: usize = 8;
const PICKED_ALBUMS: usize = 12;
/// How many of the artists found, and of the ones already played, lend their albums.
const ALBUM_ARTISTS: usize = 4;
/// How many of the artists played most get a shelf for their fans, and how many artists
/// like them lend it an album.
const FANS_OF: usize = 2;
const FAN_ARTISTS: usize = 8;
/// The most releases of the artist played most the home offers.
const MORE_OF: usize = 12;

/// The home page's picks, out of the radios of what the user plays. Each part is sent as
/// soon as it is ready, and a part the catalog fails is a shelf less.
async fn discover(shared: &Shared, taste: Taste) {
    let radios: Vec<Vec<Track>> = join_all(taste.seeds.iter().map(|seed| async { shared.youtube.radio(&seed.id).await.unwrap_or_default() })).await;
    let mix = discover::blend(&radios, &taste.tracks, MIX);
    // A seed's own mix goes further into its radio than the blend of them all did.
    let mut shown = taste.tracks.clone();
    shown.extend(mix.iter().map(|track| track.id.clone()));
    let own = |(seed, radio): (&Track, &Vec<Track>)| (seed.clone(), discover::blend(std::slice::from_ref(radio), &shown, MIX));
    let because = taste.seeds.iter().zip(&radios).take(BECAUSE).map(own).collect();
    (shared.reply)(Event::Discovered(Picks::Tracks { mix, because }));

    // Twice the names: a radio also names uploaders, who have no artist's page.
    let names = discover::newcomers(&radios, &taste.artists, PICKED_ARTISTS * 2);
    let found = join_all(names.iter().map(|name| shared.youtube.artist_named(name))).await;
    let artists: Vec<ArtistRef> = found.into_iter().filter_map(|artist| artist.ok().flatten()).take(PICKED_ARTISTS).collect();
    (shared.reply)(Event::Discovered(Picks::Artists(artists.clone())));

    // An album of each in turn: of the artists just found, then of the ones already played.
    let ids = artists.iter().map(|artist| &artist.id).take(ALBUM_ARTISTS).chain(taste.artist_ids.iter().take(ALBUM_ARTISTS));
    let pages: Vec<Vec<AlbumRef>> = join_all(ids.map(|id| async move { shared.youtube.albums_of(id).await.unwrap_or_default() })).await;
    let mut albums: Vec<AlbumRef> = Vec::new();
    for rank in 0..pages.iter().map(Vec::len).max().unwrap_or(0) {
        albums.extend(pages.iter().filter_map(|page| page.get(rank)).filter(|album| !taste.albums.contains(&album.id)).cloned());
    }
    albums.truncate(PICKED_ALBUMS);
    (shared.reply)(Event::Discovered(Picks::Albums(albums)));

    // Today's: what goes with the latest plays, past what the user has and the mix.
    let latest: Vec<Vec<Track>> = join_all(taste.recent.iter().map(|track| async { shared.youtube.radio(&track.id).await.unwrap_or_default() })).await;
    (shared.reply)(Event::Discovered(Picks::Recent(discover::blend(&latest, &shown, MIX))));

    // For the fans of each artist played most, as Spotify's "For fans of".
    let fans = join_all(taste.seeds.iter().zip(&radios).take(FANS_OF).map(|(seed, radio)| fans_of(shared, &taste, seed, radio))).await;
    (shared.reply)(Event::Discovered(Picks::FansOf(fans.into_iter().flatten().collect())));

    // More of the artist played most: their releases, the ones not kept yet.
    if let Some(seed) = taste.seeds.first() {
        if let Ok(Some(artist)) = shared.youtube.artist_named(&seed.artist).await {
            let mut releases = shared.youtube.albums_of(&artist.id).await.unwrap_or_default();
            releases.retain(|album| !taste.albums.contains(&album.id));
            releases.truncate(MORE_OF);
            if !releases.is_empty() {
                (shared.reply)(Event::Discovered(Picks::MoreOf(artist, releases)));
            }
        }
    }
}


/// For the fans of `seed`'s artist: an album of each of the artists its radio plays that the
/// user does not, none the user keeps. None when the artist or all of the albums are not found.
async fn fans_of(shared: &Shared, taste: &Taste, seed: &Track, radio: &Vec<Track>) -> Option<(ArtistRef, Vec<AlbumRef>)> {
    let artist = shared.youtube.artist_named(&seed.artist).await.ok().flatten()?;
    let names = discover::newcomers(std::slice::from_ref(radio), &taste.artists, FAN_ARTISTS);
    let found = join_all(names.iter().map(|name| shared.youtube.artist_named(name))).await;
    let ids: Vec<String> = found.into_iter().filter_map(|artist| artist.ok().flatten()).map(|artist| artist.id).collect();
    let pages = join_all(ids.iter().map(|id| async move { shared.youtube.albums_of(id).await.unwrap_or_default() })).await;
    let albums: Vec<AlbumRef> = pages.into_iter().filter_map(|page| page.into_iter().find(|album| !taste.albums.contains(&album.id))).collect();
    (!albums.is_empty()).then_some((artist, albums))
}

/// Downloads a track whole and keeps it, with its cover, in the library.
async fn save(shared: &Arc<Shared>, track: &Track) -> Result<(), String> {
    let stream = shared.youtube.stream(&track.id).await.map_err(|error| error.to_string())?;
    let best = stream.best().ok_or_else(|| t!("backend-no-audio"))?.clone();
    let buffer = MediaBuffer::new(usize::try_from(best.size).ok().filter(|size| *size > 0));
    download(shared.clone(), best.clone(), buffer.clone()).await;
    let audio = buffer.bytes().ok_or_else(|| t!("backend-download-cut"))?;
    let cover = match &track.artwork_url {
        Some(url) => cover_bytes(shared, url).await,
        None => None,
    };
    let (shared, track) = (shared.clone(), track.clone());
    blocking(move || {
        let mut library = shared.library.lock().unwrap();
        library.add(track, best.mime, best.loudness_db, &audio, cover.as_deref()).map_err(|error| t!("backend-save-failed", error = error.to_string()).into_owned())
    })
    .await
}

/// What brings a track down to the reference loudness. One under it stays as it is, as in
/// YouTube's own player: lifting it would need to know its peaks.
fn gain(loudness_db: Option<f64>) -> f32 {
    10f32.powf(-(loudness_db.unwrap_or(0.0).max(0.0) as f32) / 20.0)
}

/// A saved track as a load that needs no network.
async fn saved(shared: &Arc<Shared>, track: &Track, standby: bool) -> Option<(Load, Download)> {
    let (path, mime_type, loudness_db) = shared.library.lock().unwrap().audio(&track.id)?;
    // A file that went missing is not an error: the track is fetched like any other.
    let bytes = blocking(move || std::fs::read(path).map_err(|error| error.to_string())).await.ok()?;
    let buffer = MediaBuffer::new(Some(bytes.len()));
    buffer.append(&bytes);
    buffer.complete();
    let load = Load {
        track_id: track.id.clone(),
        buffer: buffer.clone(),
        mime_type,
        duration_sec: (track.duration_ms > 0).then_some(track.duration_ms as f64 / 1000.0),
        start_sec: None,
        standby,
        gain: gain(loudness_db),
    };
    Some((load, Download { track_id: track.id.clone(), task: tokio::spawn(async {}), buffer }))
}

/// Resolves the track and starts downloading it into the buffer the returned load reads from,
/// unless the library already holds it.
async fn fetch(shared: &Arc<Shared>, track: &Track, standby: bool) -> Result<(Load, Download), String> {
    if let Some(ready) = saved(shared, track, standby).await {
        return Ok(ready);
    }
    let stream = shared.youtube.stream(&track.id).await.map_err(|error| error.to_string())?;
    let best = stream.at_most(shared.quality.load(Ordering::SeqCst)).ok_or_else(|| t!("backend-no-audio"))?.clone();
    let buffer = MediaBuffer::new(usize::try_from(best.size).ok().filter(|size| *size > 0));
    let load = Load {
        track_id: track.id.clone(),
        buffer: buffer.clone(),
        mime_type: best.mime.clone(),
        duration_sec: (stream.duration_seconds > 0.0).then_some(stream.duration_seconds),
        start_sec: None,
        standby,
        gain: gain(best.loudness_db),
    };
    let task = tokio::spawn(download(shared.clone(), best, buffer.clone()));
    Ok((load, Download { track_id: track.id.clone(), task, buffer }))
}

/// Fills `buffer` with the stream. YouTube throttles a plain GET to a crawl, so every request
/// carries a `Range`, which is also how a broken connection resumes where it stopped.
async fn download(shared: Arc<Shared>, audio: Audio, buffer: MediaBuffer) {
    let mut received: u64 = 0;
    for _ in 0..DOWNLOAD_ATTEMPTS {
        let request = shared.youtube.http().get(&audio.url).header("Range", format!("bytes={received}-"));
        let Ok(mut response) = request.send().await else { continue };
        if !response.status().is_success() {
            break;
        }
        while let Ok(Some(chunk)) = response.chunk().await {
            received += chunk.len() as u64;
            buffer.append(&chunk);
        }
        if audio.size > 0 && received >= audio.size {
            buffer.complete();
            return;
        }
    }
    buffer.fail();
}

const COVERS_AT_ONCE: usize = 6;

/// The most artists the first run's picker is offered at once.
const PICKER_MOST: usize = 48;

/// How long another device has to answer, and the most an account from it may weigh.
const HANDOFF_WAIT: Duration = Duration::from_secs(15);
const HANDOFF_MOST: u64 = 64 * 1024 * 1024;
/// The pixels a square of the code takes.
const CODE_SCALE: u32 = 8;

/// The code a phone's camera reads an account's address from, as a picture limn draws.
fn code_image(link: &str) -> Option<RasterImage> {
    let png = handoff::code(link, CODE_SCALE)?;
    let digest: String = Sha256::digest(&png).iter().map(|byte| format!("{byte:02x}")).collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(&png);
    RasterImage::new(digest, RasterImageFormat::Png, u32::try_from(png.len()).ok()?, encoded.into(), t!("backend-handoff-code"))
}

/// An account served by another device: read, and its pictures kept with the covers. A
/// picture is taken only under a picture's own name, and only as the picture it decodes to.
async fn fetch_bundle(shared: &Shared, url: &str) -> Result<Bundle, String> {
    let unreachable = || t!("backend-handoff-unreachable").into_owned();
    let response = shared.youtube.http().get(url).timeout(HANDOFF_WAIT).send().await.map_err(|_| unreachable())?.error_for_status().map_err(|_| unreachable())?;
    if response.content_length().is_none_or(|length| length > HANDOFF_MOST) {
        return Err(t!("backend-handoff-unreadable").into_owned());
    }
    let mut bundle = handoff::read(&response.bytes().await.map_err(|_| unreachable())?)?;
    let pictures = std::mem::take(&mut bundle.pictures);
    tokio::task::block_in_place(|| {
        let mut library = shared.library.lock().unwrap();
        for (name, encoded) in pictures.into_iter().filter(|(name, _)| name.starts_with(PICTURE_NAME)) {
            let picture = base64::engine::general_purpose::STANDARD.decode(encoded).ok().and_then(|bytes| square_jpeg(&bytes));
            if let Some(jpeg) = picture { let _ = library.keep_cover(&name, &jpeg); }
        }
    });
    Ok(bundle)
}

/// The side a picture of the user's own is kept at.
const PICTURE_SIDE: u32 = 640;
/// How the name of a picture of the user's own begins.
const PICTURE_NAME: &str = "surco:picture:";

/// A picture from the user's files, through the system's own dialog. A phone has its own way.
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
async fn pick_picture() -> Option<Vec<u8>> {
    let file = rfd::AsyncFileDialog::new().add_filter(t!("backend-pictures"), &["jpg", "jpeg", "png", "webp"]).pick_file().await?;
    Some(file.read().await)
}

/// On a phone, from its photos, through the system's own picker.
#[cfg(target_os = "ios")]
async fn pick_picture() -> Option<Vec<u8>> {
    crate::ios_ui::pick_photo();
    loop {
        tokio::time::sleep(Duration::from_millis(250)).await;
        match crate::ios_ui::take_picked_photo() {
            crate::ios_ui::Picked::Open => continue,
            crate::ios_ui::Picked::Photo(photo) => return Some(photo),
            crate::ios_ui::Picked::Left => return None,
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos", target_os = "ios")))]
async fn pick_picture() -> Option<Vec<u8>> {
    None
}

/// The middle square of a picture, at most `PICTURE_SIDE` a side, as JPEG.
fn square_jpeg(bytes: &[u8]) -> Option<Vec<u8>> {
    let picture = image::load_from_memory(bytes).ok()?;
    let edge = picture.width().min(picture.height());
    let side = edge.min(PICTURE_SIDE);
    let square = picture
        .crop_imm((picture.width() - edge) / 2, (picture.height() - edge) / 2, edge, edge)
        .resize_exact(side, side, image::imageops::FilterType::Triangle)
        .to_rgb8();
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 88).encode_image(&square).ok()?;
    Some(jpeg)
}

/// A cover from the library when it is there, from the network otherwise, where the catalog's
/// own size would show blurred.
async fn cover_bytes(shared: &Shared, url: &str) -> Option<Vec<u8>> {
    // Reading the disk holds the library: off the runtime's two workers.
    let kept = tokio::task::block_in_place(|| shared.library.lock().unwrap().cover(url));
    match kept {
        Some(bytes) => Some(bytes),
        None => Some(shared.youtube.http().get(cover_at(url, COVER_SIDE)).send().await.ok()?.error_for_status().ok()?.bytes().await.ok()?.to_vec()),
    }
}

/// The color a cover lends its card: the mean of its pixels, the vivid ones counting more
/// than its blacks, whites and grays.
fn tint_of(bytes: &[u8]) -> Option<[u8; 3]> {
    let small = image::load_from_memory(bytes).ok()?.thumbnail(24, 24).to_rgb8();
    let (mut sum, mut weight) = ([0f32; 3], 0f32);
    for pixel in small.pixels() {
        let [red, green, blue] = pixel.0.map(f32::from);
        let vivid = 1.0 + (red.max(green).max(blue) - red.min(green).min(blue)) / 16.0;
        sum = [sum[0] + red * vivid, sum[1] + green * vivid, sum[2] + blue * vivid];
        weight += vivid;
    }
    (weight > 0.0).then(|| sum.map(|channel| (channel / weight).round() as u8))
}

async fn artwork(shared: &Shared, url: &str) -> Option<(RasterImage, Option<[u8; 3]>, Option<(u32, u32)>)> {
    let bytes = cover_bytes(shared, url).await?;
    let format = match bytes.get(..12)? {
        [0xFF, 0xD8, ..] => RasterImageFormat::Jpeg,
        [0x89, b'P', b'N', b'G', ..] => RasterImageFormat::Png,
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P'] => RasterImageFormat::Webp,
        [b'G', b'I', b'F', ..] => RasterImageFormat::Gif,
        _ => return None,
    };
    // Hashing, encoding and decoding for the tint are the CPU's, not the network's: they
    // leave the runtime's workers free for playback meanwhile.
    tokio::task::block_in_place(|| {
        let digest: String = Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect();
        let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let image = RasterImage::new(digest, format, u32::try_from(bytes.len()).ok()?, encoded.into(), t!("backend-cover"))?;

        // Read from its header alone: a release's picture is laid out at its own proportions.
        let size = image::ImageReader::new(std::io::Cursor::new(&bytes)).with_guessed_format().ok().and_then(|reader| reader.into_dimensions().ok());
        Some((image, tint_of(&bytes), size))
    })
}

#[cfg(test)]
mod tests {
    use super::tint_of;

    #[test]
    fn a_picture_chosen_is_kept_as_its_middle_square() {
        let mut png = Vec::new();
        image::RgbImage::from_fn(30, 20, |x, _| image::Rgb([if x < 5 { 255 } else { 0 }, 0, 0]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let kept = image::load_from_memory(&super::square_jpeg(&png).unwrap()).unwrap();
        assert_eq!((kept.width(), kept.height()), (20, 20), "no larger than it came");
        assert!(kept.to_rgb8().get_pixel(0, 10)[0] < 64, "the red edge was cut away");
        assert!(super::square_jpeg(b"not a picture").is_none());
    }

    #[test]
    fn an_unavailable_audio_engine_answers_device_requests_with_an_error() {
        use super::{handle, Command, Event, Player};
        let events = std::cell::RefCell::new(Vec::new());
        let reply = |event| events.borrow_mut().push(event);
        let mut player = Player::default();
        let engine = Err("No audio device".to_owned());
        handle(&engine, &reply, &mut player, Command::Outputs);
        handle(&engine, &reply, &mut player, Command::Output(None));
        let events = events.into_inner();
        assert!(matches!(&events[0], Event::OutputsFailed(message) if message == "No audio device"));
        assert!(matches!(&events[1], Event::OutputFailed(message) if message.contains("No audio device")));
    }

    /// A square cover whose first `red_columns` columns are red and the rest black.
    fn cover(side: u32, red_columns: u32) -> Vec<u8> {
        let raw: Vec<u8> = (0..side * side).flat_map(|pixel| if pixel % side < red_columns { [220, 20, 20] } else { [0, 0, 0] }).collect();
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::write_buffer_with_format(&mut bytes, &raw, side, side, image::ColorType::Rgb8, image::ImageFormat::Png).unwrap();
        bytes.into_inner()
    }

    #[test]
    fn a_cover_lends_its_vivid_color_more_than_its_grays() {
        assert_eq!(tint_of(&cover(48, 48)), Some([220, 20, 20]));
        let [red, green, blue] = tint_of(&cover(48, 12)).unwrap();
        assert!(red > 150 && green < 30 && blue < 30, "a quarter of red outweighs three of black: {red} {green} {blue}");
        assert_eq!(tint_of(b"not a picture"), None);
    }
}
