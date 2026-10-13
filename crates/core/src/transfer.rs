//! Bringing a list in from another app and taking one out: whose list a link names, which
//! of the catalog's songs a song named elsewhere is, and a list as text.

use crate::Track;
use crate::discover::plain;

/// Whose list a link is, and its id there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// Spotify's: a `playlist` or an `album`.
    Spotify(&'static str, String),
    /// Apple Music's, playlist or album: the address of its page.
    Apple(String),
    /// YouTube's or YouTube Music's.
    YouTube(String),
}

/// The list a link of another app names. Anything else is not such a link.
pub fn source(link: &str) -> Option<Source> {
    let id = |text: &str| {
        let id: String = text.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
        (!id.is_empty()).then_some(id)
    };
    let link = link.trim();
    if let Some((_, path)) = link.split_once("open.spotify.com/") {
        // A link may say its reader's language first: `intl-es/playlist/…`.
        let mut parts = path.split('/').filter(|part| !part.starts_with("intl-") && *part != "embed");
        let said = parts.next()?;
        let kind = ["playlist", "album"].into_iter().find(|kind| *kind == said)?;
        return Some(Source::Spotify(kind, id(parts.next()?)?));
    }
    if let Some((_, path)) = link.split_once("music.apple.com/") {
        let path = path.split(['?', '#']).next()?;
        return (path.contains("/playlist/") || path.contains("/album/")).then(|| Source::Apple(format!("https://music.apple.com/{path}")));
    }
    if link.contains("youtube.com/") || link.contains("youtu.be/") {
        return Some(Source::YouTube(id(link.split(['?', '&']).find_map(|pair| pair.strip_prefix("list="))?)?));
    }
    None
}

/// A song as another app names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wanted {
    pub title: String,
    /// All of them, as one text.
    pub artist: String,
    /// Zero when it was not said.
    pub duration_ms: u64,
}

/// Renditions that are another recording of a song. One that is such where the other is
/// not is not the same song.
const RENDITIONS: [&[&str]; 6] = [
    &["live", "vivo", "directo", "unplugged", "concert"],
    &["acoustic", "acustic"],
    &["remix", "rmx"],
    &["instrumental", "karaoke"],
    &["demo"],
    &["cover", "tribute", "tributo"],
];

/// How good a match has to be to be taken, out of 1: more than a song of the same name
/// and length by somebody else comes to, and no more than another cut of the song does.
const CLOSE_ENOUGH: f32 = 0.7;

impl Wanted {
    /// What to ask the catalog for, one way after another: whose it is and its name as it
    /// is written, its bare name before whose it is, and its bare name alone. The catalog
    /// answers a new song asked for one way with nothing, and asked for another with it.
    pub fn queries(&self) -> Vec<String> {
        let (artist, bare) = (first(&self.artist), name(&self.title));
        let mut queries = vec![format!("{artist} {}", self.title).trim().to_owned()];
        for other in [format!("{bare} {artist}").trim().to_owned(), bare.to_owned()] {
            if !queries.contains(&other) {
                queries.push(other);
            }
        }
        queries
    }

    fn likeness(&self, track: &Track) -> f32 {
        let title = alike(&bare(name(&self.title)), &bare(name(&track.title)));
        // Catalogs credit a song by several in their own order: any of ours among theirs will do.
        let theirs = bare(&track.artist);
        let mut ours = self.artist.split([',', '&']).map(bare).filter(|one| !one.is_empty()).peekable();
        let artist = if ours.peek().is_none() { 0.5 } else { f32::from(ours.any(|one| theirs.contains(&one) || alike(&one, &theirs) >= 0.8)) };
        // Catalogs trim silence differently; past a few seconds it is another cut.
        let length = match self.duration_ms.abs_diff(track.duration_ms) / 1000 {
            _ if self.duration_ms == 0 || track.duration_ms == 0 => 0.5,
            0..=2 => 1.0,
            3..=5 => 0.85,
            6..=10 => 0.55,
            11..=20 => 0.2,
            _ => 0.0,
        };
        let (ours, theirs) = (self.title.to_lowercase(), track.title.to_lowercase());
        let other = RENDITIONS.iter().filter(|words| words.iter().any(|word| ours.contains(word)) != words.iter().any(|word| theirs.contains(word))).count();
        // Of the very length wanted, it is the recording, whatever its title leaves unsaid.
        let other = if length == 1.0 { 0 } else { other };
        title * 0.4 + length * 0.25 + artist * 0.35 - 0.3 * other as f32
    }
}

/// Which of the catalog's answers is the song wanted: the one most like it by name, by
/// length and by whose it is, when one is like it enough.
pub fn pick<'a>(wanted: &Wanted, found: &'a [Track]) -> Option<&'a Track> {
    let best = found.iter().map(|track| (wanted.likeness(track), track)).max_by(|a, b| a.0.total_cmp(&b.0))?;
    (best.0 >= CLOSE_ENOUGH).then_some(best.1)
}

/// A list as text, a song to a line: what the services that carry lists between apps read.
pub fn as_text(tracks: &[Track]) -> String {
    tracks.iter().map(|track| format!("{} - {}", track.artist, track.title)).collect::<Vec<_>>().join("\n")
}

/// A name as two catalogs' spellings of it are compared: `plain`, and without its accents.
fn bare(name: &str) -> String {
    let unaccented = |c| match c {
        'á' | 'à' | 'ä' | 'â' | 'ã' => 'a',
        'é' | 'è' | 'ë' | 'ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' | 'õ' => 'o',
        'ú' | 'ù' | 'ü' | 'û' => 'u',
        'ñ' => 'n',
        'ç' => 'c',
        other => other,
    };
    plain(name).chars().map(unaccented).collect()
}

/// The first of several artists written as one text.
fn first(artists: &str) -> &str {
    artists.split([',', '&']).next().unwrap_or(artists).trim()
}

/// A title without what follows its name: `(feat. …)`, `- Remastered 2011`.
fn name(title: &str) -> &str {
    let end = [" (", " [", " - "].into_iter().filter_map(|mark| title.find(mark)).min().filter(|end| *end > 0);
    end.map_or(title, |end| &title[..end])
}

/// How alike two texts are, from 0 to 1, by the pairs of letters they share: a word more
/// or a letter off costs little.
fn alike(a: &str, b: &str) -> f32 {
    let pairs = |text: &str| text.chars().zip(text.chars().skip(1)).collect::<Vec<_>>();
    let (ours, mut theirs) = (pairs(a), pairs(b));
    if a == b {
        return 1.0;
    }
    let whole = ours.len() + theirs.len();
    let shared = ours.iter().filter(|pair| theirs.iter().position(|other| other == *pair).map(|at| theirs.swap_remove(at)).is_some()).count();
    if whole == 0 { 0.0 } else { 2.0 * shared as f32 / whole as f32 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str, title: &str, artist: &str, seconds: u64) -> Track {
        Track { id: id.into(), title: title.into(), artist: artist.into(), artist_id: None, album: None, album_id: None, artwork_url: None, duration_ms: seconds * 1000 }
    }

    #[test]
    fn a_link_of_another_app_names_its_list() {
        let spotify = |kind, id: &str| Some(Source::Spotify(kind, id.into()));
        assert_eq!(source(" https://open.spotify.com/intl-es/playlist/37i9dQZF1DXcBWIGoYBM5M?si=abc "), spotify("playlist", "37i9dQZF1DXcBWIGoYBM5M"));
        assert_eq!(source("https://open.spotify.com/album/4aawyAB9vmqN3uQ7FjRGTy"), spotify("album", "4aawyAB9vmqN3uQ7FjRGTy"));
        assert_eq!(source("https://music.apple.com/us/playlist/todays-hits/pl.f4d1?uo=4"), Some(Source::Apple("https://music.apple.com/us/playlist/todays-hits/pl.f4d1".into())));
        assert_eq!(source("https://music.youtube.com/playlist?list=PL_a-b1&si=x"), Some(Source::YouTube("PL_a-b1".into())));
        assert_eq!(source("https://www.youtube.com/watch?v=abc&list=OLAK5uy_x"), Some(Source::YouTube("OLAK5uy_x".into())));
        for not_one in ["https://music.apple.com/us/artist/zoe/123", "https://open.spotify.com/track/abc", "https://open.spotify.com/playlist/", "https://music.youtube.com/watch?v=abc", "labios rotos", "surco://lista/a?nombre=x"] {
            assert_eq!(source(not_one), None, "{not_one}");
        }
    }

    #[test]
    fn the_song_wanted_is_the_answer_most_like_it_and_none_when_none_is() {
        let wanted = Wanted { title: "Labios Rotos - Remastered 2011".into(), artist: "Zoé, Otro".into(), duration_ms: 243_000 };
        assert_eq!(wanted.queries(), ["Zoé Labios Rotos - Remastered 2011", "Labios Rotos Zoé", "Labios Rotos"]);
        assert_eq!(Wanted { title: "Soñé".into(), artist: String::new(), duration_ms: 0 }.queries(), ["Soñé"]);
        let found = [
            track("live", "Labios Rotos (En Vivo)", "Zoé", 262),
            track("cover", "Labios Rotos", "Banda Tributo", 243),
            track("own", "Labios Rotos", "Zoé", 244),
            track("other", "Soñé", "Zoé", 243),
        ];
        assert_eq!(pick(&wanted, &found).map(|track| track.id.as_str()), Some("own"));
        assert_eq!(pick(&wanted, &found[..1]), None, "another rendition is not the song");
        assert_eq!(pick(&wanted, &found[1..2]), None, "nor is somebody else's, of the same name and length");
        let unsaid = Wanted { title: "Labios Rotos".into(), artist: "Zoé".into(), duration_ms: 262_000 };
        assert_eq!(pick(&unsaid, &found).map(|track| track.id.as_str()), Some("live"), "a live cut of the very length is the one, though its name does not say so");
        let spelt = Wanted { title: "Labios Rotos".into(), artist: "Otro & Zoe".into(), duration_ms: 0 };
        assert_eq!(pick(&spelt, &found).map(|track| track.id.as_str()), Some("own"), "an accent or the order of its artists apart");
        assert_eq!(pick(&wanted, &found[3..]), None);
        let live = Wanted { title: "Labios Rotos - En Vivo".into(), artist: "Zoé".into(), duration_ms: 0 };
        assert_eq!(pick(&live, &found).map(|track| track.id.as_str()), Some("live"));
        assert_eq!(pick(&wanted, &[]), None);
    }

    #[test]
    fn a_list_goes_out_as_a_line_for_each_song() {
        assert_eq!(as_text(&[track("a", "Soñé", "Zoé", 1), track("b", "K.", "Cigarettes After Sex", 1)]), "Zoé - Soñé\nCigarettes After Sex - K.");
        assert_eq!(as_text(&[]), "");
    }
}
