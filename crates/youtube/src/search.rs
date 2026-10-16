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
    if !omit_unlinked_albums(&mut response) { return Err(error.into()); }
    Ok(ytmapi_rs::process_json::<SongsQuery<'_>, NoAuthToken>(response.to_string(), query)?)
}

fn omit_unlinked_albums(response: &mut serde_json::Value) -> bool {
    // ytmapi-rs 0.3.3 requires an album browseId when the byline has three
    // fields. YouTube also returns unlinked album names. Express just that
    // known shape as the provider's supported artist/duration byline: the song
    // stays playable, without inventing an album ID. Upstream still parses and
    // validates every other field, including rows elsewhere in this response.
    // https://github.com/nick42d/youtui/blob/379efe9ef63d703c7891a964215e7a211b209758/ytmapi-rs/src/parse/search.rs#L564-L628
    let Some(sections) = response.pointer_mut("/contents/tabbedSearchResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents")
        .and_then(serde_json::Value::as_array_mut) else { return false; };
    let mut changed = false;
    for section in sections {
        let Some(rows) = section.pointer_mut("/musicShelfRenderer/contents")
            .and_then(serde_json::Value::as_array_mut) else { continue; };
        for row in rows {
            let Some(runs) = row.pointer_mut("/musicResponsiveListItemRenderer/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text/runs")
                .and_then(serde_json::Value::as_array_mut) else { continue; };
            if runs.len() < 5 { continue; }
            let album = runs.len() - 3;
            if runs[album - 1]["text"] != " • " || runs[album + 1]["text"] != " • " { continue; }
            if runs[album].pointer("/navigationEndpoint/browseEndpoint/browseId").is_some_and(|id| !id.is_null()) { continue; }
            let Some(artists) = runs[..album - 1].iter().map(|run| run["text"].as_str()).collect::<Option<String>>() else { continue; };
            if artists.trim().is_empty() || artists.contains(" • ")
                || runs[album]["text"].as_str().is_none_or(|text| text.is_empty()) { continue; }
            let Some(duration) = runs[album + 2]["text"].as_str() else { continue; };
            let parts: Vec<_> = duration.split(':').collect();
            if !(2..=3).contains(&parts.len()) || !parts.iter().all(|part|
                !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit())) { continue; }
            // Artist collaborations can have several linked runs before the
            // first separator; retain those credits and their punctuation.
            runs.drain(album - 1..album + 1);
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

    const RESPONSE: &str = include_str!("../tests/fixtures/search-missing-album-id.json");

    #[test]
    fn a_missing_album_id_keeps_every_song_and_the_other_albums() {
        let query = SearchQuery::new_filtered("prueba", SongsFilter);
        let tracks: Vec<_> = parse_response(RESPONSE, &query).unwrap().into_iter().map(found_song).collect();
        assert_eq!(tracks.len(), 3);
        for (track, id, title, artist, duration) in [
            (&tracks[0], "linked-song", "Con álbum", "Artista uno", 181_000),
            (&tracks[1], "unlinked-song", "Sin enlace de álbum", "Artista dos", 242_000),
            (&tracks[2], "no-album-song", "Sin álbum", "Artista tres", 63_000),
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
    fn an_unlinked_album_preserves_multiple_artist_runs() {
        let query = SearchQuery::new_filtered("prueba", SongsFilter);
        let mut response: Value = serde_json::from_str(RESPONSE).unwrap();
        let runs = response.pointer_mut("/contents/tabbedSearchResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicShelfRenderer/contents/1/musicResponsiveListItemRenderer/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text/runs").unwrap();
        *runs = serde_json::json!([
            {"text": "Artista dos", "navigationEndpoint": {"browseEndpoint": {"browseId": "UC-artist-two"}}},
            {"text": ", "},
            {"text": "Invitada", "navigationEndpoint": {"browseEndpoint": {"browseId": "UC-guest"}}},
            {"text": " • "}, {"text": "Álbum sin enlace"}, {"text": " • "}, {"text": "4:02"}
        ]);
        let tracks: Vec<_> = parse_response(&response.to_string(), &query).unwrap().into_iter().map(found_song).collect();
        assert_eq!(tracks.len(), 3);
        assert_eq!(tracks[1].id, "unlinked-song");
        assert_eq!(tracks[1].title, "Sin enlace de álbum");
        assert_eq!(tracks[1].artist, "Artista dos, Invitada");
        assert_eq!(tracks[1].duration_ms, 242_000);
        assert!(tracks[1].album_id.is_none());
        assert_eq!(tracks[0].album_id.as_deref(), Some("MPRE-linked-album"));
    }
}
