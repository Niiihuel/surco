//! Audio streams from an InnerTube `player` request made as the VISIONOS client.
//!
//! That client returns direct URLs: no signature, no `n` parameter, no PO token, so no
//! JavaScript has to run. The constants are the ones yt-dlp ships for `visionos` (its master
//! on 2026-10-12). YouTube closed the previous client of this kind (android_vr) in August
//! 2026; when this one goes the fallback is a PO token bound to the video id.

use serde_json::{json, Value};

use crate::Result;

const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15";
const CLIENT_VERSION: &str = "1.02";
/// A fresh session is sometimes restricted (see `restricted`); this many are tried.
const MAX_SESSIONS: u32 = 4;

#[derive(Clone, Debug, PartialEq)]
pub struct Audio {
    pub itag: u64,
    /// As YouTube writes it, e.g. `audio/webm; codecs="opus"`.
    pub mime: String,
    pub average_bitrate: u64,
    pub size: u64,
    pub url: String,
}

/// A picture-only rendition, cut in fragments: `init` and `index` are the byte ranges of
/// its header and of the table that says where each fragment is.
#[derive(Clone, Debug, PartialEq)]
pub struct Video {
    pub itag: u64,
    /// As YouTube writes it, e.g. `video/mp4; codecs="avc1.4d401e"`.
    pub mime: String,
    pub width: u64,
    pub height: u64,
    /// Pictures in a second; 0 when the format does not say.
    pub fps: u64,
    pub size: u64,
    pub init: (u64, u64),
    pub index: (u64, u64),
    pub url: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stream {
    pub duration_seconds: f64,
    /// Seconds until the URLs stop working.
    pub expires_in_seconds: u64,
    pub audio: Vec<Audio>,
    pub video: Vec<Video>,
}

impl Stream {
    /// The highest-bitrate audio. In practice Opus in WebM at 48 kHz (itag 251), with AAC in
    /// M4A (itag 140) a little below it.
    pub fn best(&self) -> Option<&Audio> {
        self.audio.iter().max_by_key(|audio| audio.average_bitrate)
    }

    /// The highest-bitrate audio at or under `bps`, or the lowest on offer when all are above.
    pub fn at_most(&self, bps: u64) -> Option<&Audio> {
        let under = self.audio.iter().filter(|audio| audio.average_bitrate <= bps).max_by_key(|audio| audio.average_bitrate);
        under.or_else(|| self.audio.iter().min_by_key(|audio| audio.average_bitrate))
    }
}

/// Without a visitor id the player answers LOGIN_REQUIRED ("confirm you're not a bot").
async fn visitor_data(http: &reqwest::Client) -> Result<String> {
    let text = http
        .get("https://www.youtube.com/sw.js_data")
        .header("User-Agent", USER_AGENT)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    // JSON behind the anti-XSSI prefix `)]}'`.
    let start = text.find('[').ok_or("sw.js_data: no JSON")?;
    let value: Value = serde_json::from_str(&text[start..])?;
    Ok(value[0][2][0][0][13].as_str().ok_or("sw.js_data: no visitor data")?.to_owned())
}

async fn player(http: &reqwest::Client, video_id: &str) -> Result<Stream> {
    let visitor = visitor_data(http).await?;
    let body = json!({
        "context": {"client": {
            "clientName": "VISIONOS", "clientVersion": CLIENT_VERSION,
            "deviceMake": "Apple", "deviceModel": "RealityDevice17,1",
            "osName": "visionOS", "osVersion": "26.5.23O471",
            "userAgent": USER_AGENT, "hl": "en", "gl": "US", "visitorData": visitor,
        }},
        "videoId": video_id, "contentCheckOk": true, "racyCheckOk": true,
    });
    let response: Value = http
        .post("https://www.youtube.com/youtubei/v1/player?prettyPrint=false")
        .header("User-Agent", USER_AGENT)
        .header("X-YouTube-Client-Name", "101")
        .header("X-YouTube-Client-Version", CLIENT_VERSION)
        .header("X-Goog-Visitor-Id", &visitor)
        .header("Origin", "https://www.youtube.com")
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    parse(&response)
}

fn parse(response: &Value) -> Result<Stream> {
    let status = response["playabilityStatus"]["status"].as_str().unwrap_or("?");
    if status != "OK" {
        let reason = response["playabilityStatus"]["reason"].as_str().unwrap_or("");
        return Err(format!("not playable: {status} {reason}").into());
    }
    // YouTube writes these numbers sometimes as numbers and sometimes as strings.
    let number = |value: &Value| value.as_u64().or_else(|| value.as_str()?.parse().ok()).unwrap_or(0);
    let audio = response["streamingData"]["adaptiveFormats"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|format| {
            let mime = format["mimeType"].as_str()?;
            Some(Audio {
                itag: format["itag"].as_u64()?,
                mime: mime.starts_with("audio/").then(|| mime.to_owned())?,
                average_bitrate: number(&format["averageBitrate"]),
                size: number(&format["contentLength"]),
                // A format without a URL is SABR-only or ciphered: not usable here.
                url: format["url"].as_str()?.to_owned(),
            })
        })
        .collect();
    let range = |range: &Value| Some((range["start"].as_str()?.parse().ok()?, range["end"].as_str()?.parse().ok()?));
    let video = response["streamingData"]["adaptiveFormats"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|format| {
            let mime = format["mimeType"].as_str()?;
            Some(Video {
                itag: format["itag"].as_u64()?,
                mime: mime.starts_with("video/").then(|| mime.to_owned())?,
                width: format["width"].as_u64()?,
                height: format["height"].as_u64()?,
                fps: format["fps"].as_u64().unwrap_or(0),
                size: number(&format["contentLength"]),
                init: range(&format["initRange"])?,
                index: range(&format["indexRange"])?,
                url: format["url"].as_str()?.to_owned(),
            })
        })
        .collect();
    Ok(Stream {
        duration_seconds: number(&response["videoDetails"]["lengthSeconds"]) as f64,
        expires_in_seconds: number(&response["streamingData"]["expiresInSeconds"]),
        audio,
        video,
    })
}

/// Some fresh sessions fall in an experiment that serves only about the first minute and
/// answers 403 after it (3 of 51 sessions when measured). The last two bytes are the cheapest
/// reliable probe; a new visitor id is the cure, a PO token was not.
async fn restricted(http: &reqwest::Client, audio: &Audio) -> Result<bool> {
    if audio.size < 2 {
        return Ok(false);
    }
    let status = http
        .get(&audio.url)
        .header("Range", format!("bytes={}-{}", audio.size - 2, audio.size - 1))
        .send()
        .await?
        .status();
    Ok(status == reqwest::StatusCode::FORBIDDEN)
}

pub(crate) async fn resolve(http: &reqwest::Client, video_id: &str) -> Result<Stream> {
    for _ in 0..MAX_SESSIONS {
        let stream = player(http, video_id).await?;
        let best = stream.best().ok_or("no audio format with a direct URL")?;
        if !restricted(http, best).await? {
            return Ok(stream);
        }
    }
    Err(format!("streams still restricted after {MAX_SESSIONS} sessions").into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_audio_formats_that_have_a_url_and_reads_numbers_in_either_form() {
        let response = json!({
            "playabilityStatus": {"status": "OK"},
            "videoDetails": {"lengthSeconds": "290"},
            "streamingData": {"expiresInSeconds": "21540", "adaptiveFormats": [
                {"itag": 251, "mimeType": "audio/webm; codecs=\"opus\"", "averageBitrate": 141572, "contentLength": "5134839", "url": "https://a/251"},
                {"itag": 140, "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "averageBitrate": "129524", "contentLength": "4697618", "url": "https://a/140"},
                {"itag": 137, "mimeType": "video/mp4; codecs=\"avc1\"", "averageBitrate": 4000000, "contentLength": "9", "url": "https://v/137"},
                {"itag": 250, "mimeType": "audio/webm; codecs=\"opus\"", "averageBitrate": 71994, "contentLength": "2611256"},
            ]},
        });
        let stream = parse(&response).unwrap();
        assert_eq!(stream.duration_seconds, 290.0);
        assert_eq!(stream.expires_in_seconds, 21540);
        assert_eq!(stream.audio.iter().map(|a| a.itag).collect::<Vec<_>>(), [251, 140]);
        assert_eq!(stream.at_most(135_000).map(|audio| audio.itag), Some(140));
        assert_eq!(stream.at_most(64_000).map(|audio| audio.itag), Some(140), "nothing that low: the lowest there is");
        let best = stream.best().unwrap();
        assert_eq!((best.itag, best.size), (251, 5_134_839));
    }

    #[test]
    fn an_unplayable_video_is_an_error_with_the_reason() {
        let response = json!({"playabilityStatus": {"status": "LOGIN_REQUIRED", "reason": "Sign in to confirm you're not a bot"}});
        let error = parse(&response).unwrap_err().to_string();
        assert!(error.contains("LOGIN_REQUIRED") && error.contains("not a bot"), "{error}");
    }
}
