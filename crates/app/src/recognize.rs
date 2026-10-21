//! Naming the song that plays nearby, as Opentune does after SongRec
//! (https://github.com/marin-m/SongRec, GPL-3.0), which worked the format out: the microphone's
//! sound becomes a fingerprint, the strongest peaks of its spectrum in four bands packed into
//! Shazam's own binary signature, and only that is asked about, never the recording.
//!
//! 16 kHz mono goes in; a 2048-point FFT every 128 samples; each frame's peaks spread across
//! neighbouring bins and frames, then kept only where a bin stands above everything around it
//! in both frequency and time.

use std::collections::BTreeMap;
use std::f64::consts::PI;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const WINDOW: usize = 2048;
const BINS: usize = 1025;
const STEP: usize = 128;
const RING: usize = 256;
const FORMER: [isize; 3] = [-1, -3, -6];
const NEIGHBOURS: [isize; 8] = [-10, -7, -4, -3, 1, 2, 5, 8];
/// The frames a peak has to stand above as well, back and ahead of it in the ring.
const OTHER_FRAMES: [isize; 14] = [-53, -45, 165, 172, 179, 186, 193, 200, 214, 221, 228, 235, 242, 249];
/// When a lookup is made, by seconds heard, the first answer ending it.
pub const ASK_AT: [f64; 4] = [4.0, 7.0, 10.0, 13.0];

struct Peak {
    pass: u32,
    magnitude: u16,
    bin: u16,
}

/// A fingerprint as the sound comes in.
pub struct Signature {
    samples: Vec<i16>,
    samples_at: usize,
    ffts: Vec<[f32; BINS]>,
    fft_at: usize,
    spread: Vec<[f32; BINS]>,
    spread_at: usize,
    spread_written: usize,
    /// How many samples have gone in.
    pub sample_count: usize,
    peaks: BTreeMap<u8, Vec<Peak>>,
    pending: Vec<i16>,
    hann: Vec<f64>,
}

impl Default for Signature {
    fn default() -> Self {
        Self {
            samples: vec![0; WINDOW],
            samples_at: 0,
            ffts: vec![[0.0; BINS]; RING],
            fft_at: 0,
            spread: vec![[0.0; BINS]; RING],
            spread_at: 0,
            spread_written: 0,
            sample_count: 0,
            peaks: BTreeMap::new(),
            pending: Vec::with_capacity(STEP),
            // numpy's hanning(2050) without its two zero ends.
            hann: (0..WINDOW).map(|index| 0.5 - 0.5 * (2.0 * PI * (index + 1) as f64 / 2049.0).cos()).collect(),
        }
    }
}

impl Signature {
    /// Adds 16 kHz mono samples; any short of a whole step wait for the next.
    pub fn add(&mut self, pcm: &[i16]) {
        for sample in pcm {
            self.pending.push(*sample);
            if self.pending.len() == STEP {
                for (index, sample) in self.pending.drain(..).enumerate() {
                    self.samples[(self.samples_at + index) % WINDOW] = sample;
                }
                self.samples_at = (self.samples_at + STEP) % WINDOW;
                self.sample_count += STEP;
                self.fft();
                self.spread_peaks();
                if self.spread_written >= 46 { self.find_peaks(); }
            }
        }
    }

    /// How long it has heard, in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        self.sample_count as u64 * 1000 / 16_000
    }

    fn fft(&mut self) {
        let mut re: Vec<f64> = (0..WINDOW).map(|index| f64::from(self.samples[(self.samples_at + index) % WINDOW]) * self.hann[index]).collect();
        let mut im = vec![0.0; WINDOW];
        transform(&mut re, &mut im);
        let out = &mut self.ffts[self.fft_at];
        for bin in 0..BINS {
            out[bin] = (((re[bin] * re[bin] + im[bin] * im[bin]) / f64::from(1 << 17)) as f32).max(1e-10);
        }
        self.fft_at = (self.fft_at + 1) % RING;
    }

    fn ring(at: usize, offset: isize) -> usize {
        (at as isize + offset).rem_euclid(RING as isize) as usize
    }

    fn spread_peaks(&mut self) {
        let origin = self.ffts[Self::ring(self.fft_at, -1)];
        self.spread[self.spread_at] = origin;
        for bin in 0..BINS {
            let row = &mut self.spread[self.spread_at];
            if bin < BINS - 2 { row[bin] = row[bin].max(row[bin + 1].max(row[bin + 2])); }
            let mut top = row[bin];
            for back in FORMER {
                let former = &mut self.spread[Self::ring(self.spread_at, back)];
                top = former[bin].max(top);
                former[bin] = top;
            }
        }
        self.spread_at = (self.spread_at + 1) % RING;
        self.spread_written += 1;
    }

    fn find_peaks(&mut self) {
        let f46 = self.ffts[Self::ring(self.fft_at, -46)];
        let s49 = self.spread[Self::ring(self.spread_at, -49)];
        let level = |value: f32| value.max(1.0 / 64.0).ln() * 1477.3 + 6144.0;
        for bin in 10..1015 {
            let value = f46[bin];
            if value < 1.0 / 64.0 || value < s49[bin - 1] { continue; }
            let near = NEIGHBOURS.iter().fold(0.0f32, |top, offset| s49[(bin as isize + offset) as usize].max(top));
            if value <= near { continue; }
            let around = OTHER_FRAMES.iter().fold(near, |top, offset| self.spread[Self::ring(self.spread_at, *offset)][bin - 1].max(top));
            if value <= around { continue; }
            let pass = (self.spread_written - 46) as u32;
            let magnitude = level(value);
            let (before, after) = (level(f46[bin - 1]), level(f46[bin + 1]));
            let first = magnitude * 2.0 - before - after;
            if first <= 0.0 { continue; }
            let corrected = bin as f32 * 64.0 + (after - before) * 32.0 / first;
            let hz = corrected * (16_000.0 / 2.0 / 1024.0 / 64.0);
            let band = match hz {
                hz if hz > 250.0 && hz < 520.0 => 0,
                hz if hz > 520.0 && hz < 1450.0 => 1,
                hz if hz > 1450.0 && hz < 3500.0 => 2,
                hz if hz > 3500.0 && hz <= 5500.0 => 3,
                _ => continue,
            };
            self.peaks.entry(band).or_default().push(Peak { pass, magnitude: magnitude as u16, bin: corrected as u16 });
        }
    }

    /// The fingerprint in Shazam's binary format.
    pub fn encode(&self) -> Vec<u8> {
        let mut contents = Vec::new();
        for (band, peaks) in &self.peaks {
            let mut body = Vec::new();
            let mut pass = 0u32;
            for peak in peaks {
                if peak.pass - pass >= 255 {
                    body.push(0xFF);
                    body.extend(peak.pass.to_le_bytes());
                    pass = peak.pass;
                }
                body.push((peak.pass - pass) as u8);
                body.extend(peak.magnitude.to_le_bytes());
                body.extend(peak.bin.to_le_bytes());
                pass = peak.pass;
            }
            contents.extend((0x6003_0040u32 + u32::from(*band)).to_le_bytes());
            contents.extend((body.len() as u32).to_le_bytes());
            let padding = (4 - body.len() % 4) % 4;
            contents.extend(body);
            contents.extend(std::iter::repeat_n(0u8, padding));
        }
        let size = (contents.len() + 8) as u32;
        let mut out = Vec::with_capacity(48 + contents.len());
        // The 48-byte header; the checksum goes in once the rest is known.
        for word in [0xCAFE_2580u32, 0, size, 0x9411_9C00, 0, 0, 0, 3 << 27, 0, 0,
            (self.sample_count as f64 + 16_000.0 * 0.24) as u32, (15 << 19) + 0x40000, 0x4000_0000, size] {
            out.extend(word.to_le_bytes());
        }
        out.extend(contents);
        let crc = crc32(&out[8..]);
        out[4..8].copy_from_slice(&crc.to_le_bytes());
        out
    }
}

/// In-place radix-2 FFT.
fn transform(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let angle = -2.0 * PI / len as f64;
        let (wr, wi) = (angle.cos(), angle.sin());
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0, 0.0);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let ar = re[b] * cr - im[b] * ci;
                let ai = re[b] * ci + im[b] * cr;
                re[b] = re[a] - ar;
                im[b] = im[a] - ai;
                re[a] += ar;
                im[a] += ai;
                let next = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = next;
            }
        }
        len <<= 1;
    }
}

/// The CRC-32 of zip and PNG, which the signature's header carries.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 { crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 }; }
    }
    !crc
}

/// A song named from what was heard.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Recognized {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub cover: Option<String>,
    /// When, in seconds since 1970.
    pub at: i64,
}

/// The song in Shazam's answer, when it knew it.
fn parse(answer: &Value, at: i64) -> Option<Recognized> {
    let track = answer.get("track")?;
    let title = track["title"].as_str().filter(|title| !title.is_empty())?;
    let album = track["sections"].as_array().into_iter().flatten()
        .flat_map(|section| section["metadata"].as_array().into_iter().flatten())
        .find(|entry| entry["title"].as_str().is_some_and(|name| name.eq_ignore_ascii_case("album")))
        .and_then(|entry| entry["text"].as_str()).map(str::to_owned);
    let images = &track["images"];
    let cover = images["coverarthq"].as_str().or(images["coverart"].as_str()).filter(|url| !url.is_empty()).map(str::to_owned);
    Some(Recognized { title: title.into(), artist: track["subtitle"].as_str().unwrap_or_default().into(), album, cover, at })
}

fn uuid() -> String {
    let hex: String = (0..32).map(|_| char::from_digit(fastrand::u32(..16), 16).unwrap().to_ascii_uppercase()).collect();
    format!("{}-{}-{}-{}-{}", &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..])
}

/// Asks Shazam about `signature`, as its own app does; none when it does not know the song.
pub async fn lookup(youtube: &surco_youtube::YouTube, signature: &[u8], duration_ms: u64, now: i64) -> Result<Option<Recognized>, String> {
    use base64::Engine as _;
    let uri = format!("data:audio/vnd.shazam.sig;base64,{}", base64::engine::general_purpose::STANDARD.encode(signature));
    let body = json!({
        "timezone": "UTC",
        "signature": { "uri": uri, "samplems": duration_ms },
        "timestamp": now * 1000,
        "context": {},
        "geolocation": {},
    });
    let url = format!("https://amp.shazam.com/discovery/v5/en/US/android/-/tag/{}/{}?sync=true&webv3=true&sampling=true&connected=&shazamapiversion=v3&sharehub=true&hubv5minorversion=v5.1&hidelb=true&video=v3", uuid(), uuid());
    let answer = youtube.http().post(url).header("User-Agent", "Dalvik/2.1.0 (Linux; U; Android 14; Pixel 8 Build/UQ1A.240205.004)").header("Content-Language", "en_US")
        .json(&body).send().await.map_err(|error| error.to_string())?;
    let status = answer.status();
    if !status.is_success() { return Err(format!("Shazam answered {status}")); }
    let answer: Value = answer.json().await.map_err(|error| error.to_string())?;
    Ok(parse(&answer, now))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fft_finds_a_tone_in_its_bin() {
        let mut re: Vec<f64> = (0..64).map(|index| (2.0 * PI * 5.0 * index as f64 / 64.0).cos()).collect();
        let mut im = vec![0.0; 64];
        transform(&mut re, &mut im);
        let loudest = (0..32).max_by(|a, b| (re[*a].hypot(im[*a])).total_cmp(&re[*b].hypot(im[*b]))).unwrap();
        assert_eq!(loudest, 5);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926, "the standard check value");
    }

    #[test]
    fn a_tone_heard_becomes_peaks_in_its_band_and_a_signature_with_its_header() {
        let mut signature = Signature::default();
        // Three seconds of 1 kHz pulses, band 1: a steady tone stands above nothing around it
        // in time, and gives no peak at all.
        let pulse = |index: usize| index % 6_400 < 1_600;
        let tone: Vec<i16> = (0..48_000).map(|index| if pulse(index) { ((2.0 * PI * 1000.0 * index as f64 / 16_000.0).sin() * 12_000.0) as i16 } else { 0 }).collect();
        signature.add(&tone);
        assert_eq!(signature.duration_ms(), 3000);
        assert!(signature.peaks.values().map(Vec::len).sum::<usize>() > 0);
        assert!(signature.peaks.keys().all(|band| *band == 1), "{:?}", signature.peaks.keys().collect::<Vec<_>>());
        let encoded = signature.encode();
        assert_eq!(&encoded[..4], &0xCAFE_2580u32.to_le_bytes());
        assert_eq!(u32::from_le_bytes(encoded[4..8].try_into().unwrap()), crc32(&encoded[8..]));
        // 48 bytes of header, 8 of the container, then the band with its own tag.
        assert_eq!(&encoded[56..60], &0x6003_0041u32.to_le_bytes(), "the band's own tag");
    }

    #[test]
    fn shazams_answer_names_the_song_its_artist_album_and_cover() {
        let answer = json!({"track": {"title": "Persiana americana", "subtitle": "Soda Stereo",
            "images": {"coverart": "https://c/1.jpg", "coverarthq": "https://c/hq.jpg"},
            "sections": [{"metadata": [{"title": "Album", "text": "Signos"}, {"title": "Label", "text": "Sony"}]}]}});
        let found = parse(&answer, 7).unwrap();
        assert_eq!((found.title.as_str(), found.artist.as_str(), found.album.as_deref(), found.cover.as_deref()),
            ("Persiana americana", "Soda Stereo", Some("Signos"), Some("https://c/hq.jpg")));
        assert!(parse(&json!({"matches": []}), 7).is_none(), "not known");
    }
}
