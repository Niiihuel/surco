//! What is sounding, in four bands from low to high, for a level display.

use std::sync::atomic::{AtomicU32, Ordering};

use crate::equalizer::Biquad;

pub const BANDS: usize = 4;

static LEVELS: [AtomicU32; BANDS] = [const { AtomicU32::new(0) }; BANDS];

/// The peak of each band over the last span that played, from 0 to 1. A deck that stops
/// leaves its last levels here: whoever shows them knows whether anything plays.
pub fn levels() -> [f32; BANDS] {
    std::array::from_fn(|band| f32::from_bits(LEVELS[band].load(Ordering::Relaxed)))
}

/// The middle of each band, in hertz, and how wide it listens.
const CENTRES: [f32; BANDS] = [90.0, 450.0, 2_000.0, 8_000.0];
const Q: f32 = 0.9;
/// How much each band is lifted before it is compared with the others: music carries far
/// less in its highs than in its lows.
const LIFT: [f32; BANDS] = [1.0, 1.6, 2.5, 4.0];
/// What the loudest recent peak keeps of itself from one span to the next: it halves in
/// about a second and a half, so the bars fill their height on a quiet track as on a loud one.
const CEILING_KEPT: f32 = 0.99;
/// A ceiling never falls under this, or the hiss of a silence would stand as tall as a song.
const QUIET: f32 = 0.02;

/// Splits what a source plays into bands and publishes their peaks every 20 ms.
pub(crate) struct Meter {
    filters: [Biquad; BANDS],
    peaks: [f32; BANDS],
    /// The loudest lifted peak of late, which stands for full height.
    ceiling: f32,
    /// Only the first channel is measured: a filter's history is one signal's.
    channels: usize,
    turn: usize,
    span: usize,
    fed: usize,
}

impl Meter {
    pub(crate) fn new(sample_rate: f32, channels: usize) -> Self {
        Self {
            filters: CENTRES.map(|hz| Biquad::band_pass(hz, sample_rate, Q)),
            peaks: [0.0; BANDS],
            ceiling: QUIET,
            channels: channels.max(1),
            turn: 0,
            span: (sample_rate as usize / 50).max(1),
            fed: 0,
        }
    }

    /// Takes the next sample of an interleaved stream.
    #[inline]
    pub(crate) fn feed(&mut self, sample: f32) {
        let turn = self.turn;
        self.turn = (turn + 1) % self.channels;
        if turn != 0 {
            return;
        }
        for (peak, filter) in self.peaks.iter_mut().zip(&mut self.filters) {
            *peak = peak.max(filter.process(sample).abs());
        }
        self.fed += 1;
        if self.fed >= self.span {
            for (level, shown) in LEVELS.iter().zip(self.reading()) {
                level.store(shown.to_bits(), Ordering::Relaxed);
            }
        }
    }

    /// The peaks since the last reading as shares of the recent ceiling, and a fresh start.
    fn reading(&mut self) -> [f32; BANDS] {
        let lifted: [f32; BANDS] = std::array::from_fn(|band| self.peaks[band] * LIFT[band]);
        self.ceiling = lifted.iter().fold((self.ceiling * CEILING_KEPT).max(QUIET), |ceiling, peak| ceiling.max(*peak));
        self.peaks = [0.0; BANDS];
        self.fed = 0;
        lifted.map(|peak| peak / self.ceiling)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The levels a sine at `hz` settles on, measured on a meter of its own.
    fn of(hz: f32, amplitude: f32) -> [f32; BANDS] {
        let rate = 48_000.0;
        let mut meter = Meter::new(rate, 2);
        meter.span = usize::MAX;
        for n in 0..96_000 {
            if n == 48_000 {
                // Past the filters' start-up, which rings in every band.
                meter.reading();
                meter.ceiling = QUIET;
            }
            let sample = amplitude * (std::f32::consts::TAU * hz * n as f32 / rate).sin();
            meter.feed(sample);
            meter.feed(-sample);
        }
        meter.reading()
    }

    #[test]
    fn a_tone_shows_in_its_own_band_and_silence_in_none() {
        let low = of(80.0, 0.5);
        assert!(low[0] == 1.0 && low[1] < 0.5 && low[3] < 0.1, "{low:?}");
        let high = of(9_000.0, 0.5);
        assert!(high[3] == 1.0 && high[0] < 0.1, "{high:?}");
        assert_eq!(of(440.0, 0.0), [0.0; BANDS]);
        let faint = of(450.0, 0.004);
        assert!(faint[1] < 0.4, "a whisper does not stand as tall as a song: {faint:?}");
    }

    #[test]
    fn full_height_follows_how_loud_the_track_has_been() {
        let mut meter = Meter::new(48_000.0, 1);
        meter.peaks = [0.8, 0.0, 0.0, 0.0];
        assert_eq!(meter.reading()[0], 1.0);
        meter.peaks = [0.4, 0.0, 0.0, 0.0];
        let after = meter.reading()[0];
        assert!(after > 0.5 && after < 0.52, "half as loud right after is half as tall: {after}");
        for _ in 0..500 {
            meter.reading();
        }
        meter.peaks = [0.4, 0.0, 0.0, 0.0];
        assert_eq!(meter.reading()[0], 1.0, "ten seconds on, the quieter level is the track's own");
    }
}
