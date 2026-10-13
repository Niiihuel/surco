//! YouTube Music as a catalog and as a source of audio.
//!
//! Search goes through `ytmapi-rs`. Streams come from our own InnerTube `player` request
//! (see [`stream`]): no crate tried on 2026-10-12 returned URLs that play past the first minute.

mod stream;

use futures_util::TryStreamExt;
use surco_core::{Album, AlbumRef, Artist, ArtistRef, Found, Track};
use ytmapi_rs::parse::{SearchResultSong, SearchResultVideo};
use ytmapi_rs::auth::noauth::NoAuthToken;
use ytmapi_rs::common::{AlbumID, ArtistChannelID, PlaylistID, Thumbnail, VideoID, YoutubeID};
use ytmapi_rs::parse::{ParsedSongArtist, PlaylistItem};
use ytmapi_rs::query::GetPlaylistTracksQuery;
use ytmapi_rs::query::search::{AlbumsFilter, ArtistsFilter, SearchQuery, SongsFilter, SpellingMode};

pub use stream::{Audio, Stream, Video};

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

    /// A stretch of what an address serves, from one byte to another, both included.
    pub async fn bytes(&self, url: &str, from: u64, to: u64) -> Result<Vec<u8>> {
        let response = self.http.get(url).header("Range", format!("bytes={from}-{to}")).send().await?.error_for_status()?;
        Ok(response.bytes().await?.to_vec())
    }

    /// Songs, artists and albums for one query. A query that finds nothing is tried again as
    /// the service would complete it, which is how a misspelt title still finds its song.
    pub async fn search(&self, query: &str) -> Result<Found> {
        let found = self.find(query).await?;
        if !(found.tracks.is_empty() && found.artists.is_empty() && found.albums.is_empty()) {
            return Ok(found);
        }
        let suggestions = self.music().await?.get_search_suggestions(query).await.unwrap_or_default();
        match suggestions.first().map(|suggestion| suggestion.get_text()).filter(|better| better != query) {
            Some(better) => self.find(&better).await,
            None => Ok(found),
        }
    }

    /// Only the songs are required: a search that finds them still answers when the artists
    /// or the albums fail.
    async fn find(&self, query: &str) -> Result<Found> {
        let music = self.music().await?;
        // As the service itself searches: a misspelt title still finds its song. The library's
        // own default is an exact match, which answers a typo with nothing.
        let (songs, artists, albums) = tokio::join!(
            music.search_songs(SearchQuery::new_filtered(query, SongsFilter).with_spelling_mode(SpellingMode::WithSuggestions)),
            music.search_artists(SearchQuery::new_filtered(query, ArtistsFilter).with_spelling_mode(SpellingMode::WithSuggestions)),
            music.search_albums(SearchQuery::new_filtered(query, AlbumsFilter).with_spelling_mode(SpellingMode::WithSuggestions)),
        );
        let tracks = songs?.into_iter().map(found_song).collect();
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

    /// The songs alone for one query: a song another app names, looked for by that name.
    pub async fn songs(&self, query: &str) -> Result<Vec<Track>> {
        let songs = self.music().await?.search_songs(SearchQuery::new_filtered(query, SongsFilter).with_spelling_mode(SpellingMode::WithSuggestions)).await?;
        Ok(songs.into_iter().map(found_song).collect())
    }

    /// One of the service's playlists by its id: its name, and what it lists in order.
    pub async fn playlist_named(&self, id: &str) -> Result<(String, Vec<Track>)> {
        let id = playlist_browse_id(id);
        let tracks = self.playlist(PlaylistID::from_raw(&id)).await?;
        let name = match album_playlist_name(&id, &tracks) {
            Some(name) => name.to_owned(),
            None => self.music().await?.get_playlist_details(PlaylistID::from_raw(&id)).await?.title,
        };
        Ok((name, tracks))
    }

    /// The id of the artist a name most likely means, for a track that names its artist
    /// without linking to it.
    pub async fn find_artist(&self, name: &str) -> Result<Option<String>> {
        let artists = self.music().await?.search_artists(name).await?;
        Ok(artists.into_iter().next().map(|artist| artist.browse_id.get_raw().to_owned()))
    }

    /// The artist of exactly this name, as a list shows them. A name the catalog only knows
    /// as an uploader's channel finds nobody.
    pub async fn artist_named(&self, name: &str) -> Result<Option<ArtistRef>> {
        let artists = self.music().await?.search_artists(name).await?;
        let wanted = surco_core::discover::plain(name);
        Ok(artists.into_iter().find(|artist| surco_core::discover::plain(&artist.artist) == wanted).map(|artist| ArtistRef {
            id: artist.browse_id.get_raw().to_owned(),
            name: artist.artist,
            subscribers: artist.subscribers,
            artwork_url: artwork(artist.thumbnails),
        }))
    }

    /// An artist as their own page presents them, and what it says of them.
    pub async fn about(&self, id: &str) -> Result<(ArtistRef, Option<String>)> {
        let artist = self.music().await?.get_artist(ArtistChannelID::from_raw(id)).await?;
        let known = ArtistRef { id: id.to_owned(), name: artist.name, subscribers: artist.subscribers, artwork_url: artwork(artist.thumbnails) };
        Ok((known, artist.description))
    }

    /// An artist's albums, each naming them: the page without the songs, which cost a
    /// second request.
    pub async fn albums_of(&self, id: &str) -> Result<Vec<AlbumRef>> {
        Ok(self.releases(id).await?.0)
    }

    /// An artist's albums and their singles, as their page lists them, each naming them.
    pub async fn releases(&self, id: &str) -> Result<(Vec<AlbumRef>, Vec<AlbumRef>)> {
        let artist = self.music().await?.get_artist(ArtistChannelID::from_raw(id)).await?;
        let named = |albums: Option<ytmapi_rs::parse::GetArtistAlbums>| -> Vec<AlbumRef> {
            let albums = albums.map(|albums| albums.results).unwrap_or_default();
            albums
                .into_iter()
                .map(|album| AlbumRef {
                    id: album.album_id.get_raw().to_owned(),
                    title: album.title,
                    artist: Some(artist.name.clone()),
                    year: album.year,
                    artwork_url: artwork(album.thumbnails),
                })
                .collect()
        };
        Ok((named(artist.top_releases.albums), named(artist.top_releases.singles)))
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
        let query = GetPlaylistTracksQuery::new(id);
        let items = self.music().await?.stream(&query).try_concat().await?;
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
                // A list its owner made may hold a video where the catalog has no song.
                PlaylistItem::Video(video) if video.is_available => Some(Track {
                    id: video.video_id.get_raw().to_owned(),
                    title: video.title,
                    artist: video.channel_name,
                    artist_id: None,
                    album: None,
                    album_id: None,
                    artwork_url: artwork(video.thumbnails),
                    duration_ms: duration_ms(&video.duration),
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

    /// A track by its id alone, as a shared link names it.
    pub async fn song(&self, video_id: &str) -> Result<Track> {
        // The radio of a track opens with the track itself.
        let track = self.radio(video_id).await?.into_iter().find(|track| track.id == video_id);
        Ok(track.ok_or("the catalog does not know this track")?)
    }

    /// The id of a song's own video: the first one the catalog finds for it that its artist
    /// published under the song's name. A song listed in the catalog is a still picture
    /// over its audio; this is the video made for it, when there is one.
    pub async fn video_for(&self, artist: &str, title: &str) -> Result<Option<String>> {
        let videos = self.music().await?.search_videos(format!("{artist} {title}").as_str()).await?;
        Ok(videos.into_iter().find_map(|video| match video {
            SearchResultVideo::Video { title: name, channel_name, video_id, .. } if official(artist, title, &channel_name, &name) => {
                Some(video_id.get_raw().to_owned())
            }
            _ => None,
        }))
    }

    /// Playable audio for a video id. The URLs expire; resolve again rather than keep them.
    pub async fn stream(&self, video_id: &str) -> Result<Stream> {
        stream::resolve(&self.http, video_id).await
    }
}

fn playlist_browse_id(id: &str) -> String {
    // Public links carry playlist IDs; the browse endpoint needs its VL prefix.
    format!("VL{}", id.strip_prefix("VL").unwrap_or(id))
}

fn album_playlist_name<'a>(id: &str, tracks: &'a [Track]) -> Option<&'a str> {
    // Auto album playlists have no header; their tracks carry the album's name.
    id.strip_prefix("VLOLAK5uy_")?;
    tracks.first()?.album.as_deref()
}

fn found_song(song: SearchResultSong) -> Track {
    Track {
        id: song.video_id.get_raw().to_owned(),
        title: song.title,
        artist: song.artist,
        // Song results carry the artist as a name only.
        artist_id: None,
        album_id: song.album.as_ref().map(|album| album.id.get_raw().to_owned()),
        album: song.album.map(|album| album.name),
        artwork_url: artwork(song.thumbnails),
        duration_ms: duration_ms(&song.duration),
    }
}

/// Whether the video `name` on `channel` is `artist`'s own video of the song `title`.
fn official(artist: &str, title: &str, channel: &str, name: &str) -> bool {
    // "TheWeekndVEVO" and "The Weeknd" are the same publisher.
    let plain = |text: &str| text.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect::<String>();
    let channel = plain(channel);
    let channel = channel.strip_suffix("vevo").unwrap_or(&channel);
    // A song by several lists them; the first is whose channel it is on.
    let artist = plain(artist.split([',', '&']).next().unwrap_or(artist));
    !artist.is_empty() && channel == artist && plain(name).contains(&plain(title))
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
    use super::{album_playlist_name, duration_ms, official, playlist_browse_id};
    use surco_core::Track;

    #[test]
    fn public_playlist_ids_are_browse_ids_without_a_second_prefix() {
        assert_eq!(playlist_browse_id("PLcU2QLIBn7eo"), "VLPLcU2QLIBn7eo");
        assert_eq!(playlist_browse_id("VLPLcU2QLIBn7eo"), "VLPLcU2QLIBn7eo");
        assert_eq!(playlist_browse_id("OLAK5uy_album"), "VLOLAK5uy_album");
    }

    #[test]
    fn only_auto_album_playlists_get_their_name_from_a_tracks_album() {
        let mut track = Track {
            id: "song".into(), title: "K.".into(), artist: "Cigarettes After Sex".into(),
            album: Some("Cigarettes After Sex".into()), album_id: None, artist_id: None,
            artwork_url: None, duration_ms: 320_000,
        };
        assert_eq!(album_playlist_name("VLOLAK5uy_album", std::slice::from_ref(&track)), Some("Cigarettes After Sex"));
        assert_eq!(album_playlist_name("VLPLcU2QLIBn7eo", std::slice::from_ref(&track)), None);
        assert_eq!(album_playlist_name("VLOLAK5uy_album", &[]), None);
        track.album = None;
        assert_eq!(album_playlist_name("VLOLAK5uy_album", &[track]), None);
    }

    #[test]
    fn a_songs_video_is_the_one_its_own_artist_published_under_its_name() {
        assert!(official("The Weeknd", "Blinding Lights", "The Weeknd", "Blinding Lights (Official Video)"));
        assert!(official("The Weeknd, Daft Punk", "Starboy", "TheWeekndVEVO", "The Weeknd - Starboy ft. Daft Punk"));
        assert!(!official("The Weeknd", "Blinding Lights", "Jared Halley", "The Weeknd - Blinding Lights (HYBRID ACAPELLA)"));
        assert!(!official("The Weeknd", "Blinding Lights", "The Weeknd", "Save Your Tears (Official Video)"));
        assert!(!official("", "x", "", "x"));
    }

    #[test]
    fn durations_are_read_from_clock_text() {
        assert_eq!(duration_ms("4:51"), 291_000);
        assert_eq!(duration_ms("1:02:03"), 3_723_000);
        assert_eq!(duration_ms(" 0:07 "), 7_000);
        assert_eq!(duration_ms(""), 0);
        assert_eq!(duration_ms("live"), 0);
    }
}
