# Lyrics loading investigation — 2026-10-15

The physical iPhone report is that lyrics never appear. No physical-network trace
was supplied, so this investigation does not claim a single iPhone-only root cause.

## Reproduction with the real catalog and provider

The existing Rust YouTube probe returned `eVRbaYq5Aw4`, `Sunsetz`,
`Cigarettes After Sex`, duration `215000 ms`. An HTTPS request to
`https://lrclib.net/api/search` with that artist/title returned HTTP 200 and 20
records with synchronized lyrics. Four records matched the 215-second duration.
No lyric text is retained in this document.

A subsequent request returned HTTP 429, `Retry-After: 1`, and the provider's
`RateLimited` error. The previous backend converted every HTTP/network/JSON failure
into the same `None` as a successful catalog miss. State then remembered that miss
for the whole current track, with no retry action. Requests also had no deadline,
so a stalled response could leave the card in loading state indefinitely.

LRCLIB's [API documentation](https://lrclib.net/docs) requires an identifying
User-Agent containing the app/version/project, describes rate limiting and the
Retry-After header, and exposes both plain and synchronized lyric text. The previous
request did not identify Surco and discarded plain text when timestamps were absent.

## Verification scope

The catalog/provider reproduction above ran on Linux. The client uses the same
`surco_youtube::YouTube` reqwest client with `rustls-tls-webpki-roots` on iOS and
Linux; there is no platform-specific lyric URL or parser. Playback/catalog success
alone does not establish LRCLIB reachability from the user's iPhone.

Local HTTP regression tests cover the provider boundary using real reqwest sockets,
including a body that stalls after successful HTTP headers. State and UI tests cover
retry, unrelated-track results, plain lyrics without invented timing, and successful
empty results separately from failures. The manual Rust probe reports counts and
error categories without printing lyric text.

## Implemented recovery and checks

LRCLIB requests identify Surco and have a 12-second total deadline, including
response body reads. A secondary video-lyrics request has an 8-second deadline. HTTP errors, connection failures, timeouts and invalid responses become
visible retryable errors, while a successful empty response stays a catalog miss.
A shared client cooldown honors Retry-After even when the current track changes;
repaint ticks and repeated taps do not create duplicate requests.

Valid synchronized records are preferred by duration, skipping invalid LRC records
rather than stopping at the nearest one. Available plain text is shown explicitly
without synchronization. A late error/miss/plain response for A after A→B→A cannot
erase already-loaded synchronized words.

The exact Rust provider probe returned `synced lines=19 first_at_ms=25550` for
Sunsetz. A subsequent run reported the live HTTP 429 as a retryable error with a
one-second cooldown. Both outcomes are valid provider evidence; the success is not
proof that adding User-Agent alone removed rate limiting.

Final local validation for this change: 233 workspace tests passed, with the same
3 existing external-service tests ignored; workspace/all-targets Clippy passed with
warnings denied. New tests were exercised RED then GREEN for identification,
HTTP 429, malformed JSON, a stalled body, explicit retry, plain text, invalid LRC
selection, cross-track cooldown and late-result degradation. No new tests were
ignored and no lyric text or credentials are committed as evidence.

## Exact-video fallback

The installed ytmapi-rs 0.3.3 supports anonymous `get_lyrics_id(VideoID)` followed
by `get_lyrics(LyricsID)`, including the supplied source attribution. Its protocol
was checked against the real Sunsetz video without an account: one description
shelf, 28 plain lines, 706 characters, source present. No lyric text or anonymous
visitor identifier was retained.

Surco uses that exact video's tab only when LRCLIB fails or has no usable words.
Already-available LRCLIB lyrics never trigger a second provider. Non-video keys,
Spotify IDs, URLs and local fixture IDs are rejected before anonymous client
initialization: a YouTube video ID must be 11 ASCII letters/digits, hyphens or
underscores. No title search guesses a replacement recording.

The fallback retains the provider attribution and shows plain text explicitly;
it does not manufacture synchronized timing. Successful results for up to 32
complete track records are kept in memory across A→B→A. Failures are not cached as
successful misses. If both providers fail, the primary error and its cooldown
remain available for retry.

The exact Rust fallback probe command
`cargo run -p surco --example lyrics_probe -- --youtube eVRbaYq5Aw4` passed with
`youtube plain lines=28 chars=706 source=Source: LyricFind`. This exercises the
actual anonymous reqwest/ytmapi method and parser, skips LRCLIB, and prints no
lyric text. The fallback/cache regressions ran RED (9 passed, 2 failed), then GREEN;
the video-ID validator also ran RED before its 11-byte restriction was applied.

The final 233-test workspace and all-targets Clippy runs used sibling limn commit
`7907efe7b9d01a3b4293b9967cb6e89c57a523bc` without overrides. The three ignored
tests predate this change. iPhone-network behavior still requires the installed
app; Linux and simulator checks are not physical-device evidence.
