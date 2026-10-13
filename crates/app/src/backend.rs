//! The contract between the window and everything that takes time: the catalog, the network
//! and the audio engine live on the other side of these two channels.

use dnm_core::Track;
use limn::RasterImage;

/// What the window asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Search(String),
    /// Start this track from the beginning, replacing whatever plays.
    Play(Track),
    Pause,
    Resume,
    /// Seconds from the start of the current track.
    Seek(f64),
    /// 0 to 1.
    Volume(f32),
    FetchArtwork(String),
}

/// What comes back. Every variant names what it answers, because answers arrive late and
/// out of order: a result for a query the user already replaced must be ignored.
#[derive(Clone, Debug)]
pub enum Event {
    SearchResults { query: String, tracks: Vec<Track> },
    SearchFailed { query: String, message: String },
    Artwork { url: String, image: RasterImage },
    Position { track_id: String, seconds: f64, duration: f64 },
    Ended { track_id: String },
    PlaybackFailed { track_id: String, message: String },
}

use std::sync::mpsc::{channel, Receiver, Sender};

use eframe::egui;

/// Starts the worker and returns its two ends. The worker wakes the window with
/// `request_repaint` whenever it has something to say, so the window can sleep.
pub fn spawn(ctx: egui::Context) -> (Sender<Command>, Receiver<Event>) {
    let (commands, inbox) = channel::<Command>();
    let (outbox, events) = channel::<Event>();
    std::thread::Builder::new()
        .name("backend".into())
        .spawn(move || {
            let reply = |event: Event| {
                // The window is gone when this fails; the loop ends with the channel.
                let _ = outbox.send(event);
                ctx.request_repaint();
            };
            for command in inbox {
                match command {
                    Command::Search(query) => {
                        reply(Event::SearchFailed { query, message: "El catálogo todavía no está conectado.".into() })
                    }
                    Command::Play(track) => reply(Event::PlaybackFailed {
                        track_id: track.id,
                        message: "El motor de audio todavía no está conectado.".into(),
                    }),
                    Command::Pause | Command::Resume | Command::Seek(_) | Command::Volume(_) | Command::FetchArtwork(_) => {}
                }
            }
        })
        .expect("the backend thread starts");
    (commands, events)
}
