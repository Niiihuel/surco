//! Plays a file through the engine and prints every event.
//!
//! ```text
//! cargo run -p dnm-audio --example play -- <file> [--seek S] [--volume V]
//!     [--eq "preamp,b1,...,b10"] [--next <file> [--crossfade S]] [--device ID] [--seconds N]
//! cargo run -p dnm-audio --example play -- --list-devices
//! ```
//!
//! `--next` alone hands over when the first track ends. With `--crossfade S` the handover
//! starts `S` seconds before that; at zero it is a single step, taken on the last position
//! tick before the end.

use std::io::Read;
use std::str::FromStr;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use dnm_audio::equalizer::{self, EqualizerValues, BAND_COUNT};
use dnm_audio::{list_output_devices, Event, Load, MediaBuffer, NativeAudio, TICK};

fn main() -> Result<(), String> {
    let mut file = None;
    let mut next = None;
    let mut seek = None;
    let mut volume = 1.0;
    let mut eq = None;
    let mut crossfade = None;
    let mut device = None;
    let mut seconds = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--list-devices" => {
                for device in list_output_devices()? {
                    let mark = if device.is_default { " (default)" } else { "" };
                    println!("{}\t{}{mark}", device.id, device.name);
                }
                return Ok(());
            }
            "--seek" => seek = Some(parse(&value()?)?),
            "--volume" => volume = parse(&value()?)?,
            "--eq" => eq = Some(parse_eq(&value()?)?),
            "--next" => next = Some(value()?),
            "--crossfade" => crossfade = Some(parse::<f64>(&value()?)?),
            "--device" => device = Some(value()?),
            "--seconds" => seconds = Some(parse::<f64>(&value()?)?),
            _ => file = Some(arg),
        }
    }
    let file = file.ok_or("usage: play <file> [options], see the top of examples/play.rs")?;

    let (tx, events) = mpsc::channel();
    let audio = NativeAudio::start(device, move |event| {
        let _ = tx.send(event);
    })?;
    if let Some(values) = eq {
        equalizer::set_values(values);
    }
    audio.set_volume(volume, false)?;

    let duration = audio.load(load(&file, seek, false)?)?;
    println!("loaded {file} duration={duration:?}");
    if let Some(next) = &next {
        let duration = audio.load(load(next, None, true)?)?;
        println!("standby {next} duration={duration:?}");
    }
    audio.play()?;

    let started_at = Instant::now();
    let deadline = seconds.map(|seconds| started_at + Duration::from_secs_f64(seconds));
    let mut pending = next;
    loop {
        let event = match deadline {
            Some(deadline) => {
                match events.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    Ok(event) => event,
                    Err(_) => break,
                }
            }
            None => events.recv().map_err(|_| "the engine stopped")?,
        };
        println!("{:7.3} {event:?}", started_at.elapsed().as_secs_f64());

        let fade = match (&event, crossfade) {
            (Event::Position { position_sec, duration_sec: Some(duration), .. }, Some(fade))
                if duration - position_sec <= fade.max(TICK.as_secs_f64()) =>
            {
                Some(fade)
            }
            (Event::Ended { .. }, _) => Some(0.0),
            (Event::Failed { message, .. } | Event::StreamFailed { message }, _) => {
                return Err(message.clone())
            }
            _ => None,
        };
        let Some(fade) = fade else { continue };
        match pending.take() {
            Some(next) => {
                let swapped = audio.transition(&next, Duration::from_secs_f64(fade))?;
                let at = started_at.elapsed().as_secs_f64();
                println!("{at:7.3} transition fade={fade} swapped={swapped}");
            }
            None if matches!(event, Event::Ended { .. }) => break,
            None => {}
        }
    }
    Ok(())
}

fn parse<T: FromStr>(text: &str) -> Result<T, String> {
    text.parse().map_err(|_| format!("not a number: {text}"))
}

fn parse_eq(text: &str) -> Result<EqualizerValues, String> {
    let gains = text.split(',').map(|gain| parse(gain.trim())).collect::<Result<Vec<f32>, _>>()?;
    let [preamp_db, bands @ ..] = gains.as_slice() else { return Err("--eq is empty".into()) };
    let bands_db = <[f32; BAND_COUNT]>::try_from(bands)
        .map_err(|_| format!("--eq takes a preamp and {BAND_COUNT} bands"))?;
    Ok(EqualizerValues { preamp_db: *preamp_db, bands_db })
}

/// Feeds the file into its buffer in small chunks from another thread, the way a download
/// would, so the decoder starts on a body that is still arriving.
fn load(path: &str, start_sec: Option<f64>, standby: bool) -> Result<Load, String> {
    let mut file = std::fs::File::open(path).map_err(|error| format!("{path}: {error}"))?;
    let total = file.metadata().map_err(|error| format!("{path}: {error}"))?.len() as usize;
    let buffer = MediaBuffer::new(Some(total));

    let filling = buffer.clone();
    std::thread::spawn(move || {
        let mut chunk = [0u8; 16 * 1024];
        loop {
            match file.read(&mut chunk) {
                Ok(0) => return filling.complete(),
                Ok(read) => filling.append(&chunk[..read]),
                Err(_) => return filling.fail(),
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });

    let mime_type = match path.rsplit('.').next() {
        Some("webm") => "audio/webm",
        Some("opus" | "ogg") => "audio/ogg",
        Some("mp3") => "audio/mpeg",
        Some("flac") => "audio/flac",
        _ => "audio/mp4",
    };
    Ok(Load {
        track_id: path.to_string(),
        buffer,
        mime_type: mime_type.to_string(),
        duration_sec: None,
        start_sec,
        standby,
    })
}
