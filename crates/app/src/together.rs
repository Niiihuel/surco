//! Listening together, as Opentune's rooms do: one device hosts and the others play what it
//! plays, in step, each fetching its own audio. Rooms run over public Nostr relays: every
//! message is encrypted with a key that comes from the room's code, signed with a key made for
//! the session, and sent as an ephemeral event, which relays pass on and do not keep.
//!
//! Only "what, where, playing or not" travels. Guests follow the host, correcting when they
//! drift, can suggest songs for its queue and, when the host lets them, play, pause, seek and
//! skip for everyone.

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use futures_util::{SinkExt as _, StreamExt as _};
use k256::schnorr::SigningKey;
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// Ephemeral events, as NIP-16 numbers them: relays pass them on without keeping them.
pub const KIND: u16 = 21_420;
/// The relays a room goes through, all at once: it works as long as any one does.
const RELAYS: [&str; 8] = [
    "wss://relay.primal.net",
    "wss://relay.snort.social",
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://nostr.mom",
    "wss://relay.nostr.net",
    "wss://nostr-pub.wellorder.net",
    "wss://offchain.pub",
];
/// Crockford's base 32, as the handoff's codes are.
const DIGITS: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const CODE_LENGTH: usize = 12;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn random_bytes<const N: usize>() -> [u8; N] {
    use ring::rand::SecureRandom as _;
    let mut bytes = [0u8; N];
    ring::rand::SystemRandom::new().fill(&mut bytes).expect("the system's randomness");
    bytes
}

/// Whether `said` looks like a room's code, typed as it comes, before its key is stretched.
pub fn code_shape(said: &str) -> bool {
    let code: Vec<char> = said.chars().filter(|c| !matches!(c, '-' | ' ')).collect();
    code.len() == CODE_LENGTH && code.iter().all(|c| c.is_ascii_alphanumeric() && !matches!(c.to_ascii_uppercase(), 'U'))
}

/// A room, by its 12-character code, like K7QX-M2PA-9DTE. Everything else comes from it: the
/// tag relays file its messages under, which says nothing of the code, and the key they are
/// sealed with, stretched so that the code cannot be guessed from the tag quickly.
pub struct Room {
    code: String,
    tag: String,
    key: LessSafeKey,
}

impl Room {
    pub fn new() -> Self {
        let code: String = random_bytes::<CODE_LENGTH>().iter().map(|byte| DIGITS[usize::from(*byte) % 32] as char).collect();
        Self::from_code(&code).expect("a code of its own digits")
    }

    /// The room a code names, typed as it comes: any case, spaces or dashes, an O for a zero.
    pub fn from_code(said: &str) -> Option<Self> {
        let code: String = said.chars().filter(|c| !matches!(c, '-' | ' ')).map(|c| match c.to_ascii_uppercase() {
            'O' => '0', 'I' | 'L' => '1', c => c,
        }).collect();
        if code.len() != CODE_LENGTH || !code.bytes().all(|byte| DIGITS.contains(&byte)) { return None; }
        let tag = hex(&Sha256::digest(format!("surco/together/tag/{code}")))[..32].to_owned();
        let mut key = [0u8; 32];
        ring::pbkdf2::derive(ring::pbkdf2::PBKDF2_HMAC_SHA256, std::num::NonZeroU32::new(120_000).unwrap(), b"surco/together/key", code.as_bytes(), &mut key);
        let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &key).ok()?);
        Some(Self { code, tag, key })
    }

    /// The code as it is shown and typed: in fours.
    pub fn pretty(&self) -> String {
        format!("{}-{}-{}", &self.code[..4], &self.code[4..8], &self.code[8..])
    }

    /// `plain`, sealed for the room: base64 of the nonce and the ciphertext, bound to its tag.
    fn seal(&self, plain: &str) -> String {
        let nonce = random_bytes::<12>();
        let mut sealed = plain.as_bytes().to_vec();
        self.key.seal_in_place_append_tag(Nonce::assume_unique_for_key(nonce), Aad::from(self.tag.as_bytes()), &mut sealed).expect("a message to seal");
        base64::engine::general_purpose::STANDARD.encode([nonce.as_slice(), &sealed].concat())
    }

    /// A message sealed for the room, or none when it was not, or was tampered with.
    fn open(&self, sealed: &str) -> Option<String> {
        let bytes = base64::engine::general_purpose::STANDARD.decode(sealed).ok()?;
        if bytes.len() < 12 + 16 { return None; }
        let (nonce, sealed) = bytes.split_at(12);
        let mut sealed = sealed.to_vec();
        let plain = self.key.open_in_place(Nonce::try_assume_unique_for_key(nonce).ok()?, Aad::from(self.tag.as_bytes()), &mut sealed).ok()?;
        String::from_utf8(plain.to_vec()).ok()
    }
}

/// One Nostr event, as NIP-01 has it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Event {
    id: String,
    pubkey: String,
    created_at: i64,
    kind: u16,
    tags: Vec<Vec<String>>,
    content: String,
    sig: String,
}

/// The session's own key, made fresh for each room.
fn session_key() -> SigningKey {
    loop {
        if let Ok(key) = SigningKey::from_bytes(&random_bytes::<32>()) { return key; }
    }
}

impl Event {
    /// Built and signed: its id is the hash of `[0,pubkey,created_at,kind,tags,content]` as
    /// JSON with NIP-01's escaping, which serde_json's own is, and the signature is BIP-340's.
    fn signed(key: &SigningKey, kind: u16, tags: Vec<Vec<String>>, content: String, created_at: i64) -> Self {
        let pubkey = hex(&key.verifying_key().to_bytes());
        let id: [u8; 32] = Sha256::digest(json!([0, pubkey, created_at, kind, tags, content]).to_string()).into();
        let sig = key.sign_raw(&id, &random_bytes::<32>()).expect("a signature over a hash");
        Self { id: hex(&id), pubkey, created_at, kind, tags, content, sig: hex(&sig.to_bytes()) }
    }
}

/// A song as a room passes it around: enough for another device to play it.
pub type Track = surco_core::Track;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Member {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub host: bool,
}

/// Everything said in a room. Sealed; relays never see it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum Msg {
    /// A guest arriving, and asking for the room's state.
    Hello { name: String },
    /// A guest still here.
    Here { name: String },
    Bye,
    /// The host's playback, on every change and every few seconds: the position in
    /// milliseconds at the host's time `at`.
    State {
        seq: u64,
        track: Option<Track>,
        playing: bool,
        pos: u64,
        at: i64,
        #[serde(default)]
        next: Vec<Track>,
        #[serde(default)]
        members: Vec<Member>,
        /// Guests may play, pause, seek and skip, not only suggest.
        #[serde(default)]
        open: bool,
    },
    /// The host closed the room.
    End,
    /// The asker's clock, for the host's to be compared with.
    Ping { c: i64 },
    /// The host's clock as it answered `to`.
    Pong { c: i64, h: i64, to: String },
    /// A guest asking the host: add, play, pause, resume, seek, next or previous.
    Ask {
        action: String,
        #[serde(default)]
        pos: u64,
        #[serde(default)]
        track: Option<Track>,
    },
    Chat { text: String },
}

/// Where a guest should be in the song now, given the host's last state and how far the host's
/// clock is ahead of this one's, in milliseconds.
pub fn target_position(playing: bool, pos: u64, at: i64, now: i64, offset: i64) -> u64 {
    if playing { (pos as i64 + now + offset - at).max(0) as u64 } else { pos }
}

/// How far the host's clock is ahead of this one's, from one ping: it answered at `host`,
/// taken to be halfway through the round trip.
pub fn clock_offset(sent: i64, host: i64, received: i64) -> i64 {
    host - (sent + (received - sent) / 2)
}

/// The relays a room is reached through: every message goes to each one connected, and each
/// one that comes is handed on once. Dropped connections come back with growing pauses.
pub struct Relays {
    frames: tokio::sync::broadcast::Sender<String>,
    connected: Arc<AtomicUsize>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
    key: SigningKey,
    room: Arc<Room>,
}

impl Drop for Relays {
    fn drop(&mut self) {
        for task in &self.tasks { task.abort(); }
    }
}

impl Relays {
    /// Connects to every relay, listening for the room's events from now on, handed to `heard`
    /// with who sent them; one's own are not.
    pub fn connect(room: Arc<Room>, heard: tokio::sync::mpsc::UnboundedSender<(String, Msg)>) -> Self {
        let (frames, _) = tokio::sync::broadcast::channel::<String>(64);
        let connected = Arc::new(AtomicUsize::new(0));
        let seen = Arc::new(Mutex::new(HashSet::<String>::new()));
        let key = session_key();
        let me = hex(&key.verifying_key().to_bytes());
        let since = chrono::Utc::now().timestamp() - 5;
        let request = json!(["REQ", "room", {"kinds": [KIND], "#t": [room.tag], "since": since}]).to_string();
        let tasks = RELAYS.iter().map(|url| {
            let (url, request, frames, connected, seen, heard, room, me) = (url.to_string(), request.clone(), frames.subscribe(), connected.clone(), seen.clone(), heard.clone(), room.clone(), me.clone());
            tokio::spawn(async move {
                let mut frames = frames;
                let mut pause = Duration::from_secs(2);
                loop {
                    if let Ok((mut socket, _)) = tokio_tungstenite::connect_async(&url).await {
                        pause = Duration::from_secs(2);
                        if socket.send(request.clone().into()).await.is_ok() {
                            connected.fetch_add(1, Ordering::SeqCst);
                            loop {
                                tokio::select! {
                                    frame = frames.recv() => match frame {
                                        Ok(frame) => if socket.send(frame.into()).await.is_err() { break },
                                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                                        Err(_) => return,
                                    },
                                    message = socket.next() => match message {
                                        Some(Ok(message)) => {
                                            let Ok(text) = message.to_text() else { continue };
                                            let Ok(Value::Array(parts)) = serde_json::from_str::<Value>(text) else { continue };
                                            if parts.first().and_then(Value::as_str) != Some("EVENT") { continue; }
                                            let Some(event) = parts.get(2).and_then(|event| serde_json::from_value::<Event>(event.clone()).ok()) else { continue };
                                            if event.pubkey == me || !seen.lock().unwrap().insert(event.id.clone()) { continue; }
                                            if let Some(msg) = room.open(&event.content).and_then(|plain| serde_json::from_str::<Msg>(&plain).ok()) {
                                                let _ = heard.send((event.pubkey, msg));
                                            }
                                        }
                                        _ => break,
                                    },
                                }
                            }
                            connected.fetch_sub(1, Ordering::SeqCst);
                        }
                    }
                    tokio::time::sleep(pause).await;
                    pause = (pause * 2).min(Duration::from_secs(60));
                }
            })
        }).collect();
        Self { frames, connected, tasks, key, room }
    }

    /// Who this device is in the room.
    pub fn me(&self) -> String {
        hex(&self.key.verifying_key().to_bytes())
    }

    /// How many relays carry the room now.
    pub fn connected(&self) -> usize {
        self.connected.load(Ordering::SeqCst)
    }

    /// Says `msg` to the room, through every relay connected.
    pub fn say(&self, msg: &Msg) {
        let Ok(plain) = serde_json::to_string(msg) else { return };
        let event = Event::signed(&self.key, KIND, vec![vec!["t".into(), self.room.tag.clone()]], self.room.seal(&plain), chrono::Utc::now().timestamp());
        let _ = self.frames.send(json!(["EVENT", event]).to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_typed_as_it_comes_names_the_room_and_only_its_key_opens_what_it_seals() {
        let room = Room::new();
        assert_eq!(room.pretty().len(), 14);
        let typed = Room::from_code(&room.pretty().to_lowercase().replace('0', "o")).unwrap();
        assert_eq!(typed.tag, room.tag);
        let sealed = room.seal("{\"t\":\"bye\"}");
        assert_eq!(typed.open(&sealed).as_deref(), Some("{\"t\":\"bye\"}"));
        assert!(Room::new().open(&sealed).is_none(), "another room's key");
        let mut tampered = base64::engine::general_purpose::STANDARD.decode(&sealed).unwrap();
        *tampered.last_mut().unwrap() ^= 1;
        assert!(room.open(&base64::engine::general_purpose::STANDARD.encode(tampered)).is_none());
        assert!(Room::from_code("K7QX-M2PA").is_none() && Room::from_code("K7QX-M2PA-9DTU").is_none(), "short, or with a U");
    }

    #[test]
    fn an_event_is_signed_as_bip_340_has_it_over_its_nip_01_id() {
        // BIP-340's first test vector: secret key 3, message and aux_rand zero.
        let key = SigningKey::from_bytes(&[[0u8; 31].as_slice(), &[3]].concat()).unwrap();
        let sig = key.sign_raw(&[0u8; 32], &[0u8; 32]).unwrap();
        assert_eq!(hex(&key.verifying_key().to_bytes()), "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9");
        assert_eq!(hex(&sig.to_bytes()), "e907831f80848d1069a5371b402410364bdf1c5f8307b0084c55f1ce2dca821525f66a4a85ea8b71e482a74f382d2ce5ebeee8fdb2172f477df4900d310536c0");
        let event = Event::signed(&key, KIND, vec![vec!["t".into(), "x".into()]], "a \"quote\"\nand a line".into(), 1_700_000_000);
        let expected = format!("[0,\"{}\",1700000000,21420,[[\"t\",\"x\"]],\"a \\\"quote\\\"\\nand a line\"]", event.pubkey);
        assert_eq!(event.id, hex(&Sha256::digest(expected)));
        let id: [u8; 32] = Sha256::digest(json!([0, event.pubkey, 1_700_000_000, KIND, [["t", "x"]], "a \"quote\"\nand a line"]).to_string()).into();
        let sig = k256::schnorr::Signature::try_from(<[u8; 64]>::try_from(&*base16(&event.sig)).unwrap().as_slice()).unwrap();
        assert!(key.verifying_key().verify_raw(&id, &sig).is_ok());
    }

    fn base16(text: &str) -> Vec<u8> {
        (0..text.len()).step_by(2).map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap()).collect()
    }

    #[test]
    fn messages_travel_as_opentunes_and_a_guest_finds_where_the_host_is() {
        assert_eq!(serde_json::to_string(&Msg::Hello { name: "Ana".into() }).unwrap(), "{\"t\":\"hello\",\"name\":\"Ana\"}");
        assert_eq!(serde_json::from_str::<Msg>("{\"t\":\"ask\",\"action\":\"next\"}").unwrap(), Msg::Ask { action: "next".into(), pos: 0, track: None });
        // A ping answered at 1_100 by the host, sent at 1_000 and back at 1_040: the host is 80 ahead.
        let offset = clock_offset(1_000, 1_100, 1_040);
        assert_eq!(offset, 80);
        assert_eq!(target_position(true, 30_000, 5_080, 5_500, offset), 30_500, "half a second later");
        assert_eq!(target_position(false, 30_000, 5_080, 9_000, offset), 30_000, "paused, it stays");
    }
}
