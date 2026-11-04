//! The TV's settings, as BitChord's lays them out after tvOS: the app on the left with what
//! the focused row is for, the rows in their groups on the right. Values are written out and
//! change in place on select.

use std::borrow::Cow;

use super::*;
use crate::state::{Quality, CROSSFADE_MAX};

/// A row: what select does, its name, its value written out, and what the left says of it.
struct Row {
    id: &'static str,
    title: Cow<'static, str>,
    value: Cow<'static, str>,
    about: Cow<'static, str>,
}

fn on_off(on: bool) -> Cow<'static, str> {
    if on { t!("common-on") } else { t!("common-off") }
}

fn groups(state: &State) -> Vec<(Cow<'static, str>, Vec<Row>)> {
    let settings = &state.settings;
    let row = |id, title, value, about| Row { id, title, value, about };
    let crossfade = if settings.crossfade == 0.0 { t!("common-off") } else { format!("{:.0} s", settings.crossfade).into() };
    let quality = Quality::ALL.iter().find(|(quality, ..)| *quality == settings.quality).map_or("", |(.., label)| label.text());
    vec![
        (t!("common-settings-playback"), vec![
            row("setting:autoplay", t!("mobile-autoplay"), on_off(settings.autoplay), t!("desktop-autoplay-detail")),
            row(CROSSFADE, t!("desktop-crossfade"), crossfade, t!("desktop-crossfade-detail")),
            row("setting:skip-silence", t!("common-skip-silence"), on_off(settings.skip_silence), t!("common-skip-silence-detail")),
        ]),
        (t!("common-settings-sound"), vec![
            row(QUALITY, t!("mobile-quality"), quality.into(), t!("mobile-quality-note")),
            row("setting:normalize", t!("desktop-normalize"), on_off(settings.normalize), t!("desktop-normalize-detail")),
            row("setting:mono", t!("desktop-mono"), on_off(settings.mono), t!("desktop-mono-detail")),
        ]),
        (t!("mobile-display"), vec![
            row("setting:motion", t!("mobile-reduce-motion"), on_off(settings.reduce_motion), t!("desktop-reduce-motion-detail")),
            row(LANGUAGE, t!("common-language"), crate::i18n::current().name().into(), t!("common-language-detail")),
        ]),
        (t!("mobile-about"), vec![
            row("tv-version", t!("mobile-version"), env!("CARGO_PKG_VERSION").into(), t!("common-tv-settings-about")),
        ]),
    ]
}

/// The rows that step their value along on select.
pub const CROSSFADE: &str = "tv-setting:crossfade";
pub const QUALITY: &str = "tv-setting:quality";
pub const LANGUAGE: &str = "tv-setting:language";
/// How far each select moves the crossfade, in seconds.
pub const CROSSFADE_STEP: f32 = CROSSFADE_MAX / 4.0;

pub(super) fn page(state: &State, window: Size, palette: ComponentPalette) -> Element {
    let groups = groups(state);
    let about = groups.iter().flat_map(|(_, rows)| rows).find(|row| lit(state, row.id)).map_or_else(|| t!("common-tv-settings-about"), |row| row.about.clone());
    let left = Element::column(vec![
        Element::spacer().flex(1.0),
        app_mark().map_or_else(Element::spacer, |mark| Element::raster_image(mark)).size(Some(120.0), Some(120.0)).radius(27.0).centered_x(),
        Element::spacer().size(None, Some(22.0)),
        Text::new(about, palette).font_size(13.0).line_height(18.0).render().foreground(SECONDARY).center_content().size(Some(272.0), None).centered_x(),
        Element::spacer().flex(1.0),
    ]).size(Some(window.width * 0.42), None);
    let mut rows = Vec::new();
    for (title, group) in groups {
        rows.push(Text::new(title, palette).font_size(12.0).render().foreground(SECONDARY).padding(4.0));
        rows.extend(group.iter().map(|row| list_row(state, row.id, &row.title, Some(&row.value), false, Color::rgba(255, 255, 255, 15), palette)));
    }
    rows.push(Element::spacer().size(None, Some(38.0)));
    let right = ScrollArea::new("tv-settings", horizontal_inset(Element::column(rows).gap(5.0).padding(5.0), 0.0, SIDE - 5.0))
        .visibility(UNSEEN_BAR).render().flex(1.0);
    Element::column(vec![Element::spacer().size(None, Some(BAR)), Element::row(vec![left, right]).flex(1.0)])
}

/// Surco's mark, as the window and the task bar have it.
fn app_mark() -> Option<limn::RasterImage> {
    use base64::Engine as _;
    use sha2::Digest as _;
    const BYTES: &[u8] = include_bytes!("../../../assets/icon-rounded.png");
    static MARK: std::sync::OnceLock<(String, std::sync::Arc<str>)> = std::sync::OnceLock::new();
    let (digest, encoded) = MARK.get_or_init(|| {
        let digest: String = sha2::Sha256::digest(BYTES).iter().map(|byte| format!("{byte:02x}")).collect();
        (digest, base64::engine::general_purpose::STANDARD.encode(BYTES).into())
    });
    limn::RasterImage::new(digest.clone(), limn::RasterImageFormat::Png, BYTES.len() as u32, encoded.clone(), "Surco")
}

#[cfg(test)]
mod tests {
    use super::*;
    use limn::{UiAction, UiTree};

    #[test]
    fn select_steps_a_value_along_and_the_left_says_what_the_focused_row_is_for() {
        let mut state = State::default();
        state.tv = true;
        state.update(&UiAction::Clicked { id: "settings".into() });
        let steps: Vec<f32> = (0..5).map(|_| { state.update(&UiAction::Clicked { id: CROSSFADE.into() }); state.settings.crossfade }).collect();
        assert_eq!(steps, [3.0, 6.0, 9.0, 12.0, 0.0], "round to off again");
        let quality = state.settings.quality;
        state.update(&UiAction::Clicked { id: QUALITY.into() });
        assert_ne!(state.settings.quality, quality);
        state.tv_focused = Some("setting:mono".into());
        let size = Size::new(960.0, 540.0);
        let frame = UiTree::new(crate::view::view(&state, size)).layout(size);
        assert!(frame.access.iter().any(|node| node.id == "setting:mono"));
        let painted: Vec<&str> = frame.commands.iter().filter_map(|command| match command {
            limn::DrawCommand::Text { text, .. } => Some(text.as_str()),
            _ => None,
        }).collect();
        let detail = t!("desktop-mono-detail");
        let first = detail.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
        assert!(painted.iter().any(|text| text.starts_with(&first)), "the left says what mono is for: {painted:?}");
    }
}
