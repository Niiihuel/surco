//! LyricsPlus, BetterLyrics and LRCLIB requests are kept separate from the player/audio lifetime.

use std::time::{Duration, Instant};
use std::sync::Mutex;
use std::collections::VecDeque;
use std::future::Future;
use surco_core::{Track, lyrics::{Line, parse_lrc, words_from_runs}};
use surco_youtube::YouTube;
use crate::i18n::t;

const ENDPOINT: &str = "https://lrclib.net/api/search";
const TIMEOUT: Duration = Duration::from_secs(12);
/// LyricsPlus, the open backend behind YouLy+ that BitChord asks first: lyrics timed by the
/// word or the syllable, from Apple Music, Musixmatch and QQ. It lives on volunteers' mirrors,
/// any of which may be down or lack a song another has; these two answered on 2026-11-04.
const LYRICS_PLUS: [&str; 2] = ["https://lyricsplus.binimum.org", "https://lyricsplus.prjktla.my.id"];
const LYRICS_PLUS_TIMEOUT: Duration = Duration::from_secs(8);
/// BetterLyrics, which BitChord asks too: Apple Music's lyrics as TTML, timed by the word for
/// most songs. On 2026-11-05 it had songs LyricsPlus's one mirror still up lacked.
const BETTER_LYRICS: &str = "https://lyrics-api.boidu.dev/getLyrics";
const USER_AGENT: &str = concat!("Surco/", env!("CARGO_PKG_VERSION"), " (https://github.com/Niiihuel/surco)");

#[derive(Clone, Debug, PartialEq)]
pub enum Found {
    Synced(Vec<Line>),
    Plain { lines: Vec<String>, source: String },
}

#[derive(Debug, PartialEq)]
pub struct Failure {
    pub message: String,
    pub retry_after: Duration,
}

impl Failure {
    fn limited(wait: Duration) -> Self {
        let seconds = wait.as_secs().saturating_add(u64::from(wait.subsec_nanos() > 0)).max(1);
        Self { message: t!("backend-lyrics-limited", seconds = seconds).into_owned(), retry_after: wait }
    }
}

/// Rate limits apply to the client, including when the user changes tracks.
#[derive(Default)]
pub struct Provider {
    blocked_until: Mutex<Option<Instant>>,
    successful: Mutex<VecDeque<(Track, Found)>>,
}

impl Provider {
    pub async fn fetch(&self, youtube: &YouTube, track: &Track) -> Result<Option<Found>, Failure> {
        self.cached(track, async {
            // Asked together, so the wait is the slowest one's. Words timed one by one win, as
            // a phone's lyrics fill with them, LyricsPlus's before BetterLyrics's; then LRCLIB's
            // timed lines, then either's.
            let (plus, better, primary) = tokio::join!(lyrics_plus(youtube, track), better_lyrics(youtube, track), self.fetch_from(youtube, track, ENDPOINT, TIMEOUT));
            let word_timed = |lines: &Option<Vec<Line>>| lines.as_ref().is_some_and(|lines| lines.iter().any(|line| !line.words.is_empty()));
            let (plus_timed, better_timed) = (word_timed(&plus), word_timed(&better));
            let primary = match (plus, better, primary) {
                (Some(lines), _, _) if plus_timed => Ok(Some(Found::Synced(lines))),
                (_, Some(lines), _) if better_timed => Ok(Some(Found::Synced(lines))),
                (_, _, Ok(Some(found @ Found::Synced(_)))) => Ok(Some(found)),
                (Some(lines), _, _) | (None, Some(lines), _) => Ok(Some(Found::Synced(lines))),
                (None, None, primary) => primary,
            };
            resolve(primary, async {
                let found = tokio::time::timeout(Duration::from_secs(8), youtube.lyrics(&track.id)).await;
                match found {
                    Ok(Ok((words, source))) => {
                        let lines = plain_lines(&words);
                        Ok((!lines.is_empty()).then_some(Found::Plain { lines, source }))
                    }
                    _ => Err(Failure { message: t!("backend-lyrics-youtube-failed").into_owned(), retry_after: Duration::ZERO }),
                }
            }).await
        }).await
    }

    async fn cached(&self, track: &Track, fetch: impl Future<Output = Result<Option<Found>, Failure>>) -> Result<Option<Found>, Failure> {
        if let Some((_, found)) = self.successful.lock().unwrap().iter().find(|(asked, _)| asked == track) {
            return Ok(Some(found.clone()));
        }
        let result = fetch.await;
        if let Ok(Some(found)) = &result {
            let mut saved = self.successful.lock().unwrap();
            if !matches!(found, Found::Synced(_)) {
                if let Some((_, available)) = saved.iter().find(|(asked, available)| asked == track && matches!(available, Found::Synced(_))) {
                    return Ok(Some(available.clone()));
                }
            }
            saved.retain(|(asked, _)| asked != track);
            saved.push_back((track.clone(), found.clone()));
            while saved.len() > 32 { saved.pop_front(); }
        }
        result
    }

    async fn fetch_from(&self, youtube: &YouTube, track: &Track, endpoint: &str, timeout: Duration) -> Result<Option<Found>, Failure> {
        if let Some(wait) = self.blocked_until.lock().unwrap()
            .and_then(|until| until.checked_duration_since(Instant::now())) {
            return Err(Failure::limited(wait));
        }
        let result = fetch_at(youtube, track, endpoint, timeout).await;
        if let Err(failure) = &result {
            if !failure.retry_after.is_zero() {
                let now = Instant::now();
                let until = now.checked_add(failure.retry_after).unwrap_or(now + Duration::from_secs(86_400));
                let mut blocked = self.blocked_until.lock().unwrap();
                *blocked = Some(blocked.map_or(until, |existing| existing.max(until)));
            }
        }
        result
    }
}

async fn resolve(primary: Result<Option<Found>, Failure>, fallback: impl Future<Output = Result<Option<Found>, Failure>>) -> Result<Option<Found>, Failure> {
    if matches!(primary, Ok(Some(_))) { return primary; }
    match fallback.await {
        Ok(Some(found)) => Ok(Some(found)),
        _ => primary,
    }
}

/// Both mirrors at once, for each name the song's artist may be catalogued under in turn: the
/// first answer with its words timed, or else the first with its lines.
async fn lyrics_plus(youtube: &YouTube, track: &Track) -> Option<Vec<Line>> {
    let word_timed = |lines: &Option<Vec<Line>>| lines.as_ref().is_some_and(|lines| lines.iter().any(|line| !line.words.is_empty()));
    let title = search_title(&track.title);
    let mut found = None;
    for artist in search_artists(&track.artist) {
        let (first, second) = tokio::join!(lyrics_plus_at(youtube, track, LYRICS_PLUS[0], &title, artist), lyrics_plus_at(youtube, track, LYRICS_PLUS[1], &title, artist));
        let lines = if word_timed(&second) && !word_timed(&first) { second } else { first.or(second) };
        if word_timed(&lines) { return lines; }
        found = found.or(lines);
    }
    found
}

/// A title as a catalogue files it, as BitChord asks for it: without its credits and its
/// packaging, "(feat. …)" or "(Official Video)", though never without what names another
/// recording, "(Remix)" or "(Live)", whose words would be wrong rather than missing.
fn search_title(title: &str) -> String {
    const CREDITS: [&str; 4] = ["feat", "ft", "featuring", "with"];
    const PACKAGING: [&str; 15] = ["official", "music", "video", "audio", "lyric", "lyrics", "visualizer", "visualiser", "mv", "m/v", "hd", "hq", "4k", "full", "song"];
    let mut kept = String::new();
    let mut rest = title;
    while let Some(open) = rest.find(['(', '[']) {
        let Some(close) = rest[open..].find([')', ']']).map(|end| open + end) else { break };
        let inside = rest[open + 1..close].to_lowercase();
        let credit = inside.split(|c: char| !c.is_alphanumeric()).find(|word| !word.is_empty()).is_some_and(|word| CREDITS.contains(&word));
        let packaging = inside.split_whitespace().all(|word| PACKAGING.contains(&word.trim_matches('.')));
        kept.push_str(&rest[..if credit || packaging { open } else { close + 1 }]);
        rest = &rest[close + 1..];
    }
    kept.push_str(rest);
    // ASCII only, so its offsets are the title's own.
    let lower = kept.to_ascii_lowercase();
    let end = [" feat. ", " feat ", " ft. ", " featuring "].into_iter().filter_map(|mark| lower.find(mark)).min().unwrap_or(kept.len());
    let name = kept[..end].split_whitespace().collect::<Vec<_>>().join(" ");
    let name = name.trim_end_matches([',', '-', '–', '—', ' ']);
    if name.is_empty() { title.trim().to_owned() } else { name.to_owned() }
}

/// The names a song's artist may be catalogued under, the likelier first: the first one its
/// credit names, then, where that joins two with "&" as YouTube joins a song's artists, the
/// one before it. "Simon & Garfunkel" is one artist, so the whole is asked for first.
fn search_artists(credit: &str) -> Vec<&str> {
    let mut first = credit.trim().trim_end_matches(" - Topic");
    for separator in [", ", " feat. ", " ft. ", " featuring "] {
        if let Some((before, _)) = first.split_once(separator) { first = before; }
    }
    let first = first.trim();
    let mut names = vec![first];
    names.extend(first.split_once(" & ").map(|(before, _)| before.trim()).filter(|before| !before.is_empty()));
    names
}

/// A mirror's lines for `track` under `title` and `artist`; none when it is down, slow or has
/// not got the song, as LRCLIB still stands behind it.
async fn lyrics_plus_at(youtube: &YouTube, track: &Track, host: &str, title: &str, artist: &str) -> Option<Vec<Line>> {
    let mut query = vec![("title", title.to_owned()), ("artist", artist.to_owned())];
    if track.duration_ms >= 1000 { query.push(("duration", (track.duration_ms / 1000).to_string())); }
    if let Some(album) = track.album.as_ref().filter(|album| !album.is_empty()) { query.push(("album", album.clone())); }
    let response = youtube.http().get(format!("{host}/v2/lyrics/get")).header("User-Agent", USER_AGENT)
        .timeout(LYRICS_PLUS_TIMEOUT).query(&query).send().await.ok()?;
    if !response.status().is_success() { return None; }
    let lines = plus_lines(&serde_json::from_slice(&response.bytes().await.ok()?).ok()?);
    (!lines.is_empty()).then_some(lines)
}

/// BetterLyrics's lines for `track`, for each name its artist may be catalogued under in turn;
/// none when it is down, slow or has not got the song.
async fn better_lyrics(youtube: &YouTube, track: &Track) -> Option<Vec<Line>> {
    let title = search_title(&track.title);
    for artist in search_artists(&track.artist) {
        let mut query = vec![("s", title.clone()), ("a", artist.to_owned())];
        if track.duration_ms >= 1000 { query.push(("d", (track.duration_ms / 1000).to_string())); }
        if let Some(album) = track.album.as_ref().filter(|album| !album.is_empty()) { query.push(("al", album.clone())); }
        let request = youtube.http().get(BETTER_LYRICS).header("User-Agent", USER_AGENT).timeout(LYRICS_PLUS_TIMEOUT).query(&query);
        let Some(response) = request.send().await.ok().filter(|response| response.status().is_success()) else { continue };
        let Some(answer) = response.bytes().await.ok().and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok()) else { continue };
        let lines = ttml_lines(answer["ttml"].as_str().unwrap_or_default());
        if !lines.is_empty() { return Some(lines); }
    }
    None
}

/// Apple Music's TTML as lines, in the order they are sung: each `<p>` a line, its `<span>`s
/// the words or syllables it is timed by, glued into words where no space parts them; a line
/// timed only as a whole keeps its text alone. Background vocals are left out, as the lead's
/// line is what fills.
fn ttml_lines(ttml: &str) -> Vec<Line> {
    const METADATA: &str = "http://www.w3.org/ns/ttml#metadata";
    let Ok(document) = roxmltree::Document::parse(ttml) else { return Vec::new() };
    let time = |node: roxmltree::Node, name: &str| node.attribute(name).and_then(ttml_ms);
    let lead = |node: &roxmltree::Node| node.attribute((METADATA, "role")) != Some("x-bg");
    let mut lines: Vec<Line> = document.descendants().filter(|node| node.has_tag_name("p")).filter_map(|p| {
        let at = time(p, "begin")?;
        // A space between two spans ends the word in hand; spans side by side are its syllables.
        let runs: Vec<(u64, u64, String)> = p.children().filter(lead).filter_map(|child| match child.text() {
            Some(text) if child.is_text() => Some((0, 0, if text.trim().is_empty() { text.to_owned() } else { " ".to_owned() })),
            _ if child.has_tag_name("span") => Some((time(child, "begin")?, time(child, "end")?,
                child.descendants().filter(|node| node.is_text()).filter_map(|node| node.text()).collect())),
            _ => None,
        }).collect();
        let words = words_from_runs(&runs.iter().map(|(start, end, text)| (*start, *end, text.as_str())).collect::<Vec<_>>());
        if let Some(first) = words.first() {
            let text = words.iter().map(|word| word.text.as_str()).collect::<Vec<_>>().join(" ");
            return Some(Line { at_ms: at.min(first.start_ms), text, words });
        }
        let text: String = p.children().filter(lead).flat_map(|child| child.descendants()).filter(|node| node.is_text()).filter_map(|node| node.text()).collect();
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        (!text.is_empty()).then(|| Line { at_ms: at, text, words: Vec::new() })
    }).collect();
    lines.sort_by_key(|line| line.at_ms);
    lines
}

/// A TTML clock value in milliseconds: `26.158`, `1:02.5`, `00:01:02.5` or `26.158s`.
fn ttml_ms(value: &str) -> Option<u64> {
    let seconds = value.trim().trim_end_matches('s').split(':').try_fold(0.0, |total: f64, part| Some(total * 60.0 + part.parse::<f64>().ok()?))?;
    (seconds.is_finite() && seconds >= 0.0).then(|| (seconds * 1000.0).round() as u64)
}

/// LyricsPlus's v2 answer as lines, in the order they are sung: each line's words glued back
/// from the syllables it comes in, or the line alone where only it is timed.
fn plus_lines(answer: &serde_json::Value) -> Vec<Line> {
    let mut lines: Vec<Line> = answer["lyrics"].as_array().into_iter().flatten().filter_map(|line| {
        let at = line["time"].as_u64()?;
        let runs: Vec<(u64, u64, &str)> = line["syllabus"].as_array().into_iter().flatten().filter_map(|run| {
            let start = run["time"].as_u64()?;
            Some((start, start + run["duration"].as_u64().unwrap_or(0), run["text"].as_str()?))
        }).collect();
        let words = words_from_runs(&runs);
        if let Some(first) = words.first() {
            let text = words.iter().map(|word| word.text.as_str()).collect::<Vec<_>>().join(" ");
            return Some(Line { at_ms: at.min(first.start_ms), text, words });
        }
        let text = line["text"].as_str()?.split_whitespace().collect::<Vec<_>>().join(" ");
        (!text.is_empty()).then(|| Line { at_ms: at, text, words: Vec::new() })
    }).collect();
    lines.sort_by_key(|line| line.at_ms);
    lines
}

fn plain_lines(text: &str) -> Vec<String> {
    text.trim().lines().map(|line| line.trim().to_owned()).collect()
}

async fn fetch_at(youtube: &YouTube, track: &Track, endpoint: &str, timeout: Duration) -> Result<Option<Found>, Failure> {
    let transport = |timed_out: bool| Failure {
        message: if timed_out { t!("backend-lyrics-timeout") } else { t!("backend-lyrics-unreachable") }.into_owned(),
        retry_after: Duration::ZERO,
    };
    let response = youtube.http().get(endpoint)
        .header("User-Agent", USER_AGENT)
        .timeout(timeout)
        .query(&[("artist_name", &track.artist), ("track_name", &track.title)])
        .send().await.map_err(|error| transport(error.is_timeout()))?;
    if !response.status().is_success() {
        let limited = response.status().as_u16() == 429;
        let seconds = response.headers().get("Retry-After").and_then(|value| value.to_str().ok()).and_then(|value| value.parse::<u64>().ok()).unwrap_or(30);
        return Err(if limited { Failure::limited(Duration::from_secs(seconds.max(1))) }
            else { Failure { message: t!("backend-lyrics-unavailable", status = response.status().as_u16()).into_owned(), retry_after: Duration::ZERO } });
    }
    // RequestBuilder's deadline includes the body, not just DNS/connection/headers.
    let body = response.bytes().await.map_err(|error| transport(error.is_timeout()))?;
    let found: Vec<serde_json::Value> = serde_json::from_slice(&body).map_err(|_| Failure {
        message: t!("backend-lyrics-unreadable").into_owned(), retry_after: Duration::ZERO,

    })?;
    let seconds = track.duration_ms as f64 / 1000.0;
    let apart = |entry: &&serde_json::Value| (entry["duration"].as_f64().unwrap_or(f64::INFINITY) - seconds).abs();
    let mut ranked: Vec<_> = found.iter().collect();
    ranked.sort_by(|a, b| apart(a).total_cmp(&apart(b)));
    // A malformed/empty nearest record must not hide a valid synchronized one.
    if let Some(lines) = ranked.iter().filter_map(|entry| entry["syncedLyrics"].as_str())
        .map(parse_lrc).find(|lines| !lines.is_empty()) {
        return Ok(Some(Found::Synced(lines)));
    }
    // Plain text is useful too, but it never receives fabricated seek positions.
    let plain = ranked.iter().filter_map(|entry| entry["plainLyrics"].as_str())
        .map(plain_lines)
        .find(|lines| lines.iter().any(|line| !line.is_empty()));
    Ok(plain.map(|lines| Found::Plain { lines, source: "LRCLIB".into() }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::{Read, Write}, net::TcpListener, sync::mpsc, thread};

    #[test]
    fn apple_ttml_is_read_into_words_lines_and_their_times() {
        let ttml = r#"<tt xmlns="http://www.w3.org/ns/ttml" xmlns:ttm="http://www.w3.org/ns/ttml#metadata"><body><div>
            <p begin="1:02.5" end="1:05"><span begin="1:02.5" end="1:03">Well,</span> <span begin="1:03" end="1:03.4">beau</span><span begin="1:03.4" end="1:04">ti</span><span begin="1:04" end="1:04.5">ful</span> <span ttm:role="x-bg"><span begin="1:04.5" end="1:05">(ooh)</span></span></p>
            <p begin="26.158s" end="28.9">A line   timed whole</p>
            <p end="30">Untimed, left out</p>
        </div></body></tt>"#;
        let lines = ttml_lines(ttml);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!((lines[0].at_ms, lines[0].text.as_str(), lines[0].words.is_empty()), (26_158, "A line timed whole", true));
        let sung = &lines[1];
        assert_eq!((sung.at_ms, sung.text.as_str()), (62_500, "Well, beautiful"), "syllables side by side are one word, the background vocal is left out");
        assert_eq!(sung.words.iter().map(|word| (word.start_ms, word.end_ms)).collect::<Vec<_>>(), vec![(62_500, 63_000), (63_000, 64_500)]);
        assert!(ttml_lines("not xml").is_empty());
        assert_eq!((ttml_ms("00:01:02.5"), ttml_ms("26.158s"), ttml_ms("-1"), ttml_ms("x")), (Some(62_500), Some(26_158), None, None));
    }

    fn track() -> Track {
        Track { id: "song".into(), title: "Song".into(), artist: "Artist".into(),
            artist_id: None, album: None, album_id: None, artwork_url: None, duration_ms: 215_000 }
    }

    // Real HTTP, with the provider boundary local and deterministic. No sound device or window.
    fn server(status: &str, headers: &str, body: &str, delay_body: Duration) -> (String, mpsc::Receiver<String>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/api/search", listener.local_addr().unwrap());
        let (sender, received) = mpsc::channel();
        let response = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n{headers}\r\n", body.len());
        let body = body.to_owned();
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                assert_eq!(stream.read(&mut byte).unwrap(), 1);
                request.push(byte[0]);
            }
            sender.send(String::from_utf8(request).unwrap()).unwrap();
            stream.write_all(response.as_bytes()).unwrap();
            thread::sleep(delay_body);
            // The timeout test closes the socket before this body arrives.
            let _ = stream.write_all(body.as_bytes());
        });
        (endpoint, received, worker)
    }

    #[tokio::test]
    async fn a_provider_request_identifies_the_app_and_decodes_real_http() {
        let body = r#"[{"trackName":"Song","artistName":"Artist","duration":215,"syncedLyrics":"[00:01.00]test line"}]"#;
        let (endpoint, request, worker) = server("200 OK", "", body, Duration::ZERO);
        let lines = fetch_at(&YouTube::new().unwrap(), &track(), &endpoint, Duration::from_secs(2)).await.unwrap().unwrap();
        assert_eq!(lines, Found::Synced(vec![Line { words: Vec::new(), at_ms: 1000, text: "test line".into() }]));
        let request = request.recv().unwrap().to_lowercase();
        assert!(request.contains("user-agent: surco/"), "LRCLIB requires an identified client");
        assert!(request.contains("github.com/niiihuel/surco"));
        worker.join().unwrap();
    }

    #[test]
    fn a_song_is_asked_for_by_its_catalogued_names() {
        for (title, asked) in [("Dracula (feat. JENNIE)", "Dracula"), ("Song [Official Music Video]", "Song"), ("Tití Me Preguntó (Remix)", "Tití Me Preguntó (Remix)"),
            ("Persiana Americana (Live) (Official Video)", "Persiana Americana (Live)"), ("Song feat. Someone", "Song"), ("(Official Video)", "(Official Video)"),
            ("İSTANBUL feat. Ünlü", "İSTANBUL")] {
            assert_eq!(search_title(title), asked, "{title}");
        }
        assert_eq!(search_artists("Luis Fonsi & Daddy Yankee"), ["Luis Fonsi & Daddy Yankee", "Luis Fonsi"]);
        assert_eq!(search_artists("Bizarrap, Shakira"), ["Bizarrap"]);
        assert_eq!(search_artists("Soda Stereo - Topic"), ["Soda Stereo"]);
    }

    #[test]
    fn lyrics_plus_lines_keep_their_words_timed_and_lines_timed_alone_as_they_are() {
        let answer: serde_json::Value = serde_json::from_str(r#"{"type":"Word","lyrics":[
            {"time":17612,"duration":1649,"text":"Ti-ri, Daddy","syllabus":[
                {"time":17612,"duration":177,"text":"Ti-"},{"time":17789,"duration":234,"text":"ri, "},{"time":18848,"duration":413,"text":"Daddy"}]},
            {"time":11884,"duration":1353,"text":"  Me verás   volar ","syllabus":[]},
            {"time":20000,"syllabus":[],"text":""},
            {"text":"no time"}]}"#).unwrap();
        let lines = plus_lines(&answer);
        assert_eq!(lines.iter().map(|line| (line.at_ms, line.text.as_str())).collect::<Vec<_>>(), [(11_884, "Me verás volar"), (17_612, "Ti-ri, Daddy")]);
        assert!(lines[0].words.is_empty(), "a line timed alone");
        let words: Vec<_> = lines[1].words.iter().map(|word| (word.start_ms, word.end_ms, word.text.as_str())).collect();
        assert_eq!(words, [(17_612, 18_023, "Ti-ri,"), (18_848, 19_261, "Daddy")]);
        assert!(plus_lines(&serde_json::json!({"error": {"status": 404}})).is_empty());
    }

    #[tokio::test]
    async fn rate_limiting_is_a_retryable_failure_and_preserves_the_requested_wait() {
        let (endpoint, request, worker) = server("429 Too Many Requests", "Retry-After: 7\r\n", "{}", Duration::ZERO);
        let failure = fetch_at(&YouTube::new().unwrap(), &track(), &endpoint, Duration::from_secs(2)).await.unwrap_err();
        assert_eq!(failure.retry_after, Duration::from_secs(7));
        assert!(failure.message.contains("límite"));
        request.recv().unwrap();
        worker.join().unwrap();
    }

    #[tokio::test]
    async fn a_timeout_while_reading_the_body_finishes_with_an_error() {
        let (endpoint, request, worker) = server("200 OK", "", "[]", Duration::from_millis(200));
        let failure = fetch_at(&YouTube::new().unwrap(), &track(), &endpoint, Duration::from_millis(30)).await.unwrap_err();
        assert!(failure.message.contains("tardó"));
        request.recv().unwrap();
        worker.join().unwrap();
    }

    #[tokio::test]
    async fn invalid_timing_in_the_nearest_record_does_not_hide_another_valid_record() {
        let body = r#"[{"duration":215,"syncedLyrics":"not a timed lyric"},{"duration":216,"syncedLyrics":"[00:02.00]valid line"}]"#;
        let (endpoint, request, worker) = server("200 OK", "", body, Duration::ZERO);
        let found = fetch_at(&YouTube::new().unwrap(), &track(), &endpoint, Duration::from_secs(2)).await.unwrap();
        assert_eq!(found, Some(Found::Synced(vec![Line { words: Vec::new(), at_ms: 2000, text: "valid line".into() }])));
        request.recv().unwrap(); worker.join().unwrap();
    }

    #[tokio::test]
    async fn plain_lyrics_are_available_without_inventing_a_timeline() {
        let body = r#"[{"duration":215,"syncedLyrics":null,"plainLyrics":"First line\n\nSecond line"}]"#;
        let (endpoint, request, worker) = server("200 OK", "", body, Duration::ZERO);
        let found = fetch_at(&YouTube::new().unwrap(), &track(), &endpoint, Duration::from_secs(2)).await.unwrap();
        assert_eq!(found, Some(Found::Plain { lines: vec!["First line".into(), "".into(), "Second line".into()], source: "LRCLIB".into() }));
        request.recv().unwrap(); worker.join().unwrap();
    }

    #[tokio::test]
    async fn changing_tracks_during_a_rate_limit_does_not_issue_another_http_request() {
        let (endpoint, request, worker) = server("429 Too Many Requests", "Retry-After: 7\r\n", "{}", Duration::ZERO);
        let provider = Provider::default();
        let youtube = YouTube::new().unwrap();
        let first = provider.fetch_from(&youtube, &track(), &endpoint, Duration::from_secs(2)).await.unwrap_err();
        assert_eq!(first.retry_after, Duration::from_secs(7));
        request.recv().unwrap(); worker.join().unwrap();
        // The listener no longer exists: any second network request would be a connection error.
        let next = Track { id: "another".into(), ..track() };
        let failure = provider.fetch_from(&youtube, &next, &endpoint, Duration::from_secs(2)).await.unwrap_err();
        assert!(failure.retry_after > Duration::from_secs(6), "the client preserves the remaining cooldown");
        assert!(failure.message.contains("límite"));
    }

    #[tokio::test]
    async fn an_available_video_lyric_recovers_a_limited_primary_provider() {
        let recovered = resolve(Err(Failure::limited(Duration::from_secs(7))), async {
            Ok(Some(Found::Plain { lines: vec!["Actual fallback words".into()], source: "Source: Test".into() }))
        }).await.unwrap();
        assert!(matches!(recovered, Some(Found::Plain { source, .. }) if source == "Source: Test"));
    }

    #[tokio::test]
    async fn synchronized_words_do_not_request_a_secondary_provider_and_both_failures_keep_the_primary_wait() {
        let lines = vec![Line { words: Vec::new(), at_ms: 1000, text: "Timed words".into() }];
        let found = resolve(Ok(Some(Found::Synced(lines.clone()))), async { panic!("must not poll fallback for available lyrics") }).await.unwrap();
        assert_eq!(found, Some(Found::Synced(lines)));
        let failed = resolve(Err(Failure::limited(Duration::from_secs(7))), async {
            Err(Failure { message: "secondary unavailable".into(), retry_after: Duration::ZERO })
        }).await.unwrap_err();
        assert_eq!(failed.retry_after, Duration::from_secs(7));
    }

    #[tokio::test]
    async fn successful_lyrics_are_reused_across_tracks_without_another_request() {
        let provider = Provider::default();
        let first = track();
        let second = Track { id: "second".into(), ..track() };
        let fetched = std::cell::Cell::new(0);
        for song in [&first, &second, &first] {
            let result = provider.cached(song, async {
                fetched.set(fetched.get() + 1);
                Ok(Some(Found::Plain { lines: vec!["Words".into()], source: "Test".into() }))
            }).await.unwrap();
            assert!(result.is_some());
        }
        assert_eq!(fetched.get(), 2, "returning to a song must reuse successful lyrics");
    }

    #[tokio::test]
    async fn a_late_plain_fallback_cannot_downgrade_a_cached_synchronized_result() {
        let provider = Provider::default();
        let track = track();
        let (done, signal) = tokio::sync::oneshot::channel();
        let late = provider.cached(&track, async {
            signal.await.unwrap();
            Ok(Some(Found::Plain { lines: vec!["Untimed".into()], source: "Test".into() }))
        });
        let synced = provider.cached(&track, async {
            done.send(()).unwrap();
            Ok(Some(Found::Synced(vec![Line { words: Vec::new(), at_ms: 1000, text: "Timed".into() }])))
        });
        let _ = tokio::join!(late, synced);
        let found = provider.cached(&track, async { panic!("already cached") }).await.unwrap();
        assert!(matches!(found, Some(Found::Synced(_))));
    }

    #[tokio::test]
    async fn a_malformed_provider_response_is_not_a_catalog_miss() {
        let (endpoint, request, worker) = server("200 OK", "", "not json", Duration::ZERO);
        let failure = fetch_at(&YouTube::new().unwrap(), &track(), &endpoint, Duration::from_secs(2)).await.unwrap_err();
        assert!(failure.message.contains("respuesta"));
        request.recv().unwrap();
        worker.join().unwrap();
    }
}
