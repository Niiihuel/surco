//! The silence a track opens and closes with, as Opentune's "skip silence" passes it by.
//!
//! A track that starts from its beginning drops the silent frames it opens with, up to
//! `LEAD_MOST`, and says how much it dropped: the deck adds that to where it reports being, so
//! a position is still the track's own and lyrics and video stay with it. Whatever plays notes
//! when it last heard something, which tells the engine a track's last seconds are silent.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rodio::{ChannelCount, SampleRate, Source};

/// Below this a sample is silence: -50 dBFS.
const THRESHOLD: f32 = 0.003_162;
/// The most of a track's opening dropped as silence.
const LEAD_MOST: f64 = 5.0;

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Whether the silence tracks open and close with is passed by.
pub fn set_skip_silence(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

pub(crate) fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// What a `Quiet` source tells its deck, in seconds of the track.
#[derive(Default)]
pub(crate) struct Heard {
    /// The opening dropped, until a seek makes positions the track's own again.
    skipped: AtomicU64,
    /// When the track last made a sound.
    last: AtomicU64,
}

impl Heard {
    #[cfg(test)]
    pub(crate) fn at(skipped: f64, last: f64) -> Self {
        Self { skipped: AtomicU64::new(skipped.to_bits()), last: AtomicU64::new(last.to_bits()) }
    }

    pub(crate) fn skipped(&self) -> f64 {
        f64::from_bits(self.skipped.load(Ordering::Relaxed))
    }

    pub(crate) fn last(&self) -> f64 {
        f64::from_bits(self.last.load(Ordering::Relaxed))
    }
}

pub(crate) struct Quiet<S> {
    inner: S,
    heard: Arc<Heard>,
    /// Whether the opening is still being looked through for its first sound.
    trimming: bool,
    /// Where in the track the next sample is, in frames.
    frame: u64,
    /// The sample of the frame the next one belongs to.
    channel: u16,
    /// A frame read past the silence before it, still to be given out.
    held: Vec<f32>,
}

impl<S: Source> Quiet<S> {
    /// `trim` drops the silence it opens with: only a track played from its beginning.
    pub(crate) fn new(inner: S, trim: bool) -> (Self, Arc<Heard>) {
        let heard = Arc::new(Heard::default());
        (Self { inner, heard: heard.clone(), trimming: trim, frame: 0, channel: 0, held: Vec::new() }, heard)
    }

    fn seconds(&self) -> f64 {
        self.frame as f64 / f64::from(self.inner.sample_rate().get())
    }

    fn step(&mut self, sample: f32) {
        if sample.abs() > THRESHOLD {
            self.heard.last.store(self.seconds().to_bits(), Ordering::Relaxed);
        }
        self.channel += 1;
        if self.channel >= self.inner.channels().get() {
            self.channel = 0;
            self.frame += 1;
        }
    }

    /// Past the frames of silence the track opens with, whole frames so that its channels stay
    /// in their places, to the first that sounds.
    fn trim(&mut self) {
        self.trimming = false;
        let channels = usize::from(self.inner.channels().get());
        let mut frame = Vec::with_capacity(channels);
        let mut dropped = 0u64;
        let most = (LEAD_MOST * f64::from(self.inner.sample_rate().get())) as u64;
        loop {
            frame.clear();
            for _ in 0..channels {
                match self.inner.next() {
                    Some(sample) => frame.push(sample),
                    None => break,
                }
            }
            let silent = frame.iter().all(|sample| sample.abs() <= THRESHOLD);
            if frame.len() < channels || !silent || dropped >= most {
                break;
            }
            dropped += 1;
        }
        self.frame = dropped;
        let skipped = dropped as f64 / f64::from(self.inner.sample_rate().get());
        self.heard.skipped.store(skipped.to_bits(), Ordering::Relaxed);
        self.heard.last.store(skipped.to_bits(), Ordering::Relaxed);
        frame.reverse();
        self.held = frame;
    }
}

impl<S: Source> Iterator for Quiet<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.trimming {
            self.trim();
        }
        let sample = match self.held.pop() {
            Some(sample) => sample,
            None => self.inner.next()?,
        };
        self.step(sample);
        Some(sample)
    }
}

impl<S: Source> Source for Quiet<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }

    fn channels(&self) -> ChannelCount {
        self.inner.channels()
    }

    fn sample_rate(&self) -> SampleRate {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    /// A seek puts it where it was asked: positions are the track's own from there.
    fn try_seek(&mut self, position: Duration) -> Result<(), rodio::source::SeekError> {
        self.inner.try_seek(position)?;
        self.trimming = false;
        self.held.clear();
        self.channel = 0;
        self.frame = (position.as_secs_f64() * f64::from(self.inner.sample_rate().get())) as u64;
        self.heard.skipped.store(0f64.to_bits(), Ordering::Relaxed);
        self.heard.last.store(position.as_secs_f64().to_bits(), Ordering::Relaxed);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::buffer::SamplesBuffer;

    fn stereo(frames: &[f32]) -> SamplesBuffer {
        let samples: Vec<f32> = frames.iter().flat_map(|level| [*level, -*level]).collect();
        SamplesBuffer::new(ChannelCount::new(2).unwrap(), SampleRate::new(10).unwrap(), samples)
    }

    #[test]
    fn a_track_from_its_beginning_drops_the_silence_it_opens_with() {
        let (source, heard) = Quiet::new(stereo(&[0.0, 0.001, 0.0, 0.5, 0.0, 0.25]), true);
        let samples: Vec<f32> = source.collect();
        assert_eq!(samples, [0.5, -0.5, 0.0, -0.0, 0.25, -0.25], "whole frames, the channels in their places");
        assert!((heard.skipped() - 0.3).abs() < 1e-9, "three frames of a tenth of a second");
        assert!((heard.last() - 0.5).abs() < 1e-9, "the last sound, in the track's own time");
    }

    #[test]
    fn a_track_resumed_or_handed_over_keeps_its_opening() {
        let (source, heard) = Quiet::new(stereo(&[0.0, 0.0, 0.5]), false);
        assert_eq!(source.count(), 6);
        assert_eq!(heard.skipped(), 0.0);
    }

    #[test]
    fn no_more_than_its_most_is_dropped_and_a_silent_track_still_plays() {
        let silent = vec![0.0; 80];
        let (source, heard) = Quiet::new(stereo(&silent), true);
        assert_eq!(source.count(), (80 - 50) * 2, "five seconds at ten frames a second");
        assert!((heard.skipped() - LEAD_MOST).abs() < 1e-9);
    }
}
