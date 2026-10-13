//! Bringing a list in from another app: reading it where it is, and finding its songs in
//! the catalog.

use futures_util::future::join_all;
use surco_core::transfer::{self, Source, Wanted};
use surco_core::Track;
use surco_youtube::YouTube;

/// Spotify's page for embedded players answers a browser, and nothing else, with the list.
const BROWSER: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
/// How many songs are looked for at a time: each is a search, and the catalog counts them.
const AT_A_TIME: usize = 4;

/// The list a link names, as the catalog has it: its name, its songs in order, and how
/// many of them the catalog does not have.
pub async fn list(youtube: &YouTube, link: &str) -> Result<(String, Vec<Track>, usize), String> {
    let (name, tracks, missed) = match transfer::source(link).ok_or("Ese enlace no es de una playlist de Spotify ni de YouTube Music.")? {
        Source::YouTube(id) => {
            let (name, tracks) = youtube.playlist_named(&id).await.map_err(|_| "No pudimos leer esa playlist. Tiene que ser pública.")?;
            (name, tracks, 0)
        }
        Source::Spotify(kind, id) => {
            let page = youtube.http().get(format!("https://open.spotify.com/embed/{kind}/{id}")).header("User-Agent", BROWSER);
            let page = page.send().await.map_err(|error| error.to_string())?.text().await.map_err(|error| error.to_string())?;
            let (name, wanted) = spotify(&page).ok_or("Spotify no mostró esa lista. Tiene que ser pública.")?;
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
            let missed = wanted.len() - tracks.len();
            (name, tracks, missed)
        }
    };
    if tracks.is_empty() {
        return Err("No encontramos ninguna de las canciones de esa lista.".to_owned());
    }
    Ok((name, tracks, missed))
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

    #[test]
    fn spotifys_page_for_embedded_players_names_the_list_and_its_songs() {
        let page = r#"<html><script id="__NEXT_DATA__" type="application/json">{"props":{"pageProps":{"state":{"data":{"entity":{"name":"Viaje",
            "trackList":[{"title":"Global Warming (feat. Sensato)","subtitle":"Pitbull, Sensato","duration":85400},{"title":"Sin autor"},{"subtitle":"sin título"}]}}}}}}</script></html>"#;
        let song = |title: &str, artist: &str, duration_ms| Wanted { title: title.into(), artist: artist.into(), duration_ms };
        assert_eq!(spotify(page), Some(("Viaje".into(), vec![song("Global Warming (feat. Sensato)", "Pitbull, Sensato", 85400), song("Sin autor", "", 0)])));
        assert_eq!(spotify("<html>Update your browser</html>"), None);
        assert_eq!(spotify(r#"<script id="__NEXT_DATA__">{"props":{}}</script>"#), None);
    }
}
