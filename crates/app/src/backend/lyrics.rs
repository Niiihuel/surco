//! LRCLIB requests are kept separate from the player/audio lifetime.

use std::time::{Duration, Instant};
use std::sync::Mutex;
use std::collections::VecDeque;
use std::future::Future;
use surco_core::{Track, lyrics::{Line, parse_lrc}};
use surco_youtube::YouTube;
use crate::i18n::t;

const ENDPOINT: &str = "https://lrclib.net/api/search";
const TIMEOUT: Duration = Duration::from_secs(12);

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
            let primary = self.fetch_from(youtube, track, ENDPOINT, TIMEOUT).await;
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

fn plain_lines(text: &str) -> Vec<String> {
    text.trim().lines().map(|line| line.trim().to_owned()).collect()
}

async fn fetch_at(youtube: &YouTube, track: &Track, endpoint: &str, timeout: Duration) -> Result<Option<Found>, Failure> {
    let transport = |timed_out: bool| Failure {
        message: if timed_out { t!("backend-lyrics-timeout") } else { t!("backend-lyrics-unreachable") }.into_owned(),
        retry_after: Duration::ZERO,
    };
    let response = youtube.http().get(endpoint)
        .header("User-Agent", concat!("Surco/", env!("CARGO_PKG_VERSION"), " (https://github.com/Niiihuel/surco)"))
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
        assert_eq!(lines, Found::Synced(vec![Line { at_ms: 1000, text: "test line".into() }]));
        let request = request.recv().unwrap().to_lowercase();
        assert!(request.contains("user-agent: surco/"), "LRCLIB requires an identified client");
        assert!(request.contains("github.com/niiihuel/surco"));
        worker.join().unwrap();
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
        assert_eq!(found, Some(Found::Synced(vec![Line { at_ms: 2000, text: "valid line".into() }])));
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
        let lines = vec![Line { at_ms: 1000, text: "Timed words".into() }];
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
            Ok(Some(Found::Synced(vec![Line { at_ms: 1000, text: "Timed".into() }])))
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
