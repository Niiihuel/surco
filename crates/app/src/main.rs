//! dnmusic for the desktop.
//!
//! `cargo run -p dnmusic` opens the player. For checking a build without touching it:
//! `--search <text>` submits a search at startup, `--play <n>` plays the n-th result when
//! it arrives, and `--screenshot <file.png>` saves the window after `--after <ms>` and exits.

mod backend;
mod state;
mod view;

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use eframe::egui;
use limn::{Element, Size, UiAction, UiTree};

use backend::{Command, Event};
use state::State;

struct Host {
    tree: UiTree,
    state: State,
    commands: Sender<Command>,
    events: Receiver<Event>,
    started: bool,
    opened: Instant,
    script: Script,
}

/// What the command line asked the window to do by itself.
#[derive(Default)]
struct Script {
    search: Option<String>,
    play: Option<usize>,
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
            if index < self.state.results.len() {
                self.script.play = None;
                let commands = self.state.update(&UiAction::Clicked { id: format!("track:{index}") });
                self.send(commands);
            }
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

impl eframe::App for Host {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !self.started {
            self.started = true;
            limn_egui::attach_fonts(&mut self.tree, &ctx);
        }
        limn_egui::normalize_keyboard_input(&ctx);
        self.tree.set_reduced_motion(self.script.screenshot.is_some());
        let area = ui.max_rect();
        let window = Size::new(area.width(), area.height());

        while let Ok(event) = self.events.try_recv() {
            let commands = self.state.apply(event);
            self.send(commands);
        }
        self.run_script();

        self.tree.set_root(view::view(&self.state, window));
        self.tree.layout(window);
        for event in limn_egui::events(&ctx, area.min) {
            for action in self.tree.handle_event(event) {
                let commands = self.state.update(&action);
                self.send(commands);
            }
        }
        self.tree.set_root(view::view(&self.state, window));
        let frame = self.tree.layout(window);
        limn_egui::paint_at(ui, &frame, area.min);

        if let Some(path) = self.script.screenshot.clone() {
            self.capture(&ctx, &path);
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
        viewport: egui::ViewportBuilder::default().with_inner_size([1180.0, 780.0]).with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "dnmusic",
        options,
        Box::new(move |creation| {
            limn_egui::install_fonts(&creation.egui_ctx);
            let (commands, events) = backend::spawn(creation.egui_ctx.clone());
            Ok(Box::new(Host {
                tree: UiTree::new(Element::spacer()),
                state: State::default(),
                commands,
                events,
                started: false,
                opened: Instant::now(),
                script,
            }))
        }),
    )
}
