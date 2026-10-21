//! SponsorBlock (sponsor.ajay.app): the parts of a YouTube video its viewers marked, most
//! usefully the talking, skits and intros of a music video that are not the song, as
//! Opentune skips them.
//!
//! A video is asked for by the first four characters of its id's SHA-256, so the server never
//! learns which one plays; the one that does is picked out of the answer here. The segments
//! are CC BY-NC-SA 4.0, from sponsor.ajay.app.

use serde_json::Value;
use sha2::{Digest, Sha256};

const API: &str = "https://sponsor.ajay.app/api/skipSegments";

/// Each category the service knows, by its name there and the key of its words here.
pub const CATEGORIES: [(&str, &str); 8] = [
    ("music_offtopic", "common-sponsorblock-music-offtopic"),
    ("sponsor", "common-sponsorblock-sponsor"),
    ("selfpromo", "common-sponsorblock-selfpromo"),
    ("interaction", "common-sponsorblock-interaction"),
    ("intro", "common-sponsorblock-intro"),
    ("outro", "common-sponsorblock-outro"),
    ("preview", "common-sponsorblock-preview"),
    ("filler", "common-sponsorblock-filler"),
];

/// What is skipped unless the user chose otherwise, as Opentune's: what is not music, sponsors,
/// self-promotion and the reminders to like and subscribe. A bit each, in `CATEGORIES` order.
pub const DEFAULT: u8 = 0b1111;

/// A stretch of a video to jump over.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub category: String,
    pub uuid: String,
    /// The length of the video it was marked on, 0 when not given.
    pub video_duration: f64,
}

impl Segment {
    /// Whether it was marked on the cut that plays, `duration` long: one of another length
    /// would jump over the wrong seconds.
    pub fn fits(&self, duration: f64) -> bool {
        self.video_duration <= 0.0 || duration <= 0.0 || (self.video_duration - duration).abs() <= 2.0
    }
}

/// The names of the categories `mask` holds.
fn names(mask: u8) -> Vec<&'static str> {
    CATEGORIES.iter().enumerate().filter(|(bit, _)| mask & (1 << bit) != 0).map(|(_, (name, _))| *name).collect()
}

/// The words for a category, for what was skipped.
pub fn key_of(category: &str) -> &'static str {
    CATEGORIES.iter().find(|(name, _)| *name == category).map_or("common-sponsorblock-segment", |(_, key)| *key)
}

fn hash_prefix(video_id: &str) -> String {
    Sha256::digest(video_id.as_bytes()).iter().take(2).map(|byte| format!("{byte:02x}")).collect()
}

/// The segments of `video_id` in an answer for its hash's prefix, in order.
fn parse(answer: &Value, video_id: &str) -> Vec<Segment> {
    let Some(video) = answer.as_array().and_then(|videos| videos.iter().find(|video| video["videoID"] == video_id)) else { return Vec::new() };
    let mut segments: Vec<Segment> = video["segments"].as_array().into_iter().flatten().filter_map(|segment| {
        if segment["actionType"].as_str().unwrap_or("skip") != "skip" { return None; }
        let (start, end) = (segment["segment"][0].as_f64()?, segment["segment"][1].as_f64()?);
        (end > start).then(|| Segment {
            start,
            end,
            category: segment["category"].as_str().unwrap_or_default().to_owned(),
            uuid: segment["UUID"].as_str().map_or_else(|| format!("{start}-{end}"), str::to_owned),
            video_duration: segment["videoDuration"].as_f64().unwrap_or(0.0),
        })
    }).collect();
    segments.sort_by(|a, b| a.start.total_cmp(&b.start));
    segments
}

/// The segments of `video_id` in the categories of `mask`: none where nothing is marked, and
/// an error where the service could not be asked, so that it is asked again next time.
pub async fn segments(youtube: &surco_youtube::YouTube, video_id: &str, mask: u8) -> Result<Vec<Segment>, String> {
    let names = names(mask);
    if names.is_empty() { return Ok(Vec::new()); }
    let categories = serde_json::to_string(&names).map_err(|error| error.to_string())?;
    let answer = youtube.http().get(format!("{API}/{}", hash_prefix(video_id)))
        .query(&[("categories", categories.as_str()), ("actionTypes", "[\"skip\"]")])
        .send().await.map_err(|error| error.to_string())?;
    // Nothing marked for any video under this prefix.
    if answer.status().as_u16() == 404 { return Ok(Vec::new()); }
    let answer: Value = answer.error_for_status().map_err(|error| error.to_string())?.json().await.map_err(|error| error.to_string())?;
    Ok(parse(&answer, video_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_video_is_asked_for_by_four_characters_of_its_hash() {
        // sha256("dQw4w9WgXcQ") begins 5f6b.
        assert_eq!(hash_prefix("dQw4w9WgXcQ"), "5f6b");
        assert_eq!(names(DEFAULT), ["music_offtopic", "sponsor", "selfpromo", "interaction"]);
        assert_eq!(names(0), Vec::<&str>::new());
    }

    #[test]
    fn the_segments_of_the_video_that_plays_are_picked_out_in_order() {
        let answer = json!([
            {"videoID": "other", "segments": [{"segment": [1.0, 2.0], "category": "sponsor", "UUID": "x"}]},
            {"videoID": "song", "segments": [
                {"segment": [180.5, 200.0], "category": "music_offtopic", "UUID": "b", "actionType": "skip", "videoDuration": 200.4},
                {"segment": [0.0, 12.0], "category": "music_offtopic", "UUID": "a", "actionType": "skip", "videoDuration": 200.4},
                {"segment": [50.0, 60.0], "category": "sponsor", "UUID": "m", "actionType": "mute"},
                {"segment": [70.0, 70.0], "category": "sponsor", "UUID": "z"}
            ]}
        ]);
        let segments = parse(&answer, "song");
        assert_eq!(segments.iter().map(|segment| segment.uuid.as_str()).collect::<Vec<_>>(), ["a", "b"], "in order, only what is skipped, none empty");
        assert!(segments[0].fits(201.0) && !segments[0].fits(230.0), "another cut of the video does not take them");
        assert!(parse(&answer, "missing").is_empty());
        assert_eq!(key_of("sponsor"), "common-sponsorblock-sponsor");
    }
}
