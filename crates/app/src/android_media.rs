//! Android's media session: what plays, shown on the lock screen and in the notification, and its buttons.

use crate::ios_media::MetadataDiff;
use crate::media_keys::Key;
use crate::player::PlayerView;

/// The key a `media.key` event asks for, or None for one it does not know.
fn key(body: &str) -> Option<Key> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    match value.get("key")?.as_str()? {
        "toggle" => Some(Key::Toggle),
        "play" => Some(Key::Play),
        "pause" => Some(Key::Pause),
        "next" => Some(Key::Next),
        "previous" => Some(Key::Previous),
        "seek" => value.get("seconds")?.as_f64().filter(|seconds| seconds.is_finite()).map(Key::Seek),
        _ => None,
    }
}

/// The `media.publish` body for `view`, or None when the session already shows it.
fn publish_body(diff: &mut MetadataDiff, view: &PlayerView, now: f64) -> Option<String> {
    let snapshot = diff.changed(view, now)?;
    // A snapshot of plain numbers and strings always serializes.
    serde_json::to_string(&snapshot).ok()
}

#[cfg(target_os = "android")]
mod native {
    use super::*;
    use crate::android;
    use crate::media_keys::MediaEvent;
    use base64::Engine as _;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Mutex, Once};
    use std::time::Instant;

    type Handler = Box<dyn Fn(MediaEvent) + Send>;
    static HANDLER: Mutex<Option<Handler>> = Mutex::new(None);
    // The bridge keeps a listener for good, so it is registered once, whatever the Controls count.
    static LISTENING: Once = Once::new();
    // Which Controls is the newest: an old backend's, stopped after a new one started, must not
    // take the new one's handler and session with it.
    static NEWEST: AtomicU64 = AtomicU64::new(0);

    fn deliver(event: MediaEvent) {
        if let Some(handler) = &*HANDLER.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) {
            handler(event);
        }
    }

    pub struct Controls {
        generation: u64,
        metadata: MetadataDiff,
        artwork: Option<(String, String)>,
        started: Instant,
    }

    impl Controls {
        pub fn start(handler: impl Fn(MediaEvent) + Send + 'static) -> Self {
            // Under the handler's lock, so an older Controls dropping meanwhile sees it is not the newest.
            let generation = {
                let mut held = HANDLER.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                *held = Some(Box::new(handler));
                NEWEST.fetch_add(1, Ordering::SeqCst) + 1
            };
            LISTENING.call_once(|| {
                android::listen("media.key", |body: &str| {
                    if let Some(key) = key(body) {
                        deliver(MediaEvent::Key(key));
                    }
                });
            });
            Self { generation, metadata: MetadataDiff::default(), artwork: None, started: Instant::now() }
        }

        pub fn show(&mut self, view: &PlayerView) {
            let Some(json) = publish_body(&mut self.metadata, view, self.started.elapsed().as_secs_f64()) else {
                return;
            };
            android::call("media.publish", &json);
            let artwork = view
                .current
                .as_ref()
                .and_then(|track| track.artwork_url.clone().map(|url| (track.id.clone(), url)));
            if artwork != self.artwork {
                self.artwork = artwork.clone();
                if let Some((track_id, url)) = artwork {
                    deliver(MediaEvent::Artwork { track_id, url });
                }
            }
        }
    }

    impl Drop for Controls {
        fn drop(&mut self) {
            let mut handler = HANDLER.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            if NEWEST.load(Ordering::SeqCst) != self.generation { return; }
            *handler = None;
            drop(handler);
            android::call("media.stop", "{}");
        }
    }

    /// Hands the cover fetched for `track` to the session, which shows it with the track.
    pub fn artwork(track: &str, url: &str, bytes: &[u8]) {
        let message = serde_json::json!({
            "track_id": track,
            "url": url,
            "bytes": base64::engine::general_purpose::STANDARD.encode(bytes),
        });
        android::call("media.artwork", &message.to_string());
    }
}

#[cfg(target_os = "android")]
pub use native::{artwork, Controls};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_key_is_read_and_unknown_ones_are_not() {
        assert_eq!(key(r#"{"key":"toggle"}"#), Some(Key::Toggle));
        assert_eq!(key(r#"{"key":"play"}"#), Some(Key::Play));
        assert_eq!(key(r#"{"key":"pause"}"#), Some(Key::Pause));
        assert_eq!(key(r#"{"key":"next"}"#), Some(Key::Next));
        assert_eq!(key(r#"{"key":"previous"}"#), Some(Key::Previous));
        assert_eq!(key(r#"{"key":"stop"}"#), None);
    }

    #[test]
    fn seek_needs_a_finite_number_of_seconds() {
        assert_eq!(key(r#"{"key":"seek","seconds":12.5}"#), Some(Key::Seek(12.5)));
        assert_eq!(key(r#"{"key":"seek"}"#), None);
        assert_eq!(key(r#"{"key":"seek","seconds":null}"#), None);
        assert_eq!(key(r#"{"key":"seek","seconds":"12"}"#), None);
    }

    #[test]
    fn garbage_is_no_key() {
        assert_eq!(key(""), None);
        assert_eq!(key("not json"), None);
        assert_eq!(key("[]"), None);
        assert_eq!(key(r#"{"seconds":3}"#), None);
    }

    #[test]
    fn the_publish_body_is_the_serialized_snapshot_and_repeats_are_skipped() {
        let view = PlayerView {
            current: Some(surco_core::Track {
                id: "a".into(),
                title: "Song".into(),
                artist: "Artist".into(),
                artist_id: None,
                album: Some("Album".into()),
                album_id: None,
                artwork_url: Some("https://cover/a".into()),
                duration_ms: 200_000,
            }),
            playing: true,
            position: 30.0,
            duration: 200.0,
            ..Default::default()
        };
        let mut diff = MetadataDiff::default();
        let body = publish_body(&mut diff, &view, 0.0).expect("the first snapshot is published");
        let expected = serde_json::to_string(&MetadataDiff::default().changed(&view, 0.0).unwrap()).unwrap();
        assert_eq!(body, expected);
        assert!(body.contains(r#""rate":1.0"#), "a playing track says so: {body}");
        assert_eq!(publish_body(&mut diff, &view, 0.1), None, "an unchanged view is not published again");
    }
}
