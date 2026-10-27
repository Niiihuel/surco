//! Text typed through Android's input methods, kept in step with the field that has focus.

/// What the frame's focused field is: its id and its value as the app holds it.
pub struct Focused<'a> {
    pub id: &'a str,
    pub value: &'a str,
}

/// What the host must do this frame.
#[derive(Debug, Default, PartialEq)]
pub struct Step {
    /// Some(true): show the soft keyboard; Some(false): hide it; None: leave it.
    pub keyboard: Option<bool>,
    /// Text to put into the input method's state, the cursor at its end.
    pub set_text: Option<String>,
    /// A field's new value, typed through the input method, for the app to take in.
    pub changed: Option<(String, String)>,
}

/// Tracks the field being edited and the text the input method and the app last agreed on.
#[derive(Default)]
pub struct Sync {
    id: Option<String>,
    agreed: String,
    /// Text handed to the input method that it has not shown back yet: GameActivity passes it
    /// through Java's thread, so for a frame or more it still holds the old text.
    sent: Option<String>,
}

impl Sync {
    /// `focused`: the field with focus this frame, if any. `ime`: the input method's text now
    /// (pass "" when there is no state yet).
    pub fn step(&mut self, focused: Option<Focused<'_>>, ime: &str) -> Step {
        let Some(focused) = focused else {
            let was_focused = self.id.take().is_some();
            self.agreed.clear();
            self.sent = None;
            return Step { keyboard: was_focused.then_some(false), ..Step::default() };
        };

        if self.id.as_deref() != Some(focused.id) {
            // The IME's text is stale on the frame focus moves, so nothing is read from it here.
            let keyboard = if self.id.is_none() { Some(true) } else { None };
            self.id = Some(focused.id.to_owned());
            self.agreed = focused.value.to_owned();
            // Already holding it, the input method has nothing to show back.
            self.sent = (ime != focused.value).then(|| self.agreed.clone());
            return Step { keyboard, set_text: Some(self.agreed.clone()), changed: None };
        }

        if let Some(sent) = &self.sent {
            if ime != sent { return Step::default(); }
            self.sent = None;
        }

        if ime != self.agreed {
            // The user typed. When the app also changed the value, the typing wins.
            self.agreed = ime.to_owned();
            return Step {
                changed: Some((focused.id.to_owned(), ime.to_owned())),
                ..Step::default()
            };
        }

        if focused.value != self.agreed {
            self.agreed = focused.value.to_owned();
            self.sent = Some(self.agreed.clone());
            return Step { set_text: Some(self.agreed.clone()), ..Step::default() };
        }

        Step::default()
    }
}

/// Where a cursor at the end of `text` stands in the units GameActivity counts in.
// GameTextInput takes its text from Java Strings, so the cursor counts UTF-16 code units.
pub fn end_of(text: &str) -> usize {
    text.encode_utf16().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn focus<'a>(id: &'a str, value: &'a str) -> Option<Focused<'a>> {
        Some(Focused { id, value })
    }

    #[test]
    fn focusing_a_field_shows_the_keyboard_and_loads_its_value() {
        let mut sync = Sync::default();
        assert_eq!(
            sync.step(focus("search", "jazz"), ""),
            Step { keyboard: Some(true), set_text: Some("jazz".into()), changed: None }
        );
    }

    #[test]
    fn the_stale_ime_text_on_the_focus_frame_is_not_taken_as_typing() {
        let mut sync = Sync::default();
        sync.step(focus("search", "jazz"), "");
        assert_eq!(sync.step(focus("search", "jazz"), "jazz"), Step::default());
    }

    #[test]
    fn blurring_hides_the_keyboard_and_forgets_the_field() {
        let mut sync = Sync::default();
        sync.step(focus("search", "jazz"), "jazz");
        assert_eq!(sync.step(None, "jazz"), Step { keyboard: Some(false), ..Step::default() });
        assert_eq!(sync.step(None, ""), Step::default());
        assert_eq!(
            sync.step(focus("search", "jazz"), ""),
            Step { keyboard: Some(true), set_text: Some("jazz".into()), changed: None }
        );
    }

    #[test]
    fn moving_focus_loads_the_new_field_without_touching_the_keyboard() {
        let mut sync = Sync::default();
        sync.step(focus("search", "jazz"), "jazz");
        assert_eq!(
            sync.step(focus("rename", "Blue"), "jazz"),
            Step { keyboard: None, set_text: Some("Blue".into()), changed: None }
        );
        assert_eq!(sync.step(focus("rename", "Blue"), "Blue"), Step::default());
    }

    #[test]
    fn typing_through_the_input_method_reaches_the_field() {
        let mut sync = Sync::default();
        sync.step(focus("search", ""), "");
        assert_eq!(
            sync.step(focus("search", ""), "a"),
            Step { changed: Some(("search".into(), "a".into())), ..Step::default() }
        );
        assert_eq!(sync.step(focus("search", "a"), "a"), Step::default());
    }

    #[test]
    fn an_app_change_reaches_the_input_method() {
        let mut sync = Sync::default();
        sync.step(focus("search", "jazz"), "jazz");
        assert_eq!(
            sync.step(focus("search", ""), "jazz"),
            Step { set_text: Some("".into()), ..Step::default() }
        );
    }

    #[test]
    fn when_the_user_and_the_app_both_change_the_user_wins() {
        let mut sync = Sync::default();
        sync.step(focus("search", "jazz"), "jazz");
        assert_eq!(
            sync.step(focus("search", "soul"), "jazzy"),
            Step { changed: Some(("search".into(), "jazzy".into())), ..Step::default() }
        );
    }

    #[test]
    fn the_old_text_the_input_method_still_holds_after_a_set_is_not_typing() {
        let mut sync = Sync::default();
        sync.step(focus("search", "jazz"), "");
        // GameActivity has not shown "jazz" back yet: its "" must not clear the field.
        assert_eq!(sync.step(focus("search", "jazz"), ""), Step::default());
        assert_eq!(sync.step(focus("search", "jazz"), ""), Step::default());
        assert_eq!(sync.step(focus("search", "jazz"), "jazz"), Step::default());
        assert_eq!(
            sync.step(focus("search", "jazz"), "jazzy"),
            Step { changed: Some(("search".into(), "jazzy".into())), ..Step::default() }
        );
    }

    #[test]
    fn nothing_focused_before_or_now_does_nothing() {
        let mut sync = Sync::default();
        assert_eq!(sync.step(None, "stray"), Step::default());
    }

    #[test]
    fn a_typed_sequence_with_an_app_clear_stays_in_step() {
        let mut sync = Sync::default();
        assert_eq!(
            sync.step(focus("search", ""), ""),
            Step { keyboard: Some(true), set_text: Some("".into()), changed: None }
        );
        assert_eq!(
            sync.step(focus("search", ""), "a"),
            Step { changed: Some(("search".into(), "a".into())), ..Step::default() }
        );
        assert_eq!(
            sync.step(focus("search", "a"), "ab"),
            Step { changed: Some(("search".into(), "ab".into())), ..Step::default() }
        );
        assert_eq!(
            sync.step(focus("search", ""), "ab"),
            Step { set_text: Some("".into()), ..Step::default() }
        );
        assert_eq!(sync.step(focus("search", ""), ""), Step::default());
        assert_eq!(
            sync.step(focus("search", ""), "c"),
            Step { changed: Some(("search".into(), "c".into())), ..Step::default() }
        );
        assert_eq!(sync.step(None, "c"), Step { keyboard: Some(false), ..Step::default() });
    }

    #[test]
    fn the_cursor_at_the_end_counts_utf16_units() {
        assert_eq!(end_of("añ😀"), 4);
        assert_eq!(end_of(""), 0);
    }
}
