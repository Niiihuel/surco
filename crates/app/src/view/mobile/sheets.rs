//! Controlled touch surfaces over the same track commands used by desktop.

use super::*;
use limn::{BottomSheet, Separator};

#[cfg(test)]
#[path = "library_tests.rs"]
mod library_tests;

pub(super) fn render(state: &State, base: Element, window: Size, palette: ComponentPalette) -> Element {
    let width = window.width;
    let context = state.mobile_ui.context.as_ref();
    let menu = context.map(|context| song_menu(state, &context.row, &context.track, palette));
    let base = match state.mobile_ui.menu_origin {
        Some(origin) => bubble(base, menu, origin, (state.mobile_ui.content_insets.top, window.height - state.mobile_ui.content_insets.bottom)),
        None => BottomSheet::new(base, context.is_some(), t!("common-song-options"), "", "mobile-context-close", SCHEME.menu())
            .max_height(680.0).body(menu.unwrap_or_else(Element::spacer)).glass(super::SHEET_GLASS).ios(super::ios_sheet(state)).render(),
    };
    let base = offset_sheet(state, info_sheet(state, picker(state, base, palette), palette), palette);
    pipeline(state, sleep_sheet(state, connections(state, library_sheet(state, base, palette), palette), palette), width, palette)
}

pub(super) const SHARE: &str = include_str!("../../../assets/icons/share-2.svg");
const HEART_CRACK: &str = include_str!("../../../assets/icons/heart-crack.svg");
const LIST_PLUS: &str = include_str!("../../../assets/icons/list-plus.svg");
const LIST_START: &str = include_str!("../../../assets/icons/list-start.svg");
const RADIO: &str = include_str!("../../../assets/icons/radio.svg");
const DISC: &str = include_str!("../../../assets/icons/disc-3.svg");
const ARTIST: &str = include_str!("../../../assets/icons/user-round.svg");
const TRASH: &str = include_str!("../../../assets/icons/trash-2.svg");
const CIRCLE_MINUS: &str = include_str!("../../../assets/icons/circle-minus.svg");
const SLIDERS: &str = include_str!("../../../assets/icons/sliders.svg");

/// BitChord's menu, in aurora's glass: a tall pane, as wide as its rows want, that pops out of
/// the button or row that opened it as aurora's menus do and melts back into it.
const BUBBLE_WIDTH: f32 = 262.0;
const BUBBLE_RADIUS: f32 = 32.0;
/// How far it keeps from the screen's safe edges.
const BUBBLE_MARGIN: f32 = 8.0;

/// `safe` is the screen's top and foot clear of the system's bars.
fn bubble(base: Element, menu: Option<Element>, origin: limn::Rect, safe: (f32, f32)) -> Element {
    let Some(menu) = menu else { return base };
    let (top, foot) = (safe.0 + BUBBLE_MARGIN, safe.1 - BUBBLE_MARGIN);
    // A long menu scrolls within the screen's safe edges rather than running off it.
    let surface = super::pane_of_glass(Element::panel(Element::scroll("mobile-context-menu", menu.padding(10.0))), false)
        .radius(BUBBLE_RADIUS).clip_children()
        .size(Some(BUBBLE_WIDTH), None).max_height((foot - top).max(TAP));
    // Its right edge is the trigger's: it runs down from the trigger's top, or, where that
    // leaves too little room, ends at the foot and covers the trigger, as BitChord's does.
    let from = origin.y.clamp(top, foot);
    let edge = Element::spacer().size(Some(0.0), Some(foot - from));
    let trigger = Element::popover(edge, surface, "mobile-context-close", limn::PopoverPlacement::LeftStart)
        .popover_gap(0.0)
        // aurora's pop: a drop leaves the button and swells into the menu; closing, back into it.
        .popover_morph(origin, origin.width.min(origin.height) / 2.0)
        .popover_animation_ms(limn::GLASS_POP_OPEN_MS).popover_close_animation_ms(limn::GLASS_POP_CLOSE_MS);
    let placed = Element::column(vec![Element::spacer().size(None, Some(from)),
        Element::row(vec![Element::spacer().size(Some(origin.x + origin.width), None), trigger])]);
    Element::stack(vec![base, placed])
}

/// What can be done with a song, grouped as BitChord's menu: liking it, not wanting it and
/// sharing it across the top; then putting it in a list; playing it; going to where it comes
/// from; and, for what plays, the timer and the lyrics' step.
fn song_menu(state: &State, row: &str, track: &Track, palette: ComponentPalette) -> Element {
    let liked = state.is_liked(&track.id);
    let disliked = state.collection.is_disliked(&track.id);
    let top = |action: &str, glyph: Icon, label: &str| {
        let face = Element::column(vec![
            Element::icon(glyph).large_icon(24.0).foreground(palette.text).centered_x(),
            Text::new(label, palette).font_size(12.0).truncate().render().centered_x(),
        ]).gap(6.0).centered();
        Element::clickable(format!("do:{action}:{row}"), face).accessible(limn::Access::new(limn::Role::Button).name(label))
            .pressed_background(palette.selected).radius(12.0).size(None, Some(64.0)).flex(1.0)
    };
    let head = Element::row(vec![
        top("like", if liked { Icon::HeartFill } else { Icon::Heart }, &if liked { t!("desktop-remove-like") } else { t!("common-like") }),
        top("dislike", Icon::Custom(HEART_CRACK), &if disliked { t!("mobile-undislike") } else { t!("mobile-dislike") }),
        top("share", Icon::Custom(SHARE), &t!("desktop-share")),
    ]);
    let item = |action: &str, label: &str, glyph: Icon, detail: Option<String>| {
        let item = ListRow::new(format!("do:{action}:{row}"), label, palette)
            .sizing(ListRowSizing { height: Some(48.0), two_line_height: Some(60.0), title_size: 16.0, leading_width: 24.0, ..TRACK_ROW })
            .leading(Element::icon(glyph).foreground(palette.text).size(Some(22.0), Some(22.0)));
        match detail { Some(detail) => item.subtitle(detail), None => item }.render()
    };
    let mut groups = vec![vec![item("playlists", &t!("mobile-add-to-playlist"), Icon::Custom(LIST_PLUS), None)]];
    let mut play = vec![
        item("radio", &t!("mobile-start-radio"), Icon::Custom(RADIO), None),
        item("next", &t!("desktop-play-next"), Icon::Custom(LIST_START), None),
        item("queue", &t!("desktop-add-to-queue"), Icon::Custom(LIST_PLUS), None),
        if state.is_saved(&track.id) { item("unsave", &t!("desktop-remove-download"), Icon::Custom(TRASH), None) }
            else { item("save", &t!("desktop-download"), Icon::Custom(crate::view::DOWNLOAD), None) },
    ];
    if row.starts_with("upcoming:") { play.push(item("unqueue", &t!("mobile-remove-from-queue"), Icon::Minus, None)); }
    if matches!(state.page, Page::Playlist(_)) && row.starts_with("page-track:") {
        play.push(item("remove", &t!("desktop-remove-from-playlist"), Icon::Custom(CIRCLE_MINUS), None));
    }
    groups.push(play);
    let mut go = Vec::new();
    if track.album_id.is_some() { go.push(item("album", &t!("desktop-go-to-album"), Icon::Custom(DISC), track.album.clone())); }
    go.extend([
        item("artist", &t!("desktop-go-to-artist"), Icon::Custom(ARTIST), Some(track.artist.clone())),
        item("info", &t!("mobile-song-info"), Icon::InfoCircle, None),
    ]);
    groups.push(go);
    // What plays has no timer of its own on the phone, nor a way to put its lyrics in step.
    if row == "current" {
        let left = state.player.sleep.is_some().then(|| crate::view::sleep_hint(state));
        let offset = (state.lyrics_offset != 0.0).then(|| format!("{:+.1} s", state.lyrics_offset));
        groups.push(vec![
            item("sleep", &t!("desktop-sleep"), Icon::Custom(crate::view::glyph::SLEEP), left),
            item("offset", &t!("mobile-lyrics-offset"), Icon::Custom(SLIDERS), offset),
        ]);
    }
    let mut items = vec![head];
    for group in groups {
        items.push(Separator::horizontal(palette).margin(4.0).render());
        items.extend(group);
    }
    Element::column(items).gap(2.0)
}

/// The lists a song can go into, after the menu's "Add to a playlist": a new one first.
fn picker(state: &State, base: Element, palette: ComponentPalette) -> Element {
    let Some(row) = &state.mobile_ui.picking else { return base };
    let held = |playlist: &crate::state::Playlist| state.track_at(row).is_some_and(|track| playlist.tracks.iter().any(|kept| kept.id == track.id));
    let entry = |id: String, label: &str, glyph: Icon| ListRow::new(id, label, palette)
        .sizing(ListRowSizing { height: Some(52.0), title_size: 16.0, leading_width: 24.0, ..TRACK_ROW })
        .leading(Element::icon(glyph).foreground(palette.text).size(Some(22.0), Some(22.0))).render();
    let mut rows = vec![entry(format!("do:add-new:{row}"), &t!("desktop-add-to-new-playlist"), Icon::Add)];
    rows.extend(state.collection.playlists.iter().filter(|playlist| !held(playlist))
        .map(|playlist| entry(format!("do:add-{}:{row}", playlist.id), &playlist.name, Icon::Playlist)));
    popped(state, BottomSheet::new(base, true, t!("mobile-add-to-playlist"), "", "mobile-picking-close", SCHEME.menu())
        .max_height(560.0).body(Element::column(rows).gap(2.0)).glass(super::SHEET_GLASS).ios(super::ios_sheet(state)).render())
}

/// A song's details, after the menu's "Song info".
fn info_sheet(state: &State, base: Element, palette: ComponentPalette) -> Element {
    let Some(track) = &state.mobile_ui.info else { return base };
    let line = |key: &str, value: String| Element::row(vec![
        Text::new(format!("{key}:"), palette).font_size(15.0).render().font_family(FontFamily::SansBold),
        Text::new(value, palette).font_size(15.0).render().flex(1.0),
    ]).gap(6.0);
    let mut lines = vec![line(&t!("mobile-info-title"), track.title.clone()), line(&t!("mobile-info-artist"), track.artist.clone())];
    lines.extend(track.album.clone().map(|album| line(&t!("mobile-info-album"), album)));
    lines.push(line(&t!("mobile-info-length"), crate::view::clock(track.duration_ms as f64 / 1000.0)));
    lines.extend(state.format_of(&track.id).map(|format| line(&t!("mobile-info-stream"), format.to_owned())));
    lines.push(line(&t!("mobile-info-id"), track.id.clone()));
    popped(state, BottomSheet::new(base, true, t!("mobile-song-info"), "", "mobile-info-close", SCHEME.menu())
        .max_height(480.0).body(Element::column(lines).gap(10.0).padding(8.0)).glass(super::SHEET_GLASS).ios(super::ios_sheet(state)).render())
}

/// How far ahead of the song its lyrics run, moved half a second at a time.
fn offset_sheet(state: &State, base: Element, palette: ComponentPalette) -> Element {
    if !state.mobile_ui.offset_open { return base; }
    let value = Text::new(format!("{:+.1} s", state.lyrics_offset), palette).font_size(34.0).render().font_family(FontFamily::SansBold).centered_x();
    let step = |id: &str, label: &str| Button::new(id, label, palette).variant(ButtonVariant::Default)
        .sizing(limn::ButtonSizing { height: TAP, ..ControlSize::Spacious.button_sizing() }).round().render().flex(1.0);
    let steps = Element::row(vec![step("lyrics-offset:earlier", &t!("mobile-lyrics-earlier")), step("lyrics-offset:later", &t!("mobile-lyrics-later"))]).gap(12.0);
    let reset = Button::new("lyrics-offset:reset", t!("mobile-lyrics-reset"), palette).variant(ButtonVariant::Ghost).render().centered_x();
    popped(state, BottomSheet::new(base, true, t!("mobile-lyrics-offset"), t!("mobile-lyrics-offset-detail"), "lyrics-offset-close", SCHEME.menu())
        .max_height(360.0).body(Element::column(vec![value, steps, reset]).gap(16.0).padding(8.0)).glass(super::SHEET_GLASS).ios(super::ios_sheet(state)).render())
}

/// The sleep timer's choices, as the desktop's menu has them.
fn sleep_sheet(state: &State, base: Element, palette: ComponentPalette) -> Element {
    if !state.sleep_open { return base; }
    let rows = crate::view::sleep_choices(state).into_iter().filter_map(|entry| match entry {
        DropdownMenuEntry::Item(item) => {
            let row = ListRow::new(item.action_id.clone(), item.label.clone(), palette).sizing(ListRowSizing { height: Some(52.0), title_size: 15.0, ..TRACK_ROW });
            Some(if item.selected { row.trailing_icon(Icon::Check) } else { row }.render())
        }
        _ => None,
    }).collect();
    popped(state, BottomSheet::new(base, true, t!("desktop-sleep"), crate::view::sleep_hint(state), "sleep-dismiss", SCHEME.menu())
        .max_height(560.0).body(Element::column(rows).gap(4.0)).return_focus("sleep").glass(super::SHEET_GLASS).ios(super::ios_sheet(state)).render())
}

fn library_sheet(state: &State, base: Element, palette: ComponentPalette) -> Element {
    let Some(context) = &state.mobile_ui.library else { return base };
    let pinned = state.collection.pinned.contains(&context.key);
    let row = |action: &str, label: &str, glyph| ListRow::new(format!("library-action:{action}"), label, palette)
        .sizing(ListRowSizing { height: Some(52.0), title_size: 15.0, ..TRACK_ROW })
        .leading(Element::icon(glyph).foreground(palette.text).size(Some(24.0), Some(24.0))).render();
    let header = Element::row(vec![cover(state, &context.artwork_url, 56.0, palette),
        Text::new(&context.title, palette).font_size(18.0).truncate().render().centered_y().flex(1.0),
    ]).gap(16.0).padding(8.0);
    let kept = context.entry.as_ref().is_none_or(|entry| state.card_entry_kept(entry));
    let mut body = vec![header, Separator::horizontal(palette).render(), row("open", &t!("mobile-open"), Icon::ArrowRight)];
    if kept { body.push(row("pin", &if pinned { t!("mobile-unpin") } else { t!("mobile-pin") }, Icon::Pin)); }
    if context.entry.is_some() && !kept {
        body.push(row("keep", &if context.key.starts_with("artist:") { t!("mobile-follow-artist") } else { t!("mobile-save-to-library") }, Icon::Add));
    } else if context.key.starts_with("album:") || context.key.starts_with("artist:") {
        body.push(row("remove", &if context.key.starts_with("artist:") { t!("mobile-unfollow") } else { t!("mobile-remove-from-library") }, Icon::Minus));
    }
    popped(state, BottomSheet::new(base, true, t!("mobile-library-options"), "", "mobile-library-close", SCHEME.menu())
        .max_height(480.0).body(Element::column(body).gap(8.0))
        .return_focus(format!("library-more:{}", context.key)).glass(super::SHEET_GLASS).ios(super::ios_sheet(state)).render())
}

/// A choice's card in the output sheet, as BitChord's: its own panel, 16 round, a lighter one
/// for where it plays now.
const CARD: Color = Color::rgba(255, 255, 255, 15);
const CARD_ON: Color = Color::rgba(255, 255, 255, 28);
/// The disc each card's glyph stands on.
const BADGE: Color = Color::rgba(255, 255, 255, 22);
const PHONE: &str = include_str!("../../../assets/icons/phone.svg");
const EARBUDS: &str = include_str!("../../../assets/icons/earbuds.svg");

/// A card of the output sheet: `glyph` on its disc, a title over what it is, and what ends it.
fn card(glyph: Icon, title: &str, detail: Option<&str>, on: bool, end: Option<Element>, palette: ComponentPalette) -> Element {
    let badge = Element::panel(Element::icon(glyph).large_icon(20.0).foreground(palette.text).centered()).background(BADGE).round().size(Some(40.0), Some(40.0)).centered_y();
    let mut copy = vec![Text::new(title, palette).font_size(17.0).truncate().render().font_family(FontFamily::SansMedium)];
    copy.extend(detail.map(|detail| Text::new(detail, palette).font_size(13.0).muted().truncate().render()));
    let mut row = vec![badge, Element::column(copy).gap(2.0).centered_y().flex(1.0)];
    row.extend(end.map(Element::centered_y));
    Element::panel(Element::row(row).gap(14.0).padding(12.0)).background(if on { CARD_ON } else { CARD }).radius(16.0).size(None, Some(64.0))
}

/// Where the music plays, as BitChord's sheet: a card for this phone and one for where it
/// plays if that is elsewhere, the one it plays on ticked; on the iPhone, AirPlay and
/// Bluetooth; listening together; the volume; and the path the sound takes.
fn connections(state: &State, base: Element, palette: ComponentPalette) -> Element {
    let tick = || Element::icon(Icon::Check).foreground(palette.text);
    let chevron = || Element::icon(Icon::ChevronRight).foreground(palette.muted);
    let here = t!("mobile-playing-here");
    // iOS names the route; its own speaker is this phone.
    let elsewhere = state.mobile_ui.route.as_ref().filter(|(_, port)| !port.contains("Speaker") && !port.is_empty());
    let mut cards = vec![card(Icon::Custom(PHONE), &t!("mobile-this-phone"), elsewhere.is_none().then_some(here.as_ref()), elsewhere.is_none(), elsewhere.is_none().then(tick), palette)];
    if let Some((name, port)) = elsewhere {
        let glyph = if port.contains("AirPlay") { AUDIO_ROUTE } else { EARBUDS };
        cards.push(card(Icon::Custom(glyph), name, Some(&here), true, Some(tick()), palette));
    }
    // Where the system names its routes, as iOS does, its picker chooses another.
    if cfg!(target_os = "ios") || state.mobile_ui.native_routes || state.mobile_ui.route.is_some() {
        // UIKit owns the visible route button and its tap. This slot only reserves
        // geometry; no simulated click or private picker presentation is used.
        let picker = Element::panel(Element::spacer()).size(Some(TAP), Some(TAP))
            .accessible(limn::Access::new(limn::Role::Group).id("ios-route-picker").name(t!("mobile-route-picker")));
        let detail = if state.mobile_ui.native_routes { t!("mobile-open-route-picker") } else { t!("mobile-on-iphone") };
        cards.push(card(Icon::Custom(AUDIO_ROUTE), &t!("mobile-airplay-bluetooth"), Some(&detail), false, Some(picker), palette));
    }
    cards.push(super::super::jam::queue_entry(state, palette));
    let quiet = Element::icon(Icon::Custom(crate::view::glyph::VOLUME_HIGH)).large_icon(20.0).foreground(palette.muted).centered_y();
    let level = Slider::new("volume", state.volume, ComponentPalette { selected: Color::rgba(255, 255, 255, 66), ..palette }).thumb(6.0, 10.0).bar(6.0, 10.0)
        .ink(Color::rgba(255, 255, 255, 235)).accessibility_label(t!("mobile-volume")).render().size(None, Some(TAP)).flex(1.0);
    cards.push(Element::panel(Element::row(vec![quiet, level]).gap(14.0).padding(10.0)).background(CARD).radius(16.0).size(None, Some(64.0)));
    let summary = state.player.pipeline.as_ref().map(|pipeline| format!("{} · {}", sample_format(&pipeline.sample_format), hertz(pipeline.output_rate)));
    let path = card(Icon::Audio, &t!("mobile-pipeline"), summary.as_deref(), false, Some(chevron()), palette);
    cards.push(Element::clickable("audio-pipeline", path).accessible(limn::Access::new(limn::Role::Button).name(t!("mobile-pipeline"))));
    popped(state, BottomSheet::new(base, state.mobile_ui.connections, t!("mobile-audio-output"), "", "mobile-connections-close", SCHEME.menu())
        .max_height(640.0).body(Element::column(cards).gap(8.0)).return_focus("mobile-connections").glass(super::SHEET_GLASS).ios(super::ios_sheet(state)).render())
}

/// A rate in hertz as kilohertz, as a player prints it: `44.1 kHz`, `48 kHz`.
fn hertz(rate: u32) -> String {
    let khz = f64::from(rate) / 1000.0;
    if khz.fract() == 0.0 { format!("{khz:.0} kHz") } else { format!("{khz:.1} kHz") }
}

/// A sample format as BitChord writes it: `f32` is `32-bit float`, `i16` is `16-bit`.
fn sample_format(format: &str) -> String {
    let bits: String = format.chars().filter(char::is_ascii_digit).collect();
    match format.chars().next() {
        Some('f') => format!("{bits}-bit float"),
        Some('i' | 'u') if !bits.is_empty() => format!("{bits}-bit"),
        _ => format.to_owned(),
    }
}

/// How long the signal's light takes to run its line, from the track to the output.
const SIGNAL_MS: u128 = 2_400;

/// BitChord's Audio Pipeline: what the sound goes through, from the track to the device, one
/// station after another down a line a light runs along. Over the output sheet, with a Done.
fn pipeline(state: &State, base: Element, width: f32, palette: ComponentPalette) -> Element {
    if !state.mobile_ui.pipeline { return base; }
    let on_off = |on: bool| if on { t!("mobile-pipeline-on") } else { t!("mobile-pipeline-off") }.into_owned();
    let channels = |count: u16| match count { 1 => t!("mobile-pipeline-mono").into_owned(), 2 => "Stereo".to_owned(), count => count.to_string() };
    let track = state.current();
    let engine = state.player.pipeline.as_ref().filter(|pipeline| track.is_some_and(|track| track.id == pipeline.track_id));
    let waiting = || vec![(t!("mobile-pipeline-name").into_owned(), t!("mobile-pipeline-waiting").into_owned())];
    let stream = track.and_then(|track| state.format_of(&track.id)).unwrap_or("—");
    let source = if track.is_some_and(|track| state.is_saved(&track.id)) { t!("mobile-pipeline-downloaded") } else { "YouTube Music".into() };
    let mut sections = vec![(Icon::File, t!("mobile-pipeline-track"), vec![
        (t!("mobile-pipeline-source").into_owned(), source.into_owned()),
        (t!("mobile-pipeline-stream").into_owned(), stream.to_owned()),
    ])];
    match engine {
        Some(engine) => {
            sections.push((Icon::Code, t!("mobile-pipeline-decoder"), vec![
                (t!("mobile-pipeline-name").into_owned(), engine.decoder.to_owned()),
                (t!("mobile-pipeline-format").into_owned(), format!("{} · {}", hertz(engine.source_rate), channels(engine.source_channels))),
            ]));
            let same = engine.source_rate == engine.output_rate;
            sections.push((Icon::Settings, t!("mobile-pipeline-resampler"), vec![
                (t!("mobile-pipeline-rate").into_owned(), format!("{} → {}", hertz(engine.source_rate), hertz(engine.output_rate))),
                (t!("mobile-pipeline-type").into_owned(), if same { t!("mobile-pipeline-passthrough").into_owned() } else { "rodio".to_owned() }),
            ]));
        }
        None => sections.push((Icon::Code, t!("mobile-pipeline-decoder"), waiting())),
    }
    let equalizer = match crate::state::equalizer_preset(state.equalizer) {
        Some(("flat", _)) if state.equalizer.preamp_db == 0.0 => t!("mobile-pipeline-off").into_owned(),
        Some((_, name)) => name.to_owned(),
        None => t!("mobile-pipeline-custom").into_owned(),
    };
    let crossfade = if state.settings.crossfade > 0.0 { format!("{:.0} s", state.settings.crossfade) } else { on_off(false) };
    sections.push((Icon::Audio, t!("mobile-pipeline-dsp"), vec![
        (t!("mobile-pipeline-equalizer").into_owned(), equalizer),
        (t!("mobile-pipeline-normalize").into_owned(), on_off(state.settings.normalize)),
        (t!("mobile-pipeline-mono").into_owned(), on_off(state.settings.mono)),
        (t!("mobile-pipeline-crossfade").into_owned(), crossfade),
        (t!("mobile-pipeline-volume").into_owned(), format!("{:.0} %", state.volume * 100.0)),
    ]));
    let mut output = match engine {
        Some(engine) => vec![
            (t!("mobile-pipeline-device").into_owned(), engine.device.clone().unwrap_or_else(|| "—".into())),
            (t!("mobile-pipeline-api").into_owned(), engine.host.to_owned()),
            (t!("mobile-pipeline-format").into_owned(), format!("{} · {} · {}", sample_format(&engine.sample_format), hertz(engine.output_rate), channels(engine.output_channels))),
        ],
        None => waiting(),
    };
    output.extend(state.mobile_ui.route.as_ref().map(|(name, port)| (t!("mobile-pipeline-route").into_owned(), format!("{name} · {port}"))));
    sections.push((Icon::Volume, t!("mobile-pipeline-output"), output));

    let line = Color::rgba(255, 255, 255, 46);
    let node = |glyph: Icon| Element::panel(Element::icon(glyph).large_icon(14.0).foreground(palette.text).centered())
        .background(SCHEME.raised).border(line, 1.0).round().size(Some(26.0), Some(26.0));
    let count = sections.len();
    let blocks: Vec<Element> = sections.into_iter().enumerate().flat_map(|(index, (glyph, title, rows))| {
        let mut copy = vec![Text::new(title.to_uppercase(), palette).font_size(13.0).muted().render().font_family(FontFamily::SansBold).size(None, Some(26.0)).centered_y()];
        copy.extend(rows.into_iter().map(|(key, value)| Element::row(vec![
            Text::new(format!("{key}:"), palette).font_size(15.0).render().font_family(FontFamily::SansBold),
            Text::new(value, palette).font_size(15.0).render().flex(1.0),
        ]).gap(5.0)));
        let block = Element::row(vec![node(glyph), Element::column(copy).gap(5.0).flex(1.0)]).gap(14.0);
        let mut parts = vec![block];
        if index + 1 < count { parts.push(Element::column(vec![hairline()]).padding(10.0)); }
        parts
    }).collect();
    // The line the stations stand on, and the light running down it.
    let at = if state.signal_runs() {
        let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
        (millis % SIGNAL_MS) as f32 / SIGNAL_MS as f32
    } else { 0.5 };
    let light = Element::panel(Element::spacer()).background(Color::rgba(255, 255, 255, 230)).round().size(Some(4.0), Some(28.0));
    let rail = Element::stack(vec![
        Element::panel(Element::spacer()).background(line).size(Some(2.0), None).centered_x(),
        Element::column(vec![Element::spacer().flex(at.max(0.001)), light.centered_x(), Element::spacer().flex((1.0 - at).max(0.001))]),
    ]).size(Some(26.0), None);
    let stations = Element::stack(vec![Element::row(vec![rail, Element::spacer().flex(1.0)]), Element::column(blocks)]);
    // BitChord's card: its name and what it is centred over the stations, which scroll, and a
    // Done under a hairline; grey, with no outline.
    let head = Element::column(vec![
        Text::new(t!("mobile-pipeline"), palette).font_size(20.0).render().font_family(FontFamily::SansBold).centered_x(),
        Text::new(t!("mobile-pipeline-detail"), palette).font_size(15.0).muted().render().centered_x(),
    ]).gap(4.0).padding(16.0);
    let scroll = ScrollArea::new("audio-pipeline-stations", stations.padding(16.0)).render().flex(1.0);
    let done = Element::clickable("audio-pipeline-close", Text::new(t!("mobile-done"), palette).font_size(18.0).render().font_family(FontFamily::SansBold).centered())
        .accessible(limn::Access::new(limn::Role::Button).name(t!("mobile-done"))).size(None, Some(TAP + 8.0));
    let body = Element::column(vec![head, hairline(), scroll, hairline(), done]);
    let card = ComponentPalette { surface: DIALOG, ..palette };
    popped(state, limn::Modal::new(base, true, t!("mobile-pipeline"), t!("mobile-pipeline-detail"), "audio-pipeline-close", card)
        .width(limn::Modal::fit_width(width, 340.0)).height(550.0).headless(true).body(body).outline(Color::TRANSPARENT).backdrop_dismiss(true)
        .glass(super::SHEET_GLASS).render())
}

/// The Audio Pipeline's card, as BitChord's: a grey lifted off the sheet, its lines white at 10%.
const DIALOG: Color = Color::rgb(44, 44, 46);

fn hairline() -> Element {
    Element::spacer().background(Color::rgba(255, 255, 255, 26)).size(None, Some(1.0))
}

#[cfg(test)]
mod menu_tests {
    use super::*;
    use crate::{backend::Command, player::PlayerView};
    use limn::{UiAction, UiTree};

    fn track(id: &str) -> Track {
        Track { id: id.into(), title: id.into(), artist: "Artista".into(), artist_id: None, album: None, album_id: None, artwork_url: None, duration_ms: 180_000 }
    }

    fn phone() -> State {
        let mut state = State::default();
        state.mobile = true;
        state.page = Page::Playing;
        state.show(PlayerView { current: Some(track("a")), upcoming: vec![track("b")], playing: true, duration: 180.0, ..PlayerView::default() });
        state
    }

    fn click(state: &mut State, id: &str) -> Vec<Command> {
        state.update(&UiAction::Clicked { id: id.into() })
    }

    #[test]
    fn the_songs_menu_is_grouped_as_bitchords_with_its_timer_and_lyrics_step_for_what_plays() {
        let mut state = phone();
        click(&mut state, "more:current");
        let mut tree = UiTree::new(render(&state, Element::spacer().size(Some(390.0), Some(844.0)), Size::new(390.0, 844.0), SCHEME.palette()));
        tree.set_reduced_motion(true);
        let frame = tree.layout(Size::new(390.0, 844.0));
        let ids: Vec<_> = frame.access.iter().map(|node| node.id.as_str()).filter(|id| id.starts_with("do:")).collect();
        assert_eq!(&ids[..3], ["do:like:current", "do:dislike:current", "do:share:current"], "like, dislike and share across the top");
        for id in ["do:playlists:current", "do:radio:current", "do:next:current", "do:queue:current", "do:artist:current", "do:info:current", "do:sleep:current", "do:offset:current"] {
            assert!(ids.contains(&id), "{id} in {ids:?}");
        }
    }

    #[test]
    fn the_menu_is_glass_grown_from_its_button_its_right_edge_the_buttons_and_kept_clear_of_the_bars() {
        let mut state = phone();
        state.mobile_ui.content_insets.top = 47.0;
        state.mobile_ui.content_insets.bottom = 34.0;
        let button = limn::Rect::new(320.0, 440.0, 44.0, 44.0);
        state.mobile_ui.menu_origin = Some(button);
        click(&mut state, "more:current");
        let window = Size::new(390.0, 844.0);
        let mut tree = UiTree::new(render(&state, Element::spacer().size(Some(390.0), Some(844.0)), window, SCHEME.palette()));
        tree.set_reduced_motion(true);
        let frame = tree.layout(window);
        let pane = frame.commands.iter().find_map(|command| match command { limn::DrawCommand::Glass { rect, .. } => Some(*rect), _ => None })
            .expect("the menu is a pane of glass");
        assert!((pane.x + pane.width - (button.x + button.width)).abs() < 0.5, "its right edge is the button's: {pane:?}");
        assert!(pane.y >= 47.0 && pane.y + pane.height <= 844.0 - 34.0, "clear of the bars: {pane:?}");
        assert!(frame.regions.iter().any(|region| region.id == "do:info:current"), "its rows are there to press");
    }

    #[test]
    fn a_song_not_wanted_is_skipped_kept_out_of_the_picks_and_unliked() {
        let mut state = phone();
        click(&mut state, "do:like:current");
        assert!(state.is_liked("a"));
        assert_eq!(click(&mut state, "do:dislike:current"), vec![Command::Next], "what plays gives way");
        assert!(state.collection.is_disliked("a") && !state.is_liked("a"));
        assert!(state.taste().tracks.iter().any(|id| id == "a"), "as good as known, never picked");
        let next = state.show(PlayerView { current: Some(track("a")), ..state.player.clone() });
        assert!(next.is_empty() || !next.contains(&Command::Next), "already playing it: no second skip");
        state.show(PlayerView { current: Some(track("b")), ..state.player.clone() });
        assert!(state.show(PlayerView { current: Some(track("a")), ..state.player.clone() }).contains(&Command::Next), "when it comes again it is passed over");
        click(&mut state, "do:dislike:current");
        assert!(!state.collection.is_disliked("a"), "and the mark comes off");
    }

    #[test]
    fn radio_playlists_info_and_the_lyrics_step_answer_from_the_menu() {
        let mut state = phone();
        assert_eq!(click(&mut state, "do:radio:current"), vec![Command::StartRadio(track("a"))]);
        click(&mut state, "do:playlists:current");
        assert_eq!(state.mobile_ui.picking.as_deref(), Some("current"), "the lists are offered");
        click(&mut state, "do:add-new:current");
        assert!(state.mobile_ui.picking.is_none() && state.creating.is_some(), "a new list is made for it, and the offer closes");
        click(&mut state, "do:info:current");
        assert_eq!(state.mobile_ui.info.as_ref().map(|track| track.id.as_str()), Some("a"));
        state.lyrics = Some(("a".into(), crate::state::Lyrics::Lines(vec![
            surco_core::lyrics::Line { words: Vec::new(), at_ms: 0, text: "uno".into() },
            surco_core::lyrics::Line { words: Vec::new(), at_ms: 1_000, text: "dos".into() },
        ])));
        state.player.position = 0.6;
        assert_eq!(state.active_line(), Some(0));
        click(&mut state, "lyrics-offset:earlier");
        assert_eq!(state.active_line(), Some(1), "half a second earlier, the next line is already sung");
        click(&mut state, "lyrics-offset:reset");
        assert_eq!(state.lyrics_offset, 0.0);
    }
}

#[cfg(test)]
mod pipeline_tests {
    use super::*;
    use crate::player::PlayerView;
    use limn::{DrawCommand, UiAction, UiTree};

    #[test]
    fn the_pipeline_melts_back_into_its_card_when_it_is_done() {
        let mut state = State::default();
        state.mobile = true;
        state.page = Page::Playing;
        state.mobile_ui.connections = true;
        state.mobile_ui.sheet_from = Some(limn::Rect::new(20.0, 700.0, 350.0, 60.0));
        let window = Size::new(390.0, 844.0);
        let view = |state: &State| render(state, Element::spacer().size(Some(390.0), Some(844.0)), window, SCHEME.palette());
        // The panes themselves, not the glass circles of a sheet's bar.
        let panes = |frame: &limn::Frame| frame.commands.iter().filter(|command| matches!(command, DrawCommand::Glass { rect, .. } if rect.width > 100.0)).count();
        let now = std::time::Instant::now();
        let mut tree = UiTree::new(view(&state));
        tree.layout_at(window, now);
        // The pipeline is opened from the output sheet, over it.
        state.mobile_ui.pipeline = true;
        tree.set_root(view(&state));
        tree.layout_at(window, now);
        assert_eq!(panes(&tree.layout_at(window, now + std::time::Duration::from_millis(800))), 2, "the sheet and the pipeline over it");
        state.update(&UiAction::Clicked { id: "audio-pipeline-close".into() });
        tree.set_root(view(&state));
        tree.layout_at(window, now + std::time::Duration::from_millis(800));
        let leaving = tree.layout_at(window, now + std::time::Duration::from_millis(880));
        assert_eq!(panes(&leaving), 2, "the pipeline still on its way back into the card");
        let widths: Vec<f32> = leaving.commands.iter().filter_map(|command| match command { DrawCommand::Glass { rect, .. } => Some(rect.width), _ => None }).collect();
        assert!(widths[1] < widths[0], "and over the sheet it was opened from, not under it: {widths:?}");
    }

    #[test]
    fn the_pipeline_tells_what_the_engine_set_up_from_the_decoder_to_the_device() {
        assert_eq!((hertz(44_100), hertz(48_000)), ("44.1 kHz".to_owned(), "48 kHz".to_owned()));
        assert_eq!((sample_format("f32"), sample_format("i16")), ("32-bit float".to_owned(), "16-bit".to_owned()));
        let track = Track { id: "song".into(), title: "Canción".into(), artist: "Artista".into(), artist_id: None, album: None, album_id: None, artwork_url: None, duration_ms: 180_000 };
        let pipeline = surco_audio::Pipeline { track_id: "song".into(), decoder: "libopus", source_rate: 48_000, source_channels: 2,
            device: Some("Auriculares USB".into()), host: "ALSA", output_rate: 44_100, output_channels: 2, sample_format: "f32".into() };
        let mut state = State::default();
        state.mobile = true;
        state.show(PlayerView { current: Some(track), playing: true, duration: 180.0, pipeline: Some(pipeline), ..PlayerView::default() });
        state.update(&UiAction::Clicked { id: "mobile-connections".into() });
        // What is drawn, run together as the lines wrap it, from the top of the dialog's body
        // or scrolled to its end.
        let texts = |state: &State, end: bool| {
            let size = Size::new(390.0, 844.0);
            let mut tree = UiTree::new(render(state, Element::spacer().size(Some(390.0), Some(844.0)), Size::new(390.0, 844.0), SCHEME.palette()));
            tree.set_reduced_motion(true);
            let frame = tree.layout(size);
            if end {
                for scroll in frame.regions.iter().filter(|region| region.kind == limn::HitKind::Scroll).map(|region| region.id.clone()).collect::<Vec<_>>() { tree.scroll_to_end(&scroll); }
            }
            tree.layout(size).commands.iter().filter_map(|command| match command { DrawCommand::Text { text, .. } => Some(text.clone()), _ => None }).collect::<String>()
        };
        assert!(texts(&state, false).contains("32-bit float · 44.1 kHz"), "the sheet's card says what the device is fed");
        state.update(&UiAction::Clicked { id: "audio-pipeline".into() });
        let (top, end) = (texts(&state, false), texts(&state, true));
        for expected in ["libopus", "48 kHz → 44.1 kHz", "rodio"] {
            assert!(top.contains(expected), "{expected} in {top}");
        }
        for expected in ["Auriculares USB", "ALSA", "32-bit float · 44.1 kHz · Stereo"] {
            assert!(end.contains(expected), "{expected} in {end}");
        }
        state.update(&UiAction::Clicked { id: "audio-pipeline-close".into() });
        assert!(!state.mobile_ui.pipeline && state.mobile_ui.connections, "Done goes back to the sheet");
    }
}
