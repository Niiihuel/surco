//! dnmusic for the desktop.
//!
//! `cargo run -p dnmusic` opens the player. For checking a build without touching it:
//! `--search <text>` submits a search at startup, `--play <n>` plays the n-th result when
//! it arrives, `--seek <s>` jumps there once it sounds, `--click <id>@<ms>` presses a control
//! and `--slide <id>=<fraction>@<ms>` sets a slider that long after the window opened (give them
//! in order), and `--screenshot <file.png>` saves
//! the window after `--after <ms>` and exits.

mod backend;
mod library;
mod media_keys;
mod player;
mod state;
mod view;

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
    opened: Instant,
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

/// Its own id, and with it its own settings folder: the Electron app is "dnmusic".
const APP_ID: &str = "dnmusic-native";
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
            self.send(vec![Command::Volume(self.state.volume), Command::Equalizer(self.state.equalizer), Command::Autoplay(self.state.settings.autoplay), Command::Crossfade(f64::from(self.state.settings.crossfade)), Command::Quality(self.state.settings.quality.bps())]);
            if self.state.output.is_some() {
                self.send(vec![Command::Output(self.state.output.clone())]);
            }
            let covers = self.state.collection_covers();
            self.send(covers);
            let recommendations = self.state.ask_recommendations();
            self.send(recommendations);
        }
        limn_egui::normalize_keyboard_input(&ctx);
        // Motion stays on for a scripted run too: whoever looks at its window sees the app as
        // it is, and the screenshot is taken once things have settled.
        let still = self.state.settings.reduce_motion;
        self.tree.set_reduced_motion(still);
        self.state.motion = !still;
        let area = ui.max_rect();
        let window = Size::new(area.width(), area.height());

        // Ctrl+B folds the library, as it folds a sidebar anywhere.
        if ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::B)) {
            let commands = self.state.update(&UiAction::Clicked { id: "toggle-library".into() });
            self.send(commands);
        }
        while let Ok(event) = self.events.try_recv() {
            let commands = self.state.apply(event);
            self.send(commands);
        }
        if self.player.has_changed().unwrap_or(false) {
            let view = self.player.borrow_and_update().clone();
            let commands = self.state.show(view);
            self.send(commands);
        }
        self.run_script();
        // The row of what plays draws its sound: a reading each frame keeps its bars moving.
        let sounding = self.state.player.playing && !self.state.settings.reduce_motion;
        self.state.levels = if sounding { dnm_audio::meter::levels() } else { Default::default() };

        self.tree.set_root(view::view(&self.state, window));
        // The line being sung is kept in the middle of its list.
        if let Some(line) = self.state.line_to_reveal() {
            self.tree.center(&line);
        }
        self.tree.layout(window);
        for event in limn_egui::events(&ctx, area.min) {
            for action in self.tree.handle_event(event) {
                let commands = self.state.update(&action);
                self.send(commands);
            }
        }
        if let Some(text) = self.state.copy.take() {
            ctx.copy_text(text);
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
            ctx.request_repaint_after(after);
        }
    }
}

fn main() -> eframe::Result {
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
            .with_title("dnmusic")
            .with_inner_size([1180.0, 780.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        APP_ID,
        options,
        Box::new(move |creation| {
            limn_egui::install_fonts(&creation.egui_ctx);
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
                opened: Instant::now(),
                script,
            }))
        }),
    )
}
