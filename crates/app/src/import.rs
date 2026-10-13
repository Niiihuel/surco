//! Bringing a list in from another app: reading it where it is, and finding its songs in
//! the catalog.

use futures_util::future::join_all;
use surco_core::transfer::{self, Source, Wanted};
use surco_core::Track;
use surco_youtube::YouTube;

/// The pages read here answer a browser, and nothing else, with the list.
const BROWSER: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
/// How many songs are looked for at a time: each is a search, and the catalog counts them.
const AT_A_TIME: usize = 4;

/// What makes a list's name and its songs out of the page that shows it.
type Reader = fn(&str) -> Option<(String, Vec<Wanted>)>;

/// The list a link names, as the catalog has it: its name, its songs in order, and how
/// many of them the catalog does not have.
pub async fn list(youtube: &YouTube, link: &str) -> Result<(String, Vec<Track>, usize), String> {
    // The others name their songs; the catalog's own lists them as it has them.
    let (address, read): (String, Reader) = match transfer::source(link).ok_or("Ese enlace no es de una playlist de Spotify, Apple Music ni YouTube Music.")? {
        Source::YouTube(id) => {
            let (name, tracks) = youtube.playlist_named(&id).await.map_err(|_| "No pudimos leer esa playlist. Tiene que ser pública.")?;
            return if tracks.is_empty() { Err("Esa playlist no tiene canciones.".to_owned()) } else { Ok((name, tracks, 0)) };
        }
        Source::Spotify(kind, id) => (format!("https://open.spotify.com/embed/{kind}/{id}"), spotify),
        Source::Apple(address) => (address, apple),
    };
    let page = youtube.http().get(address).header("User-Agent", BROWSER);
    let page = page.send().await.map_err(|error| error.to_string())?.text().await.map_err(|error| error.to_string())?;
    let (name, wanted) = read(&page).ok_or("No pudimos leer esa lista. Tiene que ser pública.")?;
    let mut tracks: Vec<Track> = Vec::new();
    for some in wanted.chunks(AT_A_TIME) {
        let found = join_all(some.iter().map(|song| async move { transfer::pick(song, &youtube.songs(&song.query()).await.ok()?).cloned() })).await;
        // Two songs that turn out to be the same one are it once.
        for track in found.into_iter().flatten() {
            if tracks.iter().all(|taken| taken.id != track.id) {
                tracks.push(track);
            }
        }
    }
    if tracks.is_empty() {
        return Err("No encontramos ninguna de las canciones de esa lista.".to_owned());
    }
    let missed = wanted.len() - tracks.len();
    Ok((name, tracks, missed))
}

/// The name and the songs of the list a page of Apple Music shows. The page carries its
/// name for search engines and its songs for its own script.
fn apple(page: &str) -> Option<(String, Vec<Wanted>)> {
    let script = |opening: &str| Some(page.split_once(opening)?.1.split_once('>')?.1.split_once("</script>")?.0);
    let said = |opening: &str| serde_json::from_str::<serde_json::Value>(script(opening)?).ok();
    let name = ["<script id=schema:music-playlist", "<script id=schema:music-album"].into_iter().find_map(|about| said(about)?["name"].as_str().map(str::to_owned))?;
    let mut songs = Vec::new();
    songs_in(&said(r#"id="serialized-server-data""#)?, &mut songs);
    Some((name, songs))
}

/// Every song in what a page carries, wherever it is in it: what has a title, an artist
/// and a length.
fn songs_in(value: &serde_json::Value, songs: &mut Vec<Wanted>) {
    use serde_json::Value;
    match value {
        Value::Object(fields) => match (fields.get("title").and_then(Value::as_str), fields.get("artistName").and_then(Value::as_str), fields.get("duration").and_then(Value::as_u64)) {
            (Some(title), Some(artist), Some(duration_ms)) => songs.push(Wanted { title: title.to_owned(), artist: artist.to_owned(), duration_ms }),
            _ => fields.values().for_each(|inner| songs_in(inner, songs)),
        },
        Value::Array(items) => items.iter().for_each(|inner| songs_in(inner, songs)),
        _ => {}
    }
}

/// The name and the songs of the list a page of Spotify's embedded player shows. The page
/// carries them as JSON for its own script; it lists a hundred songs at the most.
fn spotify(page: &str) -> Option<(String, Vec<Wanted>)> {
    let (_, data) = page.split_once(r#"<script id="__NEXT_DATA__""#)?;
    let data = data.split_once('>')?.1.split_once("</script>")?.0;
    let data: serde_json::Value = serde_json::from_str(data).ok()?;
    let list = &data["props"]["pageProps"]["state"]["data"]["entity"];
    let songs = list["trackList"].as_array()?.iter().filter_map(|song| {
        Some(Wanted {
            title: song["title"].as_str()?.to_owned(),
            // Its artists come as one text, a hard space after each comma.
            artist: song["subtitle"].as_str().unwrap_or_default().replace('\u{a0}', " "),
            duration_ms: song["duration"].as_u64().unwrap_or(0),
        })
    });
    Some((list["name"].as_str()?.to_owned(), songs.collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "reads public YouTube Music lists over the network"]
    async fn public_youtube_music_album_and_playlist_links_import() {
        let youtube = YouTube::new().unwrap();
        let album = "https://music.youtube.com/playlist?list=OLAK5uy_khL-3qIv3An2R3B0mBXdO7vgCo3P-GA5c";
        let (name, tracks, missed) = list(&youtube, album).await.unwrap();
        assert_eq!(name, "Cigarettes After Sex");
        assert_eq!(tracks.len(), 10);
        assert_eq!(missed, 0);
        assert_eq!(tracks.first().unwrap().title, "K.");
        assert_eq!(tracks.last().unwrap().title, "Young & Dumb");

        let playlist = "https://music.youtube.com/playlist?list=PLcU2QLIBn7eo";
        let (name, tracks, missed) = list(&youtube, playlist).await.unwrap();
        assert_eq!(name, "睡前");
        assert!(!tracks.is_empty());
        assert_eq!(missed, 0);
    }

    #[tokio::test]
    #[ignore = "reads a public YouTube Music playlist with multiple pages over the network"]
    async fn public_youtube_music_playlist_imports_past_its_first_page() {
        let youtube = YouTube::new().unwrap();
        let (name, tracks, _) = list(&youtube, "https://music.youtube.com/playlist?list=PLFDRC9rgfQf8").await.unwrap();
        assert_eq!(name, "SjK 1000 Songs");
        assert!(tracks.len() > 100, "only {} tracks imported", tracks.len());
    }

    #[test]
    fn spotifys_page_for_embedded_players_names_the_list_and_its_songs() {
        let page = r#"<html><script id="__NEXT_DATA__" type="application/json">{"props":{"pageProps":{"state":{"data":{"entity":{"name":"Viaje",
            "trackList":[{"title":"Global Warming (feat. Sensato)","subtitle":"Pitbull, Sensato","duration":85400},{"title":"Sin autor"},{"subtitle":"sin título"}]}}}}}}</script></html>"#;
        let song = |title: &str, artist: &str, duration_ms| Wanted { title: title.into(), artist: artist.into(), duration_ms };
        assert_eq!(spotify(page), Some(("Viaje".into(), vec![song("Global Warming (feat. Sensato)", "Pitbull, Sensato", 85400), song("Sin autor", "", 0)])));
        assert_eq!(spotify("<html>Update your browser</html>"), None);
        assert_eq!(spotify(r#"<script id="__NEXT_DATA__">{"props":{}}</script>"#), None);
    }

    #[test]
    fn apple_musics_page_names_the_list_and_carries_its_songs_wherever_they_are() {
        let page = r#"<script id=schema:music-album type="application/ld+json">{"name":"Música de Fondo"}</script>
            <script type="application/json" id="serialized-server-data">[{"data":{"title":"not a song","sections":[{"items":[
            {"title":"Sombras","artistName":"Zoé","duration":252322},{"title":"Soñé","artistName":"Zoé","duration":226627}]}]}}]</script>"#;
        let song = |title: &str, duration_ms| Wanted { title: title.into(), artist: "Zoé".into(), duration_ms };
        assert_eq!(apple(page), Some(("Música de Fondo".into(), vec![song("Sombras", 252322), song("Soñé", 226627)])));
        assert_eq!(apple("<html></html>"), None);
    }
}
