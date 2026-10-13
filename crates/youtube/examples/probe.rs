//! Checks the two things this crate does against the real service:
//! `cargo run -p dnm-youtube --example probe -- "<search text>"`
//! searches, resolves the first result and downloads its best audio stream.

use std::time::Instant;

#[tokio::main]
async fn main() -> dnm_youtube::Result<()> {
    let query = std::env::args().nth(1).unwrap_or_else(|| "apocalypse cigarettes after sex".into());
    let youtube = dnm_youtube::YouTube::new().await?;

    let started = Instant::now();
    let tracks = youtube.search(&query).await?;
    println!("search {query:?}: {} tracks in {:?}", tracks.len(), started.elapsed());
    for track in tracks.iter().take(5) {
        println!(
            "  {} | {} | {} | {} | {} ms | artwork {}",
            track.id,
            track.title,
            track.artist,
            track.album.as_deref().unwrap_or("-"),
            track.duration_ms,
            track.artwork_url.as_deref().map_or("no", |_| "yes"),
        );
    }
    let first = tracks.first().ok_or("no results")?;

    let started = Instant::now();
    let stream = youtube.stream(&first.id).await?;
    println!("stream {}: {} s, {} formats in {:?}", first.id, stream.duration_seconds, stream.audio.len(), started.elapsed());
    for audio in &stream.audio {
        println!("  itag {} | {} | {} bps | {} bytes", audio.itag, audio.mime, audio.average_bitrate, audio.size);
    }
    let best = stream.best().ok_or("no audio")?;

    // A plain GET is throttled to a crawl; a Range request is not.
    let started = Instant::now();
    let response = youtube.http().get(&best.url).header("Range", "bytes=0-").send().await?;
    let status = response.status();
    let bytes = response.bytes().await?;
    println!(
        "download itag {}: HTTP {status}, {} of {} bytes in {:?}",
        best.itag,
        bytes.len(),
        best.size,
        started.elapsed()
    );
    Ok(())
}
