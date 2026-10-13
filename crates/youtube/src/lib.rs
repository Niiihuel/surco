//! YouTube Music as a catalog and as a source of audio.
//!
//! Search goes through `ytmapi-rs`. Streams come from our own InnerTube `player` request
//! (see [`stream`]): no crate tried on 2026-10-12 returned URLs that play past the first minute.

mod stream;

use dnm_core::{Album, AlbumRef, Artist, ArtistRef, Found, Track};
use ytmapi_rs::auth::noauth::NoAuthToken;
use ytmapi_rs::common::{AlbumID, ArtistChannelID, PlaylistID, Thumbnail, VideoID, YoutubeID};
use ytmapi_rs::parse::{ParsedSongArtist, PlaylistItem};

pub use stream::{Audio, Stream};

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

type Music = ytmapi_rs::YtMusic<NoAuthToken>;

pub struct YouTube {
    /// Built on first use: it asks the service who we are, and the app has to open and play
    /// what it already holds without a network.
    music: tokio::sync::OnceCell<Music>,
    http: reqwest::Client,
}

impl YouTube {
    /// Anonymous: nothing here needs an account. Nothing is sent until the first request.
    pub fn new() -> Result<Self> {
        Ok(Self { music: tokio::sync::OnceCell::new(), http: reqwest::Client::builder().build()? })
    }

    async fn music(&self) -> Result<&Music> {
        self.music.get_or_try_init(|| async { Ok(ytmapi_rs::YtMusicBuilder::new_rustls_tls().build().await?) }).await
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    /// Songs, artists and albums for one query. Only the songs are required: a search that
    /// finds them still answers when one of the other two fails.
    pub async fn search(&self, query: &str) -> Result<Found> {
        let music = self.music().await?;
        let (songs, artists, albums) = tokio::join!(music.search_songs(query), music.search_artists(query), music.search_albums(query));
        let tracks = songs?
            .into_iter()
            .map(|song| Track {
                id: song.video_id.get_raw().to_owned(),
                title: song.title,
                artist: song.artist,
                // Song results carry the artist as a name only.
                artist_id: None,
                album_id: song.album.as_ref().map(|album| album.id.get_raw().to_owned()),
                album: song.album.map(|album| album.name),
                artwork_url: artwork(song.thumbnails),
                duration_ms: duration_ms(&song.duration),
            })
            .collect();
        let artists = artists
            .unwrap_or_default()
            .into_iter()
            .map(|artist| ArtistRef {
                id: artist.browse_id.get_raw().to_owned(),
                name: artist.artist,
                subscribers: artist.subscribers,
                artwork_url: artwork(artist.thumbnails),
            })
            .collect();
        let albums = albums
            .unwrap_or_default()
            .into_iter()
            .map(|album| AlbumRef {
                id: album.album_id.get_raw().to_owned(),
                title: album.title,
                artist: Some(album.artist),
                year: album.year,
                artwork_url: artwork(album.thumbnails),
            })
            .collect();
        Ok(Found { artists, albums, tracks })
    }

    /// The id of the artist a name most likely means, for a track that names its artist
    /// without linking to it.
    pub async fn find_artist(&self, name: &str) -> Result<Option<String>> {
        let artists = self.music().await?.search_artists(name).await?;
        Ok(artists.into_iter().next().map(|artist| artist.browse_id.get_raw().to_owned()))
    }

    pub async fn artist(&self, id: &str) -> Result<Artist> {
        let artist = self.music().await?.get_artist(ArtistChannelID::from_raw(id)).await?;
        let releases = artist.top_releases;
        let release = |album: ytmapi_rs::parse::AlbumResult| AlbumRef {
            id: album.album_id.get_raw().to_owned(),
            title: album.title,
            artist: None,
            year: album.year,
            artwork_url: artwork(album.thumbnails),
        };
        let songs = match releases.songs {
            Some(songs) => {
                // The artist page lists its songs with neither cover nor duration; the playlist
                // behind "all songs" has both, and more of them.
                let full = self.playlist(songs.browse_id).await.unwrap_or_default();
                if full.is_empty() {
                    songs
                        .results
                        .into_iter()
                        .map(|song| Track {
                            id: song.video_id.get_raw().to_owned(),
                            title: song.title,
                            artist: names(&song.artists),
                            artist_id: Some(id.to_owned()),
                            album_id: Some(song.album.id.get_raw().to_owned()),
                            album: Some(song.album.name),
                            artwork_url: None,
                            duration_ms: 0,
                        })
                        .collect()
                } else {
                    full
                }
            }
            None => Vec::new(),
        };
        Ok(Artist {
            id: id.to_owned(),
            name: artist.name,
            subscribers: artist.subscribers,
            artwork_url: artwork(artist.thumbnails),
            songs,
            albums: releases.albums.map(|albums| albums.results.into_iter().map(release).collect()).unwrap_or_default(),
            singles: releases.singles.map(|singles| singles.results.into_iter().map(release).collect()).unwrap_or_default(),
        })
    }

    pub async fn album(&self, id: &str) -> Result<Album> {
        let album = self.music().await?.get_album(AlbumID::from_raw(id)).await?;
        let artist = names(&album.artists);
        let artist_id = album.artists.iter().find_map(|artist| artist.id.as_ref()).map(|id| id.get_raw().to_owned());
        let artwork_url = artwork(album.thumbnails);
        let tracks = album
            .tracks
            .into_iter()
            .map(|song| Track {
                id: song.video_id.get_raw().to_owned(),
                title: song.title,
                artist: artist.clone(),
                artist_id: artist_id.clone(),
                album: Some(album.title.clone()),
                album_id: Some(id.to_owned()),
                artwork_url: artwork_url.clone(),
                duration_ms: duration_ms(&song.duration),
            })
            .collect();
        Ok(Album { id: id.to_owned(), title: album.title, artist, artist_id, year: album.year, artwork_url, tracks })
    }

    async fn playlist(&self, id: PlaylistID<'_>) -> Result<Vec<Track>> {
        let items = self.music().await?.get_playlist_tracks(id).await?;
        Ok(items
            .into_iter()
            .filter_map(|item| match item {
                PlaylistItem::Song(song) if song.is_available => Some(Track {
                    id: song.video_id.get_raw().to_owned(),
                    title: song.title,
                    artist: names(&song.artists),
                    artist_id: song.artists.iter().find_map(|artist| artist.id.as_ref()).map(|id| id.get_raw().to_owned()),
                    album_id: Some(song.album.id.get_raw().to_owned()),
                    album: Some(song.album.name),
                    artwork_url: artwork(song.thumbnails),
                    duration_ms: duration_ms(&song.duration),
                }),
                _ => None,
            })
            .collect())
    }

    /// What YouTube Music would play after this track: more of its artist and album, with
    /// other artists in between. The track itself usually comes first.
    pub async fn radio(&self, video_id: &str) -> Result<Vec<Track>> {
        let tracks = self.music().await?.get_watch_playlist_from_video_id(VideoID::from_raw(video_id)).await?;
        Ok(tracks
            .into_iter()
            .map(|track| Track {
                id: track.video_id.get_raw().to_owned(),
                title: track.title,
                artist: track.author,
                artist_id: None,
                album: None,
                album_id: None,
                artwork_url: artwork(track.thumbnails),
                duration_ms: duration_ms(&track.duration),
            })
            .collect())
    }

    /// Playable audio for a video id. The URLs expire; resolve again rather than keep them.
    pub async fn stream(&self, video_id: &str) -> Result<Stream> {
        stream::resolve(&self.http, video_id).await
    }
}

/// The largest rendition on offer.
fn artwork(thumbnails: Vec<Thumbnail>) -> Option<String> {
    thumbnails.into_iter().max_by_key(|thumbnail| thumbnail.width).map(|thumbnail| thumbnail.url)
}

fn names(artists: &[ParsedSongArtist]) -> String {
    artists.iter().map(|artist| artist.name.as_str()).collect::<Vec<_>>().join(", ")
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
