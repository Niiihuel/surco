//! LRCLIB requests are kept separate from the player/audio lifetime.

use std::time::{Duration, Instant};
use std::sync::Mutex;
use surco_core::{Track, lyrics::{Line, parse_lrc}};
use surco_youtube::YouTube;

const ENDPOINT: &str = "https://lrclib.net/api/search";
const TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, PartialEq)]
pub enum Found {
    Synced(Vec<Line>),
    Plain(Vec<String>),
}

#[derive(Debug, PartialEq)]
pub struct Failure {
    pub message: String,
    pub retry_after: Duration,
}

impl Failure {
    fn limited(wait: Duration) -> Self {
        let seconds = wait.as_secs().saturating_add(u64::from(wait.subsec_nanos() > 0)).max(1);
        Self { message: format!("El servicio de letras alcanzó su límite. Esperá {seconds} segundos y volvé a intentar."), retry_after: wait }
    }
}

/// Rate limits apply to the client, including when the user changes tracks.
#[derive(Default)]
pub struct Provider {
    blocked_until: Mutex<Option<Instant>>,
}

impl Provider {
    pub async fn fetch(&self, youtube: &YouTube, track: &Track) -> Result<Option<Found>, Failure> {
        self.fetch_from(youtube, track, ENDPOINT, TIMEOUT).await
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

async fn fetch_at(youtube: &YouTube, track: &Track, endpoint: &str, timeout: Duration) -> Result<Option<Found>, Failure> {
    let transport = |timed_out: bool| Failure {
        message: if timed_out { "El servicio de letras tardó demasiado. Volvé a intentar." }
            else { "No pudimos conectar con el servicio de letras. Revisá la conexión y volvé a intentar." }.into(),
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
            else { Failure { message: format!("El servicio de letras no está disponible (HTTP {}). Volvé a intentar.", response.status().as_u16()), retry_after: Duration::ZERO } });
    }
    // RequestBuilder's deadline includes the body, not just DNS/connection/headers.
    let body = response.bytes().await.map_err(|error| transport(error.is_timeout()))?;
    let found: Vec<serde_json::Value> = serde_json::from_slice(&body).map_err(|_| Failure {
        message: "El servicio de letras envió una respuesta que no pudimos leer. Volvé a intentar.".into(), retry_after: Duration::ZERO,
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
        .map(|text| text.trim().lines().map(|line| line.trim().to_owned()).collect::<Vec<_>>())
        .find(|lines| lines.iter().any(|line| !line.is_empty()));
    Ok(plain.map(Found::Plain))
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
        assert_eq!(found, Some(Found::Plain(vec!["First line".into(), "".into(), "Second line".into()])));
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
    async fn a_malformed_provider_response_is_not_a_catalog_miss() {
        let (endpoint, request, worker) = server("200 OK", "", "not json", Duration::ZERO);
        let failure = fetch_at(&YouTube::new().unwrap(), &track(), &endpoint, Duration::from_secs(2)).await.unwrap_err();
        assert!(failure.message.contains("respuesta"));
        request.recv().unwrap();
        worker.join().unwrap();
    }
}
