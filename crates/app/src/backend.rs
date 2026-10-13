//! The contract between the window and everything that takes time: the catalog, the network,
//! the player and the audio engine live on the other side of these two channels.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use dnm_audio::equalizer::{self, EqualizerValues};
use dnm_audio::{Load, MediaBuffer, NativeAudio};
use dnm_core::{Album, Artist, Found, Track};
use dnm_youtube::{Audio, YouTube};
use eframe::egui;
use limn::{RasterImage, RasterImageFormat};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::player::{Effect, Player, PlayerView};

/// A page of the catalog.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Artist(String),
    /// A track that names its artist without linking to it.
    ArtistNamed(String),
    Album(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Content {
    Artist(Artist),
    Album(Album),
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
}

/// What comes back. Every answer names what it answers, because answers arrive late and out of
/// order: a result for a query or a page the user already left must be ignored.
#[derive(Clone, Debug)]
pub enum Event {
    SearchResults { query: String, found: Found },
    SearchFailed { query: String, message: String },
    Opened { target: Target, result: Result<Content, String> },
    Artwork { url: String, image: RasterImage },
}

/// How many times a broken download resumes from where it stopped before the track fails.
const DOWNLOAD_ATTEMPTS: u32 = 4;

/// What reaches the player from the worker's own side.
enum Input {
    Engine(dnm_audio::Event),
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
    downloads: Mutex<Downloads>,
    /// Held while a task changes what the active deck holds, so the newest `Play` has the last word.
    decks: tokio::sync::Mutex<()>,
    /// The way back into the player.
    inputs: UnboundedSender<Input>,
}

impl Shared {
    fn is_current(&self, generation: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == generation
    }
}

/// Starts the worker and returns its ends. The worker wakes the window with `request_repaint`
/// whenever it has something to say, so the window can sleep.
///
/// The player is a value to read, not a stream of events: a hidden window stops repainting,
/// and positions would pile up for as long as it stays hidden.
pub fn spawn(ctx: egui::Context) -> (UnboundedSender<Command>, Receiver<Event>, watch::Receiver<PlayerView>) {
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
                let shared = start(reply.clone(), inputs.clone()).await.map(Arc::new);
                let mut player = Player::default();
                loop {
                    let effects = tokio::select! {
                        command = inbox.recv() => match command {
                            Some(command) => handle(&shared, &reply, &mut player, command),
                            None => return,
                        },
                        Some(input) = inbox_own.recv() => match input {
                            Input::Engine(event) => player.engine(event),
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

async fn start(
    reply: impl Fn(Event) + Send + Sync + 'static,
    inputs: UnboundedSender<Input>,
) -> Result<Shared, String> {
    let youtube = YouTube::new().map_err(|error| error.to_string())?;
    let events = inputs.clone();
    let audio = NativeAudio::start(None, move |event| drop(events.send(Input::Engine(event))))
        .map_err(|error| format!("No se pudo abrir la salida de audio: {error}"))?;
    Ok(Shared {
        reply: Box::new(reply),
        youtube,
        audio,
        generation: AtomicU64::new(0),
        downloads: Mutex::default(),
        decks: tokio::sync::Mutex::default(),
        inputs,
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
                if let Some(image) = artwork(&shared, &url).await {
                    (shared.reply)(Event::Artwork { url, image });
                }
            });
        }
        // The slider is evenly spaced to the ear, not to the amplitude: half way is far quieter
        // than half the samples. The engine only fails this when its thread is gone, which the
        // next `Play` reports.
        (Command::Volume(volume), Ok(shared)) => drop(shared.audio.set_volume(volume.powf(2.5), false)),
        (Command::FetchArtwork(_) | Command::Volume(_), Err(_)) => {}
        (Command::Equalizer(values), _) => equalizer::set_values(values),
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
    };
    content.map_err(|error| error.to_string())
}

fn run(shared: &Arc<Shared>, effect: Effect) {
    match effect {
        Effect::Play(track) => {
            let generation = shared.generation.fetch_add(1, Ordering::SeqCst) + 1;
            // The old track stops now, not when the new one has loaded.
            drop(shared.audio.pause());
            {
                let mut downloads = shared.downloads.lock().unwrap();
                if let Some(replaced) = downloads.active.take() {
                    replaced.cancel();
                }
                if downloads.standby.as_ref().is_some_and(|download| download.track_id == track.id) {
                    downloads.active = downloads.standby.take();
                }
            }
            let shared = shared.clone();
            tokio::spawn(async move {
                if let Err(message) = play(&shared, &track, generation).await {
                    if shared.is_current(generation) {
                        let failed = dnm_audio::Event::Failed { track_id: track.id, message };
                        let _ = shared.inputs.send(Input::Engine(failed));
                    }
                }
            });
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
    }
}

/// The engine answers these on its own thread, and `load` waits for the container header to
/// arrive, so neither can run on a runtime thread.
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tokio::task::spawn_blocking(work).await.map_err(|error| error.to_string())?
}

async fn play(shared: &Arc<Shared>, track: &Track, generation: u64) -> Result<(), String> {
    {
        let _decks = shared.decks.lock().await;
        if !shared.is_current(generation) {
            return Ok(());
        }
        let (engine, id) = (shared.clone(), track.id.clone());
        // A track that was preloaded starts in one step.
        if blocking(move || engine.audio.transition(&id, Duration::ZERO)).await? {
            return Ok(());
        }
    }
    let (load, download) = fetch(shared, track, false).await?;
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

/// Resolves the track and starts downloading it into the buffer the returned load reads from.
async fn fetch(shared: &Arc<Shared>, track: &Track, standby: bool) -> Result<(Load, Download), String> {
    let stream = shared.youtube.stream(&track.id).await.map_err(|error| error.to_string())?;
    let best = stream.best().ok_or("YouTube no ofreció audio para este tema.")?.clone();
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

async fn artwork(shared: &Shared, url: &str) -> Option<RasterImage> {
    let bytes = shared.youtube.http().get(url).send().await.ok()?.error_for_status().ok()?.bytes().await.ok()?;
    let format = match bytes.get(..12)? {
        [0xFF, 0xD8, ..] => RasterImageFormat::Jpeg,
        [0x89, b'P', b'N', b'G', ..] => RasterImageFormat::Png,
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P'] => RasterImageFormat::Webp,
        [b'G', b'I', b'F', ..] => RasterImageFormat::Gif,
        _ => return None,
    };
    let digest: String = Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    RasterImage::new(digest, format, u32::try_from(bytes.len()).ok()?, encoded.into(), "Portada")
}
