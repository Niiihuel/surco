//! Surco's shared Rust application: desktop executable and iOS static library.
//!
//! `cargo run -p surco` opens the player. For checking a build without touching it:
//! `--search <text>` submits a search at startup, `--play <n>` plays the n-th result when
//! it arrives, `--seek <s>` jumps there once it sounds, `--click <id>@<ms>` presses a control
//! and `--slide <id>=<fraction>@<ms>` sets a slider that long after the window opened (give them
//! in order), and `--screenshot <file.png>` saves
//! the window after `--after <ms>` and exits. `--mobile` previews the phone layout.

mod backend;
mod clip;
mod handoff;
mod i18n;
mod import;
#[cfg(any(target_os = "ios", test))]
mod ios_media;
mod library;
mod media_keys;
mod mobile_input;
mod mobile_motion;
#[cfg(any(target_os = "ios", test))]
mod native_surfaces;
#[cfg(any(target_os = "ios", test))]
mod ios_ui;
#[cfg(any(target_os = "ios", test))]
mod ios_symbols;
mod paths;
mod player;
mod state;
mod update;
mod view;
mod window;

use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use eframe::egui;
use limn::{Element, Size, UiAction, UiTree};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::watch;

use backend::{Command, Event};
use player::PlayerView;
use state::{Side, State};

#[cfg(target_os = "ios")]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

struct Host {
    tree: UiTree,
    state: State,
    commands: UnboundedSender<Command>,
    events: Receiver<Event>,
    player: watch::Receiver<PlayerView>,
    started: bool,
    /// The clip of what plays, while its panel shows it.
    clip: Option<clip::Screen>,
    /// Whether the window was last asked to take the whole screen.
    fullscreen: bool,
    /// Who draws the window's frame.
    chrome: window::Chrome,
    opened: Instant,
    /// When something other than the song going on last happened to the window.
    stirred: Instant,
    script: Script,
    /// The song the panel of what plays was last scrolled for.
    panel_song: Option<String>,
    /// What played when the app was last closed, handed to the player as it starts.
    session: Option<state::Session>,
    touch: mobile_input::TouchInput,
    motion: mobile_motion::Motion,
    /// The notice already revealed; ordinary repaints preserve manual scrolling.
    notice: Option<String>,
    #[cfg(target_os = "ios")]
    native_ui: Option<ios_ui::NativeUi>,
    #[cfg(target_os = "ios")]
    surfaces: native_surfaces::Controller,
    #[cfg(target_os = "ios")]
    keyboard_inset: limn::ScalarTransition,
}

/// What the command line asked the window to do by itself.
#[derive(Default)]
struct Script {
    mobile: bool,
    /// Open the first run's steps whatever is kept, to see them.
    onboarding: bool,
    search: Option<String>,
    play: Option<usize>,
    seek: Option<f64>,
    actions: Vec<(Duration, UiAction)>,
    screenshot: Option<PathBuf>,
    after: Duration,
    requested: bool,
}

/// Follow against the current page's measured viewport and overlaid bars.
fn update_lyrics_viewport(state: &mut State, tree: &mut UiTree) -> bool {
    let reset = std::mem::take(&mut state.lyrics_scroll_reset);
    if reset { tree.scroll_to_start("lyrics-list"); }
    // Keep the notice readable until dismissal or a deliberate manual scroll.
    // Do not consume the new active line: closing the notice should reveal it.
    if state.mobile && state.mobile_ui.notice.is_some() { return reset; }
    if let Some(line) = state.line_to_reveal() {
        if state.mobile {
            let frame = tree.frame();
            let top = frame.regions.iter().find(|region| region.id == "mobile-header-bar")
                .map_or(0.0, |region| region.rect.y + region.rect.height);
            let scroll_top = frame.regions.iter().find(|region| region.id == "lyrics-list")
                .map_or(0.0, |region| region.rect.y);
            let body = frame.regions.iter().find(|region| region.id == "mobile-footer-bar")
                .map_or(0.0, |region| region.rect.y - top);
            // Centering the full viewport puts wrapped lines behind the fixed
            // lyrics transport in landscape. Rest the line at the anchor of the
            // unobscured body between the header and the transport instead.
            tree.reveal_at_start(&line, (top - scroll_top).max(0.0) + view::lyric_anchor(body));
        } else {
            tree.center(&line);
        }
        true
    } else {
        reset
    }
}

/// Follow the active lyric, laying out the page it moved, and on a phone let the rows glide
/// after that jump. A new track's reset only starts the list over.
fn follow_lyrics(state: &mut State, tree: &mut UiTree, window: Size, seen: &mut Option<String>) -> Option<limn::Frame> {
    let (from, reset) = (tree.scroll_offset("lyrics-list"), state.lyrics_scroll_reset);
    if !update_lyrics_viewport(state, tree) { return None; }
    let frame = layout_with_notice(state, tree, window, seen);
    if state.mobile && !reset {
        state.glide_lyrics(tree.scroll_offset("lyrics-list") - from, Instant::now());
    }
    Some(frame)
}

/// Measure where a long song list stands in its page and, when that brings rows into view
/// that were not built, lay the page out again with them.
fn follow_track_window(state: &mut State, tree: &mut UiTree, window: Size, seen: &mut Option<String>) -> Option<limn::Frame> {
    let measured = tree.anchor_offset(view::TRACK_LIST).map(|top| (top, window.height));
    let (count, pitch) = (state.page_rows().len(), view::track_pitch(state));
    let built = view::track_range(count, pitch, state.track_window);
    state.track_window = measured;
    (view::track_range(count, pitch, measured) != built).then(|| layout_with_notice(state, tree, window, seen))
}

/// Show the page's bar once its actions have scrolled under the page's top, and lay out
/// again when that changes.
fn follow_page_actions(state: &mut State, tree: &mut UiTree, window: Size, seen: &mut Option<String>) -> Option<limn::Frame> {
    let stuck = tree.anchor_offset(view::PAGE_ACTIONS).is_some_and(|top| top < view::STUCK_FROM);
    (stuck != state.page_stuck).then(|| {
        state.page_stuck = stuck;
        layout_with_notice(state, tree, window, seen)
    })
}

/// Slide the player's lyric card so the line before the one being sung is at its top,
/// laying out again when it moved.
fn follow_lyric_card(state: &mut State, tree: &mut UiTree, window: Size, seen: &mut Option<String>) -> Option<limn::Frame> {
    let line = |index: usize| tree.anchor_offset(&format!("lyric-card:{index}"));
    let to = line(0).map(|first| state.active_line().and_then(|sung| line(sung.saturating_sub(1))).map_or(0.0, |top| top - first));
    state.follow_lyric_card(to, Instant::now()).then(|| layout_with_notice(state, tree, window, seen))
}

/// Slides a long song name in the right panel along its room: how far it reaches is measured
/// once a song, by asking its strip for more than it has, as it stops at its end; then the
/// clock moves it, laying out again when it moved.
fn follow_title(state: &mut State, tree: &mut UiTree, window: Size, seen: &mut Option<String>) -> Option<limn::Frame> {
    let song = state.panel_title()?.to_owned();
    if state.title_reach.as_ref().is_none_or(|(id, ..)| *id != song) {
        tree.set_horizontal_scroll_offset(view::PANEL_TITLE, f32::MAX);
        layout_with_notice(state, tree, window, seen);
        let reach = tree.horizontal_scroll_offset(view::PANEL_TITLE);
        state.title_reach = Some((song, if reach.is_finite() { reach } else { 0.0 }, Instant::now()));
        tree.set_horizontal_scroll_offset(view::PANEL_TITLE, 0.0);
        return Some(layout_with_notice(state, tree, window, seen));
    }
    if !state.title_moves() { return None; }
    let (_, reach, since) = state.title_reach.as_ref()?;
    let at = state::marquee(since.elapsed(), *reach);
    ((tree.horizontal_scroll_offset(view::PANEL_TITLE) - at).abs() >= 0.5).then(|| {
        tree.set_horizontal_scroll_offset(view::PANEL_TITLE, at);
        layout_with_notice(state, tree, window, seen)
    })
}

fn layout_view(state: &mut State, tree: &mut UiTree, window: Size) -> limn::Frame {
    tree.set_root(view::view(state, window));
    let frame = tree.layout(window);
    // Header geometry must match this page and this scroll position, including
    // the first paint after an input event when no later repaint is scheduled.
    if mobile_motion::update_header(state, tree) {
        tree.set_root(view::view(state, window));
        tree.layout(window)
    } else {
        frame
    }
}

fn layout_with_notice(state: &mut State, tree: &mut UiTree, window: Size, seen: &mut Option<String>) -> limn::Frame {
    let mut frame = layout_view(state, tree, window);
    let notice = state.mobile.then_some(state.mobile_ui.notice.as_deref()).flatten();
    if seen.as_deref() == notice { return frame; }
    *seen = notice.map(str::to_owned);
    if seen.is_none() { return frame; }
    // A first reveal locates a newly inserted notice even in a culled branch.
    if !frame.access.iter().any(|node| node.id == "mobile-notice-close") {
        tree.reveal("mobile-notice-close");
        frame = layout_view(state, tree, window);
    }
    let Some(close) = frame.access.iter().find(|node| node.id == "mobile-notice-close") else { return frame; };
    let center = limn::Point::new(close.rect.x + close.rect.width / 2.0, close.rect.y + close.rect.height / 2.0);
    let Some(scroll) = frame.regions.iter().rev().find(|region|
        region.kind == limn::HitKind::Scroll && region.rect.contains(center)) else { return frame; };
    let header_bottom = frame.regions.iter().find(|region| region.id == "mobile-header-bar")
        .map_or(0.0, |region| region.rect.y + region.rect.height);
    // Fixed filters can already put this viewport below the header. Reserve
    // only the remaining overlap plus room for the short decorative fade.
    let margin = (header_bottom - scroll.rect.y).max(0.0) + 20.0;
    tree.reveal_at_start("mobile-notice-close", margin);
    layout_view(state, tree, window)
}

fn native_surfaces_present(state: &State) -> bool {
    #[cfg(any(target_os = "ios", test))]
    { native_surfaces::has_sheet(state) }
    #[cfg(not(any(target_os = "ios", test)))]
    { let _ = state; false }
}

fn mobile_canvas(ui: &mut egui::Ui, preview: bool) -> egui::Rect {
    // egui's root Ui covers the safe content rect. The phone background must
    // also paint behind the status bar and home indicator.
    let full = ui.ctx().viewport_rect();
    let canvas = if preview {
        // A tiling compositor can ignore the preview window's requested size.
        egui::Rect::from_center_size(full.center(), egui::vec2(full.width().min(390.0), full.height().min(844.0)))
    } else { full };
    ui.set_clip_rect(canvas);
    canvas
}

fn mobile_content_insets(canvas: egui::Rect, content: egui::Rect, keyboard: f32) -> state::mobile::ContentInsets {
    // Validate the platform boundary once; consumers can use these as padding.
    let bounded = |value: f32, limit: f32| {
        if value.is_finite() { value.clamp(0.0, limit) } else { 0.0 }
    };
    let width = if canvas.width().is_finite() { canvas.width().max(0.0) } else { 0.0 };
    let height = if canvas.height().is_finite() { canvas.height().max(0.0) } else { 0.0 };
    let top = bounded(content.top() - canvas.top(), height);
    let left = bounded(content.left() - canvas.left(), width);
    let right = bounded(canvas.right() - content.right(), width - left);
    let safe_bottom = bounded(canvas.bottom() - content.bottom(), height - top);
    let bottom = safe_bottom.max(bounded(keyboard, height - top));
    state::mobile::ContentInsets { top, right, bottom, left }
}

impl Host {
    fn send(&self, commands: Vec<Command>) {
        for command in commands {
            // The backend only goes away when the window does.
            let _ = self.commands.send(command);
        }
    }

    fn run_script(&mut self) {
        if let Some(query) = self.script.search.take() {
            self.state.query = query;
            let commands = self.state.search();
            self.send(commands);
        }
        if let Some(index) = self.script.play {
            if index < self.state.found.tracks.len() {
                self.script.play = None;
                let commands = self.state.update(&UiAction::Clicked { id: format!("track:{index}") });
                self.send(commands);
            }
        }
        if let Some(seconds) = self.script.seek.filter(|_| self.state.player.position > 0.0) {
            self.script.seek = None;
            self.send(vec![Command::Seek(seconds)]);
        }
        while self.script.actions.first().is_some_and(|(at, _)| *at <= self.opened.elapsed()) {
            let (_, action) = self.script.actions.remove(0);
            let commands = self.state.update(&action);
            self.send(commands);
        }
    }

    fn capture(&mut self, ctx: &egui::Context, path: &PathBuf) {
        if !self.script.requested && self.opened.elapsed() > self.script.after {
            self.script.requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let image = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            let [width, height] = image.size;
            if let Err(error) = image::save_buffer(path, image.as_raw(), width as u32, height as u32, image::ColorType::Rgba8) {
                eprintln!("could not save {}: {error}", path.display());
                std::process::exit(1);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint();
    }
}

/// For this long after something happened to it the window draws at the display's own
/// rate. After that only what plays moves it, and its bars and its video gain nothing
/// from more than thirty frames a second, each of which draws the whole window. A video
/// wakes the window for each of its own pictures: beside one, the bars only make sure
/// of a frame now and then.
const SETTLED_AFTER: Duration = Duration::from_millis(500);
// A phone's battery is worth more than its bars' smoothness: fifteen frames there.
const SETTLED_FRAME: Duration = Duration::from_millis(if cfg!(target_os = "ios") { 66 } else { 33 });
const SETTLED_FRAME_BESIDE_VIDEO: Duration = Duration::from_millis(100);
/// What drifts by itself, slowly, needs few frames: fifteen a second.
const AMBIENT_FRAME: Duration = Duration::from_millis(66);
/// The least time between two frames of the video seen through the shut panel's strip.
const PEEK_PACE: Duration = Duration::from_millis(125);

/// Its own id, and with it its own settings folder.
const APP_ID: &str = "surco";
const VOLUME_KEY: &str = "volume";
const COLLECTION_KEY: &str = "collection";
const SETTINGS_KEY: &str = "settings";
const LIBRARY_OPEN_KEY: &str = "library-open";
const PANEL_OPEN_KEY: &str = "panel-open";
const LEFT_KEY: &str = "left-width";
const RIGHT_KEY: &str = "right-width";
const EQUALIZER_KEY: &str = "equalizer";
const OUTPUT_KEY: &str = "output";
const PROFILE_KEY: &str = "profile";
const SESSION_KEY: &str = "session";

impl eframe::App for Host {
    #[cfg(target_os = "ios")]
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Match the opaque launch canvas, including outside UIKit's safe area.
        egui::Color32::from_rgb(18, 18, 18).to_normalized_gamma_f32()
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(VOLUME_KEY, self.state.volume.to_string());
        if let Ok(collection) = serde_json::to_string(&self.state.collection) {
            storage.set_string(COLLECTION_KEY, collection);
        }
        if let Ok(settings) = serde_json::to_string(&self.state.settings) {
            storage.set_string(SETTINGS_KEY, settings);
        }
        storage.set_string(LIBRARY_OPEN_KEY, self.state.library_open.to_string());
        storage.set_string(PANEL_OPEN_KEY, self.state.panel_open.to_string());
        storage.set_string(LEFT_KEY, self.state.left_width.to_string());
        storage.set_string(RIGHT_KEY, self.state.right_width.to_string());
        storage.set_string(EQUALIZER_KEY, state::equalizer_text(self.state.equalizer));
        storage.set_string(OUTPUT_KEY, self.state.output.clone().unwrap_or_default());
        if let Ok(profile) = serde_json::to_string(&self.state.profile) {
            storage.set_string(PROFILE_KEY, profile);
        }
        // Nothing playing leaves nothing to go on with.
        let session = self.state.session().and_then(|session| serde_json::to_string(&session).ok());
        storage.set_string(SESSION_KEY, session.unwrap_or_default());
    }

    fn ui(&mut self, ui: &mut egui::Ui, host: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !self.started {
            self.started = true;
            limn_egui::attach_fonts(&mut self.tree, &ctx);
            // The engine starts at full volume and flat; the window owns both settings.
            self.send(vec![Command::Volume(self.state.volume), Command::Equalizer(self.state.equalizer), Command::Autoplay(self.state.settings.autoplay), Command::Crossfade(f64::from(self.state.settings.crossfade)), Command::Quality(self.state.settings.quality.bps()), Command::Mono(self.state.settings.mono), Command::Normalize(self.state.settings.normalize)]);
            if self.state.output.is_some() {
                self.send(vec![Command::Output(self.state.output.clone())]);
            }
            if let Some(state::Session { tracks, position }) = self.session.take() {
                self.send(vec![Command::Session { tracks, position }]);
            }
            // A first start goes through its steps, or offers to bring music in; a scripted run
            // is about something else unless it asks for them.
            let scripted = !self.script.actions.is_empty() || self.script.search.is_some();
            self.state.welcome = !self.state.settings.welcomed && !scripted;
            if self.script.onboarding || (self.state.first_run() && !scripted) {
                self.state.welcome = false;
                let commands = self.state.begin_onboarding(false);
                self.send(commands);
            }
            let covers = self.state.collection_covers();
            self.send(covers);
            let picks = self.state.ask_picks();
            self.send(picks);
            let news = self.state.ask_news();
            self.send(news);
        }
        limn_egui::normalize_keyboard_input(&ctx);
        // Motion stays on for a scripted run too: whoever looks at its window sees the app as
        // it is, and the screenshot is taken once things have settled.
        let still = self.state.settings.reduce_motion;
        #[cfg(target_os = "ios")]
        let still = still || ios_ui::reduce_motion_enabled();
        // Decoded covers are only a cache: short of memory, they are decoded again when seen.
        #[cfg(target_os = "ios")]
        if ios_ui::take_memory_warning() { limn_egui::clear_image_cache(); }
        // A code read by the camera opens the account another device serves.
        #[cfg(target_os = "ios")]
        if let Some(link) = ios_ui::take_opened_link() {
            let commands = self.state.receive_from(&link);
            self.send(commands);
        }
        self.tree.set_reduced_motion(still);
        self.motion.set_reduced_motion(still);
        self.state.motion = !still;
        #[cfg(target_os = "ios")]
        {
            let mut surface_events = Vec::new();
            self.state.mobile_ui.native_routes = false;
            self.state.mobile_ui.native_surfaces = false;
            if let Some(native) = &mut self.native_ui {
                if let Ok(handle) = host.window_handle() {
                    if let RawWindowHandle::UiKit(handle) = handle.as_raw() {
                        // eframe supplies the live main-thread UIView for this window.
                        unsafe { native.update_view(handle.ui_view); }
                        self.state.mobile_ui.native_routes = true;
                        self.state.mobile_ui.native_surfaces = true;
                    }
                }
                self.state.mobile_ui.route = native.current_route().map(|route| (route.name.clone(), route.port_type.clone()));
                match native.take_surface_events::<native_surfaces::Event>() {
                    Ok(events) => surface_events = events,
                    Err(error) => eprintln!("Surco native surface events: {error}"),
                }
            }
            for event in surface_events {
                if matches!(event, native_surfaces::Event::MenuOpen { .. }) {
                    self.tree.handle_event(limn::UiEvent::PointerLeave);
                    self.touch = mobile_input::TouchInput::default();
                }
                let commands = self.surfaces.apply(&mut self.state, event);
                self.send(commands);
                self.stirred = Instant::now();
            }
            self.state.mobile_ui.keyboard_visible = self.native_ui.as_ref().is_some_and(|native| native.keyboard_top(ctx.viewport_rect()).is_some())
                && !(self.state.mobile_ui.native_surfaces && native_surfaces::has_sheet(&self.state));
        }
        // Full screen, AppKit shows the lights in a bar of its own that slides down.
        #[cfg(target_os = "macos")]
        if self.chrome == window::Chrome::Overlaid && !ctx.input(|input| input.viewport().fullscreen.unwrap_or(false)) {
            window::center_lights(host, view::TOP_BAR_HEIGHT);
        }
        let area = if self.state.mobile {
            mobile_canvas(ui, self.script.mobile)
        } else {
            let frame = self.tree.frame();
            window::show(ui, self.state.cinema, self.chrome, view::TOP_BAR_HEIGHT, |point| frame.press_region(limn::Point::new(point.x, point.y)).is_some())
        };
        #[cfg(target_os = "ios")]
        let keyboard_inset = {
            let now = Instant::now();
            let native_sheet = self.state.mobile_ui.native_surfaces && native_surfaces::has_sheet(&self.state);
            let covered = if native_sheet { 0.0 } else { self.native_ui.as_ref().map_or(0.0, |native| native.keyboard_occlusion(ctx.viewport_rect(), area)) };
            if native_sheet { self.keyboard_inset.snap_to(0.0, now); }
            if self.state.mobile_ui.keyboard_visible && covered == 0.0 {
                self.keyboard_inset.snap_to(0.0, now);
            } else {
                self.keyboard_inset.retarget_with_motion(covered, now, Duration::from_millis(220), still);
            }
            if self.keyboard_inset.is_active(now) { ctx.request_repaint(); }
            self.keyboard_inset.value(now)
        };
        #[cfg(not(target_os = "ios"))]
        let keyboard_inset = 0.0;
        let safe_content = ctx.content_rect();
        #[cfg(target_os = "ios")]
        let safe_content = self.native_ui.as_ref()
            .and_then(|native| native.safe_content_rect(area))
            .unwrap_or(safe_content);
        self.state.mobile_ui.content_insets = if self.state.mobile {
            mobile_content_insets(area, safe_content, keyboard_inset)
        } else { Default::default() };
        let window = Size::new(area.width(), area.height());
        self.state.wide = window.width >= view::PANEL_FROM;

        // Ctrl+B folds the library, as it folds a sidebar anywhere.
        if ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::B)) {
            let commands = self.state.update(&UiAction::Clicked { id: "toggle-library".into() });
            self.send(commands);
        }
        while let Ok(event) = self.events.try_recv() {
            // A cover arriving is drawn once; it does not lift the settled frame rate,
            // which a library's worth of them arriving at launch would keep lifted.
            let quiet = matches!(event, backend::Event::Artwork { .. } | backend::Event::ArtworkMissing(_));
            let commands = self.state.apply(event);
            self.send(commands);
            if !quiet { self.stirred = Instant::now(); }
        }
        if self.player.has_changed().unwrap_or(false) {
            let view = self.player.borrow_and_update().clone();
            // Another track, or the same one stopping or starting: more than its going on.
            if view.current != self.state.player.current || view.playing != self.state.player.playing {
                self.stirred = Instant::now();
            }
            let commands = self.state.show(view);
            self.send(commands);
        }
        self.run_script();
        let commands = self.state.search_tick(Instant::now());
        self.send(commands);
        let commands = self.state.onboarding_tick(Instant::now());
        self.send(commands);
        if let Some(due) = self.state.onboarding_search_due() {
            ctx.request_repaint_after(due.saturating_duration_since(Instant::now()));
        }
        let commands = self.state.ask_suggested();
        self.send(commands);
        let commands = self.state.ask_translation();
        self.send(commands);
        if let Some((_, due)) = &self.state.search_due {
            ctx.request_repaint_after(due.saturating_duration_since(Instant::now()));
        }
        // The row of what plays draws its sound: a reading each frame keeps its bars moving.
        let sounding = self.state.player.playing && !still && on_screen();
        self.state.levels = if sounding { surco_audio::meter::levels() } else { Default::default() };

        let commands = self.motion.advance(&mut self.state, window, Instant::now());
        self.send(commands);
        // Escape leaves the whole window to the app again, and the screen follows.
        if self.state.cinema && ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            let commands = self.state.update(&UiAction::Clicked { id: "cinema".into() });
            self.send(commands);
        }
        if self.fullscreen != self.state.cinema {
            self.fullscreen = self.state.cinema;
            // A scripted run keeps the window it was given.
            if self.script.screenshot.is_none() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
            }
        }
        // The footage of what plays goes where its cover is, or over the whole window, for
        // as long as that is on screen.
        // Its own handle on the footage, so the state can be told what is on screen meanwhile.
        match self.state.screening().map(|(name, footage, showing)| (name, footage.clone(), showing)) {
            Some((name, footage, showing)) => {
                let fit = match showing {
                    clip::Showing::Large => {
                        let room = view::cinema_room(window);
                        clip::Fit::Whole { width: view::cinema_picture(&footage.large.0, window).width, room: (room.width, room.height) }
                    }
                    // Full-bleed across the panel, and tall, as Spotify's Canvas.
                    _ => clip::Fit::Cover { width: self.state.right_width, height: view::canvas_height(self.state.right_width, window) },
                };
                if self.clip.as_ref().is_none_or(|screen| screen.key != name) {
                    limn_egui::forget_surface(view::CLIP_SURFACE);
                    limn_egui::forget_surface(view::CLIP_BACKDROP);
                    self.state.clip_ready = None;
                    let wake = ctx.clone();
                    self.clip = Some(clip::Screen::start(name, footage, showing, fit, move || wake.request_repaint_after(backend::SOON)));
                }
                if let Some(screen) = &self.clip {
                    // Behind the shut panel's strip a few frames a second will do.
                    let pace = if self.state.peeks() && !self.state.cinema { PEEK_PACE } else { Duration::ZERO };
                    screen.follow(self.state.player.playing, self.state.player.position, fit, pace);
                    if let Some(frame) = screen.take() {
                        limn_egui::present_surface(&ctx, view::CLIP_SURFACE, frame);
                        // From its first frame on, the video fades in over the cover.
                        if self.state.clip_ready.as_deref() != Some(screen.key.as_str()) { self.state.clip_ready = Some(screen.key.clone()); }
                    }
                    if let Some(glow) = screen.take_backdrop() {
                        limn_egui::present_surface(&ctx, view::CLIP_BACKDROP, glow);
                    }
                }
            }
            None => {
                if self.clip.take().is_some() {
                    limn_egui::forget_surface(view::CLIP_SURFACE);
                    limn_egui::forget_surface(view::CLIP_BACKDROP);
                    self.state.clip_ready = None;
                }
            }
        }
        // Measure this page before using its bars to follow the active lyric.
        let mut input_frame = layout_with_notice(&mut self.state, &mut self.tree, window, &mut self.notice);
        if let Some(frame) = follow_lyrics(&mut self.state, &mut self.tree, window, &mut self.notice) {
            self.stirred = Instant::now();
            input_frame = frame;
        }
        if let Some(frame) = follow_lyric_card(&mut self.state, &mut self.tree, window, &mut self.notice) {
            input_frame = frame;
        }
        // A newly appeared field enters focus_order during layout, before it can take the keyboard.
        if let Some(field) = self.state.focus.take() {
            // On a desktop the search is the top bar's.
            let field = if field == "search" && !self.state.mobile { search_field(&self.state) } else { field.as_str() };
            self.tree.focus(field);
        }
        let input = if self.state.mobile { limn_egui::events_with_touch(&ctx, area.min, &input_frame) }
            else { limn_egui::events(&ctx, area.min) };
        let input = if self.state.mobile {
            let raw = ctx.input(|input| input.events.clone());
            let input = self.touch.translate(&raw, input, area.min, &input_frame,
                state::mobile::is_sheet_handle);
            self.motion.filter(&raw, input, &self.state, mobile_motion::DragArea {
                frame: &input_frame, origin: area.min, size: window,
                scroll: self.tree.scroll_offset("mobile-playing"),
            }, Instant::now())
        } else { input };
        #[cfg(target_os = "ios")]
        let input = if self.state.mobile_ui.native_surfaces && native_surfaces::has_sheet(&self.state) {
            self.tree.handle_event(limn::UiEvent::PointerLeave);
            self.touch = mobile_input::TouchInput::default();
            Vec::new()
        } else { input };
        // Whether this frame changed anything after it was measured for input.
        let mut acted = false;
        for event in input {
            acted = true;
            self.stirred = Instant::now();
            #[cfg(target_os = "ios")]
            if self.state.mobile_ui.native_surfaces && matches!(event, limn::UiEvent::ContextMenu { .. }) { continue; }
            #[cfg(target_os = "ios")]
            if matches!(event, limn::UiEvent::Paste(_)) {
                // egui-winit's iOS clipboard is app-local. The shortcut still
                // represents a user gesture; bind the system read to its field.
                self.state.paste = self.tree.focused_id().map(str::to_owned);
                continue;
            }
            for action in self.tree.handle_event(event) {
                let commands = self.state.update(&action);
                self.send(commands);
            }
        }
        // The field keeps what was typed in it: what the app itself empties or writes there
        // has to be handed to it.
        if self.tree.input_value(search_field(&self.state)).is_some_and(|shown| shown != self.state.query) {
            self.tree.set_input_value(search_field(&self.state), self.state.query.clone());
            acted = true;
        }
        // The recent searches drop down from the field while it holds the keyboard, and stay
        // while one of them does: a press on a row takes the keyboard before its click lands.
        let focused = !self.state.mobile && self.tree.focused_id().is_some_and(state::keeps_recent_searches);
        if focused != self.state.search_focused {
            self.state.search_focused = focused;
            acted = true;
        }
        if !self.state.mobile && self.state.point_at(self.tree.hovered_id()) {
            acted = true;
        }
        // The panel of what plays starts at its top for each song, and its header takes the
        // panel's floor once it is scrolled from there.
        let current = self.state.current().map(|track| track.id.clone());
        if current != self.panel_song {
            self.panel_song = current;
            self.tree.set_scroll_offset("playing", 0.0);
        }
        let scrolled = self.tree.scroll_offset("playing") > 8.0;
        if scrolled != self.state.panel_scrolled {
            self.state.panel_scrolled = scrolled;
            acted = true;
        }
        if let Some(text) = self.state.copy.take() {
            #[cfg(target_os = "ios")]
            {
                let result = self.native_ui.as_mut()
                    .ok_or_else(|| "iOS clipboard is unavailable".to_owned())
                    .and_then(|native| native.copy_text(&text));
                if let Err(error) = result {
                    eprintln!("Surco clipboard: {error}");
                    self.state.mobile_ui.notice = Some(crate::i18n::t!("state-copy-failed").into_owned());
                    self.state.copied = false;
                    self.state.list_copied = None;
                    self.state.list_exported = None;
                }
            }
            ctx.copy_text(text);
        }
        if let Some(url) = self.state.open_url.take() {
            ctx.open_url(egui::OpenUrl::new_tab(url));
        }
        // The later version is in place: it starts, and this one leaves. What this one has is
        // written first: the later one reads it as it starts, before this one has closed.
        if std::mem::take(&mut self.state.restart) {
            if let Some(storage) = host.storage_mut() {
                self.save(storage);
                storage.flush();
            }
            if let Some(program) = update::running() {
                let _ = std::process::Command::new(program).spawn();
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if std::mem::take(&mut self.state.mobile_ui.blur) { self.tree.clear_focus(); acted = true; }
        let now = Instant::now();
        let moving = self.motion.active(now) || self.state.is_sliding(now) || self.state.ambient();
        let commands = self.motion.advance(&mut self.state, window, now);
        acted |= !commands.is_empty();
        self.send(commands);
        // Untouched since it was measured for input, that frame is this one too: building and
        // laying out a long list again costs a frame's worth of time for nothing.
        let frame = if acted || moving { layout_with_notice(&mut self.state, &mut self.tree, window, &mut self.notice) } else { input_frame };
        let frame = follow_track_window(&mut self.state, &mut self.tree, window, &mut self.notice).unwrap_or(frame);
        let frame = follow_page_actions(&mut self.state, &mut self.tree, window, &mut self.notice).unwrap_or(frame);
        let frame = follow_title(&mut self.state, &mut self.tree, window, &mut self.notice).unwrap_or(frame);
        #[cfg(target_os = "ios")]
        let frame = {
            let mut frame = frame;
            if let Some(target) = self.state.paste.take() {
                // A visible Pegar button may have just mounted the Search field.
                if self.state.focus.as_deref() == Some(&target) {
                    self.state.focus = None;
                    self.tree.focus(&target);
                    self.tree.reveal(&target);
                    frame = self.tree.layout(window);
                }
                if mobile_input::can_paste(&self.tree, &frame, &target) {
                    let result = self.native_ui.as_mut()
                        .ok_or_else(|| "iOS clipboard is unavailable".to_owned())
                        .and_then(ios_ui::NativeUi::paste_text);
                    match result {
                        Ok(Some(text)) if !text.is_empty() => {
                            for action in self.tree.handle_event(limn::UiEvent::PasteTo { id: target, text }) {
                                let commands = self.state.update(&action);
                                self.send(commands);
                            }
                            let commands = self.motion.advance(&mut self.state, window, Instant::now());
                            self.send(commands);
                            frame = layout_with_notice(&mut self.state, &mut self.tree, window, &mut self.notice);
                        }
                        Err(error) => {
                            eprintln!("Surco clipboard: {error}");
                            self.state.mobile_ui.notice = Some(crate::i18n::t!("state-paste-failed").into_owned());
                            ctx.request_repaint();
                        }
                        _ => {}
                    }
                }
            }
            frame
        };
        let native_sheet = self.state.mobile_ui.native_surfaces && native_surfaces_present(&self.state);
        if self.state.mobile && !native_sheet {
            mobile_input::update_ime(&ctx, &self.tree, &frame, area.min);
        } else if native_sheet {
            ctx.output_mut(|output| { output.mutable_text_under_cursor = false; output.ime = None; });
        }
        #[cfg(target_os = "ios")]
        if let Some(native) = &mut self.native_ui {
            let slot = frame.access.iter().find(|node| node.id == "ios-route-picker").map(|node| {
                egui::Rect::from_min_size(area.min + egui::vec2(node.rect.x, node.rect.y), egui::vec2(node.rect.width, node.rect.height))
            }).filter(|rect| rect.width() >= 43.9 && rect.height() >= 43.9);
            let visible = self.state.mobile_ui.native_routes && self.state.mobile_ui.connections
                && self.state.sharing.is_none() && self.state.creating.is_none() && self.state.importing.is_none()
                && self.state.confirm.is_none() && !self.state.equalizer_open && !self.state.missed_open;
            native.route_picker(slot.unwrap_or(egui::Rect::ZERO), visible && slot.is_some());
            let payload = self.surfaces.payload(&self.state, &frame, area.min);
            if let Err(error) = native.update_surfaces(&payload) {
                eprintln!("Surco native surfaces: {error}");
                self.state.mobile_ui.native_surfaces = false;
                ctx.request_repaint();
            }
        }
        limn_egui::set_cursor(&ctx, self.tree.cursor());
        if let Some(underlay) = self.motion.underlay(&self.state, window) {
            limn_egui::paint_at(ui, underlay, area.min);
        }
        limn_egui::paint_at(ui, &frame, area.min);
        self.motion.remember(&frame, &self.state, window);

        if let Some(path) = self.script.screenshot.clone() {
            self.capture(&ctx, &path);
        } else if !on_screen() {
            // Behind the lock screen nothing animates; the next frame waits for the return.
        } else if self.state.is_sliding(Instant::now()) || self.motion.active(Instant::now()) {
            ctx.request_repaint();
        } else if let Some(after) = match (self.tree.animation_repaint_after(), (self.state.ambient() || self.state.title_moves()).then_some(AMBIENT_FRAME)) {
            (Some(own), Some(ambient)) => Some(own.min(ambient)),
            (own, ambient) => own.or(ambient),
        } {
            let settled = self.stirred.elapsed() > SETTLED_AFTER;
            let frame = if self.clip.is_some() && self.state.player.playing { SETTLED_FRAME_BESIDE_VIDEO } else { SETTLED_FRAME };
            // egui draws a frame's length ahead of the time it is asked for.
            let ahead = Duration::try_from_secs_f32(ctx.input(|input| input.predicted_dt)).unwrap_or_default();
            // A running animation asks for the very next frame: never throttle it.
            ctx.request_repaint_after(if settled && !after.is_zero() { after.max(frame + ahead) } else { after });
        }
    }
}

/// The id of the field the search is typed in: the phone's own, or the desktop top bar's.
fn search_field(state: &state::State) -> &'static str {
    if state.mobile { "search" } else { "nav:search" }
}

/// Whether anyone can see the window: always on a desktop, not on a phone in the background.
fn on_screen() -> bool {
    #[cfg(target_os = "ios")]
    return ios_ui::on_screen();
    #[cfg(not(target_os = "ios"))]
    true
}

/// The app's own mark, for the window and the task bar where the system takes it from the window.
fn icon() -> egui::IconData {
    // The desktop's mark has its corners rounded and clear: iOS rounds the square one itself.
    let mark = image::load_from_memory(include_bytes!("../assets/icon-rounded.png")).map(|mark| mark.to_rgba8());
    mark.map_or_else(|_| egui::IconData::default(), |mark| egui::IconData { width: mark.width(), height: mark.height(), rgba: mark.into_raw() })
}

fn ios_graphics_limits(supported: &eframe::wgpu::Limits) -> eframe::wgpu::Limits {
    // Metal Simulator exposes 15 shader varyings; WebGPU defaults require 16.
    // Our renderer fits within the downlevel limits: https://github.com/gfx-rs/wgpu/issues/9386
    eframe::wgpu::Limits {
        max_texture_dimension_2d: supported.max_texture_dimension_2d.min(8192),
        ..eframe::wgpu::Limits::downlevel_defaults()
    }
}

fn graphics_configuration() -> eframe::WgpuConfiguration {
    let mut configuration = eframe::NativeOptions::default().wgpu_options;
    if cfg!(target_os = "ios") {
        if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut configuration.wgpu_setup {
            setup.device_descriptor = std::sync::Arc::new(|adapter| eframe::wgpu::DeviceDescriptor {
                label: Some("Surco iOS graphics"),
                required_limits: ios_graphics_limits(&adapter.limits()),
                ..Default::default()
            });
        }
    }
    configuration
}

pub fn run() -> eframe::Result {
    // Where this program is, while the system still says: a later version takes its place.
    update::running();
    limn::set_labels(limn::Labels::SPANISH);
    let mut script = Script { after: Duration::from_millis(900), ..Script::default() };
    #[cfg(not(target_os = "ios"))]
    let mut arguments = std::env::args().skip(1);
    // UIKit may supply its own launch arguments; they are not desktop script commands.
    #[cfg(target_os = "ios")]
    let mut arguments = std::iter::empty::<String>();
    while let Some(argument) = arguments.next() {
        let mut value = || arguments.next().unwrap_or_else(|| {
            eprintln!("{argument} needs a value");
            std::process::exit(2)
        });
        match argument.as_str() {
            "--mobile" => script.mobile = true,
            "--onboarding" => script.onboarding = true,
            "--search" => script.search = Some(value()),
            "--play" => script.play = value().parse().ok(),
            "--seek" => script.seek = value().parse().ok(),
            "--click" | "--slide" | "--type" => {
                let value = value();
                let (what, at) = value.rsplit_once('@').unwrap_or((value.as_str(), "0"));
                let action = match what.split_once('=') {
                    Some((id, fraction)) if argument == "--slide" => {
                        UiAction::SliderChanged { id: id.to_owned(), fraction: fraction.parse().unwrap_or(0.0), committed: true }
                    }
                    Some((id, text)) if argument == "--type" => UiAction::InputChanged { id: id.to_owned(), value: text.to_owned() },
                    _ => UiAction::Clicked { id: what.to_owned() },
                };
                script.actions.push((Duration::from_millis(at.parse().unwrap_or(0)), action));
            }
            "--screenshot" => script.screenshot = Some(PathBuf::from(value())),
            "--after" => script.after = Duration::from_millis(value().parse().unwrap_or(900)),
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    let mobile = cfg!(target_os = "ios") || script.mobile;
    let data = paths::data_dir(APP_ID).map_err(|error| eframe::Error::AppCreation(Box::new(error)))?;
    #[cfg(target_os = "windows")]
    backend::register_with_windows(&data);
    let chrome = window::Chrome::current(mobile);
    let overlaid = chrome == window::Chrome::Overlaid;
    let viewport = egui::ViewportBuilder::default()
        .with_title("Surco")
        .with_decorations(chrome != window::Chrome::Drawn)
        // macOS keeps its lights, over the app's top bar instead of a title bar of their own.
        .with_fullsize_content_view(overlaid)
        .with_titlebar_shown(!overlaid)
        .with_title_shown(!overlaid)
        .with_inner_size(if mobile { [390.0, 844.0] } else { [1180.0, 780.0] })
        .with_min_inner_size(if mobile { [320.0, 480.0] } else { [720.0, 480.0] });
    // iOS shows the bundle's icon; decoding this one would only delay the first frame.
    let viewport = if cfg!(target_os = "ios") { viewport } else { viewport.with_icon(icon()) };
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: graphics_configuration(),
        persist_window: !mobile,
        persistence_path: paths::persistence_path(&data),
        viewport,
        run_and_return: !cfg!(target_os = "ios"),
        ..Default::default()
    };
    eframe::run_native(
        APP_ID,
        options,
        Box::new(move |creation| {
            // Figtree in limn's Ubuntu's place: round and geometric, as Spotify's own face is.
            let figtree = |bytes: &'static [u8]| limn::Font::from_static(bytes).expect("Figtree is a font");
            limn::set_fonts(limn::Fonts::bundled()
                .face(limn::FontFamily::Sans, figtree(include_bytes!("../assets/fonts/Figtree-Regular.ttf")))
                .face(limn::FontFamily::SansMedium, figtree(include_bytes!("../assets/fonts/Figtree-Medium.ttf")))
                .face(limn::FontFamily::SansBold, figtree(include_bytes!("../assets/fonts/Figtree-Bold.ttf"))));
            limn_egui::install_fonts(&creation.egui_ctx);
            #[cfg(target_os = "ios")]
            ios_symbols::install();
            // A page is covers by the dozen: with limn's default of 16 a shelf decodes them again as it scrolls.
            // A phone's covers are drawn at three pixels a point, a megabyte each: it keeps fewer.
            limn_egui::set_image_cache_entries(if cfg!(target_os = "ios") { 96 } else { 256 });
            // A desktop's corners are Spotify's, tighter than limn's: cards at 6, panels at 4. A
            // phone keeps limn's, so its menus and popovers are drawn as the desktop's are; what
            // a finger lights is made round where it is drawn.
            if !mobile {
                let tokens = limn::tokens();
                limn::set_tokens(limn::Tokens { radius: limn::Radius { card: 6.0, panel: 4.0, ..tokens.radius }, tooltip: Some(view::TOOLTIP), ..tokens });
            }
            // Beside the settings; without a place of its own the library lasts the session.
            let data = data.join("offline");
            let (commands, events, player) = backend::spawn(creation.egui_ctx.clone(), data);
            let mut state = State::default();
            state.mobile = mobile;
            state.caption = chrome.inset();
            if let Some(storage) = creation.storage {
                let volume = storage.get_string(VOLUME_KEY).and_then(|volume| volume.parse::<f32>().ok());
                state.volume = volume.filter(|volume| volume.is_finite()).map_or(state.volume, |volume| volume.clamp(0.0, 1.0));
                let width = |key: &str, side: Side| storage.get_string(key).and_then(|width| width.parse::<f32>().ok()).map(|width| side.width(width));
                state.left_width = width(LEFT_KEY, Side::Left).unwrap_or(state.left_width);
                state.right_width = width(RIGHT_KEY, Side::Right).unwrap_or(state.right_width);
                let open = |key: &str| storage.get_string(key).and_then(|open| open.parse::<bool>().ok()).unwrap_or(true);
                state.restore_sides(open(LIBRARY_OPEN_KEY), open(PANEL_OPEN_KEY));
                let collection = storage.get_string(COLLECTION_KEY).and_then(|text| serde_json::from_str(&text).ok());
                state.collection = collection.unwrap_or_default();
                let settings = storage.get_string(SETTINGS_KEY).and_then(|text| serde_json::from_str(&text).ok());
                state.settings = settings.unwrap_or_default();
                let equalizer = storage.get_string(EQUALIZER_KEY).and_then(|text| state::equalizer_from(&text));
                state.equalizer = equalizer.unwrap_or(state.equalizer);
                state.output = storage.get_string(OUTPUT_KEY).filter(|device| !device.is_empty());
                state.profile = storage.get_string(PROFILE_KEY).and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default();
            }
            let session = creation.storage.and_then(|storage| storage.get_string(SESSION_KEY)).and_then(|text| serde_json::from_str(&text).ok());
            i18n::set(state.settings.language.unwrap_or_else(i18n::Language::system));
            #[cfg(target_os = "ios")]
            let native_ui = match ios_ui::NativeUi::new(&creation.egui_ctx) {
                Ok(mut native) => {
                    native.set_route_picker_colors(egui::Color32::WHITE, egui::Color32::WHITE);
                    Some(native)
                }
                Err(error) => { eprintln!("Surco native controls are unavailable: {error}"); None }
            };
            Ok(Box::new(Host {
                tree: UiTree::new(Element::spacer()),
                state,
                commands,
                events,
                player,
                started: false,
                clip: None,
                fullscreen: false,
                chrome,
                opened: Instant::now(),
                stirred: Instant::now(),
                script,
                touch: mobile_input::TouchInput::default(),
                motion: mobile_motion::Motion::new(Instant::now()),
                notice: None,
                #[cfg(target_os = "ios")]
                native_ui,
                #[cfg(target_os = "ios")]
                surfaces: native_surfaces::Controller::default(),
                #[cfg(target_os = "ios")]
                keyboard_inset: limn::ScalarTransition::new(0.0, Instant::now()),
                panel_song: None,
                session,
            }))
        }),
    )
}

/// UIKit's main-thread bootstrap enters the same application as the desktop binary.
/// Winit owns the iOS event loop after this call: https://docs.rs/winit/0.30.13/winit/platform/ios/
#[cfg(target_os = "ios")]
#[no_mangle]
pub extern "C" fn surco_start() -> i32 {
    ios_logging();
    match run() {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("Surco could not start: {error}");
            1
        }
    }
}

#[cfg(target_os = "ios")]
fn ios_logging() {
    struct StderrLogger;
    impl log::Log for StderrLogger {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            // winit warns that iOS lacks window titles, visibility and themes each time
            // eframe asks, several times a frame; only its errors are worth a line.
            let limit = if metadata.target().starts_with("winit") { log::Level::Error } else { log::Level::Warn };
            metadata.level() <= limit
        }

        fn log(&self, record: &log::Record<'_>) {
            if self.enabled(record.metadata()) {
                eprintln!("[{} {}] {}", record.level(), record.target(), record.args());
            }
        }

        fn flush(&self) {}
    }
    static LOGGER: StderrLogger = StderrLogger;
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(log::LevelFilter::Warn);
    }
}

#[cfg(test)]
mod lyrics_viewport_tests {
    use super::*;
    use surco_core::{Track, lyrics::Line};

    #[test]
    fn a_long_song_name_in_the_right_panel_is_measured_once_and_slides_from_its_start() {
        let mut state = State::default();
        let mut view = song("a", 10.0);
        view.current.as_mut().unwrap().title = "Un nombre de canción demasiado largo para el panel de la derecha".into();
        state.show(view);
        let size = Size::new(1400.0, 900.0);
        let mut tree = UiTree::new(view::view(&state, size));
        tree.layout(size);
        let mut seen = None;
        assert!(follow_title(&mut state, &mut tree, size, &mut seen).is_some(), "measured, and laid out again from its start");
        let reach = state.title_reach.as_ref().map(|(_, reach, _)| *reach).unwrap();
        assert!(reach > 40.0, "it reaches past its room: {reach}");
        assert_eq!(tree.horizontal_scroll_offset(view::PANEL_TITLE), 0.0);
        assert!(follow_title(&mut state, &mut tree, size, &mut seen).is_none(), "still at first");

        state.show(song("b", 0.0));
        follow_title(&mut state, &mut tree, size, &mut seen);
        assert!(state.title_reach.as_ref().is_some_and(|(id, reach, _)| id == "b" && *reach == 0.0), "a short name stays put");
        assert!(!state.title_moves());
    }

    #[test]
    fn a_long_name_rests_slides_rests_and_comes_back() {
        let reach = 60.0;
        let at = |seconds: f32| state::marquee(Duration::from_secs_f32(seconds), reach);
        assert_eq!(at(1.0), 0.0);
        assert!((at(3.5) - 30.0).abs() < 0.01, "a second along at 30 a second");
        assert_eq!(at(5.0), reach);
        assert!((at(8.0) - 30.0).abs() < 0.01, "and back");
        assert_eq!(at(9.0 + 1.0), 0.0, "around again");
    }

    fn song(id: &str, position: f64) -> PlayerView {
        PlayerView {
            current: Some(Track { id: id.into(), title: id.into(), artist: "Artist".into(),
                artist_id: None, album: None, album_id: None, artwork_url: None,
                duration_ms: 200_000 }),
            position, duration: 200.0, playing: true, ..PlayerView::default()
        }
    }

    fn lyrics(state: &mut State, id: &str) {
        state.apply(Event::Lyrics { track_id: id.into(), lines: Some((0..40)
            .map(|index| Line { at_ms: 5_000 + index * 1_000, text: format!("Verse {index}") })
            .collect()) });
    }

    fn long_lyrics(state: &mut State) {
        state.apply(Event::Lyrics { track_id: "a".into(), lines: Some((0..21)
            .map(|index| Line { at_ms: index * 1_000,
                text: "La canción continúa en dos renglones mientras atravesamos la ciudad.".into() })
            .collect()) });
    }

    fn assert_active_clear_of_chrome(frame: &limn::Frame, id: &str) {
        let header = frame.regions.iter().find(|region| region.id == "mobile-header-bar").unwrap().rect;
        let footer = frame.regions.iter().find(|region| region.id == "mobile-footer-bar").unwrap().rect;
        let row = frame.regions.iter().find(|region| region.id == id).unwrap().rect;
        assert!(row.height > 50.0, "fixture really wraps: {row:?}");
        assert!(row.y >= header.y + header.height + 20.0 - 0.1, "line clears header and fade: {row:?}");
        assert!(row.y + row.height <= footer.y - 24.0 + 0.1, "whole line clears transport and fade: {row:?}; {footer:?}");
        for y in [row.y + 1.0, row.y + row.height - 1.0] {
            let point = limn::Point::new(row.x + row.width / 2.0, y);
            assert_eq!(frame.press_region(point).unwrap().id, id, "the lyric remains hittable, including its last line");
        }
    }

    /// Where the row of `id` rests below the header, and where the anchor of the body is.
    fn anchor_of(frame: &limn::Frame, id: &str) -> (f32, f32) {
        let rect = |id: &str| frame.regions.iter().find(|region| region.id == id).unwrap().rect;
        let (header, footer) = (rect("mobile-header-bar"), rect("mobile-footer-bar"));
        let top = header.y + header.height;
        (rect(id).y - top, view::lyric_anchor(footer.y - top))
    }

    fn phone(lines: impl Iterator<Item = Line>, position: f64) -> (State, UiTree, Size) {
        let size = Size::new(402.0, 874.0);
        let mut state = State::default();
        state.mobile = true;
        state.page = state::Page::Lyrics;
        state.mobile_ui.content_insets = state::mobile::ContentInsets { top: 62.0, bottom: 34.0,
            ..state::mobile::ContentInsets::default() };
        state.show(song("a", position));
        state.apply(Event::Lyrics { track_id: "a".into(), lines: Some(lines.collect()) });
        let mut tree = UiTree::new(Element::spacer());
        tree.set_reduced_motion(true);
        tree.set_text_measurer(|text, size, _| text.chars().count() as f32 * size * 0.5);
        layout_view(&mut state, &mut tree, size);
        (state, tree, size)
    }

    fn verses(count: u64) -> impl Iterator<Item = Line> {
        (0..count).map(|index| Line { at_ms: index * 1_000, text: format!("Verse {index}") })
    }

    #[test]
    fn the_first_and_the_last_line_both_rest_at_the_anchor_with_lines_on_either_side() {
        for (position, line) in [(0.5, 0), (19.5, 19), (39.5, 39)] {
            let (mut state, mut tree, size) = phone(verses(40), position);
            let mut seen = None;
            assert!(follow_lyrics(&mut state, &mut tree, size, &mut seen).is_some());
            let frame = layout_view(&mut state, &mut tree, size);
            let (at, anchor) = anchor_of(&frame, &format!("lyric:{line}"));
            assert!((at - anchor).abs() < 1.0, "line {line} rests at the anchor: {at} vs {anchor}");
            assert!(anchor > 150.0, "a third down a portrait body, not at its top");
            if line > 0 {
                let (above, _) = anchor_of(&frame, &format!("lyric:{}", line - 1));
                assert!(above > 0.0, "the line just sung stays visible above");
            }
        }
    }

    #[test]
    fn the_player_card_slides_the_line_before_the_sung_one_to_its_top() {
        let (mut state, mut tree, size) = phone(verses(40), 10.5);
        state.page = state::Page::Playing;
        layout_view(&mut state, &mut tree, size);
        let mut seen = None;
        assert!(follow_lyric_card(&mut state, &mut tree, size, &mut seen).is_some());
        let now = std::time::Instant::now();
        let pitch = tree.anchor_offset("lyric-card:1").unwrap() - tree.anchor_offset("lyric-card:0").unwrap();
        assert!(pitch > 20.0 && (state.lyric_card_offset(now) - 9.0 * pitch).abs() < 1.0,
            "verse 9 tops the card: {} by {pitch}", state.lyric_card_offset(now));
        assert!(follow_lyric_card(&mut state, &mut tree, size, &mut seen).is_none(), "and stays while the line holds");
    }

    #[test]
    fn the_desktop_panel_card_slides_the_same_way() {
        let (mut state, mut tree, _) = phone(verses(40), 10.5);
        let size = Size::new(1400.0, 900.0);
        (state.mobile, state.page, state.panel_open) = (false, state::Page::Search, true);
        layout_view(&mut state, &mut tree, size);
        let mut seen = None;
        assert!(follow_lyric_card(&mut state, &mut tree, size, &mut seen).is_some());
        let pitch = tree.anchor_offset("lyric-card:1").unwrap() - tree.anchor_offset("lyric-card:0").unwrap();
        let offset = state.lyric_card_offset(std::time::Instant::now());
        assert!(pitch > 20.0 && (offset - 9.0 * pitch).abs() < 1.0, "verse 9 tops the panel's card: {offset} by {pitch}");
    }

    #[test]
    fn a_new_line_glides_the_rows_after_the_jump_and_a_new_track_or_reduced_motion_does_not() {
        let (mut state, mut tree, size) = phone(verses(40), 10.5);
        state.motion = true;
        let mut seen = None;
        follow_lyrics(&mut state, &mut tree, size, &mut seen);
        layout_view(&mut state, &mut tree, size);
        let before = tree.scroll_offset("lyrics-list");
        state.show(song("a", 11.5));
        follow_lyrics(&mut state, &mut tree, size, &mut seen);
        let jump = tree.scroll_offset("lyrics-list") - before;
        let now = std::time::Instant::now();
        assert!(jump > 20.0, "the list moved on by a row: {jump}");
        assert!((state.lyric_lag(11, now) - jump).abs() < 2.0, "the row is painted where it was");
        assert!(state.is_sliding(now));

        state.show(song("b", 0.0));
        assert_eq!(state.lyric_lag(11, now), 0.0, "a new track starts still");
        state.apply(Event::Lyrics { track_id: "b".into(), lines: Some(verses(40).collect()) });
        state.show(song("b", 5.5));
        follow_lyrics(&mut state, &mut tree, size, &mut seen);
        state.show(song("b", 6.5));
        state.motion = false;
        follow_lyrics(&mut state, &mut tree, size, &mut seen);
        assert_eq!(state.lyric_lag(6, std::time::Instant::now()), 0.0, "reduced motion follows at once");
    }

    #[test]
    fn mobile_follow_keeps_wrapped_active_lyrics_inside_the_real_portrait_and_landscape_body() {
        use state::mobile::ContentInsets;
        for (size, insets) in [
            (Size::new(402.0, 874.0), ContentInsets { top: 62.0, bottom: 34.0, ..ContentInsets::default() }),
            (Size::new(874.0, 402.0), ContentInsets { left: 62.0, right: 34.0, bottom: 21.0, top: 0.0 }),
        ] {
            let mut state = State::default();
            state.mobile = true;
            state.page = state::Page::Lyrics;
            state.mobile_ui.content_insets = insets;
            state.show(song("a", 10.0));
            long_lyrics(&mut state);
            let mut tree = UiTree::new(Element::spacer());
            tree.set_reduced_motion(true);
            tree.set_text_measurer(|text, size, _| text.chars().count() as f32 * size * 0.5);
            layout_view(&mut state, &mut tree, size);
            assert!(update_lyrics_viewport(&mut state, &mut tree));
            // Host runs its input layout and then the final paint layout.
            layout_view(&mut state, &mut tree, size);
            let frame = layout_view(&mut state, &mut tree, size);
            assert_eq!(state.active_line(), Some(10));
            assert_active_clear_of_chrome(&frame, "lyric:10");
            let (at, anchor) = anchor_of(&frame, "lyric:10");
            assert!((at - anchor).abs() < 1.0, "the line rests at the body's anchor: {at} vs {anchor}");
        }
    }

    #[test]
    fn mobile_notice_pauses_follow_without_consuming_the_line_to_reveal_after_close() {
        for next_position in [10.0, 11.0] {
            let size = Size::new(402.0, 874.0);
            let mut state = State::default();
            state.mobile = true;
            state.page = state::Page::Lyrics;
            state.mobile_ui.content_insets = state::mobile::ContentInsets { top: 62.0, bottom: 34.0,
                ..state::mobile::ContentInsets::default() };
            state.show(song("a", 10.0));
            long_lyrics(&mut state);
            let mut tree = UiTree::new(Element::spacer());
            tree.set_reduced_motion(true);
            tree.set_text_measurer(|text, size, _| text.chars().count() as f32 * size * 0.5);
            layout_view(&mut state, &mut tree, size);
            update_lyrics_viewport(&mut state, &mut tree);
            layout_view(&mut state, &mut tree, size);
            layout_view(&mut state, &mut tree, size);
            state.mobile_ui.notice = Some("No pudimos completar esta acción.".into());
            let mut seen = None;
            layout_with_notice(&mut state, &mut tree, size, &mut seen);
            let notice_offset = tree.scroll_offset("lyrics-list");
            state.show(song("a", next_position));
            assert!(!update_lyrics_viewport(&mut state, &mut tree));
            let visible = layout_with_notice(&mut state, &mut tree, size, &mut seen);
            assert_eq!(tree.scroll_offset("lyrics-list"), notice_offset);
            let close = visible.access.iter().find(|node| node.id == "mobile-notice-close").unwrap().rect;
            let point = limn::Point::new(close.x + close.width / 2.0, close.y + close.height / 2.0);
            assert_eq!(visible.press_region(point).unwrap().id, "mobile-notice-close");
            tree.handle_event(limn::UiEvent::PointerDown(point));
            for action in tree.handle_event(limn::UiEvent::PointerUp(point)) { state.update(&action); }
            assert!(state.mobile_ui.notice.is_none());
            assert!(update_lyrics_viewport(&mut state, &mut tree), "closing resumes the current verse even when its position did not change");
            layout_view(&mut state, &mut tree, size);
            let frame = layout_view(&mut state, &mut tree, size);
            assert_active_clear_of_chrome(&frame, &format!("lyric:{}", next_position as usize));
        }
    }

    #[test]
    fn changing_tracks_resets_scrolled_lyrics_before_the_first_line_including_returning_tracks() {
        let size = Size::new(390.0, 500.0);
        let mut state = State::default();
        state.mobile = true;
        state.page = state::Page::Lyrics;
        state.show(song("a", 40.0));
        lyrics(&mut state, "a");
        let mut tree = UiTree::new(view::view(&state, size));
        tree.set_reduced_motion(true);
        tree.layout(size);
        for id in ["b", "a"] {
            tree.scroll_to_end("lyrics-list");
            tree.layout(size);
            assert!(tree.scroll_offset("lyrics-list") > 0.0);
            state.show(song(id, 0.0));
            lyrics(&mut state, id);
            tree.set_root(view::view(&state, size));
            update_lyrics_viewport(&mut state, &mut tree);
            tree.layout(size);
            assert_eq!(tree.scroll_offset("lyrics-list"), 0.0,
                "new track {id} must not inherit the last song's viewport");

            tree.scroll_to_end("lyrics-list");
            tree.layout(size);
            let manual = tree.scroll_offset("lyrics-list");
            state.show(song(id, 1.0));
            tree.set_root(view::view(&state, size));
            update_lyrics_viewport(&mut state, &mut tree);
            tree.layout(size);
            assert_eq!(tree.scroll_offset("lyrics-list"), manual,
                "same-track ticks before its first line preserve manual scrolling");
        }
        state.show(song("a", 20.0));
        tree.set_root(view::view(&state, size));
        assert!(update_lyrics_viewport(&mut state, &mut tree));
        tree.layout(size);
        // Reveal requests set the scroll target after measuring the row; the
        // following paint uses that target, just like Host's input/paint passes.
        let frame = tree.layout(size);
        assert!(frame.regions.iter().any(|region| region.id == "lyric:15"),
            "following the active line still reveals its real row");
    }
}

#[cfg(test)]
mod mobile_geometry_tests {
    use super::*;
    use egui::{Color32, RawInput, Rect, SafeAreaInsets, epaint::MarginF32, pos2, vec2};
    use state::mobile::ContentInsets;

    #[test]
    fn phone_canvas_and_painter_cover_the_real_safe_edges() {
        let ctx = egui::Context::default();
        let full = Rect::from_min_size(pos2(0.0, 0.0), vec2(402.0, 874.0));
        let mut output = ctx.run_ui(RawInput {
            screen_rect: Some(full),
            safe_area_insets: Some(SafeAreaInsets(MarginF32 { top: 62.0, bottom: 34.0, left: 0.0, right: 0.0 })),
            events: vec![egui::Event::PointerMoved(pos2(20.0, 100.0))],
            ..Default::default()
        }, |ui| {
            let safe = Rect::from_min_max(pos2(0.0, 62.0), pos2(402.0, 840.0));
            assert_eq!(ctx.content_rect(), safe);
            // A panel can restrict its painter even though the root Ui spans the viewport.
            ui.set_clip_rect(safe);
            let canvas = mobile_canvas(ui, false);
            assert_eq!(canvas, full);
            assert_eq!(ui.clip_rect(), full);
            assert_eq!(mobile_content_insets(canvas, ctx.content_rect(), 0.0),
                ContentInsets { top: 62.0, bottom: 34.0, left: 0.0, right: 0.0 });
            assert!(limn_egui::events(&ctx, canvas.min).iter().any(|event|
                matches!(event, limn::UiEvent::PointerMove(point) if point.x == 20.0 && point.y == 100.0)),
                "input uses full-canvas coordinates without subtracting the safe top again");
            ui.painter().rect_filled(canvas, 0.0, Color32::BLUE);
        });
        output.textures_delta.clear();
        assert!(output.shapes.iter().any(|shape|
            shape.clip_rect == full && matches!(&shape.shape,
                egui::Shape::Rect(rect) if rect.rect == full && rect.fill == Color32::BLUE)),
            "the background must reach the status bar and home indicator with a full clip");
    }

    #[test]
    fn safe_edges_follow_rotation_and_merge_keyboard_without_adding_it_twice() {
        let full = Rect::from_min_size(pos2(10.0, 20.0), vec2(402.0, 874.0));
        let safe = Rect::from_min_max(pos2(10.0, 82.0), pos2(412.0, 860.0));
        let insets = mobile_content_insets(full, safe, 374.0);
        assert_eq!(insets, ContentInsets { top: 62.0, bottom: 374.0, left: 0.0, right: 0.0 });
        assert_eq!(full.bottom() - insets.bottom, 520.0, "controls end at the keyboard top");
        assert_eq!(mobile_content_insets(full, safe, 0.0).bottom, 34.0,
            "a native sheet owns its keyboard; the canvas keeps only its physical safe edge");

        let landscape = Rect::from_min_size(pos2(0.0, 0.0), vec2(874.0, 402.0));
        let safe = Rect::from_min_max(pos2(62.0, 0.0), pos2(812.0, 381.0));
        assert_eq!(mobile_content_insets(landscape, safe, 0.0),
            ContentInsets { top: 0.0, bottom: 21.0, left: 62.0, right: 62.0 });
        assert_eq!(mobile_content_insets(landscape, safe, 180.0).bottom, 180.0);
    }

    #[test]
    fn malformed_platform_insets_cannot_make_content_dimensions_negative_or_nonfinite() {
        let full = Rect::from_min_size(pos2(0.0, 0.0), vec2(402.0, 874.0));
        let insets = mobile_content_insets(full,
            Rect::from_min_max(pos2(600.0, 900.0), pos2(-20.0, -40.0)), f32::INFINITY);
        assert_eq!(insets, ContentInsets { top: 874.0, bottom: 0.0, left: 402.0, right: 0.0 });
        assert_eq!(mobile_content_insets(full,
            Rect::from_min_max(pos2(f32::NAN, f32::NAN), pos2(f32::NAN, f32::NAN)), f32::NAN),
            ContentInsets::default());
    }
}

#[cfg(test)]
mod graphics_tests {
    use super::ios_graphics_limits;
    use eframe::wgpu::Limits;

    #[test]
    fn native_graphics_keeps_default_surface_latency() {
        assert_eq!(super::graphics_configuration().surface, eframe::NativeOptions::default().wgpu_options.surface);
    }

    #[test]
    fn ios_graphics_supports_simulator_limits_and_available_texture_sizes() {
        // Metal Simulator exposes Apple2-like capabilities: 15 shader varyings, below WebGPU's 16.
        let supported = Limits { max_inter_stage_shader_variables: 15, ..Limits::default() };
        assert!(!Limits::default().check_limits(&supported));
        assert!(ios_graphics_limits(&supported).check_limits(&supported));
        for (available, requested) in [(4096, 4096), (8192, 8192), (16384, 8192)] {
            let supported = Limits { max_texture_dimension_2d: available, ..supported.clone() };
            let limits = ios_graphics_limits(&supported);
            assert_eq!(limits.max_texture_dimension_2d, requested);
            assert!(limits.check_limits(&supported));
        }
    }
}

#[cfg(test)]
mod track_window_tests {
    use super::*;
    use surco_core::Track;

    fn liked(mobile: bool, count: usize) -> (State, UiTree, Size) {
        let size = if mobile { Size::new(402.0, 874.0) } else { Size::new(1180.0, 780.0) };
        let mut state = State::default();
        state.mobile = mobile;
        state.collection.liked = (0..count).map(|index| Track { id: format!("t{index}"), title: format!("Song {index}"),
            artist: "Artist".into(), artist_id: None, album: Some("Album".into()), album_id: None,
            artwork_url: None, duration_ms: 1000 }).collect();
        state.page = state::Page::Liked;
        let mut tree = UiTree::new(Element::spacer());
        tree.set_reduced_motion(true);
        tree.set_text_measurer(|text, size, _| text.chars().count() as f32 * size * 0.5);
        layout_view(&mut state, &mut tree, size);
        (state, tree, size)
    }

    fn rows(frame: &limn::Frame) -> Vec<(usize, f32)> {
        frame.access.iter().filter_map(|node| Some((node.id.strip_prefix("page-track:")?.parse().ok()?, node.rect.y))).collect()
    }

    #[test]
    fn the_window_is_whole_for_short_lists_and_blocks_around_the_view_for_long_ones() {
        assert_eq!(view::track_range(100, 60.0, Some((-6000.0, 800.0))), 0..100, "short lists are built whole");
        assert_eq!(view::track_range(1000, 60.0, None), 0..1000, "an unmeasured list is built whole");
        let range = view::track_range(1000, 60.0, Some((-6000.0, 800.0)));
        assert!(range.start <= 86 && range.end >= 114 && range.len() < 80, "{range:?}");
        assert_eq!(range.start % 16, 0);
        assert_eq!(view::track_range(1000, 60.0, Some((-600_000.0, 800.0))), 1000..1000);
    }

    #[test]
    fn a_long_list_builds_its_rows_in_view_where_the_whole_list_would_put_them() {
        for mobile in [false, true] {
            let (mut state, mut tree, size) = liked(mobile, 1000);
            for offset in [0.0, 30_000.0, 1_000_000.0] {
                state.track_window = None;
                tree.set_scroll_offset("liked", offset);
                let expected = rows(&layout_view(&mut state, &mut tree, size));
                let extent = tree.scroll_offset("liked");
                assert!(!expected.is_empty());
                // The window follows the list until it holds what is in view.
                let mut seen = None;
                let mut passes = 0;
                while follow_track_window(&mut state, &mut tree, size, &mut seen).is_some() {
                    passes += 1;
                    assert!(passes < 4, "the window settles");
                }
                assert!(state.track_window.is_some());
                tree.set_scroll_offset("liked", offset);
                let shown = layout_view(&mut state, &mut tree, size);
                assert_eq!(tree.scroll_offset("liked"), extent, "the list is as tall either way (mobile {mobile}, {offset})");
                assert_eq!(rows(&shown), expected, "rows stand where the whole list puts them (mobile {mobile}, {offset})");
            }
        }
    }

    #[test]
    fn the_pages_bar_takes_over_once_its_actions_scroll_away_and_leaves_with_them() {
        let (mut state, mut tree, size) = liked(false, 60);
        let mut seen = None;
        let bar = |frame: &limn::Frame| frame.access.iter().find(|node| node.id == "stuck-play").map(|node| node.rect.y);
        assert!(follow_page_actions(&mut state, &mut tree, size, &mut seen).is_none(), "at the top the actions are in view");
        assert!(!state.page_stuck);
        tree.set_scroll_offset("liked", 400.0);
        layout_view(&mut state, &mut tree, size);
        let frame = follow_page_actions(&mut state, &mut tree, size, &mut seen).expect("the bar comes");
        assert!(state.page_stuck);
        let top = bar(&frame).expect("its play button is there");
        assert!(top < view::TOP_BAR_HEIGHT + 2.0 * 8.0 + 64.0, "at the top of the page: {top}");
        assert_eq!(click(&mut state, "stuck-play"), click(&mut state, "page-play"));
        tree.set_scroll_offset("liked", 0.0);
        layout_view(&mut state, &mut tree, size);
        let frame = follow_page_actions(&mut state, &mut tree, size, &mut seen).expect("the bar goes");
        assert!(!state.page_stuck && bar(&frame).is_none());
        for offset in (0..900).step_by(7) {
            tree.set_scroll_offset("liked", offset as f32);
            layout_view(&mut state, &mut tree, size);
            follow_page_actions(&mut state, &mut tree, size, &mut seen);
            assert!(follow_page_actions(&mut state, &mut tree, size, &mut seen).is_none(), "the bar settles at {offset}");
        }
    }

    fn click(state: &mut State, id: &str) -> Vec<backend::Command> {
        state.update(&limn::UiAction::Clicked { id: id.into() })
    }

    #[test]
    fn scrolling_a_long_list_closes_nothing_and_a_removed_song_still_closes() {
        let (mut state, mut tree, size) = liked(false, 300);
        let mut seen = None;
        // As a frame does: lay out, then build the rows that came into view.
        let settle = |state: &mut State, tree: &mut UiTree, seen: &mut Option<String>| {
            let mut frame = layout_view(state, tree, size);
            while let Some(next) = follow_track_window(state, tree, size, seen) { frame = next; }
            frame
        };
        // The page is in place before motion is on, so only the rows can move.
        settle(&mut state, &mut tree, &mut seen);
        tree.set_reduced_motion(false);
        settle(&mut state, &mut tree, &mut seen);
        tree.set_scroll_offset("liked", 6_000.0);
        let scrolled = rows(&settle(&mut state, &mut tree, &mut seen));
        let pitch = view::track_pitch(&state);
        // The first row in view is cut by the top of the area; the rest stand a row apart.
        assert!(scrolled[1..].windows(2).all(|pair| pair[1].0 == pair[0].0 + 1 && (pair[1].1 - pair[0].1 - pitch).abs() < 0.5),
            "rows scrolled out of the window leave no closing slot: {scrolled:?}");
        // Unliking a song in view: the next one takes its place only once its slot closes.
        let (gone, at) = scrolled[3];
        let next = scrolled[4].1;
        state.collection.liked.remove(gone);
        let closing = rows(&settle(&mut state, &mut tree, &mut seen));
        let (_, moved) = closing.iter().copied().find(|(place, _)| *place == gone).unwrap();
        assert!((moved - next).abs() < 1.0, "the slot is still open: {moved} vs {next}");
        std::thread::sleep(Duration::from_millis(900));
        let closed = rows(&settle(&mut state, &mut tree, &mut seen));
        let (_, moved) = closed.iter().copied().find(|(place, _)| *place == gone).unwrap();
        assert!((moved - at).abs() < 1.0, "and then it closes: {moved} vs {at}");
    }
}

