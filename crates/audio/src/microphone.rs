//! Listening through the default microphone, for a song to be named from what it hears.
//!
//! The sound comes as 16 kHz mono, as a fingerprint is made from: whatever the device gives is
//! mixed down and brought to that rate a sample at a time, each the mean of the input it spans,
//! which also keeps what is above 8 kHz from folding back below it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use rodio::cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rodio::cpal::{SampleFormat, SizedSample};

/// The rate a song's fingerprint is made at.
pub const RATE: u32 = 16_000;

/// The microphone, listening until it is dropped.
pub struct Microphone {
    stop: Arc<AtomicBool>,
}

impl Drop for Microphone {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Mixes down and brings to `RATE` what the device gives, a block at a time.
struct Down {
    channels: usize,
    /// Input samples to an output one.
    step: f64,
    phase: f64,
    sum: f32,
    count: u32,
}

impl Down {
    fn new(channels: u16, rate: u32) -> Self {
        Self { channels: usize::from(channels.max(1)), step: f64::from(rate) / f64::from(RATE), phase: 0.0, sum: 0.0, count: 0 }
    }

    fn feed(&mut self, samples: impl Iterator<Item = f32>, out: &mut Vec<i16>) {
        let mut frame = 0.0;
        for (index, sample) in samples.enumerate() {
            frame += sample;
            if (index + 1) % self.channels != 0 { continue; }
            self.sum += frame / self.channels as f32;
            frame = 0.0;
            self.count += 1;
            self.phase += 1.0;
            if self.phase >= self.step {
                self.phase -= self.step;
                let mean = self.sum / self.count as f32;
                out.push((mean.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16);
                self.sum = 0.0;
                self.count = 0;
            }
        }
    }
}

/// Starts listening, handing `heard` each stretch of 16 kHz mono sound as it comes, on a
/// thread of the microphone's own. Says why when there is no microphone to listen through.
pub fn listen(heard: impl FnMut(&[i16]) + Send + 'static) -> Result<Microphone, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let (ready, started) = mpsc::channel();
    let stopping = stop.clone();
    std::thread::Builder::new().name("surco-microphone".into()).spawn(move || {
        let stream = (|| {
            let device = rodio::cpal::default_host().default_input_device().ok_or("no microphone")?;
            let config = device.default_input_config().map_err(|error| error.to_string())?;
            let down = Down::new(config.channels(), config.sample_rate());
            let failed = |error| crate::warn(format_args!("microphone: {error}"));
            fn stream<T: SizedSample + 'static>(device: &rodio::cpal::Device, config: rodio::cpal::StreamConfig, mut down: Down, float: fn(T) -> f32,
                mut heard: impl FnMut(&[i16]) + Send + 'static, failed: impl FnMut(rodio::cpal::Error) + Send + 'static) -> Result<rodio::cpal::Stream, String> {
                let mut out = Vec::new();
                device.build_input_stream::<T, _, _>(config, move |data: &[T], _| {
                    out.clear();
                    down.feed(data.iter().map(|sample| float(*sample)), &mut out);
                    if !out.is_empty() { heard(&out); }
                }, failed, None).map_err(|error| error.to_string())
            }
            let stream = match config.sample_format() {
                SampleFormat::F32 => stream::<f32>(&device, config.config(), down, |sample| sample, heard, failed)?,
                SampleFormat::I16 => stream::<i16>(&device, config.config(), down, |sample| f32::from(sample) / f32::from(i16::MAX), heard, failed)?,
                SampleFormat::I32 => stream::<i32>(&device, config.config(), down, |sample| sample as f32 / i32::MAX as f32, heard, failed)?,
                other => return Err(format!("the microphone gives {other} samples")),
            };
            stream.play().map_err(|error| error.to_string())?;
            Ok::<_, String>(stream)
        })();
        let stream = match stream {
            Ok(stream) => { let _ = ready.send(Ok(())); stream }
            Err(why) => { let _ = ready.send(Err(why)); return }
        };
        while !stopping.load(Ordering::Relaxed) { std::thread::sleep(Duration::from_millis(50)); }
        drop(stream);
    }).map_err(|error| error.to_string())?;
    started.recv().map_err(|_| "the microphone did not start".to_owned())??;
    Ok(Microphone { stop })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_at_48_khz_comes_down_to_16_khz_mono_each_the_mean_it_spans() {
        let mut down = Down::new(2, 48_000);
        let mut out = Vec::new();
        // Three frames, left and right opposite: their mean is silence; then three of a half.
        let input = [0.5, -0.5, 0.5, -0.5, 0.5, -0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5];
        down.feed(input.into_iter(), &mut out);
        assert_eq!(out, [0, i16::MAX / 2]);
        // A rate that does not divide evenly keeps its place across blocks.
        let mut down = Down::new(1, 44_100);
        let mut out = Vec::new();
        for _ in 0..441 { down.feed(std::iter::repeat_n(0.25, 100), &mut out); }
        assert!(out.len().abs_diff(16_000) <= 1, "a second in, a second out: {}", out.len());
    }
}
