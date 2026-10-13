//! Surco: a music player for the desktop.
//!
//! `cargo run -p surco` opens the player. For checking a build without touching it:
//! `--search <text>` submits a search at startup, `--play <n>` plays the n-th result when
//! it arrives, `--seek <s>` jumps there once it sounds, `--click <id>@<ms>` presses a control
//! and `--slide <id>=<fraction>@<ms>` sets a slider that long after the window opened (give them
//! in order), and `--screenshot <file.png>` saves
//! the window after `--after <ms>` and exits.

// A release build on Windows opens its window alone, without a console behind it.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod backend;
mod clip;
mod import;
mod library;
mod media_keys;
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
}

/// What the command line asked the window to do by itself.
#[derive(Default)]
struct Script {
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
        #[cfg(not(target_os = "windows"))]
        let area = ui.max_rect();
        #[cfg(target_os = "windows")]
        let area = window::show(ui, self.state.cinema);
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
        // A field that has just appeared takes the keyboard.
        if let Some(field) = self.state.focus.take() {
            self.tree.focus(&field);
        }
        // The line being sung is kept in the middle of its list.
        if let Some(line) = self.state.line_to_reveal() {
            self.tree.center(&line);
            self.stirred = Instant::now();
        }
        self.tree.layout(window);
        for event in limn_egui::events(&ctx, area.min) {
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
        self.tree.set_root(view::view(&self.state, window));
        let frame = self.tree.layout(window);
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

fn main() -> eframe::Result {
    // Where this program is, while the system still says: a later version takes its place.
    update::running();
    limn::set_labels(limn::Labels::SPANISH);
    let mut script = Script { after: Duration::from_millis(900), ..Script::default() };
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        let mut value = || arguments.next().unwrap_or_else(|| {
            eprintln!("{argument} needs a value");
            std::process::exit(2)
        });
        match argument.as_str() {
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
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_title("Surco")
            .with_decorations(!cfg!(target_os = "windows"))
            .with_icon(icon())
            .with_inner_size([1180.0, 780.0])
            .with_min_inner_size([720.0, 480.0]),
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
            let data = eframe::storage_dir(APP_ID).unwrap_or_else(std::env::temp_dir).join("offline");
            let (commands, events, player) = backend::spawn(creation.egui_ctx.clone(), data);
            let mut state = State::default();
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
            }))
        }),
    )
}
