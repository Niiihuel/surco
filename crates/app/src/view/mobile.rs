//! Phone layout over Surco's existing pages, actions and playback state.

use super::*;
use std::borrow::Cow;

pub(super) mod search;
pub(super) mod home;
mod player;
pub(super) mod lyrics;
mod chrome;
pub(super) mod controls;
mod sheets;
mod queue;
pub(super) mod details;
pub(super) mod settings;
#[cfg(test)]
mod dialog_tests;
#[cfg(test)]
mod native_ui_geometry_tests;
#[cfg(test)]
mod edge_chrome_tests;
#[cfg(test)]
mod notice_tests;

const TAP: f32 = 44.0;
/// The floating tab bar's pill.
const NAV_HEIGHT: f32 = 62.0;
const HEADER_HEIGHT: f32 = MOBILE_HEADER_HEIGHT;
/// The mini-player's pill and the gap that floats it above the tabs.
const MINI_HEIGHT: f32 = MINI_PILL + 8.0;
const MINI_PILL: f32 = 56.0;
/// The floating bars keep this clear of the screen's sides, and grow no wider than BAR_MAX.
const BAR_SIDE: f32 = 19.0;
const BAR_MAX: f32 = 440.0;
// shortcut: limn has no backdrop blur, and blending in linear light lets even 8% of a white
// row through a dark tint, so the frosted bars are opaque; let rows pass blurred beneath them
// once limn can blur.
const FROST: Color = Color::rgb(24, 24, 26);
const HAIRLINE: Color = Color::rgba(255, 255, 255, 26);
/// The hairline every thumbnail keeps, so a dark cover does not sink into the page.
pub(in crate::view) const THUMB_RIM: Color = Color::rgba(255, 255, 255, 38);
/// The black over a phone's video while its controls stand on it, as BitChord's 50%, and
/// the shade at its foot under the song's name once they are put away.
const CANVAS_DIM: Color = Color::rgba(0, 0, 0, 128);
const CANVAS_FOOT: Color = Color::rgba(0, 0, 0, 115);
/// The chosen tab's platter.
const TAB_LIT: Color = Color::rgba(255, 255, 255, 36);
pub(super) const SCROLL_INDICATOR: limn::ScrollbarVisibility = limn::ScrollbarVisibility::AutoHide { hold_ms: 650, fade_ms: 220 };

pub(super) fn icon(id: &str, glyph: Icon, label: &str, palette: ComponentPalette) -> Element {
    controls::action(id, glyph, label, 22.0, palette)
}

/// A phone's search field with its cross inside its end, a plain one, whose `id` empties or
/// shuts it.
// shortcut: limn's own clear button squares the field's corners, so this one lies over its end
// and a query longer than the field passes under it; use limn's once it keeps them.
pub(super) fn inside_cross(field: Element, id: &str, label: &str, palette: ComponentPalette) -> Element {
    let cross = Element::icon_button(id.to_owned(), Icon::Close, "").foreground(palette.muted).pressed_foreground(palette.text)
        .accessible(limn::Access::new(limn::Role::Button).name(label)).size(Some(TAP), Some(TAP));
    Element::stack(vec![field, Element::row(vec![Element::spacer().flex(1.0), cross])])
}

/// Spotify's chip on a phone: a 32pt pill, lit while chosen, on a touch target of full
/// height, its words on one line.
pub(in crate::view) fn chip(id: String, label: &str, on: bool, role: limn::Role, palette: ComponentPalette) -> Element {
    let copy = Text::new(label, palette).font_size(14.0).render().foreground(if on { SCHEME.canvas } else { palette.text }).centered_y();
    let edge = || Element::spacer().size(Some(14.0), None);
    let pill = Element::panel(Element::row(vec![edge(), copy, edge()]))
        .background(if on { palette.text } else { Color::rgb(42, 42, 42) }).pressed_background(if on { ROUND_HELD } else { Color::rgb(62, 62, 62) })
        .radius(16.0).size(None, Some(32.0));
    Element::clickable(id, pill.centered_y()).accessible(limn::Access::new(role).name(label).selected(on)).size(None, Some(TAP))
}

/// The account's face in a tab's header.
const ACCOUNT_FACE: f32 = 32.0;

pub(super) fn view(state: &State, window: Size) -> Element {
    let palette = SCHEME.palette();
    let insets = state.mobile_ui.content_insets;
    let width = (window.width - insets.left - insets.right).max(TAP);
    let surface = page_surface(state);
    let searching = state.page == Page::Search && state.mobile_search;
    // BitChord's queue opens in the player, on the lyrics' shell: the song above, its controls below.
    let queue = state.page == Page::Playing && state.queue_open;
    let title = title(state);
    let mut header = Vec::new();
    // Where a tab starts there is no way back, as Spotify's on a phone: the account's face is
    // there instead, and opens the profile.
    let tab = matches!(state.page, Page::Search | Page::Library | Page::Browse) && !state.mobile_search;
    // BitChord's bar has no pane of its own: its buttons float as discs. The player's stand
    // bare on the song.
    let disc = |button: Element| if state.page == Page::Playing { button } else { button.background(FROST).border(HAIRLINE, 0.5) };
    if tab {
        let face = Element::clickable("account-profile", super::profile_face(state, ACCOUNT_FACE, palette).centered()).size(Some(TAP), Some(TAP))
            .accessible(limn::Access::new(limn::Role::Button).name(t!("desktop-profile")));
        header.push(face.centered_y());
    }
    // Over the player's banner its handle stands alone, as BitChord's, with what plays under it.
    let banner_head = state.page == Page::Playing && !state.queue_open;
    if ((state.can_go_back() && !tab) || state.page == Page::Playing) && !banner_head {
        header.push(disc(icon(if state.page == Page::Playing && !state.queue_open { "mobile-player-dismiss" } else if state.can_go_back() { "back" } else { "home" }, if state.page == Page::Playing { Icon::ChevronDown } else { Icon::ArrowLeft }, &t!("common-back"), palette)));
    }
    let detail = matches!(state.page, Page::Album(_) | Page::Artist(_) | Page::Playlist(_) | Page::Mix(_));
    // Put away over a bare video, the controls take the handle with them.
    let bare = state.mobile_ui.controls_away && canvas_ready(state).is_some();
    let copy = if banner_head && bare {
        Element::spacer()
    } else if banner_head {
        player_handle(&title, palette)
    } else if detail && !state.mobile_ui.compact_header {
        // The page's own heading names it under its cover; the bar does once that has scrolled away.
        Element::spacer()
    } else if state.page == Page::Playing {
        Text::new(title.as_ref(), palette).font_size(14.0).truncate().render()
    } else {
        // A tab's name stands large, as BitChord's headings; a page pushed on it a step smaller.
        let size = if state.mobile_ui.compact_header { 18.0 } else if tab { 30.0 } else { 24.0 };
        Text::new(title.as_ref(), palette).font_size(size).truncate().render().font_family(FontFamily::SansBold)
    };
    header.push(if state.page == Page::Playing { copy.centered().flex(1.0) } else { copy.centered_y().flex(1.0) });
    if state.page == Page::Library {
        header.push(disc(icon("playlist-new", Icon::Add, &t!("common-create-playlist"), palette)));
    }
    // A list's search and order, as BitChord's: together in a floating pill of the bar. An
    // artist's page has neither, as BitChord's has not.
    if matches!(state.page, Page::Album(_) | Page::Playlist(_) | Page::Mix(_)) && !state.page_tracks().is_empty() {
        header.push(details::bar_tools(state, palette));
    }
    if !matches!(state.page, Page::Settings | Page::SettingsPart(_) | Page::Equalizer | Page::Playing | Page::Album(_) | Page::Artist(_) | Page::Playlist(_)) {
        header.push(disc(icon("settings", Icon::Settings, &t!("common-settings"), palette)));
    }
    let editing = searching && state.mobile_ui.search_editing;
    let keyboard = editing || state.mobile_ui.keyboard_visible;
    // Under Liquid Glass the field floats at the foot, as Apple Music's, below a plain header.
    let field_on_top = search_in_header(state);
    let header_height = if field_on_top { state.mobile_ui.search_header.unwrap_or(if state.mobile_search_compact() { 0.0 } else { search_header_height(state) }) } else if state.page == Page::Lyrics || queue { lyrics::head_height(window.height) } else { HEADER_HEIGHT };
    // UIKit draws the header and tabs over these slots outside the song pages.
    let native_chrome = state.mobile_ui.native_bars && state.mobile_ui.native_surfaces && !matches!(state.page, Page::Playing | Page::Lyrics);
    let header = if native_chrome {
        Element::spacer().accessible(limn::Access::new(limn::Role::Group).id("ios-header").name(title.as_ref()))
    } else if state.page == Page::Lyrics || queue { lyrics::header(state, window.height, palette) }
        else { horizontal_inset(Element::row(header).gap(8.0).padding(6.0), 10.0, 10.0) }
        .size(None, Some(header_height)).clip_children();
    let mut top = vec![Element::spacer().size(None, Some(insets.top)), header];
    let mut top_height = insets.top + header_height;
    if field_on_top {
        top.push(search::field(state, palette));
        top_height += search::FIELD_HEIGHT;
    }
    let mut content = if queue {
        queue::content(state, palette)
    } else if searching {
        search::content(state, width, palette)
    } else if state.page == Page::Search {
        home::content(state, width, palette)
    } else if state.page == Page::Browse {
        explore(state, width, palette)
    } else if state.page == Page::Lyrics {
        // What the lines are seen through, between the header and the transport.
        lyrics(state, width, window.height - top_height - insets.bottom - lyrics::footer_height(state, window.height), palette)
    } else if state.page == Page::Playing && bare {
        player::bare(state, width, palette)
    } else if state.page == Page::Playing {
        player::screen(state, width, window, top_height, palette)
    } else {
        page(state, width, palette)
    };
    if state.page == Page::Lyrics {
        // The mobile shell owns the full-height song color, including plain/error states.
        content.style.bottom_background_fade = None;
    }
    let content = with_notice(content, state, palette);
    let mut bottom = Vec::new();
    let mut bottom_height = insets.bottom;
    // The tab bar floats off the foot, by at least 15 where the system leaves no inset.
    let mut lift = insets.bottom;
    if state.page == Page::Lyrics || queue {
        bottom.push(lyrics::footer(state, window.height, palette));
        bottom_height += lyrics::footer_height(state, window.height);
    } else if state.page != Page::Playing && !keyboard {
        if let Some(track) = state.current() {
            // Folded, the mini player rides in the tabs' row; its room stays, so the page does not jump.
            if native_chrome || !state.mobile_ui.bar_inline {
                bottom.push(if native_chrome && glass(state) { mini_slot(track) } else { mini_player(state, track, width, palette) });
            }
            bottom_height += MINI_HEIGHT;
        }
    }
    if searching && !field_on_top {
        // Kept while typing: it rides the keyboard, which the bottom inset makes room for.
        bottom.push(search::foot(keyboard));
        bottom_height += NAV_HEIGHT;
    } else if !matches!(state.page, Page::Playing | Page::Lyrics) && !keyboard {
        bottom.push(if native_chrome {
            Element::spacer().size(None, Some(NAV_HEIGHT))
                .accessible(limn::Access::new(limn::Role::Group).id("ios-tabs").name(t!("mobile-tabs")))
                .block_pointer("mobile-nav-bar")
        } else { navigation(state, width, palette) });
        bottom_height += NAV_HEIGHT;
        if !native_chrome { lift = insets.bottom.max(15.0) + 6.0; }
    }
    bottom.push(Element::spacer().size(None, Some(lift)));
    bottom_height += lift - insets.bottom;
    let route = route_key(state);
    let content = if (state.page == Page::Playing && !state.queue_open) || state.mobile_ui.revealed.as_deref() == Some(route.as_str()) { content }
        else { content.animate_enter(route) };
    let header_edge = mix(surface.0, surface.1, (top_height / window.height.max(1.0)).clamp(0.0, 1.0));
    let song_surface = matches!(state.page, Page::Playing | Page::Lyrics);
    let footer_edge = if song_surface {
        mix(surface.0, surface.1, ((window.height - bottom_height) / window.height.max(1.0)).clamp(0.0, 1.0))
    } else { Color::TRANSPARENT };
    // Natively, rows pass beneath a blurred header and fade out behind the transport. The
    // player has no bar below, only the home indicator's room: it runs clear to the bottom.
    let player = state.page == Page::Playing;
    let ground = song_ground(state, window);
    // Over the blurred cover the bars are clear, as the cover runs beneath them.
    let (header_edge, footer_edge) = if native_chrome || ground.is_some() { (Color::TRANSPARENT, Color::TRANSPARENT) } else { (header_edge, footer_edge) };
    let surface = if ground.is_some() { (Color::TRANSPARENT, Color::TRANSPARENT) } else { surface };
    // The large title is the page's first row, scrolling up under the bar; over Search it
    // stays in the header, above the field.
    let scroll_top = top_height + if large_title(state) && !field_on_top { MOBILE_LARGE_TITLE } else { 0.0 };
    // limn fades an edge into a colour, which the blurred cover has none of: over it the lines
    // are cut between the bars instead of passing under them.
    let content = if ground.is_some() && (state.page == Page::Lyrics || queue) {
        Element::column(vec![Element::spacer().size(None, Some(scroll_top)), content.flex(1.0), Element::spacer().size(None, Some(bottom_height))])
    } else { chrome::under_bars(content, scroll_top, bottom_height, header_edge, if player { Color::TRANSPARENT } else { footer_edge }) };
    let top = Element::column(top);
    let top = if native_chrome { top } else { top.vertical_gradient(vec![surface.0, header_edge]) }.block_pointer("mobile-header-bar");
    let footer = Element::column(bottom);
    // The scrim is the tab bar's; the lyrics' transport stands on the song's colour.
    let footer = if player { footer }
        else if native_chrome && state.mobile_ui.glass { footer.vertical_gradient(chrome::glass_fade(SCHEME.canvas)) }
        else if native_chrome { footer.vertical_gradient(chrome::spotify_fade(SCHEME.canvas)) }
        else if song_surface { footer.background(footer_edge).vertical_gradient(vec![footer_edge, surface.1]) }
        else { footer.vertical_gradient(chrome::floating_fade(SCHEME.canvas)) }.block_pointer("mobile-footer-bar");
    let bars = Element::column(vec![top, Element::spacer().flex(1.0), footer]);
    let content = Element::stack(vec![content, bars]);
    let content = horizontal_inset(Element::column(vec![content.flex(1.0)]), insets.left, insets.right);
    let content = match flying_cover(state, window) { Some(cover) => Element::stack(vec![cover, content]), None => content };
    let content = match player_canvas(state, window) { Some(canvas) => Element::stack(vec![canvas, content]), None => content };
    let content = match player_banner(state, window) { Some(banner) => Element::stack(vec![banner, content]), None => content };
    let content = match ground { Some(ground) => Element::stack(vec![ground, content]), None => content };
    let tree = Element::panel(content).background(SCHEME.canvas).vertical_gradient(vec![surface.0, surface.1])
        .size(Some(window.width), Some(window.height));
    let tree = if state.page == Page::Playing { tree.translate(limn::Point::new(0.0, state.mobile_ui.player_offset)) } else { tree };
    // The cover goes its own way as the player comes down, into the mini player's thumbnail.
    let tree = match docking_cover(state, window) { Some(cover) => Element::stack(vec![tree, cover]), None => tree };
    let tree = Element::panel(tree).size(Some(window.width), Some(window.height)).clip_children();
    // Replay's story stands over everything, the whole screen, on every platform.
    super::wrapped::story(state, dialogs(state, tree, window.width, palette, cfg!(target_os = "ios") && state.mobile_ui.native_surfaces), window)
}

/// BitChord's Explore: the catalog's moods and genres two to a row, each a card of its colour
/// with its first list's cover redrawn in it. While they come their places pulse; without
/// them, the genres Surco names itself, each a search.
fn explore(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let inner = width - 2.0 * MOOD_GUTTER;
    let (card, height) = ((inner - MOOD_GAP) / 2.0, ((inner - MOOD_GAP) / 2.0 / MOOD_ASPECT).round());
    let grid = match &state.moods {
        Some(Ok(moods)) if !moods.is_empty() => {
            let cards: Vec<Element> = moods.iter().enumerate().map(|(index, mood)| mood_card(state, index, mood, card, height, palette)).collect();
            pairs(cards)
        }
        Some(_) => search::browse(usize::MAX, inner, 2, height, palette),
        None => pairs((0..12).map(|_| limn::Skeleton::new("mood-coming", palette).render().radius(MOOD_RADIUS).size(Some(card), Some(height))).collect()),
    };
    page_scroll("mobile-explore", Element::column(vec![grid]).padding(MOOD_GUTTER))
}

/// Cards two to a row, `MOOD_GAP` apart both ways.
fn pairs(cards: Vec<Element>) -> Element {
    let mut cards = cards.into_iter();
    let mut rows = Vec::new();
    while let Some(first) = cards.next() {
        rows.push(Element::row(vec![first, cards.next().unwrap_or_else(Element::spacer).flex(1.0)]).gap(MOOD_GAP));
    }
    Element::column(rows).gap(MOOD_GAP)
}

/// BitChord's mood card: corners of 18, width over height 1.72, 16 from the sides, 12 apart.
const MOOD_GUTTER: f32 = 16.0;
const MOOD_GAP: f32 = 12.0;
const MOOD_ASPECT: f32 = 1.72;
const MOOD_RADIUS: f32 = 18.0;
/// How much of the card its colour's stripe takes before the cover starts.
const MOOD_STRIPE: f32 = 0.28;

/// A mood as BitChord's Explore draws it: a stripe of its colour down the left, its first list's
/// cover filling the rest as a duotone of that colour, and its name over a soft shade.
fn mood_card(state: &State, index: usize, mood: &surco_youtube::Mood, width: f32, height: f32, palette: ComponentPalette) -> Element {
    let color = crate::state::mood_color(mood);
    let base = Color::rgb((color >> 16) as u8, (color >> 8) as u8, color as u8);
    let (stripe, shadow, light) = (mix(base, Color::rgb(0, 0, 0), 0.08), mix(base, Color::rgb(0, 0, 0), 0.68), mix(base, Color::rgb(255, 255, 255), 0.32));
    let cover = state.mood_lists.get(&mood.params).and_then(|lists| lists.iter().find_map(|list| list.cover.as_deref()))
        .and_then(|cover| state.artwork.get(&crate::state::duotone_key(cover, color)));
    let side = width * (1.0 - MOOD_STRIPE);
    let art = match cover {
        Some(image) => Element::raster_image(image.clone().cover()).size(Some(side), Some(height)),
        None => Element::panel(Element::spacer()).background(mix(shadow, light, 0.45)).size(Some(side), Some(height)),
    };
    // Pale stripes, as Energize's cream, would swallow white words: a shade under the name.
    let shade = Element::panel(Element::spacer()).vertical_gradient(vec![Color::rgba(0, 0, 0, 61), Color::TRANSPARENT, Color::TRANSPARENT]);
    let name = Text::new(&mood.title, palette).font_size(17.0).render().max_lines(2).font_family(FontFamily::SansBold).foreground(Color::rgb(255, 255, 255));
    let card = Element::stack(vec![
        Element::panel(Element::spacer()).background(stripe),
        Element::row(vec![Element::spacer().flex(1.0), art]),
        shade,
        Element::column(vec![name, Element::spacer().flex(1.0)]).padding(12.0),
    ]).size(Some(width), Some(height)).radius(MOOD_RADIUS).clip_children();
    Element::clickable(format!("mood:{index}"), card).accessible(limn::Access::new(limn::Role::Button).name(&mood.title)).radius(MOOD_RADIUS).flex(1.0)
}

/// A mood's or genre's page: its lists two to a row, each its cover over its name and maker;
/// pressed, one opens to be looked at and played, not kept.
pub(in crate::view) fn mood_page(state: &State, params: &str, width: f32, palette: ComponentPalette) -> Element {
    let Some(lists) = state.mood_lists.get(params) else { return super::loading("mood-lists", palette) };
    let side = (width - 2.0 * MOOD_GUTTER - MOOD_GAP) / 2.0;
    let tiles = lists.iter().enumerate().map(|(index, list)| {
        let body = Element::column(vec![
            cover(state, &list.cover, side, palette).radius(12.0).border(THUMB_RIM, 0.5),
            Text::new(&list.title, palette).font_size(15.0).truncate().render().font_family(FontFamily::SansBold),
            Text::new(&list.author, palette).font_size(13.0).muted().truncate().render(),
        ]).gap(4.0);
        Element::clickable(format!("catalog:{index}"), body).accessible(limn::Access::new(limn::Role::Button).name(format!("{} · {}", list.title, list.author))).flex(1.0)
    }).collect();
    page_scroll("mobile-mood", Element::column(vec![pairs(tiles)]).padding(MOOD_GUTTER))
}

/// The phone's dialogs over `tree`. UIKit owns its sheets when its bridge is ready, `uikit`;
/// the limn ones stay for other platforms or if the bridge is unavailable. The greeting,
/// the profile and moving an account are limn's on every platform: no UIKit surface draws
/// them, and left behind the bridge they would open to nothing.
fn dialogs(state: &State, tree: Element, width: f32, palette: ComponentPalette, uikit: bool) -> Element {
    let tree = welcome_dialog(state, tree, width);
    let tree = super::jam::invite(state, super::profile_dialog(state, super::handoff_dialog(state, tree, width), width), width);
    // limn's on the iPhone too: it stands over everything, as moving an account does.
    let tree = super::crop::dialog(state, tree, width);
    if uikit { return tree; }
    let tree = sheets::render(state, tree, width, palette);
    let tree = share_dialog(state, tree, width, palette);
    let tree = playlist_dialog(state, tree, width, palette);
    let tree = missed_dialog(state, super::import_run_dialog(state, import_dialog(state, tree, width, palette), width, palette), width, palette);
    confirm_dialog(state, tree, width, palette)
}

/// The pages whose name UIKit draws large beneath the bar, as at a navigation stack's root.
pub(crate) fn large_title(state: &State) -> bool {
    state.mobile_ui.native_bars && state.mobile_ui.native_surfaces
        && matches!(state.page, Page::Search | Page::Library | Page::Settings | Page::SettingsPart(_) | Page::Equalizer | Page::Patches)
}

/// Search's whole header: its bar and, natively, the large title over the field.
pub(crate) fn search_header_height(state: &State) -> f32 {
    HEADER_HEIGHT + if state.mobile_ui.native_bars && state.mobile_ui.native_surfaces { MOBILE_LARGE_TITLE } else { 0.0 }
}

/// The page title, shared by the limn header and the native iOS header.
pub(crate) fn title(state: &State) -> Cow<'_, str> {
    let searching = state.page == Page::Search && state.mobile_search;
    match &state.page {
        Page::Library => t!("mobile-tab-library"),
        Page::Browse => t!("mobile-tab-explore"),
        Page::Mood(params) => state.moods.as_ref().and_then(|moods| moods.as_ref().ok()?.iter().find(|mood| mood.params == *params))
            .map_or_else(|| t!("mobile-tab-explore"), |mood| Cow::Borrowed(mood.title.as_str())),
        Page::Settings => t!("common-settings"),
        Page::SettingsPart(part) => settings::part_title(*part),
        Page::Equalizer => t!("common-equalizer"),
        Page::Playing if state.queue_open => t!("mobile-queue"),
        Page::Playing => playing_from(state),
        Page::Lyrics => t!("mobile-lyrics"),
        Page::Search if searching => t!("common-search"),
        Page::Search => greeting(chrono::Timelike::hour(&chrono::Local::now())).into(),
        Page::Downloads => t!("common-downloads"),
        Page::Liked => t!("common-liked-songs"),
        Page::Patches => t!("common-patches"),
        Page::News => t!("desktop-news"),
        Page::Profile => t!("desktop-profile"),
        Page::Replay => t!("common-replay"),
        Page::Recognize => t!("common-recognize-title"),
        Page::Together => t!("common-together"),
        Page::Album(album) if state.mobile_ui.compact_header => Cow::Borrowed(&album.title),
        Page::Artist(artist) if state.mobile_ui.compact_header => Cow::Borrowed(&artist.name),
        Page::Playlist(id) if state.mobile_ui.compact_header => state.collection.playlist(*id).map_or_else(|| t!("common-playlist"), |playlist| Cow::Borrowed(&playlist.name)),
        Page::Mix(mix) if state.mobile_ui.compact_header => Cow::Owned(super::mix_name(mix)),
        _ => t!("mobile-your-music"),
    }
}

/// What the player says it plays from, as BitChord's: the place it was started, or else the
/// song's album.
fn playing_from(state: &State) -> Cow<'_, str> {
    let album = state.current().and_then(|track| track.album.as_deref());
    match state.playing_from.as_deref().or(album) {
        Some(name) => Cow::Owned(t!("mobile-playing-from", name = name).into_owned()),
        None => t!("mobile-now-playing"),
    }
}

fn with_notice(mut content: Element, state: &State, palette: ComponentPalette) -> Element {
    let Some(message) = &state.mobile_ui.notice else { return content; };
    let notice = Element::panel(Element::row(vec![Text::new(message, palette).render().flex(1.0),
        icon("mobile-notice-close", Icon::Close, &t!("mobile-close-notice"), palette)]).gap(8.0).padding(12.0))
        .background(SCHEME.raised);
    let mut notice = Some(notice);
    if prepend_notice(&mut content, &mut notice) { return content; }
    // Loading and error panes have no scroll of their own. Keep even a long
    // notice reachable without adding a second viewport to the normal pages.
    ScrollArea::new(format!("mobile-notice:{}", route_key(state)),
        Element::column(vec![notice.unwrap(), content])).render().flex(1.0)
}

fn prepend_notice(content: &mut Element, notice: &mut Option<Element>) -> bool {
    match &mut content.kind {
        limn::ElementKind::Scroll { child, .. } => {
            let body = std::mem::replace(child.as_mut(), Element::spacer());
            **child = Element::column(vec![notice.take().unwrap(), body]);
            true
        }
        limn::ElementKind::Column(children) => children.iter_mut().any(|child| prepend_notice(child, notice)),
        _ => false,
    }
}

pub(in crate::view) fn horizontal_inset(content: Element, left: f32, right: f32) -> Element {
    Element::row(vec![Element::spacer().size(Some(left), None), content.flex(1.0),
        Element::spacer().size(Some(right), None)])
}

fn mix(from: Color, to: Color, amount: f32) -> Color {
    let channel = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * amount).round() as u8;
    Color::rgb(channel(from.r, to.r), channel(from.g, to.g), channel(from.b, to.b))
}

/// A colour's relative luminance, from 0 for black to 1 for white.
fn luminance(color: Color) -> f32 {
    let linear = |channel: u8| {
        let s = f32::from(channel) / 255.0;
        if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
}

/// Keep even a very bright cover readable with Surco's secondary text token.
fn cover_tone(tint: Color, amount: f32) -> Color {
    let mut color = mix(SCHEME.canvas, tint, amount);
    for _ in 0..16 {
        if (luminance(SCHEME.muted) + 0.05) / (luminance(color) + 0.05) >= 4.5 { break; }
        color = mix(color, SCHEME.canvas, 0.12);
    }
    color
}

/// What the song's pages stand on, as BitChord's player: its cover blurred across the whole
/// screen under a black scrim, darker toward the controls. Until the blur has come they keep
/// the cover's colour.
fn song_ground(state: &State, window: Size) -> Option<Element> {
    if !matches!(state.page, Page::Playing | Page::Lyrics) { return None; }
    let url = state.current()?.artwork_url.as_deref()?;
    let blurred = state.artwork.get(&crate::state::backdrop_key(url))?;
    // The player's is BitChord's own shade, 34 → 48% → 64%, which gives its screens' greys over
    // the same cover. A near-white cover, and the lyrics, keep the deeper one white words stay
    // legible on over any cover.
    let light = state.tints.get(url).is_some_and(|tint| luminance(*tint) > 0.6);
    let shade = if state.page == Page::Playing && !state.queue_open && !light { [87, 122, 163] } else { [120, 160, 205] };
    let scrim = Element::panel(Element::spacer()).vertical_gradient(shade.map(|alpha| Color::rgba(0, 0, 0, alpha)).to_vec());
    let whole = |element: Element| element.size(Some(window.width), Some(window.height));
    Some(Element::stack(vec![whole(Element::raster_image(blurred.clone().cover())), whole(scrim)]))
}

/// The player's cover, edge to edge across the top over its ground, which it dissolves into;
/// a shade over its top keeps the system's bar and the handle legible on a light cover.
fn player_banner(state: &State, window: Size) -> Option<Element> {
    if state.page != Page::Playing || state.queue_open || cover_flight(state, window.height).is_some() || docking(state, window).is_some() { return None; }
    let insets = state.mobile_ui.content_insets;
    let banner = player::banner(state, window.width, player::banner_height(window, insets.bottom))?;
    let shade = Element::panel(Element::spacer()).vertical_gradient(vec![Color::rgba(0, 0, 0, 120), Color::TRANSPARENT]).size(None, Some(insets.top + HEADER_HEIGHT + 32.0));
    let at_top = |element: Element| Element::column(vec![element, Element::spacer().flex(1.0)]).size(Some(window.width), Some(window.height));
    Some(Element::stack(vec![at_top(banner), at_top(shade)]))
}

/// How far the player's cover has gone toward the lyrics' or the queue's head while it is on
/// its way there or back, on a screen `height` tall enough for the head to take it; none at rest.
fn cover_flight(state: &State, height: f32) -> Option<f32> {
    (height >= lyrics::SHORT && state.cover_moving()).then_some(state.mobile_ui.collapse)
}

/// The cover on its way between the player's banner and the lyrics' or the queue's head, as
/// BitChord's: it shrinks from the banner into the head's 54 thumbnail, its corners rounding,
/// and grows back the same way.
fn flying_cover(state: &State, window: Size) -> Option<Element> {
    let at = cover_flight(state, window.height)?;
    let insets = state.mobile_ui.content_insets;
    let (from, to) = (limn::Rect::new(0.0, 0.0, window.width, player::banner_height(window, insets.bottom)), lyrics::thumb(insets));
    let mix = |a: f32, b: f32| a + (b - a) * at;
    let picture = player::picture(state, mix(from.width, to.width), mix(from.height, to.height))?
        .radius(8.0 * at).translate(limn::Point::new(mix(from.x, to.x), mix(from.y, to.y)));
    Some(Element::column(vec![Element::row(vec![picture, Element::spacer().flex(1.0)]), Element::spacer().flex(1.0)])
        .size(Some(window.width), Some(window.height)))
}

/// How far the player has come down toward the mini player, while it is on its way down or up.
fn docking(state: &State, window: Size) -> Option<f32> {
    let offset = state.mobile_ui.player_offset;
    (state.page == Page::Playing && !state.queue_open && offset > 0.5 && state.current().is_some()).then(|| (offset / window.height.max(1.0)).clamp(0.0, 1.0))
}

/// Where the mini player's thumbnail stands over the tabs, on a screen `window` whose safe
/// edges the state knows: where the player's cover goes as the player comes down.
fn mini_thumb(state: &State, window: Size) -> limn::Rect {
    let insets = state.mobile_ui.content_insets;
    let width = (window.width - insets.left - insets.right).max(TAP);
    let span = (width - 2.0 * BAR_SIDE).clamp(TAP, BAR_MAX);
    let pill = window.height - (insets.bottom.max(15.0) + 6.0) - NAV_HEIGHT - MINI_HEIGHT;
    limn::Rect::new(insets.left + (width - span) / 2.0 + 8.0, pill + (MINI_PILL - 40.0) / 2.0, 40.0, 40.0)
}

/// The player's cover leaving with it, as BitChord's: while the player comes down it shrinks
/// from the banner into the mini player's thumbnail, and grows back from it as the player
/// rises, at the pace the player moves, a finger's too.
fn docking_cover(state: &State, window: Size) -> Option<Element> {
    let at = docking(state, window)?;
    let (from, to) = (limn::Rect::new(0.0, 0.0, window.width, player::banner_height(window, state.mobile_ui.content_insets.bottom)), mini_thumb(state, window));
    let mix = |a: f32, b: f32| a + (b - a) * at;
    let picture = player::picture(state, mix(from.width, to.width), mix(from.height, to.height))?
        .radius(8.0 * at).translate(limn::Point::new(mix(from.x, to.x), mix(from.y, to.y)));
    Some(Element::column(vec![Element::row(vec![picture, Element::spacer().flex(1.0)]), Element::spacer().flex(1.0)])
        .size(Some(window.width), Some(window.height)))
}

/// The video's showing, once its first frame is on the screen behind a phone's player.
fn canvas_ready(state: &State) -> Option<String> {
    let (name, ..) = state.screening().filter(|_| state.canvas())?;
    (state.clip_ready.as_ref() == Some(&name)).then_some(name)
}

/// The song's video across the whole screen behind the player, as BitChord's over a Spotify
/// Canvas: from its first frame it fades in over the cover, under a black of 50% and, at its
/// foot, the picture blurred for the controls to stand on. A tap on the bare video puts the
/// controls, the black and the blur away, and another brings them back. A shade at its foot
/// keeps the song's name legible over a light picture meanwhile.
fn player_canvas(state: &State, window: Size) -> Option<Element> {
    // The cover travels alone: the video gives way to it, as BitChord's still frame does.
    if cover_flight(state, window.height).is_some() || docking(state, window).is_some() { return None; }
    let name = canvas_ready(state)?;
    let whole = |element: Element| element.size(Some(window.width), Some(window.height));
    let at_foot = |element: Element, share: f32| whole(Element::column(vec![Element::spacer().flex(1.0), element.size(Some(window.width), Some(window.height * share))]));
    let mut layers = vec![whole(Element::surface(CLIP_SURFACE, t!("desktop-song-video"))).animate_enter(format!("clip:{name}"))];
    let away = state.mobile_ui.controls_away;
    if away {
        let shade = Element::panel(Element::spacer()).vertical_gradient(vec![Color::TRANSPARENT, CANVAS_FOOT]);
        layers.push(at_foot(shade, 0.3).animate_presence("mobile-canvas-foot"));
    } else {
        layers.push(at_foot(Element::surface(CLIP_BACKDROP, ""), crate::clip::DECK).animate_presence("mobile-canvas-deck"));
        layers.push(whole(Element::panel(Element::spacer()).background(CANVAS_DIM)).animate_presence("mobile-canvas-dim"));
    }
    let label = if away { t!("mobile-show-controls") } else { t!("mobile-hide-controls") };
    layers.push(whole(Element::clickable("mobile-canvas", Element::spacer()).accessible(limn::Access::new(limn::Role::Button).name(label))));
    Some(Element::stack(layers))
}

/// BitChord's handle over the player's banner, what plays named under it; pressed, it puts
/// the player away, as dragging it down does.
fn player_handle(title: &str, palette: ComponentPalette) -> Element {
    let grip = Element::panel(Element::spacer()).background(Color::rgba(255, 255, 255, 150)).round().size(Some(36.0), Some(5.0)).centered_x();
    let words = Text::new(title, palette).font_size(13.0).ink(Color::rgba(255, 255, 255, 225)).truncate().render().font_family(FontFamily::SansMedium).centered_x();
    Element::clickable("mobile-player-dismiss", Element::column(vec![grip, words]).gap(6.0).centered())
        .accessible(limn::Access::new(limn::Role::Button).name(t!("common-close")))
        .size(Some(200.0), Some(TAP))
}

fn page_surface(state: &State) -> (Color, Color) {
    let artwork = match &state.page {
        Page::Playing | Page::Lyrics => state.current().and_then(|track| track.artwork_url.as_deref()),
        Page::Album(album) => album.artwork_url.as_deref(),
        Page::Artist(artist) => artist.artwork_url.as_deref(),
        Page::Playlist(id) => state.collection.playlist(*id).and_then(|playlist| playlist.cover()).map(String::as_str),
        _ => None,
    };
    let Some(tint) = artwork.and_then(|url| state.tints.get(url)).copied() else { return (SCHEME.canvas, SCHEME.canvas); };
    // A song list's page is BitChord's: one dark colour of its cover, all the way down.
    if !matches!(state.page, Page::Playing | Page::Lyrics) { let tone = page_tone(tint); return (tone, tone); }
    (cover_tone(tint, 0.60), cover_tone(tint, 0.28))
}

/// The colour a song list's page is painted in, as BitChord's: its cover's, dark enough for
/// the secondary ink.
pub(in crate::view) fn page_tone(tint: Color) -> Color {
    cover_tone(tint, 0.32)
}

pub(crate) fn route_key(state: &State) -> String {
    match &state.page {
        Page::Search => if state.mobile_search { "mobile:search" } else { "mobile:home" }.into(),
        Page::Album(album) => format!("mobile:album:{}", album.id),
        Page::Artist(artist) => format!("mobile:artist:{}", artist.id),
        Page::Playlist(id) => format!("mobile:playlist:{id}"),
        page => format!("mobile:{page:?}"),
    }
}

/// BitChord's tab bar: a frosted pill floating clear of the edges, the chosen tab on a lit
/// platter that slides over to the next one chosen, and Search a circle of its own beside it.
/// While the page goes down it folds into one row: the chosen tab's circle, the mini player
/// and Search, each gliding to its place.
fn navigation(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let span = (width - 2.0 * BAR_SIDE).clamp(TAP, BAR_MAX);
    let disc = |content: Element| Element::panel(content).background(FROST).border(HAIRLINE, 0.5).round().clip_children();
    let circle = |id: &str, glyph: Icon, label: &str, lit: bool| {
        let face = Element::icon(glyph).large_icon(24.0).foreground(if lit { palette.text } else { palette.muted }).centered();
        disc(Element::clickable(id, face).accessible(limn::Access::new(limn::Role::Button).name(label).selected(lit)).round())
            .size(Some(NAV_HEIGHT), Some(NAV_HEIGHT))
    };
    let search = circle("mobile-search", Icon::Search, &t!("common-search"), state.page == Page::Search && state.mobile_search).animate_move("mobile-bar-search");
    let row = if state.mobile_ui.bar_inline {
        let tabs = tabs(state);
        let (id, glyph, _, label, _) = tabs.iter().copied().find(|tab| tab.4).unwrap_or(tabs[0]);
        let mut row = vec![circle(id, glyph, label, true).animate_enter("mobile-bar-folded")];
        row.push(match state.current() {
            Some(track) => disc(mini_controls(state, track, palette)).size(None, Some(NAV_HEIGHT)).flex(1.0).animate_move("mobile-mini"),
            None => Element::spacer().flex(1.0),
        });
        row.push(search);
        row
    } else {
        let tab = |id: &str, glyph: Icon, label: &str, selected: bool| {
            let ink = if selected { palette.text } else { palette.muted };
            let copy = Element::column(vec![
                Element::icon(glyph).large_icon(24.0).foreground(ink).centered_x().animate_foreground(format!("mobile-tab:{id}:icon")),
                Text::new(label, palette).font_size(10.0).render().font_family(limn::FontFamily::SansMedium).foreground(ink).centered_x()
                    .animate_foreground(format!("mobile-tab:{id}:label")),
            ]).gap(3.0).centered();
            let cell = if selected {
                Element::stack(vec![Element::panel(Element::spacer()).background(TAB_LIT).round().animate_move("mobile-tab-platter"), copy])
            } else { copy };
            Element::clickable(id, cell).accessible(limn::Access::new(limn::Role::Button).name(label).selected(selected)).radius((NAV_HEIGHT - 8.0) / 2.0)
                .size(None, Some(NAV_HEIGHT - 8.0)).flex(1.0)
        };
        let tabs = Element::row(tabs(state).into_iter().map(|(id, glyph, _, label, selected)| tab(id, glyph, label, selected)).collect()).padding(4.0);
        vec![disc(tabs).size(None, Some(NAV_HEIGHT)).flex(1.0).animate_enter("mobile-bar-open"), search]
    };
    Element::row(row).gap(8.0).size(Some(span), Some(NAV_HEIGHT)).centered_x().block_pointer("mobile-nav-bar")
}

/// A floating bar's pill: frosted, rimmed with a hairline, centred `BAR_SIDE` clear of the
/// sides and no wider than `BAR_MAX`.
fn floating_bar(content: Element, width: f32, height: f32) -> Element {
    let span = (width - 2.0 * BAR_SIDE).clamp(TAP, BAR_MAX);
    Element::panel(content).background(FROST).border(HAIRLINE, 0.5).round().clip_children()
        .size(Some(span), Some(height)).centered_x()
}

/// Each tab's id, limn glyph, SF Symbol, label and selection, shared with the native tab bar:
/// BitChord's Home, Explore and Library. Search is the circle beside them.
pub(crate) fn tabs(state: &State) -> [(&'static str, Icon, &'static str, &'static str, bool); 3] {
    let home = state.page == Page::Search && !state.mobile_search;
    let library = matches!(state.page, Page::Library | Page::Downloads | Page::Liked | Page::Playlist(_));
    [
        ("home", Icon::Home, if home { "house.fill" } else { "house" }, crate::i18n::fixed(t!("mobile-tab-home")), home),
        ("browse", Icon::Grid, "square.grid.2x2.fill", crate::i18n::fixed(t!("mobile-tab-explore")), state.page == Page::Browse),
        ("mobile-library", Icon::Library, if library { "books.vertical.fill" } else { "books.vertical" }, crate::i18n::fixed(t!("mobile-tab-library")), library),
    ]
}

/// (dispatch id, label, SF Symbol)
#[cfg(any(target_os = "ios", test))]
pub(crate) type HeaderButton = (&'static str, &'static str, &'static str);

/// The native header's (leading, trailing) buttons.
/// Mirrors the limn header in `view` for the pages outside the player.
#[cfg(any(target_os = "ios", test))]
pub(crate) fn header_buttons(state: &State) -> (Vec<HeaderButton>, Vec<HeaderButton>) {
    let detail = matches!(state.page, Page::Album(_) | Page::Artist(_) | Page::Playlist(_));
    let leading = if state.can_go_back() { vec![("back", crate::i18n::fixed(t!("common-back")), "chevron.left")] } else { Vec::new() };
    let mut trailing = Vec::new();
    if state.mobile_ui.compact_header && detail && !state.page_rows().is_empty() { trailing.push(("page-play", crate::i18n::fixed(t!("common-play")), "play.fill")); }
    // A new playlist is made from the library's bar, a glass button beside Settings.
    if state.page == Page::Library { trailing.push(("playlist-new", crate::i18n::fixed(t!("common-create-playlist")), "plus")); }
    if !matches!(state.page, Page::Settings | Page::SettingsPart(_) | Page::Equalizer) && !detail { trailing.push(("settings", crate::i18n::fixed(t!("common-settings")), "gearshape")); }
    (leading, trailing)
}

#[cfg(any(target_os = "ios", test))]
pub(crate) fn surface_tint(state: &State) -> Color { page_surface(state).0 }

/// UIKit draws iOS 26's bars in Liquid Glass, their content its own.
pub(super) fn glass(state: &State) -> bool {
    state.mobile_ui.native_bars && state.mobile_ui.native_surfaces && state.mobile_ui.glass
}

/// Search with its field under the header, which folds away as the field rises; under Liquid
/// Glass the field is at the foot instead and the header stays a plain one.
pub(crate) fn search_in_header(state: &State) -> bool {
    state.page == Page::Search && state.mobile_search && !glass(state)
}

/// UIKit's search of the page's songs, at its top as dnmusic's lists keep it: scrolled away
/// under the header with the page, Cancel empties it.
pub(in crate::view) fn page_search_slot(state: &State) -> Option<Element> {
    // A search that finds nothing keeps its field, to be emptied.
    let songs = !state.page_rows().is_empty() || !state.page_search.is_empty();
    // Under Liquid Glass only: older iOS keeps limn's magnifier.
    (cfg!(any(target_os = "ios", test)) && state.mobile && glass(state) && songs)
        .then(|| Element::spacer().size(None, Some(TAP))
            .accessible(limn::Access::new(limn::Role::Group).id("ios-page-search").name(t!("desktop-search-page"))))
}

/// The mini-player's place under Liquid Glass, where UIKit floats its capsule above the tabs.
fn mini_slot(track: &Track) -> Element {
    let slot = Element::spacer().size(None, Some(MINI_HEIGHT - 10.0))
        .accessible(limn::Access::new(limn::Role::Group).id("ios-mini").name(t!("mobile-open-player", title = track.title.as_str())));
    Element::column(vec![Element::spacer().size(None, Some(4.0)), horizontal_inset(slot, 8.0, 8.0),
        Element::spacer().size(None, Some(6.0))]).size(None, Some(MINI_HEIGHT)).block_pointer("mobile-mini-bar")
}

/// The native mini-player's buttons after the song: id, label and SF Symbol.
#[cfg(any(target_os = "ios", test))]
pub(crate) fn mini_buttons(state: &State) -> [HeaderButton; 3] {
    let liked = state.current().is_some_and(|track| state.is_liked(&track.id));
    let playing = state.player.playing;
    [
        ("mobile-connections", crate::i18n::fixed(t!("mobile-connect-device")), "airplay.audio"),
        ("like", crate::i18n::fixed(if liked { t!("mobile-unlike") } else { t!("common-like") }), if liked { "heart.fill" } else { "heart" }),
        ("play-pause", crate::i18n::fixed(if playing { t!("common-pause") } else { t!("common-play") }), if playing { "pause.fill" } else { "play.fill" }),
    ]
}

/// BitChord's mini-player: a frosted pill floating over the tabs, the song beside its cover,
/// then play and next.
fn mini_player(state: &State, track: &Track, width: f32, palette: ComponentPalette) -> Element {
    Element::column(vec![floating_bar(mini_controls(state, track, palette), width, MINI_PILL).animate_move("mobile-mini"), Element::spacer().size(None, Some(MINI_HEIGHT - MINI_PILL))])
        .size(None, Some(MINI_HEIGHT)).block_pointer("mobile-mini-bar")
}

/// What the mini player holds, over the tabs or folded among them: the song by its cover,
/// which opens the player, play and next.
fn mini_controls(state: &State, track: &Track, palette: ComponentPalette) -> Element {
    let label = Element::row(vec![
        cover(state, &track.artwork_url, 40.0, palette).radius(8.0).border(THUMB_RIM, 1.0).centered_y(),
        Element::column(vec![
            Text::new(&track.title, palette).font_size(15.0).truncate().render().font_family(limn::FontFamily::SansBold),
            Text::new(state.player.error.as_deref().unwrap_or(&track.artist), palette).font_size(13.0).muted().truncate().render(),
        ]).gap(2.0).centered_y().flex(1.0),
    ]).gap(10.0).size(None, Some(TAP));
    let open = Element::clickable("mobile-playing", label)
        .accessible(limn::Access::new(limn::Role::Button).name(t!("mobile-open-player", title = track.title.as_str()))).flex(1.0);
    let playing = state.player.playing;
    Element::row(vec![
        Element::spacer().size(Some(8.0), None),
        open,
        controls::action("play-pause", if playing { Icon::PauseFill } else { Icon::PlayFill }, &if playing { t!("common-pause") } else { t!("common-play") }, 26.0, palette),
        controls::action("mobile-mini-next", Icon::SkipForward, &t!("common-next-song"), 22.0, palette).dispatch_as("next"),
        Element::spacer().size(Some(4.0), None),
    ]).centered_y()
}

/// A song row's cover, as BitChord's: 52 across, its corners 8, rimmed with a hairline.
pub(in crate::view) const THUMB: f32 = 52.0;
/// BitChord's segmented control: a dim track, the chosen kind on a lighter pill.
const SEGMENT_TRACK: Color = Color::rgb(34, 34, 37);
const SEGMENT_ON: Color = Color::rgb(99, 99, 102);
const SEGMENT_HEIGHT: f32 = 36.0;

/// BitChord's segmented control: its `cells`, (id, label, chosen), in one pill track, the one
/// chosen on a lighter pill that slides, under `platter`, to the next one chosen. Each is
/// touched on the track's whole height, and said to be a `role`.
pub(in crate::view) fn segments<'a>(cells: impl Iterator<Item = (&'a str, &'a str, bool)>, role: limn::Role, platter: &str, palette: ComponentPalette) -> Element {
    let cells = cells.map(|(id, label, on)| {
        let copy = Text::new(label, palette).font_size(13.0).truncate().render().font_family(FontFamily::SansBold).centered();
        let pill = |cell: Element| Element::column(vec![Element::spacer().flex(1.0), cell.size(None, Some(SEGMENT_HEIGHT - 6.0)), Element::spacer().flex(1.0)]);
        let cell = if on {
            pill(Element::stack(vec![Element::panel(Element::spacer()).background(SEGMENT_ON).round().animate_move(platter), copy]))
        } else { copy };
        Element::clickable(id, cell).accessible(limn::Access::new(role).name(label).selected(on)).size(None, Some(TAP)).flex(1.0)
    }).collect();
    let track = Element::panel(Element::spacer()).background(SEGMENT_TRACK).round().size(None, Some(SEGMENT_HEIGHT));
    Element::stack(vec![Element::column(vec![Element::spacer().flex(1.0), track, Element::spacer().flex(1.0)]),
        horizontal_inset(Element::row(cells), 3.0, 3.0)]).size(None, Some(TAP))
}
/// The row of the song that plays: a faint platter, faint as limn blends in linear light.
const PLAYING_ROW: Color = Color::rgba(255, 255, 255, 8);

fn thumb(state: &State, artwork: &Option<String>, palette: ComponentPalette) -> Element {
    cover(state, artwork, THUMB, palette).radius(8.0).border(THUMB_RIM, 0.5)
}

/// A song's name over its artist, as BitChord's rows: the name a weight up, the artist in the
/// secondary ink.
fn song_copy(title: &str, subtitle: &str, palette: ComponentPalette) -> Element {
    Element::column(vec![
        Text::new(title, palette).font_size(16.0).truncate().render().font_family(FontFamily::SansMedium),
        Text::new(subtitle, palette).font_size(14.0).muted().truncate().render(),
    ]).gap(2.0).centered_y().flex(1.0)
}

/// Overflow stays visible on touch screens, independently of pointer hover.
pub(super) fn track_row(state: &State, row: String, track: &Track, subtitle: &str, palette: ComponentPalette) -> Element {
    let more = format!("more:{row}");
    let dots = icon(&more, Icon::More, &t!("mobile-options-for", title = track.title.as_str()), palette);
    let copy = song_copy(&track.title, subtitle, palette);
    // The song that plays sounds over its cover, as on a desktop, and its row is lit.
    let playing = state.current().is_some_and(|current| current.id == track.id);
    let picture = thumb(state, &track.artwork_url, palette);
    let picture = if playing {
        let sound = Element::panel(wave(state, format!("wave:{row}"), palette).centered()).background(COVER_SCRIM).radius(8.0).size(Some(THUMB), Some(THUMB));
        Element::stack(vec![picture, sound])
    } else { picture };
    let body = Element::row(vec![picture, copy]).gap(14.0);
    let main = Element::clickable(&row, body).accessible(limn::Access::new(limn::Role::Button).name(format!("{} · {subtitle}", track.title))).flex(1.0);
    let row_element = Element::row(vec![main, dots.centered_y()]).gap(4.0).padding(8.0).size(None, Some(68.0));
    let row_element = if playing { row_element.background(PLAYING_ROW).radius(12.0) } else { row_element };
    ContextMenu::new(format!("mobile-context:{row}:{}", track.id), row_element,
        vec![ContextMenuEntry::Item(ContextMenuItem::new(&more, t!("common-song-options")))], SCHEME.menu())
        .open_action(more).render()
}

/// Songs recommended for a playlist: a tap plays one, its button adds it to the list, and
/// the one below trades them for others.
pub(super) fn suggested(state: &State, shown: &[&Track], palette: ComponentPalette) -> Element {
    let rows = shown.iter().enumerate().map(|(index, track)| {
        let body = Element::row(vec![thumb(state, &track.artwork_url, palette), song_copy(&track.title, &track.artist, palette)]).gap(14.0);
        let main = Element::clickable(format!("suggested:{index}"), body)
            .accessible(limn::Access::new(limn::Role::Button).name(format!("{} · {}", track.title, track.artist))).flex(1.0);
        let add = icon(&format!("suggested-add:{index}"), Icon::Add, &t!("common-recommended-add-title", title = track.title.as_str()), palette);
        Element::row(vec![main, add.centered_y()]).gap(4.0).padding(8.0).size(None, Some(68.0))
    });
    let refresh = Button::new("suggested-refresh", t!("common-refresh"), palette).variant(ButtonVariant::Secondary).round()
        .sizing(limn::ButtonSizing { height: TAP, ..ControlSize::Spacious.button_sizing() }).render();
    Element::column(vec![Element::column(rows.collect()).gap(4.0), refresh.centered_x()]).gap(12.0)
}

/// Outside the phone's shell, which knows its height, the player is as tall as a square cover
/// and its controls.
pub(super) fn playing(state: &State, width: f32, palette: ComponentPalette) -> Element {
    let window = Size::new(width, width + player::CONTROLS);
    let screen = player::screen(state, width, window, 0.0, palette);
    match player::banner(state, width, player::banner_height(window, 0.0)) {
        Some(banner) => Element::stack(vec![Element::column(vec![banner, Element::spacer().flex(1.0)]), screen]).size(None, Some(window.height)),
        None => screen,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Command;
    use crate::player::PlayerView;
    use limn::{UiAction, UiTree};
    use surco_core::ArtistRef;

    fn playing_state() -> State {
        let track = Track { id: "song".into(), title: "Una canción con un título largo para el teléfono".into(), artist: "Un artista".into(), artist_id: None, album: None, album_id: None, artwork_url: None, duration_ms: 180_000 };
        let mut state = State::default();
        state.mobile = true;
        state.show(PlayerView { current: Some(track), playing: true, position: 30.0, duration: 180.0, ..PlayerView::default() });
        state
    }

    #[test]
    fn phone_navigation_and_transport_fit_without_desktop_panels() {
        for size in [Size::new(390.0, 844.0), Size::new(430.0, 932.0)] {
            let mut state = playing_state();
            for page in [Page::Search, Page::Library, Page::Settings, Page::Playing] {
                state.page = page;
                let mut tree = UiTree::new(super::view(&state, size));
                let frame = tree.layout(size);
                assert_eq!(frame.access.iter().filter(|node| node.id == "settings").count(), usize::from(!matches!(state.page, Page::Settings | Page::Playing)));
                let ids = if state.page == Page::Playing { vec!["play-pause", "next", "previous", "queue", "mobile-connections"] } else { vec!["home", "mobile-search", "mobile-library", "play-pause", "mobile-mini-next"] };
                for id in ids {
                    let nodes: Vec<_> = frame.access.iter().filter(|node| node.id == id).collect();
                    assert_eq!(nodes.len(), 1, "one {id} on {:?} at {size:?}", state.page);
                    // Access nodes deduplicate IDs; hit regions also catch two live
                    // buttons sharing the same retained color/focus state.
                    assert_eq!(frame.regions.iter().filter(|region| region.id == id && region.kind == limn::HitKind::Button).count(), 1,
                        "one hit target for {id} on {:?}", state.page);
                    let rect = nodes[0].rect;
                    assert!(rect.x >= 0.0 && rect.y >= 0.0 && rect.x + rect.width <= size.width + 0.01 && rect.y + rect.height <= size.height + 0.01, "{id}: {rect:?}");
                    assert!(rect.width >= TAP && rect.height >= TAP, "{id} is touch sized: {rect:?}");
                }
                assert!(!frame.regions.iter().any(|region| region.id.starts_with("resize:")));
            }
        }
    }

    #[test]
    fn folded_the_bar_is_one_row_of_the_chosen_tab_the_mini_player_and_search() {
        let mut state = playing_state();
        state.page = Page::Library;
        state.mobile_ui.bar_inline = true;
        let size = Size::new(390.0, 844.0);
        let mut tree = UiTree::new(super::view(&state, size));
        tree.set_reduced_motion(true);
        let frame = tree.layout(size);
        let rect = |id: &str| frame.access.iter().find(|node| node.id == id).unwrap_or_else(|| panic!("{id}")).rect;
        let (tab, mini, search) = (rect("mobile-library"), rect("mobile-playing"), rect("mobile-search"));
        assert!(!frame.access.iter().any(|node| node.id == "home"), "the other tabs fold away");
        assert!(tab.x + tab.width <= mini.x && mini.x + mini.width <= search.x, "{tab:?} {mini:?} {search:?}");
        let middle = |rect: limn::Rect| rect.y + rect.height / 2.0;
        assert!((middle(tab) - middle(search)).abs() < 1.0 && (middle(mini) - middle(search)).abs() < 1.0, "one row");
    }

    #[test]
    fn navigation_labels_are_visible_and_tabs_do_not_overlap() {
        let mut state = playing_state();
        state.player.current = None;
        let mut tree = UiTree::new(super::view(&state, Size::new(390.0, 844.0)));
        let frame = tree.layout(Size::new(390.0, 844.0));
        for label in ["Inicio", "Explorar", "Biblioteca"] {
            let positions: Vec<_> = frame.commands.iter().filter_map(|command| match command {
                limn::DrawCommand::Text { text, position, .. } if text == label => Some(position),
                _ => None,
            }).collect();
            assert_eq!(positions.len(), 1, "{label}: {positions:?}");
        }
        let tabs: Vec<_> = ["home", "browse", "mobile-library", "mobile-search"].iter().map(|id| frame.access.iter().find(|node| node.id == *id).unwrap().rect).collect();
        for pair in tabs.windows(2) { assert!(pair[0].x + pair[0].width <= pair[1].x + 0.1, "{tabs:?}"); }
    }

    #[test]
    fn library_import_card_actions_have_phone_sized_touch_targets() {
        let mut state = playing_state();
        state.page = Page::Library;
        for width in [320.0, 390.0, 430.0] {
            let size = Size::new(width, 844.0);
            let mut tree = UiTree::new(super::view(&state, size));
            tree.set_reduced_motion(true);
            let frame = tree.layout(size);
            for id in ["import-open", "import-dismiss"] {
                let rect = frame.access.iter().find(|node| node.id == id).unwrap().rect;
                assert!(rect.width >= TAP && rect.height >= TAP, "{id}: {rect:?}");
                assert!(rect.x >= 0.0 && rect.x + rect.width <= width && rect.y + rect.height <= size.height - NAV_HEIGHT - MINI_HEIGHT);
            }
        }
    }

    #[test]
    fn behind_uikit_moving_an_account_and_the_profile_still_open() {
        let size = Size::new(390.0, 844.0);
        let mut state = playing_state();
        let ids = |state: &State| {
            let tree = dialogs(state, Element::spacer().size(Some(size.width), Some(size.height)), size.width, SCHEME.palette(), true);
            UiTree::new(tree).layout(size).access.iter().map(|node| node.id.clone()).collect::<Vec<_>>()
        };
        state.update(&UiAction::Clicked { id: "account-handoff".into() });
        let offer = crate::handoff::Offer { code: "0000-0000-0001".into(), link: "surco://receive?from=x".into() };
        state.handoff.as_mut().unwrap().offer = Some(Ok((offer, None)));
        assert!(ids(&state).iter().any(|id| id == "handoff-copy"), "its code, over what UIKit draws");
        state.handoff = None;
        state.update(&UiAction::Clicked { id: "profile-edit".into() });
        assert!(ids(&state).iter().any(|id| id == "profile-name"), "the profile's name");
    }

    #[test]
    fn library_import_card_reserves_a_native_paste_control_without_replacing_import() {
        let mut state = playing_state();
        state.page = Page::Library;
        state.mobile_ui.native_surfaces = true;
        for width in [320.0, 390.0, 430.0] {
            let size = Size::new(width, 844.0);
            let mut tree = UiTree::new(super::view(&state, size));
            tree.set_reduced_motion(true);
            let frame = tree.layout(size);
            let paste = frame.access.iter().find(|node| node.id == "ios-paste:import-card").unwrap();
            let import = frame.access.iter().find(|node| node.id == "import-open").unwrap();
            assert_eq!(paste.role, limn::Role::Group);
            assert!(paste.rect.width >= 112.0 && paste.rect.height >= TAP);
            assert!(paste.rect.x >= import.rect.x + import.rect.width);
            assert!(paste.rect.x + paste.rect.width <= width);
            assert!(paste.rect.y + paste.rect.height <= size.height - NAV_HEIGHT - MINI_HEIGHT);
            let mut native = crate::native_surfaces::Controller::default();
            assert_eq!(native.payload(&state, &frame, eframe::egui::Pos2::ZERO).paste.map(|slot| slot.target).as_deref(),
                Some("import-card"), "UIKit is handed the slot at {width}");
        }
    }

    #[test]
    fn editing_search_reserves_the_keyboard_without_changing_playback() {
        let mut state = playing_state();
        state.mobile_search = true;
        state.mobile_ui.search_editing = true;
        state.page = Page::Search;
        for size in [Size::new(320.0, 360.0), Size::new(390.0, 500.0), Size::new(430.0, 500.0)] {
            let mut tree = UiTree::new(super::view(&state, size));
            let frame = tree.layout(size);
            assert!(!frame.access.iter().any(|node| matches!(node.id.as_str(), "home" | "mobile-library" | "play-pause")));
            for id in ["search", "mobile-search-cancel"] {
                let rect = frame.access.iter().find(|node| node.id == id).unwrap().rect;
                assert!(rect.x >= 0.0 && rect.y >= 0.0 && rect.x + rect.width <= size.width + 0.1
                    && rect.y + rect.height <= size.height + 0.1 && rect.height >= TAP, "{id}: {rect:?}");
            }
            assert!(tree.focus("search"));
            let frame = tree.layout(size);
            assert!(crate::mobile_input::can_paste(&tree, &frame, "search"));
        }
        assert!(state.player.playing);
        assert_eq!(state.current().unwrap().id, "song");
    }

    #[test]
    fn device_sheet_has_a_real_native_slot_and_closes_without_playing_a_row() {
        let mut state = playing_state();
        state.update(&UiAction::Clicked { id: "mobile-connections".into() });
        state.mobile_ui.route = Some(("Auriculares".into(), "BluetoothA2DP".into()));
        for width in [320.0, 390.0, 430.0] {
            let size = Size::new(width, 720.0);
            let mut tree = UiTree::new(super::view(&state, size));
            tree.set_reduced_motion(true);
            let frame = tree.layout(size);
            let slot = frame.access.iter().find(|node| node.id == "ios-route-picker").unwrap().rect;
            assert!(slot.width >= TAP && slot.height >= TAP && slot.x + slot.width <= width);
            assert!(frame.commands.iter().any(|command| matches!(command, limn::DrawCommand::Text { text, .. } if text == "Auriculares")));
        }
        assert!(state.update(&UiAction::Clicked { id: "mobile-connections-close".into() }).is_empty());
        assert!(!state.mobile_ui.connections && state.player.playing);
    }

    #[test]
    fn home_track_tiles_keep_the_cover_clear_and_hold_the_correct_song() {
        let mut state = playing_state();
        let track = state.current().unwrap().clone();
        state.collection.heard.insert(0, track.clone());
        let tile = tile(&state, "heard:0".into(), &track.title, Some(track.artist.clone()), &None, false, SCHEME.palette());
        let mut tree = UiTree::new(tile);
        tree.set_reduced_motion(true);
        let frame = tree.layout(Size::new(TILE_WIDTH, 240.0));
        assert!(!frame.access.iter().any(|node| node.id.starts_with("more:")),
            "the artwork has no permanent overflow button");
        let point = limn::Point::new(30.0, 60.0);
        assert_eq!(frame.context_menu_at(point), Some("tile-context:heard:0:song"));
        let actions = tree.handle_event(limn::UiEvent::ContextMenu { position: point });
        assert_eq!(actions, vec![UiAction::Clicked { id: "more:heard:0".into() }]);
        state.update(&actions[0]);
        assert_eq!(state.mobile_ui.context.as_ref().unwrap().track.id, track.id);
    }

    #[test]
    fn home_album_and_artist_cards_offer_hold_actions_in_quick_access_and_shelves() {
        let mut state = playing_state();
        state.page = Page::Search;
        state.collection.recent = vec![
            Entry::Album(AlbumRef { id: "album".into(), title: "Un álbum".into(), artist: Some("Una banda".into()), year: String::new(), artwork_url: None }),
            Entry::Artist(ArtistRef { id: "artist".into(), name: "Una artista".into(), subscribers: None, artwork_url: None }),
        ];
        state.collection.kept = state.collection.recent.clone();
        for index in 0..2 {
            let row = format!("recent:{index}");
            let size = Size::new(390.0, 844.0);
            let mut tree = UiTree::new(super::view(&state, size));
            tree.set_reduced_motion(true);
            let frame = tree.layout(size);
            let rect = frame.access.iter().find(|node| node.id == row).unwrap().rect;
            let actions = tree.handle_event(limn::UiEvent::ContextMenu {
                position: limn::Point::new(rect.x + 20.0, rect.y + 20.0),
            });
            assert_eq!(actions, vec![UiAction::Clicked { id: format!("entry-more:{row}") }]);
            let entry = &state.collection.kept[index];
            let row = format!("home-kept:{index}");
            let mut tree = UiTree::new(tile(&state, row.clone(), entry.title(), None, entry.artwork(), false, SCHEME.palette()));
            tree.set_reduced_motion(true);
            tree.layout(Size::new(TILE_WIDTH, 240.0));
            assert_eq!(tree.handle_event(limn::UiEvent::ContextMenu { position: limn::Point::new(20.0, 60.0) }),
                vec![UiAction::Clicked { id: format!("entry-more:{row}") }]);
        }
    }

    #[test]
    fn phone_home_shelves_start_without_a_permanent_scroll_indicator() {
        let mut state = playing_state();
        state.collection.heard = (0..6).map(|index| Track { id: format!("song-{index}"), ..state.current().unwrap().clone() }).collect();
        let size = Size::new(390.0, 844.0);
        let mut tree = UiTree::new(super::view(&state, size));
        tree.set_reduced_motion(true);
        let frame = tree.layout(size);
        assert!(frame.regions.iter().any(|region| region.id == "home-heard" && region.kind == limn::HitKind::Scroll));
        assert!(!frame.commands.iter().any(|command| matches!(command,
            limn::DrawCommand::FillRect { rect, radius, .. } if rect.height == 3.0 && *radius == 1.5)),
            "the horizontal indicator stays hidden until interaction");
    }

    #[test]
    fn phone_pages_use_the_existing_search_and_playback_commands() {
        let mut state = playing_state();
        let click = |state: &mut State, id: &str| state.update(&UiAction::Clicked { id: id.into() });
        click(&mut state, "mobile-search");
        assert!(state.mobile_search && state.page == Page::Search && state.focus.is_none());
        click(&mut state, "mobile-search-edit");
        assert_eq!(state.focus.as_deref(), Some("search"));
        state.update(&UiAction::InputChanged { id: "search".into(), value: "De música ligera".into() });
        assert_eq!(click(&mut state, "search-submit"), vec![Command::Search("De música ligera".into())]);
        click(&mut state, "mobile-library");
        assert_eq!(state.page, Page::Library);
        click(&mut state, "mobile-playing");
        assert_eq!(state.page, Page::Playing);
        assert_eq!(click(&mut state, "play-pause"), vec![Command::PlayPause]);
        assert_eq!(click(&mut state, "next"), vec![Command::Next]);
        click(&mut state, "back");
        assert_eq!(state.page, Page::Library);
        assert_eq!(state.current().unwrap().id, "song", "navigation keeps the same player");
        click(&mut state, "home");
        assert!(state.page == Page::Search && !state.mobile_search);
    }

    #[test]
    fn details_scroll_beneath_fixed_chrome_and_reach_the_last_song() {
        for width in [320.0, 390.0, 430.0] {
            let size = Size::new(width, 600.0);
            let mut state = playing_state();
            state.collection.playlists.push(Playlist { id: 7, name: "Mis viajes".into(), collaborative: false, picture: None,
                tracks: (0..20).map(|index| Track { id: format!("song-{index}"), ..state.current().unwrap().clone() }).collect() });
            state.page = Page::Playlist(7);
            let mut tree = UiTree::new(super::view(&state, size));
            tree.set_reduced_motion(true);
            let first = tree.layout(size);
            let scroll = first.regions.iter().find(|region| region.id == "mobile-playlist" && region.kind == limn::HitKind::Scroll).unwrap().rect;
            assert_eq!(scroll.y, 0.0, "the page continues behind the header");
            assert_eq!(scroll.height, size.height);
            let nav = first.access.iter().find(|node| node.id == "mobile-library").unwrap().rect;
            tree.scroll_to_end("mobile-playlist");
            let last = tree.layout(size);
            let song = last.access.iter().find(|node| node.id == "page-track:19").unwrap().rect;
            assert!(song.height >= TAP && song.y + song.height <= nav.y - MINI_HEIGHT + 0.1, "last song stays above transport: {song:?}");
            let title = limn::Point::new(width / 2.0, 36.0);
            assert_eq!(last.press_region(title).unwrap().id, "mobile-header-bar");
            tree.handle_event(limn::UiEvent::PointerDown(title));
            assert!(tree.handle_event(limn::UiEvent::PointerUp(title)).is_empty(), "a bar never plays the song below it");
            // Between the mini-player's pill and the tab bar's, which floats 4 above its tabs.
            let margin = limn::Point::new(4.0, nav.y - 8.0);
            assert_eq!(last.press_region(margin).unwrap().id, "mobile-mini-bar");
            tree.handle_event(limn::UiEvent::PointerDown(margin));
            assert!(tree.handle_event(limn::UiEvent::PointerUp(margin)).is_empty());
            // Scrolled, the bar names the list between the way back and its search, as
            // BitChord's: the search opens from there.
            state.mobile_ui.compact_header = true;
            tree.set_root(super::view(&state, size));
            let compact = tree.layout(size);
            let id = "page-search";
            assert_eq!(compact.regions.iter().filter(|region| region.id == id && region.kind == limn::HitKind::Button).count(), 1);
            let button = compact.access.iter().find(|node| node.id == id).unwrap().rect;
            let point = limn::Point::new(button.x + 20.0, button.y + 20.0);
            tree.handle_event(limn::UiEvent::PointerDown(point));
            assert_eq!(tree.handle_event(limn::UiEvent::PointerUp(point)), vec![UiAction::Clicked { id: "page-search".into() }]);
        }
    }

    #[test]
    fn player_translation_moves_header_and_hits_together_without_duplicate_controls() {
        let mut state = playing_state();
        state.page = Page::Playing;
        let size = Size::new(390.0, 844.0);
        let mut tree = UiTree::new(super::view(&state, size));
        tree.set_reduced_motion(true);
        let first = tree.layout(size);
        let original = first.access.iter().find(|node| node.id == "mobile-player-dismiss").unwrap().rect;
        state.mobile_ui.player_offset = 100.0;
        tree.set_root(super::view(&state, size));
        let moved = tree.layout(size);
        let close = moved.access.iter().find(|node| node.id == "mobile-player-dismiss").unwrap().rect;
        assert_eq!(close.y, original.y + 100.0);
        assert_ne!(moved.hit_test(limn::Point::new(original.x + 20.0, original.y + 20.0)), Some("mobile-player-dismiss"));
        assert_eq!(moved.hit_test(limn::Point::new(close.x + 20.0, close.y + 20.0)), Some("mobile-player-dismiss"));
        for id in ["play-pause", "like"] {
            assert_eq!(moved.regions.iter().filter(|region| region.id == id && region.kind == limn::HitKind::Button).count(), 1);
        }
        assert!(state.player.playing);
    }
}
