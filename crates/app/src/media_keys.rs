//! System media keys and Now Playing, independent of the app window's repaint loop.
// shortcut: MPRIS on Linux only; Windows needs the window's handle and macOS its main thread, wire each when there is a build of it to test.

use crate::backend::Command;
use crate::player::PlayerView;

/// What the desktop asks of the player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Key {
    Toggle,
    Play,
    Pause,
    Next,
    Previous,
    /// Seconds from the start of the track.
    Seek(f64),
    /// Seconds from where it is, back when negative.
    SeekBy(f64),
}

pub enum MediaEvent {
    Key(Key),
    #[cfg(target_os = "ios")]
    Session(crate::ios_media::SessionEvent),
    #[cfg(any(target_os = "ios", target_os = "android"))]
    Artwork { track_id: String, url: String },
}

impl Key {
    /// The command it stands for, given what the player is doing.
    pub fn command(self, view: &PlayerView) -> Option<Command> {
        match self {
            Key::Toggle => Some(Command::PlayPause),
            Key::Play => (!view.playing).then_some(Command::PlayPause),
            Key::Pause => view.playing.then_some(Command::PlayPause),
            Key::Next => Some(Command::Next),
            Key::Previous => Some(Command::Previous),
            Key::Seek(seconds) => Some(Command::Seek(seconds.clamp(0.0, view.duration))),
            Key::SeekBy(seconds) => Some(Command::Seek((view.position + seconds).clamp(0.0, view.duration))),
        }
    }
}

pub struct MediaKeys {
    #[cfg(target_os = "linux")]
    controls: Option<souvlaki::MediaControls>,
    #[cfg(target_os = "ios")]
    ios: crate::ios_media::Controls,
    #[cfg(target_os = "android")]
    android: crate::android_media::Controls,
    /// What the desktop was last told: the track, whether it plays, and the second it is at.
    shown: (Option<String>, bool, u64),
}

impl MediaKeys {
    /// A desktop without a session bus has no media keys, and the player goes on without them.
    pub fn start(press: impl Fn(MediaEvent) + Send + 'static) -> Self {
        #[cfg(target_os = "linux")]
        let controls = {
            use souvlaki::{MediaControlEvent, MediaControls, PlatformConfig, SeekDirection};
            // A name of its own for each process, as browsers do: a second window would
            // otherwise find the first one's taken.
            let name = format!("surco.instance{}", std::process::id());
            let config = PlatformConfig { display_name: "Surco", dbus_name: &name, hwnd: None };
            MediaControls::new(config).ok().and_then(|mut controls| {
                let attached = controls.attach(move |event| {
                    let signed = |direction, seconds: f64| if direction == SeekDirection::Forward { seconds } else { -seconds };
                    let key = match event {
                        MediaControlEvent::Toggle => Key::Toggle,
                        MediaControlEvent::Play => Key::Play,
                        MediaControlEvent::Pause | MediaControlEvent::Stop => Key::Pause,
                        MediaControlEvent::Next => Key::Next,
                        MediaControlEvent::Previous => Key::Previous,
                        MediaControlEvent::SetPosition(position) => Key::Seek(position.0.as_secs_f64()),
                        MediaControlEvent::SeekBy(direction, by) => Key::SeekBy(signed(direction, by.as_secs_f64())),
                        MediaControlEvent::Seek(direction) => Key::SeekBy(signed(direction, 5.0)),
                        _ => return,
                    };
                    press(MediaEvent::Key(key));
                });
                attached.ok().map(|()| controls)
            })
        };
        #[cfg(target_os = "ios")]
        let ios = crate::ios_media::Controls::start(press);
        #[cfg(target_os = "android")]
        let android = crate::android_media::Controls::start(press);
        #[cfg(not(any(target_os = "linux", target_os = "ios", target_os = "android")))]
        drop(press);
        Self {
            #[cfg(target_os = "linux")]
            controls,
            #[cfg(target_os = "ios")]
            ios,
            #[cfg(target_os = "android")]
            android,
            shown: (None, false, 0),
        }
    }

    /// Tells the desktop what plays. Called on every change of the player; it speaks up once a second at most.
    pub fn show(&mut self, view: &PlayerView) {
        #[cfg(target_os = "ios")]
        self.ios.show(view);
        #[cfg(target_os = "android")]
        self.android.show(view);
        let shown = (view.current.as_ref().map(|track| track.id.clone()), view.playing, view.position.max(0.0) as u64);
        if shown == self.shown {
            return;
        }
        #[cfg(target_os = "linux")]
        if let Some(controls) = &mut self.controls {
            use souvlaki::{MediaMetadata, MediaPlayback, MediaPosition};
            use std::time::Duration;
            // The desktop only shows these; one it does not take is not worth stopping the music for.
            if let Some(track) = view.current.as_ref().filter(|_| shown.0 != self.shown.0) {
                let _ = controls.set_metadata(MediaMetadata {
                    title: Some(&track.title),
                    artist: Some(&track.artist),
                    album: track.album.as_deref(),
                    cover_url: track.artwork_url.as_deref(),
                    duration: (view.duration > 0.0).then(|| Duration::from_secs_f64(view.duration)),
                });
            }
            let progress = Some(MediaPosition(Duration::from_secs(shown.2)));
            let _ = controls.set_playback(match (&view.current, view.playing) {
                (None, _) => MediaPlayback::Stopped,
                (Some(_), true) => MediaPlayback::Playing { progress },
                (Some(_), false) => MediaPlayback::Paused { progress },
            });
        }
        self.shown = shown;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_asks_only_for_what_would_change_something() {
        let playing = PlayerView { playing: true, position: 30.0, duration: 200.0, ..PlayerView::default() };
        let paused = PlayerView { playing: false, ..playing.clone() };
        assert_eq!(Key::Play.command(&playing), None, "it already plays");
        assert_eq!(Key::Play.command(&paused), Some(Command::PlayPause));
        assert_eq!(Key::Pause.command(&playing), Some(Command::PlayPause));
        assert_eq!(Key::Pause.command(&paused), None);
        assert_eq!(Key::Next.command(&paused), Some(Command::Next));
        assert_eq!(Key::SeekBy(-45.0).command(&playing), Some(Command::Seek(0.0)), "not before the start");
        assert_eq!(Key::Seek(900.0).command(&playing), Some(Command::Seek(200.0)), "nor past the end");
    }
}
