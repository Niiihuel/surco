// SPDX-License-Identifier: Apache-2.0
//
// Adapted from noFAYZ/zuno at commit 1fa9a9669e4a, src-tauri/src/audio.rs (plus the
// `native_audio_*` wrappers of src-tauri/src/lib.rs), licensed under Apache-2.0.
// Changed: no Tauri — the handle owns the thread and events go to a callback; every source is a
// `MediaBuffer`, decoded here; a start position, `Failed`/`StreamFailed` events and a shutdown
// on drop were added; our PipeWire stream is found by the running executable's name; the ALSA
// device filter sees past cpal's host prefix; the end of a track is polled for in its last tick.

/*!
 * Native decode and output.
 *
 * symphonia decodes, cpal plays, rodio wires the two together and resamples to whatever the
 * output device wants.
 *
 * Two sinks stand in for two decks. The next track is decoded and appended to the standby sink
 * while the current one is still playing, so a handover is a volume ramp between two live sinks
 * rather than a stop followed by a load. That is what makes gapless and crossfade work.
 *
 * Everything runs on one owned thread behind a channel. cpal's stream handle is not `Send` on
 * every backend, so nothing but the `Sender` leaves this module. The same thread runs the
 * position tick out of its own `recv_timeout`, rather than a second thread contending for the
 * same state.
 */

use std::io::{self, Read, Seek, SeekFrom};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use std::time::{Duration, Instant};

use rodio::cpal::{Error as StreamError, ErrorKind};
use rodio::stream::{DeviceSinkBuilder, MixerDeviceSink};
use rodio::{DeviceTrait, Player, Source};
use symphonia::core::io::MediaSource;

use crate::equalizer::EqualizedSource;
use crate::opus_source::{self, OpusSource};
use crate::{warn, MediaBuffer};

/// How often position is reported and the end of a track is checked for. A slower feed would
/// make a progress bar visibly step.
pub const TICK: Duration = Duration::from_millis(250);

/// Ramp granularity during a crossfade. 50 steps a second is well below what the ear resolves
/// as stepping, and the thread is otherwise idle.
const FADE_STEP: Duration = Duration::from_millis(20);

/// How often the end of a track is looked for once it is within a tick of it.
const END_POLL: Duration = Duration::from_millis(5);

/// How long a decoder read waits for bytes that have not landed yet before giving up.
const READ_TIMEOUT: Duration = Duration::from_secs(20);
/// Poll interval while waiting on the download. Short enough to be inaudible.
const READ_POLL: Duration = Duration::from_millis(10);

/// A stream that dies sooner than this after opening is a device that will not come back by
/// reopening it, so loads refuse for this long instead of walking the queue through it.
const STREAM_RETRY_WINDOW: Duration = Duration::from_secs(10);

const STREAM_LOST: &str = "the audio output stream failed";

/// A decoded stream ready to be handed to a deck. rodio resamples it to the device's rate.
type BoxedSource = Box<dyn Source + Send>;

/// Odd epochs accept loads. Releasing an output invalidates readers already waiting on bytes.
#[derive(Clone)]
struct OutputEpoch(Arc<AtomicU64>);

impl OutputEpoch {
    fn new(allowed: bool) -> Self { Self(Arc::new(AtomicU64::new(u64::from(allowed)))) }

    fn current(&self) -> Option<u64> {
        let epoch = self.0.load(Ordering::SeqCst);
        (epoch & 1 != 0).then_some(epoch)
    }

    fn accepts(&self, epoch: u64) -> bool { self.current() == Some(epoch) }

    fn prepare(&self) {
        let _ = self.0.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |epoch| Some(epoch | 1));
    }

    fn release(&self) {
        let _ = self.0.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |epoch| Some(if epoch & 1 != 0 { epoch.wrapping_add(1) } else { epoch }));
    }
}

/// What the engine reports, on its own thread, to the callback given to `NativeAudio::start`.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// Every `TICK` while a track plays. `duration_sec` is `None` when neither the container
    /// nor the caller knew it.
    Position { track_id: String, position_sec: f64, duration_sec: Option<f64> },
    Ended { track_id: String },
    /// The track left its deck before its end: its download failed, or the output stream died
    /// under it. Loading it again at the last reported position is the recovery.
    Failed { track_id: String, message: String },
    /// The output stream died and both decks went with it. The next load reopens the device.
    StreamFailed { message: String },
    /// The output moved to another device — the system's default changed, or the one playing
    /// went away — and both decks went with it. Loading the track again where it was resumes it.
    Rerouted,
}

/// A track to decode onto a deck.
pub struct Load {
    /// Chosen by the caller. Events name the track by it, and so does `transition`.
    pub track_id: String,
    pub buffer: MediaBuffer,
    /// What the body was declared as. Only a hint: a container recognised from its first bytes
    /// wins.
    pub mime_type: String,
    /// The provider's own duration, used when the container does not declare one — Opus in
    /// WebM often does not.
    pub duration_sec: Option<f64>,
    /// Where playback starts, `None` for the beginning.
    pub start_sec: Option<f64>,
    /// Decode onto the idle deck instead of the playing one, for `transition` to swap to.
    pub standby: bool,
    /// What its samples are multiplied by while tracks are normalized; 1 for as it comes.
    pub gain: f32,
}

/**
 * A `Read + Seek` view over a body that is still downloading.
 *
 * This is what lets the first sample play before the last byte arrives: the decoder reads the
 * container header out of the head of the body while the rest is still in flight. A read never
 * sees a hole — it waits for one to fill.
 *
 * ponytail: the wait happens on rodio's mixing thread, so a connection that falls behind
 * playback is an audible stall rather than a dropped track. Move decoding onto its own thread
 * behind a bounded channel if that ever shows up in practice.
 */
struct BufferReader {
    buffer: MediaBuffer,
    position: usize,
    output: Option<(OutputEpoch, u64)>,
}

impl BufferReader {
    fn new(buffer: MediaBuffer) -> Self {
        Self { buffer, position: 0, output: None }
    }
}

impl Read for BufferReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }

        let deadline = Instant::now() + READ_TIMEOUT;
        loop {
            if self.output.as_ref().is_some_and(|(output, epoch)| !output.accepts(*epoch)) {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "audio output was released"));
            }
            if let Some(read) = self.buffer.read_at(self.position, out) {
                self.position += read;
                return Ok(read);
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "timed out waiting for audio bytes",
                ));
            }
            std::thread::sleep(READ_POLL);
        }
    }
}

/**
 * Seekable and self-describing, so symphonia can probe a container it is still downloading.
 *
 * `byte_len` is the declared length rather than what has arrived — the demuxer uses it to
 * locate the tail of a Matroska file, and answering with the current fill would point it at the
 * middle of the download. A body of undeclared length is forward-only instead: the Matroska and
 * MP4 demuxers refuse a seekable source that cannot say how long it is.
 */
impl MediaSource for BufferReader {
    fn is_seekable(&self) -> bool {
        self.buffer.total().is_some()
    }

    fn byte_len(&self) -> Option<u64> {
        self.buffer.total().map(|total| total as u64)
    }
}

impl Seek for BufferReader {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let total = self.buffer.total();
        let target = match from {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::Current(delta) => self.position as i64 + delta,
            SeekFrom::End(delta) => match total {
                Some(total) => total as i64 + delta,
                None => {
                    return Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "the length of the audio body is not known yet",
                    ))
                }
            },
        };
        if target < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek before the start of the audio body",
            ));
        }
        /*
         * Seeking past what has arrived is allowed and does not wait. The next `read` is where
         * the wait belongs — symphonia probes by seeking around the container, and blocking on
         * a seek it may never read from would stall a load for no reason.
         */
        self.position = total.map_or(target as usize, |total| (target as usize).min(total));
        Ok(self.position as u64)
    }
}

/**
 * Puts the equaliser and a limiter between a decoded stream and its deck.
 *
 * The limiter is not decoration. Ten bands boosted 12 dB is far more headroom than a normalised
 * track has, and without it the sum clips into distortion that sounds like a broken decoder
 * rather than like an equaliser set too high. rodio ships one, so this is a line.
 *
 * The duration is read *before* wrapping: it is what the caller reports to the UI, and reading
 * it through two adapters is one more place for it to go missing.
 */
fn equalized<S>(source: S, gain: f32) -> (BoxedSource, Option<f64>)
where
    S: Source + Send + 'static,
{
    let duration = source.total_duration().map(|value| value.as_secs_f64());
    let shaped = EqualizedSource::new(source, gain).limit(rodio::source::LimitSettings::new());
    (Box::new(shaped), duration)
}

struct Deck {
    sink: Player,
    track_id: Option<String>,
    duration_sec: Option<f64>,
    /**
     * The deck's window onto its own download.
     *
     * A deck is loaded as soon as the decoder can read the container header, while the rest of
     * the file is still arriving. When that download fails there is nothing about the deck
     * itself that looks wrong: it holds a real source with a real duration, and it will play
     * the seconds it has and then simply stop.
     */
    buffer: Option<MediaBuffer>,
}

impl Deck {
    fn disconnected() -> Self {
        let (sink, _) = Player::new();
        sink.pause();
        Self { sink, track_id: None, duration_sec: None, buffer: None }
    }

    fn clear(&mut self) {
        self.sink.stop();
        self.track_id = None;
        self.duration_sec = None;
        self.buffer = None;
    }

    fn is_healthy(&self) -> bool {
        self.buffer.as_ref().is_none_or(|buffer| !buffer.is_failed())
    }

    fn seek(&self, seconds: f64) {
        // `from_secs_f64` panics on a value it cannot represent, and that would take the whole
        // audio thread down over a bad number from the caller.
        let result = Duration::try_from_secs_f64(seconds.max(0.0))
            .map_err(|error| error.to_string())
            .and_then(|target| self.sink.try_seek(target).map_err(|error| error.to_string()));
        if let Err(error) = result {
            warn(format_args!("seek failed: {error}"));
        }
    }
}

/// An in-progress crossfade. Held so the tick can advance it a step at a time instead of the
/// command loop sleeping through it and starving position events.
struct Fade {
    started_at: Instant,
    duration: Duration,
    /// The deck being faded out — already the standby by the time this exists.
    outgoing: usize,
}

enum Command {
    Load {
        output_epoch: u64,
        track_id: String,
        source: BoxedSource,
        duration_sec: Option<f64>,
        start_sec: Option<f64>,
        standby: bool,
        buffer: MediaBuffer,
        reply: Sender<Result<(), String>>,
    },
    Play(Sender<Result<(), String>>),
    Pause,
    Stop,
    Seek(f64),
    Volume {
        volume: f32,
        muted: bool,
    },
    Rate(f32),
    SetOutputDevice {
        id: Option<String>,
        reply: Sender<Result<(), String>>,
    },
    Transition {
        track_id: String,
        fade: Duration,
        reply: Sender<bool>,
    },
    HasStandby {
        track_id: String,
        reply: Sender<bool>,
    },
    DropStandby,
    /// The output stream of this generation died. Sent by the stream's own error callback.
    StreamFailed(u64, String),
    DropActive,
    ReleaseOutput(Sender<()>),
    Shutdown,
}

/// The handle to the engine. `Send + Sync`; dropping it stops the thread and closes the output.
pub struct NativeAudio {
    sender: Sender<Command>,
    output: OutputEpoch,
}

impl NativeAudio {
    /**
     * Starts the audio thread and opens `device` — an id from `list_output_devices` — or the
     * OS default for `None`.
     *
     * A failure here is what a machine with no working audio device reports. `on_event` runs
     * on the audio thread, so it must hand the event off rather than do work.
     * On iOS opening is deferred: activate the system session and call `prepare_output` before
     * the first load, so launching the UI alone does not interrupt another app's audio.
     */
    pub fn start(
        device: Option<String>,
        on_event: impl FnMut(Event) + Send + 'static,
    ) -> Result<Self, String> {
        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let commands = tx.clone();
        let output = OutputEpoch::new(!cfg!(target_os = "ios"));
        let epoch = output.clone();
        std::thread::Builder::new()
            .name("surco-audio".into())
            .spawn(move || run(Box::new(on_event), rx, commands, ready_tx, device, epoch))
            .map_err(|error| format!("audio thread failed to start: {error}"))?;

        ready_rx
            .recv()
            .map_err(|_| "audio thread stopped before it was ready".to_string())??;
        Ok(Self { sender: tx, output })
    }

    fn send(&self, command: Command) -> Result<(), String> {
        self.sender
            .send(command)
            .map_err(|_| "audio thread is gone".to_string())
    }

    /// Sends a command and waits for its reply, mapping a dead thread to an error rather than a
    /// hang. Every reply channel in `Command` is a one-shot, so a single `recv` is the whole
    /// protocol.
    fn request<T>(&self, build: impl FnOnce(Sender<T>) -> Command) -> Result<T, String> {
        let (tx, rx) = mpsc::channel();
        self.send(build(tx))?;
        rx.recv()
            .map_err(|_| "audio thread dropped the request".to_string())
    }

    /**
     * Decodes a track onto a deck, paused. Returns its duration when anything knows it.
     *
     * Blocks until the decoder has read the container header out of the buffer, which for a
     * download means waiting on the network — so the decode runs here, on the caller's thread,
     * and not on the audio thread, which stays free to go on reporting the position of the
     * track that is still playing. Call it from a worker.
     */
    pub fn load(&self, load: Load) -> Result<Option<f64>, String> {
        let output_epoch = self.output.current().ok_or("audio output is not active")?;
        let mut reader = BufferReader::new(load.buffer.clone());
        reader.output = Some((self.output.clone(), output_epoch));
        let mime_type = match opus_source::sniff_container_mime(&mut reader) {
            Ok(Some(sniffed)) => sniffed,
            // Unreadable here means unreadable for the decoder too; let that report it.
            _ => load.mime_type.as_str(),
        };

        /*
         * Opus goes to libopus, everything else to rodio.
         *
         * Not a preference — symphonia has no Opus decoder at all, and rodio can only offer
         * what symphonia implements.
         */
        let decoded = if opus_source::is_opus(mime_type) {
            OpusSource::new(Box::new(reader), mime_type).map(|source| equalized(source, load.gain))
        } else {
            let total = load.buffer.total();
            let builder = rodio::Decoder::builder().with_data(reader);
            /*
             * rodio answers `is_seekable` and `byte_len` from its own settings, not from the
             * reader, and they default to a forward-only stream of unknown length: no seeking
             * backwards, and an MP4 whose `moov` follows its audio does not open. Setting the
             * length also marks the stream seekable.
             */
            match total {
                Some(total) => builder.with_byte_len(total as u64),
                None => builder,
            }
            .build()
            .map(|source| equalized(source, load.gain))
            .map_err(|error| format!("audio decode failed: {error}"))
        };
        let (source, decoded_duration) = decoded?;

        let known = |value: &f64| *value > 0.0;
        let duration_sec = decoded_duration.filter(known).or(load.duration_sec.filter(known));
        self.request(|reply| Command::Load {
            output_epoch,
            track_id: load.track_id,
            source,
            duration_sec,
            start_sec: load.start_sec,
            standby: load.standby,
            buffer: load.buffer,
            reply,
        })??;
        Ok(duration_sec)
    }

    pub fn play(&self) -> Result<(), String> {
        self.request(Command::Play)?
    }

    pub fn pause(&self) -> Result<(), String> {
        self.send(Command::Pause)
    }

    /// Stops and clears both decks.
    pub fn stop(&self) -> Result<(), String> {
        self.send(Command::Stop)
    }

    pub fn seek(&self, position_sec: f64) -> Result<(), String> {
        self.send(Command::Seek(position_sec))
    }

    /// `volume` is linear, 0 to 1.
    pub fn set_volume(&self, volume: f32, muted: bool) -> Result<(), String> {
        self.send(Command::Volume { volume, muted })
    }

    /**
     * Playback speed, 0.25 to 4.
     *
     * ponytail: this resamples, so it transposes — a song at 1.25x plays a little sharp. Rate
     * is a podcast feature in a music app and defaults to 1. Add a time-stretcher between the
     * decoder and the sink if it turns out to matter. Positions count time played, so away
     * from 1 they also stop matching the place in the track.
     */
    pub fn set_rate(&self, rate: f32) -> Result<(), String> {
        self.send(Command::Rate(rate))
    }

    /// Hands playback to the standby deck, over an equal-power crossfade when `fade` is above
    /// zero and in a single step when it is not. False means `track_id` was not preloaded, or
    /// its download failed, and the caller should load normally.
    ///
    /// The single step is gapless only while the outgoing track is still playing. `Ended` is
    /// noticed within `END_POLL` when the duration is known and right, and up to a `TICK` late
    /// otherwise, so a handover that waits for it has that much silence in it.
    pub fn transition(&self, track_id: &str, fade: Duration) -> Result<bool, String> {
        self.request(|reply| Command::Transition { track_id: track_id.to_string(), fade, reply })
    }

    /// Whether `track_id` is sitting decoded on the standby deck.
    pub fn has_standby(&self, track_id: &str) -> Result<bool, String> {
        self.request(|reply| Command::HasStandby { track_id: track_id.to_string(), reply })
    }

    pub fn drop_standby(&self) -> Result<(), String> {
        self.send(Command::DropStandby)
    }

    /**
     * Stops and clears the active deck only, leaving the standby deck untouched.
     *
     * The narrow counterpart to `drop_standby`, and the one a cancelled *load* needs: `stop`
     * clears both decks, which is right for an actual stop but wrong for a skip that lands
     * mid-load — it would discard whatever the standby was decoding for the track the skip was
     * heading towards.
     */
    pub fn drop_active(&self) -> Result<(), String> {
        self.send(Command::DropActive)
    }

    /// Reopens the stream on a different device, `None` for the OS default. Both decks are lost
    /// with the old stream — the caller reloads whatever was playing.
    pub fn set_output_device(&self, id: Option<String>) -> Result<(), String> {
        self.request(|reply| Command::SetOutputDevice { id, reply })?
    }

    /// Closes the device and clears both decks. The caller retains the track and reloads it.
    /// After `prepare_output`, the next load can reopen immediately without the failure backoff.
    pub fn release_output(&self) -> Result<(), String> {
        self.output.release();
        self.request(Command::ReleaseOutput)
    }

    /// Allows a new load after the platform has activated its playback session.
    pub fn prepare_output(&self) { self.output.prepare(); }
}

impl Drop for NativeAudio {
    // The thread holds a sender of its own for the stream's error callback, so the channel
    // never disconnects by itself.
    fn drop(&mut self) {
        self.output.release();
        let _ = self.sender.send(Command::Shutdown);
    }
}

#[derive(Clone, Debug)]
pub struct AudioOutputDevice {
    /// Opaque, and the only part of this that round-trips through `set_output_device`. A
    /// device's name is not guaranteed unique or stable across a reconnect on every backend;
    /// its id is what cpal documents as surviving both.
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/**
 * Enumerates output devices.
 *
 * Pure enumeration, not routed through the audio thread — it never opens a device, and it
 * works before the engine has started.
 */
pub fn list_output_devices() -> Result<Vec<AudioOutputDevice>, String> {
    // When a sound server owns the hardware, its own sink list is the truth about what's
    // actually plugged in (it already does jack-sensing); raw ALSA per-card enumeration below
    // can't tell a live port from an empty one and duplicates what the server already presents.
    #[cfg(target_os = "linux")]
    if let Some(sinks) = pipewire_sinks() {
        return Ok(sinks);
    }

    use rodio::cpal::traits::HostTrait;

    let host = rodio::cpal::default_host();
    let default_id = host.default_output_device().and_then(|device| device.id().ok());
    let devices = host
        .output_devices()
        .map_err(|error| format!("failed to list audio output devices: {error}"))?;

    Ok(devices
        .filter_map(|device| {
            let id = device.id().ok()?;
            let id_string = id.to_string();
            #[cfg(target_os = "linux")]
            if is_alsa_plugin_noise(&id_string) {
                return None;
            }
            let name = device.description().ok()?.name().to_string();
            let is_default = default_id.as_ref() == Some(&id);
            Some(AudioOutputDevice { id: id_string, name, is_default })
        })
        .collect())
}

/// ALSA registers dozens of generic "type" PCMs — rate converters, plugin wrappers, and
/// per-card surround/iec958 variants most cards don't actually support — globally in
/// alsa.conf regardless of what hardware is plugged in, and cpal's hint enumeration surfaces
/// every one of them. On top of that, cpal's physical-device probe re-adds every card as raw
/// `hw:`/`plughw:` entries, duplicating the friendly `sysdefault:`/`hdmi:` hints for the same
/// hardware. This keeps only the handful of genuinely distinct, selectable outputs (default,
/// pulse/pipewire, and each card's sysdefault:/typed hint) and drops the rest.
///
/// ponytail: excluding `hw`/`plughw` outright assumes every card also gets a friendly hint
/// (true for the stock alsa.conf `sysdefault` auto-config). A card with no such hint would
/// become unselectable — if that surfaces, add it back only when its CARD= has no other entry.
#[cfg(target_os = "linux")]
fn is_alsa_plugin_noise(id: &str) -> bool {
    const NOISY_TYPES: &[&str] = &[
        "null", "jack", "oss", "speex", "speexrate", "lavrate", "samplerate", "upmix", "vdownmix",
        "surround21", "surround40", "surround41", "surround50", "surround51", "surround71",
        "iec958", "spdif", "usbstream", "dmix", "dsnoop", "front", "rear", "center_lfe", "side",
        "modem", "phoneline", "shm", "tee", "file", "empty", "rate", "route", "mulaw", "alaw",
        "adpcm", "plug", "multi", "ladspa", "asym", "hw", "plughw",
    ];
    // cpal prefixes the PCM name with its host, "alsa:front:CARD=PCH,DEV=0".
    let pcm = id.strip_prefix("alsa:").unwrap_or(id);
    let type_name = pcm.split(':').next().unwrap_or(pcm);
    NOISY_TYPES.contains(&type_name)
}

#[cfg(all(test, target_os = "linux"))]
mod alsa_device_filter_tests {
    use super::is_alsa_plugin_noise;

    #[test]
    fn drops_alsa_plugin_and_virtual_types() {
        assert!(is_alsa_plugin_noise("null"));
        assert!(is_alsa_plugin_noise("surround51:CARD=PCH,DEV=0"));
        assert!(is_alsa_plugin_noise("iec958:CARD=PCH,DEV=0"));
        assert!(is_alsa_plugin_noise("usbstream:CARD=Device"));
        assert!(is_alsa_plugin_noise("front:CARD=PCH,DEV=0"));
        // Raw hw:/plughw: duplicate the friendly sysdefault:/hdmi: hints for the same card.
        assert!(is_alsa_plugin_noise("hw:CARD=PCH,DEV=0"));
        assert!(is_alsa_plugin_noise("plughw:CARD=PCH,DEV=0"));
        // As cpal actually spells them.
        assert!(is_alsa_plugin_noise("alsa:null"));
        assert!(is_alsa_plugin_noise("alsa:surround21:CARD=Generic_1,DEV=0"));
    }

    #[test]
    fn keeps_genuinely_selectable_outputs() {
        assert!(!is_alsa_plugin_noise("default"));
        assert!(!is_alsa_plugin_noise("pulse"));
        assert!(!is_alsa_plugin_noise("pipewire"));
        assert!(!is_alsa_plugin_noise("sysdefault:CARD=PCH"));
        assert!(!is_alsa_plugin_noise("hdmi:CARD=HDMI,DEV=0"));
        assert!(!is_alsa_plugin_noise("alsa:default"));
        assert!(!is_alsa_plugin_noise("alsa:sysdefault:CARD=Generic_1"));
    }
}

/// Prefix marking an id as a PipeWire/PulseAudio sink name rather than a cpal `DeviceId` — the
/// two live in different namespaces and `open_device_sink` needs to tell them apart.
#[cfg(target_os = "linux")]
const PIPEWIRE_SINK_ID_PREFIX: &str = "pipewire-sink:";

/// Sink list from `pactl`, when a sound server is running. `None` means no server (or `pactl`
/// missing) — falls back to raw ALSA enumeration.
#[cfg(target_os = "linux")]
fn pipewire_sinks() -> Option<Vec<AudioOutputDevice>> {
    let output = std::process::Command::new("pactl")
        .args(["-f", "json", "list", "sinks"])
        .output()
        .ok()?;
    let sinks = serde_json::from_slice::<serde_json::Value>(&output.stdout).ok()?;
    let sinks = sinks.as_array().filter(|sinks| !sinks.is_empty())?;

    let default_name = std::process::Command::new("pactl")
        .arg("get-default-sink")
        .output()
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string());

    Some(
        sinks
            .iter()
            .filter_map(|sink| {
                let name = sink.get("name")?.as_str()?;
                let description = sink.get("description").and_then(|d| d.as_str()).unwrap_or(name);
                Some(AudioOutputDevice {
                    id: format!("{PIPEWIRE_SINK_ID_PREFIX}{name}"),
                    name: description.to_string(),
                    is_default: default_name.as_deref() == Some(name),
                })
            })
            .collect(),
    )
}

/// Steers our own stream to `sink_name` after it's already open on whatever the server picked.
/// The ALSA "pulse"/"pipewire" virtual device cpal opens can't target a specific sink by name —
/// that's a sound-server-level move, done here via `pactl` rather than at open time.
///
/// A failure here is never fatal: the stream keeps playing on the server's chosen sink, same as
/// before the user picked a different one, which is a smaller surprise than losing audio.
#[cfg(target_os = "linux")]
fn move_our_stream_to_pipewire_sink(sink_name: &str) {
    // ponytail: bounded retry rather than a proper "client registered" signal — PipeWire needs
    // a moment after the stream opens before our sink-input shows up in its graph.
    for attempt in 0..10 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(50));
        }
        let Some(sink_input_id) = our_sink_input_id() else { continue };
        let moved = std::process::Command::new("pactl")
            .args(["move-sink-input", &sink_input_id.to_string(), sink_name])
            .status()
            .is_ok_and(|status| status.success());
        if moved {
            return;
        }
    }
    warn(format_args!("failed to move audio stream to pipewire sink sink={sink_name}"));
}

/// Lets our stream follow the server's default again. WirePlumber remembers the sink a choice
/// moved it to and hands that to every stream we open after, across restarts too; moving to the
/// default sink is how a PulseAudio client says to forget it.
#[cfg(target_os = "linux")]
fn follow_pipewire_default() {
    // Without pactl there is nothing to tell the server with, and no waiting on it either.
    if std::process::Command::new("pactl").arg("--version").output().is_ok() {
        move_our_stream_to_pipewire_sink("@DEFAULT_SINK@");
    }
}

/// The freshest sink-input opened by this executable — cpal opens a brand new PipeWire client
/// on every device switch, so the highest index among ours is the one that was just opened.
///
/// The ALSA plugin names its client after the binary, "PipeWire ALSA [surco]", and that name
/// is all a sink-input carries: the process id is only on the client object.
#[cfg(target_os = "linux")]
fn our_sink_input_id() -> Option<u64> {
    let output = std::process::Command::new("pactl")
        .args(["-f", "json", "list", "sink-inputs"])
        .output()
        .ok()?;
    let inputs = serde_json::from_slice::<serde_json::Value>(&output.stdout).ok()?;
    let ours = format!("[{}]", std::env::current_exe().ok()?.file_name()?.to_str()?);
    inputs
        .as_array()?
        .iter()
        .filter(|input| {
            input
                .get("properties")
                .and_then(|props| props.get("application.name"))
                .and_then(|name| name.as_str())
                .is_some_and(|name| name.ends_with(&ours))
        })
        .filter_map(|input| input.get("index")?.as_u64())
        .max()
}

/**
 * Opens the sink for `id`, or the OS default when `id` is `None`.
 *
 * A device id that no longer resolves — unplugged, a saved choice from a machine that changed
 * underneath it — falls back to the default rather than refusing outright. The alternative is a
 * stored preference from weeks ago turning into silence the user has to notice and go fix in
 * settings, which is a worse failure than picking a device for them.
 */
fn open_device_sink(
    id: Option<&str>,
    on_error: impl FnMut(StreamError) + Send + 'static,
) -> Result<MixerDeviceSink, String> {
    #[cfg(target_os = "linux")]
    if let Some(sink_name) = id.and_then(|id| id.strip_prefix(PIPEWIRE_SINK_ID_PREFIX)) {
        let stream = open_default_sink(on_error)?;
        move_our_stream_to_pipewire_sink(sink_name);
        return Ok(stream);
    }

    let device = id.and_then(|id| {
        let device = find_output_device(id);
        if device.is_none() {
            warn(format_args!("output device not found, using default id={id}"));
        }
        device
    });

    match device {
        Some(device) => DeviceSinkBuilder::from_device(device)
            .and_then(|builder| builder.with_error_callback(on_error).open_stream())
            .map_err(|error| format!("failed to open audio output device: {error}")),
        None => {
            let stream = open_default_sink(on_error)?;
            #[cfg(target_os = "linux")]
            follow_pipewire_default();
            Ok(stream)
        }
    }
}

// ponytail: rodio's fallback to another device keeps its own logging callback, so a stream on
// a fallback device is not recovered. Reimplement `open_default_sink` if that ever matters.
fn open_default_sink(
    on_error: impl FnMut(StreamError) + Send + 'static,
) -> Result<MixerDeviceSink, String> {
    DeviceSinkBuilder::from_default_device()
        .and_then(|builder| builder.with_error_callback(on_error).open_stream())
        .or_else(|_| DeviceSinkBuilder::open_default_sink())
        .map_err(|error| format!("no audio output device: {error}"))
}

fn stream_failure_reporter(
    commands: Sender<Command>,
    generation: u64,
) -> impl FnMut(StreamError) + Send + 'static {
    let mut reported = false;
    move |error: StreamError| {
        // A glitch, or cpal already moving a default stream to the new default, as on macOS:
        // either way it plays on. Anything else leaves the stream dead, and a dead stream can
        // go on reporting until it is dropped.
        if matches!(error.kind(), ErrorKind::Xrun | ErrorKind::DeviceChanged | ErrorKind::RealtimeDenied) || reported {
            return;
        }
        reported = true;
        let _ = commands.send(Command::StreamFailed(generation, error.to_string()));
    }
}

/// Opens `id` with two fresh, paused decks on it. The stream reports its own death once, as
/// `StreamFailed(generation, ..)`.
fn open_output(
    id: Option<&str>,
    commands: Sender<Command>,
    generation: u64,
) -> Result<(MixerDeviceSink, [Deck; 2]), String> {
    let mut stream = open_device_sink(id, stream_failure_reporter(commands, generation))?;
    // rodio prints a notice to stderr for every stream it drops, and a device switch drops one.
    stream.log_on_drop(false);
    let deck = || {
        let sink = Player::connect_new(stream.mixer());
        sink.pause();
        Deck { sink, track_id: None, duration_sec: None, buffer: None }
    };
    let decks = [deck(), deck()];
    Ok((stream, decks))
}

fn find_output_device(id: &str) -> Option<rodio::Device> {
    use rodio::cpal::traits::HostTrait;

    let id: rodio::cpal::DeviceId = id.parse().ok()?;
    rodio::cpal::default_host().device_by_id(&id)
}

fn run(
    emit: Box<dyn FnMut(Event) + Send>,
    rx: Receiver<Command>,
    commands: Sender<Command>,
    ready: Sender<Result<(), String>>,
    initial_device: Option<String>,
    output: OutputEpoch,
) {
    // An iOS session becomes active only after a play request, before the first load.
    #[cfg(target_os = "ios")]
    let (stream, decks) = (None, [Deck::disconnected(), Deck::disconnected()]);
    #[cfg(not(target_os = "ios"))]
    let (stream, decks) = match open_output(initial_device.as_deref(), commands.clone(), 0) {
        Ok((stream, decks)) => (Some(stream), decks),
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };

    let mut engine = Engine {
        emit,
        stream,
        default_device: if initial_device.is_none() { default_output_id() } else { None },
        device_id: initial_device,
        commands,
        generation: 0,
        opened_at: Instant::now(),
        reopen_after: None,
        lost_track: None,
        decks,
        active: 0,
        volume: 1.0,
        muted: false,
        rate: 1.0,
        playing: false,
        fade: None,
        output,
    };

    if ready.send(Ok(())).is_err() {
        return;
    }

    let mut next_tick = Instant::now() + TICK;
    loop {
        let now = Instant::now();
        let until_tick = next_tick.saturating_duration_since(now);
        // A fade and the last moments of a track need finer steps than the position tick, so
        // whichever is due first wins.
        let timeout = if engine.fade.is_some() {
            until_tick.min(FADE_STEP)
        } else if engine.is_ending() {
            until_tick.min(END_POLL)
        } else {
            until_tick
        };

        match rx.recv_timeout(timeout) {
            Ok(command) => {
                if engine.handle(command) {
                    return;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }

        engine.advance_fade();
        if Instant::now() >= next_tick {
            engine.tick();
            next_tick = Instant::now() + TICK;
        } else if engine.is_ending() {
            engine.finish();
        }
    }
}

/// The system's default output now. On Linux it is the sound server's "default", which the
/// server itself moves between sinks, so it never changes here.
fn default_output_id() -> Option<rodio::cpal::DeviceId> {
    use rodio::cpal::traits::HostTrait;

    rodio::cpal::default_host().default_output_device()?.id().ok()
}

struct Engine {
    output: OutputEpoch,
    emit: Box<dyn FnMut(Event) + Send>,
    /// Dropping this closes the output device, so it outlives every deck connected to it.
    /// `None` once it has died, until the next load reopens it.
    stream: Option<MixerDeviceSink>,
    /// What to reopen, `None` for the OS default.
    device_id: Option<String>,
    /// Which device the default was when the stream opened on it, to tell when it moves.
    default_device: Option<rodio::cpal::DeviceId>,
    /// Handed to each stream's error callback, the way back onto this thread.
    commands: Sender<Command>,
    /// Bumped per stream, so a late error from a replaced stream is ignored.
    generation: u64,
    opened_at: Instant,
    /// Loads refuse to reopen before this; see `STREAM_RETRY_WINDOW`.
    reopen_after: Option<Instant>,
    /// The track a paused deck lost with its stream, reported as failed on the next play.
    lost_track: Option<String>,
    decks: [Deck; 2],
    active: usize,
    volume: f32,
    muted: bool,
    rate: f32,
    playing: bool,
    fade: Option<Fade>,
}

impl Engine {
    fn standby(&self) -> usize {
        1 - self.active
    }

    fn output_volume(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.volume
        }
    }

    /// Returns true when the thread should stop.
    fn handle(&mut self, command: Command) -> bool {
        match command {
            Command::Load { output_epoch, track_id, source, duration_sec, start_sec, standby, buffer, reply } => {
                if !self.output.accepts(output_epoch) {
                    let _ = reply.send(Err("audio load was cancelled when its output was released".to_owned()));
                    return false;
                }
                if self.stream.is_none() {
                    let reopened = if self.reopen_after.is_some_and(|at| Instant::now() < at) {
                        Err("Audio output keeps failing. Try another output device.".to_string())
                    } else {
                        self.reopen(self.device_id.clone())
                    };
                    if let Err(error) = reopened {
                        let _ = reply.send(Err(error));
                        return false;
                    }
                }
                let index = if standby { self.standby() } else { self.active };
                if !standby {
                    // A fade against a deck that is being reloaded would ramp a track that is
                    // no longer there.
                    self.cancel_fade();
                }

                /*
                 * The standby is silent until a transition ramps it up. A deck that inherited
                 * the output volume would be audible the instant it was started, and a gapless
                 * swap starts it in the same tick it becomes active.
                 */
                let volume = if standby { 0.0 } else { self.output_volume() };
                let rate = self.rate;

                let deck = &mut self.decks[index];
                deck.sink.stop();
                deck.sink.append(source);
                deck.sink.pause();
                deck.sink.set_volume(volume);
                deck.sink.set_speed(rate);
                if let Some(start_sec) = start_sec {
                    deck.seek(start_sec);
                }
                deck.track_id = Some(track_id);
                deck.duration_sec = duration_sec;
                deck.buffer = Some(buffer);

                if !standby {
                    self.playing = false;
                }
                let _ = reply.send(Ok(()));
            }
            Command::Play(reply) => {
                // Paused when its stream died: reported now, so the caller reloads it.
                if self.decks[self.active].track_id.is_none() {
                    if let Some(track_id) = self.lost_track.take() {
                        (self.emit)(Event::Failed { track_id, message: STREAM_LOST.to_string() });
                        let _ = reply.send(Ok(()));
                        return false;
                    }
                }
                /*
                 * A crossfade already started this deck and owns its volume. A caller that
                 * plays right after a transition would otherwise write full volume here and
                 * jump the ramp to its end for the rest of the fade window.
                 */
                let volume = self.fade.is_none().then(|| self.output_volume());
                let deck = &mut self.decks[self.active];
                if deck.track_id.is_none() {
                    let _ = reply.send(Err("no track is loaded".to_string()));
                    return false;
                }
                if let Some(volume) = volume {
                    deck.sink.set_volume(volume);
                }
                deck.sink.play();
                self.playing = true;
                let _ = reply.send(Ok(()));
            }
            Command::Pause => {
                self.decks[self.active].sink.pause();
                self.playing = false;
            }
            Command::Stop => {
                self.cancel_fade();
                for deck in &mut self.decks {
                    deck.clear();
                }
                self.playing = false;
            }
            Command::Seek(seconds) => self.decks[self.active].seek(seconds),
            Command::Volume { volume, muted } => {
                self.volume = volume.clamp(0.0, 1.0);
                self.muted = muted;
                // A fade owns both volumes until it finishes; writing here would jump the ramp.
                if self.fade.is_none() {
                    self.decks[self.active].sink.set_volume(self.output_volume());
                }
            }
            Command::Rate(rate) => {
                self.rate = rate.clamp(0.25, 4.0);
                for deck in &self.decks {
                    deck.sink.set_speed(self.rate);
                }
            }
            Command::Transition { track_id, fade, reply } => {
                let standby = self.standby();
                if self.decks[standby].track_id.as_deref() != Some(track_id.as_str()) {
                    let _ = reply.send(false);
                    return false;
                }
                /*
                 * Checked again here, not only in `HasStandby`.
                 *
                 * The download can fail in the gap between the caller asking and the caller
                 * swapping, and this is the last point where refusing is free — past it the
                 * outgoing deck has been cleared and there is nothing to fall back to.
                 */
                if !self.decks[standby].is_healthy() {
                    warn(format_args!("transition refused, download failed track_id={track_id}"));
                    self.decks[standby].clear();
                    let _ = reply.send(false);
                    return false;
                }

                self.cancel_fade();
                let target = self.output_volume();
                self.decks[standby]
                    .sink
                    .set_volume(if fade.is_zero() { target } else { 0.0 });
                self.decks[standby].sink.set_speed(self.rate);
                self.decks[standby].sink.play();

                let outgoing = self.active;
                self.active = standby;
                self.playing = true;

                if fade.is_zero() {
                    self.decks[outgoing].clear();
                } else {
                    self.fade = Some(Fade { started_at: Instant::now(), duration: fade, outgoing });
                }
                let _ = reply.send(true);
            }
            Command::HasStandby { track_id, reply } => {
                let standby = self.standby();
                let matched = self.decks[standby].track_id.as_deref() == Some(track_id.as_str());
                /*
                 * A preloaded deck whose download died is worse than no preload at all: the
                 * caller would hand over gaplessly and play the few seconds that did arrive.
                 * Reporting it as absent sends the caller down the normal load path.
                 */
                if matched && !self.decks[standby].is_healthy() {
                    warn(format_args!("standby discarded, download failed track_id={track_id}"));
                    self.decks[standby].clear();
                    let _ = reply.send(false);
                    return false;
                }
                let _ = reply.send(matched);
            }
            Command::DropStandby => {
                let standby = self.standby();
                self.decks[standby].clear();
            }
            Command::DropActive => {
                // A fade reads the active deck's volume every tick; cancel it first, or the
                // next tick would ramp a deck that was just cleared.
                self.cancel_fade();
                self.decks[self.active].clear();
                self.playing = false;
            }
            Command::SetOutputDevice { id, reply } => {
                let _ = reply.send(self.reopen(id));
            }
            Command::ReleaseOutput(reply) => {
                self.fade = None;
                // Stop alone leaves Rodio's source counter pending if its mixer
                // disappears. Fresh sinks make paused seeks return immediately.
                self.decks = [Deck::disconnected(), Deck::disconnected()];
                self.stream = None;
                self.generation += 1;
                self.reopen_after = None;
                self.lost_track = None;
                self.playing = false;
                let _ = reply.send(());
            }
            Command::StreamFailed(generation, message) => {
                if generation != self.generation || self.stream.is_none() || self.output.current().is_none() {
                    return false;
                }
                // Dropping it is what ends cpal's loop on the dead device.
                self.stream = None;
                self.fade = None;
                /*
                 * A device that went away, as headphones unplugged, or a default that moved, as
                 * headphones plugged in on Windows: the music goes on on the one to use now. One
                 * that dies right after opening while still the one to use is left to the
                 * backoff below.
                 */
                let moved = self.device_id.is_none() && default_output_id() != self.default_device;
                if (moved || self.opened_at.elapsed() >= STREAM_RETRY_WINDOW) && self.reroute() {
                    return false;
                }
                if self.opened_at.elapsed() < STREAM_RETRY_WINDOW {
                    self.reopen_after = Some(Instant::now() + STREAM_RETRY_WINDOW);
                }
                let lost = self.decks[self.active].track_id.clone();
                self.decks = [Deck::disconnected(), Deck::disconnected()];
                (self.emit)(Event::StreamFailed { message });
                match lost {
                    Some(track_id) if self.playing => {
                        (self.emit)(Event::Failed { track_id, message: STREAM_LOST.to_string() });
                    }
                    lost => self.lost_track = lost,
                }
                self.playing = false;
            }
            Command::Shutdown => return true,
        }
        false
    }

    /// Reopens the output on `id`. Both decks go with the old stream; the caller reloads.
    fn reopen(&mut self, id: Option<String>) -> Result<(), String> {
        let generation = self.generation + 1;
        let (stream, decks) = open_output(id.as_deref(), self.commands.clone(), generation)?;
        // A fade on the old decks is stale — dropped rather than settled through `cancel_fade`,
        // which would write a volume to a deck this is about to discard anyway.
        self.fade = None;
        self.stream = Some(stream);
        self.decks = decks;
        self.default_device = if id.is_none() { default_output_id() } else { None };
        self.device_id = id;
        self.generation = generation;
        self.opened_at = Instant::now();
        self.lost_track = None;
        self.active = 0;
        self.playing = false;
        Ok(())
    }

    /// Reopens the output on the device to use now, for the caller to load its track again.
    fn reroute(&mut self) -> bool {
        match self.reopen(self.device_id.clone()) {
            Ok(()) => {
                (self.emit)(Event::Rerouted);
                true
            }
            Err(error) => {
                warn(format_args!("failed to move to the current audio output: {error}"));
                false
            }
        }
    }

    fn cancel_fade(&mut self) {
        if self.fade.take().is_some() {
            let volume = self.output_volume();
            self.decks[self.active].sink.set_volume(volume);
        }
    }

    /// Moves an in-progress crossfade one step, and finishes it when the window closes.
    fn advance_fade(&mut self) {
        let Some(fade) = &self.fade else { return };
        let progress = (fade.started_at.elapsed().as_secs_f32()
            / fade.duration.as_secs_f32().max(f32::EPSILON))
        .clamp(0.0, 1.0);
        let outgoing = fade.outgoing;
        let target = self.output_volume();

        /*
         * Equal-power rather than linear. Two linear ramps crossing at half volume sum to an
         * audible dip in the middle, because loudness is not proportional to amplitude; the
         * sine/cosine pair holds perceived level constant across the transition.
         */
        self.decks[outgoing]
            .sink
            .set_volume(target * (progress * std::f32::consts::FRAC_PI_2).cos());
        self.decks[self.active]
            .sink
            .set_volume(target * (progress * std::f32::consts::FRAC_PI_2).sin());

        if progress >= 1.0 {
            self.fade = None;
            self.decks[outgoing].clear();
            self.decks[self.active].sink.set_volume(target);
        }
    }

    /// Whether the active track is within a tick of its end. `Ended` is then looked for every
    /// `END_POLL`, so a handover that waits for it follows without an audible gap.
    fn is_ending(&self) -> bool {
        let deck = &self.decks[self.active];
        self.playing
            && self.fade.is_none()
            && deck.duration_sec.is_some_and(|duration| {
                duration - deck.sink.get_pos().as_secs_f64() < TICK.as_secs_f64()
            })
    }

    /// Reports the end of the active track, and says whether it did.
    fn finish(&mut self) -> bool {
        if self.output.current().is_none() { return false; }
        let index = self.active;
        let Some(track_id) = self.decks[index].track_id.clone() else { return false };

        /*
         * An empty sink means the decoder ran out, which is the only end-of-track signal there
         * is. Suppressed mid-fade: the outgoing deck emptying there is the transition working,
         * not the new track finishing.
         */
        if !(self.playing && self.fade.is_none() && self.decks[index].sink.empty()) {
            return false;
        }
        // A failed download reads as an end of stream, so the decoder ran out early.
        let event = if self.decks[index].is_healthy() {
            Event::Ended { track_id }
        } else {
            Event::Failed { track_id, message: "the audio download failed".to_string() }
        };
        self.decks[index].clear();
        self.playing = false;
        (self.emit)(event);
        true
    }

    fn tick(&mut self) {
        if self.finish() || !self.playing {
            return;
        }
        let index = self.active;
        let Some(track_id) = self.decks[index].track_id.clone() else { return };
        (self.emit)(Event::Position {
            track_id,
            position_sec: self.decks[index].sink.get_pos().as_secs_f64(),
            duration_sec: self.decks[index].duration_sec,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn released_outputs_do_not_advance_the_queue_or_accept_late_loads() {
        let output = OutputEpoch::new(true);
        let token = output.current().unwrap();
        let (commands, _) = mpsc::channel();
        let (events, received) = mpsc::channel();
        let mut engine = Engine {
            output: output.clone(), emit: Box::new(move |event| { let _ = events.send(event); }),
            stream: None, device_id: None, default_device: None, commands, generation: 1, opened_at: Instant::now(),
            reopen_after: Some(Instant::now() + STREAM_RETRY_WINDOW), lost_track: Some("a".into()),
            decks: [Deck::disconnected(), Deck::disconnected()], active: 0, volume: 1.0,
            muted: false, rate: 1.0, playing: true, fade: None,
        };
        engine.decks[0].track_id = Some("a".into());
        // A disconnected mixer never iterates the queued source to its end.
        // Rodio's stop alone leaves that Player's sound_count pending.
        engine.decks[0].sink.append(rodio::source::SineWave::new(440.0));
        assert!(!engine.decks[0].sink.empty());
        output.release();
        assert!(!engine.finish(), "a cancelled decoder is not the song ending");
        let (reply, done) = mpsc::channel();
        engine.handle(Command::ReleaseOutput(reply));
        done.recv().unwrap();
        assert!(engine.decks.iter().all(|deck| deck.sink.empty()), "release must discard sinks whose mixer can no longer answer seek");
        assert!(engine.reopen_after.is_none());
        assert!(engine.lost_track.is_none());
        assert!(!engine.playing);
        output.prepare();
        let (reply, result) = mpsc::channel();
        engine.handle(Command::Load {
            output_epoch: token, track_id: "late".into(), source: Box::new(rodio::source::Empty::new()),
            duration_sec: None, start_sec: None, standby: false, buffer: MediaBuffer::new(None), reply,
        });
        assert!(result.recv().unwrap().is_err());
        assert!(engine.stream.is_none(), "a stale load must not touch the sound card");
        assert!(received.try_recv().is_err());
    }

    #[test]
    fn releasing_output_cancels_a_waiting_decoder_without_discarding_downloaded_bytes() {
        let buffer = MediaBuffer::new(Some(4));
        let epoch = OutputEpoch::new(true);
        let token = epoch.current().unwrap();
        let mut reader = BufferReader::new(buffer.clone());
        reader.output = Some((epoch.clone(), token));
        let waiting = std::thread::spawn(move || reader.read(&mut [0u8; 4]));
        epoch.release();
        assert_eq!(waiting.join().unwrap().unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
        assert!(!buffer.is_failed(), "pausing must preserve the download for resume");
        buffer.append(&[1, 2, 3, 4]);
        buffer.complete();
        epoch.prepare();
        assert!(!epoch.accepts(token), "a late load cannot join the new output");
        let mut reader = BufferReader::new(buffer);
        reader.output = Some((epoch.clone(), epoch.current().unwrap()));
        let mut bytes = [0u8; 4];
        assert_eq!(reader.read(&mut bytes).unwrap(), 4);
        assert_eq!(bytes, [1, 2, 3, 4]);
    }

    #[test]
    fn stream_failure_is_reported_once_and_underruns_never() {
        let (tx, rx) = mpsc::channel();
        let mut report = stream_failure_reporter(tx, 7);
        let pollerr = || StreamError::with_message(ErrorKind::BackendError, "`alsa::poll()` returned POLLERR");

        report(ErrorKind::Xrun.into());
        assert!(rx.try_recv().is_err(), "an underrun is recovered by cpal, not by reopening");
        report(ErrorKind::DeviceChanged.into());
        assert!(rx.try_recv().is_err(), "a default stream cpal moved itself plays on");

        for _ in 0..1000 {
            report(pollerr());
        }
        assert!(matches!(rx.try_recv(), Ok(Command::StreamFailed(7, _))));
        assert!(rx.try_recv().is_err(), "cpal repeats POLLERR in a loop; only the first counts");
    }

    /// The streaming reader the decoder pulls through: a read waits for bytes that have not
    /// landed instead of reporting an end the decoder would take for the end of the song.
    #[test]
    fn buffer_reader_waits_for_bytes_and_ends_when_the_download_does() {
        let buffer = MediaBuffer::new(Some(6));
        let mut reader = BufferReader::new(buffer.clone());

        let filling = buffer.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            filling.append(&[1, 2]);
        });

        let started_at = Instant::now();
        let mut out = [0u8; 8];
        let read = reader.read(&mut out).unwrap();
        assert_eq!(&out[..read], &[1, 2]);
        assert!(started_at.elapsed() >= Duration::from_millis(25));

        buffer.append(&[3, 4]);
        let read = reader.read(&mut out).unwrap();
        assert_eq!(&out[..read], &[3, 4]);

        /*
         * The tail never arrives and the download gives up. EOF, not an error and not a hang:
         * the decoder finishes the frames it has and the deck empties, so the queue advances
         * instead of stalling on a deck that never would.
         */
        buffer.fail();
        assert_eq!(reader.read(&mut out).unwrap(), 0);

        // Symphonia probes containers by seeking around; past the end must clamp rather than
        // fail, or a probe of a body still downloading kills the load.
        assert_eq!(reader.seek(SeekFrom::End(0)).unwrap(), 6);
        assert_eq!(reader.seek(SeekFrom::Start(999)).unwrap(), 6);
        assert_eq!(reader.seek(SeekFrom::Start(2)).unwrap(), 2);
    }

    /// A body of undeclared length ends where `complete` says, and is forward-only until then.
    #[test]
    fn a_body_of_unknown_length_ends_when_it_is_marked_complete() {
        let buffer = MediaBuffer::new(None);
        let mut reader = BufferReader::new(buffer.clone());
        assert!(!reader.is_seekable());
        assert_eq!(reader.byte_len(), None);
        assert!(reader.seek(SeekFrom::End(0)).is_err());

        buffer.append(&[1, 2, 3]);
        buffer.complete();
        let mut out = [0u8; 8];
        assert_eq!(reader.read(&mut out).unwrap(), 3);
        assert_eq!(reader.read(&mut out).unwrap(), 0);
        assert_eq!(reader.byte_len(), Some(3));
    }
}
