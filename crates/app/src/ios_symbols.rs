//! UIKit supplies Apple glyphs while limn keeps layout, tint, input and accessibility.
//! App/third-party brands and unknown custom artwork retain their original SVG.

use eframe::egui::ColorImage;
use limn::Icon;

fn symbol_name(icon: Icon) -> Option<&'static str> {
    match icon {
        Icon::PanelLeft | Icon::PanelLeftOpen | Icon::PanelLeftClose => Some("sidebar.left"),
        Icon::PanelBottom => Some("rectangle.bottomthird.inset.filled"),
        Icon::PanelRight | Icon::PanelRightOpen | Icon::PanelRightClose => Some("sidebar.right"),
        Icon::Add => Some("plus"),
        Icon::Minus => Some("minus"),
        Icon::Microphone => Some("mic"),
        Icon::Text => Some("text.alignleft"),
        Icon::Image => Some("photo"),
        Icon::Audio => Some("waveform"),
        Icon::Video => Some("video"),
        Icon::File => Some("doc"),
        Icon::Infinity => Some("infinity"),
        Icon::Paperclip => Some("paperclip"),
        Icon::Target => Some("scope"),
        Icon::Plan => Some("checklist"),
        Icon::Brain => Some("lightbulb"),
        Icon::Check => Some("checkmark"),
        Icon::InfoCircle => Some("info.circle"),
        Icon::CheckCircle => Some("checkmark.circle"),
        Icon::WarningTriangle => Some("exclamationmark.triangle"),
        Icon::ErrorCircle | Icon::CloseCircle => Some("xmark.circle"),
        Icon::Settings => Some("gearshape"),
        Icon::Chat => Some("bubble.left"),
        Icon::Folder => Some("folder"),
        Icon::Laptop | Icon::LaptopFill => Some("laptopcomputer"),
        Icon::Globe => Some("globe"),
        Icon::GitBranch => Some("arrow.triangle.branch"),
        Icon::Grid => Some("square.grid.2x2"),
        Icon::Palette => Some("paintpalette"),
        Icon::Users => Some("person.2"),
        Icon::Puzzle => Some("puzzlepiece"),
        Icon::Book => Some("book"),
        Icon::Server | Icon::ServerFill => Some("server.rack"),
        Icon::Bolt => Some("bolt"),
        Icon::ChartBar => Some("chart.bar"),
        Icon::Keyboard => Some("keyboard"),
        Icon::Hash => Some("number"),
        Icon::Code => Some("chevron.left.forwardslash.chevron.right"),
        Icon::Terminal => Some("terminal"),
        Icon::ArrowUp | Icon::Send => Some("arrow.up"),
        Icon::ArrowDown => Some("arrow.down"),
        Icon::ArrowLeft => Some("chevron.left"),
        Icon::ArrowRight | Icon::ChevronRight => Some("chevron.right"),
        Icon::ChevronDown => Some("chevron.down"),
        Icon::ChevronUp => Some("chevron.up"),
        Icon::Search => Some("magnifyingglass"),
        Icon::Stop | Icon::StopFill => Some("stop.fill"),
        Icon::Play | Icon::PlayFill => Some("play.fill"),
        Icon::Pause | Icon::PauseFill => Some("pause.fill"),
        Icon::History => Some("clock.arrow.circlepath"),
        Icon::Close | Icon::CloseBold => Some("xmark"),
        Icon::Refresh => Some("arrow.clockwise"),
        Icon::Menu | Icon::DragHandle => Some("line.3.horizontal"),
        Icon::More => Some("ellipsis"),
        Icon::Pin => Some("pin"),
        Icon::Archive => Some("archivebox"),
        Icon::ArchiveUp => Some("tray.and.arrow.up"),
        Icon::Trash => Some("trash"),
        Icon::Edit => Some("pencil"),
        Icon::Copy => Some("doc.on.doc"),
        Icon::Unread => Some("text.bubble"),
        Icon::Filter => Some("line.3.horizontal.decrease.circle"),
        Icon::Sort => Some("arrow.up.arrow.down"),
        Icon::Collapse | Icon::Compact => Some("arrow.down.right.and.arrow.up.left"),
        Icon::Expand | Icon::Fullscreen => Some("arrow.up.left.and.arrow.down.right"),
        Icon::Clock => Some("clock"),
        Icon::FolderFill => Some("folder.fill"),
        Icon::GridFill => Some("square.grid.2x2.fill"),
        Icon::SettingsFill => Some("gearshape.fill"),
        Icon::PaletteFill => Some("paintpalette.fill"),
        Icon::TerminalFill => Some("terminal.fill"),
        Icon::KeyboardFill => Some("keyboard.fill"),
        Icon::ChatFill => Some("bubble.left.fill"),
        Icon::UsersFill => Some("person.2.fill"),
        Icon::PuzzleFill => Some("puzzlepiece.fill"),
        Icon::BookFill => Some("book.fill"),
        Icon::ClockFill => Some("clock.fill"),
        Icon::BoltFill => Some("bolt.fill"),
        Icon::ArchiveFill => Some("archivebox.fill"),
        Icon::ChartBarFill => Some("chart.bar.fill"),
        Icon::Bell => Some("bell"),
        Icon::BellFill => Some("bell.fill"),
        Icon::BellSlash => Some("bell.slash"),
        Icon::SkipBack => Some("backward.end.fill"),
        Icon::SkipForward => Some("forward.end.fill"),
        Icon::Shuffle => Some("shuffle"),
        Icon::Repeat => Some("repeat"),
        Icon::RepeatOne => Some("repeat.1"),
        Icon::Volume => Some("speaker.wave.2"),
        Icon::VolumeMuted => Some("speaker.slash"),
        Icon::Heart => Some("heart"),
        Icon::HeartFill => Some("heart.fill"),
        Icon::MusicNote => Some("music.note"),
        Icon::Playlist => Some("music.note.list"),
        Icon::Home => Some("house.fill"),
        Icon::Library => Some("books.vertical.fill"),
        Icon::Sliders => Some("slider.horizontal.3"),
        Icon::MicrophoneMuted => Some("mic.slash"),
        Icon::Headphones => Some("headphones"),
        Icon::Emoji => Some("face.smiling"),
        Icon::Reply => Some("arrowshape.turn.up.left"),
        Icon::At => Some("at"),
        Icon::Phone => Some("phone"),
        Icon::HangUp => Some("phone.down"),
        Icon::Screen => Some("desktopcomputer"),
        Icon::VideoOff => Some("video.slash"),
        Icon::Lock => Some("lock"),
        Icon::Neutral => Some("line.diagonal"),
        Icon::Invite => Some("person.badge.plus"),
        Icon::Leave => Some("rectangle.portrait.and.arrow.right"),
        Icon::Custom(svg) => CUSTOM_SYMBOLS.iter().find_map(|(source, symbol)| (*source == svg).then_some(*symbol)),
        Icon::Github | Icon::FileType(_) => None,
    }
}

const CUSTOM_SYMBOLS: &[(&str, &str)] = &[
    (include_str!("../assets/icons/airplay-audio.svg"), "airplay.audio"),
    (include_str!("../assets/icons/circle-minus.svg"), "minus.circle"),
    (include_str!("../assets/icons/disc-3.svg"), "opticaldisc"),
    (include_str!("../assets/icons/download.svg"), "arrow.down.to.line"),
    (include_str!("../assets/icons/list-plus.svg"), "text.badge.plus"),
    (include_str!("../assets/icons/list-start.svg"), "text.insert"),
    (include_str!("../assets/icons/message-square-quote.svg"), "quote.bubble"),
    (include_str!("../assets/icons/share-2.svg"), "square.and.arrow.up"),
    (include_str!("../assets/icons/shuffle-spark.svg"), "shuffle.circle"),
    (include_str!("../assets/icons/trash-2.svg"), "trash"),
    (include_str!("../assets/icons/upload.svg"), "arrow.up.to.line"),
    (include_str!("../assets/icons/user-round.svg"), "person.crop.circle"),
];

fn rgba_len(pixels: u16) -> Option<usize> {
    if !(1..=512).contains(&pixels) { return None; }
    Some(usize::from(pixels).pow(2) * 4)
}

fn decode(pixels: u16, bytes: &[u8]) -> Option<ColorImage> {
    (bytes.len() == rgba_len(pixels)?).then(|| ColorImage::from_rgba_premultiplied([usize::from(pixels); 2], bytes))
}

#[cfg(target_os = "ios")]
unsafe extern "C" {
    fn surco_ui_symbol(name: *const u8, length: usize, pixels: u16) -> *mut u8;
    fn surco_ui_free_symbol(bytes: *mut u8);
}

#[cfg(target_os = "ios")]
struct NativeRgba(std::ptr::NonNull<u8>);

#[cfg(target_os = "ios")]
impl Drop for NativeRgba {
    fn drop(&mut self) { unsafe { surco_ui_free_symbol(self.0.as_ptr()); } }
}

#[cfg(target_os = "ios")]
fn rasterize(icon: Icon, pixels: u16) -> Option<ColorImage> {
    let name = symbol_name(icon)?;
    let length = rgba_len(pixels)?;
    // The public UIKit bridge asserts main-thread access and returns an owned,
    // square RGBA buffer of exactly pixels²×4 bytes, or null for missing artwork.
    let owned = NativeRgba(std::ptr::NonNull::new(unsafe {
        surco_ui_symbol(name.as_ptr(), name.len(), pixels)
    })?);
    let bytes = unsafe { std::slice::from_raw_parts(owned.0.as_ptr(), length) };
    decode(pixels, bytes)
}

#[cfg(target_os = "ios")]
pub fn install() {
    limn_egui::set_icon_rasterizer(Some(rasterize));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phone_navigation_and_transport_use_native_filled_shapes() {
        for (icon, name) in [
            (Icon::Home, "house.fill"), (Icon::Library, "books.vertical.fill"),
            (Icon::Search, "magnifyingglass"), (Icon::Play, "play.fill"),
            (Icon::PlayFill, "play.fill"), (Icon::Pause, "pause.fill"),
            (Icon::PauseFill, "pause.fill"), (Icon::SkipBack, "backward.end.fill"),
            (Icon::SkipForward, "forward.end.fill"),
        ] { assert_eq!(symbol_name(icon), Some(name)); }
    }

    #[test]
    fn actions_keep_outline_semantics_and_selected_variants_remain_distinct() {
        for (icon, name) in [
            (Icon::Heart, "heart"), (Icon::HeartFill, "heart.fill"),
            (Icon::Settings, "gearshape"), (Icon::SettingsFill, "gearshape.fill"),
            (Icon::Trash, "trash"), (Icon::Sliders, "slider.horizontal.3"),
            (Icon::More, "ellipsis"), (Icon::ChevronDown, "chevron.down"),
            (Icon::Headphones, "headphones"), (Icon::Playlist, "music.note.list"),
        ] { assert_eq!(symbol_name(icon), Some(name)); }
    }

    #[test]
    fn bundled_lucide_actions_are_native_including_lyrics_and_playlist_tools() {
        for (svg, name) in [
            (include_str!("../assets/icons/circle-minus.svg"), "minus.circle"),
            (include_str!("../assets/icons/disc-3.svg"), "opticaldisc"),
            (include_str!("../assets/icons/download.svg"), "arrow.down.to.line"),
            (include_str!("../assets/icons/list-plus.svg"), "text.badge.plus"),
            (include_str!("../assets/icons/list-start.svg"), "text.insert"),
            (include_str!("../assets/icons/message-square-quote.svg"), "quote.bubble"),
            (include_str!("../assets/icons/share-2.svg"), "square.and.arrow.up"),
            (include_str!("../assets/icons/shuffle-spark.svg"), "shuffle.circle"),
            (include_str!("../assets/icons/trash-2.svg"), "trash"),
            (include_str!("../assets/icons/upload.svg"), "arrow.up.to.line"),
            (include_str!("../assets/icons/user-round.svg"), "person.crop.circle"),
        ] { assert_eq!(symbol_name(Icon::Custom(svg)), Some(name)); }
        assert_ne!(symbol_name(Icon::Shuffle), symbol_name(Icon::Custom(include_str!("../assets/icons/shuffle-spark.svg"))),
            "smart shuffle must remain distinguishable from ordinary shuffle");
    }

    #[test]
    fn audio_route_actions_keep_the_waveform_reserved_for_media() {
        assert_eq!(symbol_name(Icon::Audio), Some("waveform"));
        assert_eq!(symbol_name(Icon::Custom(include_str!("../assets/icons/airplay-audio.svg"))), Some("airplay.audio"));
    }

    #[test]
    fn unknown_custom_marks_and_colored_file_artwork_use_the_original_renderer() {
        assert_eq!(symbol_name(Icon::Github), None);
        assert_eq!(symbol_name(limn::file_icon("main.rs")), None);
        assert_eq!(symbol_name(Icon::Custom("<svg><path id=\"surco-original-mark\"/></svg>")), None);
    }

    #[test]
    fn native_antialiasing_is_already_premultiplied_and_must_not_be_multiplied_again() {
        let rgba = [32, 64, 96, 128];
        let image = decode(1, &rgba).unwrap();
        assert_eq!(image.size, [1, 1]);
        assert_eq!(image.pixels[0].to_array(), rgba);
    }

    #[test]
    fn invalid_pixel_sizes_and_mismatched_native_buffers_are_rejected() {
        assert_eq!(rgba_len(0), None);
        assert_eq!(rgba_len(513), None);
        assert_eq!(rgba_len(u16::MAX), None);
        assert_eq!(rgba_len(512), Some(512 * 512 * 4));
        assert!(decode(1, &[255, 255, 255]).is_none());
        assert!(decode(1, &[0; 8]).is_none());
        assert!(decode(0, &[]).is_none());
    }
}
