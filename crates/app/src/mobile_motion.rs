//! Mobile surface motion and ownership of a downward player pull.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use eframe::egui::{self, Pos2};
use limn::{DismissDrag, Frame, HitKind, Point, Role, ScalarSpring, ScalarTransition, Size, SpringMotion, UiAction, UiEvent, UiTree};

use crate::{backend::Command, state::{Page, State}};

const HEADER: f32 = crate::view::MOBILE_HEADER_HEIGHT;
const RETURN: Duration = Duration::from_millis(220);
/// The sheet's pace: critically damped, so it never bounces past the top, and
/// 99% of the way after this, close to the timed open it replaces.
const SHEET: Duration = Duration::from_millis(300);
/// A closing sheet aims this far past the bottom edge and is gone on reaching it,
/// so it leaves at a gentle speed instead of creeping over the last pixels.
const PAST: f32 = 20.0;

pub struct Motion {
    player: ScalarSpring,
    search: ScalarTransition,
    showing: bool,
    closing: bool,
    reduced: bool,
    drag: DismissDrag,
    primary: Option<Touch>,
    underlay: Option<(Size, Frame)>,
}

struct Touch {
    device: egui::TouchDeviceId,
    id: egui::TouchId,
    claimed: bool,
    eligible: bool,
    finished: bool,
    base: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum Pointer { Move, Down, Up, Gone }

struct Plan { pointer: Pointer, consume: bool, cancel: bool }

pub struct DragArea<'a> {
    pub frame: &'a Frame,
    pub origin: Pos2,
    pub size: Size,
    pub scroll: f32,
}

impl Motion {
    pub fn new(now: Instant) -> Self {
        Self { player: ScalarSpring::new(0.0, now).motion(SpringMotion::settling_in(SHEET)), search: ScalarTransition::new(HEADER, now),
            showing: false, closing: false, reduced: false, drag: DismissDrag::new(120.0, 900.0),
            primary: None, underlay: None }
    }

    pub fn set_reduced_motion(&mut self, reduced: bool) { self.reduced = reduced; }

    pub fn advance(&mut self, state: &mut State, size: Size, now: Instant) -> Vec<Command> {
        let reduced = self.reduced || state.settings.reduce_motion;
        let showing = state.mobile && state.page == Page::Playing;
        if showing && !self.showing {
            self.player.snap_to(if reduced || state.queue_open { 0.0 } else { size.height }, now);
            self.launch(0.0, now, reduced);
        } else if !showing || state.queue_open {
            self.drag.cancel();
            self.primary = None;
            self.closing = false;
            self.player.snap_to(0.0, now);
        }
        self.showing = showing;
        if std::mem::take(&mut state.mobile_ui.player_close) && showing && !state.queue_open {
            self.drag.cancel();
            self.primary = None;
            self.closing = true;
            self.launch(size.height + PAST, now, reduced);
        }
        if reduced && !self.drag.is_dragging() { self.player.snap_to(self.player.target(), now); }
        if self.closing && self.player.target() != size.height + PAST {
            self.launch(size.height + PAST, now, reduced);
        }
        if self.closing && (self.offset(now) >= size.height || !self.player.is_active(now)) {
            self.closing = false;
            self.showing = false;
            self.player.snap_to(0.0, now);
            state.mobile_ui.player_offset = 0.0;
            let id = if state.can_go_back() { "back" } else { "home" };
            let commands = state.update(&UiAction::Clicked { id: id.into() });
            state.mobile_ui.revealed = Some(crate::view::mobile_route_key(state));
            return commands;
        }
        state.mobile_ui.player_offset = self.offset(now);
        let search = state.mobile && state.page == Page::Search && state.mobile_search;
        if search {
            self.search.retarget_with_motion(if state.mobile_search_compact() { 0.0 } else { HEADER }, now, RETURN, reduced);
            state.mobile_ui.search_header = Some(self.search.value(now));
        } else {
            self.search.snap_to(HEADER, now);
            state.mobile_ui.search_header = None;
        }
        Vec::new()
    }

    /// TouchInput may replace a pointer event with Leave + Scroll2D, or nothing.
    /// Match those units to the raw sequence so only the claimed touch is consumed.
    pub fn filter(&mut self, raw: &[egui::Event], input: Vec<UiEvent>, state: &State, area: DragArea<'_>, now: Instant) -> Vec<UiEvent> {
        let DragArea { frame, origin, size, scroll } = area;
        let mut plans = VecDeque::new();
        let mut cancel = false;
        for event in raw {
            match event {
                egui::Event::Touch { device_id, id, phase, pos, .. } => {
                    let point = Point::new(pos.x - origin.x, pos.y - origin.y);
                    if *phase == egui::TouchPhase::Start && self.primary.is_none() {
                        let base = self.offset(now);
                        let eligible = eligible(state, frame, point, size, scroll, base);
                        self.drag.begin(point, now, eligible);
                        if eligible {
                            // Preserve the grabbed point while deciding whether this is a tap or a pull.
                            self.player.snap_to(base, now);
                            self.closing = false;
                        }
                        self.primary = Some(Touch { device: *device_id, id: *id, claimed: false, eligible, finished: false, base });
                    } else if let Some(touch) = self.primary.as_mut()
                        .filter(|touch| touch.device == *device_id && touch.id == *id && !touch.finished) {
                        if matches!(phase, egui::TouchPhase::Move | egui::TouchPhase::End) {
                            let owns = self.drag.update(point, now);
                            if owns && !touch.claimed {
                                touch.claimed = true;
                                self.closing = false;
                                cancel = true;
                            }
                            if touch.claimed {
                                self.player.snap_to(touch.base + self.drag.offset(), now);
                            }
                        }
                        if matches!(phase, egui::TouchPhase::End | egui::TouchPhase::Cancel) {
                            // The finger's speed carries into the spring: a flick leaves fast, a pull bounces back.
                            let velocity = self.drag.velocity(now);
                            let dismiss = *phase == egui::TouchPhase::End && self.drag.release(now);
                            self.drag.cancel();
                            if touch.claimed || touch.eligible {
                                self.closing = dismiss;
                                let target = if dismiss { size.height + PAST } else { 0.0 };
                                if self.reduced || state.settings.reduce_motion {
                                    self.player.snap_to(target, now);
                                } else {
                                    self.player.release(self.player.value(now), velocity, target, now);
                                }
                            }
                            touch.finished = true;
                        }
                    }
                }
                egui::Event::WindowFocused(false) => {
                    cancel |= self.primary.as_ref().is_some_and(|touch| touch.claimed);
                    self.drag.cancel();
                    self.primary = None;
                    self.closing = false;
                    self.launch(0.0, now, self.reduced || state.settings.reduce_motion);
                }
                _ => {}
            }
            let pointer = match event {
                egui::Event::PointerMoved(_) => Some(Pointer::Move),
                egui::Event::PointerButton { button: egui::PointerButton::Primary, pressed, .. } =>
                    Some(if *pressed { Pointer::Down } else { Pointer::Up }),
                egui::Event::PointerGone => Some(Pointer::Gone),
                _ => None,
            };
            if let Some(pointer) = pointer {
                plans.push_back(Plan { pointer, consume: self.primary.as_ref().is_some_and(|touch| touch.claimed),
                    cancel: std::mem::take(&mut cancel) });
                if pointer == Pointer::Gone { self.primary = None; }
            }
        }
        let mut output = Vec::with_capacity(input.len());
        let mut input = input.into_iter().peekable();
        while let Some(event) = input.next() {
            let pan_pair = matches!(event, UiEvent::PointerLeave)
                && matches!(input.peek(), Some(UiEvent::Scroll2D { direct: true, .. }));
            let pointer = match event {
                UiEvent::PointerMove(_) => Some(Pointer::Move),
                UiEvent::PointerDown(_) | UiEvent::PointerDownModified { .. } | UiEvent::PointerDownEditor { .. } => Some(Pointer::Down),
                UiEvent::PointerUp(_) => Some(Pointer::Up),
                UiEvent::Scroll2D { direct: true, .. } => Some(Pointer::Move),
                UiEvent::PointerLeave if pan_pair => Some(Pointer::Move),
                UiEvent::PointerLeave => Some(Pointer::Gone),
                _ => None,
            };
            if let Some(pointer) = pointer {
                // A zero-delta pan has no translated unit. Skip only its stale plan.
                while plans.front().is_some_and(|plan| plan.pointer != pointer
                    && !(pointer == Pointer::Move && plan.pointer == Pointer::Up)) {
                    if plans.pop_front().is_some_and(|plan| plan.cancel) { output.push(UiEvent::PointerLeave); }
                }
                if let Some(plan) = plans.pop_front() {
                    if plan.cancel { output.push(UiEvent::PointerLeave); }
                    if plan.consume {
                        if pan_pair { input.next(); }
                        continue;
                    }
                }
            }
            output.push(event);
            if pan_pair { output.extend(input.next()); }
        }
        if cancel || plans.iter().any(|plan| plan.cancel) { output.push(UiEvent::PointerLeave); }
        output
    }

    pub fn active(&self, now: Instant) -> bool { self.player.is_active(now) || self.search.is_active(now) }

    /// An upward release can carry the spring past the top; the sheet stops at the screen's edge.
    fn offset(&self, now: Instant) -> f32 { self.player.value(now).max(0.0) }

    /// A discrete open or close: start at an ease-out's speed, or keep a faster one in flight.
    fn launch(&mut self, target: f32, now: Instant, reduced: bool) {
        if reduced { self.player.snap_to(target, now) } else { self.player.launch(target, SpringMotion::settling_in(SHEET), now) }
    }

    pub fn remember(&mut self, frame: &Frame, state: &State, size: Size) {
        if state.mobile && state.page != Page::Playing {
            let mut visual = Frame::default();
            visual.commands = frame.commands.clone();
            self.underlay = Some((size, visual));
        }
    }

    pub fn underlay(&self, state: &State, size: Size) -> Option<&Frame> {
        self.underlay.as_ref().filter(|(saved, _)| state.mobile && state.page == Page::Playing
            && state.mobile_ui.player_offset > 0.0 && *saved == size).map(|(_, frame)| frame)
    }
}

/// Call only after laying out the current page, before publishing its frame.
pub fn update_header(state: &mut State, tree: &UiTree) -> bool {
    let anchor = match state.page {
        Page::Playing if !state.queue_open => Some("mobile-player-title"),
        Page::Album(_) | Page::Artist(_) | Page::Playlist(_) => Some("mobile-detail-title"),
        _ => None,
    };
    let compact = state.mobile && anchor.and_then(|id| tree.anchor_offset(id)).is_some_and(|top| top < state.mobile_ui.content_insets.top + HEADER);
    std::mem::replace(&mut state.mobile_ui.compact_header, compact) != compact
}

fn eligible(state: &State, frame: &Frame, point: Point, size: Size, scroll: f32, offset: f32) -> bool {
    if !state.mobile || state.page != Page::Playing || state.queue_open
        || !point.x.is_finite() || !point.y.is_finite() || point.x < 0.0 || point.x >= size.width
        || point.y < offset || point.y >= size.height
        || state.creating.is_some() || state.importing.is_some() || state.missed_open || state.confirm.is_some()
        || state.sort_open || state.menu.is_some() || state.equalizer_open || state.sharing.is_some()
        || state.output_open || state.mobile_ui.context.is_some() || state.mobile_ui.library.is_some()
        || state.mobile_ui.connections {
        return false;
    }
    if frame.press_region(point).is_some_and(|hit| matches!(hit.kind,
        HitKind::TextInput | HitKind::TextArea | HitKind::CodeEditor | HitKind::CodeDiff | HitKind::Scrollbar)
        || frame.access.iter().any(|node| node.id == hit.id && node.role == Role::Slider)) {
        return false;
    }
    point.y < offset + state.mobile_ui.content_insets.top + HEADER || (scroll <= 0.01 && frame.regions.iter().any(|region|
        region.id == "mobile-playing" && region.kind == HitKind::Scroll && region.rect.contains(point)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::PlayerView;
    use limn::{Component, Element, HitRegion, Rect, Slider, ComponentPalette};
    use crate::state::Playlist;
    use surco_core::{Album, AlbumRef, Artist, Track};

    fn phone() -> State {
        let track = Track { id: "song".into(), title: "Canción".into(), artist: "Artista".into(), artist_id: None,
            album: None, album_id: None, artwork_url: None, duration_ms: 180_000 };
        let mut state = State::default();
        state.mobile = true;
        state.page = Page::Library;
        state.show(PlayerView { current: Some(track), playing: true, ..PlayerView::default() });
        state.update(&limn::UiAction::Clicked { id: "mobile-playing".into() });
        state
    }

    fn surface() -> Frame {
        let mut frame = Frame::default();
        frame.regions.push(HitRegion { id: "mobile-playing".into(), rect: Rect::new(0.0, HEADER, 390.0, 700.0), kind: HitKind::Scroll });
        frame
    }

    fn raw(phase: egui::TouchPhase, x: f32, y: f32) -> Vec<egui::Event> {
        let pos = egui::pos2(x, y);
        let mut events = vec![egui::Event::Touch { device_id: egui::TouchDeviceId(1), id: egui::TouchId(1), phase, pos, force: None }];
        match phase {
            egui::TouchPhase::Start => {
                events.push(egui::Event::PointerMoved(pos));
                events.push(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() });
            }
            egui::TouchPhase::Move => events.push(egui::Event::PointerMoved(pos)),
            egui::TouchPhase::End => {
                events.push(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() });
                events.push(egui::Event::PointerGone);
            }
            egui::TouchPhase::Cancel => events.push(egui::Event::PointerGone),
        }
        events
    }

    fn settled(now: Instant) -> (Motion, State) {
        let mut motion = Motion::new(now);
        let mut state = phone();
        motion.advance(&mut state, Size::new(390.0, 844.0), now);
        motion.advance(&mut state, Size::new(390.0, 844.0), now + Duration::from_secs(1));
        assert!(!motion.active(now + Duration::from_secs(1)), "the opening spring settles");
        (motion, state)
    }

    fn detail_pages(state: &mut State) -> Vec<(Page, &'static str, &'static str)> {
        let tracks: Vec<_> = (0..30).map(|index| Track {
            id: format!("song-{index}"), title: format!("Canción {index}"), ..state.current().unwrap().clone()
        }).collect();
        state.collection.playlists.push(Playlist { id: 7, name: "Rutas de madrugada".into(), collaborative: false, tracks: tracks.clone() });
        vec![
            (Page::Playlist(7), "mobile-playlist", "Rutas de madrugada"),
            (Page::Album(Album { id: "album".into(), title: "Un álbum".into(), artist: "Artista".into(), artist_id: None,
                year: "2026".into(), artwork_url: None, tracks: tracks.clone() }), "mobile-album", "Un álbum"),
            (Page::Artist(Artist { id: "artist".into(), name: "Una artista".into(), subscribers: None, artwork_url: None,
                songs: tracks, albums: vec![AlbumRef { id: "release".into(), title: "Álbum real".into(), artist: None, year: String::new(), artwork_url: None }],
                singles: vec![AlbumRef { id: "single".into(), title: "Sencillo real".into(), artist: None, year: String::new(), artwork_url: None }] }), "mobile-artist", "Una artista"),
        ]
    }

    fn paint_header(motion: &mut Motion, state: &mut State, tree: &mut UiTree, size: Size, now: Instant) -> Frame {
        motion.advance(state, size, now);
        crate::layout_view(state, tree, size)
    }

    #[test]
    fn detail_headers_keep_the_title_after_culling_and_expand_when_scrolled_back_to_top() {
        let now = Instant::now();
        let size = Size::new(402.0, 778.0);
        let mut state = phone();
        for (page, scroll, title) in detail_pages(&mut state) {
            state.page = page;
            let mut motion = Motion::new(now);
            let mut tree = UiTree::new(crate::view::view(&state, size));
            tree.set_reduced_motion(true);
            tree.layout(size);
            assert!(tree.anchor_offset("mobile-detail-title").unwrap() > HEADER);
            paint_header(&mut motion, &mut state, &mut tree, size, now);
            assert!(!state.mobile_ui.compact_header);

            tree.reveal_at_start("mobile-detail-title", 0.0);
            tree.layout(size);
            paint_header(&mut motion, &mut state, &mut tree, size, now);
            assert!(state.mobile_ui.compact_header, "title takes over the header while passing it");

            tree.scroll_to_end(scroll);
            let end = tree.layout(size);
            let last = if matches!(state.page, Page::Artist(_)) { "page-single:0" } else { "page-track:29" };
            assert!(end.access.iter().any(|node| node.id == last), "the end of {scroll} is visible");
            assert!(tree.anchor_offset("mobile-detail-title").unwrap() < 0.0, "the culled title retains its actual geometry");
            let end = paint_header(&mut motion, &mut state, &mut tree, size, now);
            assert!(state.mobile_ui.compact_header, "{scroll} stays compact after its title is culled");
            assert!(end.commands.iter().any(|command| matches!(command,
                limn::DrawCommand::Text { text, font_size, .. } if text == title && *font_size == 18.0)), "the compact title is painted");

            tree.scroll_to_start(scroll);
            tree.layout(size);
            paint_header(&mut motion, &mut state, &mut tree, size, now);
            assert!(!state.mobile_ui.compact_header, "scrolling back restores the expanded header");

            tree.scroll_to_end(scroll);
            tree.layout(size);
            paint_header(&mut motion, &mut state, &mut tree, size, now);
            assert!(state.mobile_ui.compact_header, "jumping straight to the end also keeps the title");
        }
    }

    #[test]
    fn compact_geometry_is_not_inherited_by_another_detail_or_an_unrelated_page() {
        let now = Instant::now();
        let size = Size::new(402.0, 778.0);
        let mut state = phone();
        let pages = detail_pages(&mut state);
        state.page = pages[0].0.clone();
        let mut motion = Motion::new(now);
        let mut tree = UiTree::new(crate::view::view(&state, size));
        tree.set_reduced_motion(true);
        tree.layout(size);
        paint_header(&mut motion, &mut state, &mut tree, size, now);
        tree.scroll_to_end("mobile-playlist");
        tree.layout(size);
        paint_header(&mut motion, &mut state, &mut tree, size, now);
        assert!(state.mobile_ui.compact_header);

        for page in [pages[1].0.clone(), Page::Library] {
            state.page = page;
            // The host advances motion once against the previous page's frame.
            paint_header(&mut motion, &mut state, &mut tree, size, now);
            assert!(!state.mobile_ui.compact_header);
            paint_header(&mut motion, &mut state, &mut tree, size, now);
            assert!(!state.mobile_ui.compact_header);
        }
        state.page = pages[0].0.clone();
        paint_header(&mut motion, &mut state, &mut tree, size, now);
        assert!(tree.scroll_offset("mobile-playlist") > 0.0, "navigation restores the named viewport");
        assert!(tree.anchor_offset("mobile-detail-title").unwrap() < 0.0);
        paint_header(&mut motion, &mut state, &mut tree, size, now);
        assert!(state.mobile_ui.compact_header, "restoring a deep viewport also restores its compact title");
    }

    #[test]
    fn navigation_never_measures_the_previous_pages_visible_title_for_the_new_route() {
        let now = Instant::now();
        let size = Size::new(402.0, 778.0);
        let mut state = phone();
        let pages = detail_pages(&mut state);
        state.page = pages[0].0.clone();
        let mut motion = Motion::new(now);
        let mut tree = UiTree::new(crate::view::view(&state, size));
        tree.set_reduced_motion(true);
        tree.layout(size);
        paint_header(&mut motion, &mut state, &mut tree, size, now);
        tree.reveal_at_start("mobile-detail-title", 0.0);
        tree.layout(size);
        paint_header(&mut motion, &mut state, &mut tree, size, now);
        assert!(tree.anchor_offset("mobile-detail-title").unwrap() < HEADER);
        assert!(state.mobile_ui.compact_header);

        state.page = pages[1].0.clone();
        paint_header(&mut motion, &mut state, &mut tree, size, now);
        assert!(!state.mobile_ui.compact_header, "the outgoing playlist title is not an album measurement");
        paint_header(&mut motion, &mut state, &mut tree, size, now);
        assert!(!state.mobile_ui.compact_header);
        tree.scroll_to_end("mobile-album");
        tree.layout(size);
        paint_header(&mut motion, &mut state, &mut tree, size, now);
        assert!(state.mobile_ui.compact_header);
        let narrow = Size::new(320.0, 600.0);
        paint_header(&mut motion, &mut state, &mut tree, narrow, now);
        paint_header(&mut motion, &mut state, &mut tree, narrow, now);
        assert!(state.mobile_ui.compact_header, "a deep viewport remains compact after resizing");
        tree.scroll_to_start("mobile-album");
        tree.layout(narrow);
        paint_header(&mut motion, &mut state, &mut tree, narrow, now);
        assert!(!state.mobile_ui.compact_header, "the new layout replaces its measured position at the top");
    }

    #[test]
    fn player_transport_remains_compact_over_long_lyrics_and_queue_navigation() {
        let now = Instant::now();
        let size = Size::new(402.0, 778.0);
        let mut state = phone();
        state.settings.reduce_motion = true;
        state.lyrics = Some(("song".into(), crate::state::Lyrics::Plain {
            lines: vec!["Una letra larga que ocupa varias líneas de la pantalla ".repeat(24); 4], source: "Fixture".into(),
        }));
        let mut motion = Motion::new(now);
        let mut tree = UiTree::new(crate::view::view(&state, size));
        tree.set_reduced_motion(true);
        tree.layout(size);
        paint_header(&mut motion, &mut state, &mut tree, size, now);
        tree.scroll_to_end("mobile-playing");
        tree.layout(size);
        assert!(tree.anchor_offset("mobile-player-title").unwrap() < 0.0);
        let end = paint_header(&mut motion, &mut state, &mut tree, size, now);
        assert!(state.mobile_ui.compact_header);
        assert!(end.access.iter().any(|node| node.id == "mobile-player-header-play"));
        state.queue_open = true;
        paint_header(&mut motion, &mut state, &mut tree, size, now);
        assert!(!state.mobile_ui.compact_header);
        state.queue_open = false;
        paint_header(&mut motion, &mut state, &mut tree, size, now);
        paint_header(&mut motion, &mut state, &mut tree, size, now);
        assert!(state.mobile_ui.compact_header, "queue navigation retains the deep player's geometry");
        tree.scroll_to_start("mobile-playing");
        tree.layout(size);
        paint_header(&mut motion, &mut state, &mut tree, size, now);
        assert!(!state.mobile_ui.compact_header);
        let track = Track { id: "next-song".into(), title: "Otra canción".into(), ..state.current().unwrap().clone() };
        state.show(PlayerView { current: Some(track), playing: true, ..PlayerView::default() });
        paint_header(&mut motion, &mut state, &mut tree, size, now);
        paint_header(&mut motion, &mut state, &mut tree, size, now);
        assert!(!state.mobile_ui.compact_header, "another track starts with its own title geometry");
    }

    #[test]
    fn a_new_album_identity_can_start_at_the_restored_deep_offset() {
        let now = Instant::now();
        let size = Size::new(402.0, 778.0);
        let mut state = phone();
        let pages = detail_pages(&mut state);
        state.page = pages[1].0.clone();
        let mut motion = Motion::new(now);
        let mut tree = UiTree::new(crate::view::view(&state, size));
        tree.set_reduced_motion(true);
        tree.layout(size);
        paint_header(&mut motion, &mut state, &mut tree, size, now);
        tree.scroll_to_end("mobile-album");
        tree.layout(size);
        paint_header(&mut motion, &mut state, &mut tree, size, now);
        let Page::Album(mut next) = state.page.clone() else { unreachable!() };
        next.id = "next-album".into();
        next.title = "El álbum siguiente".into();
        state.page = Page::Album(next);
        let next = paint_header(&mut motion, &mut state, &mut tree, size, now);
        assert!(state.mobile_ui.compact_header, "a new identity requires no previous visible measurement");
        assert!(next.commands.iter().any(|command| matches!(command,
            limn::DrawCommand::Text { text, font_size, .. } if text == "El álbum siguiente" && *font_size == 18.0)));
    }

    #[test]
    fn a_direct_jump_uses_the_hosts_input_then_advance_then_layout_order() {
        let now = Instant::now();
        let size = Size::new(402.0, 778.0);
        let mut state = phone();
        let pages = detail_pages(&mut state);
        state.page = pages[0].0.clone();
        state.player.playing = false;
        let mut motion = Motion::new(now);
        let mut tree = UiTree::new(crate::view::view(&state, size));
        tree.set_reduced_motion(true);
        tree.layout(size);
        paint_header(&mut motion, &mut state, &mut tree, size, now);
        tree.handle_event(UiEvent::ScrollDirect { position: Point::new(200.0, 400.0), delta_y: -10_000.0 });
        // Host advances against the old Frame while the new offset is already set.
        let jumped = paint_header(&mut motion, &mut state, &mut tree, size, now);
        assert!(tree.is_scroll_at_end("mobile-playlist", 0.1));
        assert!(state.mobile_ui.compact_header, "the first paint is correct without more input or playback ticks");
        assert!(jumped.commands.iter().any(|command| matches!(command,
            limn::DrawCommand::Text { text, font_size, .. } if text == "Rutas de madrugada" && *font_size == 18.0)));
    }

    #[test]
    fn a_new_track_has_a_compact_transport_at_an_already_deep_player_offset() {
        let now = Instant::now();
        let size = Size::new(402.0, 778.0);
        let mut state = phone();
        state.settings.reduce_motion = true;
        let lines = vec!["Una letra larga que ocupa varias líneas de la pantalla ".repeat(24); 4];
        state.lyrics = Some(("song".into(), crate::state::Lyrics::Plain { lines: lines.clone(), source: "Fixture".into() }));
        let mut motion = Motion::new(now);
        let mut tree = UiTree::new(crate::view::view(&state, size));
        tree.set_reduced_motion(true);
        tree.layout(size);
        paint_header(&mut motion, &mut state, &mut tree, size, now);
        tree.scroll_to_end("mobile-playing");
        tree.layout(size);
        paint_header(&mut motion, &mut state, &mut tree, size, now);
        let track = Track { id: "next-song".into(), title: "La siguiente canción".into(), ..state.current().unwrap().clone() };
        state.show(PlayerView { current: Some(track), playing: true, ..PlayerView::default() });
        state.lyrics = Some(("next-song".into(), crate::state::Lyrics::Plain { lines, source: "Fixture".into() }));
        let next = paint_header(&mut motion, &mut state, &mut tree, size, now);
        assert!(state.mobile_ui.compact_header, "a new track requires no previous visible title");
        assert!(next.access.iter().any(|node| node.id == "mobile-player-header-play"));
        assert!(next.commands.iter().any(|command| matches!(command,
            limn::DrawCommand::Text { text, font_size, .. } if text == "La siguiente canción" && *font_size == 17.0)));
    }

    #[test]
    fn opening_and_explicit_closing_retarget_without_pausing_and_return_to_the_previous_page() {
        let now = Instant::now();
        let mut motion = Motion::new(now);
        let mut state = phone();
        let size = Size::new(390.0, 844.0);
        motion.advance(&mut state, size, now);
        assert_eq!(state.mobile_ui.player_offset, size.height);
        motion.advance(&mut state, size, now + Duration::from_millis(80));
        let middle = state.mobile_ui.player_offset;
        assert!(middle > 0.0 && middle < size.height);
        state.update(&limn::UiAction::Clicked { id: "mobile-player-dismiss".into() });
        motion.advance(&mut state, size, now + Duration::from_millis(80));
        assert!((state.mobile_ui.player_offset - middle).abs() < 0.01, "closing starts from the displayed position");
        motion.advance(&mut state, size, now + Duration::from_millis(96));
        assert_eq!(state.page, Page::Playing);
        let commands = motion.advance(&mut state, size, now + Duration::from_secs(1));
        assert_eq!(state.page, Page::Library);
        assert_eq!(state.mobile_ui.revealed.as_deref(), Some("mobile:Library"), "the uncovered page does not fade in again");
        assert!(state.player.playing && commands.iter().all(|command| !matches!(command, Command::PlayPause)));
    }

    #[test]
    fn a_normal_tap_passes_unchanged_but_a_claim_cancels_press_and_consumes_pan_and_release() {
        let now = Instant::now();
        let (mut motion, mut state) = settled(now);
        let size = Size::new(390.0, 844.0);
        let at = now + Duration::from_secs(1);
        let down = vec![UiEvent::PointerMove(Point::new(20.0, 200.0)), UiEvent::PointerDown(Point::new(20.0, 200.0))];
        assert_eq!(motion.filter(&raw(egui::TouchPhase::Start, 20.0, 200.0), down.clone(), &state, DragArea { frame: &surface(), origin: Pos2::ZERO, size, scroll: 0.0 }, at), down);
        let up = vec![UiEvent::PointerUp(Point::new(20.0, 200.0)), UiEvent::PointerLeave];
        assert_eq!(motion.filter(&raw(egui::TouchPhase::End, 20.0, 200.0), up.clone(), &state, DragArea { frame: &surface(), origin: Pos2::ZERO, size, scroll: 0.0 }, at + Duration::from_millis(20)), up);
        motion.filter(&raw(egui::TouchPhase::Start, 20.0, 200.0), down, &state, DragArea { frame: &surface(), origin: Pos2::ZERO, size, scroll: 0.0 }, at + Duration::from_secs(1));
        let pan = vec![UiEvent::PointerLeave, UiEvent::Scroll2D { position: Point::new(20.0, 200.0), delta_x: 0.0, delta_y: 60.0, direct: true, shift: false }];
        let consumed = motion.filter(&raw(egui::TouchPhase::Move, 20.0, 260.0), pan, &state, DragArea { frame: &surface(), origin: Pos2::ZERO, size, scroll: 0.0 }, at + Duration::from_millis(1500));
        assert_eq!(consumed, vec![UiEvent::PointerLeave]);
        motion.advance(&mut state, size, at + Duration::from_millis(1500));
        assert_eq!(state.mobile_ui.player_offset, 60.0);
        let released = motion.filter(&raw(egui::TouchPhase::End, 20.0, 260.0), vec![UiEvent::PointerUp(Point::new(20.0, 260.0)), UiEvent::PointerLeave], &state, DragArea { frame: &surface(), origin: Pos2::ZERO, size, scroll: 0.0 }, at + Duration::from_millis(1510));
        assert!(!released.iter().any(|event| matches!(event, UiEvent::PointerUp(_) | UiEvent::Scroll2D { .. })));
        motion.advance(&mut state, size, at + Duration::from_millis(2510));
        assert_eq!(state.mobile_ui.player_offset, 0.0);
        assert_eq!(state.page, Page::Playing, "a short slow pull returns instead of navigating");
    }

    #[test]
    fn body_eligibility_is_frozen_at_begin_and_controls_modals_and_secondary_fingers_cannot_steal() {
        let state = phone();
        let size = Size::new(390.0, 844.0);
        assert!(eligible(&state, &surface(), Point::new(20.0, 20.0), size, 100.0, 0.0));
        assert!(eligible(&state, &surface(), Point::new(20.0, 200.0), size, 0.0, 0.0));
        assert!(!eligible(&state, &surface(), Point::new(20.0, 200.0), size, 1.0, 0.0));
        for kind in [HitKind::TextInput, HitKind::TextArea, HitKind::CodeEditor, HitKind::CodeDiff, HitKind::Scrollbar] {
            let mut frame = surface();
            frame.regions.push(HitRegion { id: "control".into(), rect: Rect::new(0.0, 0.0, 390.0, 844.0), kind });
            assert!(!eligible(&state, &frame, Point::new(20.0, 200.0), size, 0.0, 0.0));
        }
        let mut header = surface();
        header.regions.push(HitRegion { id: "header".into(), rect: Rect::new(0.0, 0.0, 390.0, HEADER), kind: HitKind::Surface });
        assert!(eligible(&state, &header, Point::new(20.0, 20.0), size, 100.0, 0.0));
        let mut modal = phone();
        modal.mobile_ui.connections = true;
        assert!(!eligible(&modal, &surface(), Point::new(20.0, 20.0), size, 0.0, 0.0));
        let slider = Slider::new("seek", 0.5, ComponentPalette::neutral_dark()).render().size(None, Some(44.0));
        let frame = UiTree::new(slider).layout(Size::new(390.0, 44.0));
        assert!(!eligible(&state, &frame, Point::new(20.0, 20.0), size, 0.0, 0.0));
    }

    #[test]
    fn reduced_motion_snaps_autonomous_motion_but_does_not_disable_direct_manipulation() {
        let now = Instant::now();
        let (mut motion, mut state) = settled(now);
        state.settings.reduce_motion = true;
        let size = Size::new(390.0, 844.0);
        let at = now + Duration::from_secs(1);
        motion.filter(&raw(egui::TouchPhase::Start, 20.0, 200.0), Vec::new(), &state, DragArea { frame: &surface(), origin: Pos2::ZERO, size, scroll: 0.0 }, at);
        motion.filter(&raw(egui::TouchPhase::Move, 20.0, 240.0), Vec::new(), &state, DragArea { frame: &surface(), origin: Pos2::ZERO, size, scroll: 0.0 }, at + Duration::from_millis(100));
        motion.advance(&mut state, size, at + Duration::from_millis(100));
        assert_eq!(state.mobile_ui.player_offset, 40.0);
        motion.filter(&raw(egui::TouchPhase::Cancel, 20.0, 240.0), Vec::new(), &state, DragArea { frame: &surface(), origin: Pos2::ZERO, size, scroll: 0.0 }, at + Duration::from_millis(110));
        motion.advance(&mut state, size, at + Duration::from_millis(110));
        assert_eq!(state.mobile_ui.player_offset, 0.0);
        assert!(!motion.active(at + Duration::from_millis(110)));
    }

    #[test]
    fn search_header_retargets_from_its_current_height_and_queue_navigation_does_not_reopen_player() {
        let now = Instant::now();
        let (mut motion, mut state) = settled(now);
        let size = Size::new(390.0, 844.0);
        state.queue_open = true;
        motion.advance(&mut state, size, now + Duration::from_secs(1));
        state.queue_open = false;
        motion.advance(&mut state, size, now + Duration::from_millis(1100));
        assert_eq!(state.mobile_ui.player_offset, 0.0);
        state.page = Page::Search;
        state.mobile_search = true;
        motion.advance(&mut state, size, now + Duration::from_secs(2));
        assert_eq!(state.mobile_ui.search_header, Some(HEADER));
        state.mobile_ui.search_editing = true;
        motion.advance(&mut state, size, now + Duration::from_secs(2));
        motion.advance(&mut state, size, now + Duration::from_millis(2080));
        let middle = state.mobile_ui.search_header.unwrap();
        assert!(middle > 0.0 && middle < HEADER);
        state.mobile_ui.search_editing = false;
        motion.advance(&mut state, size, now + Duration::from_millis(2080));
        assert_eq!(state.mobile_ui.search_header, Some(middle));
        motion.advance(&mut state, size, now + Duration::from_millis(2300));
        assert_eq!(state.mobile_ui.search_header, Some(HEADER));
    }

    #[test]
    fn submitted_search_keeps_the_heading_compact_until_the_unfocused_query_is_empty() {
        let now = Instant::now();
        let mut motion = Motion::new(now);
        motion.set_reduced_motion(true);
        let mut state = State::default();
        state.mobile = true;
        state.mobile_search = true;
        state.query = "Consulta conservada".into();
        state.mobile_ui.blur = true;
        let size = Size::new(390.0, 844.0);
        motion.advance(&mut state, size, now);
        assert_eq!(state.mobile_ui.search_header, Some(0.0));
        assert!(!state.mobile_ui.search_editing && state.focus.is_none(), "visual compactness cannot request keyboard focus");
        state.update(&UiAction::Clicked { id: "mobile-search-cancel".into() });
        motion.advance(&mut state, size, now + Duration::from_millis(1));
        assert_eq!(state.mobile_ui.search_header, Some(0.0));
        assert_eq!(state.query, "Consulta conservada");
        state.query.clear();
        motion.advance(&mut state, size, now + Duration::from_millis(2));
        assert_eq!(state.mobile_ui.search_header, Some(HEADER));
        assert!(!state.mobile_ui.search_editing && state.focus.is_none());
    }

    #[test]
    fn starting_in_scrolled_body_stays_ineligible_and_a_second_finger_cannot_move_the_player() {
        let now = Instant::now();
        let (mut motion, mut state) = settled(now);
        let size = Size::new(390.0, 844.0);
        let frame = surface();
        let area = |scroll| DragArea { frame: &frame, origin: Pos2::ZERO, size, scroll };
        let at = now + Duration::from_secs(1);
        motion.filter(&raw(egui::TouchPhase::Start, 20.0, 200.0), Vec::new(), &state, area(100.0), at);
        motion.filter(&raw(egui::TouchPhase::Move, 20.0, 400.0), Vec::new(), &state, area(0.0), at + Duration::from_millis(200));
        motion.advance(&mut state, size, at + Duration::from_millis(200));
        assert_eq!(state.mobile_ui.player_offset, 0.0);
        motion.filter(&raw(egui::TouchPhase::End, 20.0, 400.0), Vec::new(), &state, area(0.0), at + Duration::from_millis(210));
        motion.filter(&raw(egui::TouchPhase::Start, 20.0, 20.0), Vec::new(), &state, area(100.0), at + Duration::from_secs(1));
        let other = |phase, y| vec![egui::Event::Touch { device_id: egui::TouchDeviceId(1), id: egui::TouchId(2), phase, pos: egui::pos2(20.0, y), force: None }];
        motion.filter(&other(egui::TouchPhase::Start, 200.0), Vec::new(), &state, area(0.0), at + Duration::from_millis(1100));
        motion.filter(&other(egui::TouchPhase::Move, 500.0), Vec::new(), &state, area(0.0), at + Duration::from_millis(1200));
        motion.advance(&mut state, size, at + Duration::from_millis(1200));
        assert_eq!(state.mobile_ui.player_offset, 0.0);
        motion.filter(&raw(egui::TouchPhase::Move, 20.0, 70.0), Vec::new(), &state, area(100.0), at + Duration::from_millis(1500));
        motion.advance(&mut state, size, at + Duration::from_millis(1500));
        assert_eq!(state.mobile_ui.player_offset, 50.0, "the original header finger still owns the gesture");
    }

    #[test]
    fn flick_release_dismisses_but_focus_loss_returns_without_navigation() {
        let now = Instant::now();
        for cancel in [false, true] {
            let (mut motion, mut state) = settled(now);
            let size = Size::new(390.0, 844.0);
            let frame = surface();
            let area = || DragArea { frame: &frame, origin: Pos2::ZERO, size, scroll: 0.0 };
            let at = now + Duration::from_secs(1);
            motion.filter(&raw(egui::TouchPhase::Start, 20.0, 20.0), Vec::new(), &state, area(), at);
            motion.filter(&raw(egui::TouchPhase::Move, 20.0, 80.0), Vec::new(), &state, area(), at + Duration::from_millis(40));
            let end = if cancel { vec![egui::Event::WindowFocused(false)] } else { raw(egui::TouchPhase::End, 20.0, 80.0) };
            let output = motion.filter(&end, Vec::new(), &state, area(), at + Duration::from_millis(50));
            if cancel { assert_eq!(output, vec![UiEvent::PointerLeave]); }
            motion.advance(&mut state, size, at + Duration::from_secs(1));
            assert_eq!(state.page, if cancel { Page::Playing } else { Page::Library });
            assert!(state.player.playing);
            assert_eq!(state.mobile_ui.player_offset, 0.0);
        }
    }

    #[test]
    fn a_batched_drag_consumes_only_its_pointer_units_and_keeps_keyboard_and_the_next_tap() {
        let now = Instant::now();
        let (mut motion, state) = settled(now);
        let size = Size::new(390.0, 844.0);
        let frame = surface();
        let area = || DragArea { frame: &frame, origin: Pos2::ZERO, size, scroll: 0.0 };
        let at = now + Duration::from_secs(1);
        let first = Point::new(20.0, 200.0);
        let mut events = raw(egui::TouchPhase::Start, first.x, first.y);
        events.extend(raw(egui::TouchPhase::Move, 20.0, 250.0));
        events.extend(raw(egui::TouchPhase::End, 20.0, 250.0));
        events.extend(raw(egui::TouchPhase::Start, 100.0, 20.0));
        events.extend(raw(egui::TouchPhase::End, 100.0, 20.0));
        let next = Point::new(100.0, 20.0);
        let output = motion.filter(&events, vec![
            UiEvent::PointerMove(first), UiEvent::PointerDown(first),
            UiEvent::Text("ñ".into()), UiEvent::PointerLeave,
            UiEvent::Scroll2D { position: first, delta_x: 0.0, delta_y: 50.0, direct: true, shift: false },
            UiEvent::PointerLeave,
            UiEvent::PointerMove(next), UiEvent::PointerDown(next), UiEvent::PointerUp(next), UiEvent::PointerLeave,
        ], &state, area(), at);
        assert_eq!(output, vec![UiEvent::PointerMove(first), UiEvent::PointerDown(first), UiEvent::Text("ñ".into()),
            UiEvent::PointerLeave, UiEvent::PointerMove(next), UiEvent::PointerDown(next), UiEvent::PointerUp(next), UiEvent::PointerLeave]);
    }

    #[test]
    fn saved_background_has_only_paint_and_is_invalid_after_resize() {
        let now = Instant::now();
        let mut motion = Motion::new(now);
        let mut state = phone();
        state.page = Page::Library;
        let size = Size::new(390.0, 844.0);
        let frame = UiTree::new(Element::button("background-button", "Library")).layout(size);
        motion.remember(&frame, &state, size);
        state.page = Page::Playing;
        state.mobile_ui.player_offset = 50.0;
        let saved = motion.underlay(&state, size).expect("same-size background");
        assert_eq!(saved.commands, frame.commands);
        assert!(saved.access.is_empty() && saved.regions.is_empty());
        assert!(motion.underlay(&state, Size::new(844.0, 390.0)).is_none());
        state.mobile_ui.player_offset = 0.0;
        assert!(motion.underlay(&state, size).is_none());
    }

    #[test]
    fn catching_an_entering_player_keeps_the_grabbed_point_under_the_finger() {
        let now = Instant::now();
        let mut motion = Motion::new(now);
        let mut state = phone();
        let size = Size::new(390.0, 844.0);
        motion.advance(&mut state, size, now);
        let at = now + Duration::from_millis(80);
        motion.advance(&mut state, size, at);
        let base = state.mobile_ui.player_offset;
        let frame = surface();
        let area = || DragArea { frame: &frame, origin: Pos2::ZERO, size, scroll: 0.0 };
        motion.filter(&raw(egui::TouchPhase::Start, 20.0, base + 20.0), Vec::new(), &state, area(), at);
        motion.advance(&mut state, size, at + Duration::from_millis(20));
        assert_eq!(state.mobile_ui.player_offset, base, "hold the surface while deciding its axis");
        motion.filter(&raw(egui::TouchPhase::Move, 20.0, base + 80.0), Vec::new(), &state, area(), at + Duration::from_millis(50));
        motion.advance(&mut state, size, at + Duration::from_millis(50));
        assert_eq!(state.mobile_ui.player_offset, base + 60.0);
        motion.filter(&raw(egui::TouchPhase::Cancel, 20.0, base + 80.0), Vec::new(), &state, area(), at + Duration::from_millis(60));
        motion.advance(&mut state, size, at + Duration::from_millis(1060));
        assert_eq!(state.mobile_ui.player_offset, 0.0);
        assert_eq!(state.page, Page::Playing);
    }

    /// Pull from the header to y = 160, release at `release` after the start, then
    /// advance every millisecond until the player navigates away. Returns the
    /// offset one 60 Hz frame after release and the time from release to navigation.
    fn dismiss(moves: &[(u64, f32)], release: u64) -> (f32, Duration) {
        let now = Instant::now();
        let (mut motion, mut state) = settled(now);
        let size = Size::new(390.0, 844.0);
        let frame = surface();
        let area = || DragArea { frame: &frame, origin: Pos2::ZERO, size, scroll: 0.0 };
        let at = now + Duration::from_secs(1);
        motion.filter(&raw(egui::TouchPhase::Start, 20.0, 20.0), Vec::new(), &state, area(), at);
        for &(ms, y) in moves {
            motion.filter(&raw(egui::TouchPhase::Move, 20.0, y), Vec::new(), &state, area(), at + Duration::from_millis(ms));
        }
        let released = at + Duration::from_millis(release);
        motion.filter(&raw(egui::TouchPhase::End, 20.0, 160.0), Vec::new(), &state, area(), released);
        let mut first = 0.0;
        for ms in 1..1000 {
            let elapsed = Duration::from_millis(ms);
            motion.advance(&mut state, size, released + elapsed);
            if ms == 17 { first = state.mobile_ui.player_offset; }
            if state.page != Page::Playing {
                assert_eq!(state.mobile_ui.player_offset, 0.0);
                assert!(!motion.active(released + elapsed), "the closed sheet stops repainting");
                return (first, elapsed);
            }
            assert!(motion.active(released + elapsed), "the host repaints while the sheet moves");
        }
        panic!("the released player never finished closing");
    }

    #[test]
    fn a_flick_carries_its_speed_and_leaves_sooner_than_a_slow_pull_past_the_threshold() {
        let (flick, fast) = dismiss(&[(40, 160.0)], 50);
        let (pull, slow) = dismiss(&[(500, 160.0), (1000, 160.0)], 1010);
        assert!(flick > pull + 10.0, "the flick's first frame is already further down: {flick} vs {pull}");
        assert!(fast < slow, "the flick reaches the bottom sooner: {fast:?} vs {slow:?}");
        assert!(slow < Duration::from_millis(300), "a slow pull past the threshold still closes promptly: {slow:?}");
    }

    #[test]
    fn a_short_pull_bounces_back_to_rest_without_navigation() {
        let now = Instant::now();
        let (mut motion, mut state) = settled(now);
        let size = Size::new(390.0, 844.0);
        let frame = surface();
        let area = || DragArea { frame: &frame, origin: Pos2::ZERO, size, scroll: 0.0 };
        let at = now + Duration::from_secs(1);
        motion.filter(&raw(egui::TouchPhase::Start, 20.0, 20.0), Vec::new(), &state, area(), at);
        motion.filter(&raw(egui::TouchPhase::Move, 20.0, 70.0), Vec::new(), &state, area(), at + Duration::from_millis(150));
        motion.filter(&raw(egui::TouchPhase::Move, 20.0, 80.0), Vec::new(), &state, area(), at + Duration::from_millis(200));
        motion.filter(&raw(egui::TouchPhase::End, 20.0, 80.0), Vec::new(), &state, area(), at + Duration::from_millis(210));
        let mut previous = 60.0;
        for frame in 1..60 {
            let time = at + Duration::from_millis(210) + Duration::from_micros(16_667 * frame);
            motion.advance(&mut state, size, time);
            assert!(state.mobile_ui.player_offset <= previous + 1.0 && state.mobile_ui.player_offset >= 0.0);
            previous = state.mobile_ui.player_offset;
            assert_eq!(state.page, Page::Playing);
        }
        assert_eq!(state.mobile_ui.player_offset, 0.0);
        assert!(!motion.active(at + Duration::from_secs(2)), "the returned sheet settles and stops repainting");
    }

    #[test]
    fn reduced_motion_opens_and_closes_the_sheet_without_a_spring() {
        let now = Instant::now();
        let mut motion = Motion::new(now);
        motion.set_reduced_motion(true);
        let mut state = phone();
        let size = Size::new(390.0, 844.0);
        motion.advance(&mut state, size, now);
        assert_eq!(state.mobile_ui.player_offset, 0.0);
        assert!(!motion.active(now));
        state.update(&limn::UiAction::Clicked { id: "mobile-player-dismiss".into() });
        motion.advance(&mut state, size, now);
        assert_eq!(state.page, Page::Library, "an instant close still navigates back");
        assert!(!motion.active(now));
    }
}
