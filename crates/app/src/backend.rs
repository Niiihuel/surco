//! The contract between the window and everything that takes time: the catalog, the network,
//! the player and the audio engine live on the other side of these two channels.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use dnm_audio::equalizer::{self, EqualizerValues};
use dnm_audio::{AudioOutputDevice, Load, MediaBuffer, NativeAudio};
use dnm_core::discover;
use dnm_core::lyrics::{parse_lrc, Line};
use dnm_video::{Clip, Film};
use futures_util::future::join_all;
use dnm_core::{Album, AlbumRef, Artist, ArtistRef, Found, Track};
use dnm_youtube::{Audio, YouTube};
use eframe::egui;
use limn::{RasterImage, RasterImageFormat};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::library::Library;
use crate::media_keys::{Key, MediaKeys};
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
}

#[derive(Clone, Debug, PartialEq)]
pub enum Content {
    Artist(Artist),
    Album(Album),
    Song(Track),
    List { name: String, tracks: Vec<Track> },
}

/// What the window asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Search(String),
    Open(Target),
    FetchArtwork(String),
    /// Replace the queue with these tracks and play the one at `index`.
    PlayList { tracks: Vec<Track>, index: usize },
    PlayPause,
    Next,
    /// Play the n-th of the upcoming tracks, skipping the ones before it.
    Jump(usize),
    /// Queue a track by hand: right after the current one, or after the ones already queued.
    PlayNext(Track),
    Enqueue(Track),
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
    /// Ask the catalog for the home page's picks.
    Discover(Taste),
    /// Look for a track's own video and fetch a few seconds of it.
    Clip(Track),
    /// Go on from where the track that plays is with another rendition of it in its place.
    Swap(Track),
    /// Ask who the artist of a track is.
    About(Track),
    /// Keep a track on disk to play it without a network, or stop keeping it.
    Save(Track),
    Unsave(String),
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
}

/// What comes back. Every answer names what it answers, because answers arrive late and out of
/// order: a result for a query or a page the user already left must be ignored.
#[derive(Clone, Debug)]
pub enum Event {
    SearchResults { query: String, found: Found },
    SearchFailed { query: String, message: String },
    Opened { target: Target, result: Result<Content, String> },
    /// What is kept on disk, newest first, the ids on their way there, and the room it takes.
    Library { saved: Vec<Track>, saving: HashSet<String>, bytes: u64 },
    SaveFailed { title: String, message: String },
    /// A cover, and the color it lends to a card of its track when it could be read.
    Artwork { url: String, image: RasterImage, tint: Option<[u8; 3]> },
    Outputs(Vec<AudioOutputDevice>),
    /// A track's lines and when each starts, or none when nobody has synced them.
    Lyrics { track_id: String, lines: Option<Vec<Line>> },
    /// A track's own video, or none when it has no such video.
    Clip { track_id: String, footage: Option<Arc<Footage>> },
    /// A part of the home page's picks.
    Discovered(Picks),
    /// Who the artist a track names is, or nobody when the catalog knows no artist of
    /// that name.
    About { artist: String, found: Option<Known> },
    /// The device that was asked for is gone: the system's own plays instead.
    OutputLost,
}

/// How many times a broken download resumes from where it stopped before the track fails.
const DOWNLOAD_ATTEMPTS: u32 = 4;

/// What reaches the player from the worker's own side.
enum Input {
    Engine(dnm_audio::Event),
    Key(Key),
    Radio { seed: String, tracks: Vec<Track> },
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
    audio: NativeAudio,
    /// Bumped by every `Effect::Play`. A task that finds another number was overtaken and stops.
    generation: AtomicU64,
    /// The most bits per second a stream is fetched at.
    quality: AtomicU64,
    downloads: Mutex<Downloads>,
    /// Held while a task changes what the active deck holds, so the newest `Play` has the last word.
    decks: tokio::sync::Mutex<()>,
    /// The way back into the player.
    inputs: UnboundedSender<Input>,
    library: Mutex<Library>,
    saving: Mutex<HashSet<String>>,
}

impl Shared {
    fn is_current(&self, generation: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == generation
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

/// Starts the worker and returns its ends. The worker wakes the window with `request_repaint`
/// whenever it has something to say, so the window can sleep.
///
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
        ctx.request_repaint();
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
                let mut keys = MediaKeys::start(move |key| drop(pressed.send(Input::Key(key))));
                loop {
                    let effects = tokio::select! {
                        command = inbox.recv() => match command {
                            Some(command) => handle(&shared, &reply, &mut player, command),
                            None => return,
                        },
                        Some(input) = inbox_own.recv() => match input {
                            Input::Engine(event) => player.engine(event),
                            Input::Key(key) => key.command(&player.view()).map(|command| player.command(command)).unwrap_or_default(),
                            Input::Radio { seed, tracks } => {
                                player.radio(&seed, tracks);
                                Vec::new()
                            }
                        },
                    };
                    for effect in effects {
                        match (&shared, effect) {
                            (Ok(shared), effect) => run(shared, effect),
                            // Nothing can play: the track fails with the reason the backend did not start.
                            (Err(message), Effect::Play(track)) => {
                                let failed = dnm_audio::Event::Failed { track_id: track.id, message: message.clone() };
                                let _ = inputs.send(Input::Engine(failed));
                            }
                            _ => {}
                        }
                    }
                    let view = player.view();
                    keys.show(&view);
                    let changed = views.send_if_modified(|shown| {
                        let changed = *shown != view;
                        *shown = view;
                        changed
                    });
                    if changed {
                        window.request_repaint();
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
        .map_err(|error| format!("No se pudo abrir la salida de audio: {error}"))?;
    Ok(Shared {
        reply: Box::new(reply),
        youtube,
        audio,
        generation: AtomicU64::new(0),
        quality: AtomicU64::new(u64::MAX),
        downloads: Mutex::default(),
        decks: tokio::sync::Mutex::default(),
        inputs,
        library: Mutex::new(Library::open(library)),
        saving: Mutex::default(),
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
        (Command::FetchArtwork(url), Ok(shared)) => {
            let shared = shared.clone();
            tokio::spawn(async move {
                // A cover that does not arrive leaves its placeholder; nothing to report.
                if let Some((image, tint)) = artwork(&shared, &url).await {
                    (shared.reply)(Event::Artwork { url, image, tint });
                }
            });
        }
        // The slider is evenly spaced to the ear, not to the amplitude: half way is far quieter
        // than half the samples. The engine only fails this when its thread is gone, which the
        // next `Play` reports.
        (Command::Volume(volume), Ok(shared)) => drop(shared.audio.set_volume(volume.powf(2.5), false)),
        (Command::FetchArtwork(_) | Command::Volume(_), Err(_)) => {}
        (Command::Equalizer(values), _) => equalizer::set_values(values),
        (Command::Quality(bps), Ok(shared)) => shared.quality.store(bps, Ordering::SeqCst),
        (Command::Quality(_), Err(_)) => {}
        (Command::Output(id), Ok(shared)) => {
            // The engine reopens its stream and both decks go with the old one. A device
            // that is gone gives way to the system's.
            if shared.audio.set_output_device(id.clone()).is_err() && id.is_some() {
                let _ = shared.audio.set_output_device(None);
                reply(Event::OutputLost);
            }
            return player.reload();
        }
        (Command::Outputs, Ok(shared)) => {
            let shared = shared.clone();
            tokio::spawn(async move {
                // Listing may ask the sound server, which takes a moment.
                if let Ok(devices) = blocking(dnm_audio::list_output_devices).await {
                    (shared.reply)(Event::Outputs(devices));
                }
            });
        }
        (Command::Output(_) | Command::Outputs, Err(_)) => {}
        (Command::Lyrics(track), Ok(shared)) => {
            let shared = shared.clone();
            tokio::spawn(async move {
                let lines = lyrics(&shared, &track).await;
                (shared.reply)(Event::Lyrics { track_id: track.id, lines });
            });
        }
        (Command::Lyrics(track), Err(_)) => reply(Event::Lyrics { track_id: track.id, lines: None }),
        (Command::Discover(taste), Ok(shared)) => {
            let shared = shared.clone();
            tokio::spawn(async move { discover(&shared, taste).await });
        }
        (Command::Discover(_), Err(_)) => {}
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
            Ok(None) => return Err(format!("No encontramos a {name} en el catálogo.")),
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
                return Err("No pudimos traer ninguna de las canciones de esa playlist.".to_owned());
            }
            Ok(Content::List { name: name.clone(), tracks })
        }
    };
    content.map_err(|error| error.to_string())
}

fn run(shared: &Arc<Shared>, effect: Effect) {
    match effect {
        Effect::Play(track) => begin(shared, track, Duration::ZERO, None),
        Effect::Fade(track, seconds) => begin(shared, track, Duration::from_secs_f64(seconds), None),
        Effect::Restore(track, at, playing) => begin(shared, track, Duration::ZERO, Some((at, playing))),
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
    {
        let mut downloads = shared.downloads.lock().unwrap();
        // The track that fades out still reads its download: cancelling it would end it there.
        if let Some(replaced) = downloads.active.take().filter(|_| fade.is_zero()) {
            replaced.cancel();
        }
        if downloads.standby.as_ref().is_some_and(|download| download.track_id == track.id) {
            downloads.active = downloads.standby.take();
        }
    }
    let shared = shared.clone();
    tokio::spawn(async move {
        if let Err(message) = play(&shared, &track, generation, fade, resume).await {
            if shared.is_current(generation) {
                let failed = dnm_audio::Event::Failed { track_id: track.id, message };
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
        engine.audio.load(load)?;
        // What was paused comes back paused.
        if resume.is_some_and(|(_, playing)| !playing) {
            return Ok(());
        }
        engine.audio.play()
    })
    .await
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
    let _ = blocking(move || engine.audio.load(load)).await;
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
        let header = dnm_video::header(head.get(..=usize::try_from(video.init.1).ok()?)?)?;
        let fragments = dnm_video::fragments(head.get(usize::try_from(video.index.0).ok()?..)?, video.index.1);
        let (width, height) = (usize::try_from(video.width).ok()?, usize::try_from(video.height).ok()?);
        Some((Film { header, fragments, width, height }, video.url.clone()))
    };
    let (small, large) = tokio::join!(film(CLIP_HEIGHT), film(FILM_HEIGHT));
    let (small, large) = (small?, large?);
    let (from, to) = dnm_video::stretch(&small.0.fragments, CLIP_SECONDS)?;
    let samples = dnm_video::samples(&shared.youtube.bytes(&small.1, from, to).await.ok()?, &small.0.header);
    let video = Track { id, duration_ms: (stream.duration_seconds * 1000.0) as u64, ..track.clone() };
    (!samples.is_empty()).then(|| Footage { clip: Clip { parameters: small.0.header.parameters.clone(), samples }, small, large, video })
}

/// Synced lyrics from LRCLIB. It is searched, not asked for the exact track, because an
/// exact lookup misses on any difference in length; the entry closest in length wins.
/// How many tracks the mix has, how many seeds get a shelf of their own, and how many
/// artists and albums are picked.
const MIX: usize = 20;
const BECAUSE: usize = 2;
const PICKED_ARTISTS: usize = 8;
const PICKED_ALBUMS: usize = 12;
/// How many of the artists found, and of the ones already played, lend their albums.
const ALBUM_ARTISTS: usize = 4;

/// The home page's picks, out of the radios of what the user plays. Each part is sent as
/// soon as it is ready, and a part the catalog fails is a shelf less.
async fn discover(shared: &Shared, taste: Taste) {
    let radios: Vec<Vec<Track>> = join_all(taste.seeds.iter().map(|seed| async { shared.youtube.radio(&seed.id).await.unwrap_or_default() })).await;
    let mix = discover::blend(&radios, &taste.tracks, MIX);
    // A seed's own shelf goes further into its radio than the mix did.
    let mut shown = taste.tracks.clone();
    shown.extend(mix.iter().map(|track| track.id.clone()));
    let own = |(seed, radio): (&Track, &Vec<Track>)| (seed.clone(), discover::blend(std::slice::from_ref(radio), &shown, crate::state::SHELF));
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
}

async fn lyrics(shared: &Shared, track: &Track) -> Option<Vec<Line>> {
    let request = shared.youtube.http().get("https://lrclib.net/api/search").query(&[("artist_name", &track.artist), ("track_name", &track.title)]);
    let body = request.send().await.ok()?.error_for_status().ok()?.bytes().await.ok()?;
    let found: Vec<serde_json::Value> = serde_json::from_slice(&body).ok()?;
    let seconds = track.duration_ms as f64 / 1000.0;
    let apart = |entry: &serde_json::Value| (entry["duration"].as_f64().unwrap_or(0.0) - seconds).abs();
    let best = found.iter().filter(|entry| entry["syncedLyrics"].is_string()).min_by(|a, b| apart(a).total_cmp(&apart(b)))?;
    let lines = parse_lrc(best["syncedLyrics"].as_str()?);
    (!lines.is_empty()).then_some(lines)
}

const NO_AUDIO: &str = "YouTube no ofreció audio para este tema.";

/// Downloads a track whole and keeps it, with its cover, in the library.
async fn save(shared: &Arc<Shared>, track: &Track) -> Result<(), String> {
    let stream = shared.youtube.stream(&track.id).await.map_err(|error| error.to_string())?;
    let best = stream.best().ok_or(NO_AUDIO)?.clone();
    let buffer = MediaBuffer::new(usize::try_from(best.size).ok().filter(|size| *size > 0));
    download(shared.clone(), best.clone(), buffer.clone()).await;
    let audio = buffer.bytes().ok_or("La descarga se cortó antes de terminar.")?;
    let cover = match &track.artwork_url {
        Some(url) => cover_bytes(shared, url).await,
        None => None,
    };
    let (shared, track) = (shared.clone(), track.clone());
    blocking(move || {
        let mut library = shared.library.lock().unwrap();
        library.add(track, best.mime, &audio, cover.as_deref()).map_err(|error| format!("No se pudo guardar en el disco: {error}"))
    })
    .await
}

/// A saved track as a load that needs no network.
async fn saved(shared: &Arc<Shared>, track: &Track, standby: bool) -> Option<(Load, Download)> {
    let (path, mime_type) = shared.library.lock().unwrap().audio(&track.id)?;
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
    let best = stream.at_most(shared.quality.load(Ordering::SeqCst)).ok_or(NO_AUDIO)?.clone();
    let buffer = MediaBuffer::new(usize::try_from(best.size).ok().filter(|size| *size > 0));
    let load = Load {
        track_id: track.id.clone(),
        buffer: buffer.clone(),
        mime_type: best.mime.clone(),
        duration_sec: (stream.duration_seconds > 0.0).then_some(stream.duration_seconds),
        start_sec: None,
        standby,
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

/// A cover from the library when it is there, from the network otherwise.
async fn cover_bytes(shared: &Shared, url: &str) -> Option<Vec<u8>> {
    let kept = shared.library.lock().unwrap().cover(url);
    match kept {
        Some(bytes) => Some(bytes),
        None => Some(shared.youtube.http().get(url).send().await.ok()?.error_for_status().ok()?.bytes().await.ok()?.to_vec()),
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

async fn artwork(shared: &Shared, url: &str) -> Option<(RasterImage, Option<[u8; 3]>)> {
    let bytes = cover_bytes(shared, url).await?;
    let format = match bytes.get(..12)? {
        [0xFF, 0xD8, ..] => RasterImageFormat::Jpeg,
        [0x89, b'P', b'N', b'G', ..] => RasterImageFormat::Png,
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P'] => RasterImageFormat::Webp,
        [b'G', b'I', b'F', ..] => RasterImageFormat::Gif,
        _ => return None,
    };
    let digest: String = Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let image = RasterImage::new(digest, format, u32::try_from(bytes.len()).ok()?, encoded.into(), "Portada")?;
    Some((image, tint_of(&bytes)))
}

#[cfg(test)]
mod tests {
    use super::tint_of;

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
