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
mod import;
#[cfg(any(target_os = "ios", test))]
mod ios_media;
mod library;
mod media_keys;
mod mobile_input;
#[cfg(any(target_os = "ios", test))]
mod ios_ui;
mod paths;
mod player;
mod state;
mod update;
mod view;
#[cfg(any(target_os = "windows", test))]
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
    /// The shelf an arrow is carrying along, and where it is on its way.
    glide: Option<(String, limn::ScalarSpring)>,
    opened: Instant,
    /// When something other than the song going on last happened to the window.
    stirred: Instant,
    script: Script,
    touch: mobile_input::TouchInput,
    #[cfg(target_os = "ios")]
    native_ui: Option<ios_ui::NativeUi>,
    #[cfg(target_os = "ios")]
    keyboard_inset: limn::ScalarTransition,
}

/// What the command line asked the window to do by itself.
#[derive(Default)]
struct Script {
    mobile: bool,
    search: Option<String>,
    play: Option<usize>,
    seek: Option<f64>,
    actions: Vec<(Duration, UiAction)>,
    screenshot: Option<PathBuf>,
    after: Duration,
    requested: bool,
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
const SETTLED_FRAME: Duration = Duration::from_millis(33);
const SETTLED_FRAME_BESIDE_VIDEO: Duration = Duration::from_millis(100);

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

impl eframe::App for Host {
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
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !self.started {
            self.started = true;
            limn_egui::attach_fonts(&mut self.tree, &ctx);
            // The engine starts at full volume and flat; the window owns both settings.
            self.send(vec![Command::Volume(self.state.volume), Command::Equalizer(self.state.equalizer), Command::Autoplay(self.state.settings.autoplay), Command::Crossfade(f64::from(self.state.settings.crossfade)), Command::Quality(self.state.settings.quality.bps()), Command::Mono(self.state.settings.mono), Command::Normalize(self.state.settings.normalize)]);
            if self.state.output.is_some() {
                self.send(vec![Command::Output(self.state.output.clone())]);
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
        self.tree.set_reduced_motion(still);
        self.state.motion = !still;
        #[cfg(target_os = "ios")]
        if let Some(native) = &mut self.native_ui {
            self.state.mobile_ui.native_routes = false;
            if let Ok(handle) = _frame.window_handle() {
                if let RawWindowHandle::UiKit(handle) = handle.as_raw() {
                    // eframe supplies the live main-thread UIView for this window.
                    unsafe { native.update_view(handle.ui_view); }
                    self.state.mobile_ui.native_routes = true;
                }
            }
            self.state.mobile_ui.route = native.current_route().map(|route| (route.name.clone(), route.port_type.clone()));
            self.state.mobile_ui.keyboard_visible = native.keyboard_top(ctx.viewport_rect()).is_some();
        }
        #[cfg(not(target_os = "windows"))]
        let area = if self.state.mobile {
            // Keep phone controls inside the platform's notch and home-indicator insets.
            let safe = ui.max_rect().intersect(ctx.content_rect());
            if self.script.mobile {
                // A tiling desktop compositor can ignore the preview window's requested size.
                // Keep the phone canvas at phone dimensions inside that larger surface.
                egui::Rect::from_center_size(safe.center(), egui::vec2(safe.width().min(390.0), safe.height().min(844.0)))
            } else { safe }
        } else {
            ui.max_rect()
        };
        #[cfg(target_os = "windows")]
        let area = window::show(ui, self.state.cinema);
        #[cfg(target_os = "ios")]
        let area = {
            let now = Instant::now();
            let covered = self.native_ui.as_ref().map_or(0.0, |native| native.keyboard_occlusion(ctx.viewport_rect(), area));
            if self.state.mobile_ui.keyboard_visible && covered == 0.0 {
                self.keyboard_inset.snap_to(0.0, now);
            } else {
                self.keyboard_inset.retarget_with_motion(covered, now, Duration::from_millis(220), still);
            }
            if self.keyboard_inset.is_active(now) { ctx.request_repaint(); }
            egui::Rect::from_min_max(area.min, egui::pos2(area.max.x, (area.max.y - self.keyboard_inset.value(now)).max(area.min.y + 44.0)))
        };
        let window = Size::new(area.width(), area.height());

        // Ctrl+B folds the library, as it folds a sidebar anywhere.
        if ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::B)) {
            let commands = self.state.update(&UiAction::Clicked { id: "toggle-library".into() });
            self.send(commands);
        }
        while let Ok(event) = self.events.try_recv() {
            let commands = self.state.apply(event);
            self.send(commands);
            self.stirred = Instant::now();
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
        let commands = self.state.mobile_tick(Instant::now());
        self.send(commands);
        if let Some((_, due)) = &self.state.mobile_ui.pending {
            ctx.request_repaint_after(due.saturating_duration_since(Instant::now()));
        }
        // The row of what plays draws its sound: a reading each frame keeps its bars moving.
        let sounding = self.state.player.playing && !self.state.settings.reduce_motion;
        self.state.levels = if sounding { surco_audio::meter::levels() } else { Default::default() };

        self.tree.set_root(view::view(&self.state, window));
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
        match self.state.screening() {
            Some((name, footage, showing)) => {
                let fit = match showing {
                    clip::Showing::Large => {
                        let room = view::cinema_room(window);
                        clip::Fit::Whole { width: view::cinema_picture(&footage.large.0, window).width, room: (room.width, room.height) }
                    }
                    _ => clip::Fit::Square(self.state.right_width - 32.0),
                };
                if self.clip.as_ref().is_none_or(|screen| screen.key != name) {
                    limn_egui::forget_surface(view::CLIP_SURFACE);
                    limn_egui::forget_surface(view::CLIP_BACKDROP);
                    let wake = ctx.clone();
                    self.clip = Some(clip::Screen::start(name, footage.clone(), showing, fit, move || wake.request_repaint_after(backend::SOON)));
                }
                if let Some(screen) = &self.clip {
                    screen.follow(self.state.player.playing, self.state.player.position, fit);
                    if let Some(frame) = screen.take() {
                        limn_egui::present_surface(&ctx, view::CLIP_SURFACE, frame);
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
                }
            }
        }
        // An arrow of a shelf carries it a few tiles along, on a spring.
        if let Some((shelf, way)) = self.state.shelf_push.take() {
            let now = Instant::now();
            let from = self.glide.as_ref().filter(|(gliding, _)| *gliding == shelf).map_or_else(|| self.tree.horizontal_scroll_offset(&shelf), |(_, spring)| spring.target());
            let to = (from + way * view::SHELF_STEP).max(0.0);
            let mut spring = limn::ScalarSpring::new(if self.state.settings.reduce_motion { to } else { self.tree.horizontal_scroll_offset(&shelf) }, now)
                .motion(limn::SpringMotion::settling_in(Duration::from_millis(u64::from(limn::tokens().motion.move_ms) * 2)));
            spring.retarget(to, now);
            self.glide = Some((shelf, spring));
        }
        if let Some((shelf, spring)) = &self.glide {
            let now = Instant::now();
            self.tree.set_horizontal_scroll_offset(shelf, spring.value(now));
            if spring.is_active(now) {
                ctx.request_repaint();
            } else {
                self.glide = None;
            }
        }
        // The line being sung is kept in the middle of its list.
        if let Some(line) = self.state.line_to_reveal() {
            self.tree.center(&line);
            self.stirred = Instant::now();
        }
        let input_frame = self.tree.layout(window);
        // A newly appeared field enters focus_order during layout, before it can take the keyboard.
        if let Some(field) = self.state.focus.take() {
            self.tree.focus(&field);
        }
        let input = if self.state.mobile { limn_egui::events_with_touch(&ctx, area.min, &input_frame) }
            else { limn_egui::events(&ctx, area.min) };
        let input = if self.state.mobile {
            let raw = ctx.input(|input| input.events.clone());
            self.touch.translate(&raw, input, area.min, &input_frame,
                state::mobile::is_sheet_handle)
        } else { input };
        for event in input {
            self.stirred = Instant::now();
            for action in self.tree.handle_event(event) {
                let commands = self.state.update(&action);
                self.send(commands);
            }
        }
        // The field keeps what was typed in it: what the app itself empties or writes there
        // has to be handed to it.
        if self.tree.input_value("search").is_some_and(|shown| shown != self.state.query) {
            self.tree.set_input_value("search", self.state.query.clone());
        }
        if let Some(text) = self.state.copy.take() {
            ctx.copy_text(text);
        }
        if let Some(url) = self.state.open_url.take() {
            ctx.open_url(egui::OpenUrl::new_tab(url));
        }
        // The later version is in place: it starts, and this one leaves.
        if std::mem::take(&mut self.state.restart) {
            if let Some(program) = update::running() {
                let _ = std::process::Command::new(program).spawn();
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if std::mem::take(&mut self.state.mobile_ui.blur) { self.tree.clear_focus(); }
        self.tree.set_root(view::view(&self.state, window));
        let frame = self.tree.layout(window);
        if self.state.mobile {
            mobile_input::update_ime(&ctx, &self.tree, &frame, area.min);
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
        }
        limn_egui::set_cursor(&ctx, self.tree.cursor());
        limn_egui::paint_at(ui, &frame, area.min);

        if let Some(path) = self.script.screenshot.clone() {
            self.capture(&ctx, &path);
        } else if self.state.is_sliding(Instant::now()) {
            ctx.request_repaint();
        } else if let Some(after) = self.tree.animation_repaint_after() {
            let settled = self.stirred.elapsed() > SETTLED_AFTER && self.glide.is_none();
            let frame = if self.clip.is_some() && self.state.player.playing { SETTLED_FRAME_BESIDE_VIDEO } else { SETTLED_FRAME };
            // egui draws a frame's length ahead of the time it is asked for.
            let ahead = Duration::try_from_secs_f32(ctx.input(|input| input.predicted_dt)).unwrap_or_default();
            ctx.request_repaint_after(if settled { after.max(frame + ahead) } else { after });
        }
    }
}

/// The app's own mark, for the window and the task bar where the system takes it from the window.
fn icon() -> egui::IconData {
    let mark = image::load_from_memory(include_bytes!("../assets/icon.png")).map(|mark| mark.to_rgba8());
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
            "--search" => script.search = Some(value()),
            "--play" => script.play = value().parse().ok(),
            "--seek" => script.seek = value().parse().ok(),
            "--click" | "--slide" => {
                let value = value();
                let (what, at) = value.rsplit_once('@').unwrap_or((value.as_str(), "0"));
                let action = match what.split_once('=') {
                    Some((id, fraction)) if argument == "--slide" => {
                        UiAction::SliderChanged { id: id.to_owned(), fraction: fraction.parse().unwrap_or(0.0), committed: true }
                    }
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
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: graphics_configuration(),
        persist_window: !mobile,
        persistence_path: paths::persistence_path(&data),
        viewport: egui::ViewportBuilder::default()
            .with_title("Surco")
            .with_decorations(!cfg!(target_os = "windows"))
            .with_icon(icon())
            .with_inner_size(if mobile { [390.0, 844.0] } else { [1180.0, 780.0] })
            .with_min_inner_size(if mobile { [320.0, 480.0] } else { [720.0, 480.0] }),
        run_and_return: !cfg!(target_os = "ios"),
        ..Default::default()
    };
    eframe::run_native(
        APP_ID,
        options,
        Box::new(move |creation| {
            limn_egui::install_fonts(&creation.egui_ctx);
            // A page is covers by the dozen: with limn's default of 16 a shelf decodes them again as it scrolls.
            limn_egui::set_image_cache_entries(256);
            // Beside the settings; without a place of its own the library lasts the session.
            let data = data.join("offline");
            let (commands, events, player) = backend::spawn(creation.egui_ctx.clone(), data);
            let mut state = State::default();
            state.mobile = mobile;
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
            }
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
                glide: None,
                opened: Instant::now(),
                stirred: Instant::now(),
                script,
                touch: mobile_input::TouchInput::default(),
                #[cfg(target_os = "ios")]
                native_ui,
                #[cfg(target_os = "ios")]
                keyboard_inset: limn::ScalarTransition::new(0.0, Instant::now()),
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
            metadata.level() <= log::Level::Warn
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
