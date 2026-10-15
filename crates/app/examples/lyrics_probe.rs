//! Exercise the exact Rust provider with public metadata, printing no lyric text.
//! `cargo run -p surco --example lyrics_probe -- "Sunsetz" "Cigarettes After Sex" 215000`

#[path = "../src/backend/lyrics.rs"]
mod lyrics;

#[tokio::main]
async fn main() -> surco_youtube::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let track = surco_core::Track {
        id: "probe".into(), title: args.get(1).cloned().unwrap_or_else(|| "Sunsetz".into()),
        artist: args.get(2).cloned().unwrap_or_else(|| "Cigarettes After Sex".into()),
        duration_ms: args.get(3).and_then(|value| value.parse().ok()).unwrap_or(215000),
        artist_id: None, album: None, album_id: None, artwork_url: None,
    };
    let youtube = surco_youtube::YouTube::new()?;
    match lyrics::Provider::default().fetch(&youtube, &track).await {
        Ok(Some(lyrics::Found::Synced(lines))) => println!("synced lines={} first_at_ms={}", lines.len(), lines[0].at_ms),
        Ok(Some(lyrics::Found::Plain(lines))) => println!("plain lines={}", lines.len()),
        Ok(None) => println!("catalog miss: successful response with no usable lyrics"),
        Err(failure) => {
            eprintln!("provider failure: {} retry_after_seconds={}", failure.message, failure.retry_after.as_secs());
            std::process::exit(2);
        }
    }
    Ok(())
}
