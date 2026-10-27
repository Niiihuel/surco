//! Android's clipboard, share sheet, notifications and photo picker, through the Kotlin shell.

use base64::Engine as _;
use serde_json::Value;
#[cfg(target_os = "android")]
use serde_json::json;

/// What the photo picker has produced: still open, a picture, or closed without one.
#[derive(Debug, PartialEq)]
pub enum Picked {
    /// Only the Android build makes this, while the picker is open.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    Open,
    Photo(Vec<u8>),
    Left,
}

/// The picture in a `system.photo` event, or `Left` when the user cancelled or it could not be read.
fn photo(body: &str) -> Picked {
    serde_json::from_str::<Value>(body).ok()
        .and_then(|value| value.get("bytes")?.as_str().map(str::to_owned))
        .and_then(|bytes| base64::engine::general_purpose::STANDARD.decode(bytes).ok())
        .map_or(Picked::Left, Picked::Photo)
}

/// The text Kotlin answered for `system.paste`, or `None` when the clipboard had none.
fn pasted(answer: &str) -> Option<String> {
    serde_json::from_str::<Value>(answer).ok()?.get("text")?.as_str().map(str::to_owned)
}

/// Whether Kotlin wrote the clipboard: no answer at all counts as a failure.
fn copied(answer: Option<&str>) -> Result<(), String> {
    let Some(answer) = answer else {
        return Err("Android did not answer the clipboard write".into());
    };
    let value: Value = serde_json::from_str(answer).map_err(|error| format!("Invalid Android clipboard answer: {error}"))?;
    if value.get("ok").and_then(Value::as_bool) == Some(true) {
        Ok(())
    } else {
        Err(value.get("error").and_then(Value::as_str).unwrap_or("Android could not write text to the clipboard").into())
    }
}

#[cfg(target_os = "android")]
use std::sync::{Mutex, Once};

#[cfg(target_os = "android")]
static PICKED: Mutex<Picked> = Mutex::new(Picked::Open);
#[cfg(target_os = "android")]
static LISTENING: Once = Once::new();

/// Writes `text` to the system clipboard, waiting for Android to confirm it.
#[cfg(target_os = "android")]
pub fn copy_text(text: &str) -> Result<(), String> {
    copied(crate::android::call("system.copy", &json!({ "text": text }).to_string()).as_deref())
}

/// The text on the system clipboard, read once after a user paste gesture; `None` when it is empty or not text.
#[cfg(target_os = "android")]
pub fn paste_text() -> Option<String> {
    crate::android::call("system.paste", "{}").and_then(|answer| pasted(&answer))
}

/// Opens Android's share sheet with `text`.
#[cfg(target_os = "android")]
pub fn share(title: &str, text: &str) {
    crate::android::call("system.share", &json!({ "title": title, "text": text }).to_string());
}

/// Posts a notification to the app's update channel.
#[cfg(target_os = "android")]
pub fn notify(title: &str, body: &str) {
    crate::android::call("system.notify", &json!({ "title": title, "body": body }).to_string());
}

/// Sends the app to the background, as Back does when there is nowhere left to go inside it.
#[cfg(target_os = "android")]
pub fn leave() {
    crate::android::call("system.leave", "{}");
}

/// Opens the system photo picker for one image; the result waits for [`take_picked_photo`].
#[cfg(target_os = "android")]
pub fn pick_photo() {
    LISTENING.call_once(|| crate::android::listen("system.photo", |body| *PICKED.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = photo(body)));
    *PICKED.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Picked::Open;
    crate::android::call("system.pick_photo", "{}");
}

/// The picture picked, once, or whether the picker is still open.
#[cfg(target_os = "android")]
pub fn take_picked_photo() -> Picked {
    std::mem::replace(&mut *PICKED.lock().unwrap_or_else(|poisoned| poisoned.into_inner()), Picked::Open)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn photo_with_bytes_is_the_decoded_picture() {
        let body = r#"{"bytes": "AQID"}"#;
        assert_eq!(photo(body), Picked::Photo(vec![1, 2, 3]));
    }

    #[test]
    fn photo_with_null_bytes_is_left() {
        assert_eq!(photo(r#"{"bytes": null}"#), Picked::Left);
    }

    #[test]
    fn photo_with_garbage_is_left() {
        assert_eq!(photo("not json"), Picked::Left);
        assert_eq!(photo(r#"{"bytes": "!!not base64!!"}"#), Picked::Left);
    }

    #[test]
    fn pasted_with_text_is_the_text() {
        assert_eq!(pasted(r#"{"text": "hello"}"#), Some("hello".to_string()));
    }

    #[test]
    fn pasted_with_null_is_none() {
        assert_eq!(pasted(r#"{"text": null}"#), None);
    }

    #[test]
    fn pasted_with_garbage_is_none() {
        assert_eq!(pasted("not json"), None);
    }

    #[test]
    fn copied_with_ok_is_ok() {
        assert_eq!(copied(Some(r#"{"ok": true}"#)), Ok(()));
    }

    #[test]
    fn copied_with_false_carries_the_error() {
        assert_eq!(copied(Some(r#"{"ok": false, "error": "no clipboard"}"#)), Err("no clipboard".to_string()));
    }

    #[test]
    fn copied_with_no_answer_is_an_error() {
        assert!(copied(None).is_err());
    }
}
