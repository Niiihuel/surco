//! What is listened to, sent to the listener's own Last.fm or ListenBrainz, as Opentune does.
//!
//! ListenBrainz takes the token from listenbrainz.org/settings. Last.fm takes the listener's
//! own API key and secret from last.fm/api, and their username and password once, exchanged
//! for a session key; the password is never kept. Each listen is sent once heard as Last.fm
//! asks; what could not be sent waits, and goes with the next.

use std::collections::BTreeMap;

use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use surco_core::Track;

const LISTENBRAINZ: &str = "https://api.listenbrainz.org";
const LASTFM: &str = "https://ws.audioscrobbler.com/2.0/";
/// The most listens kept waiting for a service, and sent at once.
pub const QUEUED_MOST: usize = 500;
const LASTFM_BATCH: usize = 50;
const LISTENBRAINZ_BATCH: usize = 100;

/// A song listened to, as both services take it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Listen {
    pub artist: String,
    pub track: String,
    pub album: Option<String>,
    /// When it started, in seconds since 1970.
    pub at: i64,
    pub seconds: Option<u64>,
    pub video_id: String,
}

impl Listen {
    pub fn of(track: &Track, at: i64) -> Self {
        Listen {
            artist: track.artist.clone(),
            track: track.title.clone(),
            album: track.album.clone(),
            at,
            seconds: (track.duration_ms > 0).then_some(track.duration_ms / 1000),
            video_id: track.id.clone(),
        }
    }
}

/// Whether a song heard `heard` seconds of `duration` is sent: Last.fm's rule, a song over 30
/// seconds heard for half of it or four minutes.
pub fn sends(heard: f64, duration: f64) -> bool {
    duration > 30.0 && heard >= (duration / 2.0).min(240.0)
}

/// The services a listener is signed in to, and what waits for each.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Accounts {
    pub listenbrainz: Option<ListenBrainz>,
    pub lastfm: Option<LastFm>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListenBrainz {
    pub user: String,
    pub token: String,
    #[serde(default)]
    pub waiting: Vec<Listen>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LastFm {
    pub user: String,
    pub key: String,
    pub secret: String,
    pub session: String,
    #[serde(default)]
    pub waiting: Vec<Listen>,
}

/// A service signed in to.
#[derive(Clone, Debug, PartialEq)]
pub enum Account {
    ListenBrainz(ListenBrainz),
    LastFm(LastFm),
}

/// What signing in to a service takes.
#[derive(Clone, Debug, PartialEq)]
pub enum SignIn {
    ListenBrainz { token: String },
    LastFm { key: String, secret: String, user: String, password: String },
}

/// Signs in, as asked.
pub async fn sign_in(youtube: &surco_youtube::YouTube, sign_in: SignIn) -> Result<Account, String> {
    match sign_in {
        SignIn::ListenBrainz { token } => listenbrainz_sign_in(youtube, &token).await.map(Account::ListenBrainz),
        SignIn::LastFm { key, secret, user, password } => lastfm_sign_in(youtube, &key, &secret, &user, &password).await.map(Account::LastFm),
    }
}

impl Accounts {
    /// Whether any service is signed in to.
    pub fn any(&self) -> bool {
        self.listenbrainz.is_some() || self.lastfm.is_some()
    }

    /// Keeps `listen` for every service signed in to, the oldest let go past the most.
    pub fn keep(&mut self, listen: &Listen) {
        let waiting = [self.listenbrainz.as_mut().map(|account| &mut account.waiting), self.lastfm.as_mut().map(|account| &mut account.waiting)];
        for waiting in waiting.into_iter().flatten() {
            waiting.push(listen.clone());
            if waiting.len() > QUEUED_MOST { waiting.drain(..waiting.len() - QUEUED_MOST); }
        }
    }

    /// The first `sent` of what waited for `service` went.
    pub fn sent(&mut self, service: Service, sent: usize) {
        let waiting = match service {
            Service::ListenBrainz => self.listenbrainz.as_mut().map(|account| &mut account.waiting),
            Service::LastFm => self.lastfm.as_mut().map(|account| &mut account.waiting),
        };
        if let Some(waiting) = waiting { waiting.drain(..sent.min(waiting.len())); }
    }
}

/// Which service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    ListenBrainz,
    LastFm,
}

/// The first artist a credit names: the services match better on it.
fn main_artist(credit: &str) -> &str {
    let mut first = credit;
    for separator in [", ", " feat. ", " ft. ", " featuring "] {
        if let Some((before, _)) = first.split_once(separator) { first = before; }
    }
    first.trim()
}

/// ListenBrainz's submit-listens body: no time for "playing now".
fn listenbrainz_body(kind: &str, listens: &[Listen]) -> Value {
    let payload: Vec<Value> = listens.iter().map(|listen| {
        let mut info = json!({
            "media_player": "Surco",
            "submission_client": "Surco",
            "submission_client_version": env!("CARGO_PKG_VERSION"),
            "music_service": "music.youtube.com",
            "origin_url": format!("https://music.youtube.com/watch?v={}", listen.video_id),
        });
        if let Some(seconds) = listen.seconds { info["duration_ms"] = json!(seconds * 1000); }
        let mut metadata = json!({ "artist_name": listen.artist, "track_name": listen.track, "additional_info": info });
        if let Some(album) = &listen.album { metadata["release_name"] = json!(album); }
        let mut item = json!({ "track_metadata": metadata });
        if kind != "playing_now" { item["listened_at"] = json!(listen.at); }
        item
    }).collect();
    json!({ "listen_type": kind, "payload": payload })
}

/// Last.fm's signature: the MD5 of every parameter, sorted by name, then the secret.
fn signature(params: &BTreeMap<String, String>, secret: &str) -> String {
    let mut hasher = Md5::new();
    for (name, value) in params {
        hasher.update(name.as_bytes());
        hasher.update(value.as_bytes());
    }
    hasher.update(secret.as_bytes());
    hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
}

/// A signed call to Last.fm, as every write is.
async fn lastfm(youtube: &surco_youtube::YouTube, mut params: BTreeMap<String, String>, key: &str, secret: &str, session: Option<&str>) -> Result<Value, String> {
    params.insert("api_key".into(), key.into());
    if let Some(session) = session { params.insert("sk".into(), session.into()); }
    let signed = signature(&params, secret);
    params.insert("api_sig".into(), signed);
    params.insert("format".into(), "json".into());
    let encode = |text: &str| percent_encoding::utf8_percent_encode(text, percent_encoding::NON_ALPHANUMERIC).to_string();
    let body = params.iter().map(|(name, value)| format!("{}={}", encode(name), encode(value))).collect::<Vec<_>>().join("&");
    let answer = youtube.http().post(LASTFM).header("Content-Type", "application/x-www-form-urlencoded").body(body)
        .send().await.map_err(|error| error.to_string())?;
    let status = answer.status();
    let answer: Value = answer.json().await.map_err(|_| format!("Last.fm answered {status}"))?;
    match answer["message"].as_str() {
        Some(message) if answer.get("error").is_some() => Err(message.to_owned()),
        _ => Ok(answer),
    }
}

/// Signs in to Last.fm with the listener's own API key and secret; the password goes only
/// into this one call.
pub async fn lastfm_sign_in(youtube: &surco_youtube::YouTube, key: &str, secret: &str, user: &str, password: &str) -> Result<LastFm, String> {
    let (key, secret) = (key.trim(), secret.trim());
    let params = BTreeMap::from([("method".into(), "auth.getMobileSession".into()), ("username".into(), user.trim().into()), ("password".into(), password.into())]);
    let answer = lastfm(youtube, params, key, secret, None).await?;
    let session = answer["session"]["key"].as_str().ok_or("Last.fm did not open a session")?;
    let user = answer["session"]["name"].as_str().unwrap_or(user.trim());
    Ok(LastFm { user: user.into(), key: key.into(), secret: secret.into(), session: session.into(), waiting: Vec::new() })
}

/// Checks a ListenBrainz token, and says whose it is.
pub async fn listenbrainz_sign_in(youtube: &surco_youtube::YouTube, token: &str) -> Result<ListenBrainz, String> {
    let token = token.trim();
    let answer: Value = youtube.http().get(format!("{LISTENBRAINZ}/1/validate-token")).header("Authorization", format!("Token {token}"))
        .send().await.map_err(|error| error.to_string())?.json().await.map_err(|error| error.to_string())?;
    if answer["valid"] != true { return Err(answer["message"].as_str().unwrap_or("not a valid token").to_owned()); }
    let user = answer["user_name"].as_str().ok_or("ListenBrainz did not name the user")?;
    Ok(ListenBrainz { user: user.into(), token: token.into(), waiting: Vec::new() })
}

/// "Playing now" on the listener's profile; nothing waits when it fails.
pub async fn playing_now(youtube: &surco_youtube::YouTube, accounts: &Accounts, listen: &Listen) {
    if let Some(account) = &accounts.listenbrainz {
        let _ = youtube.http().post(format!("{LISTENBRAINZ}/1/submit-listens")).header("Authorization", format!("Token {}", account.token))
            .json(&listenbrainz_body("playing_now", std::slice::from_ref(listen))).send().await;
    }
    if let Some(account) = &accounts.lastfm {
        let mut params = BTreeMap::from([("method".into(), "track.updateNowPlaying".into()), ("artist".into(), main_artist(&listen.artist).to_owned()), ("track".into(), listen.track.clone())]);
        if let Some(album) = &listen.album { params.insert("album".into(), album.clone()); }
        if let Some(seconds) = listen.seconds { params.insert("duration".into(), seconds.to_string()); }
        let _ = lastfm(youtube, params, &account.key, &account.secret, Some(&account.session)).await;
    }
}

/// Sends what waits for `service`, a batch at a time, and says how many of the first went.
pub async fn send(youtube: &surco_youtube::YouTube, accounts: &Accounts, service: Service) -> usize {
    let mut sent = 0;
    match service {
        Service::ListenBrainz => {
            let Some(account) = &accounts.listenbrainz else { return 0 };
            for batch in account.waiting.chunks(LISTENBRAINZ_BATCH) {
                let kind = if batch.len() == 1 { "single" } else { "import" };
                let answer = youtube.http().post(format!("{LISTENBRAINZ}/1/submit-listens")).header("Authorization", format!("Token {}", account.token))
                    .json(&listenbrainz_body(kind, batch)).send().await;
                if !answer.is_ok_and(|answer| answer.status().is_success()) { break; }
                sent += batch.len();
            }
        }
        Service::LastFm => {
            let Some(account) = &accounts.lastfm else { return 0 };
            for batch in account.waiting.chunks(LASTFM_BATCH) {
                let mut params = BTreeMap::from([("method".to_owned(), "track.scrobble".to_owned())]);
                for (index, listen) in batch.iter().enumerate() {
                    params.insert(format!("artist[{index}]"), main_artist(&listen.artist).to_owned());
                    params.insert(format!("track[{index}]"), listen.track.clone());
                    params.insert(format!("timestamp[{index}]"), listen.at.to_string());
                    if let Some(album) = &listen.album { params.insert(format!("album[{index}]"), album.clone()); }
                    if let Some(seconds) = listen.seconds { params.insert(format!("duration[{index}]"), seconds.to_string()); }
                }
                let answer = lastfm(youtube, params, &account.key, &account.secret, Some(&account.session)).await;
                if !answer.is_ok_and(|answer| answer.get("scrobbles").is_some()) { break; }
                sent += batch.len();
            }
        }
    }
    sent
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_song_is_sent_once_heard_as_lastfm_asks() {
        assert!(!sends(20.0, 25.0), "a song of 30 seconds or less never is");
        assert!(!sends(99.0, 200.0) && sends(100.0, 200.0), "half of it");
        assert!(sends(240.0, 900.0), "or four minutes of a long one");
    }

    #[test]
    fn lastfm_signs_the_sorted_parameters_and_the_secret() {
        // md5("") is d41d8cd98f00b204e9800998ecf8427e; this is md5("api_keyAmethodBsecret").
        let params = BTreeMap::from([("method".to_owned(), "B".to_owned()), ("api_key".to_owned(), "A".to_owned())]);
        let expected: String = Md5::digest(b"api_keyAmethodBsecret").iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(signature(&params, "secret"), expected);
        assert_eq!(Md5::digest(b"").iter().map(|byte| format!("{byte:02x}")).collect::<String>(), "d41d8cd98f00b204e9800998ecf8427e");
    }

    #[test]
    fn listenbrainz_takes_the_listen_and_a_playing_now_without_its_time() {
        let listen = Listen { artist: "Soda Stereo".into(), track: "Persiana americana".into(), album: Some("Signos".into()), at: 1_000, seconds: Some(290), video_id: "abc".into() };
        let body = listenbrainz_body("single", std::slice::from_ref(&listen));
        assert_eq!(body["listen_type"], "single");
        assert_eq!(body["payload"][0]["listened_at"], 1_000);
        assert_eq!(body["payload"][0]["track_metadata"]["release_name"], "Signos");
        assert_eq!(body["payload"][0]["track_metadata"]["additional_info"]["duration_ms"], 290_000);
        assert_eq!(body["payload"][0]["track_metadata"]["additional_info"]["origin_url"], "https://music.youtube.com/watch?v=abc");
        assert!(listenbrainz_body("playing_now", &[listen])["payload"][0].get("listened_at").is_none());
        assert_eq!(main_artist("Cerati, Soda Stereo"), "Cerati");
    }
}
