//! Bringing a list in from another app: reading it where it is, and finding its songs in
//! the catalog.

use futures_util::future::join_all;
use surco_core::transfer::{self, Source, Wanted};
use surco_core::Track;
use surco_youtube::YouTube;

use crate::i18n::t;

/// The pages read here answer a browser, and nothing else, with the list.
const BROWSER: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
/// How many songs are looked for at a time: each is a search, and the catalog counts them.
const AT_A_TIME: usize = 4;

/// What makes a list's name and its songs out of the page that shows it.
type Reader = fn(&str) -> Option<(String, Vec<Wanted>)>;

/// The list a link names, as the catalog has it: its name, its songs in order, and the
/// ones the catalog does not have, by whose they are and their name. `told` hears how
/// many of its songs have been looked for, out of how many: a long list takes minutes.
pub async fn list(youtube: &YouTube, link: &str, told: &(dyn Fn(usize, usize) + Sync)) -> Result<(String, Vec<Track>, Vec<String>), String> {
    // The others name their songs; the catalog's own lists them as it has them.
    let (address, read): (String, Reader) = match transfer::source(link).ok_or(t!("state-import-unknown-link"))? {
        Source::YouTube(id) => {
            let (name, tracks) = youtube.playlist_named(&id).await.map_err(|_| t!("state-import-unreadable-playlist"))?;
            return if tracks.is_empty() { Err(t!("state-import-empty-playlist").into_owned()) } else { Ok((name, tracks, Vec::new())) };
        }
        Source::Spotify(kind, id) => (format!("https://open.spotify.com/embed/{kind}/{id}"), spotify),
        Source::Apple(address) => (address, apple),
    };
    let page = youtube.http().get(address).header("User-Agent", BROWSER);
    let page = page.send().await.map_err(|error| error.to_string())?.text().await.map_err(|error| error.to_string())?;
    let (name, mut wanted) = read(&page).ok_or(t!("state-import-unreadable-list"))?;
    // Spotify's page stops at a hundred songs: a playlist that fills it may go on.
    if let (Some(Source::Spotify("playlist", id)), true) = (transfer::source(link), wanted.len() >= SPOTIFY_PAGE) {
        if let Some(whole) = spotify_whole(youtube, &id).await.filter(|whole| whole.len() > wanted.len()) {
            wanted = whole;
        }
    }
    let (mut tracks, mut missed): (Vec<Track>, Vec<String>) = Default::default();
    for (round, some) in wanted.chunks(AT_A_TIME).enumerate() {
        told(round * AT_A_TIME, wanted.len());
        let found = join_all(some.iter().map(|song| find(youtube, song))).await;
        for (song, found) in some.iter().zip(found) {
            match found {
                // Two songs that turn out to be the same one are it once.
                Some(track) if tracks.iter().all(|taken| taken.id != track.id) => tracks.push(track),
                Some(_) => {}
                None => missed.push(format!("{} — {}", song.artist, song.title)),
            }
        }
    }
    if tracks.is_empty() {
        return Err(t!("state-import-nothing-found").into_owned());
    }
    Ok((name, tracks, missed))
}

/// The song wanted, as the catalog has it, asked for one way after another until one
/// finds it. A search that fails is made once more: among hundreds, some do. One that
/// fails again is an answer the catalog worded in a way that cannot be read, and the
/// next way of asking may get another.
async fn find(youtube: &YouTube, song: &Wanted) -> Option<Track> {
    for query in song.queries() {
        let answers = match youtube.songs(&query).await {
            Ok(answers) => answers,
            Err(_) => {
                tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                youtube.songs(&query).await.unwrap_or_default()
            }
        };
        if let Some(track) = transfer::pick(song, &answers) {
            return Some(track.clone());
        }
    }
    None
}

/// Where the secret Spotify's web player proves itself with is kept, by its version, by
/// people who follow its changes.
const SPOTIFY_SECRETS: &str = "https://raw.githubusercontent.com/xyloflake/spot-secrets-go/main/secrets/secretDict.json";
/// The name the web player's service knows the question for a playlist's songs by.
const SPOTIFY_PLAYLIST_QUESTION: &str = "86dde7b9d9356e2369414647cf6950cfed96e778e129cfdfc99aea6c1613b3b0";
/// How many songs Spotify's page shows, and its service hands over at a time.
const SPOTIFY_PAGE: usize = 100;

/// All the songs of a Spotify playlist, past the hundred its page shows: asked of its web
/// player's own service, the way the player asks. Spotify promises none of this and
/// changes every part of it: where a step fails, what was read until then stands.
async fn spotify_whole(youtube: &YouTube, id: &str) -> Option<Vec<Wanted>> {
    use serde_json::{json, Value};
    let http = youtube.http();
    let get = |address: &str| http.get(address).header("User-Agent", BROWSER);
    let secrets: Value = get(SPOTIFY_SECRETS).send().await.ok()?.json().await.ok()?;
    let (version, secret) = secrets.as_object()?.iter().filter_map(|(version, secret)| Some((version.parse::<u32>().ok()?, secret))).max_by_key(|(version, _)| *version)?;
    // The code is made of the secret, each number of it turned by its place, and of the
    // time: Spotify's own, which a clock set wrong does not tell.
    let key: String = secret.as_array()?.iter().enumerate().filter_map(|(place, number)| Some((number.as_u64()? ^ (place as u64 % 33 + 9)).to_string())).collect();
    let front = get("https://open.spotify.com/").send().await.ok()?;
    let theirs = front.headers().get("date").and_then(|date| chrono::DateTime::parse_from_rfc2822(date.to_str().ok()?).ok());
    let code = totp(key.as_bytes(), theirs.map_or_else(|| chrono::Utc::now().timestamp(), |date| date.timestamp()) as u64);
    let token = format!("https://open.spotify.com/api/token?reason=init&productType=web-player&totp={code}&totpServer={code}&totpVer={version}");
    let token: Value = get(&token).header("Referer", "https://open.spotify.com/").send().await.ok()?.json().await.ok()?;
    let (access, client) = (token["accessToken"].as_str()?, token["clientId"].as_str()?);
    let device = format!("{:032x}", fastrand::u128(..));
    let device = format!("{}-{}-{}-{}-{}", &device[..8], &device[8..12], &device[12..16], &device[16..20], &device[20..]);
    let player = json!({ "client_data": { "client_version": "1.2.99", "client_id": client, "js_sdk_data": {
        "device_brand": "unknown", "device_model": "unknown", "os": "windows", "os_version": "NT 10.0", "device_id": device, "device_type": "computer" } } });
    let granted = http.post("https://clienttoken.spotify.com/v1/clienttoken").header("User-Agent", BROWSER).header("Accept", "application/json").json(&player);
    let granted: Value = granted.send().await.ok()?.json().await.ok()?;
    let client_token = granted["granted_token"]["token"].as_str()?;

    let mut songs = Vec::new();
    for from in (0..).step_by(SPOTIFY_PAGE) {
        let question = json!({ "operationName": "fetchPlaylist",
            "variables": { "uri": format!("spotify:playlist:{id}"), "offset": from, "limit": SPOTIFY_PAGE, "enableWatchFeedEntrypoint": false, "includeEpisodeContentRatingsV2": true },
            "extensions": { "persistedQuery": { "version": 1, "sha256Hash": SPOTIFY_PLAYLIST_QUESTION } } });
        let asked = http.post("https://api-partner.spotify.com/pathfinder/v1/query").bearer_auth(access).header("client-token", client_token).header("app-platform", "WebPlayer");
        let asked = asked.header("User-Agent", BROWSER).header("Origin", "https://open.spotify.com").header("Referer", "https://open.spotify.com/").json(&question);
        let answer: Option<Value> = async { asked.send().await.ok()?.json().await.ok() }.await;
        let Some(page) = answer.as_ref().and_then(|answer| answer["data"]["playlistV2"]["content"].as_object()) else { break };
        let items = page.get("items").and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
        songs.extend(items.iter().filter_map(|item| {
            let song = &item["itemV2"]["data"];
            let artists = song["artists"]["items"].as_array()?.iter().filter_map(|artist| artist["profile"]["name"].as_str()).collect::<Vec<_>>().join(", ");
            Some(Wanted { title: song["name"].as_str().filter(|_| song["__typename"] == "Track")?.to_owned(), artist: artists, duration_ms: song["trackDuration"]["totalMilliseconds"].as_u64().unwrap_or(0) })
        }));
        if items.is_empty() || from + SPOTIFY_PAGE >= page.get("totalCount").and_then(Value::as_u64).unwrap_or(0) as usize {
            break;
        }
    }
    Some(songs)
}

/// The six digits a secret and a moment make: RFC 6238's, with SHA-1 and steps of thirty seconds.
fn totp(key: &[u8], seconds: u64) -> String {
    use sha1::{Digest, Sha1};
    // HMAC, by hand: the key over a block, turned one way inside the hash and another outside it.
    let mut block = [0u8; 64];
    if key.len() > block.len() {
        block[..20].copy_from_slice(&Sha1::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let inner = Sha1::new().chain_update(block.map(|byte| byte ^ 0x36)).chain_update((seconds / 30).to_be_bytes()).finalize();
    let hash = Sha1::new().chain_update(block.map(|byte| byte ^ 0x5c)).chain_update(inner).finalize();
    let at = usize::from(hash[19] & 0xf);
    let number = u32::from_be_bytes([hash[at] & 0x7f, hash[at + 1], hash[at + 2], hash[at + 3]]);
    format!("{:06}", number % 1_000_000)
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
        let (name, tracks, missed) = list(&youtube, album, &|_, _| ()).await.unwrap();
        assert_eq!(name, "Cigarettes After Sex");
        assert_eq!(tracks.len(), 10);
        assert!(missed.is_empty());
        assert_eq!(tracks.first().unwrap().title, "K.");
        assert_eq!(tracks.last().unwrap().title, "Young & Dumb");

        let playlist = "https://music.youtube.com/playlist?list=PLcU2QLIBn7eo";
        let (name, tracks, missed) = list(&youtube, playlist, &|_, _| ()).await.unwrap();
        assert_eq!(name, "睡前");
        assert!(!tracks.is_empty());
        assert!(missed.is_empty());
    }

    #[tokio::test]
    #[ignore = "reads a public YouTube Music playlist with multiple pages over the network"]
    async fn public_youtube_music_playlist_imports_past_its_first_page() {
        let youtube = YouTube::new().unwrap();
        let (name, tracks, _) = list(&youtube, "https://music.youtube.com/playlist?list=PLFDRC9rgfQf8", &|_, _| ()).await.unwrap();
        assert_eq!(name, "SjK 1000 Songs");
        assert!(tracks.len() > 100, "only {} tracks imported", tracks.len());
    }

    #[tokio::test]
    #[ignore = "reads a public Spotify playlist of more than a hundred songs over the network"]
    async fn a_spotify_playlist_is_read_past_the_hundred_songs_its_page_shows() {
        let youtube = YouTube::new().unwrap();
        let whole = spotify_whole(&youtube, "37i9dQZF1DX4UtSsGT1Sbe").await.unwrap();
        assert!(whole.len() > 100, "only {} songs read", whole.len());
        assert!(whole.iter().all(|song| !song.title.is_empty() && !song.artist.is_empty() && song.duration_ms > 0));
    }

    #[test]
    fn a_code_is_made_of_a_secret_and_a_moment_as_the_standard_has_it() {
        // RFC 6238's own examples, their last six digits.
        assert_eq!(totp(b"12345678901234567890", 59), "287082");
        assert_eq!(totp(b"12345678901234567890", 1_111_111_109), "081804");
        assert_eq!(totp(&[b'k'; 80], 59).len(), 6);
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
