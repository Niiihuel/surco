//! Exercise the exact Rust provider with public metadata, printing no lyric text.
//! `cargo run -p surco --example lyrics_probe -- "Sunsetz" "Cigarettes After Sex" 215000`
//! `cargo run -p surco --example lyrics_probe -- --youtube eVRbaYq5Aw4` skips LRCLIB.

#[path = "../src/backend/lyrics.rs"]
mod lyrics;

#[tokio::main]
async fn main() -> surco_youtube::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let youtube = surco_youtube::YouTube::new()?;
    if args.get(1).is_some_and(|arg| arg == "--youtube") {
        let id = args.get(2).map(String::as_str).unwrap_or("eVRbaYq5Aw4");
        let (words, source) = tokio::time::timeout(std::time::Duration::from_secs(8), youtube.lyrics(id)).await??;
        println!("youtube plain lines={} chars={} source={source}", words.lines().count(), words.chars().count());
        return Ok(());
    }
    let track = surco_core::Track {
        id: args.get(4).cloned().unwrap_or_else(|| "eVRbaYq5Aw4".into()), title: args.get(1).cloned().unwrap_or_else(|| "Sunsetz".into()),
        artist: args.get(2).cloned().unwrap_or_else(|| "Cigarettes After Sex".into()),
        duration_ms: args.get(3).and_then(|value| value.parse().ok()).unwrap_or(215000),
        artist_id: None, album: None, album_id: None, artwork_url: None,
    };
    match lyrics::Provider::default().fetch(&youtube, &track).await {
        Ok(Some(lyrics::Found::Synced(lines))) => println!("synced lines={} first_at_ms={}", lines.len(), lines[0].at_ms),
        Ok(Some(lyrics::Found::Plain { lines, source })) => println!("plain lines={} source={source}", lines.len()),
        Ok(None) => println!("catalog miss: successful response with no usable lyrics"),
        Err(failure) => {
            eprintln!("provider failure: {} retry_after_seconds={}", failure.message, failure.retry_after.as_secs());
            std::process::exit(2);
        }
    }
    Ok(())
}
