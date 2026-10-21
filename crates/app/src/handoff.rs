//! Taking the account to another device: what of it travels, served over the local network
//! while the dialog that offers it is open, behind a token only its QR code and its short code
//! carry, and read by the other device from there.

use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use surco_core::Track;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::i18n::t;
use crate::state::{Entry, Playlist, Profile, Settings};

/// The bundle's format: one from a later version is refused rather than misread.
pub const VERSION: u32 = 1;
/// The scheme a phone's camera hands a code to Surco by.
const SCHEME: &str = "surco://receive?from=";
/// The most a request to the bundle may say before it is answered.
const REQUEST_MOST: usize = 8 * 1024;
/// The ports a bundle is served on, so a code names one by its place among them.
const FIRST_PORT: u16 = 47_310;
const PORTS: u16 = 8;
/// The bits of a code: the last two parts of the address, the port's place and the token.
const TOKEN_BITS: u32 = 21;
/// Crockford's base 32: no I, L, O or U to misread.
const DIGITS: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// Wrong tokens a bundle is asked for before it is no longer served: a code's token is short.
const MISSES_MOST: usize = 64;

/// What of an account travels, each part empty where it was left out.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Bundle {
    pub version: u32,
    pub profile: Option<Profile>,
    pub playlists: Vec<Playlist>,
    pub liked: Vec<Track>,
    /// The artists followed and the albums kept.
    pub kept: Vec<Entry>,
    pub heard: Vec<Track>,
    pub settings: Option<Settings>,
    /// The equaliser, as the app keeps it.
    pub equalizer: Option<String>,
    /// The pictures the profile and the lists name, by their names, as base64 JPEG. Taken in,
    /// they are kept with the covers and leave the bundle.
    pub pictures: Vec<(String, String)>,
}

impl Bundle {
    /// The names of the pictures of its own the account has: its face and its lists'.
    pub fn picture_names(&self) -> Vec<String> {
        let profile = self.profile.iter().filter_map(|profile| profile.picture.clone());
        profile.chain(self.playlists.iter().filter_map(|playlist| playlist.picture.clone())).collect()
    }
}

/// The parts of an account, each sent or taken in or not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parts {
    pub profile: bool,
    pub playlists: bool,
    pub liked: bool,
    pub library: bool,
    pub history: bool,
    pub settings: bool,
    pub equalizer: bool,
}

impl Default for Parts {
    /// Everything but what was played, which is the device's own.
    fn default() -> Self {
        Self { profile: true, playlists: true, liked: true, library: true, history: false, settings: true, equalizer: true }
    }
}

impl Parts {
    /// Each part by the name its switch reports, with its own.
    pub fn by_name(&mut self, name: &str) -> Option<&mut bool> {
        Some(match name {
            "profile" => &mut self.profile,
            "playlists" => &mut self.playlists,
            "liked" => &mut self.liked,
            "library" => &mut self.library,
            "history" => &mut self.history,
            "settings" => &mut self.settings,
            "equalizer" => &mut self.equalizer,
            _ => return None,
        })
    }
}

/// The code another device types, and the link the QR code a phone's camera reads holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Offer {
    /// `7K2M-QX4P`: the address on the network and the token, short enough to type.
    pub code: String,
    /// The link the code holds: Surco's own scheme, around the address.
    pub link: String,
}

/// A bundle being served, until it is dropped.
pub struct Serving {
    /// What a request is answered with; another bundle replaces it in place.
    body: Arc<Mutex<Vec<u8>>>,
    task: tokio::task::JoinHandle<()>,
    pub offer: Offer,
}

impl Serving {
    /// Answers with `body` from here on.
    pub fn replace(&self, body: Vec<u8>) {
        *self.body.lock().unwrap() = body;
    }
}

impl Drop for Serving {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// The address this device has on its local network: the one it would reach out by. Nothing
/// is sent to find it.
fn local_address() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    // A documentation address: a route to it is all that is asked for.
    socket.connect("192.0.2.1:9").ok()?;
    Some(socket.local_addr().ok()?.ip()).filter(|ip| !ip.is_loopback() && !ip.is_unspecified())
}

/// The code of a bundle served at `ip`, on the port at `slot` among the ones served on, behind
/// `token`: what of the address the other device does not share with this one, and the token.
fn encode(ip: Ipv4Addr, slot: u16, token: u32) -> String {
    let [_, _, high, low] = ip.octets();
    let value = (u64::from(high) << 32) | (u64::from(low) << 24) | (u64::from(slot) << TOKEN_BITS) | u64::from(token);
    let digits: String = (0..8).rev().map(|place| DIGITS[(value >> (place * 5)) as usize & 31] as char).collect();
    format!("{}-{}", &digits[..4], &digits[4..])
}

/// Where the bundle a code names is, from `own`, this device's address on the same network.
// shortcut: the code carries only the address's last two parts, so both devices must share
// the first two, as on any home network; carry them too if a larger one turns up.
fn decode(code: &str, own: Ipv4Addr) -> Option<String> {
    let mut value = 0u64;
    let mut count = 0;
    for c in code.chars().filter(|c| !matches!(c, '-' | ' ')) {
        let c = match c.to_ascii_uppercase() { 'O' => '0', 'I' | 'L' => '1', c => c };
        value = (value << 5) | DIGITS.iter().position(|digit| *digit as char == c)? as u64;
        count += 1;
    }
    if count != 8 { return None; }
    let [first, second, ..] = own.octets();
    let ip = Ipv4Addr::new(first, second, (value >> 32) as u8, (value >> 24) as u8);
    let slot = (value >> TOKEN_BITS) as u16 & (PORTS - 1);
    let token = value as u32 & ((1 << TOKEN_BITS) - 1);
    Some(format!("http://{ip}:{}/{token:06x}", FIRST_PORT + slot))
}

/// Starts answering `body` on the local network, to the one who has its token.
pub async fn serve(body: Vec<u8>) -> Result<Serving, String> {
    let Some(IpAddr::V4(ip)) = local_address() else { return Err(t!("backend-handoff-no-network").into_owned()) };
    let mut bound = None;
    for slot in 0..PORTS {
        if let Ok(listener) = TcpListener::bind((ip, FIRST_PORT + slot)).await {
            bound = Some((slot, listener));
            break;
        }
    }
    let (slot, listener) = bound.ok_or_else(|| t!("backend-handoff-no-port").into_owned())?;
    let token = fastrand::u32(..1 << TOKEN_BITS);
    let body = Arc::new(Mutex::new(body));
    let served = body.clone();
    let path = format!("/{token:06x}");
    let task = tokio::spawn(async move {
        let misses = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        while let Ok((stream, _)) = listener.accept().await {
            if misses.load(std::sync::atomic::Ordering::Relaxed) >= MISSES_MOST { break; }
            tokio::spawn(answer(stream, path.clone(), served.clone(), misses.clone()));
        }
    });
    let address = format!("http://{ip}:{}/{token:06x}", FIRST_PORT + slot);
    let link = format!("{SCHEME}{}", percent_encoding::utf8_percent_encode(&address, percent_encoding::NON_ALPHANUMERIC));
    Ok(Serving { body, task, offer: Offer { code: encode(ip, slot, token), link } })
}

/// One request: the bundle to `GET` its path, nothing to anything else.
async fn answer(mut stream: tokio::net::TcpStream, path: String, body: Arc<Mutex<Vec<u8>>>, misses: Arc<std::sync::atomic::AtomicUsize>) {
    let mut request = Vec::new();
    let mut chunk = [0u8; 1024];
    // Up to the end of the head, which is all that is read.
    while !request.windows(4).any(|end| end == b"\r\n\r\n") && request.len() < REQUEST_MOST {
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(read) => request.extend_from_slice(&chunk[..read]),
        }
    }
    let line = String::from_utf8_lossy(&request);
    let asked = line.lines().next().is_some_and(|first| first.split(' ').take(2).eq(["GET", path.as_str()]));
    if !asked { misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed); }
    let (status, kind, body) = if asked { ("200 OK", "application/json", body.lock().unwrap().clone()) } else { ("404 Not Found", "text/plain", Vec::new()) };
    let head = format!("HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
    let _ = stream.write_all(head.as_bytes()).await;
    let _ = stream.write_all(&body).await;
    let _ = stream.shutdown().await;
}

/// Where a bundle is, from what was typed or the QR code read: the short code, the link of the
/// QR code, or the address an earlier version showed, with or without its scheme.
pub fn locate(said: &str) -> Option<String> {
    let said = said.trim();
    if let Some(IpAddr::V4(own)) = local_address() {
        if let Some(found) = decode(said, own) { return Some(found); }
    }
    let said = match said.strip_prefix(SCHEME) {
        Some(encoded) => percent_encoding::percent_decode_str(encoded).decode_utf8().ok()?.into_owned(),
        None => said.to_owned(),
    };
    let address = said.strip_prefix("http://").unwrap_or(&said);
    // A host and port and a token, and nothing else: not a page on the internet.
    let (host, token) = address.split_once('/')?;
    let ok = host.contains(':') && !token.is_empty() && token.chars().all(|c| c.is_ascii_hexdigit());
    ok.then(|| format!("http://{address}"))
}

/// Reads a bundle as it came: one of a later version, or not one, is said so.
pub fn read(bytes: &[u8]) -> Result<Bundle, String> {
    let bundle: Bundle = serde_json::from_slice(bytes).map_err(|_| t!("backend-handoff-unreadable").into_owned())?;
    if bundle.version > VERSION {
        return Err(t!("backend-handoff-newer").into_owned());
    }
    Ok(bundle)
}

/// The code a phone's camera reads `link` from: black squares on white with their margin, as
/// PNG, `scale` pixels a square.
pub fn code(link: &str, scale: u32) -> Option<Vec<u8>> {
    let code = qrcode::QrCode::new(link.as_bytes()).ok()?;
    let squares = u32::try_from(code.width()).ok()?;
    const MARGIN: u32 = 4;
    let side = (squares + 2 * MARGIN) * scale;
    let colors = code.to_colors();
    let picture = image::GrayImage::from_fn(side, side, |x, y| {
        let (column, row) = ((x / scale).checked_sub(MARGIN), (y / scale).checked_sub(MARGIN));
        let dark = column.zip(row).filter(|(column, row)| *column < squares && *row < squares)
            .is_some_and(|(column, row)| colors[(row * squares + column) as usize] == qrcode::Color::Dark);
        image::Luma([if dark { 0 } else { 255 }])
    });
    let mut png = Vec::new();
    picture.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).ok()?;
    Some(png)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bundle_is_found_from_the_address_shown_or_the_codes_link_and_nothing_else() {
        let found = Some("http://192.168.1.20:48213/3f9cab".to_owned());
        assert_eq!(locate(" 192.168.1.20:48213/3f9cab "), found);
        assert_eq!(locate("http://192.168.1.20:48213/3f9cab"), found);
        assert_eq!(locate("surco://receive?from=http%3A%2F%2F192%2E168%2E1%2E20%3A48213%2F3f9cab"), found);
        assert_eq!(locate("https://example.com/page"), None, "not a page on the internet");
        assert_eq!(locate("192.168.1.20:48213/../etc"), None);
        assert_eq!(locate("192.168.1.20/3f9cab"), None, "a port is part of it");
    }

    #[test]
    fn a_short_code_names_the_address_port_and_token_on_the_same_network() {
        let code = encode(Ipv4Addr::new(192, 168, 1, 20), 3, 0x1a2b3);
        assert_eq!(code.len(), 9);
        let own = Ipv4Addr::new(192, 168, 7, 9);
        assert_eq!(decode(&code, own).as_deref(), Some("http://192.168.1.20:47313/01a2b3"));
        assert_eq!(decode(&code.to_lowercase().replace('-', " "), own), decode(&code, own), "typed as it comes");
        assert_eq!(decode(&code.replace('0', "O"), own), decode(&code, own), "an O read for a zero");
        assert_eq!(decode(&code[..8], own), None, "a character short");
        assert_eq!(decode("192.168.1.20:48213/3f9cab", own), None);
        // Wherever the largest values land, they come back.
        let code = encode(Ipv4Addr::new(10, 0, 255, 255), PORTS - 1, (1 << TOKEN_BITS) - 1);
        assert_eq!(decode(&code, Ipv4Addr::new(10, 0, 0, 2)).as_deref(), Some("http://10.0.255.255:47317/1fffff"));
    }

    #[test]
    fn a_bundle_reads_back_and_one_from_a_later_version_is_refused() {
        let bundle = Bundle { version: VERSION, liked: vec![Track { id: "a".into(), title: "A".into(), artist: "Ana".into(), artist_id: None, album: None, album_id: None, artwork_url: None, duration_ms: 1 }], ..Bundle::default() };
        assert_eq!(read(&serde_json::to_vec(&bundle).unwrap()), Ok(bundle));
        assert!(read(&serde_json::to_vec(&Bundle { version: VERSION + 1, ..Bundle::default() }).unwrap()).is_err());
        assert!(read(b"<html>").is_err());
    }

    #[test]
    fn the_code_is_a_square_picture_with_its_margin() {
        let png = code("surco://receive?from=http%3A%2F%2F10.0.0.2%3A9%2Fab", 4).unwrap();
        let picture = image::load_from_memory(&png).unwrap().to_luma8();
        assert_eq!(picture.width(), picture.height());
        assert_eq!(picture.get_pixel(0, 0)[0], 255, "a white margin");
        assert_eq!(picture.get_pixel(16, 16)[0], 0, "the first finder's corner");
    }

    /// A request as a browser makes it, and the answer's status line and body.
    async fn get(address: &str) -> Option<(String, Vec<u8>)> {
        let (host, path) = address.split_once('/')?;
        let mut stream = tokio::net::TcpStream::connect(host).await.ok()?;
        stream.write_all(format!("GET /{path} HTTP/1.1\r\nHost: {host}\r\n\r\n").as_bytes()).await.ok()?;
        let mut answer = Vec::new();
        stream.read_to_end(&mut answer).await.ok()?;
        let end = answer.windows(4).position(|end| end == b"\r\n\r\n")?;
        let status = String::from_utf8_lossy(&answer[..end]).lines().next()?.to_owned();
        Some((status, answer[end + 4..].to_vec()))
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_bundle_is_served_to_its_token_only_until_it_is_dropped() {
        // A machine with no network has nothing to serve on.
        let Ok(serving) = serve(b"{\"version\":1}".to_vec()).await else { return };
        let own = match local_address() { Some(IpAddr::V4(own)) => own, _ => return };
        let address = decode(&serving.offer.code, own).unwrap().trim_start_matches("http://").to_owned();
        assert_eq!(locate(&serving.offer.link).as_deref(), Some(format!("http://{address}").as_str()), "the QR code's link says the same");
        assert_eq!(get(&address).await.map(|(_, body)| body).as_deref(), Some(&b"{\"version\":1}"[..]));
        serving.replace(b"{\"version\":1,\"liked\":[]}".to_vec());
        assert_eq!(get(&address).await.map(|(_, body)| body).as_deref(), Some(&b"{\"version\":1,\"liked\":[]}"[..]), "another bundle in its place");
        let (status, body) = get(&format!("{address}0")).await.unwrap();
        assert!(status.contains("404") && body.is_empty(), "nothing to another token");
        drop(serving);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(get(&address).await.is_none(), "gone with the dialog");
    }
}
