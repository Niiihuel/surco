//! The television's layout, as BitChord's TV (docs/bitchord-look.md): a capsule of tabs at the
//! top, shelves of lockups that lift under the remote's focus, a page for what was chosen, and
//! the player over the whole screen. The remote's arrows move the focus from control to
//! control (`UiTree::set_spatial_focus`); what has it is lifted rather than outlined.

use super::*;
use super::mobile::horizontal_inset;

pub(crate) mod settings;

/// Clear of the screen's overscan, as BitChord's TV keeps: 38 at the sides, 22 above and below.
const SIDE: f32 = 38.0;
const TOP: f32 = 22.0;
/// A lockup's cover, its corners, an artist's round face, and the room between two.
const LOCKUP: f32 = 131.0;
const LOCKUP_RADIUS: f32 = 10.0;
const ARTIST: f32 = 120.0;
const CARD_GAP: f32 = 16.0;
/// A television's pages scroll by the focus, with no bar to show for it, as BitChord's.
const UNSEEN_BAR: limn::ScrollbarVisibility = limn::ScrollbarVisibility::AutoHide { hold_ms: 0, fade_ms: 0 };
/// How far what has the focus grows: a lockup, a row, a button.
const LIFT: f32 = 1.08;
const ROW_LIFT: f32 = 1.025;
const BUTTON_LIFT: f32 = 1.1;
const LIFT_SHADOW: Color = Color::rgba(0, 0, 0, 170);
/// BitChord's canvas: a lifted grey at the top to near black, the cover's colour over its top.
const CANVAS: [Color; 2] = [Color::rgb(38, 39, 44), Color::rgb(16, 16, 18)];
/// The tab capsule's glass, white at 8%; what has the focus is a white platter, its words dark.
const CAPSULE: Color = Color::rgba(255, 255, 255, 20);
const PLATTER: Color = Color::rgb(255, 255, 255);
const PLATTER_INK: Color = Color::rgb(12, 12, 14);
/// The tab that is shown, when the focus is elsewhere.
const SHOWN: Color = Color::rgba(255, 255, 255, 40);
const TAB: f32 = 36.0;
/// The tabs' strip, with its room above and below.
const BAR: f32 = TOP + TAB + 8.0 + 16.0;
/// How much room a lifted lockup needs around it not to be cut by its shelf.
const LIFT_ROOM: f32 = 12.0;

fn lit(state: &State, id: &str) -> bool {
    state.tv_focused.as_deref() == Some(id)
}

pub(super) fn view(state: &State, window: Size) -> Element {
    let palette = SCHEME.palette();
    let page = match &state.page {
        Page::Playing | Page::Lyrics => return player(state, window, palette),
        Page::Album(album) => songs_page(state, &album.title, &album.artist, &album.artwork_url, true, palette),
        Page::Artist(artist) => songs_page(state, &artist.name, &t!("common-artist"), &artist.artwork_url, false, palette),
        Page::Playlist(id) => match state.collection.playlist(*id) {
            Some(playlist) => songs_page(state, &playlist.name, &t!("common-playlist"), &playlist.cover().cloned(), false, palette),
            None => home(state, palette),
        },
        Page::Mix(mix) => songs_page(state, &mix_name(mix), &mix_detail(mix), &mix.covers().first().map(|url| (*url).clone()), false, palette),
        Page::Library => library(state, palette),
        Page::Search if state.mobile_search => search(state, palette),
        Page::Search => home(state, palette),
        Page::Settings => settings::page(state, window, palette),
        // The profile and the rest are the app's own pages, their controls
        // reached by the remote as any others.
        _ => Element::column(vec![Element::spacer().size(None, Some(BAR)), horizontal_inset(page(state, window.width - 2.0 * SIDE, palette), SIDE, SIDE).flex(1.0)]),
    };
    // The tabs float over the page, which scrolls under them, and step aside once it has.
    let tabs = bar(state, palette).animate_slide("tv-bar", if state.tv_scrolled { -BAR } else { 0.0 });
    let body = Element::stack(vec![page.size(Some(window.width), Some(window.height)), Element::column(vec![tabs, Element::spacer().flex(1.0)])]);
    // A song list's cover washes the canvas's top in its colour, as what plays does elsewhere.
    let art = match &state.page {
        Page::Album(album) => album.artwork_url.clone(),
        Page::Artist(artist) => artist.artwork_url.clone(),
        Page::Playlist(id) => state.collection.playlist(*id).and_then(|playlist| playlist.cover().cloned()),
        Page::Mix(mix) => mix.covers().first().map(|url| (*url).clone()),
        _ => state.current().and_then(|track| track.artwork_url.clone()),
    };
    let wash = art.and_then(|url| state.tints.get(&url)).map(|tint| {
        Element::panel(Element::spacer()).vertical_gradient(vec![Color { a: 110, ..*tint }, Color { a: 0, ..*tint }]).size(None, Some(window.height * 0.6))
    });
    let mut layers = vec![Element::panel(Element::spacer()).vertical_gradient(CANVAS.to_vec())];
    layers.extend(wash.map(|wash| Element::column(vec![wash, Element::spacer().flex(1.0)])));
    layers.push(body);
    Element::stack(layers).background(CANVAS[1]).size(Some(window.width), Some(window.height))
}

/// The tabs, as BitChord's TV: the name at the left, a capsule of pills and round buttons in
/// the middle, the account's face at the right.
fn bar(state: &State, palette: ComponentPalette) -> Element {
    let home = state.page == Page::Search && !state.mobile_search;
    let library = matches!(state.page, Page::Library | Page::Playlist(_) | Page::Liked | Page::Downloads);
    let mut items = vec![
        pill(state, "home", &t!("mobile-tab-home"), home, palette),
        pill(state, "mobile-library", &t!("common-library"), library, palette),
    ];
    if state.current().is_some() {
        items.push(pill(state, "mobile-playing", &t!("mobile-now-playing"), false, palette));
    }
    items.push(round(state, "mobile-search", Icon::Search, &t!("common-search"), state.mobile_search, palette));
    items.push(round(state, "settings", Icon::Settings, &t!("common-settings"), matches!(state.page, Page::Settings | Page::SettingsPart(_)), palette));
    let capsule = Element::panel(Element::row(items).gap(4.0).padding(4.0)).background(CAPSULE).round();
    let name = Text::new("Surco", palette).font_size(21.0).render().font_family(FontFamily::SansBold).centered_y();
    let face = Element::clickable("account-profile", super::profile_face(state, 32.0, palette)).focus_ring(false)
        .accessible(limn::Access::new(limn::Role::Button).name(t!("desktop-profile")))
        .animate_scale_about("tv-lift:account-profile", if lit(state, "account-profile") { 1.15 } else { 1.0 }, limn::Point::new(0.5, 0.5)).centered_y();
    let row = Element::row(vec![name, Element::spacer().flex(1.0), capsule, Element::spacer().flex(1.0), face]);
    Element::column(vec![Element::spacer().size(None, Some(TOP)), horizontal_inset(row, SIDE, SIDE)]).size(None, Some(BAR))
}

/// A tab's pill: white with dark words while the focus is on it, faintly lit while its page
/// is shown.
fn pill(state: &State, id: &str, label: &str, shown: bool, palette: ComponentPalette) -> Element {
    let on = lit(state, id);
    let ink = if on { PLATTER_INK } else if shown { palette.text } else { palette.muted };
    let copy = Text::new(label, palette).font_size(15.0).render().font_family(FontFamily::SansMedium).foreground(ink).centered();
    let fill = if on { PLATTER } else if shown { SHOWN } else { Color::TRANSPARENT };
    let pill = Element::panel(horizontal_inset(copy, 16.0, 16.0)).background(fill).round().size(None, Some(TAB));
    Element::clickable(id, pill).accessible(limn::Access::new(limn::Role::Tab).name(label).selected(shown)).focus_ring(false).round()
}

/// A round button of the capsule, lit as a pill is.
fn round(state: &State, id: &str, glyph: Icon, label: &str, shown: bool, palette: ComponentPalette) -> Element {
    let on = lit(state, id);
    let ink = if on { PLATTER_INK } else if shown { palette.text } else { palette.muted };
    let fill = if on { PLATTER } else if shown { SHOWN } else { Color::TRANSPARENT };
    let disc = Element::panel(Element::icon(glyph).large_icon(18.0).foreground(ink).centered()).background(fill).round().size(Some(TAB), Some(TAB));
    Element::clickable(id, disc).accessible(limn::Access::new(limn::Role::Button).name(label)).focus_ring(false).round()
}

/// A lockup: a cover over its name and what it is, lifted and shadowed while the focus is on it.
fn lockup(state: &State, id: String, title: &str, about: &str, artwork: &Option<String>, round: bool, palette: ComponentPalette) -> Element {
    lockup_of(state, id, title, about, artwork, round, if round { ARTIST } else { LOCKUP }, palette)
}

/// A lockup `side` across.
#[allow(clippy::too_many_arguments)]
fn lockup_of(state: &State, id: String, title: &str, about: &str, artwork: &Option<String>, round: bool, side: f32, palette: ComponentPalette) -> Element {
    let on = lit(state, &id);
    let picture = cover(state, artwork, side, palette).radius(if round { side / 2.0 } else { LOCKUP_RADIUS }).border(mobile::THUMB_RIM, 0.5);
    let picture = if on { picture.shadow(LIFT_SHADOW, limn::Point::new(0.0, 10.0), 28.0, 0.0) } else { picture };
    let line = |text: &str, size: f32, muted: bool| {
        let text = Text::new(text, palette).font_size(size).truncate().render();
        let text = if muted { text.foreground(palette.muted) } else { text.font_family(FontFamily::SansMedium) };
        if round { text.center_content() } else { text }
    };
    let copy = Element::column(vec![line(title, 14.0, false), line(about, 12.0, true)]).gap(2.0).size(Some(side), None);
    let body = Element::column(vec![picture, copy]).gap(8.0);
    Element::clickable(&id, body).accessible(limn::Access::new(limn::Role::Button).name(format!("{title} · {about}"))).focus_ring(false)
        .animate_scale_about(format!("tv-lift:{id}"), if on { LIFT } else { 1.0 }, limn::Point::new(0.5, 0.4)).size(Some(side), None)
}

/// A shelf: its name over its lockups, which scroll sideways as the focus moves along them.
fn shelf(id: &str, title: &str, cards: Vec<Element>, palette: ComponentPalette) -> Option<Element> {
    if cards.is_empty() { return None; }
    let heading = Text::new(title, palette).font_size(18.0).render().font_family(FontFamily::SansBold);
    // Room around the lockups, so one lifted is not cut by the shelf's edges.
    let strip = Element::horizontal_scroll(id, Element::row(cards).gap(CARD_GAP).padding(LIFT_ROOM));
    Some(Element::column(vec![horizontal_inset(heading, SIDE, SIDE), horizontal_inset(strip, SIDE - LIFT_ROOM, SIDE - LIFT_ROOM)]).gap(4.0))
}

/// The page's shelves, one under the other, scrolling up as the focus goes down them.
fn shelves(id: &str, shelves: Vec<Option<Element>>) -> Element {
    let room = Element::spacer().size(None, Some(BAR - 18.0));
    ScrollArea::new(id, Element::column(std::iter::once(room).chain(shelves.into_iter().flatten()).collect()).gap(18.0).padding(4.0)).visibility(UNSEEN_BAR).render().flex(1.0)
}

/// The scroll area of the page shown, whose scrolling sends the tabs aside; None for a page
/// that stays put under them.
pub(crate) fn scroller(state: &State) -> Option<&'static str> {
    match state.page {
        Page::Search if state.mobile_search => Some("tv-search"),
        Page::Search => Some("tv-home"),
        Page::Album(_) | Page::Artist(_) | Page::Playlist(_) | Page::Mix(_) => Some("tv-page-songs"),
        Page::Settings => Some("tv-settings"),
        _ => None,
    }
}

fn home(state: &State, palette: ComponentPalette) -> Element {
    let track = |prefix: &str, tracks: &[Track]| tracks.iter().take(SHELF).enumerate()
        .map(|(index, track)| lockup(state, format!("{prefix}:{index}"), &track.title, &track.artist, &track.artwork_url, false, palette)).collect::<Vec<_>>();
    let mixes = state.mixes().iter().enumerate()
        .map(|(index, mix)| lockup(state, format!("made:{index}"), &mix_name(mix), &mix_detail(mix), &mix.covers().first().map(|url| (*url).clone()), false, palette)).collect();
    let artists = state.picked_artists.iter().enumerate()
        .map(|(index, artist)| lockup(state, format!("pick-artist:{index}"), &artist.name, &t!("common-artist"), &artist.artwork_url, true, palette)).collect();
    let albums = albums(state, "pick-album", &state.picked_albums, palette);
    shelves("tv-home", vec![
        shelf("tv-mixes", &t!("desktop-your-mixes"), mixes, palette),
        shelf("tv-today", &t!("desktop-recommended-today"), track("today", &state.recent_picks), palette),
        shelf("tv-heard", &t!("desktop-listen-again"), track("heard", &state.collection.heard), palette),
        shelf("tv-artists", &t!("desktop-artists-to-discover"), artists, palette),
        shelf("tv-albums", &t!("desktop-albums-for-you"), albums, palette),
        shelf("tv-liked", &t!("common-liked-songs"), track("liked-track", &state.collection.liked), palette),
    ])
}

fn albums(state: &State, prefix: &str, albums: &[AlbumRef], palette: ComponentPalette) -> Vec<Element> {
    albums.iter().enumerate().map(|(index, album)| {
        let about = album.artist.clone().unwrap_or_else(|| album.year.clone());
        lockup(state, format!("{prefix}:{index}"), &album.title, &about, &album.artwork_url, false, palette)
    }).collect()
}

/// The library, as BitChord's TV: its categories down the left, and what the focused one
/// holds on the right, which the focus going down them previews.
fn library(state: &State, palette: ComponentPalette) -> Element {
    let shown = state.tv_library_shown();
    let mut side: Vec<Element> = state.tv_library_categories().into_iter().map(|(key, title)| {
        let id = format!("{}{key}", crate::state::tv::LIBRARY);
        list_row(state, &id, &title, None, false, if key == shown { Color::rgba(255, 255, 255, 31) } else { Color::TRANSPARENT }, palette)
    }).collect();
    side.push(list_row(state, "downloads", &t!("common-downloads"), None, true, Color::TRANSPARENT, palette));
    let side = ScrollArea::new("tv-library-side", horizontal_inset(Element::column(side).gap(3.0).padding(5.0), SIDE - 15.0, 1.0))
        .visibility(UNSEEN_BAR).render().size(Some(240.0), None);
    let list = |prefix: &str, tracks: &[Track]| -> Element {
        if tracks.is_empty() { return empty(&t!("desktop-playlist-empty"), palette); }
        Element::column(tracks.iter().enumerate().map(|(index, track)| row(state, format!("{prefix}:{index}"), track, None, palette)).collect()).gap(2.0)
    };
    let grid = |cards: Vec<Element>| -> Element {
        if cards.is_empty() { return empty(&t!("desktop-home-empty"), palette); }
        Element::row(cards).wrap().gap(CARD_GAP).line_gap(18.0).padding(LIFT_ROOM)
    };
    let pane = match shown {
        "liked" => list("liked-track", &state.collection.liked),
        "playlists" => grid(state.collection.playlists.iter().enumerate().map(|(index, playlist)| {
            lockup_of(state, format!("playlist:{index}"), &playlist.name, &songs(playlist.tracks.len()), &playlist.cover().cloned(), false, GRID, palette)
        }).collect()),
        "albums" => grid(state.collection.albums().map(|(index, album)| {
            lockup_of(state, format!("kept:{index}"), &album.title, album.artist.as_deref().unwrap_or(""), &album.artwork_url, false, GRID, palette)
        }).collect()),
        "artists" => grid(state.collection.artists().map(|(index, artist)| {
            lockup_of(state, format!("kept:{index}"), &artist.name, &t!("common-artist"), &artist.artwork_url, true, GRID, palette)
        }).collect()),
        _ => list("heard", &state.collection.heard),
    };
    let pane = ScrollArea::new("tv-library-pane", horizontal_inset(Element::column(vec![pane, Element::spacer().size(None, Some(38.0))]).padding(5.0), 11.0, SIDE - ROW_PAD))
        .visibility(UNSEEN_BAR).render().flex(1.0);
    // It stays put under the tabs, as BitChord's: each side scrolls on its own.
    Element::column(vec![Element::spacer().size(None, Some(BAR)), Element::row(vec![side, pane]).flex(1.0)])
}

/// The library's lockups, a little smaller than a shelf's.
const GRID: f32 = 120.0;

/// What a pane says with nothing in it.
fn empty(said: &str, palette: ComponentPalette) -> Element {
    Text::new(said, palette).font_size(13.0).render().foreground(SECONDARY).padding(16.0)
}

/// A row of a list, as BitChord's TV: its name, a value or a chevron at its end, on `resting`
/// until the focus turns it into the white platter.
fn list_row(state: &State, id: &str, title: &str, value: Option<&str>, chevron: bool, resting: Color, palette: ComponentPalette) -> Element {
    let on = lit(state, id);
    let ink = if on { PLATTER_INK } else { palette.text };
    let soft = if on { Color::rgb(80, 80, 86) } else { SECONDARY };
    let mut line = vec![Text::new(title, palette).font_size(14.0).truncate().render().font_family(FontFamily::SansMedium).foreground(ink).centered_y().flex(1.0)];
    if let Some(value) = value { line.push(Text::new(value, palette).font_size(13.0).render().foreground(soft).centered_y()); }
    if chevron { line.push(Element::icon(Icon::ChevronRight).large_icon(14.0).foreground(soft).centered_y()); }
    let body = Element::panel(horizontal_inset(Element::row(line).gap(11.0), 16.0, 16.0)).background(if on { PLATTER } else { resting }).radius(11.0);
    let body = if on { body.shadow(LIFT_SHADOW, limn::Point::new(0.0, 6.0), 16.0, 0.0) } else { body };
    Element::clickable(id, body).accessible(limn::Access::new(limn::Role::Button).name(title)).focus_ring(false)
        .animate_scale_about(format!("tv-lift:{id}"), if on { LIST_LIFT } else { 1.0 }, limn::Point::new(0.5, 0.5)).size(None, Some(42.0))
}

/// How far a list's row grows under the focus.
const LIST_LIFT: f32 = 1.03;

/// The tabs' capsule: a tab and the room around it.
const CAPSULE_HEIGHT: f32 = TAB + 8.0;

/// The search, as BitChord's TV: a field no wider than 576, then the results' shelves.
fn search(state: &State, palette: ComponentPalette) -> Element {
    // The field in the tabs' own shape: their capsule's floor, height and round ends, the white
    // platter with dark words under the focus.
    let on = lit(state, "search");
    let field = Element::text_input("search", &state.query, t!("common-search-placeholder"))
        .leading_icon(Icon::Search).leading_icon_tint(if on { PLATTER_INK } else { SECONDARY })
        .background(if on { PLATTER } else { CAPSULE }).foreground(if on { PLATTER_INK } else { palette.text })
        .font_family(FontFamily::SansMedium).font_size(16.0).padding(14.0).radius(CAPSULE_HEIGHT / 2.0).focus_ring(false)
        .accessible(limn::Access::new(limn::Role::SearchInput).name(t!("common-search")))
        .size(Some(576.0), Some(CAPSULE_HEIGHT));
    let field = if on { field.shadow(LIFT_SHADOW, limn::Point::new(0.0, 8.0), 20.0, 0.0) } else { field };
    let found = &state.found;
    let songs = found.tracks.iter().take(SHELF).enumerate()
        .map(|(index, track)| lockup(state, format!("track:{index}"), &track.title, &track.artist, &track.artwork_url, false, palette)).collect();
    let artists = found.artists.iter().enumerate()
        .map(|(index, artist)| lockup(state, format!("artist:{index}"), &artist.name, &t!("common-artist"), &artist.artwork_url, true, palette)).collect();
    let mut parts = vec![Some(field.centered_x())];
    parts.extend([
        shelf("tv-found-songs", &t!("common-songs-heading"), songs, palette),
        shelf("tv-found-artists", &t!("common-artists"), artists, palette),
        shelf("tv-found-albums", &t!("common-albums"), albums(state, "album", &found.albums, palette), palette),
    ]);
    shelves("tv-search", parts)
}

/// A song list's page, as BitChord's TV: its cover beside its name, its artist in red, how
/// long it runs and its Play, then its songs, all of it scrolling under the tabs.
fn songs_page(state: &State, title: &str, about: &str, artwork: &Option<String>, numbered: bool, palette: ComponentPalette) -> Element {
    let tracks = state.page_rows();
    let picture = cover(state, artwork, 232.0, palette).radius(LOCKUP_RADIUS).shadow(LIFT_SHADOW, limn::Point::new(0.0, 10.0), 24.0, 0.0);
    let minutes = tracks.iter().map(|(_, track)| track.duration_ms).sum::<u64>() / 60_000;
    let (shuffle, shuffle_hint) = shuffle_step(state);
    let actions = Element::row(vec![
        button(state, "page-shuffle", shuffle, &shuffle_hint, 43.0, Color::rgba(255, 255, 255, 41), palette),
        button(state, "page-play", Icon::PlayFill, &t!("common-play"), 58.0, RED, palette),
    ]).gap(18.0);
    let words = Element::column(vec![
        Element::spacer().flex(1.0),
        Text::new(title, palette).font_size(31.0).line_height(36.0).render().max_lines(2).font_family(FontFamily::SansBold),
        Element::spacer().size(None, Some(5.0)),
        Text::new(about, palette).font_size(16.0).truncate().render().font_family(FontFamily::SansSemibold).foreground(RED),
        Element::spacer().size(None, Some(3.0)),
        Text::new(format!("{}, {}", songs(tracks.len()), length(minutes)), palette).font_size(13.0).render().foreground(SECONDARY),
        Element::spacer().size(None, Some(19.0)),
        actions,
    ]).flex(1.0);
    let head = Element::row(vec![picture, words]).gap(32.0).size(None, Some(232.0));
    let mut items = vec![Element::spacer().size(None, Some(BAR)), horizontal_inset(head, SIDE, SIDE), Element::spacer().size(None, Some(24.0))];
    // Each row's hairline starts under its words.
    let inset = SIDE - ROW_PAD + if numbered { 43.0 } else { 56.0 };
    for (index, (place, track)) in tracks.iter().enumerate() {
        if index > 0 {
            items.push(horizontal_inset(Element::panel(Element::spacer()).background(HAIRLINE).size(None, Some(1.0)), inset, SIDE));
        }
        items.push(horizontal_inset(row(state, format!("page-track:{place}"), track, numbered.then_some(index + 1), palette), SIDE - ROW_PAD, SIDE - ROW_PAD));
    }
    items.push(Element::spacer().size(None, Some(45.0)));
    ScrollArea::new("tv-page-songs", Element::column(items)).visibility(UNSEEN_BAR).render().flex(1.0)
}

/// Apple's red, which BitChord's TV gives a list's artist, its Play and the song playing.
const RED: Color = Color::rgb(250, 45, 72);
const HAIRLINE: Color = Color::rgba(255, 255, 255, 26);
/// Inside a song's row, at each end.
const ROW_PAD: f32 = 11.0;

/// A song's row, as BitChord's TV: its number on an album, its cover elsewhere, its length at
/// the end, red while it plays, and on a white platter while the focus is on it.
fn row(state: &State, id: String, track: &Track, number: Option<usize>, palette: ComponentPalette) -> Element {
    let on = lit(state, &id);
    let playing = state.current().is_some_and(|current| current.id == track.id);
    let ink = if on { PLATTER_INK } else if playing { RED } else { palette.text };
    let soft = if on { Color::rgb(80, 80, 86) } else { SECONDARY };
    let lead = match number {
        Some(number) => Text::new(number.to_string(), palette).font_size(13.0).render().foreground(if playing && !on { RED } else { soft }).size(Some(21.0), None),
        None => cover(state, &track.artwork_url, 34.0, palette).radius(5.0),
    };
    let copy = Element::column(vec![
        Text::new(&track.title, palette).font_size(13.0).truncate().render().font_family(FontFamily::SansMedium).foreground(ink),
        Text::new(&track.artist, palette).font_size(11.0).truncate().render().foreground(soft),
    ]).centered_y().flex(1.0);
    let length = Text::new(clock(track.duration_ms as f64 / 1000.0), palette).font_size(11.0).render().foreground(soft).centered_y();
    let body = Element::panel(horizontal_inset(Element::row(vec![lead.centered_y(), copy, length]).gap(11.0), ROW_PAD, ROW_PAD))
        .background(if on { PLATTER } else { Color::TRANSPARENT }).radius(10.0);
    Element::clickable(&id, body).accessible(limn::Access::new(limn::Role::Button).name(format!("{} · {}", track.title, track.artist))).focus_ring(false)
        .animate_scale_about(format!("tv-lift:{id}"), if on { ROW_LIFT } else { 1.0 }, limn::Point::new(0.5, 0.5))
        .size(None, Some(if number.is_some() { 42.0 } else { 48.0 }))
}

/// A round button of a page's or the player's, `side` across on `rest`: white with a dark glyph
/// while the focus is on it, or while it rests on white.
fn button(state: &State, id: &str, glyph: Icon, label: &str, side: f32, rest: Color, palette: ComponentPalette) -> Element {
    let on = lit(state, id);
    let fill = if on { PLATTER } else { rest };
    let ink = if fill == PLATTER { PLATTER_INK } else { palette.text };
    let disc = Element::panel(Element::icon(glyph).large_icon(side * 0.46).foreground(ink).centered()).background(fill).round().size(Some(side), Some(side));
    let disc = if on { disc.shadow(LIFT_SHADOW, limn::Point::new(0.0, 8.0), 20.0, 0.0) } else { disc };
    Element::clickable(id, disc).accessible(limn::Access::new(limn::Role::Button).name(label)).focus_ring(false).round()
        .animate_scale_about(format!("tv-lift:{id}"), if on { BUTTON_LIFT } else { 1.0 }, limn::Point::new(0.5, 0.5)).centered_y()
}

/// The player's cover: 62% of the height in the middle, or 54% at the side beside the lyrics,
/// a little above the centre either way to keep clear of the transport.
const COVER: (f32, f32) = (0.62, 320.0);
const COVER_BESIDE: (f32, f32) = (0.54, 272.0);
const COVER_LIFT: f32 = 21.0;
/// Each side of the cover beside the lyrics.
const COVER_SIDE: f32 = 51.0;
/// The transport's round buttons, and their glass at rest and while on.
const TRANSPORT_BUTTON: f32 = 37.0;
const GLASS: Color = Color::rgba(255, 255, 255, 20);
const GLASS_ON: Color = Color::rgba(255, 255, 255, 56);
/// The second line of the player's words, white at 60%.
const SECONDARY: Color = Color::rgba(255, 255, 255, 153);

/// The player over the whole screen, as BitChord's TV: the cover blurred under it, the cover
/// itself in the middle or beside the lyrics, and the transport rising from the foot on the
/// remote's presses. With the transport away the song is named under the cover.
fn player(state: &State, window: Size, palette: ComponentPalette) -> Element {
    let Some(track) = state.current() else { return home(state, palette) };
    let blurred = track.artwork_url.as_deref().and_then(|url| state.artwork.get(&crate::state::backdrop_key(url)));
    let whole = |element: Element| element.size(Some(window.width), Some(window.height));
    let mut layers = vec![whole(Element::panel(Element::spacer()).vertical_gradient(CANVAS.to_vec()))];
    if let Some(blurred) = blurred {
        layers.push(whole(Element::raster_image(blurred.clone().cover())));
        layers.push(whole(Element::panel(Element::spacer()).background(Color::rgba(0, 0, 0, 110))));
    }
    let lyrics = !state.lines().is_empty() && !state.tv_lyrics_hidden;
    let (share, most) = if lyrics { COVER_BESIDE } else { COVER };
    let side = (window.height * share).min(most);
    let picture = sole_cover(state, &largest(state, track), side, palette).radius(11.0).shadow(LIFT_SHADOW, limn::Point::new(0.0, 14.0), 29.0, 0.0)
        .animate_enter(format!("tv-player-cover:{}", track.id));
    let mut stage = vec![Element::spacer().size(None, Some(((window.height - side) / 2.0 - COVER_LIFT).max(0.0))), picture];
    if state.tv_transport.is_none() {
        // Smaller beside the lyrics, as the cover is.
        let scale = side / (window.height * COVER.0).min(COVER.1);
        stage.push(Element::column(vec![
            Element::spacer().size(None, Some(14.0)),
            Text::new(&track.title, palette).font_size(19.0 * scale).line_height(24.0 * scale).truncate().render().font_family(FontFamily::SansBold).center_content(),
            Text::new(&track.artist, palette).font_size(14.0 * scale).truncate().render().foreground(SECONDARY).center_content(),
        ]).size(Some(side), None).animate_enter("tv-player-caption"));
    }
    let stage = Element::column(stage).size(Some(side), Some(window.height));
    layers.push(if lyrics {
        let width = window.width - 2.0 * COVER_SIDE - side - SIDE;
        let lines = crate::view::lyrics(state, width, 0.0, palette).size(Some(width), Some(window.height));
        Element::row(vec![Element::spacer().size(Some(COVER_SIDE), None), stage, Element::spacer().size(Some(COVER_SIDE), None), lines])
    } else {
        stage.centered_x()
    });
    if state.tv_up_next {
        // The stage steps back behind Up Next, as BitChord's dims it.
        layers.push(whole(Element::panel(Element::spacer()).background(Color::rgba(0, 0, 0, 200))));
    }
    if state.tv_transport.is_some() {
        layers.push(Element::column(vec![Element::spacer().flex(1.0), transport(state, track, lyrics, palette)]).size(Some(window.width), Some(window.height)));
    }
    Element::stack(layers).background(CANVAS[1]).size(Some(window.width), Some(window.height))
}

/// The transport, as BitChord's TV: what plays on the left, its buttons on the right, the
/// scrubber under both, over a shade that darkens to the foot.
fn transport(state: &State, track: &Track, lyrics: bool, palette: ComponentPalette) -> Element {
    let about = [Some(track.artist.as_str()), track.album.as_deref()].into_iter().flatten().filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>().join(" — ");
    let liked = state.is_liked(&track.id);
    let words = Element::column(vec![
        Text::new(&track.title, palette).font_size(19.0).line_height(24.0).truncate().render().font_family(FontFamily::SansBold),
        Text::new(about, palette).font_size(14.0).truncate().render().foreground(SECONDARY),
    ]).flex(1.0).centered_y();
    let mut buttons = vec![button(state, "like", if liked { Icon::HeartFill } else { Icon::Heart },
        &if liked { t!("mobile-unlike") } else { t!("common-like") }, TRANSPORT_BUTTON, GLASS, palette)];
    if !state.lines().is_empty() {
        buttons.push(button(state, "lyrics", Icon::Custom(crate::view::LYRICS), &t!("mobile-show-lyrics"), TRANSPORT_BUTTON, if lyrics { GLASS_ON } else { GLASS }, palette));
    }
    buttons.push(button(state, "tv-up-next", Icon::Playlist, &t!("desktop-up-next"), TRANSPORT_BUTTON, if state.tv_up_next { GLASS_ON } else { GLASS }, palette));
    let duration = state.player.duration.max(0.0);
    let elapsed = f64::from(state.progress()) * duration;
    let time = |value: String| Text::new(value, palette).font_size(11.0).render().foreground(SECONDARY);
    let body = Element::column(vec![
        Element::row(vec![words, Element::row(buttons).gap(11.0)]).gap(16.0),
        Element::spacer().size(None, Some(16.0)),
        scrubber(state),
        Element::spacer().size(None, Some(3.0)),
        Element::row(vec![time(clock(elapsed)), Element::spacer().flex(1.0), time(format!("-{}", clock((duration - elapsed).max(0.0).ceil())))]),
    ]);
    let body = if state.tv_up_next { Element::column(vec![body, Element::spacer().size(None, Some(18.0)), up_next(state, palette)]) } else { body };
    // limn blends in linear light: these are BitChord's black at 38% and 66% as the eye sees them.
    let shade = vec![Color::rgba(0, 0, 0, 0), Color::rgba(0, 0, 0, 166), Color::rgba(0, 0, 0, 232)];
    Element::panel(Element::column(vec![Element::spacer().size(None, Some(77.0)), horizontal_inset(body, 45.0, 45.0), Element::spacer().size(None, Some(24.0))]))
        .vertical_gradient(shade).animate_enter("tv-transport")
}

/// Up Next, as BitChord's TV: what follows as small lockups under the scrubber, with the
/// shuffle, the repeat and the autoplay beside its name.
fn up_next(state: &State, palette: ComponentPalette) -> Element {
    let small = |id: &str, glyph: Icon, label: &str, on: bool| button(state, id, glyph, label, 32.0, if on { GLASS_ON } else { GLASS }, palette);
    let repeat = state.player.repeat;
    let head = Element::row(vec![
        Text::new(t!("desktop-up-next"), palette).font_size(16.0).line_height(20.0).render().font_family(FontFamily::SansSemibold).centered_y().flex(1.0),
        Element::row(vec![
            small("shuffle", shuffle_step(state).0, &shuffle_step(state).1, state.player.shuffle),
            small("repeat", if repeat == Repeat::One { Icon::RepeatOne } else { Icon::Repeat }, &t!("mobile-repeat"), repeat != Repeat::Off),
            small("setting:autoplay", Icon::Infinity, &t!("mobile-autoplay"), state.settings.autoplay),
        ]).gap(10.0),
    ]);
    let cards: Vec<Element> = state.player.upcoming.iter().take(SHELF).enumerate()
        .map(|(index, track)| lockup_of(state, format!("upcoming:{index}"), &track.title, &track.artist, &track.artwork_url, false, UP_NEXT, palette)).collect();
    let rest = if cards.is_empty() {
        Text::new(t!("desktop-queue-empty"), palette).font_size(13.0).render().foreground(SECONDARY).size(None, Some(UP_NEXT))
    } else {
        // Room around them for the one lifted, the strip reaching the screen's edges.
        horizontal_inset(Element::horizontal_scroll("tv-up-next-cards", Element::row(cards).gap(CARD_GAP).padding(LIFT_ROOM)), -LIFT_ROOM, -LIFT_ROOM)
    };
    Element::column(vec![head, Element::spacer().size(None, Some(1.0)), rest])
}

/// Up Next's lockups, smaller than a shelf's.
const UP_NEXT: f32 = 99.0;

/// The playhead, as BitChord's TV: a capsule 4 thick, 6 with a knob while the focus is on it,
/// where select plays and pauses and left and right seek.
fn scrubber(state: &State) -> Element {
    let on = lit(state, crate::state::tv::SCRUBBER);
    let played = state.progress().clamp(0.0, 1.0);
    let thick = if on { 6.0 } else { 4.0 };
    let (rest, ink) = if on { (Color::rgba(255, 255, 255, 77), PLATTER) } else { (Color::rgba(255, 255, 255, 56), Color::rgba(255, 255, 255, 224)) };
    let bar = |color: Color| Element::panel(Element::spacer()).background(color).radius(thick / 2.0).size(None, Some(thick));
    // Split at the playhead: a share of the row's room on each side of it.
    let split = |at: Element| Element::row(vec![Element::spacer().flex(played.max(0.0001)), at, Element::spacer().flex((1.0 - played).max(0.0001))]).centered_y();
    let mut layers = vec![bar(rest).centered_y(), Element::row(vec![bar(ink).flex(played.max(0.0001)), Element::spacer().flex((1.0 - played).max(0.0001))]).centered_y()];
    if on {
        let knob = thick * 2.3;
        layers.push(split(Element::panel(Element::spacer()).background(PLATTER).round().size(Some(knob), Some(knob))));
    }
    Element::clickable(crate::state::tv::SCRUBBER, Element::stack(layers).size(None, Some(18.0)))
        .accessible(limn::Access::new(limn::Role::Slider).name(t!("mobile-playback-position"))).focus_ring(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::PlayerView;
    use limn::{DrawCommand, Key, UiAction, UiEvent, UiTree};

    const SCREEN: Size = Size::new(960.0, 540.0);

    fn track(index: usize) -> Track {
        Track { id: format!("song-{index}"), title: format!("Canción {index}"), artist: "Una banda".into(), artist_id: None,
            album: None, album_id: None, artwork_url: None, duration_ms: 200_000 }
    }

    fn television() -> State {
        let mut state = State::default();
        state.tv = true;
        state.collection.heard = (0..8).map(track).collect();
        state
    }

    fn tree(state: &State) -> UiTree {
        let mut tree = UiTree::new(crate::view::view(state, SCREEN));
        tree.set_spatial_focus(true);
        tree.set_reduced_motion(true);
        tree.layout(SCREEN);
        tree
    }

    /// A press of the remote, then the frame the host lays out after it.
    fn press(state: &mut State, tree: &mut UiTree, key: Key) -> Vec<UiAction> {
        let actions = tree.handle_event(UiEvent::Key { key, shift: false, ctrl: false, alt: false });
        state.tv_focused = tree.focused_id().map(str::to_owned);
        tree.set_root(crate::view::view(state, SCREEN));
        tree.layout(SCREEN);
        actions
    }

    #[test]
    fn the_remote_goes_from_the_tabs_down_into_a_shelf_and_along_it() {
        let mut state = television();
        let mut tree = tree(&state);
        assert!(tree.focus("home"));
        press(&mut state, &mut tree, Key::ArrowLeft);
        assert!(tree.frame().commands.iter().any(|command| matches!(command, DrawCommand::FillRect { color, .. } if *color == PLATTER)),
            "the tab with the focus is a white platter");
        press(&mut state, &mut tree, Key::ArrowDown);
        let first = tree.focused_id().unwrap().to_owned();
        assert!(first.starts_with("heard:"), "down from the tabs into the shelf: {first}");
        press(&mut state, &mut tree, Key::ArrowRight);
        let index = |id: &str| id.trim_start_matches("heard:").parse::<usize>().unwrap();
        assert_eq!(index(tree.focused_id().unwrap()), index(&first) + 1, "and along it");
        assert_eq!(press(&mut state, &mut tree, Key::ArrowUp), Vec::new());
        assert!(tree.focused_id().is_some_and(|id| ["home", "mobile-library", "mobile-search", "settings"].contains(&id)), "back up to the tabs");
    }

    #[test]
    fn a_shelf_slides_along_to_bring_the_focus_into_sight() {
        let mut state = television();
        state.collection.heard = (0..12).map(track).collect();
        let mut tree = tree(&state);
        assert!(tree.focus("heard:0"));
        for _ in 0..9 {
            press(&mut state, &mut tree, Key::ArrowRight);
        }
        assert_eq!(tree.focused_id(), Some("heard:9"));
        assert!(tree.horizontal_scroll_offset("tv-heard") > 0.0, "the shelf slid along");
        let card = tree.frame().access.iter().find(|node| node.id == "heard:9").unwrap().rect;
        assert!(card.x + card.width <= SCREEN.width + 0.5, "and the focused card is in sight: {card:?}");
    }

    #[test]
    fn the_player_is_driven_from_its_transport_which_back_sends_away_first() {
        let mut state = television();
        state.show(PlayerView { current: Some(track(0)), playing: true, position: 10.0, duration: 200.0, ..PlayerView::default() });
        state.update(&UiAction::Clicked { id: "mobile-playing".into() });
        assert_eq!(state.page, Page::Playing);
        state.tv_tick(std::time::Instant::now());
        let mut tree = tree(&state);
        assert!(tree.focus(crate::state::tv::SCRUBBER), "the transport is up on the way in");
        assert_eq!(press(&mut state, &mut tree, Key::Enter), vec![UiAction::Clicked { id: crate::state::tv::SCRUBBER.into() }]);
        press(&mut state, &mut tree, Key::ArrowUp);
        assert_eq!(tree.focused_id(), Some("like"), "up from the scrubber to the song's buttons");
        assert_eq!(crate::back_target(&state), Some("tv-transport-away"));
        state.update(&UiAction::Clicked { id: "tv-transport-away".into() });
        tree.set_root(crate::view::view(&state, SCREEN));
        tree.layout(SCREEN);
        assert!(!tree.focus("like"), "and with it its buttons");
        assert_eq!(crate::back_target(&state), Some("back"), "then Back leaves the player");
    }
}
