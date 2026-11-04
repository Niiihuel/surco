//! The television's layout, as BitChord's TV (docs/bitchord-look.md): a capsule of tabs at the
//! top, shelves of lockups that lift under the remote's focus, a page for what was chosen, and
//! the player over the whole screen. The remote's arrows move the focus from control to
//! control (`UiTree::set_spatial_focus`); what has it is lifted rather than outlined.

use super::*;
use super::mobile::horizontal_inset;

/// Clear of the screen's overscan, as BitChord's TV keeps: 38 at the sides, 22 above and below.
const SIDE: f32 = 38.0;
const TOP: f32 = 22.0;
/// A lockup's cover, its corners, an artist's round face, and the room between two.
const LOCKUP: f32 = 131.0;
const LOCKUP_RADIUS: f32 = 10.0;
const ARTIST: f32 = 120.0;
const CARD_GAP: f32 = 16.0;
/// How far what has the focus grows: a lockup, a row, a button.
const LIFT: f32 = 1.08;
const ROW_LIFT: f32 = 1.03;
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
        Page::Album(album) => songs_page(state, &album.title, &album.artist, &album.artwork_url, window, palette),
        Page::Artist(artist) => songs_page(state, &artist.name, &t!("common-artist"), &artist.artwork_url, window, palette),
        Page::Playlist(id) => match state.collection.playlist(*id) {
            Some(playlist) => songs_page(state, &playlist.name, &t!("common-playlist"), &playlist.cover().cloned(), window, palette),
            None => home(state, palette),
        },
        Page::Mix(mix) => songs_page(state, &mix_name(mix), &mix_detail(mix), &mix.covers().first().map(|url| (*url).clone()), window, palette),
        Page::Library => library(state, palette),
        Page::Search if state.mobile_search => search(state, palette),
        Page::Search => home(state, palette),
        // The settings, the profile and the rest are the app's own pages, their controls
        // reached by the remote as any others.
        _ => horizontal_inset(page(state, window.width - 2.0 * SIDE, palette), SIDE, SIDE),
    };
    let body = Element::column(vec![bar(state, palette), page.flex(1.0)]);
    // The cover of what plays washes the canvas's top in its colour.
    let wash = state.current().and_then(|track| state.tints.get(track.artwork_url.as_deref()?)).map(|tint| {
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
    let on = lit(state, &id);
    let side = if round { ARTIST } else { LOCKUP };
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
    ScrollArea::new(id, Element::column(shelves.into_iter().flatten().collect()).gap(18.0).padding(4.0)).render().flex(1.0)
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

fn library(state: &State, palette: ComponentPalette) -> Element {
    let playlists = state.collection.playlists.iter().enumerate()
        .map(|(index, playlist)| lockup(state, format!("playlist:{index}"), &playlist.name, &songs(playlist.tracks.len()), &playlist.cover().cloned(), false, palette)).collect();
    let kept_albums = state.collection.albums().map(|(index, album)| {
        lockup(state, format!("kept:{index}"), &album.title, album.artist.as_deref().unwrap_or(""), &album.artwork_url, false, palette)
    }).collect();
    let kept_artists = state.collection.artists()
        .map(|(index, artist)| lockup(state, format!("kept:{index}"), &artist.name, &t!("common-artist"), &artist.artwork_url, true, palette)).collect();
    let mut lists = vec![lockup(state, "downloads".into(), &t!("common-downloads"), &songs(state.saved.len()), &None, false, palette)];
    if !state.collection.liked.is_empty() {
        lists.push(lockup(state, "liked".into(), &t!("common-liked-songs"), &songs(state.collection.liked.len()), &None, false, palette));
    }
    lists.extend::<Vec<Element>>(playlists);
    shelves("tv-library", vec![
        shelf("tv-lists", &t!("desktop-playlists"), lists, palette),
        shelf("tv-kept-albums", &t!("common-albums"), kept_albums, palette),
        shelf("tv-kept-artists", &t!("common-artists"), kept_artists, palette),
    ])
}

/// The search, as BitChord's TV: a field no wider than 576, then the results' shelves.
fn search(state: &State, palette: ComponentPalette) -> Element {
    let field = SearchField::new("search", &state.query, t!("common-search-placeholder"), SCHEME.on_raised()).size(ControlSize::Spacious).width(576.0).render()
        .radius(12.0).focus_ring(false);
    let field = if lit(state, "search") { field.border(PLATTER, 2.0) } else { field };
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

/// A song list's page, as BitChord's TV: its cover beside its name, and its songs as rows the
/// focus goes down.
fn songs_page(state: &State, title: &str, about: &str, artwork: &Option<String>, window: Size, palette: ComponentPalette) -> Element {
    let picture = cover(state, artwork, 232.0, palette).radius(LOCKUP_RADIUS).shadow(LIFT_SHADOW, limn::Point::new(0.0, 10.0), 28.0, 0.0);
    let play = button(state, "page-play", Icon::PlayFill, &t!("common-play"), true, palette);
    let shuffle = button(state, "page-shuffle", shuffle_step(state).0, &shuffle_step(state).1, false, palette);
    let head = Element::column(vec![
        Text::new(title, palette).font_size(31.0).line_height(36.0).render().max_lines(2).font_family(FontFamily::SansBold),
        Text::new(about, palette).font_size(16.0).muted().truncate().render(),
        Element::spacer().size(None, Some(12.0)),
        Element::row(vec![play, shuffle]).gap(16.0),
    ]).gap(6.0).centered_y().flex(1.0);
    let rows: Vec<Element> = state.page_rows().into_iter().map(|(place, track)| row(state, format!("page-track:{place}"), track, palette)).collect();
    let list = ScrollArea::new("tv-page-songs", Element::column(rows).gap(4.0).padding(8.0)).render().flex(1.0);
    let side = Element::column(vec![Element::row(vec![picture, head]).gap(28.0), Element::spacer().flex(1.0)]).size(Some((window.width * 0.42).min(460.0)), None);
    horizontal_inset(Element::row(vec![side, list]).gap(32.0), SIDE, SIDE).padding(4.0)
}

/// A song's row: its number's place taken by its cover, a white platter under it while the
/// focus is on it.
fn row(state: &State, id: String, track: &Track, palette: ComponentPalette) -> Element {
    let on = lit(state, &id);
    let ink = if on { PLATTER_INK } else { palette.text };
    let picture = cover(state, &track.artwork_url, 40.0, palette).radius(6.0);
    let copy = Element::column(vec![
        Text::new(&track.title, palette).font_size(15.0).truncate().render().font_family(FontFamily::SansMedium).foreground(ink),
        Text::new(&track.artist, palette).font_size(13.0).truncate().render().foreground(if on { Color::rgb(80, 80, 86) } else { palette.muted }),
    ]).gap(2.0).centered_y().flex(1.0);
    let body = Element::panel(Element::row(vec![picture.centered_y(), copy]).gap(12.0).padding(8.0)).background(if on { PLATTER } else { Color::TRANSPARENT }).radius(10.0);
    Element::clickable(&id, body).accessible(limn::Access::new(limn::Role::Button).name(format!("{} · {}", track.title, track.artist))).focus_ring(false)
        .animate_scale_about(format!("tv-lift:{id}"), if on { ROW_LIFT } else { 1.0 }, limn::Point::new(0.5, 0.5)).size(None, Some(56.0))
}

/// A round button of the player's or a page's: white with a dark glyph while the focus is on
/// it, or while it is the page's main one.
fn button(state: &State, id: &str, glyph: Icon, label: &str, main: bool, palette: ComponentPalette) -> Element {
    let on = lit(state, id);
    let (fill, ink) = if on || main { (PLATTER, PLATTER_INK) } else { (Color::rgba(255, 255, 255, 30), palette.text) };
    let side = if main { 56.0 } else { 46.0 };
    let disc = Element::panel(Element::icon(glyph).large_icon(side * 0.45).foreground(ink).centered()).background(fill).round().size(Some(side), Some(side));
    let disc = if on { disc.shadow(LIFT_SHADOW, limn::Point::new(0.0, 8.0), 20.0, 0.0) } else { disc };
    Element::clickable(id, disc).accessible(limn::Access::new(limn::Role::Button).name(label)).focus_ring(false).round()
        .animate_scale_about(format!("tv-lift:{id}"), if on { BUTTON_LIFT } else { 1.0 }, limn::Point::new(0.5, 0.5)).centered_y()
}

/// The player over the whole screen, as BitChord's TV: the cover blurred under it, the cover
/// itself at 62% of the height, gliding left beside the lyrics when there are some.
fn player(state: &State, window: Size, palette: ComponentPalette) -> Element {
    let Some(track) = state.current() else { return home(state, palette) };
    let blurred = track.artwork_url.as_deref().and_then(|url| state.artwork.get(&crate::state::backdrop_key(url)));
    let whole = |element: Element| element.size(Some(window.width), Some(window.height));
    let mut layers = vec![whole(Element::panel(Element::spacer()).vertical_gradient(CANVAS.to_vec()))];
    if let Some(blurred) = blurred {
        layers.push(whole(Element::raster_image(blurred.clone().cover())));
        layers.push(whole(Element::panel(Element::spacer()).background(Color::rgba(0, 0, 0, 110))));
    }
    let lyrics = !state.lines().is_empty();
    let side = (window.height * if lyrics { 0.54 } else { 0.62 }).min(if lyrics { 272.0 } else { 320.0 });
    let picture = sole_cover(state, &largest(state, track), side, palette).radius(11.0).shadow(LIFT_SHADOW, limn::Point::new(0.0, 14.0), 29.0, 0.0)
        .animate_enter(format!("tv-player-cover:{}", track.id));
    let (shuffle, shuffle_hint) = shuffle_step(state);
    let playing = state.player.playing;
    let transport = Element::row(vec![
        button(state, "shuffle", shuffle, &shuffle_hint, false, palette),
        button(state, "previous", Icon::SkipBack, &t!("mobile-previous-song"), false, palette),
        button(state, "play-pause", if playing { Icon::PauseFill } else { Icon::PlayFill }, &if playing { t!("common-pause") } else { t!("common-play") }, true, palette),
        button(state, "next", Icon::SkipForward, &t!("common-next-song"), false, palette),
        button(state, "lyrics", Icon::Custom(crate::view::LYRICS), &t!("mobile-show-lyrics"), false, palette),
    ]).gap(18.0);
    let timeline = Progress::new(state.progress(), ComponentPalette { selected: Color::rgba(255, 255, 255, 66), ..palette }).height(4.0).render().size(Some(side.max(320.0)), None);
    let copy = Element::column(vec![
        Text::new(&track.title, palette).font_size(24.0).truncate().render().font_family(FontFamily::SansBold).center_content(),
        Text::new(&track.artist, palette).font_size(17.0).truncate().render().foreground(Color::rgba(255, 255, 255, 170)).center_content(),
    ]).gap(4.0).size(Some(side.max(320.0)), None);
    let stage = Element::column(vec![picture.centered_x(), copy.centered_x(), timeline.centered_x(), transport.centered_x()]).gap(18.0).centered_y();
    let content = if lyrics {
        let width = window.width * 0.5;
        let lines = crate::view::lyrics(state, width, 0.0, palette);
        Element::row(vec![stage.size(Some(window.width * 0.45), None), lines.size(Some(width), Some(window.height - 2.0 * TOP))]).gap(16.0)
    } else { stage.centered_x() };
    layers.push(horizontal_inset(Element::column(vec![Element::spacer().size(None, Some(TOP)), content.flex(1.0), Element::spacer().size(None, Some(TOP))]), SIDE, SIDE));
    Element::stack(layers).background(CANVAS[1]).size(Some(window.width), Some(window.height))
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
    fn the_player_is_driven_from_its_transport_and_left_by_back() {
        let mut state = television();
        state.show(PlayerView { current: Some(track(0)), playing: true, position: 10.0, duration: 200.0, ..PlayerView::default() });
        state.update(&UiAction::Clicked { id: "mobile-playing".into() });
        assert_eq!(state.page, Page::Playing);
        let mut tree = tree(&state);
        assert!(tree.focus("play-pause"));
        press(&mut state, &mut tree, Key::ArrowRight);
        assert_eq!(tree.focused_id(), Some("next"));
        assert_eq!(press(&mut state, &mut tree, Key::Enter), vec![UiAction::Clicked { id: "next".into() }]);
        press(&mut state, &mut tree, Key::ArrowLeft);
        press(&mut state, &mut tree, Key::ArrowLeft);
        assert_eq!(tree.focused_id(), Some("previous"));
        assert_eq!(crate::back_target(&state), Some("back"), "a television's player is a page left as any other");
    }
}
