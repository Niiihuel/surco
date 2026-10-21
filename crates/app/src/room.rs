//! A room listened in together, as it runs: hosted, it says what plays and takes what its
//! guests ask; joined, it follows the host, correcting when it drifts.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use surco_core::Track;
use tokio::sync::{mpsc, watch};

use crate::backend::Command;
use crate::player::PlayerView;
use crate::together::{clock_offset, target_position, Member, Msg, Relays, Room};

/// What the window asks of a room it is in.
#[derive(Clone, Debug, PartialEq)]
pub enum Order {
    Chat(String),
    /// A guest's: add, play, pause, resume, seek, next or previous.
    Ask { action: String, pos: u64, track: Option<Track> },
    /// The host's: whether guests may play, pause, seek and skip.
    Open(bool),
    Leave,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Connecting,
    /// A guest, with no word from the host yet.
    Waiting,
    Live,
    /// A guest whose host has not been heard from for a while.
    LostHost,
    Ended,
}

/// What a line of the room's chat is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Said {
    Chat,
    /// Someone came in; its text is empty.
    Joined,
    /// A guest suggested a song; its text is the song's title.
    Suggested,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub name: String,
    pub text: String,
    pub mine: bool,
    pub said: Said,
}

/// What the window shows of a room.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RoomView {
    pub code: String,
    pub hosting: bool,
    pub phase: Phase,
    pub relays: usize,
    pub members: Vec<Member>,
    pub chat: Vec<Line>,
    pub open: bool,
}

/// How many lines of chat are kept.
const CHAT_KEPT: usize = 100;
/// How often the host says where it is with nothing changing, and a guest that it is there.
const STATE_EVERY: Duration = Duration::from_secs(4);
const HERE_EVERY: Duration = Duration::from_secs(15);
/// A guest unheard for this long has left; a host, is lost.
const GONE_AFTER: Duration = Duration::from_secs(45);
const LOST_AFTER: Duration = Duration::from_secs(20);
/// How far a guest may drift from the host before it is put back, in milliseconds.
const DRIFT_MOST: u64 = 1_500;

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Runs a room until it is left or ended: `command` is handed to the player as the window's
/// own are, and `show` is told how the room stands on every change.
pub async fn run(room: Room, hosting: bool, name: String, mut orders: mpsc::UnboundedReceiver<Order>, mut playing: watch::Receiver<PlayerView>,
    command: impl Fn(Command), show: impl Fn(Option<RoomView>)) {
    let code = room.pretty();
    let (heard, mut inbox) = mpsc::unbounded_channel();
    let relays = Relays::connect(Arc::new(room), heard);
    let me = relays.me();
    let mut view = RoomView { code, hosting, phase: if hosting { Phase::Live } else { Phase::Connecting }, ..RoomView::default() };
    let mut guests: HashMap<String, (String, Instant)> = HashMap::new();
    let mut seq = 0u64;
    let mut said_state = Instant::now() - STATE_EVERY;
    let mut said_here = Instant::now() - HERE_EVERY;
    let mut last = playing.borrow().clone();
    // A guest's: the host, its last state as heard and when, its clock's lead, and the song
    // asked for to follow it.
    let mut host: Option<String> = None;
    let mut state: Option<(Option<Track>, bool, u64, i64, Instant)> = None;
    let mut offset = 0i64;
    let mut pinged = Instant::now() - Duration::from_secs(60);
    let mut loading: Option<String> = None;
    let mut host_playing: Option<bool> = None;
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    let line = |view: &mut RoomView, name: String, text: String, mine: bool, said: Said| {
        view.chat.push(Line { name, text, mine, said });
        if view.chat.len() > CHAT_KEPT { view.chat.remove(0); }
    };
    let members = |guests: &HashMap<String, (String, Instant)>, me: &str, name: &str| {
        let mut members = vec![Member { id: me.to_owned(), name: name.to_owned(), host: true }];
        let mut others: Vec<Member> = guests.iter().map(|(id, (name, _))| Member { id: id.clone(), name: name.clone(), host: false }).collect();
        others.sort_by(|a, b| a.name.cmp(&b.name));
        members.extend(others);
        members
    };
    let name_of = |view: &RoomView, id: &str| view.members.iter().find(|member| member.id == id).map_or_else(|| "?".to_owned(), |member| member.name.clone());
    if hosting { view.members = members(&guests, &me, &name); }
    show(Some(view.clone()));
    loop {
        let before = view.clone();
        let mut say_state = false;
        tokio::select! {
            order = orders.recv() => match order {
                Some(Order::Chat(text)) => {
                    relays.say(&Msg::Chat { text: text.clone() });
                    line(&mut view, name.clone(), text, true, Said::Chat);
                }
                Some(Order::Ask { action, pos, track }) => relays.say(&Msg::Ask { action, pos, track }),
                Some(Order::Open(open)) if hosting => { view.open = open; say_state = true; }
                Some(Order::Open(_)) => {}
                Some(Order::Leave) | None => {
                    relays.say(if hosting { &Msg::End } else { &Msg::Bye });
                    // The last word on its way before the connections go.
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    show(None);
                    return;
                }
            },
            Some((from, msg)) = inbox.recv() => match msg {
                Msg::Hello { name: who } | Msg::Here { name: who } if hosting => {
                    let arrived = guests.insert(from, (who.clone(), Instant::now())).is_none();
                    if arrived { line(&mut view, who, String::new(), false, Said::Joined); say_state = true; }
                    view.members = members(&guests, &me, &name);
                }
                Msg::Bye if hosting => {
                    guests.remove(&from);
                    view.members = members(&guests, &me, &name);
                    say_state = true;
                }
                Msg::Ping { c } if hosting => relays.say(&Msg::Pong { c, h: now_ms(), to: from }),
                Msg::Ask { action, pos, track } if hosting => {
                    let current = playing.borrow().clone();
                    match (action.as_str(), track) {
                        ("add", Some(track)) => {
                            let who = name_of(&view, &from);
                            line(&mut view, who, track.title.clone(), false, Said::Suggested);
                            command(Command::Enqueue(track));
                        }
                        ("play", Some(track)) if view.open => command(Command::PlayList { tracks: vec![track], index: 0 }),
                        ("pause", _) if view.open && current.playing => command(Command::PlayPause),
                        ("resume", _) if view.open && !current.playing => command(Command::PlayPause),
                        ("seek", _) if view.open => command(Command::Seek(pos as f64 / 1000.0)),
                        ("next", _) if view.open => command(Command::Next),
                        ("previous", _) if view.open => command(Command::Previous),
                        _ => {}
                    }
                }
                Msg::Chat { text } => {
                    let who = name_of(&view, &from);
                    line(&mut view, who, text, false, Said::Chat);
                }
                Msg::State { track, playing: on, pos, at, members, open, .. } if !hosting && host.as_ref().is_none_or(|host| *host == from) => {
                    host = Some(from);
                    view.phase = Phase::Live;
                    view.members = members;
                    view.open = open;
                    state = Some((track, on, pos, at, Instant::now()));
                }
                Msg::Pong { c, h, to } if !hosting && to == me => offset = clock_offset(c, h, now_ms()),
                Msg::End if !hosting && host.as_ref().is_some_and(|host| *host == from) => {
                    view.phase = Phase::Ended;
                    show(Some(view.clone()));
                    return;
                }
                _ => {}
            },
            Ok(()) = playing.changed(), if hosting => {
                let now = playing.borrow().clone();
                // What plays, playing or not, or a jump in it: said at once.
                let jumped = (now.position - last.position).abs() > 2.5;
                say_state = now.current.as_ref().map(|track| &track.id) != last.current.as_ref().map(|track| &track.id) || now.playing != last.playing || jumped;
                last = now;
            },
            _ = tick.tick() => {
                if hosting {
                    let gone = guests.iter().filter(|(_, (_, seen))| seen.elapsed() > GONE_AFTER).map(|(id, _)| id.clone()).collect::<Vec<_>>();
                    if !gone.is_empty() {
                        for id in gone { guests.remove(&id); }
                        view.members = members(&guests, &me, &name);
                    }
                    say_state |= said_state.elapsed() >= STATE_EVERY;
                } else {
                    if relays.connected() > 0 && (said_here.elapsed() >= HERE_EVERY || (host.is_none() && said_here.elapsed() >= Duration::from_secs(4))) {
                        said_here = Instant::now();
                        relays.say(&if host.is_none() { Msg::Hello { name: name.clone() } } else { Msg::Here { name: name.clone() } });
                    }
                    if relays.connected() > 0 && pinged.elapsed() >= Duration::from_secs(10) {
                        pinged = Instant::now();
                        relays.say(&Msg::Ping { c: now_ms() });
                    }
                    view.phase = match (&state, view.phase) {
                        (Some((.., heard)), _) if heard.elapsed() > LOST_AFTER => Phase::LostHost,
                        (Some(_), _) => Phase::Live,
                        (None, _) if relays.connected() > 0 => Phase::Waiting,
                        (None, phase) => phase,
                    };
                }
                view.relays = relays.connected();
            },
        }
        if hosting && say_state {
            said_state = Instant::now();
            seq += 1;
            let now = playing.borrow().clone();
            relays.say(&Msg::State {
                seq, track: now.current.clone(), playing: now.playing, pos: (now.position * 1000.0) as u64, at: now_ms(),
                next: now.upcoming.iter().take(10).cloned().collect(), members: view.members.clone(), open: view.open,
            });
        }
        // A guest follows: the host's song, its playing or not when that changes, and where it is.
        if let (false, Some((Some(track), on, pos, at, _))) = (hosting, &state) {
            let own = playing.borrow().clone();
            let target = target_position(*on, *pos, *at, now_ms(), offset);
            if own.current.as_ref().is_none_or(|current| current.id != track.id) {
                if loading.as_deref() != Some(track.id.as_str()) {
                    loading = Some(track.id.clone());
                    host_playing = None;
                    command(Command::PlayList { tracks: vec![track.clone()], index: 0 });
                }
            } else {
                if host_playing != Some(*on) && own.playing != *on {
                    command(Command::PlayPause);
                } else if own.playing && ((own.position * 1000.0) as u64).abs_diff(target) > DRIFT_MOST {
                    command(Command::Seek(target as f64 / 1000.0));
                }
                host_playing = Some(*on);
            }
        }
        if view != before { show(Some(view.clone())); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str) -> Track {
        Track { id: id.into(), title: id.into(), artist: "Artist".into(), artist_id: None, album: None, album_id: None, artwork_url: None, duration_ms: 200_000 }
    }

    /// Through the real relays: `cargo test -p surco --lib room -- --ignored`.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "goes through public Nostr relays"]
    async fn a_guest_plays_what_the_host_does_and_goes_to_where_it_is() {
        let host_room = Room::new();
        let guest_room = Room::from_code(&host_room.pretty()).unwrap();
        let playing = PlayerView { current: Some(track("song")), playing: true, position: 30.0, duration: 200.0, ..PlayerView::default() };
        let (host_view, host_watch) = watch::channel(playing);
        let (guest_view, guest_watch) = watch::channel(PlayerView::default());
        let (_host_orders, host_inbox) = mpsc::unbounded_channel();
        let (_guest_orders, guest_inbox) = mpsc::unbounded_channel();
        let (given, mut commands) = mpsc::unbounded_channel();
        let (shown, mut views) = mpsc::unbounded_channel();
        tokio::spawn(run(host_room, true, "Host".into(), host_inbox, host_watch, |_| {}, |_| {}));
        tokio::spawn(run(guest_room, false, "Ana".into(), guest_inbox, guest_watch, move |command| drop(given.send(command)), move |view| drop(shown.send(view))));
        let first = tokio::time::timeout(Duration::from_secs(30), commands.recv()).await.expect("an order in time").unwrap();
        assert_eq!(first, Command::PlayList { tracks: vec![track("song")], index: 0 }, "the host's song");
        // Playing it from its start, the guest is put where the host is.
        guest_view.send_replace(PlayerView { current: Some(track("song")), playing: true, position: 0.0, duration: 200.0, ..PlayerView::default() });
        let seek = tokio::time::timeout(Duration::from_secs(10), commands.recv()).await.expect("a seek in time").unwrap();
        assert!(matches!(seek, Command::Seek(at) if (29.0..45.0).contains(&at)), "{seek:?}");
        let mut members = Vec::new();
        while let Ok(Some(Some(view))) = tokio::time::timeout(Duration::from_millis(200), views.recv()).await { members = view.members; }
        assert!(members.iter().any(|member| member.host && member.name == "Host"), "{members:?}");
        drop(host_view);
    }
}
