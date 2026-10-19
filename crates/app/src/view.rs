//! The window, described from the state. Rebuilt on every frame.

mod mobile;
mod equalizer;
pub(crate) use mobile::route_key as mobile_route_key;
#[cfg(any(target_os = "ios", test))]
pub(crate) use mobile::{header_buttons, surface_tint, tabs, title as mobile_title};

use surco_audio::equalizer::BAND_HZ;
use surco_core::{discover, Album, AlbumRef, Artist, Repeat, Track};
use limn::{
    Alert, AlertDialog, Button, ButtonGroup, ButtonGroupSizing, ButtonVariant, Color, Component,
    ComponentPalette, ContextMenu, ContextMenuEntry, ContextMenuItem, ControlColors, ControlSize, DropdownMenu, DropdownMenuEntry,
    DropdownMenuItem, DropdownOption, Element, Empty, FontFamily, FormField, FormFieldSizing, Icon, IconButton, LevelBars, Link, ListRow, MediaCard, MediaTile,
    Modal, Popover, PopoverPlacement, Progress, RadiusRole, ResizeHandle, ListRowSizing, Scheme,
    Markdown, ScrollArea, ScrollbarAppearance, SearchField, Select, SelectorSizing, Separator, Size, SkeletonGroup, Slider, Spinner, Switch, Tab, Table, TableColumn, TableRow, TableSizing, Tabs, Text,
    TextField, TextRole, Ticker, Toast, TooltipPlacement, TooltipSpec, TopBar, Carousel, SideRail, SideRailItem, PeekPanel,
};

use crate::backend::{Release, Target};
use crate::clip::Showing;
use crate::i18n::{t, Language};
use crate::state::mobile::SearchFilter;
use crate::state::{
    clock, greeting, large_cover, share_link, Confirm, SUGGESTED_SHOWN, Entry, Filter, LibraryView, Lyrics, Mix, MixKind, Page, Playlist, Quality, Searched, Side, Sort, State, Updating, CROSSFADE_MAX, EQUALIZER_DB, RELEASE_LINK, SHELF,
};
use crate::update::Note;

/// The app's colors, one per role: near-black surfaces told apart by luminance, white ink.
/// Every palette a component takes is derived from here, so that a control on a card, a
/// menu and a page cannot disagree about a shared color.
pub const SCHEME: Scheme = Scheme {
    canvas: Color::rgb(18, 18, 18),
    panel: Color::rgb(18, 18, 18),
    raised: Color::rgb(28, 28, 28),
    hover: Color::rgb(34, 34, 34),
    selected: Color::rgb(42, 42, 42),
    border: Color::rgb(46, 46, 46),
    text: Color::rgb(255, 255, 255),
    muted: Color::rgb(179, 179, 179),
    muted_on_raised: Color::rgb(179, 179, 179),
    // Spotify's menu: a grey lifted off the panels, its row a step lighter under the pointer.
    menu: Color::rgb(40, 40, 40),
    menu_hover: Color::rgb(62, 62, 62),
    menu_muted: Color::rgb(179, 179, 179),
    shadow: Color::rgba(0, 0, 0, 140),
    control: ControlColors {
        surface: Color::rgb(42, 42, 42),
        hover: Color::rgb(52, 52, 52),
        pressed: Color::rgb(60, 60, 60),
        border: Color::rgb(60, 60, 60),
        text: Color::rgb(255, 255, 255),
    },
};

/// A segmented control: a track a step above its surface, the chosen segment sunk to the
/// page's color under it.
fn segmented_palette() -> ComponentPalette {
    ComponentPalette { surface: SCHEME.selected, raised: SCHEME.selected, hover: SCHEME.control.hover, selected: SCHEME.canvas, ..SCHEME.palette() }
}

/// A choice among a few, as wide as the room it is given. `group` names it, so the
/// selection travels from one segment to the next.
/// The page's search field, fully open and as it starts to grow.
const PAGE_SEARCH_WIDTH: f32 = 220.0;
const PAGE_SEARCH_SHUT: f32 = 32.0;
/// Where the window presents the frames of the clip of what plays.
pub const CLIP_SURFACE: &str = "now-clip";
/// The glow of the video behind it, where it does not fill the window.
pub const CLIP_BACKDROP: &str = "now-clip-glow";
/// The mark of the lyrics: words in a bubble.
const LYRICS: &str = include_str!("../assets/icons/message-square-quote.svg");
// A route chooser is distinct from a waveform; UIKit supplies airplay.audio on iOS.
const AUDIO_ROUTE: &str = include_str!("../assets/icons/airplay-audio.svg");
/// What dims a cover under what is drawn over it.
const COVER_SCRIM: Color = Color::rgba(0, 0, 0, 150);
/// The stripe along the foot of a mix's picture, one a mix, as each series of records has
/// a band of its own.
const MIX_STRIPES: [Color; 6] = [
    Color::rgb(255, 99, 72),
    Color::rgb(245, 179, 34),
    Color::rgb(24, 181, 168),
    Color::rgb(151, 92, 232),
    Color::rgb(62, 148, 240),
    Color::rgb(232, 72, 152),
];
/// A bar of the playing row's wave, and the space between two.
const WAVE_BAR: f32 = 3.0;
const WAVE_GAP: f32 = 2.0;
/// A line of lyrics reads from across the room; the furthest off are this much out of focus.
const LYRIC_SIZE: f32 = 24.0;
/// In a pane this wide or wider the lines grow to this.
const LYRIC_WIDE_FROM: f32 = 640.0;
const LYRIC_SIZE_WIDE: f32 = 32.0;
/// A phone holds the lyrics close: fewer, larger lines.
const LYRIC_SIZE_PHONE: f32 = 28.0;
const LYRIC_GAP: f32 = 18.0;
const LYRIC_BLUR: f32 = 2.2;
/// How far up the panel the cover's colour reaches behind the lyrics.
const LYRIC_WASH: f32 = 520.0;
/// How far down a page the colour of its cover reaches, from behind its header.
const HEADER_WASH: f32 = 360.0;
pub(crate) const MOBILE_HEADER_HEIGHT: f32 = 56.0;

fn segments(group: &str, width: f32, tabs: Vec<Tab>) -> Element {
    Tabs::new(tabs, segmented_palette()).id(group).size(ControlSize::Compact).radius(pill(ControlSize::Compact)).fill_width(width).render()
}

/// The corner of a segment's selection that makes it and its track pills (the track rounds 2 more).
fn pill(size: ControlSize) -> f32 {
    size.height() / 2.0 - 2.0
}

fn menu_sizing(state: &State) -> SelectorSizing {
    if state.mobile { SelectorSizing { row_height: 44.0, detail_row_height: 60.0, label_size: 15.0, ..SelectorSizing::COMPACT } }
    // Spotify's rows: 40 high, the label at 14.
    else { SelectorSizing { row_height: 40.0, label_size: 14.0, ..SelectorSizing::COMPACT } }
}

/// A menu as Spotify's: no rule round it nor between its groups, which its gaps tell apart.
fn menu_palette() -> ComponentPalette {
    ComponentPalette { border: Color::rgba(0, 0, 0, 0), ..SCHEME.menu() }
}

/// The window is laid out as Spotify's: a bar with the search across the top, three rounded
/// panels a gap apart on a black floor, and the playback bar across the bottom. The gaps
/// beside the middle panel are the handles that resize the other two.
const CANVAS: Color = Color::rgb(0, 0, 0);
const CHOSEN: Color = Color::rgb(31, 31, 31);
/// The lifted surface of a phone promotion card, as Spotify's.
const PROMO: Color = Color::rgb(36, 36, 36);
const GAP: f32 = 8.0;
/// Spotify's docks are nearly square: half the corner limn would give them.
const PANEL_RADIUS: f32 = 4.0;
pub(crate) const TOP_BAR_HEIGHT: f32 = 64.0;
/// The widest the top bar's search grows, and the recent searches under it with it.
const SEARCH_MAX_WIDTH: f32 = 546.0;
/// The round button's fill under the pointer and while held, and what it shows disabled.
const ROUND_HOVER: Color = Color::rgb(224, 224, 224);
const ROUND_HELD: Color = Color::rgb(204, 204, 204);
const ROUND_OFF: Color = Color::rgb(54, 54, 54);
const ROUND_OFF_INK: Color = Color::rgb(117, 117, 117);
const PLAYER_HEIGHT: f32 = 76.0;
const VOLUME_WIDTH: f32 = 93.0;
const VOLUME_WIDTH_NARROW: f32 = 64.0;
/// The library folded to its pictures: when the user folds it, and for a window too narrow
/// to spare it a column.
const RAIL_WIDTH: f32 = SideRail::WIDTH;
/// The strip a shut right panel leaves at the window's edge.
const PEEK_WIDTH: f32 = 40.0;
const RAIL_BELOW: f32 = 860.0;
/// From this width the right panel has a column of its own; below it, the queue takes the
/// content's place.
pub(crate) const PANEL_FROM: f32 = 1024.0;
/// What the middle panel keeps however wide the user drags the other two.
const CONTENT_MIN_WIDTH: f32 = 360.0;
/// The popover: ten band columns of 34 inside its padding.
const EQUALIZER_WIDTH: f32 = 364.0;
const TILE_WIDTH: f32 = 148.0;
/// On a desktop a tile is a card: its padding frames the cover, and the cards of a shelf
/// stand side by side, their padding the space between the covers.
const TILE_PAD: f32 = 12.0;
const CARD_WIDTH: f32 = TILE_WIDTH + 2.0 * TILE_PAD;
/// The space between two tiles of a shelf, and the room under them for the shelf's bar.
const TILE_GAP: f32 = 12.0;
const SHELF_BAR_ROOM: f32 = 12.0;
const COVER: f32 = 40.0;
const NUMBER_WIDTH: f32 = 24.0;
const DURATION_WIDTH: f32 = 56.0;
/// The column of a row's like, as wide as its button.
const LIKE_WIDTH: f32 = 32.0;
const MENU_WIDTH: f32 = 320.0;
/// The most rows a menu shows before it scrolls.
const MENU_ROWS: usize = 12;
const DOWNLOAD: &str = include_str!("../assets/icons/download.svg");
/// The dot under a control of the playback bar that is on.
const ACTIVE_DOT: f32 = 4.0;
/// What the bar's start keeps beside the title and artist: the cover, the heart and the gaps.
const NOW_PLAYING_ROOM: f32 = 140.0;
/// limn's shuffle with a spark over where its arrows cross.
const SMART_SHUFFLE: &str = include_str!("../assets/icons/shuffle-spark.svg");

/// Surco's own glyphs for the desktop, drawn on a 16 grid the way Spotify's are: thin straight
/// strokes, a filled shape only where something is on. A phone keeps the platform's.
mod glyph {
    pub const SHUFFLE: &str = include_str!("../assets/icons/glyph/shuffle.svg");
    pub const SMART_SHUFFLE: &str = include_str!("../assets/icons/glyph/shuffle-smart.svg");
    pub const PREVIOUS: &str = include_str!("../assets/icons/glyph/previous.svg");
    pub const NEXT: &str = include_str!("../assets/icons/glyph/next.svg");
    pub const REPEAT: &str = include_str!("../assets/icons/glyph/repeat.svg");
    pub const REPEAT_ONE: &str = include_str!("../assets/icons/glyph/repeat-one.svg");
    pub const NOW_PLAYING_VIEW: &str = include_str!("../assets/icons/glyph/now-playing-view.svg");
    pub const LYRICS: &str = include_str!("../assets/icons/glyph/microphone.svg");
    pub const QUEUE: &str = include_str!("../assets/icons/glyph/queue.svg");
    pub const DEVICE: &str = include_str!("../assets/icons/glyph/connect-device.svg");
    pub const EQUALIZER: &str = include_str!("../assets/icons/glyph/equalizer.svg");
    pub const VOLUME_OFF: &str = include_str!("../assets/icons/glyph/volume-off.svg");
    pub const VOLUME_LOW: &str = include_str!("../assets/icons/glyph/volume-low.svg");
    pub const VOLUME_HIGH: &str = include_str!("../assets/icons/glyph/volume-high.svg");
    pub const FULLSCREEN: &str = include_str!("../assets/icons/glyph/fullscreen.svg");
    pub const VIDEO: &str = include_str!("../assets/icons/glyph/video.svg");
    pub const PICTURE: &str = include_str!("../assets/icons/glyph/picture.svg");
    pub const PENCIL: &str = include_str!("../assets/icons/glyph/pencil.svg");
    pub const PLUS: &str = include_str!("../assets/icons/glyph/plus.svg");
    pub const PANEL_RIGHT: &str = include_str!("../assets/icons/glyph/panel-right.svg");
    pub const CLOSE: &str = include_str!("../assets/icons/glyph/close.svg");
    pub const CHEVRON_RIGHT: &str = include_str!("../assets/icons/glyph/chevron-right.svg");
    pub const CHEVRON_LEFT: &str = include_str!("../assets/icons/glyph/chevron-left.svg");
    pub const BELL: &str = include_str!("../assets/icons/glyph/bell.svg");
    pub const HOME: &str = include_str!("../assets/icons/glyph/home.svg");
    pub const HOME_FILL: &str = include_str!("../assets/icons/glyph/home-fill.svg");
    pub const BROWSE: &str = include_str!("../assets/icons/glyph/browse.svg");
    pub const BROWSE_FILL: &str = include_str!("../assets/icons/glyph/browse-fill.svg");
    pub const SEARCH: &str = include_str!("../assets/icons/glyph/search.svg");
    pub const BELL_FILL: &str = include_str!("../assets/icons/glyph/bell-fill.svg");
    pub const LIBRARY: &str = include_str!("../assets/icons/glyph/library.svg");
    pub const DISPLAY: &str = include_str!("../assets/icons/glyph/display.svg");
    pub const INFO: &str = include_str!("../assets/icons/glyph/info.svg");
    pub const DOWNLOAD: &str = include_str!("../assets/icons/glyph/download.svg");
    /// Liking, and liked: a circled plus, and the circle filled with the check cut out of it.
    pub const ADD: &str = include_str!("../assets/icons/glyph/add-circle.svg");
    pub const ADDED: &str = include_str!("../assets/icons/glyph/added-circle.svg");
}

/// The like of a song, as Spotify's: a circled plus that becomes a filled circle with a check,
/// in the app's own ink rather than a colour.
fn like_glyph(liked: bool) -> Icon {
    Icon::Custom(if liked { glyph::ADDED } else { glyph::ADD })
}

/// The glyph of the volume, in three steps.
fn volume_glyph(volume: f32) -> Icon {
    Icon::Custom(match volume {
        level if level <= 0.0 => glyph::VOLUME_OFF,
        level if level < 0.5 => glyph::VOLUME_LOW,
        _ => glyph::VOLUME_HIGH,
    })
}
/// An entry of the library: 64 high around a 48 tile.
const LIBRARY_ROW: ListRowSizing = ListRowSizing {
    height: Some(64.0),
    two_line_height: Some(64.0),
    padding: 8.0,
    gap: 12.0,
    leading_width: LIBRARY_TILE,
    title_size: 15.0,
    subtitle_size: 12.0,
    copy_gap: 2.0,
    title_height: Some(26.0),
    subtitle_height: Some(20.0),
    ..ListRowSizing::COMPACT
};
const LIBRARY_TILE: f32 = 48.0;
/// The most entries quick access offers, the corner of each, and its fill under the pointer.
const QUICK_MOST: usize = 8;
const QUICK_RADIUS: f32 = 4.0;
const QUICK_HOVER: Color = Color::rgb(56, 56, 56);
/// A quick-access entry of the home page on a phone: a short row around the same tile.
const QUICK_ROW: ListRowSizing = ListRowSizing { height: Some(64.0), padding: 8.0, gap: 12.0, leading_width: LIBRARY_TILE, title_size: 14.0, ..ListRowSizing::COMPACT };
/// A track's row: 60 high around a 40 cover, the title at 15 over a muted line at 12.
const TRACK_ROW: ListRowSizing = ListRowSizing {
    height: Some(60.0),
    two_line_height: Some(60.0),
    padding: 8.0,
    gap: 16.0,
    leading_width: COVER,
    leading_height: Some(44.0),
    title_size: 15.0,
    subtitle_size: 12.0,
    copy_gap: 2.0,
    title_height: Some(22.0),
    subtitle_height: Some(20.0),
    ..ListRowSizing::COMPACT
};
const ALBUM_COVER: f32 = 168.0;
const ARTIST_PICTURE: f32 = 128.0;
const PAGE_TITLE: f32 = 26.0;
/// The name over a page's songs: an artist, an album, a list. As Spotify's, it is the largest
/// thing on the page.
const HERO_TITLE: f32 = 48.0;

pub fn view(state: &State, window: Size) -> Element {
    if state.mobile {
        return mobile::view(state, window);
    }
    let palette = SCHEME.palette();
    if state.cinema {
        return update_dialog(state, cinema(state, window, palette), palette);
    }
    let surface = |element: Element| Element::panel(element).background(SCHEME.canvas).radius(PANEL_RADIUS);
    let gap = || Element::spacer().size(Some(GAP), None);
    // A panel on its way open or shut is drawn part of its width, uncovering content that
    // keeps the width it has at rest.
    let sliding = |content: Element, whole: f32, shown: f32| surface(Element::row(vec![content.size(Some(whole), None)])).size(Some(shown), None).clip_children();
    let now = std::time::Instant::now();
    // Folded, the library is a rail of pictures, as Spotify's: when the user folds it, and
    // always in a window too narrow to spare it a column. Unfolding, it grows out of the rail.
    let narrow = window.width < RAIL_BELOW;
    let unfold = if narrow { 0.0 } else { state.library_slide.value(now).clamp(0.0, 1.0) };
    let right_slide = if window.width >= PANEL_FROM { state.panel_slide.value(now).clamp(0.0, 1.0) } else { 0.0 };
    // Shut, the right panel leaves a strip at the window's edge that peeks at what it holds.
    let peek = window.width >= PANEL_FROM && right_slide == 0.0;
    // The side panels give way before the content does.
    let right_whole = state.right_width.min(window.width - 4.0 * GAP - RAIL_WIDTH - CONTENT_MIN_WIDTH);
    let right = right_whole * right_slide;
    let left_whole = state.left_width.min(window.width - 4.0 * GAP - right - CONTENT_MIN_WIDTH).max(RAIL_WIDTH);
    let left = RAIL_WIDTH + (left_whole - RAIL_WIDTH) * unfold;
    // Only a panel at rest can be resized; a folded library is dragged open from its edge.
    let handle = |side: Side, rest: bool| if rest { ResizeHandle::vertical(side.handle(), palette).thickness(GAP).render() } else { gap() };

    let mut columns = vec![gap()];
    let mut content = window.width - 2.0 * GAP - left - GAP;
    let library = if unfold > 0.0 {
        sliding(library(state, false, left_whole, palette), left_whole, left)
    } else {
        surface(library(state, true, RAIL_WIDTH, palette)).size(Some(RAIL_WIDTH), None).clip_children()
    };
    columns.extend([library, handle(Side::Left, !narrow && (unfold == 0.0 || unfold == 1.0))]);
    if peek {
        // The strip runs to the window's edge, in place of the gap there.
        content -= PEEK_WIDTH;
    }
    if right_slide > 0.0 {
        content -= right + GAP;
        columns.push(surface(pane(state, content, palette)).corner_mask(CANVAS).flex(1.0));
        // Its corners stay round over the song's video, which is drawn edge to edge.
        columns.extend([handle(Side::Right, right_slide == 1.0), sliding(panel(state, window, palette), right_whole, right).corner_mask(CANVAS)]);
    } else if state.queue_open && window.width < PANEL_FROM {
        columns.push(surface(panel(state, window, palette)).flex(1.0));
    } else {
        // A page's wash runs to the pane's edges: the mask keeps its corners round.
        columns.push(surface(pane(state, content, palette)).corner_mask(CANVAS).flex(1.0));
    }
    columns.push(gap());
    if peek {
        let strip = PeekPanel::new("panel-peek", panel(state, window, palette), palette)
            .side(limn::Side::Right)
            .width(PEEK_WIDTH)
            .content_width(right_whole)
            .accessibility_label(t!("desktop-show-now-playing"));
        columns.push(strip.render());
    }
    let window_tree = Element::panel(Element::column(vec![top_bar(state, window.width, palette), Element::row(columns).flex(1.0), player(state, window.width, palette)]))
        .background(CANVAS)
        .size(Some(window.width), Some(window.height));
    let shared = share_dialog(state, window_tree, window.width, palette);
    let created = handoff_dialog(state, profile_dialog(state, playlist_dialog(state, shared, window.width, palette), window.width), window.width);
    let imported = missed_dialog(state, import_dialog(state, created, window.width, palette), window.width, palette);
    let whole = update_dialog(state, confirm_dialog(state, welcome_dialog(state, imported, window.width), palette), palette);
    Toast::layer(whole, update_toast(state, palette), PLAYER_HEIGHT + 12.0)
}

/// A later version found while the app is open, said over the window's corner until it
/// is closed or followed. One that has to be taken is said by its dialog.
fn update_toast(state: &State, palette: ComponentPalette) -> Vec<Element> {
    let release = state.update.as_ref().filter(|release| state.update_toast && !release.mandatory);
    let card = |release: &Release| {
        Toast::new(t!("desktop-update-toast", version = release.version.as_str()), palette).description(t!("desktop-update-toast-detail")).action("update-toast").dismissible("update-toast-close").render()
    };
    release.map(card).into_iter().collect()
}

/// How a form's field reads in a dialog: its label a step up, its hint under the control.
const DIALOG_FIELD: FormFieldSizing = FormFieldSizing { label_role: TextRole::Label, hint_role: TextRole::Caption, gap: 8.0, ..FormFieldSizing::DEFAULT };

/// A dialog as Spotify's: its menus' grey with no rule round it, and fields a step lighter
/// with none either.
fn dialog_palette() -> ComponentPalette {
    ComponentPalette { surface: Color::rgb(40, 40, 40), raised: Color::rgb(62, 62, 62), border: Color::rgba(0, 0, 0, 0), ..SCHEME.palette() }
}

/// A dialog's footer: the way out first, what goes ahead last, at the end.
fn dialog_footer(cancel: Element, confirm: Element) -> Element {
    ButtonGroup::new(vec![cancel, confirm]).align_end().sizing(ButtonGroupSizing { gap: 8.0, stacked_gap: 8.0 }).render()
}

/// Making a playlist: its name, and whether others will add to it.
fn playlist_dialog(state: &State, window_tree: Element, width: f32, palette: ComponentPalette) -> Element {
    let Some(draft) = &state.creating else { return window_tree };
    if state.mobile {
        let name = TextField::new(
            "playlist-name",
            &draft.name,
            state.collection.default_name(),
            palette,
        )
        .size(ControlSize::Spacious)
        .render()
        .size(None, Some(44.0));
        let mut fields = FormField::new(t!("common-title"), name, palette)
            .sizing(DIALOG_FIELD)
            .rows();
        if let Some(track) = &draft.first {
            fields.push(
                Text::new(t!("desktop-playlist-starts-with", title = track.title.as_str()), palette)
                    .muted()
                    .render(),
            );
        }
        let cancel = Button::new("playlist-cancel", t!("common-cancel"), palette)
            .variant(ButtonVariant::Outline)
            .render()
            .size(None, Some(44.0));
        let create = Button::new("playlist-create", t!("desktop-create"), palette)
            .render()
            .size(None, Some(44.0));
        return limn::BottomSheet::new(
            window_tree,
            true,
            t!("desktop-new-playlist"),
            t!("desktop-new-playlist-detail"),
            "playlist-cancel",
            palette,
        )
        .max_height(560.0)
        .body(Element::column(fields).gap(8.0))
        .footer(dialog_footer(cancel, create))
        .initial_focus("playlist-name")
        .return_focus("playlist-new")
        .render();
    }
    let palette = dialog_palette();
    let dialog = Modal::fit_width(width, 440.0).max(240.0);
    // This dialog is short enough never to scroll: its form reaches both paddings.
    let form = Modal::fitted_body_width(dialog);
    let name = TextField::new("playlist-name", &draft.name, state.collection.default_name(), palette).size(ControlSize::Regular).render();
    let kind = |id: &str, label: &str, chosen: bool| Tab::new(id, label, 0.0).active(chosen);
    let kinds = segments(
        "playlist-kind",
        form,
        vec![kind("playlist-kind:normal", &t!("desktop-playlist-normal"), !draft.collaborative), kind("playlist-kind:collaborative", &t!("desktop-playlist-collaborative"), draft.collaborative)],
    );
    let about = if draft.collaborative {
        t!("desktop-playlist-collaborative-detail")
    } else {
        t!("desktop-playlist-normal-detail")
    };
    let mut fields: Vec<Element> = FormField::new(t!("common-title"), name, palette).sizing(DIALOG_FIELD).rows();
    fields.extend(FormField::new(t!("desktop-playlist-kind"), kinds, palette).hint(about).sizing(DIALOG_FIELD).rows());
    if let Some(track) = &draft.first {
        fields.push(Text::new(t!("desktop-playlist-starts-with", title = track.title.as_str()), palette).role(TextRole::Caption).muted().truncate().render());
    }
    let cancel = Button::new("playlist-cancel", t!("common-cancel"), palette).variant(ButtonVariant::Ghost).size(ControlSize::Regular).round().render();
    let create = Button::new("playlist-create", t!("desktop-create"), palette).size(ControlSize::Regular).round().render();
    Modal::new(window_tree, true, t!("desktop-new-playlist"), t!("desktop-new-playlist-detail"), "playlist-cancel", palette)
        .width(dialog)
        .body(Element::column(fields).gap(8.0))
        .footer(dialog_footer(cancel, create))
        .initial_focus("playlist-name")
        .return_focus("playlist-new")
        .render()
}

/// Taking the account to another device on the same network, as Spotify's devices dialog
/// reads: sending it, with its code for a phone's camera, the address for another computer and
/// the parts that go; or receiving one from the address another device shows, and choosing
/// what of it comes and whether it joins what is here or takes its place.
fn handoff_dialog(state: &State, window_tree: Element, width: f32) -> Element {
    use mobile::settings::switch;
    let Some(handoff) = &state.handoff else { return window_tree };
    let palette = dialog_palette();
    let _dense = (!state.mobile).then(mobile::settings::Dense::on);
    let dialog = Modal::fit_width(width, 560.0).max(280.0);
    let form = Modal::fitted_body_width(dialog);
    let tab = |id: &str, label: &str, chosen: bool| Tab::new(id, label, 0.0).active(chosen);
    let tabs = segments("handoff-tab", form, vec![tab("handoff-tab:send", &t!("desktop-handoff-send"), !handoff.receiving), tab("handoff-tab:receive", &t!("desktop-handoff-receive"), handoff.receiving)]);
    let mut body = vec![tabs];
    let collection = &state.collection;
    let mut cancel = Button::new("handoff-close", t!("common-close"), palette).variant(ButtonVariant::Ghost).size(ControlSize::Regular).round().render();
    let mut confirm = None;
    if !handoff.receiving {
        match &handoff.offer {
            None => body.push(Element::row(vec![Spinner::new("handoff-serving", palette.muted).size(16.0).render(), Text::new(t!("desktop-handoff-starting"), palette).muted().render()]).gap(8.0)),
            Some(Err(message)) => body.push(Alert::new(t!("desktop-handoff"), message, palette).id("handoff-error").compact(true).render()),
            Some(Ok((offer, code))) => {
                let code = Element::panel(Element::raster_image(code.clone()).size(Some(HANDOFF_CODE), Some(HANDOFF_CODE)))
                    .background(Color::WHITE).radius(RadiusRole::Card.px()).padding(6.0);
                let copy = Button::new("handoff-copy", t!("desktop-handoff-copy"), palette).variant(ButtonVariant::Outline).size(ControlSize::Compact).round().render();
                let ways = Element::column(vec![
                    Text::new(t!("desktop-handoff-phone"), palette).font_size(16.0).render().font_family(FontFamily::SansBold),
                    Text::new(t!("desktop-handoff-phone-detail"), palette).muted().render(),
                    Element::spacer().size(None, Some(8.0)),
                    Text::new(t!("desktop-handoff-computer"), palette).font_size(16.0).render().font_family(FontFamily::SansBold),
                    Text::new(t!("desktop-handoff-computer-detail"), palette).muted().render(),
                    Element::text(&offer.address).font_family(FontFamily::Monospace).font_size(13.0).foreground(palette.text),
                    Element::row(vec![copy]),
                ]).gap(6.0).flex(1.0);
                body.push(Element::row(vec![code, ways]).gap(20.0));
            }
        }
        let parts = handoff.parts;
        body.extend([
            Text::new(t!("desktop-handoff-what"), palette).muted().render(),
            switch(&t!("desktop-handoff-profile"), Some(state.profile.shown()), "handoff-part:profile", parts.profile),
            switch(&t!("desktop-playlists"), Some(&t!("desktop-profile-playlists", count = collection.playlists.len())), "handoff-part:playlists", parts.playlists),
            switch(&t!("common-liked-songs"), Some(&songs(collection.liked.len())), "handoff-part:liked", parts.liked),
            switch(&t!("desktop-handoff-library"), Some(&t!("desktop-handoff-library-detail")), "handoff-part:library", parts.library),
            switch(&t!("common-settings"), Some(&t!("desktop-handoff-settings-detail")), "handoff-part:settings", parts.settings),
            switch(&t!("common-equalizer"), None, "handoff-part:equalizer", parts.equalizer),
            switch(&t!("desktop-handoff-history"), Some(&t!("desktop-handoff-history-detail")), "handoff-part:history", parts.history),
        ]);
    } else {
        let field = TextField::new("handoff-address", &handoff.address, "192.168.1.20:48213/…", palette).size(ControlSize::Regular).render().flex(1.0);
        let look = Button::new("handoff-fetch", t!("desktop-handoff-look"), palette).variant(ButtonVariant::Outline).size(ControlSize::Regular).enabled(!handoff.fetching).render();
        body.extend([
            Text::new(t!("desktop-handoff-receive-detail"), palette).muted().render(),
            Element::row(vec![field, look]).gap(8.0),
        ]);
        if handoff.fetching {
            body.push(Element::row(vec![Spinner::new("handoff-fetching", palette.muted).size(16.0).render(), Text::new(t!("desktop-handoff-fetching"), palette).muted().render()]).gap(8.0));
        }
        match &handoff.fetched {
            Some(Err(message)) => body.push(Alert::new(t!("desktop-handoff"), message, palette).id("handoff-error").compact(true).render()),
            Some(Ok(bundle)) => {
                let parts = handoff.parts;
                let mut rows = vec![Text::new(t!("desktop-handoff-came"), palette).muted().render()];
                if let Some(profile) = &bundle.profile { rows.push(switch(&t!("desktop-handoff-profile"), Some(profile.shown()), "handoff-part:profile", parts.profile)); }
                if !bundle.playlists.is_empty() { rows.push(switch(&t!("desktop-playlists"), Some(&t!("desktop-profile-playlists", count = bundle.playlists.len())), "handoff-part:playlists", parts.playlists)); }
                if !bundle.liked.is_empty() { rows.push(switch(&t!("common-liked-songs"), Some(&songs(bundle.liked.len())), "handoff-part:liked", parts.liked)); }
                if !bundle.kept.is_empty() { rows.push(switch(&t!("desktop-handoff-library"), Some(&t!("desktop-handoff-library-detail")), "handoff-part:library", parts.library)); }
                if bundle.settings.is_some() { rows.push(switch(&t!("common-settings"), Some(&t!("desktop-handoff-settings-detail")), "handoff-part:settings", parts.settings)); }
                if bundle.equalizer.is_some() { rows.push(switch(&t!("common-equalizer"), None, "handoff-part:equalizer", parts.equalizer)); }
                if !bundle.heard.is_empty() { rows.push(switch(&t!("desktop-handoff-history"), Some(&songs(bundle.heard.len())), "handoff-part:history", parts.history)); }
                let mode = |id: &str, label: &str, chosen: bool| Tab::new(id, label, 0.0).active(chosen);
                rows.push(segments("handoff-mode", form, vec![mode("handoff-mode:add", &t!("desktop-handoff-add"), !handoff.replace), mode("handoff-mode:replace", &t!("desktop-handoff-replace"), handoff.replace)]));
                rows.push(Text::new(if handoff.replace { t!("desktop-handoff-replace-detail") } else { t!("desktop-handoff-add-detail") }, palette).role(TextRole::Caption).muted().render());
                body.extend(rows);
                confirm = Some(Button::new("handoff-take", t!("desktop-handoff-take"), palette).size(ControlSize::Regular).round().render());
            }
            None => {}
        }
    }
    if confirm.is_none() {
        // Sending, the dialog only closes: the other device does the rest.
        cancel = Button::new("handoff-close", t!("common-close"), palette).size(ControlSize::Regular).round().render();
    }
    let footer = match confirm {
        Some(confirm) => dialog_footer(cancel, confirm),
        None => ButtonGroup::new(vec![cancel]).align_end().render(),
    };
    if state.mobile {
        return limn::BottomSheet::new(window_tree, true, t!("desktop-handoff"), t!("desktop-handoff-detail"), "handoff-close", palette)
            .max_height(640.0)
            .body(Element::column(body).gap(10.0))
            .footer(footer)
            .render();
    }
    Modal::new(window_tree, true, t!("desktop-handoff"), t!("desktop-handoff-detail"), "handoff-close", palette)
        .width(dialog)
        .body(ScrollArea::new("handoff", Element::column(body).gap(10.0)).render())
        .footer(footer)
        .return_focus("account")
        .render()
}

/// The side of the code a phone's camera reads.
const HANDOFF_CODE: f32 = 168.0;

/// Changing the account's name.
fn profile_dialog(state: &State, window_tree: Element, width: f32) -> Element {
    let Some(draft) = &state.profile_draft else { return window_tree };
    let palette = dialog_palette();
    let name = TextField::new("profile-name", draft, "Surco", palette).size(ControlSize::Regular).render();
    let cancel = Button::new("profile-cancel", t!("common-cancel"), palette).variant(ButtonVariant::Ghost).size(ControlSize::Regular).round().render();
    let save = Button::new("profile-save", t!("desktop-save"), palette).size(ControlSize::Regular).round().render();
    // As Spotify's profile details: the face beside the name, with the way to change it.
    let mut choices = vec![Button::new("profile-picture", t!("desktop-choose-photo"), palette).variant(ButtonVariant::Outline).size(ControlSize::Compact).round().render()];
    if state.profile.picture.is_some() {
        choices.push(Button::new("profile-picture-remove", t!("desktop-remove-photo"), palette).variant(ButtonVariant::Ghost).size(ControlSize::Compact).round().render());
    }
    let face = Element::column(vec![profile_face(state, DIALOG_FACE, palette).centered_x(), Element::column(choices).gap(4.0)]).gap(8.0);
    let fields = Element::column(FormField::new(t!("desktop-profile-name"), name, palette).sizing(DIALOG_FIELD).rows()).gap(8.0).flex(1.0);
    Modal::new(window_tree, true, t!("desktop-profile-details"), t!("desktop-profile-details-detail"), "profile-cancel", palette)
        .width(Modal::fit_width(width, 480.0).max(240.0))
        .body(Element::row(vec![face, fields]).gap(16.0))
        .footer(dialog_footer(cancel, save))
        .initial_focus("profile-name")
        .return_focus("profile-edit")
        .render()
}

/// Bringing a list in from another app: its link there.
fn import_dialog(state: &State, window_tree: Element, width: f32, palette: ComponentPalette) -> Element {
    let Some(link) = &state.importing else { return window_tree };
    if state.mobile {
        let field = TextField::new(
            "import-link",
            link,
            t!("desktop-import-link-placeholder"),
            palette,
        )
        .size(ControlSize::Spacious)
        .render()
        .size(None, Some(44.0));
        let mut fields = FormField::new(t!("desktop-import-link"), field, palette).sizing(DIALOG_FIELD).rows();
        if cfg!(any(target_os = "ios", test)) {
            fields.push(Button::new("ios-paste:import-link", t!("desktop-paste-link"), palette)
                .variant(ButtonVariant::Secondary).sizing(limn::ButtonSizing { height: 44.0, ..ControlSize::Spacious.button_sizing() }).round().render());
        }
        let cancel = Button::new("import-cancel", t!("common-cancel"), palette)
            .variant(ButtonVariant::Outline)
            .sizing(limn::ButtonSizing { height: 44.0, ..ControlSize::Spacious.button_sizing() }).round()
            .render()
            .size(None, Some(44.0));
        let go = Button::new("import-go", t!("common-import"), palette)
            .sizing(limn::ButtonSizing { height: 44.0, ..ControlSize::Spacious.button_sizing() }).round()
            .render()
            .size(None, Some(44.0));
        return limn::BottomSheet::new(
            window_tree,
            true,
            t!("desktop-import-music"),
            if state.import_unknown { t!("desktop-import-unknown") } else { "Spotify · Apple Music · YouTube Music".into() },
            "import-cancel",
            palette,
        )
        .max_height(560.0)
        .body(Element::column(fields).gap(8.0))
        .footer(dialog_footer(cancel, go))
        .initial_focus("import-link")
        .render();
    }
    let palette = dialog_palette();
    let field = TextField::new("import-link", link, "https://open.spotify.com/playlist/…", palette).size(ControlSize::Regular).render();
    let about = if state.import_unknown {
        t!("desktop-import-unknown-detail")
    } else {
        t!("desktop-import-hint")
    };
    let cancel = Button::new("import-cancel", t!("common-cancel"), palette).variant(ButtonVariant::Ghost).size(ControlSize::Regular).round().render();
    let go = Button::new("import-go", t!("common-import"), palette).size(ControlSize::Regular).round().render();
    let out = Text::new(t!("desktop-import-export-hint"), palette).role(TextRole::Caption).muted().render();
    let mut fields: Vec<Element> = FormField::new(t!("desktop-import-list-link"), field, palette).hint(about).sizing(DIALOG_FIELD).rows();
    fields.push(out);
    Modal::new(window_tree, true, t!("desktop-import-title"), t!("desktop-import-detail"), "import-cancel", palette)
        .width(Modal::fit_width(width, 460.0).max(240.0))
        .body(Element::column(fields).gap(8.0))
        .footer(dialog_footer(cancel, go))
        .initial_focus("import-link")
        .render()
}

/// The songs of a list brought in that the catalog does not have, to be looked for by hand.
fn missed_dialog(state: &State, window_tree: Element, width: f32, palette: ComponentPalette) -> Element {
    let Some((_, missed)) = state.imported.as_ref().filter(|_| state.missed_open) else { return window_tree };
    if state.mobile {
        let rows = missed
            .iter()
            .map(|song| Text::new(song, palette).font_size(14.0).render())
            .collect();
        let copy = Button::new("import-missed-copy", t!("desktop-copy-list"), palette)
            .variant(ButtonVariant::Outline)
            .icon(Icon::Copy)
            .render()
            .size(None, Some(44.0));
        let close = Button::new("import-missed-close", t!("common-close"), palette)
            .render()
            .size(None, Some(44.0));
        return limn::BottomSheet::new(
            window_tree,
            true,
            t!("desktop-missed-title-short"),
            t!("desktop-missed-detail"),
            "import-missed-close",
            palette,
        )
        .max_height(600.0)
        .body(Element::column(rows).gap(12.0))
        .footer(dialog_footer(copy, close))
        .render();
    }
    let palette = dialog_palette();
    let rows = missed.iter().map(|song| Text::new(song, palette).truncate().render()).collect();
    let copy = Button::new("import-missed-copy", t!("desktop-copy-list"), palette).variant(ButtonVariant::Ghost).icon(Icon::Copy).size(ControlSize::Regular).round().render();
    let close = Button::new("import-missed-close", t!("common-close"), palette).size(ControlSize::Regular).round().render();
    Modal::new(window_tree, true, t!("desktop-missed-title"), t!("desktop-missed-detail"), "import-missed-close", palette)
        .width(Modal::fit_width(width, 460.0).max(240.0))
        .body(Element::column(rows).gap(6.0))
        .footer(dialog_footer(copy, close))
        .render()
}

/// The picture on the library's offer to bring music in, drawn for it (`assets/import.svg`)
/// and read once.
fn import_picture() -> Option<limn::RasterImage> {
    use base64::Engine as _;
    use sha2::Digest as _;
    const BYTES: &[u8] = include_bytes!("../assets/import.png");
    // Read once; its description is said in the language of the moment.
    static PICTURE: std::sync::OnceLock<(String, std::sync::Arc<str>)> = std::sync::OnceLock::new();
    let read = || {
        let digest: String = sha2::Sha256::digest(BYTES).iter().map(|byte| format!("{byte:02x}")).collect();
        (digest, base64::engine::general_purpose::STANDARD.encode(BYTES).into())
    };
    let (digest, encoded) = PICTURE.get_or_init(read);
    limn::RasterImage::new(digest.clone(), limn::RasterImageFormat::Png, BYTES.len() as u32, encoded.clone(), t!("desktop-import-picture"))
}

/// The library's offer to bring music in from another app, on a desktop: one of the panel's
/// own cards, as Spotify's library offers a first playlist, with a white capsule to take it
/// and a cross to put it away.
fn library_offer(width: f32, palette: ComponentPalette) -> Element {
    let raised = ComponentPalette { surface: CHOSEN, ..palette };
    let title = Text::new(t!("desktop-import-card-title"), raised).font_size(16.0).render().font_family(FontFamily::SansBold);
    let away = header_button("import-dismiss", Icon::Close, TooltipSpec::new(t!("desktop-import-dismiss")), raised).render();
    let about = Text::new(t!("desktop-import-card-detail"), raised).font_size(14.0).render();
    let white = ComponentPalette { surface: palette.text, text: SCHEME.canvas, hover: ROUND_HOVER, selected: ROUND_HELD, ..palette };
    let go = Button::new("import-open", t!("common-import"), white).variant(ButtonVariant::Subtle).size(ControlSize::Compact).round().render();
    panel_card(vec![
        Element::row(vec![title.flex(1.0), away]).gap(8.0),
        about,
        Element::spacer().size(None, Some(4.0)),
        Element::row(vec![go]),
    ], width)
}

/// The library's offer to bring music in from another app, until it is used or put away,
/// after the Spotify kit's promotion cards: the picture edge to edge over a lifted surface,
/// a bold title, a quiet line and white capsules.
fn import_card(width: f32, palette: ComponentPalette, native: bool) -> Element {
    let raised = ComponentPalette { surface: PROMO, ..palette };
    let close = mobile::icon("import-dismiss", Icon::Close, &t!("desktop-import-dismiss"), palette).background(Color { a: 140, ..CANVAS }).radius(22.0);
    let away = Element::row(vec![Element::spacer().flex(1.0), close]).padding(4.0);
    let top = match import_picture() {
        Some(picture) => Element::stack(vec![
            Element::raster_image(picture.cover()).size(Some(width), Some(width * 130.0 / 460.0)), away]),
        None => away,
    };
    let title = Text::new(t!("desktop-import-card-title"), raised).font_size(18.0).render().font_family(FontFamily::SansBold);
    let about = Text::new(t!("desktop-import-card-detail"), raised).font_size(14.0).muted().render();
    let go = Button::new("import-open", t!("common-import"), raised)
        .sizing(limn::ButtonSizing { height: 44.0, ..ControlSize::Spacious.button_sizing() }).round().render();
    let mut actions = vec![go];
    if native && cfg!(any(target_os = "ios", test)) {
        // UIKit draws and handles UIPasteControl in this slot. Pasting prepares
        // the import form; fetching a playlist still requires its explicit submit.
        actions.push(Element::panel(Element::spacer()).size(Some(112.0), Some(44.0))
            .accessible(limn::Access::new(limn::Role::Group).id("ios-paste:import-card").name(t!("desktop-paste-playlist-link"))));
    }
    let actions = Element::row(actions).gap(8.0);
    let copy = Element::column(vec![title, about, Element::spacer().size(None, Some(6.0)), actions]).gap(6.0).padding(16.0);
    Element::panel(Element::column(vec![top, copy])).background(PROMO).radius(8.0).clip_children()
}

/// The offer that greets a first start, as Spotify's: a picture of the user's covers around
/// the way in, the offer on a deep violet, a lavender button that opens the import, and a way
/// out under the card.
const WELCOME_RADIUS: f32 = 20.0;

/// The greeting's violet to peach wash, with its top corners already rounded: limn paints a
/// gradient square, and a picture's own clear corners round it on any backdrop. Made once.
fn welcome_wash() -> Option<limn::RasterImage> {
    use base64::Engine as _;
    use sha2::Digest as _;
    static WASH: std::sync::OnceLock<Option<(String, usize, std::sync::Arc<str>)>> = std::sync::OnceLock::new();
    let made = WASH.get_or_init(|| {
        // Drawn for the widest card at three pixels a point; a narrower one rounds a little less.
        let (width, height, radius) = (1140u32, 866u32, WELCOME_RADIUS * 3.0);
        let stops = [(86.0, 76.0, 236.0), (206.0, 170.0, 240.0), (252.0, 196.0, 128.0)];
        let wash = image::RgbaImage::from_fn(width, height, |x, y| {
            let along = y as f32 / (height - 1) as f32 * 2.0;
            let (from, to) = if along < 1.0 { (stops[0], stops[1]) } else { (stops[1], stops[2]) };
            let at = along.fract().max(if along >= 2.0 { 1.0 } else { 0.0 });
            let mix = |a: f32, b: f32| (a + (b - a) * at) as u8;
            // Outside the top corners' arcs, clear; across their edge, a pixel of fade.
            let (cx, cy) = ((x as f32 + 0.5).clamp(radius, width as f32 - radius), (y as f32 + 0.5).max(radius));
            let outside = ((x as f32 + 0.5 - cx).hypot(y as f32 + 0.5 - cy) - radius).clamp(0.0, 1.0);
            image::Rgba([mix(from.0, to.0), mix(from.1, to.1), mix(from.2, to.2), (255.0 * (1.0 - outside)) as u8])
        });
        let mut png = Vec::new();
        wash.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).ok()?;
        let digest: String = sha2::Sha256::digest(&png).iter().map(|byte| format!("{byte:02x}")).collect();
        Some((digest, png.len(), base64::engine::general_purpose::STANDARD.encode(&png).into()))
    });
    let (digest, length, encoded) = made.as_ref()?;
    limn::RasterImage::new(digest.clone(), limn::RasterImageFormat::Png, *length as u32, encoded.clone(), t!("common-welcome-title"))
}

pub(crate) fn welcome_dialog(state: &State, window_tree: Element, width: f32) -> Element {
    if !state.welcome {
        return window_tree;
    }
    const VIOLET: Color = Color::rgb(36, 8, 138);
    const LAVENDER: Color = Color::rgb(222, 219, 255);
    let side = (width - 48.0).clamp(240.0, 380.0);
    let art = side * 0.76;
    let mut layers = vec![match welcome_wash() {
        Some(wash) => Element::raster_image(wash).size(Some(side), Some(art)),
        None => Element::spacer().size(Some(side), Some(art)).background(Color::rgb(86, 76, 236)),
    }];
    // Covers of what the user already plays float around it, wherever they are in.
    let own = state.collection.heard.iter().chain(&state.collection.liked).chain(state.collection.playlists.iter().flat_map(|playlist| &playlist.tracks));
    let mut covers: Vec<&String> = Vec::new();
    for url in own.filter_map(|track| track.artwork_url.as_ref()).filter(|url| state.artwork.contains_key(*url)) {
        if !covers.contains(&url) && covers.len() < 6 { covers.push(url); }
    }
    // A stack stretches each of its layers to its box: each piece keeps its own size at the
    // box's corner, and is moved from there.
    let placed = |piece: Element, x: f32, y: f32| Element::column(vec![Element::row(vec![piece, Element::spacer().flex(1.0)]), Element::spacer().flex(1.0)])
        .translate(limn::Point::new(x, y));
    for (url, (x, y, size)) in covers.into_iter().zip([(0.08, 0.06, 0.22), (0.72, 0.04, 0.19), (0.03, 0.55, 0.18), (0.80, 0.52, 0.16), (0.26, 0.74, 0.19), (0.62, 0.77, 0.15)]) {
        let picture = cover(state, &Some(url.clone()), side * size, SCHEME.palette()).radius(10.0);
        layers.push(placed(picture, side * x, art * y));
    }
    if let Some(picture) = import_picture() {
        let (wide, high) = (side * 0.62, side * 0.62 * 260.0 / 920.0);
        layers.push(placed(Element::raster_image(picture.cover()).size(Some(wide), Some(high)).radius(12.0), (side - wide) / 2.0, (art - high) / 2.0));
    }
    let on_violet = ComponentPalette { surface: VIOLET, text: Color::WHITE, muted: Color::rgba(255, 255, 255, 220), ..SCHEME.palette() };
    let title = Text::new(t!("common-welcome-title"), on_violet).font_size(22.0).line_height(28.0).render().font_family(FontFamily::SansBold).center_content();
    let about = Text::new(t!("common-welcome-detail"), on_violet).font_size(15.0).line_height(21.0).muted().render().center_content();
    let label = Element::text(t!("common-welcome-import")).font_family(FontFamily::SansBold).font_size(16.0).foreground(VIOLET).center_content();
    let go = Element::clickable("welcome-import", Element::panel(label.centered()).background(LAVENDER).radius(26.0).size(Some(side * 0.62), Some(52.0)))
        .accessible(limn::Access::new(limn::Role::Button).name(t!("common-welcome-import")));
    let copy = Element::column(vec![title, about, Element::spacer().size(None, Some(8.0)), go.centered_x()]).gap(12.0).padding(24.0);
    // Round all round: the picture keeps the card's top corners, which it would otherwise cover.
    let art = Element::stack(layers).size(Some(side), Some(art)).corner_radii(limn::CornerRadii::top(WELCOME_RADIUS)).clip_children();
    let card = Element::panel(Element::column(vec![art, copy]))
        .background(VIOLET).radius(WELCOME_RADIUS).clip_children().size(Some(side), None);
    let close = Element::clickable("welcome-close", Element::text(t!("common-close")).font_family(FontFamily::SansBold).font_size(16.0).foreground(Color::WHITE).center_content().centered())
        .accessible(limn::Access::new(limn::Role::Button).name(t!("common-close"))).size(Some(side), Some(48.0));
    // As wide as the card, so the window centres it.
    let dialog = Element::column(vec![card, close]).gap(12.0).size(Some(side), None)
        .accessible(limn::Access::new(limn::Role::Dialog).id("welcome:dialog").name(t!("common-welcome-title")));
    Element::popover(window_tree, dialog, "welcome-close", PopoverPlacement::Center).popover_scrim(Color::rgba(0, 0, 0, 200))
}

/// What cannot be undone asks before it is done.
fn confirm_dialog(state: &State, window_tree: Element, palette: ComponentPalette) -> Element {
    let (title, about, yes) = match &state.confirm {
        Some(Confirm::DeletePlaylist(id)) => {
            let name = state.collection.playlist(*id).map_or_else(|| t!("desktop-this-playlist").into_owned(), |playlist| playlist.name.clone());
            (t!("desktop-delete-playlist-title", name = name).into_owned(), t!("desktop-delete-playlist-detail"), t!("common-delete"))
        }
        Some(Confirm::ClearDownloads) => (t!("desktop-clear-downloads-title").into_owned(), t!("desktop-clear-downloads-detail"), t!("common-delete")),
        Some(Confirm::ClearQueue) => (t!("desktop-clear-queue-title").into_owned(), t!("desktop-clear-queue-detail"), t!("common-clear")),
        Some(Confirm::ClearRecent) => (t!("desktop-clear-recent-title").into_owned(), t!("desktop-clear-recent-detail"), t!("common-clear")),
        None => (String::new(), "".into(), "".into()),
    };
    if state.mobile {
        let cancel = Button::new("confirm-cancel", t!("common-cancel"), palette)
            .variant(ButtonVariant::Outline)
            .render()
            .size(None, Some(44.0));
        let confirm = Button::new("confirm-yes", yes, palette)
            .render()
            .size(None, Some(44.0));
        return limn::BottomSheet::new(
            window_tree,
            state.confirm.is_some(),
            t!("desktop-confirm"),
            format!("{title}\n\n{about}"),
            "confirm-cancel",
            palette,
        )
        .max_height(480.0)
        .footer(dialog_footer(cancel, confirm))
        .initial_focus("confirm-cancel")
        .backdrop_dismiss(false)
        .render();
    }
    AlertDialog::new(window_tree, state.confirm.is_some(), title, about, "confirm-yes", "confirm-cancel", dialog_palette())
        .confirm_label(yes)
        .cancel_label(t!("common-cancel"))
        .render()
}

/// The card of a later version: what stands, how far taking it in has got, and what the
/// user can do about it. While it comes down, a bar under the words fills with it.
fn update_card(state: &State, release: &Release, width: f32, palette: ComponentPalette) -> Element {
    let button = |id: &str, label: &str| Button::new(id, label, palette).size(ControlSize::Regular).render().centered_y();
    let version = &release.version;
    let (title, about, action) = match &state.updating {
        Updating::Idle => {
            let about = t!("desktop-update-current", version = version.as_str(), current = env!("CARGO_PKG_VERSION")).into_owned();
            (t!("desktop-update-available").into_owned(), about, Some(button("update-get", &if release.program.is_some() { t!("desktop-update") } else { t!("desktop-download") })))
        }
        // The last of it is checked and put in place: a moment with nothing left to count.
        Updating::Fetching(done) if *done >= 1.0 => (t!("desktop-update-installing", version = version.as_str()).into_owned(), t!("desktop-update-moment").into_owned(), None),
        Updating::Fetching(_) => (t!("desktop-update-downloading", version = version.as_str()).into_owned(), t!("desktop-update-keep-listening").into_owned(), None),
        Updating::Ready => (t!("desktop-update-ready", version = version.as_str()).into_owned(), t!("desktop-update-restart-detail").into_owned(), Some(button("update-restart", &t!("desktop-restart-now")))),
        Updating::Failed(why) => (t!("desktop-update-failed").into_owned(), why.clone(), Some(button("update-get", &t!("common-retry")))),
    };
    let copy = Element::column(vec![Text::new(title, palette).font_size(15.0).render(), Text::new(about, palette).muted().render()]).gap(2.0).flex(1.0);
    let mut head = vec![copy];
    let mut parts = Vec::new();
    if let Updating::Fetching(done) = &state.updating {
        let percent = Element::text(format!("{:.0} %", done * 100.0)).font_family(FontFamily::Monospace).font_size(12.0).foreground(palette.muted);
        head.push(percent.centered_y());
        parts.push(Progress::new(*done, palette).height(6.0).render());
    }
    head.extend(action);
    parts.insert(0, Element::row(head).gap(12.0));
    parts.extend(release_notes(state, &release.notes, (width - 32.0).min(NOTES_WIDTH), palette));
    Element::panel(Element::column(parts).gap(12.0)).padding(16.0).background(palette.surface).radius(RadiusRole::Panel.px())
}

/// The widest what a release says of itself is set: a line longer than this is hard to read.
const NOTES_WIDTH: f32 = 600.0;

/// What a release says of itself: its text as it was written, and its pictures as wide
/// as the text, each in a room of a screen's proportions. A video opens where it is kept.
// shortcut: a video is a card that opens it in the browser, as the clips' decoder reads only
// YouTube's fragmented MP4; play it here once that decoder reads a whole MP4 too.
fn release_notes(state: &State, notes: &[Note], width: f32, palette: ComponentPalette) -> Vec<Element> {
    let radius = limn::tokens().radius.control;
    let part = |(index, note): (usize, &Note)| match note {
        Note::Video(address) => {
            let label = Element::row(vec![Element::icon(Icon::PlayFill).foreground(palette.text).size(Some(18.0), Some(18.0)).centered_y(),
                Text::new(t!("common-watch-video"), palette).render().centered_y()]).gap(8.0).centered();
            let card = Element::panel(label).background(palette.selected).radius(radius).size(Some(width), Some(width * 9.0 / 16.0));
            Element::clickable(format!("{RELEASE_LINK}video:{index}:{address}"), card)
                .accessible(limn::Access::new(limn::Role::Link).name(t!("common-watch-video")))
        }
        Note::Text(text) => Markdown::new(text, palette).width(width).link_action_prefix(RELEASE_LINK).render().element,
        Note::Picture(address) => match state.artwork.get(address) {
            Some(picture) => Element::raster_image(picture.clone().full_resolution()).size(Some(width), Some(width * 9.0 / 16.0)).radius(radius),
            None => Element::panel(Element::spacer()).background(palette.selected).radius(radius).size(Some(width), Some(width * 9.0 / 16.0)),
        },
    };
    notes.iter().enumerate().map(part).collect()
}

/// What each version brought, the latest first, as its release tells it; the one that runs
/// says so.
fn patches_page(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let title = Element::column(vec![
        Text::new(t!("common-patches"), palette).font_size(PAGE_TITLE).render(),
        Text::new(t!("common-patches-detail"), palette).muted().render(),
    ]).gap(4.0);
    let patches = match &state.patches {
        None => return Element::column(vec![title, loading("patches", palette)]).gap(16.0).padding(12.0).flex(1.0),
        Some(Err(why)) => {
            let failed = Empty::new(t!("common-patches-failed"), why, palette).icon(Icon::WarningTriangle)
                .action("patches-retry", t!("common-retry"), ButtonVariant::Secondary).render();
            return Element::column(vec![title, centered(failed)]).gap(16.0).padding(12.0).flex(1.0);
        }
        Some(Ok(patches)) => patches,
    };
    let raised = ComponentPalette { surface: CHOSEN, ..palette };
    let notes = (width - 24.0 - 32.0).min(NOTES_WIDTH);
    let mut sections = vec![title];
    for patch in patches {
        let mut about = match patch.date.splitn(3, '-').collect::<Vec<_>>()[..] {
            [year, month, day] => t!("common-patches-date", day = day.trim_start_matches('0'), month = month.trim_start_matches('0'), year = year).into_owned(),
            _ => String::new(),
        };
        if patch.version == env!("CARGO_PKG_VERSION") {
            about = [about, t!("common-patches-current").into_owned()].into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" · ");
        }
        let mut parts = vec![Element::column(vec![
            Text::new(format!("Surco {}", patch.version), raised).font_size(20.0).render(),
            Text::new(about, raised).muted().render(),
        ]).gap(2.0)];
        parts.extend(release_notes(state, &patch.notes, notes, raised));
        sections.push(Element::panel(Element::column(parts).gap(12.0)).padding(16.0).background(raised.surface).radius(RadiusRole::Panel.px()));
    }
    page_scroll("patches", Element::column(sections).gap(16.0).padding(12.0))
}

/// A version that may not keep running stands in front of everything until the later one
/// is taken in.
/// How wide the notice of a version that has to be taken is, and how tall the room in it
/// for what the version says of itself, which scrolls.
const UPDATE_DIALOG: f32 = 520.0;
const UPDATE_NOTES_ROOM: f32 = 240.0;

fn update_dialog(state: &State, window_tree: Element, palette: ComponentPalette) -> Element {
    let Some(release) = state.update.as_ref().filter(|release| release.mandatory) else { return window_tree };
    let (about, action) = match &state.updating {
        Updating::Idle if release.program.is_some() => (t!("desktop-update-mandatory-install", version = release.version.as_str()), ("update-get", t!("desktop-update-now"))),
        Updating::Idle => (t!("desktop-update-mandatory-download", version = release.version.as_str()), ("update-get", t!("desktop-download"))),
        Updating::Fetching(done) => (t!("desktop-update-downloading-percent", version = release.version.as_str(), percent = format!("{:.0}", done * 100.0)), ("update-wait", t!("desktop-downloading"))),
        Updating::Ready => (t!("desktop-update-installed", version = release.version.as_str()), ("update-restart", t!("desktop-restart-now"))),
        Updating::Failed(why) => (t!("desktop-update-failed-why", why = why.as_str()), ("update-get", t!("common-retry"))),
    };
    let mut body = Vec::new();
    if let Updating::Fetching(done) = &state.updating {
        body.push(Progress::new(*done, palette).height(6.0).render());
    }
    let notes = release_notes(state, &release.notes, UPDATE_DIALOG - 48.0, palette);
    if !notes.is_empty() {
        body.push(ScrollArea::new("update-notes", Element::column(notes).gap(12.0)).render().size(None, Some(UPDATE_NOTES_ROOM)));
    }
    // No way around it: the other button leads to the release's own page.
    let dialog = AlertDialog::new(window_tree, true, t!("desktop-update-mandatory"), about, action.0, "update-page", dialog_palette()).confirm_label(action.1).cancel_label(t!("desktop-see-release")).width(UPDATE_DIALOG);
    if body.is_empty() { dialog } else { dialog.body(Element::column(body).gap(12.0)) }.render()
}

/// A release in the feed of what is new: a row around its cover, larger than a track's.
const NEWS_COVER: f32 = 56.0;
const NEWS_ROW: ListRowSizing = ListRowSizing { height: Some(72.0), two_line_height: Some(72.0), leading_width: NEWS_COVER, ..TRACK_ROW };

/// What the artists followed have released, what is still to be looked at first, under
/// the notice of a later version of the app when there is one.
fn news_page(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let mut sections = vec![Element::column(vec![
        Text::new(t!("desktop-news"), palette).font_size(PAGE_TITLE).render(),
        Text::new(t!("desktop-news-detail"), palette).muted().render(),
    ])
    .gap(4.0)];
    if let Some(release) = &state.update {
        let raised = ComponentPalette { surface: CHOSEN, ..palette };
        sections.push(update_card(state, release, width - 24.0, raised));
    }
    sections.push(Element::row(vec![Button::new("patches", t!("common-patches"), palette).variant(ButtonVariant::Secondary).size(ControlSize::Compact).render()]));
    let row = |(index, (release, _)): (usize, &(AlbumRef, bool))| {
        let about = [release.artist.as_deref().unwrap_or_default(), release.year.as_str()].into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" · ");
        ListRow::new(format!("news:{index}"), &release.title, palette)
            .sizing(NEWS_ROW)
            .leading(middle(cover(state, &release.artwork_url, NEWS_COVER, palette)))
            .subtitle(about)
            .title_fade(true)
            .radius(RadiusRole::Card)
            .render()
    };
    let news = &state.collection.news;
    let fresh = state.news_fresh.min(news.len());
    if fresh > 0 {
        sections.extend([heading(&t!("desktop-news-new"), palette), Element::column(news[..fresh].iter().enumerate().map(row).collect()).gap(2.0)]);
    }
    if news.len() > fresh {
        let earlier = news.iter().enumerate().skip(fresh).map(row).collect();
        sections.extend([heading(&t!("desktop-news-earlier"), palette), Element::column(earlier).gap(2.0)]);
    }
    if news.is_empty() && state.update.is_none() {
        let empty = Empty::new(t!("desktop-news-empty"), t!("desktop-news-empty-detail"), palette).icon(Icon::Bell).render();
        return Element::column(vec![sections.remove(0), centered(empty)]).gap(16.0).padding(12.0).flex(1.0);
    }
    page_scroll("news", Element::column(sections).gap(16.0).padding(12.0))
}

/// The narrowest a tile of the library gets, and the width of the menu of its orders and views.
const LIBRARY_GRID_TILE: f32 = 112.0;
/// What the library's row of tools keeps for the cross of its search and the button of its order.
/// The share of its row the library's search has as it starts to open: about a magnifier's width.
const LIBRARY_SEARCH_SHUT: f32 = 0.15;
const LIBRARY_MENU_WIDTH: f32 = 220.0;

/// A button of a header: quiet, small, with its hint under it.
fn header_button(id: &str, icon: Icon, hint: TooltipSpec, palette: ComponentPalette) -> IconButton {
    IconButton::new(id, icon, palette).round().variant(ButtonVariant::Ghost).size(ControlSize::Dense).tooltip_with(hint.placement(TooltipPlacement::Below))
}

/// A track as the card that is handed around: its sleeve plays it, and while it plays the
/// record is out and turning.
fn media_card(state: &State, id: &str, track: &Track, palette: ComponentPalette) -> Element {
    let playing = state.player.playing && state.current().is_some_and(|current| current.id == track.id);
    let mut card = MediaCard::new(id, &track.title, palette)
        .artist(&track.artist)
        .playing(playing)
        .mark(Element::icon(Icon::Audio).foreground(palette.muted));
    if let Some(url) = &track.artwork_url {
        if let Some(image) = state.artwork.get(url) {
            card = card.artwork(image.clone());
        }
        if let Some(tint) = state.tints.get(url) {
            card = card.tint(*tint);
        }
    }
    card.render()
}

/// Sharing a track: its card, as whoever opens the link will see it, over the link itself.
fn share_dialog(state: &State, window_tree: Element, width: f32, palette: ComponentPalette) -> Element {
    if state.mobile {
        let Some(track) = &state.sharing else {
            return window_tree;
        };
        let playing = state.player.playing
            && state
                .current()
                .is_some_and(|current| current.id == track.id);
        let picture = Element::clickable(
            "share-card",
            cover(state, &track.artwork_url, 160.0, palette),
        )
        .accessible(limn::Access::new(limn::Role::Button).name(format!(
            "{} {}",
            if playing { t!("common-pause") } else { t!("common-play") },
            track.title
        )))
        .centered_x();
        let copy = Button::new(
            "share-copy",
            if state.copied {
                t!("common-link-copied")
            } else {
                t!("desktop-copy-link")
            },
            palette,
        )
        .icon(if state.copied {
            Icon::Check
        } else {
            Icon::Copy
        })
        .render()
        .size(None, Some(44.0));
        let body = Element::column(vec![
            picture,
            Text::new(&track.title, palette).font_size(20.0).render(),
            Text::new(&track.artist, palette).muted().render(),
            Text::new(share_link(track), palette)
                .mono()
                .muted()
                .truncate()
                .render(),
        ])
        .gap(12.0);
        return limn::BottomSheet::new(
            window_tree,
            true,
            t!("common-share-song"),
            t!("desktop-share-detail"),
            "share-close",
            palette,
        )
        .max_height(640.0)
        .body(body)
        .footer(copy)
        .render();
    }
    let (card, link) = match &state.sharing {
        Some(track) => (media_card(state, "share-card", track, palette), share_link(track)),
        None => (Element::spacer(), String::new()),
    };
    let copy = if state.copied { Button::new("share-copy", t!("common-link-copied"), palette).icon(Icon::Check) } else { Button::new("share-copy", t!("desktop-copy-link"), palette).icon(Icon::Copy) };
    Modal::new(window_tree, state.sharing.is_some(), t!("common-share-song"), t!("desktop-share-detail"), "share-close", dialog_palette())
        .width(Modal::fit_width(width, 420.0))
        .body(Element::column(vec![card.centered_x(), Text::new(link, palette).mono().muted().truncate().render()]).gap(12.0))
        .footer(ButtonGroup::new(vec![copy.size(ControlSize::Regular).render()]).align_end().render())
        .render()
}

/// The side of the chip on the bell's corner.
const BELL_CHIP: f32 = 14.0;

/// Across the top, as Spotify's: the way back, a round home and the search in the middle of
/// the window, and what is new and the settings at the end. limn's own bar.
fn top_bar(state: &State, width: f32, palette: ComponentPalette) -> Element {
    // The window's own buttons keep the end the system puts them at.
    let (left, right) = state.caption;
    // A rule tells the window's buttons apart from the app's.
    let rule = |width: f32| (width > 0.0).then(|| Separator::vertical(palette).length(20.0).margin(2.0).render());
    let mut leading: Vec<Element> = rule(left).into_iter().collect();
    // Back and forward as Spotify's: two chevrons, muted where they lead nowhere.
    leading.push(header_button("back", Icon::Custom(glyph::CHEVRON_LEFT), TooltipSpec::new(t!("common-back")), palette).enabled(state.can_go_back()).render());
    leading.push(header_button("forward", Icon::Custom(glyph::CHEVRON_RIGHT), TooltipSpec::new(t!("desktop-forward")), palette).enabled(state.can_go_forward()).render());
    // The bell is full while it has something to say.
    let bell = Icon::Custom(if state.has_news() { glyph::BELL_FILL } else { glyph::BELL });
    let bell = header_button("news", bell, TooltipSpec::new(t!("desktop-news")), palette).active(state.page == Page::News).render();
    // How many things it has to say, on its corner.
    let bell = match state.news_count() {
        0 => bell,
        count => {
            let said = if count > 9 { "9+".to_owned() } else { count.to_string() };
            let chip = Element::panel(Element::text(said).font_size(10.0).foreground(SCHEME.canvas).centered()).background(palette.text).radius(BELL_CHIP / 2.0).size(Some(if count > 9 { BELL_CHIP + 6.0 } else { BELL_CHIP }), Some(BELL_CHIP));
            Element::stack(vec![bell, Element::row(vec![Element::spacer().flex(1.0), chip])])
        }
    };
    let trailing: Vec<Element> = vec![bell, account(state, palette)].into_iter().chain(rule(right)).collect();
    let bar = TopBar::new(TOP_BAR, &state.query, t!("common-search-placeholder"), palette)
        .home_icons(Icon::Custom(glyph::HOME), Icon::Custom(glyph::HOME_FILL))
        .browse_icons(Icon::Custom(glyph::BROWSE), Icon::Custom(glyph::BROWSE_FILL))
        .search_icon(Icon::Custom(glyph::SEARCH))
        .width(width)
        .caption(left, right)
        .home_active(state.at_home())
        .browse_active(state.page == Page::Browse)
        .home_label(t!("desktop-home"))
        .browse_label(t!("desktop-browse"))
        .leading(leading)
        .trailing(trailing);
    match recent_searches(state, palette) {
        Some(panel) => bar.search_popover("recent-searches", panel),
        None => bar,
    }
    .render()
}

/// The account's face at the end of the top bar, as Spotify's: its initial on its colour, and
/// under it the menu of whose Surco this is and where to go from there.
fn account(state: &State, palette: ComponentPalette) -> Element {
    let face = profile_face(state, ACCOUNT_FACE, palette);
    // A ring of the bar's own ground, as the face of Spotify's stands on a dark disc.
    let face = Element::panel(face.centered()).background(CHOSEN).radius(ACCOUNT_RING / 2.0).size(Some(ACCOUNT_RING), Some(ACCOUNT_RING));
    let anchor = Element::clickable("account", face).accessible(limn::Access::new(limn::Role::Button).name(state.profile.shown()));
    let item = |id: &str, label: std::borrow::Cow<'static, str>| DropdownMenuEntry::Item(DropdownMenuItem::new(id, label));
    let entries = vec![
        DropdownMenuEntry::Heading(state.profile.shown().to_owned()),
        item("account-profile", t!("desktop-profile")),
        item("account-handoff", t!("desktop-handoff")),
        item("settings", t!("common-settings")),
    ];
    DropdownMenu::new(anchor, entries, state.account_open, "account-dismiss", menu_palette()).sizing(menu_sizing(state)).width(MENU_WIDTH).placement(PopoverPlacement::BottomEnd).render().centered_y()
}

/// The account's initial on its colour, the same for a name wherever it shows.
fn profile_face(state: &State, side: f32, palette: ComponentPalette) -> Element {
    if let Some(image) = state.profile.picture.as_ref().and_then(|name| state.artwork.get(name)) {
        return Element::raster_image(image.clone().cover()).radius(side / 2.0).size(Some(side), Some(side));
    }
    Element::panel(Element::text(state.profile.initial()).font_size(side * 0.44).font_family(FontFamily::SansBold).foreground(palette.surface).centered())
        .background(profile_colour(state))
        .radius(side / 2.0)
        .size(Some(side), Some(side))
}

/// A picture that is the user's to choose, as Spotify's: under the pointer it darkens and
/// offers a pencil and the words to choose another.
fn choosable(id: &str, picture: Element, side: f32, round: bool, palette: ComponentPalette) -> Element {
    let offer = Element::column(vec![
        Element::icon(Icon::Custom(glyph::PENCIL)).large_icon(CHOOSE_GLYPH).foreground(palette.text).size(Some(CHOOSE_GLYPH), Some(CHOOSE_GLYPH)).centered_x(),
        Text::new(t!("desktop-choose-photo"), palette).render().centered_x(),
    ])
    .gap(8.0)
    .centered();
    let radius = if round { side / 2.0 } else { limn::tokens().radius.control };
    let veil = Element::panel(offer).background(COVER_SCRIM).radius(radius).size(Some(side), Some(side)).reveal_on_hover(format!("{id}:veil"));
    Element::clickable(id.to_owned(), Element::stack(vec![picture, veil]))
        .hover_region()
        .accessible(limn::Access::new(limn::Role::Button).name(t!("desktop-choose-photo")))
}

/// The pencil over a picture to choose.
const CHOOSE_GLYPH: f32 = 48.0;

fn profile_colour(state: &State) -> Color {
    MIX_STRIPES[state.profile.shown().bytes().fold(0usize, |hash, byte| hash.wrapping_mul(31).wrapping_add(byte as usize)) % MIX_STRIPES.len()]
}

/// The local account's page, as Spotify's profile: its face and name over its colour, what it
/// keeps, and what its user plays most, which only they see.
fn profile_page(state: &State, palette: ComponentPalette) -> Element {
    let collection = &state.collection;
    let side = if state.mobile { PROFILE_FACE / 2.0 } else { PROFILE_FACE };
    let mut counts = vec![t!("desktop-profile-playlists", count = collection.playlists.len()).into_owned()];
    let following = collection.artists().count();
    if following > 0 { counts.push(t!("desktop-profile-following", count = following).into_owned()); }
    if !collection.liked.is_empty() { counts.push(t!("desktop-profile-liked", count = collection.liked.len()).into_owned()); }
    let name = Text::new(state.profile.shown(), palette).font_size(if state.mobile { HERO_TITLE * 0.75 } else { PROFILE_NAME }).truncate().render();
    let name = Element::clickable("profile-edit", name).accessible(limn::Access::new(limn::Role::Button).name(t!("desktop-profile-edit")));
    let about = Element::column(vec![
        Text::eyebrow(t!("desktop-profile"), palette).render(),
        name,
        Text::new(counts.join(" · "), palette).muted().truncate().render(),
    ])
    .gap(10.0)
    .flex(1.0)
    .centered_y();
    let face = if state.mobile { profile_face(state, side, palette) } else { choosable("profile-picture", profile_face(state, side, palette), side, true, palette) };
    let header = Element::row(vec![face, about]).gap(24.0);
    let mut sections = vec![header];
    let artists: Vec<Element> = discover::top_artists(&collection.heard, crate::state::TOP_ARTISTS).iter().enumerate()
        .map(|(index, track)| tile(state, format!("top-artist:{index}"), &track.artist, Some(t!("common-artist").into_owned()), &track.artwork_url, true, palette))
        .collect();
    if !artists.is_empty() {
        sections.push(headed_shelf(state, "profile-artists", None, Some(&t!("desktop-profile-only-you")), &t!("desktop-top-artists"), artists, palette));
    }
    let tracks: Vec<Element> = state.top_tracks().iter().enumerate()
        .map(|(index, track)| tile(state, format!("top-track:{index}"), &track.title, Some(track.artist.clone()), &track.artwork_url, false, palette))
        .collect();
    if !tracks.is_empty() {
        sections.push(headed_shelf(state, "profile-tracks", None, Some(&t!("desktop-profile-only-you")), &t!("desktop-top-tracks"), tracks, palette));
    }
    let playlists: Vec<Element> = collection.playlists.iter().enumerate()
        .map(|(index, playlist)| tile(state, format!("playlist:{index}"), &playlist.name, Some(songs(playlist.tracks.len())), &playlist.cover().cloned(), false, palette))
        .collect();
    if !playlists.is_empty() {
        sections.push(shelf(state, "profile-playlists", &t!("desktop-your-playlists"), playlists, palette));
    }
    let content = Element::column(sections).gap(24.0).padding(if state.mobile { 16.0 } else { 24.0 });
    page_scroll("profile", content.top_background_fade(Color { a: 150, ..profile_colour(state) }, HEADER_WASH))
}

/// The face in the dialog of the profile's details.
const DIALOG_FACE: f32 = 120.0;
/// The side of the profile's face, and the size of its name, as Spotify's.
const PROFILE_FACE: f32 = 232.0;
const PROFILE_NAME: f32 = 72.0;

/// The side of the account's face, and of the dark disc it stands on.
const ACCOUNT_FACE: f32 = 32.0;
const ACCOUNT_RING: f32 = 48.0;

/// The id of the window's top bar; its search reports as `nav:search`.
pub(crate) const TOP_BAR: &str = "nav";

/// A square of the library's size for something with no picture of its own.
fn library_tile(icon: Icon, palette: ComponentPalette) -> Element {
    Element::panel(Element::icon(icon).foreground(palette.text).centered())
        .background(CHOSEN)
        .radius(limn::tokens().radius.control)
        .size(Some(LIBRARY_TILE), Some(LIBRARY_TILE))
}

/// One row of the library.
struct LibraryEntry<'a> {
    /// What a click reports.
    id: String,
    /// What it is wherever it stands, for a pin to hold on to.
    key: String,
    picture: Element,
    /// Its cover, where it has one, and whether that is a face: for the view in tiles.
    artwork: Option<&'a String>,
    round: bool,
    name: &'a str,
    about: String,
    /// Whose it is; empty for what is the user's own.
    creator: &'a str,
    /// Its page is the one shown.
    here: bool,
}

/// The left panel: what the user keeps, under the filters that narrow it. Folded, it is a
/// rail of pictures.
fn library(state: &State, rail: bool, width: f32, palette: ComponentPalette) -> Element {
    let shows = |filter: Filter| state.filter == Filter::All || state.filter == filter;
    let (downloads, liked) = (t!("common-downloads"), t!("common-liked-songs"));
    let mut entries: Vec<LibraryEntry> = Vec::new();
    if shows(Filter::Lists) {
        let count = format!("{} · {}", t!("common-playlist"), songs(state.saved.len()));
        entries.push(LibraryEntry { id: "downloads".into(), key: "downloads".into(), picture: library_tile(Icon::Custom(DOWNLOAD), palette), artwork: None, round: false, name: &downloads, about: count, creator: "", here: state.page == Page::Downloads });
        if !state.collection.liked.is_empty() {
            let count = format!("{} · {}", t!("common-playlist"), songs(state.collection.liked.len()));
            entries.push(LibraryEntry { id: "liked".into(), key: "liked".into(), picture: library_tile(Icon::HeartFill, palette), artwork: None, round: false, name: &liked, about: count, creator: "", here: state.page == Page::Liked });
        }
        for (index, playlist) in state.collection.playlists.iter().enumerate() {
            let artwork = playlist.cover();
            let picture = match artwork {
                Some(url) => cover(state, &Some(url.clone()), LIBRARY_TILE, palette),
                None => library_tile(Icon::Playlist, palette),
            };
            let about = format!("{} · {}", if playlist.collaborative { t!("desktop-collaborative-playlist") } else { t!("common-playlist") }, songs(playlist.tracks.len()));
            entries.push(LibraryEntry { id: format!("playlist:{index}"), key: format!("playlist:{}", playlist.id), picture, artwork, round: false, name: &playlist.name, about, creator: "", here: state.page == Page::Playlist(playlist.id) });
        }
    }
    if shows(Filter::Artists) {
        for (index, artist) in state.collection.artists() {
            let picture = cover(state, &artist.artwork_url, LIBRARY_TILE, palette).radius(LIBRARY_TILE / 2.0);
            let here = matches!(&state.page, Page::Artist(page) if page.id == artist.id);
            entries.push(LibraryEntry { id: format!("kept:{index}"), key: format!("artist:{}", artist.id), picture, artwork: artist.artwork_url.as_ref(), round: true, name: &artist.name, about: t!("common-artist").into_owned(), creator: &artist.name, here });
        }
    }
    if shows(Filter::Albums) {
        for (index, album) in state.collection.albums() {
            let about = album.artist.as_ref().map_or_else(|| t!("common-album").into_owned(), |artist| format!("{} · {artist}", t!("common-album")));
            let here = matches!(&state.page, Page::Album(page) if page.id == album.id);
            let picture = cover(state, &album.artwork_url, LIBRARY_TILE, palette);
            entries.push(LibraryEntry { id: format!("kept:{index}"), key: format!("album:{}", album.id), picture, artwork: album.artwork_url.as_ref(), round: false, name: &album.title, about, creator: album.artist.as_deref().unwrap_or(""), here });
        }
    }
    match state.settings.library_sort {
        Sort::Added => {}
        Sort::Alphabetical => entries.sort_by_cached_key(|entry| entry.name.to_lowercase()),
        Sort::Creator => entries.sort_by_cached_key(|entry| (entry.creator.to_lowercase(), entry.name.to_lowercase())),
    }
    // What is pinned goes first, in the order it was pinned; the rest keeps its sort.
    let pinned = |key: &str| state.collection.pinned.iter().position(|pinned| pinned == key);
    entries.sort_by_key(|entry| pinned(&entry.key).unwrap_or(usize::MAX));
    if rail {
        // limn's rail: each entry its picture, named to the right at once under the pointer.
        let current = entries.iter().find(|entry| entry.here).map(|entry| entry.id.clone());
        let items = entries.into_iter().map(|entry| SideRailItem::new(entry.id, entry.name).subtitle(entry.about).picture(entry.picture).round(entry.round)).collect();
        let rail = SideRail::new("library-rail", items, palette).create(t!("common-create-playlist"))
            .library_icon(Icon::Custom(glyph::LIBRARY))
            .create_icon(Icon::Custom(glyph::PLUS));
        // Folded by the user, it unfolds from its head; a window too narrow keeps it folded.
        let rail = if state.library_open { rail } else { rail.expand(t!("desktop-show-library")) };
        return match current { Some(id) => rail.current(id), None => rail }.render();
    }
    // A panel is shut from its own header.
    let title = Text::new(if state.mobile { t!("desktop-saved-for-you") } else { t!("common-library") }, palette).font_size(15.0);
    // As Spotify's on a desktop: the name in bold, beside the glyph that folds the panel.
    let title = if state.mobile { title.muted().render() } else { title.font_size(16.0).render().font_family(FontFamily::SansBold) };
    let mut header = vec![title.centered_y(), Element::spacer().flex(1.0)];
    // A phone makes playlists from the page's own bar.
    if !state.mobile {
        header.push(IconButton::new("playlist-new", Icon::Custom(glyph::PLUS), palette).round().variant(ButtonVariant::Ghost).size(ControlSize::Dense).tooltip(t!("common-create-playlist")).accessibility_label(t!("common-create-playlist")).render().centered_y());
        header.insert(0, header_button("toggle-library", Icon::Custom(glyph::LIBRARY), TooltipSpec::new(t!("desktop-hide-library")).shortcut("Ctrl+B"), palette).render().centered_y());
    }
    let header = Element::row(header)
    .gap(8.0)
    .size(None, Some(if state.mobile { 44.0 } else { 40.0 }));
    let chip = |id: &str, label: &str, selected| Button::new(id, label, palette)
        .variant(if selected { ButtonVariant::Default } else { ButtonVariant::Secondary })
        .sizing(if state.mobile { limn::ButtonSizing { height: 44.0, ..ControlSize::Regular.button_sizing() } } else { ControlSize::Regular.button_sizing() })
        .round().render();
    // Pills, as Spotify's: the one chosen is filled light. A panel too narrow for them scrolls them sideways.
    let filters = Element::row(vec![
        chip("filter:all", &t!("common-all"), state.filter == Filter::All),
        chip("filter:lists", &t!("desktop-playlists"), state.filter == Filter::Lists),
        chip("filter:artists", &t!("common-artists"), state.filter == Filter::Artists),
        chip("filter:albums", &t!("common-albums"), state.filter == Filter::Albums),
    ]).gap(8.0);
    let filters = Element::horizontal_scroll_2d("mobile-library-filters", filters).drag_to_pan().scrollbar_visibility(mobile::SCROLL_INDICATOR);
    // A magnifier that opens into the search, and at the far end the order and the view.
    let looking = match state.filter {
        Filter::All => t!("desktop-search-library"),
        Filter::Lists => t!("desktop-search-in", place = t!("desktop-playlists")),
        Filter::Artists => t!("desktop-search-in", place = t!("common-artists")),
        Filter::Albums => t!("desktop-search-in", place = t!("common-albums")),
    };
    let open = state.library_search_slide.value(std::time::Instant::now()).clamp(0.0, 1.0);
    let search_button = |glyph, hint| if state.mobile { mobile::icon("library-search", glyph, hint, palette).radius(22.0).centered_y() }
        else { header_button("library-search", glyph, TooltipSpec::new(hint), palette).render().centered_y() };
    let mut tools = if open > 0.0 {
        // It grows out of where the magnifier was to all the row has left beside the order,
        // however wide the order's name leaves that: the room is shared out, not counted.
        let grown = open.max(LIBRARY_SEARCH_SHUT);
        let field = SearchField::new("library-query", &state.library_search, looking.clone(), SCHEME.on_raised()).size(if state.mobile { ControlSize::Spacious } else { ControlSize::Compact }).render();
        if state.mobile {
            let field = mobile::inside_cross(field.size(None, Some(44.0)).radius(22.0), "library-search", &t!("desktop-close-search"), palette);
            vec![field.size(None, Some(44.0)).centered_y().flex(grown), Element::spacer().flex(1.0 - grown)]
        } else {
            // The cross closes it from inside its end, as the window's search clears.
            let close = IconButton::new("library-search", Icon::Close, SCHEME.on_raised()).variant(ButtonVariant::Subtle).size(ControlSize::Dense).round()
                .tooltip(t!("desktop-close-search")).render().centered_y();
            let field = Element::stack(vec![field.radius(ControlSize::Compact.height() / 2.0), Element::row(vec![Element::spacer().flex(1.0), close]).padding(2.0)]);
            vec![field.centered_y().flex(grown), Element::spacer().flex(1.0 - grown)]
        }
    } else {
        vec![search_button(if state.mobile { Icon::Search } else { Icon::Custom(glyph::SEARCH) }, &looking), Element::spacer().flex(1.0)]
    };
    let (sort, view) = (state.settings.library_sort, state.settings.library_view);
    let choice = |id: &str, label: &str, chosen: bool| DropdownMenuEntry::Item(DropdownMenuItem::new(id, label).selected(chosen));
    let choices = vec![
        DropdownMenuEntry::Heading(t!("desktop-sort-by").into()),
        choice("library-sort:added", &t!("desktop-sort-added"), sort == Sort::Added),
        choice("library-sort:alphabetical", &t!("desktop-sort-alphabetical"), sort == Sort::Alphabetical),
        choice("library-sort:creator", &t!("common-creator"), sort == Sort::Creator),
        DropdownMenuEntry::Separator,
        DropdownMenuEntry::Heading(t!("desktop-view-as").into()),
        choice("library-view:compact", &t!("desktop-view-compact"), view == LibraryView::Compact),
        choice("library-view:list", &t!("desktop-view-list"), view == LibraryView::List),
        choice("library-view:grid", &t!("desktop-view-grid"), view == LibraryView::Grid),
    ];
    let picker = Button::new("library-sort", sort.label(), palette).icon(Icon::Sort).variant(ButtonVariant::Ghost).size(ControlSize::Compact).render();
    let picker = if state.mobile { picker.size(None, Some(44.0)).radius(22.0) } else { picker };
    // All of its choices at once: past eight a menu becomes a list that scrolls, with a rail's room beside it.
    let all = choices.len();
    tools.push(DropdownMenu::new(picker, choices, state.library_menu, "library-menu-close", menu_palette()).sizing(menu_sizing(state)).width(LIBRARY_MENU_WIDTH).max_visible_items(all).placement(PopoverPlacement::BottomEnd).render().centered_y());
    let tools = Element::row(tools).gap(8.0).size(None, Some(if state.mobile { 44.0 } else { ControlSize::Compact.height() }));

    let wanted = state.library_search.trim().to_lowercase();
    // Tiles share the panel's width, two of them at the least.
    let columns = (((width - 16.0 + GAP) / (LIBRARY_GRID_TILE + GAP)).floor() as usize).max(2);
    let tile_width = (width - 16.0 - GAP * (columns - 1) as f32) / columns as f32;
    let shown: Vec<Element> = entries
        .into_iter()
        .filter(|entry| entry.name.to_lowercase().contains(&wanted) || entry.about.to_lowercase().contains(&wanted))
        .map(|LibraryEntry { id, key, picture, artwork, round, name, about, here, .. }| {
            let held = pinned(&key).is_some();
            let about = if held { format!("{} · {about}", t!("desktop-pinned")) } else { about };
            // The page being shown stands out by its fill.
            let palette = if here { ComponentPalette { surface: CHOSEN, ..palette } } else { palette };
            let entry = match view {
                LibraryView::Grid => {
                    let tile = MediaTile::new(id, name, palette).width(tile_width).round(round).subtitle(about);
                    match (artwork.and_then(|url| state.artwork.get(url)), coming(state, artwork, palette)) {
                        (Some(image), _) => tile.artwork(Element::raster_image(image.clone().cover())),
                        (None, Some(pulse)) => tile.artwork(pulse),
                        (None, None) => tile,
                    }
                    .render()
                }
                // A line each: what it is called, and what it is.
                LibraryView::Compact => {
                    let kind = about.rsplit(" · ").last().unwrap_or_default().to_owned();
                    let sizing = if state.mobile { ListRowSizing { height: Some(44.0), ..ListRowSizing::COMPACT } } else { ListRowSizing::COMPACT };
                    let row = ListRow::new(id, name, palette).sizing(sizing).trailing(Text::new(kind, palette).role(TextRole::Caption).muted().render()).title_fade(true).radius(RadiusRole::Card);
                    if here { row.filled() } else { row }.render()
                }
                LibraryView::List => {
                    let row = ListRow::new(id, name, palette).sizing(LIBRARY_ROW).leading(picture).subtitle(about).title_fade(true).radius(RadiusRole::Card);
                    if here { row.filled() } else { row }.render()
                }
            };
            if state.mobile {
                let more = format!("library-more:{key}");
                let dots = mobile::icon(&more, Icon::More, &t!("desktop-options-of", name = name), palette);
                let entry = if view == LibraryView::Grid {
                    Element::stack(vec![entry, Element::row(vec![Element::spacer().flex(1.0), dots.background(SCHEME.raised).radius(22.0)]).padding(4.0)])
                } else { Element::row(vec![entry.flex(1.0), dots.centered_y()]).gap(4.0) };
                return ContextMenu::new(format!("library-context:{key}"), entry, Vec::new(), menu_palette()).open_action(more).render();
            }
            let pin = ContextMenuItem::new(format!("pin:{key}"), if held { t!("desktop-unpin") } else { t!("desktop-pin") });
            ContextMenu::new(format!("library-context:{key}"), entry, vec![ContextMenuEntry::Item(pin)], menu_palette()).render()
        })
        .collect();
    let list = if shown.is_empty() {
        let what = if !wanted.is_empty() {
            t!("desktop-library-not-found", query = state.library_search.trim())
        } else if state.filter == Filter::Artists { t!("desktop-library-no-artists") } else { t!("desktop-library-no-albums") };
        Element::panel(Text::new(what, palette).muted().render()).padding(8.0)
    } else if view == LibraryView::Grid {
        let mut tiles = shown.into_iter().peekable();
        let mut rows = Vec::new();
        while tiles.peek().is_some() {
            rows.push(Element::row(tiles.by_ref().take(columns).collect()).gap(GAP));
        }
        let rows = Element::column(rows).gap(GAP);
        if state.mobile { ScrollArea::new("library", rows) } else { panel_scroll("library", rows) }.render().flex(1.0)
    } else {
        let rows = Element::column(shown).gap(2.0);
        if state.mobile { ScrollArea::new("library", rows) } else { panel_scroll("library", rows) }.render().flex(1.0)
    };
    let mut list = list;
    let mut parts = vec![Element::panel(header).padding(8.0), filters, tools];
    if !state.settings.import_dismissed && wanted.is_empty() {
        if state.mobile {
            let card = import_card(width - 16.0, palette, state.mobile_ui.native_surfaces);
            if let limn::ElementKind::Scroll { child, .. } = &mut list.kind {
                let rows = std::mem::replace(child.as_mut(), Element::spacer());
                **child = Element::column(vec![card, rows]).gap(16.0);
            } else {
                list = ScrollArea::new("library", Element::column(vec![card, list]).gap(16.0)).render().flex(1.0);
            }
        } else { parts.push(library_offer(width - 16.0, palette)); }
    }
    parts.push(list);
    Element::column(parts).gap(6.0).padding(8.0)
}

/// The page the app opens on: what was opened lately, a click away, and what the user keeps.
fn home(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let collection = &state.collection;
    if collection.recent.is_empty() && collection.kept.is_empty() && state.saved.is_empty() && collection.heard.is_empty() {
        return centered(
            Empty::new(t!("desktop-home-empty"), t!("desktop-home-empty-detail"), palette).icon(Icon::Search).render(),
        );
    }
    let hour = chrono::Timelike::hour(&chrono::Local::now());
    let mut sections = if state.mobile { Vec::new() } else { vec![Text::new(greeting(hour), palette).font_size(PAGE_TITLE).render()] };

    // Quick access, as Spotify's: up to eight short rows a step above the page, in rows of
    // two, three or four as the room allows and as many as there are fill. The rows grow
    // with the room, and their pictures with them.
    let count = (collection.recent.len() + usize::from(!state.saved.is_empty())).min(QUICK_MOST);
    let count = if count < QUICK_MOST && count % 2 == 1 && count > 1 { count - 1 } else { count };
    let columns = match width {
        _ if state.mobile => 2,
        width if width < 700.0 => 2,
        _ if count % 4 == 0 => 4,
        _ => 3,
    };
    let side = ((width - 24.0 - (columns - 1) as f32 * GAP) / columns as f32).max(120.0);
    let height = if state.mobile { LIBRARY_TILE } else if width >= 1599.0 { 80.0 } else if width >= 1141.0 { 64.0 } else { 48.0 };
    let raised = ComponentPalette { surface: CHOSEN, ..palette };
    let quick = |id: String, name: &str, picture: Element, sounding: bool| {
        if state.mobile {
            let element = ListRow::new(&id, name, raised).sizing(QUICK_ROW).leading(picture).title_fade(true).radius(RadiusRole::Card).width(side).filled().render();
            return mobile_card_context(state, &id, element);
        }
        // What plays shows its sound at the row's end; the others, their play under the pointer.
        let end = if sounding {
            wave(state, format!("wave:{id}"), palette)
        } else {
            let round = ComponentPalette { surface: palette.text, text: SCHEME.canvas, hover: ROUND_HOVER, selected: ROUND_HELD, ..palette };
            IconButton::new(format!("tile-play:{id}"), Icon::PlayFill, round).variant(ButtonVariant::Subtle).size(ControlSize::Compact).round()
                .tooltip(t!("common-play")).render().reveal_on_hover(format!("quick-play:{id}"))
        };
        let said = limn::Access::new(limn::Role::Button).name(name);
        let label = Text::new(name, palette).font_size(if height >= 64.0 { 16.0 } else { 13.0 }).truncate().render().font_family(FontFamily::SansBold);
        let row = Element::row(vec![picture.size(Some(height), Some(height)), label.flex(1.0).centered_y(), end.centered_y(), Element::spacer().size(Some(8.0), None)]).gap(12.0);
        Element::clickable(&id, row)
            .accessible(said)
            .background(CHOSEN)
            .hover_background(QUICK_HOVER)
            .transition_ms(300)
            .radius(QUICK_RADIUS)
            .clip_children()
            .hover_region()
            .size(Some(side), Some(height))
    };
    let mut tiles = Vec::new();
    if !state.saved.is_empty() {
        // The library lists the same entries: here they keep ids of their own and report the library's.
        let sounding = state.player.playing && state.plays_from(&state.saved);
        tiles.push(quick("home-downloads".into(), &t!("common-downloads"), library_tile(Icon::Custom(DOWNLOAD), palette), sounding).dispatch_as("downloads"));
    }
    for (index, entry) in collection.recent.iter().enumerate() {
        let picture = cover(state, entry.artwork(), height, palette);
        let picture = match entry {
            Entry::Artist(_) if state.mobile => picture.radius(LIBRARY_TILE / 2.0),
            _ if state.mobile => picture,
            // Flush with the row's edge: its rounding is the row's.
            _ => picture.radius(0.0),
        };
        let sounding = state.player.playing && state.current().is_some_and(|current| match entry {
            Entry::Album(album) => current.album_id.as_deref() == Some(album.id.as_str()),
            Entry::Artist(artist) => current.artist_id.as_deref() == Some(artist.id.as_str()),
        });
        tiles.push(quick(format!("recent:{index}"), entry.title(), picture, sounding));
    }
    tiles.truncate(count);
    let mut tiles = tiles.into_iter().peekable();
    let mut grid = Vec::new();
    while tiles.peek().is_some() {
        grid.push(Element::row(tiles.by_ref().take(columns).collect()).gap(GAP));
    }
    sections.push(Element::column(grid).gap(GAP));

    // The catalog's picks as lists, Spotify's mixes: the time of day's leads, large, and the
    // rest are the cards of a shelf.
    let mixes = state.mixes();
    if let Some((index, mix)) = mixes.iter().enumerate().find(|(_, mix)| mix.kind == MixKind::Moment) {
        sections.push(moment_card(state, index, mix, width, palette).animate_enter("home-moment"));
    }
    let cards: Vec<Element> = mixes.iter().enumerate().filter(|(_, mix)| mix.kind != MixKind::Moment).map(|(index, mix)| mix_tile(state, index, mix, palette)).collect();
    if !cards.is_empty() {
        // Picks arrive after the page: they settle in rather than appear.
        sections.push(headed_shelf(state, "home-mixes", None, Some(&t!("desktop-made-for-you")), &t!("desktop-your-mixes"), cards, palette).animate_enter("home-mixes"));
    } else if state.picks_coming[0] {
        sections.push(headed_shelf(state, "home-mixes", None, Some(&t!("desktop-made-for-you")), &t!("desktop-your-mixes"), coming_tiles(state, false, palette), palette));
    }
    // Today's: what goes with the latest plays, as Spotify's "Recommended for today".
    let pick = |id: String, track: &Track| tile(state, id, &track.title, Some(track.artist.clone()), &track.artwork_url, false, palette);
    let today: Vec<Element> = state.recent_picks.iter().take(SHELF).enumerate().map(|(index, track)| pick(format!("today:{index}"), track)).collect();
    if !today.is_empty() {
        sections.push(headed_shelf(state, "home-today", None, Some(&t!("desktop-inspired-recent")), &t!("desktop-recommended-today"), today, palette));
    }
    // What was played, a click from playing again.
    let heard: Vec<Element> = collection.heard.iter().take(SHELF).enumerate().map(|(index, track)| pick(format!("heard:{index}"), track)).collect();
    if !heard.is_empty() {
        sections.push(shelf(state, "home-heard", &t!("desktop-listen-again"), heard, palette));
    }
    // For the fans of an artist played most: the albums of artists like them, under their face.
    let fans = |shelf: usize| state.fans_of.get(shelf).map(|(artist, albums)| {
        let face = Element::clickable(format!("fans-{shelf}"), cover(state, &artist.artwork_url, FACE, palette).radius(FACE / 2.0));
        let tiles = release_tiles(state, &format!("fans-{shelf}-album"), albums, palette);
        headed_shelf(state, &format!("home-fans:{shelf}"), Some(face), Some(&t!("desktop-for-fans-of")), &artist.name, tiles, palette)
    });
    sections.extend(fans(0));
    let artists: Vec<Element> = state
        .picked_artists
        .iter()
        .enumerate()
        .map(|(index, artist)| tile(state, format!("pick-artist:{index}"), &artist.name, Some(t!("common-artist").into_owned()), &artist.artwork_url, true, palette))
        .collect();
    if !artists.is_empty() {
        sections.push(shelf(state, "home-pick-artists", &t!("desktop-artists-to-discover"), artists, palette));
    } else if state.picks_coming[1] {
        sections.push(shelf(state, "home-pick-artists", &t!("desktop-artists-to-discover"), coming_tiles(state, true, palette), palette));
    }
    // More of the artist played most: their releases the user does not keep.
    if let Some((artist, albums)) = &state.more_of {
        let face = Element::clickable("more-artist", cover(state, &artist.artwork_url, FACE, palette).radius(FACE / 2.0));
        let tiles = release_tiles(state, "more-album", albums, palette);
        sections.push(headed_shelf(state, "home-more", Some(face), Some(&t!("desktop-more-of")), &artist.name, tiles, palette));
    }
    // What the artists followed released since, as Spotify's "Release Radar".
    let news: Vec<AlbumRef> = collection.news.iter().map(|(release, _)| release.clone()).take(SHELF).collect();
    if !news.is_empty() {
        sections.push(shelf(state, "home-news", &t!("desktop-news-from-followed"), release_tiles(state, "news", &news, palette), palette));
    }
    sections.extend(fans(1));
    if !state.picked_albums.is_empty() {
        sections.push(shelf(state, "home-pick-albums", &t!("desktop-albums-for-you"), release_tiles(state, "pick-album", &state.picked_albums, palette), palette));
    } else if state.picks_coming[2] {
        sections.push(shelf(state, "home-pick-albums", &t!("desktop-albums-for-you"), coming_tiles(state, false, palette), palette));
    }
    // What the user likes, a click from playing it.
    let liked: Vec<Element> = collection.liked.iter().take(SHELF).enumerate().map(|(index, track)| pick(format!("liked-track:{index}"), track)).collect();
    if !liked.is_empty() {
        sections.push(shelf(state, "home-liked", &t!("common-liked-songs"), liked, palette));
    }
    // Where else to go: the genres to browse, two rows of them.
    if !state.mobile {
        let room = width - 24.0;
        let columns = ((room / 200.0) as usize).clamp(2, 6);
        let side = (room - 12.0 * (columns - 1) as f32) / columns as f32;
        let genres = mobile::search::browse(columns * 2, room, columns, (side * 0.5).clamp(84.0, 120.0), palette);
        sections.push(Element::column(vec![Element::row(vec![Element::spacer().size(Some(TILE_PAD), None), heading(&t!("desktop-explore-genres"), palette)]), genres]).gap(12.0));
    }

    let albums: Vec<Element> = collection
        .albums()
        .map(|(index, album)| {
            tile(state, format!("home-kept:{index}"), &album.title, album.artist.clone(), &album.artwork_url, false, palette).dispatch_as(format!("kept:{index}"))
        })
        .collect();
    if !albums.is_empty() {
        sections.push(shelf(state, "home-albums", &t!("desktop-your-albums"), albums, palette));
    }
    let artists: Vec<Element> = collection
        .artists()
        .map(|(index, artist)| {
            tile(state, format!("home-kept:{index}"), &artist.name, Some(t!("common-artist").into_owned()), &artist.artwork_url, true, palette).dispatch_as(format!("kept:{index}"))
        })
        .collect();
    if !artists.is_empty() {
        sections.push(shelf(state, "home-artists", &t!("desktop-your-artists"), artists, palette));
    }
    // The colour of the entry under the pointer washes down from the top, as Spotify's.
    let content = Element::column(sections).gap(16.0).padding(12.0);
    let wash = state.home_wash(std::time::Instant::now());
    page_scroll("home", if wash.a == 0 || state.mobile { content } else { content.top_background_fade(wash, HOME_WASH) })
}

/// How wide a shelf's edge fades where tiles go on past it.
const SHELF_FADE: f32 = 72.0;
/// How far down the home its wash reaches.
const HOME_WASH: f32 = 320.0;
/// The face beside the heading of a shelf for an artist's fans, as Spotify's.
const FACE: f32 = 48.0;

/// What was searched lately, dropped down from the desktop's field while it holds the
/// keyboard with nothing typed, as Spotify's: a row each to search again, its cross under the
/// pointer, and a way to forget them all below. It goes when the keyboard leaves them.
fn recent_searches(state: &State, palette: ComponentPalette) -> Option<Element> {
    if !state.recent_searches_shown() {
        return None;
    }
    let raised = ComponentPalette { surface: SCHEME.raised, ..palette };
    let rows: Vec<Element> = state.collection.searches.iter().enumerate().map(|(index, searched)| {
        let (title, subtitle, artwork, round) = remembered(searched);
        let row = format!("recent-search:{index}");
        let picture = || match artwork {
            Some(artwork) => cover(state, artwork, 48.0, palette).radius(if round { 24.0 } else { 4.0 }),
            None => Element::panel(Element::icon(Icon::Custom(glyph::SEARCH)).foreground(palette.muted).size(Some(20.0), Some(20.0)).centered())
                .background(CHOSEN).radius(4.0).size(Some(48.0), Some(48.0)),
        };
        let song = match searched { Searched::Track { track } => Some(track), _ => None };
        // A song's cover says what a click does, as a list's rows: play or pause under the
        // pointer, and its wave while it sounds.
        let leading = match song {
            Some(track) => {
                let sounding = state.player.playing && state.current().is_some_and(|current| current.id == track.id);
                let veiled = |over: Element| Element::stack(vec![picture(), Element::panel(over.centered()).background(COVER_SCRIM).radius(4.0).size(Some(48.0), Some(48.0))]);
                let at_rest = if sounding { veiled(wave(state, format!("wave:{row}"), palette)) } else { picture() };
                let does = veiled(Element::icon(if sounding { Icon::PauseFill } else { Icon::PlayFill }).foreground(palette.text).size(Some(16.0), Some(16.0)));
                Element::stack(vec![at_rest.conceal_on_hover(format!("rest:{row}")), does.reveal_on_hover(format!("does:{row}"))])
            }
            None => picture(),
        };
        let forget = IconButton::new(format!("recent-search-remove:{index}"), Icon::Custom(glyph::CLOSE), raised).variant(ButtonVariant::Ghost).size(ControlSize::Dense).round()
            .tooltip(t!("desktop-forget-search", query = title)).render().reveal_on_hover(format!("forget:{index}"));
        let mut list_row = ListRow::new(&row, title, raised)
            .sizing(ListRowSizing { height: Some(64.0), two_line_height: Some(64.0), gap: 12.0, leading_width: 48.0, ..TRACK_ROW })
            .leading(leading).subtitle(subtitle).title_fade(true).radius(RadiusRole::Card);
        // A song has the options of any other, behind its dots, as Spotify's.
        let mut pressed = Vec::new();
        if let Some(track) = song {
            let (menu, options) = row_menu(state, &row, track, raised);
            pressed = options;
            list_row = list_row.trailing_action(format!("more:{row}"), middle(menu.reveal_on_hover(format!("dots:{row}"))));
        }
        let element = list_row.trailing_action(format!("recent-search-remove:{index}"), middle(forget)).render().hover_region();
        if pressed.is_empty() { element } else { ContextMenu::new(format!("context:{row}"), element, pressed, menu_palette()).render() }
    }).collect();
    let heading = Text::new(t!("desktop-recent-searches"), raised).font_size(16.0).render().font_family(FontFamily::SansBold).padding(8.0);
    // A word in the middle, lit under the pointer.
    let clear = Button::new("recent-search-clear", t!("common-clear-recent-searches"), raised).variant(ButtonVariant::Ghost).size(ControlSize::Compact).round().render();
    let list = ScrollArea::new("recent-searches", Element::column(rows).gap(2.0)).render().max_height(398.0);
    let panel = Element::panel(Element::column(vec![heading, list, Element::row(vec![Element::spacer().flex(1.0), clear, Element::spacer().flex(1.0)]).padding(8.0)]).gap(4.0))
        .padding(8.0).background(SCHEME.raised).radius(8.0)
        .shadow(Color::rgba(0, 0, 0, 77), limn::Point::new(0.0, 16.0), 24.0, 0.0)
        .size(Some(SEARCH_MAX_WIDTH.min(560.0)), None);
    // The bar floats it non-modal, so the field keeps the keyboard and typing goes on in it.
    Some(panel)
}

/// A search remembered as its row shows it, as Spotify's: its title, what it is, its picture
/// (none for what was typed) and whether that is round, as an artist's.
pub(crate) fn remembered(searched: &Searched) -> (&str, String, Option<&Option<String>>, bool) {
    match searched {
        Searched::Query(query) => (query, t!("desktop-search-tile").into_owned(), None, false),
        Searched::Track { track } => (&track.title, format!("{} • {}", t!("common-song"), track.artist), Some(&track.artwork_url), false),
        Searched::Entry(Entry::Artist(artist)) => (&artist.name, t!("common-artist").into_owned(), Some(&artist.artwork_url), true),
        Searched::Entry(Entry::Album(album)) => (&album.title, match &album.artist {
            Some(artist) => format!("{} • {artist}", t!("common-album")),
            None => t!("common-album").into_owned(),
        }, Some(&album.artwork_url), false),
    }
}

/// Where a search starts: every genre to browse; what was searched lately drops down from
/// the field instead.
fn browse_page(width: f32, palette: ComponentPalette) -> Element {
    // As Spotify's: the recent searches drop down from the field, and the page is the genres.
    let mut sections = vec![Text::new(t!("desktop-browse-all"), palette).font_size(24.0).render().font_family(FontFamily::SansBold)];
    let width = width - 24.0;
    let columns = ((width / 200.0) as usize).clamp(2, 5);
    let side = (width - 12.0 * (columns - 1) as f32) / columns as f32;
    sections.push(mobile::search::browse(usize::MAX, width, columns, (side * 0.6).clamp(98.0, 160.0), palette));
    page_scroll("browse-page", Element::column(sections).gap(16.0).padding(12.0))
}

/// A page's content under the colour of its cover, which gives way to the surface below
/// its header.
fn washed(state: &State, artwork: &Option<String>, content: Element) -> Element {
    match artwork.as_deref().and_then(|url| state.tints.get(url)) {
        Some(tint) => content.top_background_fade(Color { a: 150, ..*tint }, HEADER_WASH),
        None => content,
    }
}

/// A page of the middle panel. Its bar floats over the page, as Spotify's does, so a
/// page's wash runs under it to the panel's edge instead of stopping short of a rail.
fn page_scroll(id: &str, content: Element) -> Element {
    panel_scroll(id, content).render().flex(1.0)
}

/// A desktop panel's scroll area with Spotify's bar, as its client draws it: a thumb 12 wide
/// over the content, white at 30%, 50% under the pointer and 70% held, at least 30 long, with
/// no track. It shows while the pointer is over the panel or it moves, and fades in 0.2 s.
fn panel_scroll(id: &str, content: Element) -> ScrollArea {
    ScrollArea::new(id, content).appearance(spotify_bar()).visibility(SPOTIFY_BAR_SHOWS)
}

/// Every tooltip as Spotify's: its menus' grey, no outline, white words.
pub(crate) const TOOLTIP: limn::TooltipStyle = limn::TooltipStyle {
    background: Color::rgb(40, 40, 40),
    border: Color::rgba(0, 0, 0, 0),
    keycap: Color::rgb(62, 62, 62),
    text: Color::WHITE,
    muted: Color::rgb(179, 179, 179),
    shadow: Color::rgba(0, 0, 0, 140),
};

/// When Spotify's bar shows: under the pointer or while it moves, and a moment after.
const SPOTIFY_BAR_SHOWS: limn::ScrollbarVisibility = limn::ScrollbarVisibility::AutoHideUnlessHovered { hold_ms: 600, fade_ms: 200 };

/// Spotify's scrollbar, wherever something scrolls on a desktop: its pages, its panels, its
/// menus.
fn spotify_bar() -> ScrollbarAppearance {
    ScrollbarAppearance {
        rail_width: 14.0,
        thumb_width: 12.0,
        min_thumb_length: 30.0,
        tint: Some(Color::rgb(255, 255, 255)),
        idle_thumb_opacity: 77,
        hover_thumb_opacity: 128,
        active_thumb_opacity: 179,
        hover_track_opacity: 0,
        buttons: false,
        // Square, as it is on Windows and Linux.
        thumb_radius: Some(0.0),
        ..ScrollbarAppearance::overlay()
    }
}

fn centered(element: Element) -> Element {
    Element::column(vec![Element::spacer().flex(1.0), element.centered_x(), Element::spacer().flex(1.0)]).flex(1.0)
}

fn heading(title: &str, palette: ComponentPalette) -> Element {
    Text::new(title, palette).role(TextRole::Heading).render()
}

/// A heading over a row of tiles that scrolls sideways inside the page: by dragging it, or
/// by the trackpad.
fn shelf(state: &State, id: &str, title: &str, tiles: Vec<Element>, palette: ComponentPalette) -> Element {
    headed_shelf(state, id, None, None, title, tiles, palette)
}

/// A shelf whose heading is said in two parts, as "Hecho para vos" over "Tus mixes", and may
/// stand beside a picture, as "Para fans de" an artist does beside them.
fn headed_shelf(state: &State, id: &str, leading: Option<Element>, eyebrow: Option<&str>, title: &str, tiles: Vec<Element>, palette: ComponentPalette) -> Element {
    // The heading lines up with the covers, inside their cards' padding.
    let pad = || Element::spacer().size(Some(if state.mobile { 0.0 } else { TILE_PAD }), None);
    let heading = match eyebrow {
        Some(eyebrow) => Element::column(vec![Text::new(eyebrow, palette).role(TextRole::Caption).muted().render(), heading(title, palette)]).gap(2.0),
        None => heading(title, palette),
    };
    let mut header = vec![pad()];
    header.extend(leading.map(|picture| picture.centered_y()));
    header.extend([heading.flex(1.0).centered_y(), pad()]);
    let strip = if state.mobile {
        // A phone's touch has its own pan; under the tiles, room for the strip's bar.
        let tiles = Element::column(vec![Element::row(tiles).gap(TILE_GAP), Element::spacer().size(None, Some(SHELF_BAR_ROOM))]);
        Element::horizontal_scroll_2d(id, tiles).drag_to_pan().foreground(palette.muted).scrollbar_visibility(mobile::SCROLL_INDICATOR)
    } else {
        // limn's carousel: dragged, or sideways on a touchpad, it glides on when let go, as
        // Threads' rows; a tile dragged is not opened. The cards' padding is the space between.
        // Its cut edge fades into the page where there is more to see, as Spotify's shelves.
        Carousel::new(id, tiles, palette).gap(0.0).render().horizontal_edge_fade(palette.surface, SHELF_FADE)
    };
    Element::column(vec![Element::row(header).gap(12.0), strip]).gap(8.0)
}

/// A shelf's tiles while what fills them is on its way: the shape of each, a picture and two
/// lines, pulsing in step with every cover that is coming.
fn coming_tiles(state: &State, round: bool, palette: ComponentPalette) -> Vec<Element> {
    let (width, inset) = if state.mobile { (TILE_WIDTH, 0.0) } else { (CARD_WIDTH, TILE_PAD) };
    let side = width - 2.0 * inset;
    let pulse = ComponentPalette { raised: palette.selected, ..palette };
    (0..6).map(|_| {
        let picture = limn::Skeleton::block("cover-coming", pulse, side, side).radius(if round { side / 2.0 } else { limn::tokens().radius.control }).render();
        let lines = [0.7, 0.45].map(|share| limn::Skeleton::line("cover-coming", pulse, side * share).render());
        let lines: Vec<Element> = if round { lines.map(Element::centered_x).into() } else { lines.into() };
        Element::column(vec![picture, Element::column(lines).gap(8.0)]).gap(12.0).padding(inset).size(Some(width), None)
    }).collect()
}

fn tile(state: &State, id: String, title: &str, subtitle: Option<String>, artwork: &Option<String>, round: bool, palette: ComponentPalette) -> Element {
    let row = id.clone();
    let tile = MediaTile::new(id, title, palette).width(if state.mobile { TILE_WIDTH } else { CARD_WIDTH }).round(round);
    // On a desktop a tile is a card whose play button rises under the pointer.
    let tile = if state.mobile { tile } else {
        let sounding = state.player.playing && state.track_at(&row).zip(state.current()).is_some_and(|(track, current)| track.id == current.id);
        tile.padding(TILE_PAD).play(format!("tile-play:{row}"), t!("common-play")).playing(sounding)
    };
    let tile = match (artwork.as_ref().and_then(|url| state.artwork.get(url)), coming(state, artwork.as_ref(), palette)) {
        (Some(image), _) => tile.artwork(Element::raster_image(image.clone().cover())),
        (None, Some(pulse)) => tile.artwork(pulse),
        (None, None) => tile,
    };
    let element = match subtitle {
        Some(subtitle) => tile.subtitle(subtitle),
        None => tile,
    }
    .render();
    mobile_card_context(state, &row, element)
}

/// The part of the day an hour falls in: morning, afternoon, evening or the small hours.
fn day_part(hour: u32) -> usize {
    match hour {
        6..=11 => 0,
        12..=19 => 1,
        20..=23 => 2,
        _ => 3,
    }
}

/// "Tu domingo por la tarde".
fn moment_title(now: chrono::DateTime<chrono::Local>) -> String {
    use chrono::{Datelike, Timelike, Weekday};
    let day = match now.weekday() {
        Weekday::Mon => t!("desktop-day-monday"),
        Weekday::Tue => t!("desktop-day-tuesday"),
        Weekday::Wed => t!("desktop-day-wednesday"),
        Weekday::Thu => t!("desktop-day-thursday"),
        Weekday::Fri => t!("desktop-day-friday"),
        Weekday::Sat => t!("desktop-day-saturday"),
        Weekday::Sun => t!("desktop-day-sunday"),
    };
    let part = match day_part(now.hour()) {
        0 => t!("desktop-moment-morning"),
        1 => t!("desktop-moment-afternoon"),
        2 => t!("desktop-moment-evening"),
        _ => t!("desktop-moment-late"),
    };
    t!("desktop-moment-title", day = day.as_ref(), part = part.as_ref()).into_owned()
}

/// What a mix is called on its card and its page.
fn mix_name(mix: &Mix) -> String {
    match &mix.kind {
        MixKind::Moment => moment_title(chrono::Local::now()),
        MixKind::Artist(seed) => t!("desktop-mix-of", artist = seed.artist.as_str()).into_owned(),
        MixKind::Repeat => t!("desktop-mix-repeat").into_owned(),
    }
}

/// The line under its name: who else it plays.
fn mix_detail(mix: &Mix) -> String {
    let artists = mix.artists(3).join(", ");
    match &mix.kind {
        MixKind::Repeat => t!("desktop-mix-repeat-detail").into_owned(),
        _ if artists.is_empty() => songs(mix.tracks.len()),
        MixKind::Moment => t!("desktop-moment-detail", artists = artists.as_str()).into_owned(),
        MixKind::Artist(_) => t!("desktop-mix-with", artists = artists.as_str()).into_owned(),
    }
}

/// The same stripe for a mix wherever it shows, and a different one beside it.
fn mix_stripe(mix: &Mix) -> Color {
    let key = mix.key();
    MIX_STRIPES[key.bytes().fold(0usize, |hash, byte| hash.wrapping_mul(31).wrapping_add(byte as usize)) % MIX_STRIPES.len()]
}

/// `color` taken toward black by `share`.
fn shade(color: Color, share: f32) -> Color {
    let keep = |channel: u8| (f32::from(channel) * (1.0 - share)) as u8;
    Color::rgb(keep(color.r), keep(color.g), keep(color.b))
}

/// A mix's picture, made here where Spotify makes its own on its servers: the covers it
/// gathers (an artist's mix wears that artist's), what it is over a band of their colour,
/// and its stripe along the foot.
fn mix_cover(state: &State, mix: &Mix, side: f32, named: bool, palette: ComponentPalette) -> Element {
    let covers = mix.covers();
    let piece = |url: &String, side: f32| picture(state, &Some(url.clone()), side, false, palette).radius(0.0);
    let base = match (&mix.kind, covers.as_slice()) {
        (_, [a, b, c, d]) if !matches!(mix.kind, MixKind::Artist(_)) => {
            let half = side / 2.0;
            Element::column(vec![Element::row(vec![piece(a, half), piece(b, half)]), Element::row(vec![piece(c, half), piece(d, half)])])
        }
        (_, [first, ..]) => piece(first, side),
        _ => Element::panel(Element::spacer()).background(palette.selected),
    };
    let band = shade(covers.first().and_then(|url| state.tints.get(*url)).copied().unwrap_or(CHOSEN), 0.45);
    let (label, name) = match &mix.kind {
        MixKind::Moment => (t!("desktop-moment").into_owned(), mix_name(mix)),
        MixKind::Artist(seed) => (t!("desktop-mix").into_owned(), seed.artist.clone()),
        MixKind::Repeat => (t!("desktop-mix").into_owned(), t!("desktop-mix-repeat").into_owned()),
    };
    let words = Element::column(vec![
        Text::new(label.to_uppercase(), palette).font_size((side * 0.07).round().max(9.0)).truncate().render().font_family(FontFamily::SansBold),
        Text::new(name, palette).font_size((side * 0.1).round().max(12.0)).line_height((side * 0.12).round().max(14.0)).render().font_family(FontFamily::SansBold),
    ]).gap(2.0);
    let words = if named { words } else { Element::spacer() };
    let foot = Element::column(vec![Element::spacer().flex(1.0), words]).padding((side * 0.07).round())
        .vertical_gradient(vec![Color { a: 0, ..band }, Color { a: 200, ..band }, Color { a: 240, ..band }])
        .size(None, Some((side * 0.6).round()));
    let stripe = Element::spacer().size(None, Some((side * 0.04).round().max(3.0))).background(mix_stripe(mix));
    let over = Element::column(vec![Element::spacer().flex(1.0), foot, stripe]);
    Element::stack(vec![base, over]).size(Some(side), Some(side)).radius(limn::tokens().radius.control).clip_children()
}

/// A mix as a card of its shelf: its picture, its name, and who it plays.
fn mix_tile(state: &State, index: usize, mix: &Mix, palette: ComponentPalette) -> Element {
    let row = format!("made:{index}");
    let (width, inset) = if state.mobile { (TILE_WIDTH, 0.0) } else { (CARD_WIDTH, TILE_PAD) };
    let tile = MediaTile::new(&row, mix_name(mix), palette).width(width).artwork(mix_cover(state, mix, width - 2.0 * inset, true, palette)).subtitle(mix_detail(mix));
    let tile = if state.mobile { tile } else {
        let sounding = state.player.playing && state.plays_from(&mix.tracks);
        tile.padding(TILE_PAD).play(format!("tile-play:{row}"), t!("common-play")).playing(sounding)
    };
    tile.render()
}

/// The colours of each part of the day, from the left of the time of day's card to its right.
const MOMENT_COLOURS: [[Color; 3]; 4] = [
    [Color::rgb(232, 146, 52), Color::rgb(214, 88, 82), Color::rgb(108, 74, 168)],
    [Color::rgb(222, 84, 74), Color::rgb(196, 58, 128), Color::rgb(78, 62, 170)],
    [Color::rgb(74, 52, 168), Color::rgb(160, 48, 132), Color::rgb(28, 34, 92)],
    [Color::rgb(22, 30, 78), Color::rgb(64, 38, 128), Color::rgb(12, 78, 104)],
];

/// The time of day's mix, leading the home as a wide card: its picture, what it is called
/// and who it plays over the colours of the hour, and the play at its end.
fn moment_card(state: &State, index: usize, mix: &Mix, width: f32, palette: ComponentPalette) -> Element {
    let row = format!("made:{index}");
    let side = if state.mobile { 96.0 } else { 136.0 };
    let sounding = state.player.playing && state.plays_from(&mix.tracks);
    let copy = Element::column(vec![
        Text::new(t!("desktop-moment").to_uppercase(), palette).font_size(12.0).render().font_family(FontFamily::SansBold),
        Text::new(mix_name(mix), palette).font_size(if state.mobile { 22.0 } else { 28.0 }).truncate().render().font_family(FontFamily::SansBold),
        Text::new(mix_detail(mix), palette).ink(Color::rgba(255, 255, 255, 200)).truncate().render(),
    ]).gap(6.0).flex(1.0).centered_y();
    let round = ComponentPalette { surface: palette.text, text: SCHEME.canvas, hover: ROUND_HOVER, selected: ROUND_HELD, ..palette };
    let play = IconButton::new(format!("tile-play:{row}"), if sounding { Icon::PauseFill } else { Icon::PlayFill }, round)
        .variant(ButtonVariant::Subtle).size(ControlSize::Spacious).round().tooltip(if sounding { t!("common-pause") } else { t!("common-play") }).render();
    let mut parts = vec![mix_cover(state, mix, side, false, palette), copy];
    // While it plays, it shows what it sounds like.
    if sounding && !state.settings.reduce_motion && !state.mobile {
        let bars = LevelBars { id: "moment-wave".into(), levels: state.levels.to_vec(), bar: 5.0, gap: 4.0, color: palette.text, rest: Color::rgba(255, 255, 255, 110) };
        let width = state.levels.len() as f32 * 9.0 - 4.0;
        parts.push(Element::level_bars(bars).size(Some(width), Some(36.0)).centered_y());
    }
    parts.push(play.centered_y());
    let content = Element::row(parts).gap(if state.mobile { 12.0 } else { 20.0 }).padding(if state.mobile { 12.0 } else { 16.0 });
    Element::clickable(&row, content)
        .horizontal_gradient(moment_colours(state))
        .radius(RadiusRole::Card.px())
        .size(Some(width - 24.0), None)
}

/// How long a sheen takes to cross the time of day's card, and how much it lights it.
const MOMENT_SWAY_MS: u128 = 9_000;
const MOMENT_SHEEN: f32 = 0.22;

/// The colours across the time of day's card: the hour's three, blended in steps, with a soft
/// sheen drifting across them while the home is shown. Still for a screenshot and under
/// reduced motion.
fn moment_colours(state: &State) -> Vec<Color> {
    const STOPS: usize = 7;
    let [a, b, c] = MOMENT_COLOURS[day_part(chrono::Timelike::hour(&chrono::Local::now()))];
    let sheen = state.ambient().then(|| {
        let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
        // From just off the left edge to just off the right, then round again.
        (millis % MOMENT_SWAY_MS) as f32 / MOMENT_SWAY_MS as f32 * 1.6 - 0.3
    });
    (0..STOPS).map(|stop| {
        let at = stop as f32 / (STOPS - 1) as f32;
        let base = if at < 0.5 { crate::state::blend(a, b, at * 2.0) } else { crate::state::blend(b, c, at * 2.0 - 1.0) };
        let light = sheen.map_or(0.0, |sheen| MOMENT_SHEEN * (1.0 - ((at - sheen) / 0.3).powi(2)).max(0.0));
        crate::state::blend(base, Color::rgb(255, 255, 255), light)
    }).collect()
}

fn mobile_card_context(state: &State, row: &str, element: Element) -> Element {
    if !state.mobile { return element; }
    let Some(action) = state.card_context_action(row) else { return element; };
    let target = state.track_at(row).map_or_else(|| format!("tile-context:{row}"),
        |track| format!("tile-context:{row}:{}", track.id));
    // Touch and hold owns contextual actions; keep the cover and its primary
    // tap target clear. UIKit supplies the iOS menu, with this retained target
    // also serving keyboard and non-iOS context requests.
    ContextMenu::new(target, element,
        vec![ContextMenuEntry::Item(ContextMenuItem::new(&action, t!("common-options")))], menu_palette())
        .open_action(action).render()
}

fn release_tiles(state: &State, prefix: &str, releases: &[AlbumRef], palette: ComponentPalette) -> Vec<Element> {
    releases
        .iter()
        .enumerate()
        .map(|(index, album)| {
            // One line fits one fact: whose it is, or when it came out on its artist's page.
            let subtitle = album.artist.clone().or_else(|| (!album.year.is_empty()).then(|| album.year.clone()));
            tile(state, format!("{prefix}:{index}"), &album.title, subtitle, &album.artwork_url, false, palette)
        })
        .collect()
}

/// `element` in the middle of a row's height.
fn middle(element: Element) -> Element {
    Element::column(vec![Element::spacer().flex(1.0), element, Element::spacer().flex(1.0)])
}

/// One entry of a row's menu.
pub(crate) struct Action {
    /// What a click on it reports.
    pub(crate) id: String,
    pub(crate) label: std::borrow::Cow<'static, str>,
    icon: Icon,
    /// A second line: what the action leads to.
    pub(crate) detail: Option<String>,
}

/// What a row's menu offers for its track. `None` is a divider.
pub(crate) fn track_actions(state: &State, row: &str, track: &Track) -> Vec<Option<Action>> {
    let item = |action: &str, label: std::borrow::Cow<'static, str>, icon, detail| Some(Action { id: format!("do:{action}:{row}"), label, icon: Icon::Custom(icon), detail });
    let mut actions = vec![
        item("queue", t!("desktop-add-to-queue"), include_str!("../assets/icons/list-plus.svg"), None),
        item("next", t!("desktop-play-next"), include_str!("../assets/icons/list-start.svg"), None),
        item("share", t!("desktop-share"), include_str!("../assets/icons/share-2.svg"), None),
        Some({
            let liked = state.is_liked(&track.id);
            let icon = match (state.mobile, liked) { (true, true) => Icon::HeartFill, (true, false) => Icon::Heart, (false, _) => like_glyph(liked) };
            Action { id: format!("do:like:{row}"), label: if liked { t!("desktop-remove-like") } else { t!("common-like") }, icon, detail: None }
        }),
        None,
        item("add-new", t!("desktop-add-to-new-playlist"), include_str!("../assets/icons/list-plus.svg"), None),
    ];
    // Each of the user's playlists that does not hold the track yet.
    for playlist in state.collection.playlists.iter().filter(|playlist| !playlist.tracks.iter().any(|held| held.id == track.id)) {
        actions.push(Some(Action {
            id: format!("do:add-{}:{row}", playlist.id),
            label: t!("desktop-add-to"),
            icon: Icon::Playlist,
            detail: Some(playlist.name.clone()),
        }));
    }
    if matches!(state.page, Page::Playlist(_)) && row.starts_with("page-track:") {
        actions.push(item("remove", t!("desktop-remove-from-playlist"), include_str!("../assets/icons/circle-minus.svg"), None));
    }
    actions.push(None);
    if track.album_id.is_some() {
        actions.push(item("album", t!("desktop-go-to-album"), include_str!("../assets/icons/disc-3.svg"), track.album.clone()));
    }
    actions.push(item("artist", t!("desktop-go-to-artist"), include_str!("../assets/icons/user-round.svg"), Some(track.artist.clone())));
    actions.push(None);
    actions.push(if state.is_saved(&track.id) {
        item("unsave", t!("desktop-remove-download"), include_str!("../assets/icons/trash-2.svg"), None)
    } else {
        item("save", t!("desktop-download-offline"), DOWNLOAD, None)
    });
    actions
}

/// What plays says so with its own sound: a bar for each band of the engine's meter, on
/// springs. With movement reduced it is the still glyph.
fn wave(state: &State, id: String, palette: ComponentPalette) -> Element {
    if state.settings.reduce_motion {
        return Element::icon(Icon::Audio).foreground(palette.text).size(Some(16.0), Some(16.0));
    }
    let bars = LevelBars { id, levels: state.levels.to_vec(), bar: WAVE_BAR, gap: WAVE_GAP, color: palette.text, rest: palette.text };
    let width = state.levels.len() as f32 * (WAVE_BAR + WAVE_GAP) - WAVE_GAP;
    Element::level_bars(bars).size(Some(width), Some(16.0))
}

/// A track's cover as its row shows it. Over it, what plays wears its own sound; under
/// the pointer every cover says what a click on the row does: it pauses what plays and
/// plays the rest.
fn sleeve(state: &State, row: &str, track: &Track, palette: ComponentPalette) -> Element {
    let current = state.current().is_some_and(|current| current.id == track.id);
    let over = |content: Element| {
        Element::panel(content.centered()).background(COVER_SCRIM).radius(limn::tokens().radius.control).size(Some(COVER), Some(COVER))
    };
    let does = Element::icon(if current && state.player.playing { Icon::PauseFill } else { Icon::PlayFill }).foreground(palette.text);
    let mut layers = vec![cover(state, &track.artwork_url, COVER, palette)];
    if current {
        layers.push(over(wave(state, format!("wave:{row}"), palette)).conceal_on_hover(format!("sounding:{row}")));
    }
    layers.push(over(does).reveal_on_hover(format!("does:{row}")));
    Element::stack(layers)
}

/// The dots of a row with the menu they open, and the same actions for the menu the row
/// opens where it is pressed.
fn row_menu(state: &State, row: &str, track: &Track, palette: ComponentPalette) -> (Element, Vec<ContextMenuEntry>) {
    let open = state.menu.as_deref() == Some(row);
    let actions = track_actions(state, row, track);
    // As Spotify's: every action in rows of its own, the lists to add to in a menu beside it.
    let adds = |action: &Action| action.id.starts_with("do:add-");
    let mut lists = Vec::new();
    for action in actions.iter().flatten().filter(|action| adds(action)) {
        if action.id.starts_with("do:add-new:") {
            lists.extend([DropdownMenuEntry::Item(DropdownMenuItem::new(&action.id, t!("desktop-new-playlist")).icon(Icon::Custom(glyph::PLUS))), DropdownMenuEntry::Separator]);
        } else {
            lists.push(DropdownMenuEntry::Item(DropdownMenuItem::new(&action.id, action.detail.clone().unwrap_or_default())));
        }
    }
    let mut entries = Vec::new();
    for action in &actions {
        match action {
            Some(action) if action.id.starts_with("do:add-new:") => entries.push(DropdownMenuEntry::Item(
                DropdownMenuItem::new(format!("add-to:{row}"), t!("desktop-add-to-playlist")).icon(Icon::Custom(glyph::PLUS)).submenu(std::mem::take(&mut lists)),
            )),
            Some(action) if adds(action) => {}
            Some(action) => entries.push(DropdownMenuEntry::Item(DropdownMenuItem::new(&action.id, action.label.clone()).icon(action.icon))),
            None => entries.push(DropdownMenuEntry::Separator),
        }
    }
    let dots = IconButton::new(format!("more:{row}"), Icon::More, palette).round()
        .variant(ButtonVariant::Ghost)
        .size(ControlSize::Compact)
        .tooltip(t!("common-options"))
        .active(open)
        .render();
    let menu = DropdownMenu::new(dots, entries, open, "menu-close", menu_palette())
        .sizing(menu_sizing(state))
        .max_visible_items(MENU_ROWS)
        .scrollbar(spotify_bar())
        .scrollbar_visibility(SPOTIFY_BAR_SHOWS)
        .width(MENU_WIDTH)
        .placement(PopoverPlacement::BottomEnd)
        .render();
    let pressed = actions.into_iter().map(|action| match action {
        Some(action) => ContextMenuEntry::Item(ContextMenuItem::new(action.id, action.label).icon(action.icon)),
        None => ContextMenuEntry::Separator,
    });
    (menu, pressed.collect())
}

/// After a row's copy: that the track is kept on disk or on its way there, and how long
/// it lasts.
fn row_facts(state: &State, row: &str, track: &Track, palette: ComponentPalette) -> Vec<Element> {
    let mut facts = Vec::new();
    if state.saving.contains(&track.id) {
        facts.push(Spinner::new(format!("saving:{row}"), palette.muted).size(12.0).render().centered_y());
    } else if state.is_saved(&track.id) {
        facts.push(Element::icon(Icon::Custom(DOWNLOAD)).foreground(palette.muted).size(Some(14.0), Some(14.0)).centered_y());
    }
    if track.duration_ms > 0 {
        let length = Element::text(clock(track.duration_ms as f64 / 1000.0)).font_family(FontFamily::Monospace).font_size(12.0).foreground(palette.muted);
        facts.push(length.centered_x().size(Some(DURATION_WIDTH), None).centered_y());
    }
    facts
}

/// A track among what a search found: its cover, what it is and whose, and after them
/// what is known of it.
fn track_row(state: &State, row: String, track: &Track, subtitle: &str, palette: ComponentPalette) -> Element {
    if state.mobile { return mobile::track_row(state, row, track, subtitle, palette); }
    let (menu, pressed) = row_menu(state, &row, track, palette);
    let mut list_row = ListRow::new(&row, &track.title, palette)
        .sizing(TRACK_ROW)
        .leading(middle(sleeve(state, &row, track, palette)))
        .subtitle(subtitle)
        .title_fade(true)
        .radius(RadiusRole::Card);
    let facts = row_facts(state, &row, track, palette);
    if !facts.is_empty() {
        list_row = list_row.trailing(middle(Element::row(facts).gap(8.0)));
    }
    // The dots show while the pointer is on the row, or while their menu is open. A trailing
    // action sits flush with the row's edge: the row's own padding keeps them off it.
    let dots = Element::row(vec![menu, Element::spacer().size(Some(TRACK_ROW.padding), None)]).reveal_on_hover(format!("dots:{row}"));
    let element = list_row.trailing_action(format!("more:{row}"), middle(dots)).render().hover_region();
    ContextMenu::new(format!("context:{row}"), element, pressed, menu_palette()).render()
}

/// The width a page's table has in a pane `width` wide: the pane's, less the page's margins
/// and the room its scroll area keeps for its bar.
fn table_width(width: f32) -> f32 {
    width - 24.0 - 14.0
}

/// The narrowest a title gets, the table width under which the album gives its place,
/// and the room of the last column, for a row's dots.
const TITLE_MIN: f32 = 160.0;
const ALBUM_FROM: f32 = 520.0;
const MORE_WIDTH: f32 = 36.0;

/// A page's tracks as a table `width` wide: what each is, the album it is on and how
/// long it lasts, under heads that order them. `leaving` names the rows of a list that
/// can lose one, which then closes where it was.
/// How far apart a page's song rows stand, each row and the space after it.
pub(crate) fn track_pitch(state: &State) -> f32 {
    if state.mobile { MOBILE_TRACK_PITCH } else { TRACK_TABLE_ROW }
}
pub(crate) const MOBILE_TRACK_PITCH: f32 = 72.0;
const TRACK_TABLE_ROW: f32 = 56.0;
/// A list up to this long is built whole; a longer one only around what is in view.
const BUILT_WHOLE: usize = 120;
/// Rows are built in blocks this long, so scrolling rebuilds the list seldom.
const ROW_BLOCK: usize = 16;

/// Which of `count` rows `pitch` apart are built: those in view and a screen's worth on
/// either side. `window` is where the list's top was against its scroll area at the last
/// layout, and how tall the area is; a short list, or one not measured yet, is built whole.
pub(crate) fn track_range(count: usize, pitch: f32, window: Option<(f32, f32)>) -> std::ops::Range<usize> {
    let Some((top, height)) = window.filter(|_| count > BUILT_WHOLE && pitch > 0.0) else { return 0..count };
    let row = |at: f32| (at / pitch).max(0.0) as usize;
    let first = (row(-top - height) / ROW_BLOCK * ROW_BLOCK).min(count);
    let last = (row(-top + 2.0 * height) + 1).div_ceil(ROW_BLOCK) * ROW_BLOCK;
    first..last.clamp(first, count)
}

/// Rows of a song list with the ones `range` leaves out stood in for by space of their
/// height, `gap` apart, so the list is as tall and its rows where they would be.
fn windowed(rows: Vec<Element>, range: std::ops::Range<usize>, count: usize, pitch: f32, gap: f32) -> Element {
    let room = |rows: usize| Element::spacer().size(None, Some((rows as f32 * pitch - gap).max(0.0)));
    let mut column = Vec::with_capacity(rows.len() + 2);
    if range.start > 0 { column.push(room(range.start)); }
    column.extend(rows);
    if range.end < count { column.push(room(count - range.end)); }
    Element::column(column).gap(gap).anchor(TRACK_LIST)
}

/// The top of a page's song rows, as the Host measures it to know which are in view.
pub(crate) const TRACK_LIST: &str = "track-list";

fn track_table(state: &State, rows: &[(usize, &Track)], album: bool, leaving: Option<String>, width: f32, palette: ComponentPalette) -> Element {
    let count = rows.len();
    let range = track_range(count, track_pitch(state), state.track_window);
    if state.mobile {
        let built = rows[range.clone()].iter().map(|(place, track)| mobile::track_row(state, format!("page-track:{place}"), track, &track.artist, palette)).collect();
        return windowed(built, range, count, MOBILE_TRACK_PITCH, 4.0);
    }
    let first = range.start;
    let rows = &rows[range.clone()];
    let mut columns = vec![TableColumn::fixed("index", "#", NUMBER_WIDTH).align_end(), TableColumn::flexible("title", t!("common-title"), TITLE_MIN).sortable()];
    if album {
        columns.push(TableColumn::fixed("album", t!("common-album"), state.album_width).sortable().resizable(ALBUM_FROM - TITLE_MIN - 280.0).optional(state.album_shown).hide_below(ALBUM_FROM));
    }
    columns.push(TableColumn::fixed("like", "", LIKE_WIDTH));
    columns.push(TableColumn::fixed("duration", t!("desktop-duration"), DURATION_WIDTH + 22.0).icon(Icon::Clock).sortable().align_end().optional(state.duration_shown));
    columns.push(TableColumn::fixed("more", "", MORE_WIDTH));
    let mut menus = Vec::new();
    let listed: Vec<TableRow> = rows
        .iter()
        .enumerate()
        .map(|(position, (place, track))| {
            let position = first + position;
            let row = format!("page-track:{place}");
            let current = state.current().is_some_and(|current| current.id == track.id);
            // Numbered as they read, whatever their place in the list's own order.
            let sounding = current && state.player.playing;
            let at_rest = if sounding { wave(state, format!("wave:{row}"), palette) } else {
                Element::text((position + 1).to_string()).font_family(FontFamily::Monospace).font_size(12.0).foreground(if current { palette.text } else { palette.muted })
            };
            // Under the pointer the number says what a click on the row does, as Spotify's.
            let does = Element::icon(if sounding { Icon::PauseFill } else { Icon::PlayFill }).foreground(palette.text).size(Some(14.0), Some(14.0));
            let end = |element: Element| Element::row(vec![Element::spacer().flex(1.0), element.centered_y()]);
            let number = Element::stack(vec![end(at_rest).conceal_on_hover(format!("number:{row}")), end(does).reveal_on_hover(format!("does:{row}"))]);
            let copy = Element::column(vec![
                Text::new(&track.title, palette).font_size(TRACK_ROW.title_size).truncate().render(),
                Text::new(&track.artist, palette).font_size(TRACK_ROW.subtitle_size).muted().truncate().render(),
            ])
            .gap(TRACK_ROW.copy_gap)
            .flex(1.0)
            .centered_y();
            let mut cells = vec![number, Element::row(vec![cover(state, &track.artwork_url, COVER, palette).centered_y(), copy]).gap(TRACK_ROW.gap).flex(1.0)];
            if album {
                cells.push(Text::new(track.album.as_deref().unwrap_or_default(), palette).muted().truncate().render());
            }
            // As Spotify's: a song liked keeps its check; the others offer the plus under the pointer.
            let liked = state.is_liked(&track.id);
            let like = IconButton::new(format!("do:like:{row}"), like_glyph(liked), palette).round().variant(ButtonVariant::Ghost).size(ControlSize::Compact)
                .tooltip(if liked { t!("desktop-remove-like") } else { t!("common-like") }).ink(if liked { palette.text } else { palette.muted }).hover_ink(palette.text).render();
            cells.push(Element::row(vec![if liked { like } else { like.reveal_on_hover(format!("like:{row}")) }]).centered_y());
            cells.push(Element::row(row_facts(state, &row, track, palette)).gap(8.0));
            let (menu, pressed) = row_menu(state, &row, track, palette);
            menus.push(pressed);
            cells.push(menu.reveal_on_hover(format!("dots:{row}")));
            TableRow::new(&row, cells).controls([format!("more:{row}"), format!("do:like:{row}")])
        })
        .collect();
    let sizing = TableSizing { row_height: TRACK_TABLE_ROW, ..TableSizing::ROOMY };
    let (header, built) = Table::new("tracks", columns, width, palette)
        .rows(listed)
        .sort(state.page_sort.clone())
        .menu_palette(menu_palette())
        .radius(RadiusRole::Card)
        .sizing(sizing)
        .into_parts();
    let body = built.into_iter().zip(menus).zip(rows).map(|((element, pressed), (place, track))| {
        let element = ContextMenu::new(format!("context:page-track:{place}"), element.hover_region(), pressed, menu_palette()).render();
        match &leaving {
            Some(list) => Element::column(vec![element]).animate_height(format!("{list}:{}", track.id), false),
            None => element,
        }
    });
    let body = windowed(body.collect(), range.clone(), count, TRACK_TABLE_ROW, 0.0);
    // The rows a column remembers are named by where the built window starts: rows that only
    // scroll out of it are not removed and must not close, while one taken out of the list
    // closes where it was. limn forgets a name the frame no longer lays out.
    let body = match &leaving {
        Some(list) => body.keep_leaving(format!("{list}:rows:{}", range.start)),
        None => body,
    };
    // The heads stay in view under the page's bar while the rows pass beneath them. Their fill
    // comes with the bar, so in place they leave the page's wash whole.
    let header = Element::panel(header);
    let header = if state.page_stuck { header.background(SCHEME.canvas) } else { header }.sticky_top().sticky_top_inset(STUCK_BAR);
    Element::column(vec![header, body]).gap(sizing.header_gap)
}

/// Rows that pulse where a list is on its way.
fn loading(id: &str, palette: ComponentPalette) -> Element {
    let rows = SkeletonGroup::rows(id, palette, 8).row_height(TRACK_ROW.height.unwrap_or_default()).icon(true).into_rows();
    Element::column(rows).gap(2.0).padding(12.0).flex(1.0)
}

/// The main pane: home, the search, or the page the user walked to. A page fades in and
/// settles when it takes the pane.
fn pane(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let entering = match &state.page {
        // The results stay one page while they follow what is typed.
        Page::Search if state.at_home() => "page:home".to_owned(),
        Page::Search => "page:search".to_owned(),
        Page::Loading(_) => "page:loading".to_owned(),
        Page::Failed(..) => "page:failed".to_owned(),
        Page::Artist(artist) => format!("page:artist:{}", artist.id),
        Page::Album(album) => format!("page:album:{}", album.id),
        Page::Song(track) => format!("page:song:{}", track.id),
        Page::Downloads => "page:downloads".to_owned(),
        Page::Liked => "page:liked".to_owned(),
        Page::Playlist(id) => format!("page:playlist:{id}"),
        Page::Library => "page:library".to_owned(),
        Page::Playing => "page:playing".to_owned(),
        Page::Settings => "page:settings".to_owned(),
        Page::SettingsPart(part) => format!("page:settings:{}", part.slug()),
        Page::Lyrics => "page:lyrics".to_owned(),
        Page::News => "page:news".to_owned(),
        Page::Patches => "page:patches".to_owned(),
        Page::Browse => "page:browse".to_owned(),
        Page::Mix(mix) => format!("page:mix:{}", mix.key()),
        Page::Profile => "page:profile".to_owned(),
        Page::Equalizer => "page:equalizer".to_owned(),
    };
    let page = page(state, width, palette).animate_enter(entering);
    match stuck_bar(state, palette) {
        Some(bar) => Element::stack(vec![page, Element::column(vec![bar, Element::spacer().flex(1.0)])]),
        None => page,
    }
}

/// The top of a page's actions, as the Host measures it to know when they have scrolled
/// away under the page's bar.
pub(crate) const PAGE_ACTIONS: &str = "page-actions";
/// The bar a page keeps at its top once its actions are gone, as Spotify's.
const STUCK_BAR: f32 = 64.0;
/// Where the actions' top is when their play button reaches the bar's: there the bar takes over.
pub(crate) const STUCK_FROM: f32 = (STUCK_BAR - 36.0) / 2.0;

/// What stays at the top of a page once its actions have scrolled away: its play button and
/// its name over the page's own floor, with its wash fading down through it, so it reads as
/// the top of the page rather than a band of another colour.
fn stuck_bar(state: &State, palette: ComponentPalette) -> Option<Element> {
    if state.mobile || !state.page_stuck {
        return None;
    }
    let (title, artwork) = match &state.page {
        Page::Artist(artist) => (artist.name.clone(), artist.artwork_url.clone()),
        Page::Album(album) => (album.title.clone(), album.artwork_url.clone()),
        Page::Playlist(id) => {
            let playlist = state.collection.playlist(*id)?;
            (playlist.name.clone(), playlist.cover().cloned())
        }
        Page::Liked => (t!("common-liked-songs").into_owned(), None),
        Page::Downloads => (t!("common-downloads").into_owned(), None),
        Page::Mix(mix) => (mix_name(mix), mix.covers().first().map(|url| (*url).clone())),
        _ => return None,
    };
    let round = ComponentPalette { surface: palette.text, text: SCHEME.canvas, hover: ROUND_HOVER, selected: ROUND_HELD, ..palette };
    let play = IconButton::new("stuck-play", Icon::PlayFill, round).variant(ButtonVariant::Subtle).size(ControlSize::Spacious).round().tooltip(t!("common-play")).render();
    let title = Text::new(title, palette).font_size(24.0).truncate().render().flex(1.0).centered_y();
    let bar = Element::panel(Element::row(vec![play, title]).gap(12.0)).padding((STUCK_BAR - 36.0) / 2.0).background(SCHEME.canvas);
    let bar = match artwork.as_deref().and_then(|url| state.tints.get(url)) {
        Some(tint) => bar.top_background_fade(Color { a: 150, ..*tint }, STUCK_BAR),
        None => bar,
    };
    Some(bar.size(None, Some(STUCK_BAR)).animate_enter("stuck-bar"))
}

fn page(state: &State, width: f32, palette: ComponentPalette) -> Element {
    match &state.page {
        Page::Search if state.submitted.is_none() && !state.searching => home(state, width, palette),
        Page::Search => results(state, width, palette),
        // Each of its songs is looked for in the catalog, which takes a while.
        Page::Loading(Target::Import(_)) => {
            let (said, done) = match state.import_progress {
                Some((done, of)) if of > 0 => (t!("desktop-import-searching", done = done, of = of), done as f32 / of as f32),
                _ => (t!("desktop-import-reading"), 0.0),
            };
            let about = Text::new(t!("desktop-import-wait"), palette).role(TextRole::Caption).muted().render();
            let progress = Element::column(vec![Text::new(said, palette).render(), Progress::new(done, palette).height(6.0).render(), about]).gap(8.0);
            Element::column(vec![Element::panel(progress).padding(12.0), loading("opening", palette)])
        }
        Page::Loading(_) => loading("opening", palette),
        Page::Failed(_, message) => centered(
            Empty::new(t!("desktop-open-failed"), message, palette)
                .icon(Icon::WarningTriangle)
                .action("retry", t!("common-retry"), ButtonVariant::Secondary)
                .render(),
        ),
        Page::Artist(artist) => artist_page(state, artist, width, palette),
        Page::Album(album) => album_page(state, album, width, palette),
        Page::Song(track) => centered(
            Element::column(vec![
                media_card(state, "song-card", track, palette).centered_x(),
                Text::new(t!("desktop-song-tap"), palette).muted().render().centered_x(),
            ])
            .gap(12.0),
        ),
        Page::Downloads => downloads_page(state, width, palette),
        Page::Library => library(state, false, width, palette),
        Page::Playing => mobile::playing(state, width, palette),
        Page::Liked => liked_page(state, width, palette),
        Page::Lyrics => lyrics(state, width, 0.0, palette),
        Page::News => news_page(state, width, palette),
        Page::Patches => patches_page(state, width, palette),
        Page::Browse => browse_page(width, palette),
        Page::Mix(mix) => mix_page(state, mix, width, palette),
        Page::Profile => profile_page(state, palette),
        Page::Playlist(id) => match state.collection.playlist(*id) {
            Some(playlist) => playlist_page(state, playlist, width, palette),
            None => centered(Empty::new(t!("desktop-playlist-gone"), t!("desktop-playlist-gone-detail"), palette).icon(Icon::Playlist).render()),
        },
        Page::Settings | Page::SettingsPart(_) => settings_page(state, width, palette),
        Page::Equalizer => mobile::settings::equalizer(state, width),
    }
}

/// "1 canción", "12 canciones".
fn songs(count: usize) -> String {
    t!("common-songs", count = count).into_owned()
}

/// "45 min", "1 h 5 min".
fn length(minutes: u64) -> String {
    if minutes >= 60 { t!("common-hours-minutes", hours = minutes / 60, minutes = minutes % 60) } else { t!("common-minutes", minutes = minutes) }.into_owned()
}

/// One of the user's playlists: what it is, what to do with it, and its tracks in order.
fn playlist_page(state: &State, playlist: &Playlist, width: f32, palette: ComponentPalette) -> Element {
    if state.mobile { return mobile::details::playlist(state, playlist, width, palette); }
    let length = length(playlist.tracks.iter().map(|track| track.duration_ms).sum::<u64>() / 60_000);
    let picture = match playlist.cover() {
        Some(url) => cover(state, &Some(url.clone()), ALBUM_COVER, palette),
        None => Element::panel(Element::icon(Icon::Playlist).large_icon(48.0).foreground(palette.muted).centered())
            .background(CHOSEN)
            .radius(limn::tokens().radius.control)
            .size(Some(ALBUM_COVER), Some(ALBUM_COVER)),
    };
    let picture = choosable("playlist-picture", picture, ALBUM_COVER, false, palette);
    // What else a list can do waits behind its dots, as Spotify's.
    let item = |id: &str, label: std::borrow::Cow<'static, str>, icon: Icon| DropdownMenuEntry::Item(DropdownMenuItem::new(id, label).icon(icon));
    let copied = state.list_copied == Some(playlist.id);
    let exported = state.list_exported == Some(playlist.id);
    let mut entries = vec![item("playlist-picture", t!("desktop-choose-photo"), Icon::Custom(glyph::PENCIL))];
    if playlist.picture.is_some() {
        entries.push(item("playlist-picture-remove", t!("desktop-remove-photo"), Icon::Close));
    }
    entries.push(DropdownMenuEntry::Separator);
    if !playlist.tracks.is_empty() {
        entries.extend([
            item("playlist-share", if copied { t!("common-link-copied") } else { t!("desktop-share") }, Icon::Custom(include_str!("../assets/icons/share-2.svg"))),
            item("playlist-export", if exported { t!("desktop-songs-copied") } else { t!("desktop-copy-songs") }, Icon::Custom(include_str!("../assets/icons/upload.svg"))),
            DropdownMenuEntry::Separator,
        ]);
    }
    entries.push(item("playlist-delete", t!("common-delete"), Icon::Trash));
    let more = DropdownMenu::new(page_glyph("page-more", Icon::More, &t!("desktop-options-of", name = playlist.name.as_str()), false, palette), entries, state.page_more_open, "page-more-dismiss", menu_palette())
        .sizing(menu_sizing(state))
        .width(MENU_WIDTH)
        .placement(PopoverPlacement::BottomStart)
        .render();
    let actions = if playlist.tracks.is_empty() {
        // Nothing to play or to search yet.
        Element::row(vec![more])
    } else {
        page_actions(state, vec![more], palette)
    };
    let missed = state.imported.as_ref().filter(|(id, missed)| *id == playlist.id && !missed.is_empty()).map(|(_, missed)| {
        let said = t!("desktop-missed-count", count = missed.len());
        Button::new("import-missed", said, palette).variant(ButtonVariant::Ghost).size(ControlSize::Compact).render()
    });
    let mut about = vec![
        Text::eyebrow(if playlist.collaborative { t!("desktop-collaborative-playlist-eyebrow") } else { t!("desktop-playlist-eyebrow") }, palette).render(),
        Text::new(&playlist.name, palette).font_size(HERO_TITLE).truncate().render(),
        Text::new(format!("{} · {length}", songs(playlist.tracks.len())), palette).muted().render(),
    ];
    // What a list brought in from another app left behind, a click from being read.
    about.extend(missed.map(|missed| Element::row(vec![missed])));
    let header = Element::row(vec![picture, Element::column(about).gap(10.0).flex(1.0).centered_y()]).gap(20.0);
    let list = if playlist.tracks.is_empty() {
        Element::panel(
            Empty::new(t!("desktop-playlist-empty"), t!("desktop-playlist-empty-detail"), palette).icon(Icon::MusicNote).render().centered_x(),
        )
        .padding(24.0)
    } else {
        track_table(state, &state.page_rows(), true, Some(format!("playlist-row:{}", playlist.id)), table_width(width), palette)
    };
    let mut sections = vec![header, actions, list];
    sections.extend(suggested(state, playlist, width, palette));
    let cover = playlist.cover().cloned();
    page_scroll("playlist", washed(state, &cover, Element::column(sections).gap(16.0).padding(12.0)))
}

/// A mix the home made: its picture, what it is and who it plays, and its songs in order.
fn mix_page(state: &State, mix: &Mix, width: f32, palette: ComponentPalette) -> Element {
    if state.mobile { return mobile::details::mix(state, mix, width, palette); }
    let length = length(mix.tracks.iter().map(|track| track.duration_ms).sum::<u64>() / 60_000);
    let about = Element::column(vec![
        Text::eyebrow(if mix.kind == MixKind::Moment { t!("desktop-moment") } else { t!("desktop-mix") }, palette).render(),
        Text::new(mix_name(mix), palette).font_size(HERO_TITLE).truncate().render(),
        Text::new(mix_detail(mix), palette).muted().truncate().render(),
        Text::new(format!("{} · {length}", songs(mix.tracks.len())), palette).muted().render(),
    ])
    .gap(10.0)
    .flex(1.0)
    .centered_y();
    let header = Element::row(vec![mix_cover(state, mix, ALBUM_COVER, true, palette), about]).gap(20.0);
    let list = track_table(state, &state.page_rows(), true, None, table_width(width), palette);
    let cover = mix.covers().first().map(|url| (*url).clone());
    page_scroll("mix", washed(state, &cover, Element::column(vec![header, play_buttons(state, None, palette), list]).gap(16.0).padding(12.0)))
}

/// What goes with a playlist, ten at a time under it as Spotify offers it: its heading and
/// what it goes by, then rows as the list's own, with the album and an outlined Añadir at
/// the end; a row plays the ten from it, its button adds it to the end of the list, and the
/// text button under them, at the end, trades the ten for the next. Nothing until the catalog
/// has answered.
fn suggested(state: &State, playlist: &Playlist, width: f32, palette: ComponentPalette) -> Option<Element> {
    let shown = state.suggestions();
    if shown.is_empty() {
        return None;
    }
    let detail = if playlist.tracks.is_empty() { t!("common-recommended-listening") } else { t!("common-recommended-detail") };
    let heading = Element::column(vec![
        Text::new(t!("common-recommended"), palette).font_size(24.0).render().font_family(FontFamily::SansBold),
        Text::new(detail, palette).font_size(14.0).muted().render(),
    ]).gap(5.0);
    if state.mobile {
        return Some(Element::column(vec![heading, mobile::suggested(state, &shown[..shown.len().min(SUGGESTED_SHOWN)], palette)]).gap(12.0));
    }
    let columns = vec![
        TableColumn::flexible("title", t!("common-title"), TITLE_MIN),
        TableColumn::fixed("album", t!("common-album"), state.album_width).hide_below(ALBUM_FROM),
        TableColumn::fixed("add", "", 112.0).align_end(),
    ];
    // Spotify's small secondary button: a pill outlined in grey on nothing, 32 tall.
    let outlined = ComponentPalette { surface: Color::TRANSPARENT, hover: Color::rgba(255, 255, 255, 14), selected: Color::rgba(255, 255, 255, 28), border: Color::rgb(124, 124, 124), ..palette };
    let rows = shown.iter().take(SUGGESTED_SHOWN).enumerate().map(|(index, track)| {
        let row = format!("suggested:{index}");
        let copy = Element::column(vec![
            Text::new(&track.title, palette).font_size(TRACK_ROW.title_size).truncate().render(),
            Text::new(&track.artist, palette).font_size(TRACK_ROW.subtitle_size).muted().truncate().render(),
        ]).gap(TRACK_ROW.copy_gap).flex(1.0).centered_y();
        let add = format!("suggested-add:{index}");
        let button = Button::new(&add, t!("common-recommended-add"), outlined).variant(ButtonVariant::Outline)
            .sizing(limn::ButtonSizing { height: 32.0, font_size: 14.0, horizontal_padding: 16.0, ..ControlSize::Regular.button_sizing() }).round()
            .tooltip(t!("common-recommended-add-title", title = track.title.as_str())).render();
        TableRow::new(&row, vec![
            // Its cover plays it under the pointer and sounds while it plays, as the list's.
            Element::row(vec![sleeve(state, &row, track, palette).centered_y(), copy]).gap(TRACK_ROW.gap).flex(1.0),
            Text::new(track.album.as_deref().unwrap_or_default(), palette).muted().truncate().render(),
            Element::row(vec![Element::spacer().flex(1.0), button.centered_y()]),
        ]).controls([add])
    }).collect();
    // No heads: the rows read as more of the list above them.
    let (_, rows) = Table::new("suggested", columns, table_width(width), palette)
        .rows(rows)
        .radius(RadiusRole::Control)
        .sizing(TableSizing { row_height: TRACK_TABLE_ROW, ..TableSizing::ROOMY })
        .into_parts();
    let rows = rows.into_iter().map(Element::hover_region).collect();
    let refresh = Button::new("suggested-refresh", t!("common-recommended-refresh"), palette).variant(ButtonVariant::Ghost)
        .ink(palette.muted).size(ControlSize::Regular).round().render();
    // Thirty above it, as Spotify's, and twenty-four between its parts.
    Some(Element::column(vec![Element::spacer().size(None, Some(14.0)), heading, Element::column(rows), Element::row(vec![Element::spacer().flex(1.0), refresh])]).gap(24.0))
}

/// A size on disk as people read it.
fn megabytes(bytes: u64) -> String {
    let megabytes = bytes as f64 / 1_048_576.0;
    if megabytes >= 1024.0 { format!("{:.1} GB", megabytes / 1024.0) } else { format!("{megabytes:.0} MB") }
}

/// What the user can set, in the phone's cards: on a desktop a column of reading width with
/// its own rows (the output device, the side panels), each control beside its name while
/// there is room and under it when not.
fn settings_page(state: &State, width: f32, palette: ComponentPalette) -> Element {
    use mobile::settings::{action, arrange, cell, chevron, group, line, segmented, stacked, switch, value, GUTTER};
    use crate::state::SettingsPart;
    if state.mobile { return mobile::settings::page(state, width); }
    // The desktop's rows and words are the size of its other pages', not the phone's.
    let _dense = mobile::settings::Dense::on();
    // The whole of the pane, as its other pages.
    let across = width;
    let inner = (across - GUTTER * 4.0).max(0.0);
    let wide = inner >= 560.0;
    let span = if wide { inner.min(288.0) } else { inner };
    let white = Color::WHITE;
    let beside = |title: &str, detail: Option<&str>, control: Element| if wide { cell(line(title, detail, Some(control), white)) }
        else { stacked(line(title, detail, None, white), control) };

    let seconds = state.settings.crossfade;
    let fade = if seconds == 0.0 { t!("desktop-crossfade-off").into_owned() } else { format!("{seconds:.0} s") };
    let slider = Slider::new("setting:crossfade", seconds / CROSSFADE_MAX, SCHEME.on_raised()).steps(CROSSFADE_MAX as usize + 1).width(span).render();
    let (mixes, crossfade) = (t!("desktop-crossfade-detail"), t!("desktop-crossfade"));
    let mixes = Some(&*mixes);
    let crossfade = if wide { cell(line(&crossfade, mixes, Some(Element::row(vec![value(&fade).centered_y(), slider.centered_y()]).gap(12.0)), white)) }
        else { stacked(line(&crossfade, mixes, Some(value(&fade)), white), slider) };
    let transitions = group(&t!("common-settings-transitions"), vec![crossfade], "");
    let controls = group(&t!("common-settings-controls"), vec![
        switch(&t!("desktop-autoplay"), Some(&t!("desktop-autoplay-detail")), "setting:autoplay", state.settings.autoplay),
    ], "");

    let device = |id: &str, name: &str| DropdownOption::new(format!("setting:output:{id}"), name).selected(state.output.as_deref().unwrap_or_default() == id);
    let system_output = state.outputs.iter().find(|output| output.is_default)
        .map_or_else(|| t!("desktop-system-output").into_owned(), |output| t!("desktop-system-output-named", name = output.name.as_str()).into_owned());
    let devices = std::iter::once(device("", &system_output)).chain(state.outputs.iter().map(|output| {
        let name = if output.is_default { t!("desktop-output-default", name = output.name.as_str()).into_owned() } else { output.name.clone() };
        device(&output.id, &name)
    }));
    let missing = t!("desktop-output-missing");
    let playing_through = match &state.output {
        Some(id) => state.outputs.iter().find(|output| output.id == *id).map_or(&*missing, |output| output.name.as_str()),
        None => system_output.as_str(),
    };
    let output = Select::new("setting:output", playing_through, devices.collect(), state.output_open, "setting:output-dismiss", SCHEME.on_raised())
        .menu_palette(menu_palette())
        .sizing(menu_sizing(state))
        .accessibility_label(t!("desktop-output-device"))
        .size(ControlSize::Spacious)
        .width(if wide { inner.min(MENU_WIDTH + 40.0) } else { inner })
        .menu_width(if wide { inner.min(MENU_WIDTH + 40.0) } else { inner })
        .render();
    let output_status = if state.outputs_loading {
        t!("desktop-outputs-searching")
    } else if let Some(error) = &state.outputs_error {
        t!("desktop-outputs-error", error = error.as_str())
    } else if state.outputs.is_empty() {
        t!("desktop-outputs-none")
    } else {
        t!("desktop-outputs-count", count = state.outputs.len())
    };
    let mut connections = group(&t!("desktop-connections"), vec![
        beside(&t!("desktop-output-device"), Some(&t!("desktop-output-device-detail")), output),
        action("setting:outputs-refresh", &t!("desktop-refresh-outputs"), Some(&output_status), None, !state.outputs_loading, false),
    ], &t!("desktop-connections-footer"));
    if let Some(notice) = &state.output_notice {
        connections = Element::column(vec![connections, Alert::new(t!("desktop-audio-output"), notice, SCHEME.palette()).id("setting:output-notice").compact(true).render()]).gap(8.0);
    }

    let chosen = state.settings.quality;
    let ceiling = t!("desktop-quality-up-to", kbps = match chosen { Quality::Low => 50, Quality::Normal => 70, Quality::High => 140 });
    let streaming = group(&t!("common-settings-streaming"), vec![
        beside(&t!("desktop-quality"), Some(&ceiling), segmented("setting-quality", span, Quality::ALL.into_iter()
            .map(|(quality, name, label)| Tab::new(format!("setting:quality:{name}"), label, 0.0).active(quality == chosen)).collect())),
    ], &t!("desktop-quality-footer"));
    let sound = group(&t!("common-settings-sound"), vec![
        switch(&t!("desktop-normalize"), Some(&t!("desktop-normalize-detail")), "setting:normalize", state.settings.normalize),
        switch(&t!("desktop-mono"), Some(&t!("desktop-mono-detail")), "setting:mono", state.settings.mono),
        action("equalizer", &t!("common-equalizer"), None,
            Some(Element::row(vec![value(&equalizer::summary(state)).centered_y(), chevron().centered_y()]).gap(6.0)), true, false),
    ], "");

    let display = group(&t!("desktop-display"), vec![
        switch(&t!("desktop-show-library-setting"), Some(&t!("desktop-show-library-detail")), "setting:library-panel", state.library_open),
        switch(&t!("desktop-show-now-playing"), Some(&t!("desktop-show-now-playing-detail")), "setting:player-panel", state.panel_open),
        switch(&t!("desktop-clips"), Some(&t!("desktop-clips-detail")), "setting:clips", state.settings.clips),
        switch(&t!("desktop-reduce-motion"), Some(&t!("desktop-reduce-motion-detail")), "setting:motion", state.settings.reduce_motion),
    ], "");
    let language = group(&t!("common-language"), vec![
        beside(&t!("common-language"), Some(&t!("common-language-detail")), segmented("setting-language", span, Language::ALL.into_iter()
            .map(|language| Tab::new(format!("setting:language:{}", language.tag()), language.name(), 0.0).active(language == crate::i18n::current())).collect())),
    ], "");

    let (sort, view) = (state.settings.library_sort, state.settings.library_view);
    let library = group(&t!("common-settings-your-library"), vec![
        beside(&t!("desktop-sort"), None, segmented("setting-library-sort", span, vec![
            Tab::new("library-sort:added", t!("desktop-sort-recent"), 0.0).active(sort == Sort::Added),
            Tab::new("library-sort:alphabetical", "A–Z", 0.0).active(sort == Sort::Alphabetical),
            Tab::new("library-sort:creator", t!("common-creator"), 0.0).active(sort == Sort::Creator),
        ])),
        beside(&t!("desktop-view"), None, segmented("setting-library-view", span, vec![
            Tab::new("library-view:compact", t!("desktop-view-compact"), 0.0).active(view == LibraryView::Compact),
            Tab::new("library-view:list", t!("desktop-view-list"), 0.0).active(view == LibraryView::List),
            Tab::new("library-view:grid", t!("desktop-view-grid"), 0.0).active(view == LibraryView::Grid),
        ])),
    ], "");
    let bring = group(&t!("common-settings-bring"), vec![
        action("import-open", &t!("desktop-import-settings"), Some(&t!("desktop-import-settings-detail")), Some(chevron()), true, false),
        action("account-handoff", &t!("desktop-handoff"), Some(&t!("desktop-handoff-about")), Some(chevron()), true, false),
    ], "");

    let about = group(&t!("desktop-about"), vec![
        cell(line(&t!("desktop-version"), None, Some(value(env!("CARGO_PKG_VERSION"))), white)),
        action("patches", &t!("common-patches"), Some(&t!("common-patches-detail")), Some(chevron()), true, false),
        cell(line(&t!("desktop-licenses"), Some(&t!("desktop-licenses-detail")), None, white)),
    ], "");

    // What cannot be undone sits last, each behind a confirmation.
    let downloads = format!("{} · {}", songs(state.saved.len()), megabytes(state.saved_bytes));
    let erase = group(&t!("desktop-storage"), vec![
        action("downloads-clear", &t!("desktop-delete-downloads"), Some(&downloads), None, !state.saved.is_empty(), true),
        action("recent-clear", &t!("desktop-clear-recent"), Some(&t!("desktop-clear-recent-settings-detail")), None, !state.collection.recent.is_empty(), true),
    ], "");

    use SettingsPart::*;
    let sections = vec![(Playback, transitions), (Playback, controls), (Devices, connections), (Audio, streaming), (Audio, sound),
        (Display, display), (Display, language), (Library, library), (Library, bring), (About, about), (Storage, erase)];
    let column = Element::column(arrange(state, sections, true, palette)).gap(28.0).padding(GUTTER).size(Some(across), None);
    page_scroll("settings", column)
}

/// The results of a search, as Spotify's: chips that show all of them or one kind; all is the
/// top result beside the first songs, then a shelf of artists and one of albums; one kind is
/// the songs as a list's table, or the artists or albums as a grid. A chip with nothing found
/// is left out. What was found stays while what is typed next is looked for; only the first
/// search, with nothing to show yet, pulses in the shapes to come.
fn results(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let chip = |key: &str, label: &str, value| Button::new(format!("search-filter:{key}"), label, palette)
        .variant(if state.search_filter == value { ButtonVariant::Default } else { ButtonVariant::Secondary })
        .sizing(ControlSize::Regular.button_sizing()).round().render();
    let found = &state.found;
    let coming = state.searching && found.tracks.is_empty() && found.artists.is_empty() && found.albums.is_empty();
    let mut filters = vec![chip("all", &t!("common-all"), SearchFilter::All)];
    for (key, label, value, none) in [
        ("tracks", t!("common-songs-heading"), SearchFilter::Tracks, found.tracks.is_empty()),
        ("artists", t!("common-artists"), SearchFilter::Artists, found.artists.is_empty()),
        ("albums", t!("common-albums"), SearchFilter::Albums, found.albums.is_empty()),
    ] {
        if coming || !none || state.search_filter == value { filters.push(chip(key, &label, value)); }
    }
    let filters = Element::row(filters).gap(8.0);
    let body = if let (Some(error), false) = (&state.search_error, coming) {
        // Without a network there is still what was kept for it.
        let empty = Empty::new(t!("desktop-search-failed"), error, palette).icon(Icon::WarningTriangle);
        let empty = if state.saved.is_empty() { empty } else { empty.action("downloads", t!("desktop-see-downloads"), ButtonVariant::Secondary) };
        empty.render().centered_x().padding(48.0)
    } else if !coming && found.tracks.is_empty() && found.artists.is_empty() && found.albums.is_empty() {
        Empty::new(t!("common-no-results"), t!("desktop-try-other-words"), palette).icon(Icon::MusicNote).render().centered_x().padding(48.0)
    } else {
        let grid = |tiles: Vec<Element>| Element::row(tiles).wrap().gap(0.0);
        let artists = || found.artists.iter().enumerate().map(|(index, artist)| {
            tile(state, format!("artist:{index}"), &artist.name, Some(t!("common-artist").into_owned()), &artist.artwork_url, true, palette)
        }).collect::<Vec<_>>();
        match state.search_filter {
            SearchFilter::All => {
                let mut sections = vec![top_result(state, width, coming, palette)];
                match (coming, found.artists.is_empty()) {
                    (true, _) => sections.push(shelf(state, "found-artists", &t!("common-artists"), coming_tiles(state, true, palette), palette)),
                    (false, false) => sections.push(shelf(state, "found-artists", &t!("common-artists"), artists(), palette)),
                    (false, true) => {}
                }
                match (coming, found.albums.is_empty()) {
                    (true, _) => sections.push(shelf(state, "found-albums", &t!("common-albums"), coming_tiles(state, false, palette), palette)),
                    (false, false) => sections.push(shelf(state, "found-albums", &t!("common-albums"), release_tiles(state, "album", &found.albums, palette), palette)),
                    (false, true) => {}
                }
                Element::column(sections).gap(24.0)
            }
            SearchFilter::Tracks if coming => loading("searching", palette).padding(0.0),
            SearchFilter::Tracks => track_table(state, &state.page_rows(), true, None, table_width(width), palette),
            SearchFilter::Artists if coming => grid(coming_tiles(state, true, palette)),
            SearchFilter::Artists => grid(artists()),
            SearchFilter::Albums if coming => grid(coming_tiles(state, false, palette)),
            SearchFilter::Albums => grid(release_tiles(state, "album", &found.albums, palette)),
        }
    };
    // The chips stay under the page's bar while the results pass beneath them.
    let filters = Element::panel(filters.padding(12.0)).background(SCHEME.canvas).sticky_top();
    page_scroll("results", Element::column(vec![filters, body.padding(12.0)]))
}

/// The top result beside the first songs: the first artist when their name holds the search, else
/// the first song, else whatever was found first. A card as Spotify's, lighter under the
/// pointer, with its picture, its name and what it is, and a play that rises there. In a
/// narrow pane the songs go under it.
fn top_result(state: &State, width: f32, coming: bool, palette: ComponentPalette) -> Element {
    const PICTURE: f32 = 92.0;
    const SONGS: usize = 4;
    let found = &state.found;
    let card = |content: Element| Element::panel(content.padding(20.0)).background(Color::rgb(24, 24, 24)).hover_background(Color::rgb(40, 40, 40))
        .radius(8.0).size(None, Some(PICTURE + 40.0 + 76.0));
    let (card, songs) = if coming {
        let pulse = ComponentPalette { raised: palette.selected, ..palette };
        let lines = Element::column(vec![
            limn::Skeleton::line("cover-coming", pulse, 220.0).height(28.0).render(),
            limn::Skeleton::line("cover-coming", pulse, 110.0).render(),
        ]).gap(12.0);
        let card = card(Element::column(vec![limn::Skeleton::block("cover-coming", pulse, PICTURE, PICTURE).radius(PICTURE / 2.0).render(), lines]).gap(20.0));
        let rows = SkeletonGroup::rows("searching", palette, SONGS).row_height(TRACK_ROW.height.unwrap_or_default()).icon(true).into_rows();
        (card, Element::column(rows).gap(2.0))
    } else {
        let query = state.submitted.as_deref().unwrap_or_default().to_lowercase();
        let named = found.artists.first().filter(|artist| artist.name.to_lowercase().contains(&query));
        let top = match (named, found.tracks.first()) {
            (Some(artist), _) => Some(("artist:0".to_owned(), artist.name.as_str(), t!("common-artist").into_owned(), &artist.artwork_url, true)),
            (None, Some(track)) => Some(("track:0".to_owned(), track.title.as_str(), format!("{} • {}", t!("common-song"), track.artist), &track.artwork_url, false)),
            (None, None) => found.artists.first().map(|artist| ("artist:0".to_owned(), artist.name.as_str(), t!("common-artist").into_owned(), &artist.artwork_url, true))
                .or_else(|| found.albums.first().map(|album| ("album:0".to_owned(), album.title.as_str(), t!("common-album").into_owned(), &album.artwork_url, false))),
        };
        let Some((id, title, kind, artwork, round)) = top else { return Element::spacer() };
        let sounding = state.player.playing && state.track_at(&id).zip(state.current()).is_some_and(|(track, current)| track.id == current.id);
        let play_palette = ComponentPalette { surface: palette.text, text: SCHEME.canvas, hover: ROUND_HOVER, selected: ROUND_HELD, ..palette };
        let play = IconButton::new(format!("tile-play:{id}"), if sounding { Icon::PauseFill } else { Icon::PlayFill }, play_palette)
            .variant(ButtonVariant::Subtle).size(ControlSize::Spacious).round().tooltip(t!("common-play")).render();
        let play = if sounding { play } else { play.reveal_on_hover(format!("top-play:{id}")) };
        let copy = Element::column(vec![
            Text::new(title, palette).font_size(28.0).truncate().render().font_family(FontFamily::SansBold),
            Text::new(kind, palette).muted().truncate().render(),
        ]).gap(6.0);
        let picture = cover(state, artwork, PICTURE, palette).radius(if round { PICTURE / 2.0 } else { 6.0 })
            .shadow(Color::rgba(0, 0, 0, 128), limn::Point::new(0.0, 8.0), 24.0, 0.0);
        let content = Element::column(vec![picture, Element::row(vec![copy.flex(1.0), play.centered_y()]).gap(12.0)]).gap(20.0);
        let card = Element::clickable(id, card(content)).hover_region();
        let rows = found.tracks.iter().take(SONGS).enumerate().map(|(index, track)| track_row(state, format!("track:{index}"), track, &track.artist, palette));
        (card, Element::column(rows.collect()).gap(2.0))
    };
    let top = Element::column(vec![heading(&t!("common-top-result"), palette), card]).gap(8.0);
    if !coming && found.tracks.is_empty() {
        return top;
    }
    let songs = Element::column(vec![heading(&t!("common-songs-heading"), palette), songs]).gap(8.0);
    if width < 760.0 {
        return Element::column(vec![top, songs]).gap(24.0);
    }
    Element::row(vec![top.flex(2.0), songs.flex(3.0)]).gap(24.0)
}

/// The shuffle's three steps as its button shows them: the icon of the one it is on, and
/// what a press does next.
fn shuffle_step(state: &State) -> (Icon, std::borrow::Cow<'static, str>) {
    let (plain, smart) = if state.mobile { (Icon::Shuffle, Icon::Custom(SMART_SHUFFLE)) } else { (Icon::Custom(glyph::SHUFFLE), Icon::Custom(glyph::SMART_SHUFFLE)) };
    match (state.player.shuffle, state.player.smart) {
        (false, _) => (plain, t!("desktop-shuffle-on")),
        (true, false) => (plain, t!("desktop-smart-shuffle-on")),
        (true, true) => (smart, t!("desktop-shuffle-off")),
    }
}

/// A page's actions, in a row of their own under its header: its play, the one filled
/// control, the step of the shuffle, the page's `own`, and the search of its tracks at the
/// far end.
fn page_actions(state: &State, own: Vec<Element>, palette: ComponentPalette) -> Element {
    let round = ComponentPalette { surface: palette.text, text: SCHEME.canvas, hover: ROUND_HOVER, selected: ROUND_HELD, ..palette };
    let play = if state.mobile {
        mobile::controls::playback("page-play", false, 56.0, 28.0, palette)
    } else {
        // A glyph of 20, a step up from a button's own, as the disc is.
        let glyph = Element::icon(Icon::PlayFill).large_icon(20.0).foreground(round.text).size(Some(20.0), Some(20.0)).centered();
        let button = Element::clickable("page-play", glyph)
            .background(round.surface).hover_background(round.hover).pressed_background(round.selected)
            .radius(PAGE_PLAY / 2.0).size(Some(PAGE_PLAY), Some(PAGE_PLAY))
            .accessible(limn::Access::new(limn::Role::Button).name(t!("common-play")));
        limn::Tooltip::new(button, "page-play", t!("common-play"), palette).render()
    };
    let (icon, hint) = shuffle_step(state);
    let shuffle = if state.mobile {
        let action = mobile::controls::action("page-shuffle", icon, &hint, 22.0, palette);
        if state.player.shuffle { action.background(palette.selected) } else { action }
    } else {
        page_glyph("page-shuffle", icon, &hint, state.player.shuffle, palette)
    };
    let mut actions = vec![play, shuffle.centered_y()];
    actions.extend(own.into_iter().map(Element::centered_y));
    if state.mobile {
        actions.push(Element::spacer().flex(1.0));
        return Element::column(vec![Element::row(actions).gap(12.0), Element::row(page_search(state, palette)).gap(8.0)]).gap(8.0);
    }
    actions.extend(page_search(state, palette));
    Element::row(actions).gap(8.0).anchor(PAGE_ACTIONS)
}

/// The actions of a page the library can hold: `keep` names keeping it, before and after.
fn play_buttons(state: &State, keep: Option<(&str, &str)>, palette: ComponentPalette) -> Element {
    let keep = keep.map(|(keep, kept)| {
        let held = state.keeps_page();
        let label = if held { kept } else { keep };
        // An artist is followed by name, in an outlined pill; a record is kept with the glyph
        // a song is liked with.
        if matches!(state.page, Page::Artist(_)) {
            Button::new("keep", label, palette).variant(ButtonVariant::Outline).size(ControlSize::Compact).round().render()
        } else {
            page_glyph("keep", like_glyph(held), label, held, palette)
        }
    });
    page_actions(state, keep.into_iter().collect(), palette)
}

/// A page's action: a glyph of 24 on its own in full ink, a disc of light under the pointer,
/// and the dot of the playback bar under it while it is on.
fn page_glyph(id: &str, icon: Icon, hint: &str, on: bool, palette: ComponentPalette) -> Element {
    // A button's glyph is drawn at 16; this one is an icon of its own.
    let glyph = Element::icon(icon).large_icon(PAGE_GLYPH).foreground(palette.text).size(Some(PAGE_GLYPH), Some(PAGE_GLYPH)).centered();
    let side = PAGE_GLYPH + 16.0;
    let button = Element::clickable(id.to_owned(), glyph)
        .hover_background(GLYPH_HOVER)
        .pressed_background(GLYPH_HELD)
        .radius(side / 2.0)
        .size(Some(side), Some(side))
        .accessible(limn::Access::new(limn::Role::Button).name(hint));
    let button = limn::Tooltip::new(button, id, hint, palette).render();
    if !on { return button; }
    let dot = Element::spacer().size(Some(ACTIVE_DOT), Some(ACTIVE_DOT)).background(palette.text).radius(ACTIVE_DOT / 2.0);
    Element::stack(vec![button, Element::column(vec![Element::spacer().flex(1.0), dot.centered_x()])])
}

const PAGE_GLYPH: f32 = 24.0;
/// The cross inside a field's end.
const FIELD_CROSS: f32 = 24.0;
const PAGE_PLAY: f32 = 48.0;
const GLYPH_HOVER: Color = Color::rgba(255, 255, 255, 26);
const GLYPH_HELD: Color = Color::rgba(255, 255, 255, 46);

/// The search of the page's tracks, at the far end of its actions: a magnifier that grows
/// into a field, with the cross that shuts it where the magnifier was.
fn page_search(state: &State, palette: ComponentPalette) -> Vec<Element> {
    let open = state.page_search_slide.value(std::time::Instant::now()).clamp(0.0, 1.0);
    let toggle = |icon: Icon, hint: &str| {
        if state.mobile { mobile::icon("page-search", icon, hint, palette).radius(22.0).centered_y() }
        else { header_button("page-search", icon, TooltipSpec::new(hint), palette).render().centered_y() }
    };
    let mut parts = vec![Element::spacer().flex(1.0)];
    if open > 0.0 {
        let width = PAGE_SEARCH_SHUT + (PAGE_SEARCH_WIDTH - PAGE_SEARCH_SHUT) * open;
        let field = SearchField::new("page-query", &state.page_search, t!("desktop-search-page"), SCHEME.on_raised())
            .size(ControlSize::Compact)
            .width(width)
            .render();
        if state.mobile {
            // On a phone the cross that shuts it is a plain one inside its end, as the search's.
            let field = mobile::inside_cross(field.size(Some(width), Some(44.0)).radius(22.0), "page-search", &t!("desktop-close-search"), palette);
            parts.push(field.size(Some(width), Some(44.0)).centered_y().shrink_to_fit());
        } else {
            let field = field.radius(ControlSize::Compact.height() / 2.0);
            // The cross that shuts it stands inside its end, as Spotify's: its ink lights under
            // the pointer, with no disc of its own over the field.
            let cross = Element::icon_button("page-search".to_owned(), Icon::Custom(glyph::CLOSE), "")
                .foreground(palette.muted).hover_foreground(palette.text).pressed_foreground(palette.text)
                .size(Some(FIELD_CROSS), Some(FIELD_CROSS))
                .accessible(limn::Access::new(limn::Role::Button).name(t!("desktop-close-search")));
            let shut = Element::row(vec![limn::Tooltip::new(cross, "page-search", t!("desktop-close-search"), palette).render(), Element::spacer().size(Some(4.0), None)]).centered_y();
            let field = Element::stack(vec![field, Element::row(vec![Element::spacer().flex(1.0), shut])])
                .centered_y()
                // In a narrow pane it makes do with what the row has left.
                .shrink_to_fit();
            parts.push(field);
        }
    } else {
        parts.push(toggle(if state.mobile { Icon::Search } else { Icon::Custom(glyph::SEARCH) }, &t!("desktop-search-page")));
    }
    // Beside it, the order the page's tracks are listed in.
    let sorted = state.page_sort.as_ref().map_or("own", |sort| sort.key.as_str());
    let order = |key: &str, label: &str| DropdownMenuEntry::Item(DropdownMenuItem::new(format!("page-sort:{key}"), label).selected(sorted == key));
    let orders = vec![order("own", &t!("desktop-sort-list-order")), order("title", &t!("common-title")), order("album", &t!("common-album")), order("duration", &t!("desktop-duration"))];
    let button = if state.mobile { mobile::icon("page-sort", Icon::Sort, &t!("desktop-sort-button"), palette).radius(22.0) }
        else { header_button("page-sort", Icon::Sort, TooltipSpec::new(t!("desktop-sort-button")), palette).active(state.sort_open).render() };
    parts.push(DropdownMenu::new(button, orders, state.sort_open, "page-sort-close", menu_palette()).sizing(menu_sizing(state)).width(SORT_MENU_WIDTH).placement(PopoverPlacement::BottomEnd).render().centered_y());
    parts
}

const SORT_MENU_WIDTH: f32 = 200.0;

fn artist_page(state: &State, artist: &Artist, width: f32, palette: ComponentPalette) -> Element {
    if state.mobile { return mobile::details::artist(state, artist, width, palette); }
    let mut about = vec![Text::new(&artist.name, palette).font_size(HERO_TITLE).truncate().render()];
    if let Some(subscribers) = &artist.subscribers {
        about.push(Text::new(t!("desktop-subscribers", subscribers = subscribers.as_str()), palette).muted().render());
    }
    let header = Element::row(vec![
        cover(state, &artist.artwork_url, ARTIST_PICTURE, palette).radius(ARTIST_PICTURE / 2.0),
        // The copy gives way in a narrow pane; the picture keeps its size.
        Element::column(about).gap(10.0).flex(1.0).centered_y(),
    ])
    .gap(20.0);
    let mut sections = vec![header, play_buttons(state, Some((&t!("common-follow"), &t!("common-following"))), palette)];
    if !artist.songs.is_empty() {
        sections.extend([heading(&t!("common-songs-heading"), palette), track_table(state, &state.page_rows(), true, None, table_width(width), palette)]);
    }
    if !artist.albums.is_empty() {
        sections.push(shelf(state, "artist-albums", &t!("common-albums"), release_tiles(state, "page-album", &artist.albums, palette), palette));
    }
    if !artist.singles.is_empty() {
        let tiles = release_tiles(state, "page-single", &artist.singles, palette);
        sections.push(shelf(state, "artist-singles", &t!("desktop-singles"), tiles, palette));
    }
    page_scroll("artist", washed(state, &artist.artwork_url, Element::column(sections).gap(12.0).padding(12.0)))
}

fn downloads_page(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let failure = state.save_error.as_ref().map(|error| Alert::new(t!("desktop-download-failed"), error, palette).render());
    if state.saved.is_empty() && state.saving.is_empty() {
        let empty = Empty::new(t!("desktop-downloads-empty"), t!("desktop-downloads-empty-detail"), palette)
            .icon(Icon::Custom(DOWNLOAD))
            .render();
        return Element::column(failure.into_iter().chain([centered(empty)]).collect()).padding(12.0).flex(1.0);
    }
    let count = match (state.saved.len(), state.saving.len()) {
        (saved, 0) => t!("desktop-downloads-count", count = saved),
        (saved, saving) => t!("desktop-downloads-saving", saved = saved, saving = saving),
    };
    let mut sections = vec![Element::column(vec![
        Text::new(t!("common-downloads"), palette).font_size(HERO_TITLE).render(),
        Text::new(count, palette).muted().render(),
        play_buttons(state, None, palette),
    ])
    .gap(10.0)];
    sections.extend(failure);
    // A download that is removed closes where it was.
    sections.push(track_table(state, &state.page_rows(), true, Some("saved-row".into()), table_width(width), palette));
    page_scroll("downloads", Element::column(sections).gap(16.0).padding(12.0))
}

/// The tracks marked with the heart, the last one first.
fn liked_page(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let liked = &state.collection.liked;
    if liked.is_empty() {
        return centered(Empty::new(t!("desktop-liked-empty"), t!("desktop-liked-empty-detail"), palette).icon(Icon::Heart).render());
    }
    let header = Element::column(vec![
        Text::new(t!("common-liked-songs"), palette).font_size(HERO_TITLE).render(),
        Text::new(songs(liked.len()), palette).muted().render(),
        play_buttons(state, None, palette),
    ])
    .gap(10.0);
    // A track that loses its heart closes where it was.
    let list = track_table(state, &state.page_rows(), true, Some("liked-row".into()), table_width(width), palette);
    page_scroll("liked", Element::column(vec![header, list]).gap(16.0).padding(12.0))
}

fn album_page(state: &State, album: &Album, width: f32, palette: ComponentPalette) -> Element {
    if state.mobile { return mobile::details::album(state, album, width, palette); }
    let length = length(album.tracks.iter().map(|track| track.duration_ms).sum::<u64>() / 60_000);
    let count = songs(album.tracks.len());
    let facts: Vec<&str> = [album.year.as_str(), count.as_str(), length.as_str()].into_iter().filter(|fact| !fact.is_empty()).collect();
    let header = Element::row(vec![
        cover(state, &album.artwork_url, ALBUM_COVER, palette),
        Element::column(vec![
            Text::new(&album.title, palette).font_size(HERO_TITLE).truncate().render(),
            Link::new("page-artist", &album.artist, palette).ink(palette.text).font_size(14.0).render(),
            Text::new(facts.join(" · "), palette).muted().render(),
        ])
        .gap(10.0)
        .flex(1.0)
        .centered_y(),
    ])
    .gap(20.0);
    // Every track is of this album: its table needs no column to say so.
    let tracks = track_table(state, &state.page_rows(), false, None, table_width(width), palette);
    let sections = vec![header, play_buttons(state, Some((&t!("desktop-save"), &t!("desktop-in-library"))), palette), tracks];
    page_scroll("album", washed(state, &album.artwork_url, Element::column(sections).gap(16.0).padding(12.0)))
}

/// A panel's head: what it shows, and a muted line about it.
fn panel_header(title: &str, subtitle: &str, palette: ComponentPalette) -> Element {
    let copy = Element::column(vec![Text::new(title, palette).font_size(20.0).truncate().render(), Text::new(subtitle, palette).muted().truncate().render()]).gap(2.0);
    // A panel is shut from its own header.
    let shut = header_button("toggle-panel", Icon::Custom(glyph::PANEL_RIGHT), TooltipSpec::new(t!("desktop-hide-panel")), palette).render();
    inset(Element::row(vec![copy.flex(1.0), shut]))
}

/// A card of the panel of what plays: a step above it, with a margin of its own.
fn panel_card(parts: Vec<Element>, width: f32) -> Element {
    Element::panel(Element::column(parts).gap(8.0).padding(16.0)).background(CHOSEN).radius(RadiusRole::Card.px()).size(Some(width), None)
}

/// How many lines of the lyrics the panel of what plays shows, how tall each is and the space between two.
const LYRICS_SHOWN: usize = 5;
const LYRICS_LINE: f32 = 24.0;
const LYRICS_GAP: f32 = 8.0;

/// The lines around the one being sung, and the way to all of them. As on the phone, every
/// line is there and slides up under a window so the one sung rests second (the Host
/// measures where it is); each keeps its own ink, so it brightens and dims as it passes.
fn lyrics_preview(state: &State, width: f32, palette: ComponentPalette) -> Option<Element> {
    let lines = state.lines();
    let sung = state.active_line().map_or(-1, |index| index as i64);
    let raised = ComponentPalette { surface: CHOSEN, ..palette };
    let mut parts = vec![Text::eyebrow(t!("desktop-lyrics-eyebrow"), raised).render()];
    let rows = lines.iter().enumerate().map(|(index, line)| {
        let ink = Color { a: (255.0 * lyric_focus(index as i64 - sung).0) as u8, ..raised.text };
        Text::new(if line.text.trim().is_empty() { "♪" } else { &line.text }, raised)
            .font_size(16.0).line_height(LYRICS_LINE).ink(ink).render()
            .font_family(FontFamily::SansBold).animate_foreground(format!("lyric-card-ink:{index}"))
            .anchor(format!("lyric-card:{index}"))
    }).collect();
    let slide = limn::Point::new(0.0, -state.lyric_card_offset(std::time::Instant::now()));
    let height = LYRICS_SHOWN as f32 * LYRICS_LINE + (LYRICS_SHOWN - 1) as f32 * LYRICS_GAP;
    parts.push(Element::column(vec![Element::column(rows).gap(LYRICS_GAP).translate(slide)])
        .size(None, Some(height)).clip_children());
    let all = Button::new("lyrics", t!("desktop-show-lyrics"), raised).variant(ButtonVariant::Outline).size(ControlSize::Compact).round().render();
    parts.push(Element::row(vec![all, Element::spacer().flex(1.0)]));
    (!lines.is_empty()).then(|| panel_card(parts, width))
}

/// What plays after this, and the way to the whole queue.
fn next_up(state: &State, width: f32, palette: ComponentPalette) -> Option<Element> {
    let next = state.player.upcoming.first()?;
    let raised = ComponentPalette { surface: CHOSEN, ..palette };
    let queue = Link::new("next-queue", t!("desktop-open-queue"), raised).font_size(12.0).render();
    let head = Element::row(vec![Text::eyebrow(t!("desktop-next-eyebrow"), raised).render().centered_y(), Element::spacer().flex(1.0), queue.centered_y()]);
    let row = ListRow::new("upcoming:0", &next.title, raised)
        .sizing(TRACK_ROW)
        .leading(middle(cover(state, &next.artwork_url, COVER, raised)))
        .subtitle(&next.artist)
        .title_fade(true)
        .radius(RadiusRole::Card)
        .render();
    Some(panel_card(vec![head, row], width))
}

/// How much of what their page says of the artist the card quotes: some three lines.
const ABOUT_WORDS: usize = 150;

/// Who plays what plays, as Spotify's card presents them: their picture three by two with
/// the card's name over it, then their name, how many follow them beside the way to follow
/// them, and the opening of what their page says of them.
fn about_artist(state: &State, width: f32, palette: ComponentPalette) -> Option<Element> {
    let (artist, words) = state.about_artist()?;
    let raised = ComponentPalette { surface: CHOSEN, ..palette };
    let follows = state.is_followed(artist);
    let follow = Button::new("about-follow", if follows { t!("common-following") } else { t!("common-follow") }, raised).variant(ButtonVariant::Outline).size(ControlSize::Compact).round().render();
    let label = Text::new(t!("desktop-about-artist"), raised).font_size(16.0).truncate().render().font_family(FontFamily::SansBold);
    let mut parts = Vec::new();
    match artist.artwork_url.as_ref().and_then(|url| state.artwork.get(url)) {
        Some(image) => {
            let height = (width * 2.0 / 3.0).round();
            let scrim = Element::spacer().size(None, Some(height * 0.4)).vertical_gradient(vec![Color::rgba(0, 0, 0, 140), Color::rgba(0, 0, 0, 0)]);
            let over = Element::column(vec![label.padding(PANEL_PAD), Element::spacer().flex(1.0)]);
            let picture = Element::stack(vec![Element::raster_image(image.clone().cover()).size(Some(width), Some(height)), Element::column(vec![scrim, Element::spacer().flex(1.0)]), over]);
            parts.push(Element::clickable("about-picture", picture.size(Some(width), Some(height))).dispatch_as("about-artist"));
        }
        None => parts.push(label.padding(PANEL_PAD)),
    }
    let name = Element::clickable("about-artist", Text::new(&artist.name, raised).font_size(16.0).truncate().render().font_family(FontFamily::SansBold));
    let mut copy = vec![name];
    let followers = artist.subscribers.as_ref().map(|subscribers| Text::new(t!("desktop-subscribers", subscribers = subscribers.as_str()), raised).ink(SUBDUED).render());
    copy.push(Element::row(vec![followers.unwrap_or_else(Element::spacer).flex(1.0).centered_y(), follow.centered_y()]).gap(8.0));
    if let Some(words) = words.as_deref().map(str::trim).filter(|words| !words.is_empty()) {
        // The opening of it, cut at a word.
        let quoted = match words.char_indices().nth(ABOUT_WORDS) {
            Some((end, _)) => format!("{}…", words[..end].rsplit_once(char::is_whitespace).map_or(&words[..end], |(kept, _)| kept)),
            None => words.to_owned(),
        };
        copy.push(Text::new(quoted, raised).ink(SUBDUED).render());
    }
    parts.push(Element::column(copy).gap(8.0).padding(PANEL_PAD));
    Some(Element::panel(Element::column(parts)).background(CHOSEN).radius(RadiusRole::Card.px()).clip_children().size(Some(width), None))
}

/// A round button that stands on a picture, over a scrim of its own.
fn on_picture(id: &str, icon: Icon, hint: std::borrow::Cow<'static, str>, palette: ComponentPalette) -> Element {
    let scrim = ComponentPalette { surface: COVER_SCRIM, hover: COVER_SCRIM, selected: COVER_SCRIM, ..palette };
    IconButton::new(id, icon, scrim).variant(ButtonVariant::Subtle).size(ControlSize::Compact).round().tooltip(hint).render()
}

/// Turns the videos of what plays off and on, from where they are seen.
fn footage_switch(state: &State, palette: ComponentPalette) -> Element {
    let (icon, hint) = if state.settings.clips { (Icon::Image, t!("desktop-show-cover")) } else { (Icon::Video, t!("desktop-show-video")) };
    on_picture("cover-clips", icon, hint, palette)
}

/// The bar over what plays while it fills the window.
const CINEMA_TOP: f32 = 48.0;

/// The room over the playback bar, which what plays has while it fills the window.
pub fn cinema_room(window: Size) -> Size {
    Size::new(window.width, window.height - PLAYER_HEIGHT)
}

/// The size a film takes while it fills the window: all of it, as large as fits over the
/// playback bar.
pub fn cinema_picture(film: &surco_video::Film, window: Size) -> Size {
    let (width, height) = (film.width.max(1) as f32, film.height.max(1) as f32);
    let scale = (window.width / width).min((window.height - PLAYER_HEIGHT - CINEMA_TOP) / height);
    Size::new(width * scale, height * scale)
}

/// What plays over the whole window: its video where it has one that keeps time, its
/// cover where not, and under it the bar that plays it.
fn cinema(state: &State, window: Size, palette: ComponentPalette) -> Element {
    let room = cinema_room(window);
    let leave = on_picture("cinema", Icon::Compact, t!("desktop-exit-fullscreen"), palette);
    let top = Element::row(vec![Element::spacer().flex(1.0), footage_switch(state, palette).centered_y(), leave.centered_y()]).gap(6.0).padding(GAP).size(None, Some(CINEMA_TOP));
    let over = |picture: Element| Element::column(vec![top, centered(picture)]);
    // Around a video, its own glow; around a cover, the cover's colour.
    let stage = match (state.screening(), state.current()) {
        (Some((_, footage, Showing::Large)), _) => {
            let size = cinema_picture(&footage.large.0, window);
            let film = Element::surface(CLIP_SURFACE, t!("desktop-song-video")).size(Some(size.width), Some(size.height));
            let glow = Element::surface(CLIP_BACKDROP, "").size(Some(room.width), Some(room.height));
            Element::stack(vec![glow, over(film)]).clip_children()
        }
        (_, Some(track)) => {
            let side = (room.height - CINEMA_TOP).min(room.width) * 0.8;
            let stage = over(sole_cover(state, &largest(state, track), side, palette).radius(RadiusRole::Panel.px()));
            match track.artwork_url.as_ref().and_then(|url| state.tints.get(url)) {
                Some(tint) => stage.top_background_fade(Color { a: 150, ..*tint }, room.height),
                None => stage,
            }
        }
        _ => over(Element::spacer()),
    };
    Element::panel(Element::column(vec![stage.size(Some(room.width), Some(room.height)), player(state, window.width, palette)])).background(CANVAS).size(Some(window.width), Some(window.height))
}

/// How tall the song's video stands at the top of a panel `width` wide, as Spotify's Canvas:
/// 9 by 14.5, though no more than two thirds of the panel, so what follows shows under it,
/// and never less than square.
pub fn canvas_height(width: f32, window: Size) -> f32 {
    let panel = window.height - TOP_BAR_HEIGHT - PLAYER_HEIGHT;
    (width * 14.5 / 9.0).min(panel * 2.0 / 3.0).max(width.min(panel))
}

/// The strip the song's name in the right panel slides along in.
pub(crate) const PANEL_TITLE: &str = "panel-title";

/// The right panel's own margin. Its lists do without it: they run to the panel's edge, so
/// their scrollbar sits where the middle panel's does, and set their rows in themselves.
const PANEL_PAD: f32 = 16.0;

fn inset(part: Element) -> Element {
    let gutter = || Element::spacer().size(Some(PANEL_PAD), None);
    Element::row(vec![gutter(), part.flex(1.0), gutter()])
}

/// The right-hand column: what plays, or what comes after it while the queue is open.
fn panel(state: &State, window: Size, palette: ComponentPalette) -> Element {
    let content = if state.queue_open {
        vec![panel_header(&t!("desktop-up-next"), &t!("desktop-queue-detail"), palette), upcoming(state, palette)]
    } else {
        match state.current() {
            Some(track) => return now_playing(state, track, window, palette),
            None => vec![
                panel_header(&t!("desktop-now-playing"), &t!("desktop-nothing-yet"), palette),
                centered(Empty::new(t!("desktop-nothing-playing"), t!("desktop-nothing-playing-detail"), palette).icon(Icon::MusicNote).render()),
            ],
        }
    };
    let face = if state.queue_open { "panel:queue" } else { "panel:playing" };
    // The column's gap, plus this, is the margin over the header.
    let top = Element::spacer().size(None, Some(PANEL_PAD - 12.0));
    Element::column(std::iter::once(top).chain(content).collect()).gap(12.0).animate_enter(face)
}

/// The panel's own ink for what is said under a title, as Spotify's: white at 70%.
const SUBDUED: Color = Color::rgba(255, 255, 255, 179);
/// The panel's floor, which its scrims fade the video into.
const PANEL_FLOOR: Color = Color::rgb(18, 18, 18);
/// The height of the panel's header, which stays over what scrolls under it.
const PANEL_HEADER: f32 = 64.0;

/// What plays, as Spotify's Now Playing view lays it out. The song's own video fills the
/// panel's width from its very top, under the header and under scrims of the panel's floor,
/// its name over the foot of it; with no video its cover stands square under the header.
/// Under them, the cards: the lyrics, who sings it, and what comes next. The header floats
/// over it all, and takes the floor once the panel is scrolled.
fn now_playing(state: &State, track: &Track, window: Size, palette: ComponentPalette) -> Element {
    let width = state.right_width;
    let side = width - 2.0 * PANEL_PAD;
    let video = state.screening().is_some();
    let link = |id: &str, action: &str, text: Element| Element::clickable(id, text).dispatch_as(action);
    let liked = state.is_liked(&track.id);
    let names = Element::column(vec![
        // A name longer than its room slides along it, as Spotify's; the window moves the strip.
        link("panel-album", "now-album", Element::horizontal_scroll_2d(PANEL_TITLE, Element::text(&track.title).font_size(24.0).font_family(FontFamily::SansBold).foreground(palette.text))
            .foreground(Color::rgba(0, 0, 0, 0))
            .horizontal_edge_fade(PANEL_FLOOR, 16.0)),
        link("panel-artist", "now-artist", Text::new(&track.artist, palette).font_size(16.0).ink(SUBDUED).truncate().render()),
    ]).gap(2.0).flex(1.0).centered_y();
    let share = IconButton::new("panel-share", Icon::Custom(include_str!("../assets/icons/share-2.svg")), palette).round().variant(ButtonVariant::Ghost).size(ControlSize::Compact)
        .tooltip(t!("desktop-share")).render().reveal_on_hover("panel-share-shown").centered_y();
    let like = IconButton::new("panel-like", like_glyph(liked), palette).round().variant(ButtonVariant::Ghost).size(ControlSize::Compact)
        .tooltip(if liked { t!("desktop-remove-like") } else { t!("common-like") }).render().centered_y();
    let title = Element::row(vec![names, share, like]).gap(8.0);

    let media = if video {
        // Full-bleed and tall from the top: the cover until the first frame, then the video,
        // darkened at the top for the header and at the foot for the name over it.
        let tall = canvas_height(width, window);
        let picture = cover(state, &largest(state, track), width, palette).size(Some(width), Some(tall)).radius(0.0);
        let top = Element::spacer().size(None, Some(width * 0.25)).vertical_gradient(vec![Color { a: 161, ..PANEL_FLOOR }, Color { a: 0, ..PANEL_FLOOR }]);
        let foot = Element::spacer().size(None, Some(width * 0.45)).vertical_gradient(vec![Color { a: 0, ..PANEL_FLOOR }, Color { a: 115, ..PANEL_FLOOR }, Color { a: 230, ..PANEL_FLOOR }, PANEL_FLOOR]);
        let scrims = Element::column(vec![top, Element::spacer().flex(1.0), foot]);
        let over = Element::column(vec![Element::spacer().flex(1.0), title.padding(PANEL_PAD)]);
        // The cover until the video's first frame, which then fades in over it rather than
        // taking its place at once.
        let ready = state.screening().map(|(name, ..)| name).filter(|name| state.clip_ready.as_ref() == Some(name));
        let mut layers = vec![picture];
        layers.extend(ready.map(|name| Element::surface(CLIP_SURFACE, t!("desktop-song-video")).animate_enter(format!("clip:{name}"))));
        layers.extend([scrims, over]);
        Element::stack(layers).size(Some(width), Some(tall)).clip_children()
    } else {
        // Square under the header, a margin from each side; a click opens its release.
        let picture = link("panel-cover", "now-album", cover(state, &largest(state, track), side, palette).radius(RadiusRole::Card.px()));
        Element::column(vec![Element::spacer().size(None, Some(PANEL_HEADER)), inset(picture), inset(title)]).gap(16.0)
    };
    let mut parts = vec![media];
    parts.extend(lyrics_preview(state, side, palette).map(inset));
    parts.extend(about_artist(state, side, palette).map(inset));
    parts.extend(next_up(state, side, palette).map(inset));
    parts.push(Element::spacer().size(None, Some(PANEL_PAD)));
    let scroll = panel_scroll("playing", Element::column(parts).gap(16.0)).render().flex(1.0);

    // The header: what the song is from, the way to shut the panel and, under the pointer,
    // whether to see its video and the way to see it large.
    let context = track.album.as_deref().unwrap_or(&track.artist);
    let shut = header_button("toggle-panel", Icon::Custom(glyph::PANEL_RIGHT), TooltipSpec::new(t!("desktop-hide-panel")), palette).render().reveal_on_hover("panel-shut");
    let large = header_button("panel-cinema", Icon::Custom(glyph::FULLSCREEN), TooltipSpec::new(t!("desktop-fullscreen")), palette).render();
    // As Spotify's: the rest of what can be done with the song waits behind the dots.
    // With the glyphs a song's menu has everywhere else.
    let item = |id: &str, label: std::borrow::Cow<'static, str>, icon: &'static str| DropdownMenuEntry::Item(DropdownMenuItem::new(id, label).icon(Icon::Custom(icon)));
    let mut entries = Vec::new();
    if track.album_id.is_some() { entries.push(item("panel-go:album", t!("desktop-go-to-album"), include_str!("../assets/icons/disc-3.svg"))); }
    entries.extend([
        item("panel-go:artist", t!("desktop-go-to-artist"), include_str!("../assets/icons/user-round.svg")),
        item("panel-go:share", t!("desktop-share"), include_str!("../assets/icons/share-2.svg")),
    ]);
    let dots = header_button("panel-more", Icon::More, TooltipSpec::new(t!("desktop-options-of", name = track.title.as_str())), palette).active(state.panel_more_open).render();
    let more = DropdownMenu::new(dots, entries, state.panel_more_open, "panel-more-dismiss", menu_palette()).sizing(menu_sizing(state)).width(MENU_WIDTH).placement(PopoverPlacement::BottomEnd).render();
    // The video or the cover: the glyph is the one it turns to.
    let (glyph, hint) = if state.settings.clips { (glyph::PICTURE, t!("desktop-show-cover")) } else { (glyph::VIDEO, t!("desktop-show-video")) };
    let footage = header_button("cover-clips", Icon::Custom(glyph), TooltipSpec::new(hint), palette).render();
    // The menu open keeps them in view.
    let tools = Element::row(vec![footage, more, large]).gap(8.0);
    let tools = if state.panel_more_open { tools } else { tools.reveal_on_hover("panel-tools") };
    let header = Element::row(vec![
        shut.centered_y(),
        Element::clickable("panel-context", Text::new(context, palette).font_size(16.0).truncate().render().font_family(FontFamily::SansBold)).dispatch_as("now-album").flex(1.0).centered_y(),
        tools.centered_y(),
    ]).gap(12.0).padding(PANEL_PAD).size(None, Some(PANEL_HEADER));
    let header = if state.panel_scrolled || !video {
        header.background(PANEL_FLOOR).shadow(if state.panel_scrolled { Color::rgba(0, 0, 0, 153) } else { Color::rgba(0, 0, 0, 0) }, limn::Point::new(0.0, 6.0), 10.0, 0.0)
    } else { header };
    let floating = Element::column(vec![header, Element::spacer().flex(1.0)]);
    Element::stack(vec![scroll, floating]).hover_region().animate_enter("panel:playing")
}

/// How bright and how out of focus a line is `distance` lines after the one being sung,
/// negative before it. The line being sung is sharp and in full ink; what is coming fades
/// and blurs the further off it is, and what was sung steps back a little faster.
pub(crate) fn lyric_focus(distance: i64) -> (f32, f32) {
    match distance {
        0 => (1.0, 0.0),
        ahead @ 1.. => ((0.55 - 0.07 * (ahead - 1) as f32).max(0.18), (0.3 + 0.5 * (ahead - 1) as f32).min(LYRIC_BLUR)),
        behind => ((0.42 - 0.08 * (-behind - 1) as f32).max(0.16), (0.6 + 0.5 * (-behind - 1) as f32).min(LYRIC_BLUR)),
    }
}

/// Where the line being sung rests on a phone, below the top of the lyrics' visible `body`:
/// a third of the way down, with what was sung above it and what is coming below.
pub(crate) fn lyric_anchor(body: f32) -> f32 {
    (body / 3.0).max(20.0)
}

/// The lines of what plays down the middle of the window, the one being sung in full ink,
/// over a wash of the cover's own colour. A line that is clicked is sung from its start.
/// Given the height of the `body` they are seen through, as on a phone, the first and the
/// last line can both reach the anchor the list follows the song at.
fn lyrics(state: &State, width: f32, body: f32, palette: ComponentPalette) -> Element {
    let note = |title: &str, about: &str| centered(Empty::new(title, about, palette).icon(Icon::Custom(LYRICS)).render());
    let lines = match &state.lyrics {
        _ if state.current().is_none() => return note(&t!("desktop-nothing-playing"), &t!("desktop-lyrics-nothing-detail")),
        Some((_, Lyrics::Loading)) | None => return loading("lyrics-loading", palette),
        Some((_, Lyrics::Missing)) => return centered(Element::column(vec![
            Empty::new(t!("desktop-lyrics-missing"), t!("desktop-lyrics-missing-detail"), palette).icon(Icon::Custom(LYRICS)).render(),
            Button::new("retry-lyrics", t!("desktop-search-again"), palette).variant(ButtonVariant::Outline).size(ControlSize::Spacious).round().render(),
        ]).gap(20.0).padding(20.0)),
        Some((_, Lyrics::Failed { message, .. })) => return centered(Element::column(vec![
            Empty::new(t!("desktop-lyrics-failed"), message, palette).icon(Icon::Custom(LYRICS)).render(),
            Button::new("retry-lyrics", t!("common-retry"), palette).variant(ButtonVariant::Outline).size(ControlSize::Spacious).round().render(),
        ]).gap(20.0).padding(20.0)),
        Some((_, Lyrics::Plain { source, .. })) => {
            let words = std::iter::once(Text::new(t!("desktop-lyrics-unsynced", source = source.to_string()), palette).muted().render())
                .chain(state.plain_lyrics().iter().enumerate().map(|(index, line)| {
                    if line.is_empty() { return Element::spacer().size(None, Some(20.0)); }
                    let said = Text::new(line, palette).font_size(LYRIC_SIZE).render();
                    match state.translated(index, line) {
                        Some(translation) => Element::column(vec![said, Text::new(translation, palette).font_size(LYRIC_SIZE * TRANSLATION_SCALE).muted().render()]).gap(4.0),
                        None => said,
                    }
                })).collect();
            let list = ScrollArea::new("lyrics-list", Element::column(words).gap(LYRIC_GAP).padding(PANEL_PAD)).render().flex(1.0);
            return with_translate(state, list, palette);
        }
        Some((_, Lyrics::Lines(_))) => state.lines(),
    };
    // With room, the lines stand larger and further in from the edge.
    let (size, margin) = if width >= LYRIC_WIDE_FROM { (LYRIC_SIZE_WIDE, (width * 0.08).min(96.0)) }
        else if state.mobile { (LYRIC_SIZE_PHONE, PANEL_PAD + 8.0) } else { (LYRIC_SIZE, PANEL_PAD) };
    let (gap, line_height) = (LYRIC_GAP * size / LYRIC_SIZE, size * 1.2);
    // Before the first line everything is still to come.
    let sung = state.active_line().map_or(-1, |index| index as i64);
    let now = std::time::Instant::now();
    let rows = lines.iter().enumerate().map(|(index, line)| {
        let (ink, blur) = lyric_focus(index as i64 - sung);
        let ink = Color { a: (255.0 * ink) as u8, ..palette.text };
        let text = Element::text(&line.text)
            .font_family(FontFamily::SansBold)
            .font_size(size)
            .line_height(line_height)
            .foreground(ink)
            .animate_foreground(format!("lyric-ink:{index}"))
            .soft_focus(format!("lyric:{index}"), blur)
            // The line being sung grows a little from its leading edge; paint only, no re-wrap.
            .animate_scale(format!("lyric-scale:{index}"), if index as i64 == sung { 1.05 } else { 1.0 });
        // Its translation goes under it, smaller, and dims and blurs with it.
        let text = match state.translated(index, &line.text) {
            Some(translation) => Element::column(vec![text, Element::text(translation)
                .font_family(FontFamily::Sans)
                .font_size(size * TRANSLATION_SCALE)
                .line_height(size * TRANSLATION_SCALE * 1.25)
                .foreground(ink)
                .animate_foreground(format!("lyric-translation-ink:{index}"))
                .soft_focus(format!("lyric-translation:{index}"), blur)]).gap(gap / 3.0),
            None => text,
        };
        let row = Element::clickable(format!("lyric:{index}"), text);
        // Still catching up with the list's jump to the line being sung.
        let lag = state.lyric_lag(index, now);
        if lag == 0.0 { row } else { row.translate(limn::Point::new(0.0, lag)) }
    });
    let mut rows: Vec<Element> = rows.collect();
    if body > 0.0 {
        let anchor = lyric_anchor(body);
        rows.insert(0, Element::spacer().size(None, Some((anchor - margin - gap).max(0.0))));
        rows.push(Element::spacer().size(None, Some((body - anchor - margin - gap - line_height).max(0.0))));
    }
    // The list glides to each new line rather than stepping to it.
    let glide = limn::ScrollMotion { duration_ms: limn::tokens().motion.move_ms * 3, easing: limn::Easing::InOutCubic };
    let list = ScrollArea::new("lyrics-list", Element::column(rows).gap(gap).padding(margin)).motion(glide).render().flex(1.0);
    let page = with_translate(state, list, palette);
    match state.current().and_then(|track| state.tints.get(track.artwork_url.as_deref()?)) {
        Some(tint) => page.bottom_background_fade(Color { a: 120, ..*tint }, LYRIC_WASH),
        None => page,
    }
}

/// How large a line's translation is next to the line.
const TRANSLATION_SCALE: f32 = 0.58;

/// The lyrics with the button that translates them over their top end, on the desktop; the
/// phone has it in its header.
fn with_translate(state: &State, list: Element, palette: ComponentPalette) -> Element {
    if state.mobile || !state.translatable() {
        return Element::column(vec![list]).flex(1.0);
    }
    let on = state.settings.translate_lyrics;
    let button = IconButton::new("lyrics-translate", Icon::Custom(include_str!("../assets/icons/languages.svg")), palette)
        .round().variant(if on { ButtonVariant::Secondary } else { ButtonVariant::Ghost }).size(ControlSize::Regular)
        .tooltip(if on { t!("common-original-lyrics") } else { t!("common-translate-lyrics") }).render();
    Element::column(vec![Element::row(vec![Element::spacer().flex(1.0), button]).padding(PANEL_PAD / 2.0), list]).flex(1.0)
}

fn upcoming(state: &State, palette: ComponentPalette) -> Element {
    let rows: Vec<Element> = state
        .player
        .upcoming
        .iter()
        .enumerate()
        .map(|(index, track)| {
            if state.mobile {
                let subtitle = if state.player.picks.contains(&track.id) { format!("{} · {}", track.artist, t!("desktop-recommended")) } else { track.artist.clone() };
                return mobile::track_row(state, format!("upcoming:{index}"), track, &subtitle, palette);
            }
            let row = ListRow::new(format!("upcoming:{index}"), &track.title, palette)
                .sizing(TRACK_ROW)
                .leading(middle(cover(state, &track.artwork_url, COVER, palette)))
                // A track the shuffle brought in from the catalog says so.
                .subtitle(if state.player.picks.contains(&track.id) { format!("{} · {}", track.artist, t!("desktop-recommended")) } else { track.artist.clone() })
                .title_fade(true)
                .radius(RadiusRole::Card)
                .render();
            // A finger pans the queue; desktop pointers can reorder its rows.
            let row = if state.mobile { row } else { row.draggable().drop_target() };
            let out = ContextMenuItem::new(format!("do:unqueue:upcoming:{index}"), t!("desktop-remove-from-queue")).icon(Icon::Custom(include_str!("../assets/icons/circle-minus.svg")));
            let row = ContextMenu::new(format!("upcoming-context:{index}"), row, vec![ContextMenuEntry::Item(out)], menu_palette()).render();
            // Named by its track, so the row that starts playing closes and the rest glide up.
            Element::column(vec![row]).animate_height(format!("upcoming-row:{}", track.id), false)
        })
        .collect();
    // What was queued by hand is told apart from what the list brings, and can be dropped at once.
    let mut rows = rows;
    let queued = state.player.queued.min(rows.len());
    if queued > 0 {
        let title = |text: &str| Text::new(text, palette).muted().render().centered_y();
        let clear = Button::new("queue-clear", t!("common-clear"), palette).variant(ButtonVariant::Ghost).size(ControlSize::Compact).render();
        if queued < rows.len() {
            rows.insert(queued, Element::panel(title(&t!("desktop-next-up"))).padding(PANEL_PAD / 2.0));
        }
        rows.insert(0, Element::panel(Element::row(vec![title(&t!("desktop-in-queue")), Element::spacer().flex(1.0), clear])).padding(PANEL_PAD / 2.0));
    }
    if rows.is_empty() {
        return centered(Empty::new(t!("desktop-queue-empty"), t!("desktop-queue-empty-detail"), palette).icon(Icon::Playlist).render());
    }
    // The rows carry half the margin in themselves.
    let list = Element::column(rows).keep_leaving("upcoming-rows");
    panel_scroll("queue-list", Element::column(vec![list]).padding(PANEL_PAD / 2.0)).render().flex(1.0)
}

fn cover(state: &State, artwork: &Option<String>, side: f32, palette: ComponentPalette) -> Element {
    picture(state, artwork, side, false, palette)
}

/// A cover shown large and alone, decoded whole: limn decodes the rest at 384 pixels a side,
/// which three pixels a point blur past 128 points. limn keeps two such at once, so one a page.
fn sole_cover(state: &State, artwork: &Option<String>, side: f32, palette: ComponentPalette) -> Element {
    picture(state, artwork, side, true, palette)
}

/// The playing track's large rendition once it has arrived; until then, the row's.
fn largest(state: &State, track: &Track) -> Option<String> {
    let large = track.artwork_url.as_deref().map(large_cover).filter(|url| state.artwork.contains_key(url));
    large.or_else(|| track.artwork_url.clone())
}

/// What a cover on its way stands in with: a pulse where it will be, every cover in step, so
/// that covers do not appear out of nothing. One that has none, or could not be had, has none.
fn coming(state: &State, artwork: Option<&String>, palette: ComponentPalette) -> Option<Element> {
    artwork.filter(|url| state.cover_coming(url))
        .map(|_| limn::Skeleton::new("cover-coming", ComponentPalette { raised: palette.selected, ..palette }).render())
}

fn picture(state: &State, artwork: &Option<String>, side: f32, whole: bool, palette: ComponentPalette) -> Element {
    let radius = limn::tokens().radius.control;
    match artwork.as_ref().and_then(|url| state.artwork.get(url)) {
        // Covers are square; a video's thumbnail and an artist's banner are not.
        Some(image) if whole => Element::raster_image(image.clone().cover().full_resolution()).size(Some(side), Some(side)).radius(radius),
        Some(image) => Element::raster_image(image.clone().cover()).size(Some(side), Some(side)).radius(radius),
        None if coming(state, artwork.as_ref(), palette).is_some() => coming(state, artwork.as_ref(), palette).unwrap().size(Some(side), Some(side)).radius(radius),
        None => Element::panel(Element::icon(Icon::MusicNote).foreground(palette.muted).centered())
            .size(Some(side), Some(side))
            .background(palette.selected)
            .radius(radius),
    }
}

/// Across the bottom: what plays, the transport over the progress bar, and the rest.
fn player(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let current = state.current();
    let has_track = current.is_some();
    let wide = width >= RAIL_BELOW;
    let transport = |id: &str, icon: Icon, hint: &str| {
        IconButton::new(id, icon, palette).round().variant(ButtonVariant::Ghost).size(ControlSize::Regular).tooltip(hint).enabled(has_track)
    };
    let now_playing = match current {
        Some(track) => Element::row(vec![
            cover(state, &track.artwork_url, 56.0, palette).radius(4.0).centered_y(),
            // The title leads to the album and the artist to their page.
            // A new track's name rises into place as the last one's leaves.
            Element::column(vec![
                Ticker::new(
                    "now-title",
                    &track.id,
                    Element::clickable(
                        "now-album",
                        Element::faded_text(&track.title, 24.0).font_family(FontFamily::Sans).font_size(14.0).foreground(palette.text),
                    ),
                )
                .height(20.0)
                .render(),
                Ticker::new(
                    "now-artist-line",
                    &track.id,
                    Element::clickable(
                        "now-artist",
                        Element::faded_text(state.player.error.as_deref().unwrap_or(&track.artist), 24.0)
                            .font_family(FontFamily::Sans)
                            .font_size(12.0)
                            .foreground(palette.muted),
                    ),
                )
                .height(18.0)
                .render(),
            ])
            .gap(2.0)
            // As Spotify's, the heart stands right after the title, however short it is.
            .max_width((width * 0.3 - NOW_PLAYING_ROOM).clamp(80.0, 360.0))
            .centered_y(),
            {
                let liked = state.is_liked(&track.id);
                IconButton::new("like", like_glyph(liked), palette).round()
                    .variant(ButtonVariant::Ghost)
                    .size(ControlSize::Compact)
                    .tooltip(if liked { t!("desktop-remove-like") } else { t!("common-like") })
                    .ink(if liked { palette.text } else { palette.muted })
                    .hover_ink(palette.text)
                    .render()
                    .centered_y()
            },
            Element::spacer().flex(1.0),
        ])
        .gap(12.0),
        None => Element::spacer(),
    };
    let repeat_icon = Icon::Custom(if state.player.repeat == Repeat::One { glyph::REPEAT_ONE } else { glyph::REPEAT });
    // Play is the one filled control of the bar: a white circle.
    let round = if has_track {
        ComponentPalette { surface: palette.text, text: SCHEME.canvas, hover: ROUND_HOVER, selected: ROUND_HELD, ..palette }
    } else {
        ComponentPalette { surface: ROUND_OFF, muted: ROUND_OFF_INK, ..palette }
    };
    let play = IconButton::new("play-pause", if state.player.playing { Icon::PauseFill } else { Icon::PlayFill }, round)
        .variant(ButtonVariant::Subtle)
        .size(ControlSize::Regular)
        .round()
        .tooltip(if state.player.playing { t!("common-pause") } else { t!("common-play") })
        .enabled(has_track)
        .render()
        .center_content();
    let buttons = Element::row(vec![
        transport("shuffle", shuffle_step(state).0, &shuffle_step(state).1).active(state.player.shuffle).render(),
        transport("previous", Icon::Custom(glyph::PREVIOUS), &t!("desktop-previous")).render(),
        play,
        transport("next", Icon::Custom(glyph::NEXT), &t!("desktop-next")).render(),
        transport("repeat", repeat_icon, &t!("desktop-repeat")).active(state.player.repeat != Repeat::Off).render(),
    ])
    .gap(8.0)
    .centered_x();
    let time = |seconds: f64| {
        Element::text(clock(seconds)).font_family(FontFamily::Sans).font_size(12.0).foreground(palette.muted).centered_y()
    };
    let seek = Element::row(vec![
        time(f64::from(state.progress()) * state.player.duration),
        Slider::new("seek", state.progress(), palette).enabled(has_track && state.player.duration > 0.0).thumb_on_hover().render().flex(1.0).centered_y(),
        time(state.player.duration),
    ])
    .gap(8.0);
    // As Spotify's: muted at rest and lit under the pointer; one that is on is in full ink with
    // a dot under it.
    let side = |id: &str, icon: Icon, hint: &str, active: bool| {
        let button = IconButton::new(id, icon, palette).round().variant(ButtonVariant::Ghost).size(ControlSize::Compact).tooltip(hint)
            .ink(if active { palette.text } else { palette.muted }).hover_ink(palette.text).render();
        let dot = Element::spacer().size(Some(ACTIVE_DOT), Some(ACTIVE_DOT)).background(palette.text).radius(ACTIVE_DOT / 2.0);
        let marked = if active { Element::stack(vec![button, Element::column(vec![Element::spacer().flex(1.0), dot.centered_x()])]) } else { button };
        marked.centered_y()
    };
    let equalizer = Popover::new(
        side("equalizer", Icon::Custom(glyph::EQUALIZER), &t!("common-equalizer"), state.equalizer_open),
        equalizer::popover(state, menu_palette()),
        state.equalizer_open,
        "equalizer-close",
        menu_palette(),
    )
    .placement(PopoverPlacement::TopEnd)
    .width(EQUALIZER_WIDTH)
    .render()
    .centered_y();
    let mut controls = Vec::new();
    if wide {
        controls.push(Element::spacer().flex(1.0));
    }
    // In Spotify's order: what plays, its lyrics, what comes next, where it sounds, then the
    // sound itself and the whole window.
    let showing = state.panel_open && !state.queue_open && width >= PANEL_FROM;
    let device = |id: &str, name: &str| {
        DropdownMenuEntry::Item(DropdownMenuItem::new(format!("setting:output:{id}"), name).selected(state.output.as_deref().unwrap_or_default() == id))
    };
    let system = state.outputs.iter().find(|output| output.is_default)
        .map_or_else(|| t!("desktop-system-output").into_owned(), |output| t!("desktop-system-output-named", name = output.name.as_str()).into_owned());
    let mut devices = vec![DropdownMenuEntry::Heading(t!("desktop-connect-device").into()), device("", &system)];
    if state.outputs.is_empty() && state.outputs_loading {
        devices.push(DropdownMenuEntry::Loading { id: "device-loading".into(), rows: 2 });
    }
    devices.extend(state.outputs.iter().filter(|output| !output.is_default).map(|output| device(&output.id, &output.name)));
    let devices = DropdownMenu::new(side("device", Icon::Custom(glyph::DEVICE), &t!("desktop-connect-device"), state.device_open), devices, state.device_open, "device-dismiss", menu_palette())
        .sizing(menu_sizing(state))
        .width(MENU_WIDTH)
        .placement(PopoverPlacement::TopEnd)
        .render()
        .centered_y();
    if width >= PANEL_FROM {
        controls.push(side("now-playing-view", Icon::Custom(glyph::NOW_PLAYING_VIEW), &t!("desktop-now-playing-view"), showing));
    }
    controls.extend([
        side("lyrics", Icon::Custom(glyph::LYRICS), &t!("desktop-lyrics"), state.lyrics_open()),
        side("queue", Icon::Custom(glyph::QUEUE), &t!("desktop-queue"), state.queue_open),
        devices,
        equalizer,
        IconButton::new("mute", volume_glyph(state.volume), palette).round()
            .variant(ButtonVariant::Ghost)
            .size(ControlSize::Compact)
            .tooltip(if state.volume > 0.0 { t!("desktop-mute") } else { t!("desktop-unmute") })
            .ink(palette.muted)
            .hover_ink(palette.text)
            .render()
            .centered_y(),
        Slider::new("volume", state.volume, palette).width(if wide { VOLUME_WIDTH } else { VOLUME_WIDTH_NARROW }).thumb_on_hover().render().centered_y(),
    ]);
    // Where there is room for the right panel there is room for the whole window's button.
    if has_track && width >= PANEL_FROM {
        controls.push(side("cinema", Icon::Custom(glyph::FULLSCREEN), &t!("desktop-fullscreen"), state.cinema));
    }
    let controls = Element::row(controls).gap(6.0).centered_y();
    let middle = Element::column(vec![buttons, seek]).gap(2.0).centered_y();
    // Wide, the three parts share the bar 3:4:3 and the transport sits in the middle of the
    // window. Narrow, the controls take only what they need.
    let parts = if wide {
        vec![now_playing.flex(3.0).centered_y(), middle.flex(4.0), controls.flex(3.0)]
    } else {
        vec![now_playing.flex(1.0).centered_y(), middle.flex(1.6), controls]
    };
    Element::panel(Element::row(parts).gap(16.0)).padding(GAP).size(None, Some(PLAYER_HEIGHT))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{Content, Event, Target};
    use crate::player::PlayerView;
    use surco_core::{ArtistRef, Found};
    use limn::{HitKind, UiTree};

    fn track(id: &str) -> Track {
        Track {
            id: id.into(),
            title: format!("Title {id}"),
            artist: "Artist".into(),
            artist_id: None,
            album: None,
            album_id: None,
            artwork_url: None,
            duration_ms: 200_000,
        }
    }

    fn asked(query: &str) -> State {
        let mut state = State::default();
        state.query = query.into();
        state
    }

    fn ids(state: &State, size: Size) -> Vec<String> {
        let mut tree = UiTree::new(view(state, size));
        // A disabled control keeps a hover region for its tooltip; these are the ones that answer a press.
        tree.layout(size).regions.iter().filter(|region| matches!(region.kind, HitKind::Button | HitKind::TextInput)).map(|region| region.id.clone()).collect()
    }

    #[test]
    fn a_page_keeps_a_search_at_the_end_of_its_actions_that_opens_into_a_field_and_narrows_its_rows() {
        let size = Size::new(1400.0, 900.0);
        let album = Album { id: "lp".into(), title: "Album".into(), artist: "Artist".into(), artist_id: None, year: "2017".into(), artwork_url: None, tracks: vec![track("one"), track("two")] };
        // Without motion the field is open on the frame of the click.
        let mut state = State::default();
        state.page = Page::Album(album);
        state.motion = false;
        let has = |state: &State, id: &str| ids(state, size).iter().any(|found| found == id);
        assert!(has(&state, "page-search") && !has(&state, "page-query"), "a magnifier, shut");
        state.update(&limn::UiAction::Clicked { id: "page-search".into() });
        assert!(has(&state, "page-query") && has(&state, "page-search"), "the field, and the cross that shuts it");
        assert!(has(&state, "page-track:0") && has(&state, "page-track:1"));
        state.update(&limn::UiAction::InputChanged { id: "page-query".into(), value: "title TWO".into() });
        assert!(!has(&state, "page-track:0") && has(&state, "page-track:1"), "the rows keep their own numbers");
        state.update(&limn::UiAction::Clicked { id: "page-search".into() });
        assert!(!has(&state, "page-query") && has(&state, "page-track:0"), "shut, the page lists everything again");
    }

    #[test]
    fn a_pages_search_fits_whole_in_a_narrow_pane_beside_its_other_actions() {
        let album = Album { id: "lp".into(), title: "Album".into(), artist: "Artist".into(), artist_id: None, year: "2017".into(), artwork_url: None, tracks: vec![track("one")] };
        let mut state = State::default();
        state.page = Page::Album(album);
        state.motion = false;
        state.update(&limn::UiAction::Clicked { id: "page-search".into() });
        for width in [1100.0, 1900.0] {
            let size = Size::new(width, 700.0);
            let mut tree = UiTree::new(view(&state, size));
            let frame = tree.layout(size);
            let rect = |id: &str| frame.regions.iter().find(|region| region.id == id).map(|region| region.rect).unwrap();
            let (keep, field, shut, pane) = (rect("keep"), rect("page-query"), rect("page-search"), rect("album"));
            assert!(field.width >= 120.0, "room to type at {width}: {field:?}");
            assert!(keep.x + keep.width <= field.x, "side by side at {width}: {keep:?} {field:?}");
            assert!(shut.x > field.x + field.width / 2.0 && shut.x + shut.width <= field.x + field.width, "the cross inside the field's end at {width}: {field:?} {shut:?}");
            assert!(shut.x + shut.width <= pane.x + pane.width - 12.0, "inside the pane at {width}: {shut:?}");
        }
    }

    #[test]
    fn a_pages_table_has_heads_that_order_it_and_an_album_boundary_to_drag() {
        let song = |id: &str| Track { album: Some("Disco".into()), ..track(id) };
        let artist = Artist { id: "who".into(), name: "Who".into(), subscribers: None, artwork_url: None, songs: vec![song("a"), song("b")], albums: Vec::new(), singles: Vec::new() };
        let mut state = State::default();
        state.page = Page::Artist(artist);
        let found = ids(&state, Size::new(1600.0, 800.0));
        for id in ["tracks:sort:title", "tracks:sort:album", "tracks:sort:duration", "tracks:resize:album", "page-track:0", "more:page-track:1", "page-sort"] {
            assert!(found.iter().any(|found| found == id), "{id} among {found:?}");
        }
        // In a narrow pane the album gives its place to the title and there is nothing to drag.
        let found = ids(&state, Size::new(1100.0, 800.0));
        assert!(found.iter().any(|id| id == "tracks:sort:title") && !found.iter().any(|id| id.starts_with("tracks:") && id.contains("album")), "{found:?}");
    }

    #[test]
    fn the_library_lists_as_lines_rows_or_tiles_and_the_panel_previews_lyrics_and_what_is_next() {
        let size = Size::new(1400.0, 900.0);
        let mut state = State::default();
        for view in ["compact", "list", "grid"] {
            state.update(&limn::UiAction::Clicked { id: format!("library-view:{view}") });
            assert!(ids(&state, size).iter().any(|id| id == "downloads"), "{view} still lists what the library holds");
        }
        state.show(PlayerView { current: Some(track("a")), upcoming: vec![track("b")], duration: 200.0, playing: true, ..PlayerView::default() });
        state.apply(Event::Lyrics { track_id: "a".into(), lines: Some(vec![surco_core::lyrics::Line { at_ms: 0, text: "one".into() }]) });
        let found = ids(&state, size);
        // The card of the lines leads to all of them, and the one of what is next plays it or opens the queue.
        for id in ["lyrics", "upcoming:0", "next-queue"] {
            assert!(found.iter().filter(|found| *found == id).count() >= 1, "{id} among {found:?}");
        }
    }

    #[test]
    fn the_buttons_of_the_panel_of_what_plays_do_what_they_say_when_pressed() {
        let size = Size::new(1400.0, 900.0);
        let mut state = State::default();
        state.show(PlayerView { current: Some(track("a")), upcoming: vec![track("b")], duration: 200.0, playing: true, ..PlayerView::default() });
        // A press as the window gives it: down and up in the middle of the control.
        let press = |state: &mut State, id: &str| {
            let mut tree = UiTree::new(view(state, size));
            let frame = tree.layout(size);
            let rect = frame.regions.iter().find(|region| region.id == id).map(|region| region.rect).unwrap_or_else(|| panic!("no {id}"));
            let middle = limn::Point::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
            tree.handle_event(limn::UiEvent::PointerMove(middle));
            tree.layout(size);
            tree.handle_event(limn::UiEvent::PointerDown(middle));
            for action in tree.handle_event(limn::UiEvent::PointerUp(middle)) {
                state.update(&action);
            }
        };
        assert!(state.settings.clips);
        press(&mut state, "cover-clips");
        assert!(!state.settings.clips, "the cover is shown in the video's place");
        press(&mut state, "cover-clips");
        assert!(state.settings.clips);
        press(&mut state, "cinema");
        assert!(state.cinema, "what plays takes the whole window");
        press(&mut state, "cinema");
        assert!(!state.cinema, "and gives it back from the same button");
        press(&mut state, "next-queue");
        assert!(state.queue_open, "the link under what comes next opens the queue");
    }

    #[test]
    fn the_card_of_a_later_version_shows_a_bar_while_it_comes_down_and_a_button_otherwise() {
        let size = Size::new(1400.0, 900.0);
        let mut state = State::default();
        let release = Release { version: "9.0.0".into(), page: "https://example/9".into(), program: Some(("https://example/p".into(), "https://example/s".into())), mandatory: false, notes: Vec::new() };
        state.apply(Event::Update(release));
        let shown = |state: &State| {
            let mut tree = UiTree::new(view(state, size));
            let frame = tree.layout(size);
            let texts: Vec<String> = frame.commands.iter().filter_map(|command| match command {
                limn::DrawCommand::Text { text, .. } => Some(text.clone()),
                _ => None,
            }).collect();
            (texts, frame.regions.iter().map(|region| region.id.clone()).collect::<Vec<_>>())
        };
        let (texts, ids) = shown(&state);
        assert!(texts.iter().any(|text| text == "Surco 9.0.0 está disponible") && ids.iter().any(|id| id == "update-toast"), "found, it is said over the window: {texts:?}");
        state.update(&limn::UiAction::Clicked { id: "update-toast".into() });
        let (texts, ids) = shown(&state);
        assert!(texts.iter().any(|text| text == "Actualización disponible") && ids.iter().any(|id| id == "update-get") && !ids.iter().any(|id| id == "update-toast"));
        state.apply(Event::UpdateProgress(0.42));
        let (texts, ids) = shown(&state);
        assert!(texts.iter().any(|text| text == "Descargando Surco 9.0.0") && texts.iter().any(|text| text == "42 %"), "{texts:?}");
        assert!(!ids.iter().any(|id| id == "update-get" || id == "update-restart"), "nothing to press while it comes down");
        state.apply(Event::UpdateInstalled);
        let (texts, ids) = shown(&state);
        assert!(texts.iter().any(|text| text == "Surco 9.0.0 está lista") && ids.iter().any(|id| id == "update-restart"));
    }

    #[test]
    fn a_shut_panel_peeks_from_the_window_edge_and_opens_from_there() {
        let mut state = State::default();
        state.update(&limn::UiAction::Clicked { id: "toggle-panel".into() });
        for width in [1100.0, 1400.0, 2200.0] {
            let size = Size::new(width, 700.0);
            let mut tree = UiTree::new(view(&state, size));
            let frame = tree.layout(size);
            let rect = |id: &str| frame.regions.iter().find(|region| region.id == id).map(|region| region.rect).unwrap_or_else(|| panic!("no {id} at {width}"));
            let (bell, settings, peek) = (rect("news"), rect("account"), rect("panel-peek"));
            assert!(bell.x + bell.width <= settings.x && settings.x + settings.width <= width, "at {width}: {bell:?} {settings:?}");
            assert_eq!(peek.x + peek.width, width, "the strip runs to the window's edge at {width}");
        }
        state.update(&limn::UiAction::Clicked { id: "panel-peek".into() });
        assert!(state.panel_open, "the strip opens what it peeks at");
    }

    #[test]
    fn the_lyrics_take_the_page_and_leave_the_panel_to_what_plays() {
        let mut state = State::default();
        state.show(PlayerView { current: Some(track("a")), duration: 200.0, playing: true, ..PlayerView::default() });
        state.update(&limn::UiAction::Clicked { id: "lyrics".into() });
        state.apply(Event::Lyrics { track_id: "a".into(), lines: Some(vec![surco_core::lyrics::Line { at_ms: 0, text: "one".into() }]) });
        let size = Size::new(1400.0, 800.0);
        let mut tree = UiTree::new(view(&state, size));
        let frame = tree.layout(size);
        let rect = |id: &str| frame.regions.iter().find(|region| region.id == id).map(|region| region.rect).unwrap();
        assert!(rect("lyric:0").x < rect("panel-album").x, "the line in the middle, what plays still at the side");
    }

    #[test]
    fn every_state_lays_out_at_narrow_and_wide_windows() {
        let mut searching = asked("q");
        searching.search();
        let mut failed = asked("q");
        failed.search();
        failed.apply(Event::SearchFailed { query: "q".into(), message: "sin red".into() });
        let mut empty = asked("q");
        empty.search();
        empty.apply(Event::SearchResults { query: "q".into(), found: Found::default() });
        for state in [State::default(), searching, failed, empty] {
            for width in [720.0, 1180.0, 2200.0] {
                let found = ids(&state, Size::new(width, 700.0));
                assert!(found.iter().any(|id| id == "nav:search"), "the search is always there at {width}");
                assert!(found.iter().any(|id| id == "volume"));
                assert!(found.iter().any(|id| id == "queue") && found.iter().any(|id| id == "equalizer"));
                assert!(found.iter().any(|id| id == "downloads"), "the way to what is kept on disk");
                assert!(!found.iter().any(|id| id == "seek"), "nothing to seek without a track");
            }
        }
    }

    #[test]
    fn the_greeting_stands_over_the_window_until_it_is_answered_and_leads_to_the_import() {
        let size = Size::new(1180.0, 700.0);
        let mut state = State::default();
        assert!(!ids(&state, size).iter().any(|id| id == "welcome-import"), "only when a start asks for it");
        state.welcome = true;
        let found = ids(&state, size);
        assert!(found.iter().any(|id| id == "welcome-import") && found.iter().any(|id| id == "welcome-close"));
        state.update(&limn::UiAction::Clicked { id: "welcome-import".into() });
        assert!(!state.welcome && state.settings.welcomed && state.importing.is_some(), "answered, it opens the import and is not offered again");
        state.mobile = true;
        state.welcome = true;
        let found = ids(&state, Size::new(390.0, 844.0));
        assert!(found.iter().any(|id| id == "welcome-close"), "the phone has it too");
        state.update(&limn::UiAction::Clicked { id: "welcome-close".into() });
        assert!(!state.welcome);
    }

    #[test]
    fn the_release_notes_page_asks_once_retries_a_failure_and_opens_a_video_where_it_is_kept() {
        use crate::backend::{Command, Patch};
        use crate::update::Note;
        let size = Size::new(1180.0, 700.0);
        let mut state = State::default();
        assert!(ids(&state, size).iter().all(|id| id != "patches"), "not on the home page");
        state.update(&limn::UiAction::Clicked { id: "settings".into() });
        state.update(&limn::UiAction::Clicked { id: "settings-part:about".into() });
        assert!(ids(&state, size).iter().any(|id| id == "patches"), "the way to them is in the settings, about Surco");
        assert_eq!(state.update(&limn::UiAction::Clicked { id: "patches".into() }), vec![Command::Patches]);
        assert!(state.update(&limn::UiAction::Clicked { id: "patches".into() }).is_empty(), "asked once while they come");
        state.apply(Event::Patches(Err("sin red".into())));
        assert!(ids(&state, size).iter().any(|id| id == "patches-retry"));
        assert_eq!(state.update(&limn::UiAction::Clicked { id: "patches-retry".into() }), vec![Command::Patches]);
        let video = "https://github.com/user-attachments/assets/clip";
        let notes = vec![Note::Text("### Added\n- Recomendaciones".into()), Note::Video(video.into())];
        state.apply(Event::Patches(Ok(vec![Patch { version: env!("CARGO_PKG_VERSION").into(), date: "2026-10-17".into(), notes }])));
        let link = ids(&state, size).into_iter().find(|id| id.ends_with(video)).expect("the video is a card to open");
        state.update(&limn::UiAction::Clicked { id: link });
        assert_eq!(state.open_url.take().as_deref(), Some(video));
        assert!(state.update(&limn::UiAction::Clicked { id: "patches".into() }).is_empty(), "once had, they are kept");
    }

    #[test]
    fn the_search_starts_from_a_browse_page_of_recent_searches_and_genres() {
        let size = Size::new(1180.0, 700.0);
        let has = |state: &State, id: &str| ids(state, size).iter().any(|found| found == id);
        let mut state = asked("Cerati");
        assert!(has(&state, "nav:search:clear") && has(&state, "nav:browse"), "a typed field has the cross that empties it before the way to browse");
        state.search();
        assert_eq!(state.collection.searches, ["Cerati"], "a search made on the desktop is remembered");
        state.update(&limn::UiAction::Clicked { id: "home".into() });
        assert!(has(&state, "nav:browse") && !has(&state, "nav:search:clear"), "an empty field ends in the way to browse");
        state.update(&limn::UiAction::Clicked { id: "browse".into() });
        assert_eq!(state.page, Page::Browse);
        for id in ["browse:Rock argentino", "browse:Pop latino"] {
            assert!(has(&state, id), "{id} is on the browse page");
        }
        assert!(!has(&state, "recent-search:0"), "the recent searches wait for the field, as Spotify's");
        state.search_focused = true;
        for id in ["recent-search:0", "recent-search-remove:0", "recent-search-clear"] {
            assert!(has(&state, id), "{id} drops down from the field that holds the keyboard");
        }
        state.query = "C".into();
        assert!(!has(&state, "recent-search:0"), "typing puts them away");
        state.query.clear();
        // As the window does each frame: the field takes the keyboard, they open, and they
        // leave the keyboard in the field, frame after frame, with no flicker.
        state.search_focused = false;
        let mut tree = UiTree::new(view(&state, size));
        let field = tree.layout(size).access.iter().find(|node| node.id == "nav:search").unwrap().rect;
        let point = limn::Point::new(field.x + 40.0, field.y + field.height / 2.0);
        let frame = |state: &mut State, tree: &mut UiTree, event: Option<limn::UiEvent>| {
            for action in event.map(|event| tree.handle_event(event)).unwrap_or_default() { state.update(&action); }
            state.search_focused = tree.focused_id().is_some_and(|id| id == "nav:search" || id.starts_with("recent-search"));
            if std::mem::take(&mut state.mobile_ui.blur) { tree.clear_focus(); }
            tree.set_root(view(state, size));
            tree.layout(size);
        };
        frame(&mut state, &mut tree, Some(limn::UiEvent::PointerDown(point)));
        frame(&mut state, &mut tree, Some(limn::UiEvent::PointerUp(point)));
        frame(&mut state, &mut tree, None);
        assert!(state.recent_searches_shown() && tree.focused_id() == Some("nav:search"), "{:?}", tree.focused_id());
        // A click away, on the page's heading, takes the keyboard and them with it.
        let heading = tree.layout(size).text_layouts().into_iter().find(|block| block.source == "Explorar todo").unwrap().runs[0].rect;
        let away = limn::Point::new(heading.x + 4.0, heading.y + heading.height / 2.0);
        frame(&mut state, &mut tree, Some(limn::UiEvent::PointerDown(away)));
        frame(&mut state, &mut tree, Some(limn::UiEvent::PointerUp(away)));
        assert!(!state.recent_searches_shown(), "{:?}", tree.focused_id());

        assert!(matches!(state.update(&limn::UiAction::Clicked { id: "browse:Rock argentino".into() }).as_slice(), [crate::backend::Command::Search(query)] if query == "Rock argentino"));
        assert_eq!(state.collection.searches, ["Rock argentino", "Cerati"]);
        state.update(&limn::UiAction::Clicked { id: "recent-search-remove:1".into() });
        assert_eq!(state.collection.searches, ["Rock argentino"]);
    }

    #[test]
    fn the_top_bar_leaves_the_window_buttons_their_end() {
        let size = Size::new(1180.0, 700.0);
        let at = |state: &State, id: &str| {
            let mut tree = UiTree::new(view(state, size));
            tree.layout(size).regions.iter().find(|region| region.id == id).unwrap().rect
        };
        let mut state = State::default();
        let (back, settings, search) = (at(&state, "back"), at(&state, "account"), at(&state, "nav:search"));
        state.caption = (0.0, 138.0);
        assert!(at(&state, "account").x + at(&state, "account").width <= size.width - 138.0, "Windows' buttons keep the right end");
        assert_eq!(at(&state, "back"), back);
        state.caption = (72.0, 0.0);
        assert!(at(&state, "back").x >= 72.0, "macOS's lights keep the left end");
        assert_eq!(at(&state, "account"), settings);
        assert!((at(&state, "nav:search").x - search.x).abs() < 0.5, "the search stays in the middle");
    }

    #[test]
    fn results_are_clickable_rows_and_playing_one_enables_the_transport() {
        let mut state = asked("q");
        state.search();
        let found = Found { tracks: vec![track("a"), track("b"), track("c")], ..Found::default() };
        state.apply(Event::SearchResults { query: "q".into(), found });
        let size = Size::new(1180.0, 700.0);
        let before = ids(&state, size);
        for id in ["track:0", "track:1", "track:2"] {
            assert!(before.iter().any(|found| found == id), "{id} is a row");
        }
        assert!(!before.iter().any(|id| id == "play-pause"), "transport is passive with nothing loaded");
        state.show(PlayerView { current: Some(track("b")), playing: true, duration: 200.0, ..PlayerView::default() });
        let after = ids(&state, size);
        for id in ["play-pause", "next", "previous", "shuffle", "repeat", "seek", "mute", "volume"] {
            assert!(after.iter().any(|found| found == id), "{id} is live while a track plays");
        }
    }

    #[test]
    fn the_right_panel_has_its_column_only_where_there_is_room() {
        let mut state = State::default();
        state.show(PlayerView { current: Some(track("a")), upcoming: vec![track("b")], duration: 200.0, ..PlayerView::default() });
        let at = |state: &State, width: f32| ids(state, Size::new(width, 700.0));
        let has = |found: &[String], id: &str| found.iter().any(|found| found == id);
        let wide = at(&state, 1400.0);
        assert!(has(&wide, "panel-artist") && has(&wide, "resize:left") && has(&wide, "resize:right"), "three panels and their handles");
        let narrow = at(&state, 700.0);
        assert!(!has(&narrow, "panel-artist") && !has(&narrow, "resize:left"), "one panel beside a rail when narrow");
        assert!(has(&narrow, "downloads") && has(&narrow, "nav:search"), "the rail and the search are still there");
        state.update(&limn::UiAction::Clicked { id: "queue".into() });
        assert!(has(&at(&state, 1400.0), "upcoming:0") && has(&at(&state, 700.0), "upcoming:0"), "the queue shows at any width");
        assert!(!has(&at(&state, 1400.0), "queue-clear"), "nothing was queued by hand");
        state.show(PlayerView { current: Some(track("a")), upcoming: vec![track("b"), track("c")], queued: 1, duration: 200.0, ..PlayerView::default() });
        assert!(has(&at(&state, 1400.0), "queue-clear") && has(&at(&state, 1400.0), "upcoming:1"));
        // The library's search, once its magnifier opens it, and its order share a row inside the panel.
        assert!(has(&at(&state, 1400.0), "library-search") && !has(&at(&state, 1400.0), "library-query"));
        state.update(&limn::UiAction::Clicked { id: "library-search".into() });
        let mut tree = UiTree::new(view(&state, Size::new(1400.0, 700.0)));
        let frame = tree.layout(Size::new(1400.0, 700.0));
        let rect = |id: &str| frame.regions.iter().find(|region| region.id == id).map(|region| region.rect).unwrap();
        let (query, sort, list) = (rect("library-query"), rect("library-sort"), rect("downloads"));
        // A row of the queue taken by the pointer and let go over another is dropped on it.
        let middle = |id: &str| limn::Point::new(rect(id).x + rect(id).width / 2.0, rect(id).y + rect(id).height / 2.0);
        tree.handle_event(limn::UiEvent::PointerDown(middle("upcoming:1")));
        tree.handle_event(limn::UiEvent::PointerMove(middle("upcoming:0")));
        let dropped = tree.handle_event(limn::UiEvent::PointerUp(middle("upcoming:0")));
        assert!(matches!(dropped.as_slice(), [limn::UiAction::Dropped { id, target: Some(target), .. }] if id == "upcoming:1" && target == "upcoming:0"), "{dropped:?}");
        assert!(query.width > 80.0 && query.x + query.width <= sort.x && sort.x + sort.width <= state.left_width + 16.0 && query.y + query.height <= list.y, "{query:?} {sort:?} {list:?}");
        state.update(&limn::UiAction::Clicked { id: "queue".into() });
        assert!(!has(&at(&state, 1400.0), "resize:right"), "a closed panel leaves no handle");
    }

    #[test]
    fn no_two_controls_of_a_busy_window_answer_to_one_id() {
        let mut state = State::default();
        state.show(PlayerView { current: Some(track("a")), upcoming: vec![track("b")], duration: 200.0, playing: true, ..PlayerView::default() });
        state.collection.playlists.push(Playlist { id: 1, name: "Lista".into(), collaborative: false, picture: None, tracks: vec![track("a"), track("b")] });
        state.collection.liked.push(track("b"));
        state.page = Page::Playlist(1);
        state.panel_more_open = true;
        for width in [1100.0, 1600.0] {
            let found = ids(&state, Size::new(width, 900.0));
            let mut seen = std::collections::HashSet::new();
            let twice: Vec<&String> = found.iter().filter(|id| !seen.insert(*id)).collect();
            assert!(twice.is_empty(), "at {width}, more than one control answers to {twice:?}");
        }
    }

    #[test]
    fn a_song_among_the_recent_searches_has_its_options_and_the_list_its_clear() {
        let mut state = State::default();
        state.search_focused = true;
        state.collection.searches = vec![Searched::Track { track: track("a") }, Searched::Query("cafe".into())];
        let found = ids(&state, Size::new(1400.0, 900.0));
        for id in ["recent-search:0", "more:recent-search:0", "recent-search-remove:0", "recent-search:1", "recent-search-clear"] {
            assert!(found.iter().any(|found| found == id), "{id} among {found:?}");
        }
        assert!(!found.iter().any(|found| found == "more:recent-search:1"), "what was typed has no song's options");
        assert_eq!(state.track_at("recent-search:0").map(|track| track.id.as_str()), Some("a"), "its options act on the song");
    }

    #[test]
    fn playlists_settings_and_their_dialogs_offer_their_controls() {
        let mut state = State::default();
        let size = Size::new(1400.0, 1000.0);
        let has = |state: &State, wanted: &[&str]| {
            let found = ids(state, size);
            for id in wanted {
                assert!(found.iter().any(|found| found == id), "{id} is there: {found:?}");
            }
        };
        has(&state, &["playlist-new", "account"]);
        state.update(&limn::UiAction::Clicked { id: "account".into() });
        has(&state, &["settings"]);
        state.update(&limn::UiAction::Clicked { id: "account".into() });
        let click = |state: &mut State, id: &str| drop(state.update(&limn::UiAction::Clicked { id: id.into() }));

        click(&mut state, "playlist-new");
        has(&state, &["playlist-name", "playlist-kind:normal", "playlist-kind:collaborative", "playlist-create", "playlist-cancel"]);
        click(&mut state, "playlist-kind:collaborative");
        click(&mut state, "playlist-create");
        has(&state, &["playlist:0", "page-more"]);
        click(&mut state, "page-more");
        has(&state, &["playlist-delete"]);
        click(&mut state, "playlist-delete");
        assert!(!state.page_more_open, "a choice closes the menu");
        has(&state, &["confirm-yes", "confirm-cancel"]);
        click(&mut state, "confirm-cancel");

        click(&mut state, "settings");
        state.apply(Event::Outputs(Vec::new()));
        has(&state, &["settings-part:playback", "settings-part:devices"]);
        click(&mut state, "settings-part:playback");
        has(&state, &["setting:autoplay", "setting:crossfade"]);
        assert!(ids(&state, Size::new(720.0, 1000.0)).iter().any(|id| id == "setting:autoplay"), "and in a narrow window");
        click(&mut state, "back");
        click(&mut state, "settings-part:devices");
        has(&state, &["setting:output", "setting:outputs-refresh"]);
    }

    #[test]
    fn settings_keep_the_output_picker_and_library_choices_inside_a_narrow_page() {
        let mut state = State::default();
        for width in [360.0, 900.0] {
            let size = Size::new(width, 4000.0);
            // Each part of the settings on its own page.
            let mut frame = limn::Frame::default();
            for part in crate::state::SettingsPart::ALL {
                state.page = Page::SettingsPart(part);
                frame.access.extend(UiTree::new(settings_page(&state, width, SCHEME.palette())).layout(size).access);
            }
            for id in ["setting:output", "setting:outputs-refresh", "library-sort:alphabetical", "library-view:grid", "setting:library-panel", "setting:player-panel", "setting:motion"] {
                let node = frame.access.iter().find(|node| node.id == id).unwrap_or_else(|| panic!("{id} at {width}"));
                assert!(node.rect.x >= 0.0 && node.rect.x + node.rect.width <= width, "{id}: {:?}", node.rect);
                assert!(node.actionable, "{id} can be used");
            }
            let autoplay = frame.access.iter().find(|node| node.id == "setting:autoplay").unwrap();
            assert_eq!(autoplay.name, "Reproducción automática");
            assert!(autoplay.rect.height >= 24.0, "the switch is comfortable to target");
        }
    }

    #[test]
    fn connections_explain_loading_empty_and_error_states_without_hiding_the_system_output() {
        let size = Size::new(900.0, 4000.0);
        let shown = |state: &State| {
            let mut tree = UiTree::new(settings_page(state, size.width, SCHEME.palette()));
            tree.layout(size)
        };
        let has_text = |frame: &limn::Frame, text: &str| frame.text_layouts().iter().any(|layout| layout.source.contains(text));
        let mut state = State::default();
        state.page = Page::SettingsPart(crate::state::SettingsPart::Devices);
        let frame = shown(&state);
        assert!(has_text(&frame, "No se encontraron salidas"));
        assert!(has_text(&frame, "Bluetooth"));
        assert!(frame.access.iter().any(|node| node.id == "setting:output" && node.actionable));

        state.outputs_loading = true;
        let frame = shown(&state);
        assert!(has_text(&frame, "Buscando salidas"));
        assert!(frame.access.iter().any(|node| node.id == "setting:outputs-refresh" && node.disabled));

        state.outputs_loading = false;
        state.outputs_error = Some("El servidor de audio no responde.".into());
        state.output_notice = Some("Volvimos a la salida predeterminada.".into());
        let frame = shown(&state);
        assert!(has_text(&frame, "El servidor de audio no responde."));
        assert!(has_text(&frame, "Volvimos a la salida predeterminada."));
        assert!(frame.access.iter().any(|node| node.id == "setting:outputs-refresh" && node.actionable));

        state.outputs = vec![surco_audio::AudioOutputDevice { id: "speakers".into(), name: "Parlantes del escritorio".into(), is_default: true }];
        let frame = shown(&state);
        let selected = frame.access.iter().find(|node| node.id == "setting:output").unwrap();
        assert_eq!(selected.value_text.as_deref(), Some("Sistema: Parlantes del escritorio"));
        state.output = Some("disconnected".into());
        let frame = shown(&state);
        let selected = frame.access.iter().find(|node| node.id == "setting:output").unwrap();
        assert_eq!(selected.value_text.as_deref(), Some("Salida guardada no disponible"));
    }

    #[test]
    fn each_panel_folds_from_its_own_header_and_opens_from_what_it_leaves() {
        let mut state = State::default();
        let at = |state: &State| ids(state, Size::new(1400.0, 700.0));
        let has = |found: &[String], id: &str| found.iter().any(|found| found == id);
        let open = at(&state);
        assert!(has(&open, "toggle-library") && has(&open, "toggle-panel"));
        assert!(!has(&open, "library-rail:expand") && !has(&open, "panel-peek"));
        state.update(&limn::UiAction::Clicked { id: "toggle-library".into() });
        state.update(&limn::UiAction::Clicked { id: "toggle-panel".into() });
        let shut = at(&state);
        assert!(has(&shut, "library-rail:expand") && has(&shut, "panel-peek"), "the rail and the strip open them: {shut:?}");
        assert!(has(&shut, "downloads") && has(&shut, "resize:left") && !has(&shut, "resize:right"), "the library folds to its pictures, still dragged open from its edge");
        assert!(has(&shut, "nav:search") && has(&shut, "nav:home"), "the bars stay");
        state.update(&limn::UiAction::Clicked { id: "library-rail:expand".into() });
        state.update(&limn::UiAction::Clicked { id: "panel-peek".into() });
        assert!(state.library_open && state.panel_open);
    }

    #[test]
    fn home_and_the_library_offer_what_was_opened_and_what_is_kept() {
        let mut state = State::default();
        let size = Size::new(1400.0, 900.0);
        let has = |state: &State, id: &str| ids(state, size).iter().any(|found| found == id);
        assert!(has(&state, "filter:all") && has(&state, "filter:albums") && has(&state, "downloads"));
        assert!(!has(&state, "recent:0"), "nothing was opened yet");

        let album = Album { id: "lp".into(), title: "Album lp".into(), artist: "Artist".into(), artist_id: None, year: "2017".into(), artwork_url: None, tracks: vec![track("t1")] };
        state.page = Page::Loading(Target::Album("lp".into()));
        state.apply(Event::Opened { target: Target::Album("lp".into()), result: Ok(Content::Album(album)) });
        assert!(has(&state, "keep"));
        state.update(&limn::UiAction::Clicked { id: "keep".into() });
        assert!(has(&state, "kept:0"), "the library lists it beside its page");
        state.update(&limn::UiAction::Clicked { id: "home".into() });
        assert!(has(&state, "recent:0"), "home offers what was opened");
        state.update(&limn::UiAction::Clicked { id: "filter:artists".into() });
        assert!(!has(&state, "downloads"), "a filter leaves only its kind in the library");
        assert!(ids(&state, Size::new(700.0, 900.0)).iter().any(|found| found == "recent:0"), "and at a narrow window");
    }

    #[test]
    fn the_panels_follow_the_widths_the_user_gave_them_and_spare_the_content() {
        let mut state = State::default();
        let edges = |state: &State, width: f32| {
            let size = Size::new(width, 700.0);
            let mut tree = UiTree::new(view(state, size));
            let frame = tree.layout(size);
            let x = |id: &str| frame.regions.iter().find(|region| region.id == id).map(|region| region.rect.x).unwrap();
            (x("resize:left"), x("resize:right"))
        };
        assert_eq!(edges(&state, 1400.0), (GAP + 280.0, 1400.0 - GAP - 320.0 - GAP));
        state.left_width = 400.0;
        state.right_width = 440.0;
        assert_eq!(edges(&state, 1400.0), (GAP + 400.0, 1400.0 - GAP - 440.0 - GAP));
        let (left, right) = edges(&state, 1040.0);
        assert!(right - (left + GAP) >= CONTENT_MIN_WIDTH, "the content keeps its width: {left}..{right}");
    }

    #[test]
    fn the_player_bar_keeps_every_control_inside_the_window() {
        let mut state = State::default();
        state.show(PlayerView { current: Some(track("a")), duration: 200.0, ..PlayerView::default() });
        for width in [698.0, 720.0, 960.0, 1400.0] {
            let size = Size::new(width, 700.0);
            let mut tree = UiTree::new(view(&state, size));
            for region in tree.layout(size).regions {
                let rect = region.rect;
                assert!(rect.x >= 0.0 && rect.x + rect.width <= width, "{} spans {}..{} at {width}", region.id, rect.x, rect.x + rect.width);
            }
        }
    }

    #[test]
    fn search_artist_and_album_pages_offer_their_tiles_tracks_and_way_back() {
        let release = |id: &str| AlbumRef { id: id.into(), title: format!("Album {id}"), artist: Some("Artist".into()), year: "2017".into(), artwork_url: None };
        let mut state = asked("q");
        state.search();
        let found = Found {
            artists: vec![ArtistRef { id: "who".into(), name: "Artist".into(), subscribers: None, artwork_url: None }],
            albums: vec![release("lp")],
            tracks: vec![track("a")],
        };
        state.apply(Event::SearchResults { query: "q".into(), found });
        // Tall enough that nothing asked for below scrolls out of the pane.
        let size = Size::new(1180.0, 1400.0);
        let has = |state: &State, wanted: &[&str]| {
            let found = ids(state, size);
            for id in wanted {
                assert!(found.iter().any(|found| found == id), "{id} is on the page: {found:?}");
            }
        };
        has(&state, &["artist:0", "album:0", "track:0", "more:track:0"]);
        state.update(&limn::UiAction::Clicked { id: "more:track:0".into() });
        has(&state, &["do:queue:track:0", "do:next:track:0", "do:share:track:0", "do:artist:track:0", "do:save:track:0"]);
        state.update(&limn::UiAction::Clicked { id: "do:share:track:0".into() });
        has(&state, &["share-card", "share-copy"]);
        state.update(&limn::UiAction::Clicked { id: "share-close".into() });
        state.update(&limn::UiAction::Clicked { id: "menu-close".into() });

        state.update(&limn::UiAction::Clicked { id: "artist:0".into() });
        let artist = Artist {
            id: "who".into(),
            name: "Artist".into(),
            subscribers: Some("4.36M".into()),
            artwork_url: None,
            songs: vec![track("s1"), track("s2")],
            albums: vec![release("lp")],
            singles: vec![release("single")],
        };
        state.apply(Event::Opened { target: Target::Artist("who".into()), result: Ok(Content::Artist(artist)) });
        has(&state, &["back", "page-play", "page-shuffle", "page-track:0", "page-track:1", "page-album:0", "page-single:0"]);

        state.update(&limn::UiAction::Clicked { id: "page-album:0".into() });
        has(&state, &["back"]);
        let album = Album {
            id: "lp".into(),
            title: "Album lp".into(),
            artist: "Artist".into(),
            artist_id: Some("who".into()),
            year: "2017".into(),
            artwork_url: None,
            tracks: vec![track("t1"), track("t2")],
        };
        state.apply(Event::Opened { target: Target::Album("lp".into()), result: Ok(Content::Album(album)) });
        has(&state, &["back", "page-artist", "page-play", "page-track:0", "page-track:1"]);

        state.update(&limn::UiAction::Clicked { id: "page-artist".into() });
        state.apply(Event::Opened { target: Target::Artist("who".into()), result: Err("sin red".into()) });
        has(&state, &["back", "retry"]);
    }

    #[test]
    fn the_queue_and_the_equalizer_open_at_any_width() {
        let mut state = State::default();
        state.show(PlayerView { current: Some(track("a")), upcoming: vec![track("b"), track("c")], duration: 200.0, ..PlayerView::default() });
        state.update(&limn::UiAction::Clicked { id: "queue".into() });
        state.update(&limn::UiAction::Clicked { id: "equalizer".into() });
        for width in [720.0, 1180.0] {
            let found = ids(&state, Size::new(width, 700.0));
            for id in ["upcoming:0", "upcoming:1", "equalizer:preamp", "equalizer:0", "equalizer:9", "equalizer-reset"] {
                assert!(found.iter().any(|found| found == id), "{id} is there at {width}");
            }
        }
    }
}
