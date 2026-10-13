//! Looks for the video that goes with a song and lists what pictures it offers:
//! `cargo run -p dnm-youtube --example video_probe -- <video id>`

use ytmapi_rs::common::{VideoID, YoutubeID};
use ytmapi_rs::query::GetWatchPlaylistQuery;

#[tokio::main]
async fn main() -> dnm_youtube::Result<()> {
    let id = std::env::args().nth(1).unwrap_or_else(|| "DdI598gKkKw".into());
    let music = ytmapi_rs::YtMusicBuilder::new_rustls_tls().build().await?;
    let raw = music.raw_json_query(GetWatchPlaylistQuery::new_from_video_id(VideoID::from_raw(&id))).await?;
    println!("watch playlist: {} bytes, counterpart mentions: {}", raw.len(), raw.matches("counterpart").count());
    let json: serde_json::Value = serde_json::from_str(&raw)?;
    let mut found = Vec::new();
    walk(&json, &mut found);
    for (kind, video) in found.iter().take(6) {
        println!("  {kind}: {video}");
    }
    let query = std::env::args().nth(2).unwrap_or_else(|| "Cigarettes After Sex Apocalypse".into());
    let videos = music.search_videos(query.as_str()).await?;
    println!("videos for {query:?}: {}", videos.len());
    for video in videos.iter().take(5) {
        println!("  {video:?}");
    }
    let youtube = dnm_youtube::YouTube::new()?;
    for (label, video_id) in std::iter::once(("song".to_owned(), id.clone())).chain(found.iter().filter(|(kind, _)| kind == "counterpart").take(1).cloned()) {
        let stream = youtube.stream(&video_id).await?;
        println!("{label} {video_id}: {} s, {} pictures", stream.duration_seconds, stream.video.len());
        for video in &stream.video {
            println!("  itag {} | {} | {}x{} | {} bytes | init {:?} index {:?}", video.itag, video.mime, video.width, video.height, video.size, video.init, video.index);
        }
    }
    Ok(())
}

/// Every `counterpart` video id in the answer, and the kind of the first track.
fn walk(value: &serde_json::Value, found: &mut Vec<(String, String)>) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(counterpart) = map.get("counterpart") {
                let text = counterpart.to_string();
                if let Some(at) = text.find("\"videoId\":\"") {
                    found.push(("counterpart".into(), text[at + 11..].split('"').next().unwrap_or_default().to_owned()));
                }
            }
            if let Some(kind) = map.get("musicVideoType").and_then(|kind| kind.as_str()) {
                if found.iter().filter(|(label, _)| label == "kind").count() < 2 {
                    found.push(("kind".into(), kind.to_owned()));
                }
            }
            map.values().for_each(|value| walk(value, found));
        }
        serde_json::Value::Array(items) => items.iter().for_each(|value| walk(value, found)),
        _ => {}
    }
}
