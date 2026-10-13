/*!
 * The audio core of Surco: decode, two-deck output, equaliser. No UI and no networking.
 *
 * Adapted from the Apache-2.0 project noFAYZ/zuno; each adapted file says what it came from.
 *
 * The caller downloads a track into a [`MediaBuffer`] and hands it to [`NativeAudio::load`]
 * while the bytes are still arriving. One engine thread owns the output device and two decks:
 * the active one plays, the standby one holds the next track already decoded, and
 * [`NativeAudio::transition`] swaps them — in one step, or over a crossfade.
 *
 * ```no_run
 * use std::time::Duration;
 * use surco_audio::{equalizer, Event, Load, MediaBuffer, NativeAudio};
 *
 * # fn main() -> Result<(), String> {
 * let audio = NativeAudio::start(None, |event| match event {
 *     Event::Position { track_id, position_sec, duration_sec } => { /* progress bar */ }
 *     Event::Ended { track_id } => { /* advance the queue */ }
 *     Event::Failed { track_id, message } => { /* load it again */ }
 *     Event::StreamFailed { message } => { /* the output device died */ }
 * })?;
 *
 * let buffer = MediaBuffer::new(Some(3_400_000));
 * // On the downloader's side: buffer.append(&chunk) per chunk, then buffer.complete(),
 * // or buffer.fail().
 * let duration = audio.load(Load {
 *     track_id: "a".into(),
 *     buffer,
 *     mime_type: "audio/webm; codecs=\"opus\"".into(),
 *     duration_sec: Some(204.0),
 *     start_sec: None,
 *     standby: false,
 *     gain: 1.0,
 * })?;
 * audio.play()?;
 * audio.seek(30.0)?;
 * audio.set_volume(0.8, false)?;
 *
 * // The next track goes onto the standby deck the same way, with `standby: true`, and then:
 * if audio.has_standby("b")? {
 *     audio.transition("b", Duration::from_secs(3))?; // Duration::ZERO hands over in one step
 * }
 *
 * let mut values = equalizer::EqualizerValues::FLAT;
 * values.bands_db[5] = 6.0; // equalizer::BAND_HZ[5] is 1 kHz
 * equalizer::set_values(values);
 * # Ok(())
 * # }
 * ```
 *
 * - Engine: [`NativeAudio::start`]; dropping the handle shuts it down.
 * - Decks: [`load`](NativeAudio::load), [`transition`](NativeAudio::transition),
 *   [`has_standby`](NativeAudio::has_standby), [`drop_standby`](NativeAudio::drop_standby),
 *   [`drop_active`](NativeAudio::drop_active).
 * - Transport: [`play`](NativeAudio::play), [`pause`](NativeAudio::pause),
 *   [`stop`](NativeAudio::stop), [`seek`](NativeAudio::seek),
 *   [`set_volume`](NativeAudio::set_volume), [`set_rate`](NativeAudio::set_rate).
 * - Equaliser, process-global: [`equalizer::set_values`], [`equalizer::values`],
 *   [`equalizer::BAND_HZ`]. So are [`equalizer::set_mono`] and [`equalizer::set_normalize`].
 * - Output: [`list_output_devices`], [`NativeAudio::set_output_device`].
 * - Events: [`Event`], with a position every [`TICK`].
 *
 * `load` blocks on the download and belongs on a worker thread. Every other call is a message
 * to the engine thread and returns at once, or as soon as that thread has answered. Errors are
 * messages meant for a log or a toast.
 */

mod audio;
pub mod equalizer;
mod media_buffer;
pub mod meter;
mod opus_source;

pub use audio::{list_output_devices, AudioOutputDevice, Event, Load, NativeAudio, TICK};
pub use media_buffer::MediaBuffer;

fn warn(message: impl std::fmt::Display) {
    eprintln!("[surco-audio] {message}");
}
