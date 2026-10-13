//! Checks the two things this crate does against the real service:
//! `cargo run -p dnm-youtube --example probe -- "<search text>"`
//! searches, asks for the radio of the first result, resolves it and downloads its best audio stream.

use std::time::Instant;

#[tokio::main]
async fn main() -> dnm_youtube::Result<()> {
    let query = std::env::args().nth(1).unwrap_or_else(|| "apocalypse cigarettes after sex".into());
    let youtube = dnm_youtube::YouTube::new()?;

    let started = Instant::now();
    let found = youtube.search(&query).await?;
    let tracks = found.tracks;
    println!("search {query:?}: {} tracks, {} artists, {} albums in {:?}", tracks.len(), found.artists.len(), found.albums.len(), started.elapsed());
    if let Some(reference) = found.artists.first() {
        let started = Instant::now();
        let artist = youtube.artist(&reference.id).await?;
        println!(
            "artist {} ({:?}, artwork {}): {} songs, {} albums, {} singles in {:?}",
            artist.name,
            artist.subscribers,
            artist.artwork_url.as_deref().map_or("no", |_| "yes"),
            artist.songs.len(),
            artist.albums.len(),
            artist.singles.len(),
            started.elapsed()
        );
        for song in artist.songs.iter().take(3) {
            println!("  song {} | {} | {} ms | artwork {}", song.title, song.album.as_deref().unwrap_or("-"), song.duration_ms, song.artwork_url.as_deref().map_or("no", |_| "yes"));
        }
        for album in artist.albums.iter().take(3) {
            println!("  album {} | {} | artwork {}", album.title, album.year, album.artwork_url.as_deref().map_or("no", |_| "yes"));
        }
    }
    if let Some(reference) = found.albums.first() {
        let started = Instant::now();
        let album = youtube.album(&reference.id).await?;
        println!("album {} by {} ({:?}), {}: {} tracks in {:?}", album.title, album.artist, album.artist_id, album.year, album.tracks.len(), started.elapsed());
        for track in album.tracks.iter().take(3) {
            println!("  track {} | {} ms", track.title, track.duration_ms);
        }
    }
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
    let radio = youtube.radio(&first.id).await?;
    println!("radio of {}: {} tracks in {:?}", first.title, radio.len(), started.elapsed());
    for track in radio.iter().take(12) {
        println!("  {} | {} | {} ms | artwork {}", track.title, track.artist, track.duration_ms, track.artwork_url.as_deref().map_or("no", |_| "yes"));
    }

    let started = Instant::now();
    let stream = youtube.stream(&first.id).await?;
    println!("loudness {:?} dB over the reference", stream.best().and_then(|audio| audio.loudness_db));
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
