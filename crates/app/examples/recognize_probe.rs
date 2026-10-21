//! Checks the fingerprint against Shazam with a real song, played from its own audio instead of
//! a microphone. Prints only what Shazam named.
//! `cargo run -p surco --example recognize_probe -- "Soda Stereo De música ligera" 60`

#[allow(dead_code)]
#[path = "../src/recognize.rs"]
mod recognize;

use rodio::Source;

#[tokio::main]
async fn main() -> surco_youtube::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let query = args.get(1).cloned().unwrap_or_else(|| "Soda Stereo De música ligera".into());
    let from: f64 = args.get(2).and_then(|value| value.parse().ok()).unwrap_or(60.0);
    let youtube = surco_youtube::YouTube::new()?;
    let track = youtube.songs(&query).await?.into_iter().next().ok_or("no song")?;
    println!("song: {} — {}", track.title, track.artist);
    let stream = youtube.stream(&track.id).await?;
    let audio = stream.audio.iter().find(|audio| audio.mime.contains("mp4")).ok_or("no AAC")?;
    let bytes = youtube.bytes(&audio.url, 0, audio.size - 1).await?;
    let source = rodio::Decoder::builder().with_data(std::io::Cursor::new(bytes)).with_byte_len(audio.size).build().map_err(|error| error.to_string())?;
    let (channels, rate) = (usize::from(source.channels().get()), source.sample_rate().get());
    // Mono, then 16 kHz, each sample the mean of the input it spans.
    let mono: Vec<f32> = source.collect::<Vec<f32>>().chunks(channels).map(|frame| frame.iter().sum::<f32>() / channels as f32).collect();
    let step = f64::from(rate) / 16_000.0;
    let pcm: Vec<i16> = (0..(mono.len() as f64 / step) as usize).map(|index| {
        let (start, end) = ((index as f64 * step) as usize, (((index + 1) as f64 * step) as usize).min(mono.len()));
        let mean = mono[start..end].iter().sum::<f32>() / (end - start).max(1) as f32;
        (mean.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16
    }).collect();
    let mut signature = recognize::Signature::default();
    let start = (from * 16_000.0) as usize;
    let mut asked = 0;
    for chunk in pcm[start..].chunks(1_600) {
        signature.add(chunk);
        let heard = signature.duration_ms() as f64 / 1000.0;
        if asked < recognize::ASK_AT.len() && heard >= recognize::ASK_AT[asked] {
            asked += 1;
            let found = recognize::lookup(&youtube, &signature.encode(), signature.duration_ms(), 0).await?;
            println!("after {heard:.0} s, {} bytes: {:?}", signature.encode().len(), found.map(|found| format!("{} — {} ({:?})", found.title, found.artist, found.album)));
        }
        if asked == recognize::ASK_AT.len() { break; }
    }
    Ok(())
}
