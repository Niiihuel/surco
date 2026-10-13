//! YouTube Music as a catalog and as a source of audio.
//!
//! Search goes through `ytmapi-rs`. Streams come from our own InnerTube `player` request
//! (see [`stream`]): no crate tried on 2026-10-12 returned URLs that play past the first minute.

mod stream;

use dnm_core::Track;
use ytmapi_rs::auth::noauth::NoAuthToken;
use ytmapi_rs::common::{VideoID, YoutubeID};

pub use stream::{Audio, Stream};

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

pub struct YouTube {
    music: ytmapi_rs::YtMusic<NoAuthToken>,
    http: reqwest::Client,
}

impl YouTube {
    /// Anonymous: nothing here needs an account.
    pub async fn new() -> Result<Self> {
        Ok(Self {
            music: ytmapi_rs::YtMusicBuilder::new_rustls_tls().build().await?,
            http: reqwest::Client::builder().build()?,
        })
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    pub async fn search(&self, query: &str) -> Result<Vec<Track>> {
        let songs = self.music.search_songs(query).await?;
        Ok(songs
            .into_iter()
            .map(|song| Track {
                id: song.video_id.get_raw().to_owned(),
                title: song.title,
                artist: song.artist,
                // Song results carry the artist as a name only.
                artist_id: None,
                album_id: song.album.as_ref().map(|album| album.id.get_raw().to_owned()),
                album: song.album.map(|album| album.name),
                artwork_url: song.thumbnails.into_iter().max_by_key(|t| t.width).map(|t| t.url),
                duration_ms: duration_ms(&song.duration),
            })
            .collect())
    }

    /// What YouTube Music would play after this track: more of its artist and album, with
    /// other artists in between. The track itself usually comes first.
    pub async fn radio(&self, video_id: &str) -> Result<Vec<Track>> {
        let tracks = self.music.get_watch_playlist_from_video_id(VideoID::from_raw(video_id)).await?;
        Ok(tracks
            .into_iter()
            .map(|track| Track {
                id: track.video_id.get_raw().to_owned(),
                title: track.title,
                artist: track.author,
                artist_id: None,
                album: None,
                album_id: None,
                artwork_url: track.thumbnails.into_iter().max_by_key(|t| t.width).map(|t| t.url),
                duration_ms: duration_ms(&track.duration),
            })
            .collect())
    }

    /// Playable audio for a video id. The URLs expire; resolve again rather than keep them.
    pub async fn stream(&self, video_id: &str) -> Result<Stream> {
        stream::resolve(&self.http, video_id).await
    }
}

/// `4:51` or `1:02:03` in milliseconds; 0 when the text is something else.
fn duration_ms(text: &str) -> u64 {
    let mut seconds = 0u64;
    for part in text.trim().split(':') {
        match part.parse::<u64>() {
            Ok(n) => seconds = seconds * 60 + n,
            Err(_) => return 0,
        }
    }
    seconds * 1000
}

#[cfg(test)]
mod tests {
    use super::duration_ms;

    #[test]
    fn durations_are_read_from_clock_text() {
        assert_eq!(duration_ms("4:51"), 291_000);
        assert_eq!(duration_ms("1:02:03"), 3_723_000);
        assert_eq!(duration_ms(" 0:07 "), 7_000);
        assert_eq!(duration_ms(""), 0);
        assert_eq!(duration_ms("live"), 0);
    }
}
