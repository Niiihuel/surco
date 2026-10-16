//! Song search through the provider's public raw-response and parsing APIs.

use ytmapi_rs::auth::noauth::NoAuthToken;
use ytmapi_rs::parse::SearchResultSong;
use ytmapi_rs::query::search::{FilteredSearch, SearchQuery, SongsFilter, SpellingMode};

use super::{Music, Result};

type SongsQuery<'a> = SearchQuery<'a, FilteredSearch<SongsFilter>>;

pub(super) async fn songs(music: &Music, text: &str) -> Result<Vec<SearchResultSong>> {
    let query = SearchQuery::new_filtered(text, SongsFilter)
        .with_spelling_mode(SpellingMode::WithSuggestions);
    let raw = music.raw_json_query::<SongsQuery<'_>>(&query).await?;
    parse_response(&raw, &query)
}

fn parse_response(raw: &str, query: &SongsQuery<'_>) -> Result<Vec<SearchResultSong>> {
    let error = match ytmapi_rs::process_json::<SongsQuery<'_>, NoAuthToken>(raw.to_owned(), query) {
        Ok(songs) => return Ok(songs),
        Err(error) => error,
    };
    let mut response: serde_json::Value = serde_json::from_str(raw)?;
    if !normalize_song_rows(&mut response) { return Err(error.into()); }
    Ok(ytmapi_rs::process_json::<SongsQuery<'_>, NoAuthToken>(response.to_string(), query)?)
}

fn normalize_song_rows(response: &mut serde_json::Value) -> bool {
    // ytmapi-rs 0.3.3 requires an album browseId when the byline has three
    // fields. Mixed song searches also return OMV/UGC videos with artist,
    // views, duration in two columns. Move their real views into the separate
    // plays column expected by upstream, retaining artist/duration in byline.
    // An unlinked album with an existing plays column uses that same supported
    // byline without inventing an album ID. Upstream validates every other
    // field, including rows elsewhere in this response.
    // https://github.com/nick42d/youtui/blob/379efe9ef63d703c7891a964215e7a211b209758/ytmapi-rs/src/parse/search.rs#L564-L628
    let Some(sections) = response.pointer_mut("/contents/tabbedSearchResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents")
        .and_then(serde_json::Value::as_array_mut) else { return false; };
    let mut changed = false;
    for section in sections {
        let Some(rows) = section.pointer_mut("/musicShelfRenderer/contents")
            .and_then(serde_json::Value::as_array_mut) else { continue; };
        for row in rows {
            let video = row.pointer("/musicResponsiveListItemRenderer/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/navigationEndpoint/watchEndpoint/watchEndpointMusicSupportedConfigs/watchEndpointMusicConfig/musicVideoType")
                .and_then(serde_json::Value::as_str).is_some_and(|kind| matches!(kind, "MUSIC_VIDEO_TYPE_OMV" | "MUSIC_VIDEO_TYPE_UGC"));
            let Some(columns) = row.pointer_mut("/musicResponsiveListItemRenderer/flexColumns")
                .and_then(serde_json::Value::as_array_mut) else { continue; };
            let missing_plays = columns.len() == 2;
            let Some(runs) = columns.get_mut(1).and_then(|column| column.pointer_mut("/musicResponsiveListItemFlexColumnRenderer/text/runs"))
                .and_then(serde_json::Value::as_array_mut) else { continue; };
            if runs.len() < 5 { continue; }
            let middle = runs.len() - 3;
            if runs[middle - 1]["text"] != " • " || runs[middle + 1]["text"] != " • " { continue; }
            if runs[middle].pointer("/navigationEndpoint/browseEndpoint/browseId").is_some_and(|id| !id.is_null()) { continue; }
            let Some(artists) = runs[..middle - 1].iter().map(|run| run["text"].as_str()).collect::<Option<String>>() else { continue; };
            if artists.trim().is_empty() || artists.contains(" • ")
                || runs[middle]["text"].as_str().is_none_or(|text| text.is_empty()) { continue; }
            let Some(duration) = runs[middle + 2]["text"].as_str() else { continue; };
            let parts: Vec<_> = duration.split(':').collect();
            if !(2..=3).contains(&parts.len()) || !parts.iter().all(|part|
                !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit())) { continue; }
            // Artist collaborations can have several linked runs before the
            // first separator; retain those credits and their punctuation.
            let views = (video && missing_plays).then(|| runs[middle].clone());
            runs.drain(middle - 1..middle + 1);
            if let Some(views) = views {
                columns.push(serde_json::json!({"musicResponsiveListItemFlexColumnRenderer": {"text": {"runs": [views]}}}));
            }
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::found_song;
    use serde_json::Value;

    const RESPONSE: &str = include_str!("../tests/fixtures/search-mixed-song-video-rows.json");

    #[test]
    fn mixed_search_rows_preserve_songs_videos_and_linked_albums() {
        let query = SearchQuery::new_filtered("prueba", SongsFilter);
        let songs = parse_response(RESPONSE, &query).unwrap();
        assert_eq!(songs[0].plays, "1000 plays");
        assert_eq!(songs[1].plays, "235M views");
        assert_eq!(songs[3].plays, "2000 plays");
        let tracks: Vec<_> = songs.into_iter().map(found_song).collect();
        assert_eq!(tracks.len(), 4);
        for (track, id, title, artist, duration) in [
            (&tracks[0], "linked-song", "Con álbum", "Artista uno", 181_000),
            (&tracks[1], "video-song", "Video oficial", "Artista dos", 242_000),
            (&tracks[2], "no-album-song", "Sin álbum", "Artista tres", 63_000),
            (&tracks[3], "unlinked-song", "Sin enlace de álbum", "Artista cuatro", 154_000),
        ] {
            assert_eq!(track.id, id);
            assert_eq!(track.title, title);
            assert_eq!(track.artist, artist);
            assert_eq!(track.duration_ms, duration);
        }
        assert_eq!(tracks[0].album.as_deref(), Some("Álbum enlazado"));
        assert_eq!(tracks[0].album_id.as_deref(), Some("MPRE-linked-album"));
        assert!(tracks[1].album_id.is_none());
        assert!(tracks[2].album.is_none());
        assert!(tracks[2].album_id.is_none());
        assert!(tracks[3].album_id.is_none());
    }

    #[test]
    fn unrelated_invalid_song_data_still_returns_an_error() {
        let query = SearchQuery::new_filtered("prueba", SongsFilter);
        let mut response: Value = serde_json::from_str(RESPONSE).unwrap();
        let song = response.pointer_mut("/contents/tabbedSearchResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicShelfRenderer/contents/0/musicResponsiveListItemRenderer").unwrap();
        song.as_object_mut().unwrap().remove("playlistItemData");
        let error = parse_response(&response.to_string(), &query).unwrap_err();
        assert!(error.to_string().contains("videoId"), "{error}");
    }

    #[test]
    fn a_video_byline_preserves_multiple_artist_runs() {
        let query = SearchQuery::new_filtered("prueba", SongsFilter);
        let mut response: Value = serde_json::from_str(RESPONSE).unwrap();
        let runs = response.pointer_mut("/contents/tabbedSearchResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicShelfRenderer/contents/1/musicResponsiveListItemRenderer/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text/runs").unwrap();
        *runs = serde_json::json!([
            {"text": "Artista dos", "navigationEndpoint": {"browseEndpoint": {"browseId": "UC-artist-two"}}},
            {"text": ", "},
            {"text": "Invitada", "navigationEndpoint": {"browseEndpoint": {"browseId": "UC-guest"}}},
            {"text": " • "}, {"text": "235M views"}, {"text": " • "}, {"text": "4:02"}
        ]);
        *response.pointer_mut("/contents/tabbedSearchResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicShelfRenderer/contents/1/musicResponsiveListItemRenderer/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/navigationEndpoint/watchEndpoint/watchEndpointMusicSupportedConfigs/watchEndpointMusicConfig/musicVideoType").unwrap() = "MUSIC_VIDEO_TYPE_UGC".into();
        let songs = parse_response(&response.to_string(), &query).unwrap();
        assert_eq!(songs[1].plays, "235M views");
        let tracks: Vec<_> = songs.into_iter().map(found_song).collect();
        assert_eq!(tracks.len(), 4);
        assert_eq!(tracks[1].id, "video-song");
        assert_eq!(tracks[1].title, "Video oficial");
        assert_eq!(tracks[1].artist, "Artista dos, Invitada");
        assert_eq!(tracks[1].duration_ms, 242_000);
        assert!(tracks[1].album_id.is_none());
        assert_eq!(tracks[0].album_id.as_deref(), Some("MPRE-linked-album"));
    }
}
