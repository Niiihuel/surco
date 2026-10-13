//! The contract between the window and everything that takes time: the catalog, the network
//! and the audio engine live on the other side of these two channels.

use dnm_core::Track;
use limn::RasterImage;

/// What the window asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Search(String),
    /// Start this track from the beginning, replacing whatever plays.
    Play(Track),
    Pause,
    Resume,
    /// Seconds from the start of the current track.
    Seek(f64),
    /// 0 to 1.
    Volume(f32),
    FetchArtwork(String),
}

/// What comes back. Every variant names what it answers, because answers arrive late and
/// out of order: a result for a query the user already replaced must be ignored.
#[derive(Clone, Debug)]
pub enum Event {
    SearchResults { query: String, tracks: Vec<Track> },
    SearchFailed { query: String, message: String },
    Artwork { url: String, image: RasterImage },
    Position { track_id: String, seconds: f64, duration: f64 },
    Ended { track_id: String },
    PlaybackFailed { track_id: String, message: String },
}

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use dnm_audio::{Load, MediaBuffer, NativeAudio};
use dnm_youtube::{Audio, YouTube};
use eframe::egui;
use limn::RasterImageFormat;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio::task::JoinHandle;

/// How many times a broken download resumes from where it stopped before the track fails.
const DOWNLOAD_ATTEMPTS: u32 = 4;

/// Everything a task needs, shared between them.
struct Shared {
    reply: Box<dyn Fn(Event) + Send + Sync>,
    youtube: YouTube,
    audio: NativeAudio,
    /// Bumped by every `Play`. A task that finds another number was overtaken and stops.
    generation: AtomicU64,
    volume: Mutex<f32>,
    download: Mutex<Option<JoinHandle<()>>>,
}

/// Starts the worker and returns its two ends. The worker wakes the window with
/// `request_repaint` whenever it has something to say, so the window can sleep.
pub fn spawn(ctx: egui::Context) -> (UnboundedSender<Command>, Receiver<Event>) {
    let (commands, mut inbox) = unbounded_channel::<Command>();
    let (outbox, events) = channel::<Event>();
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
                let shared = match start(reply.clone()).await {
                    Ok(shared) => Arc::new(shared),
                    Err(message) => {
                        // Nothing can work: answer every request with the reason.
                        while let Some(command) = inbox.recv().await {
                            match command {
                                Command::Search(query) => reply(Event::SearchFailed { query, message: message.clone() }),
                                Command::Play(track) => reply(Event::PlaybackFailed { track_id: track.id, message: message.clone() }),
                                _ => {}
                            }
                        }
                        return;
                    }
                };
                while let Some(command) = inbox.recv().await {
                    handle(&shared, command);
                }
            });
        })
        .expect("the backend thread starts");
    (commands, events)
}

async fn start(reply: impl Fn(Event) + Send + Sync + Clone + 'static) -> Result<Shared, String> {
    let youtube = YouTube::new().await.map_err(|error| format!("No se pudo conectar con el catálogo: {error}"))?;
    let on_event = {
        let reply = reply.clone();
        move |event: dnm_audio::Event| match event {
            dnm_audio::Event::Position { track_id, position_sec, duration_sec } => {
                reply(Event::Position { track_id, seconds: position_sec, duration: duration_sec.unwrap_or(0.0) })
            }
            dnm_audio::Event::Ended { track_id } => reply(Event::Ended { track_id }),
            dnm_audio::Event::Failed { track_id, message } => reply(Event::PlaybackFailed { track_id, message }),
            // Both decks are gone; the next load reopens the device.
            dnm_audio::Event::StreamFailed { .. } => {}
        }
    };
    let audio = NativeAudio::start(None, on_event).map_err(|error| format!("No se pudo abrir la salida de audio: {error}"))?;
    Ok(Shared {
        reply: Box::new(reply),
        youtube,
        audio,
        generation: AtomicU64::new(0),
        volume: Mutex::new(1.0),
        download: Mutex::new(None),
    })
}

fn handle(shared: &Arc<Shared>, command: Command) {
    match command {
        Command::Search(query) => {
            let shared = shared.clone();
            tokio::spawn(async move {
                let event = match shared.youtube.search(&query).await {
                    Ok(tracks) => Event::SearchResults { query, tracks },
                    Err(error) => Event::SearchFailed { query, message: error.to_string() },
                };
                (shared.reply)(event);
            });
        }
        Command::FetchArtwork(url) => {
            let shared = shared.clone();
            tokio::spawn(async move {
                // A cover that does not arrive leaves its placeholder; nothing to report.
                if let Some(image) = artwork(&shared, &url).await {
                    (shared.reply)(Event::Artwork { url, image });
                }
            });
        }
        Command::Play(track) => {
            let generation = shared.generation.fetch_add(1, Ordering::SeqCst) + 1;
            if let Some(previous) = shared.download.lock().unwrap().take() {
                previous.abort();
            }
            let shared = shared.clone();
            tokio::spawn(async move {
                if let Err(message) = play(&shared, &track, generation).await {
                    if shared.generation.load(Ordering::SeqCst) == generation {
                        (shared.reply)(Event::PlaybackFailed { track_id: track.id, message });
                    }
                }
            });
        }
        // The engine only fails these when its thread is gone, which the next `Play` reports.
        Command::Pause => drop(shared.audio.pause()),
        Command::Resume => drop(shared.audio.play()),
        Command::Seek(seconds) => drop(shared.audio.seek(seconds)),
        Command::Volume(volume) => {
            *shared.volume.lock().unwrap() = volume;
            drop(shared.audio.set_volume(volume, false));
        }
    }
}

async fn play(shared: &Arc<Shared>, track: &Track, generation: u64) -> Result<(), String> {
    let current = |shared: &Shared| shared.generation.load(Ordering::SeqCst) == generation;
    let stream = shared.youtube.stream(&track.id).await.map_err(|error| error.to_string())?;
    let best = stream.best().ok_or("YouTube no ofreció audio para este tema.")?.clone();
    if !current(shared) {
        return Ok(());
    }
    let buffer = MediaBuffer::new(usize::try_from(best.size).ok().filter(|size| *size > 0));
    let download = tokio::spawn(download(shared.clone(), best.clone(), buffer.clone()));
    *shared.download.lock().unwrap() = Some(download);

    let load = Load {
        track_id: track.id.clone(),
        buffer,
        mime_type: best.mime.clone(),
        duration_sec: (stream.duration_seconds > 0.0).then_some(stream.duration_seconds),
        start_sec: None,
        standby: false,
    };
    let engine = shared.clone();
    // `load` waits for the container header to arrive, so it cannot run on a runtime thread.
    tokio::task::spawn_blocking(move || {
        if engine.generation.load(Ordering::SeqCst) != generation {
            return Ok(());
        }
        engine.audio.load(load)?;
        if engine.generation.load(Ordering::SeqCst) != generation {
            return Ok(());
        }
        let volume = *engine.volume.lock().unwrap();
        engine.audio.set_volume(volume, false)?;
        engine.audio.play()
    })
    .await
    .map_err(|error| error.to_string())?
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
