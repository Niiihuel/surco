//! Checks a room through the real Nostr relays: one side says hello, the other hears it.
//! `cargo run -p surco --example together_probe`

#[allow(dead_code)]
#[path = "../src/together.rs"]
mod together;

use std::sync::Arc;

#[tokio::main]
async fn main() {
    let room = Arc::new(together::Room::new());
    let typed = Arc::new(together::Room::from_code(&room.pretty()).unwrap());
    let (host_heard, mut host_inbox) = tokio::sync::mpsc::unbounded_channel();
    let (guest_heard, _guest_inbox) = tokio::sync::mpsc::unbounded_channel();
    let host = together::Relays::connect(room, host_heard);
    let guest = together::Relays::connect(typed, guest_heard);
    for _ in 0..40 {
        if host.connected() > 0 && guest.connected() > 0 { break; }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    println!("relays: host {}, guest {}", host.connected(), guest.connected());
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    guest.say(&together::Msg::Hello { name: "Ana".into() });
    match tokio::time::timeout(std::time::Duration::from_secs(10), host_inbox.recv()).await {
        Ok(Some((from, msg))) => println!("host heard {msg:?} from {}…, guest is {}…", &from[..8], &guest.me()[..8]),
        _ => println!("host heard nothing"),
    }
}
