//! Picking what a home page offers out of what its user plays. No I/O: the lists a pick is
//! made from are the catalog's answers, handed in.

use std::collections::{HashMap, HashSet};

use crate::Track;

/// How many plays back a play counts for half of the latest one.
const RECENT: f32 = 20.0;
/// What a like counts for, next to the latest play.
const LIKED: f32 = 1.5;
/// The most tracks one artist has in a blend.
const PER_ARTIST: usize = 2;

/// A name as two of them are compared: its letters and digits, in lowercase.
pub fn plain(name: &str) -> String {
    name.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// A track for each of the artists played most, the most played first. `heard` is newest
/// first, and a play counts for less the further back it is.
pub fn seeds(heard: &[Track], liked: &[Track], count: usize) -> Vec<Track> {
    let mut weight: HashMap<String, f32> = HashMap::new();
    // Each artist's latest track, in the order the artists were last played.
    let mut latest: Vec<(String, &Track)> = Vec::new();
    let plays = heard.iter().enumerate().map(|(age, track)| (track, 1.0 / (1.0 + age as f32 / RECENT)));
    for (track, counts) in plays.chain(liked.iter().map(|track| (track, LIKED))) {
        let artist = plain(&track.artist);
        *weight.entry(artist.clone()).or_default() += counts;
        if latest.iter().all(|(known, _)| *known != artist) {
            latest.push((artist, track));
        }
    }
    // Stable: between two played as much, the one played last comes first.
    latest.sort_by(|(a, _), (b, _)| weight[b].total_cmp(&weight[a]));
    latest.into_iter().take(count).map(|(_, track)| track.clone()).collect()
}

/// One list out of several: a track from each in turn, so that none of them leads, without
/// the ones the user already has and without an artist's third.
pub fn blend(lists: &[Vec<Track>], known: &HashSet<String>, len: usize) -> Vec<Track> {
    let mut picked: Vec<Track> = Vec::new();
    let mut by_artist: HashMap<String, usize> = HashMap::new();
    for rank in 0..lists.iter().map(Vec::len).max().unwrap_or(0) {
        for track in lists.iter().filter_map(|list| list.get(rank)) {
            if picked.len() == len {
                return picked;
            }
            if known.contains(&track.id) || picked.iter().any(|taken| taken.id == track.id) {
                continue;
            }
            let count = by_artist.entry(plain(&track.artist)).or_default();
            if *count < PER_ARTIST {
                *count += 1;
                picked.push(track.clone());
            }
        }
    }
    picked
}

/// The latest plays of `heard` (newest first), one for each artist, the latest first: what
/// the user has been listening to lately, however much of it.
pub fn latest_by_artist(heard: &[Track], count: usize) -> Vec<Track> {
    let mut latest: Vec<&Track> = Vec::new();
    for track in heard {
        if latest.len() == count {
            break;
        }
        if latest.iter().all(|known| plain(&known.artist) != plain(&track.artist)) {
            latest.push(track);
        }
    }
    latest.into_iter().cloned().collect()
}

/// The tracks `heard` (newest first) holds more than once, the ones played most first and,
/// between two played as much, the one played last.
pub fn on_repeat(heard: &[Track], len: usize) -> Vec<Track> {
    let mut plays: Vec<(&Track, usize)> = Vec::new();
    for track in heard {
        match plays.iter_mut().find(|(known, _)| known.id == track.id) {
            Some((_, times)) => *times += 1,
            None => plays.push((track, 1)),
        }
    }
    // Stable, so the order of the latest plays settles a tie.
    plays.sort_by_key(|(_, times)| std::cmp::Reverse(*times));
    plays.into_iter().filter(|(_, times)| *times > 1).take(len).map(|(track, _)| track.clone()).collect()
}

/// The artists in `lists` that are not among `artists` (written as `plain` writes them),
/// the ones named most first, each as the lists name it.
pub fn newcomers(lists: &[Vec<Track>], artists: &HashSet<String>, count: usize) -> Vec<String> {
    let mut named: Vec<(String, &str, usize)> = Vec::new();
    for track in lists.iter().flatten() {
        let artist = plain(&track.artist);
        if artist.is_empty() || artists.contains(&artist) {
            continue;
        }
        match named.iter_mut().find(|(known, ..)| *known == artist) {
            Some((.., times)) => *times += 1,
            None => named.push((artist, &track.artist, 1)),
        }
    }
    named.sort_by_key(|(.., times)| std::cmp::Reverse(*times));
    named.into_iter().take(count).map(|(_, name, _)| name.to_owned()).collect()
}

/// Whether the release tagged `tag` (`v1.4.0`, `1.4`) comes after version `current`.
/// A tag that is not a version comes after nothing.
pub fn newer(tag: &str, current: &str) -> bool {
    let numbers = |version: &str| version.trim().trim_start_matches('v').split('.').map(|part| part.parse::<u64>().ok()).collect::<Option<Vec<u64>>>();
    match (numbers(tag), numbers(current)) {
        (Some(mut tag), Some(mut current)) => {
            // "1.4" is "1.4.0".
            let parts = tag.len().max(current.len());
            tag.resize(parts, 0);
            current.resize(parts, 0);
            tag > current
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_is_newer_by_its_numbers_not_by_its_letters() {
        assert!(newer("v0.2.0", "0.1.9") && newer("0.10.0", "0.9.0") && newer("v1.0", "0.9.9") && newer("1.0.1", "1.0"));
        assert!(!newer("v0.1.0", "0.1.0") && !newer("0.1", "0.1.0") && !newer("nightly", "0.1.0") && !newer("v0.0.9", "0.1.0"));
    }

    fn track(id: &str, artist: &str) -> Track {
        Track { id: id.into(), title: id.into(), artist: artist.into(), artist_id: None, album: None, album_id: None, artwork_url: None, duration_ms: 0 }
    }

    fn ids(tracks: &[Track]) -> Vec<&str> {
        tracks.iter().map(|track| track.id.as_str()).collect()
    }

    #[test]
    fn the_latest_plays_are_one_for_each_artist_newest_first() {
        let heard = [track("a1", "Ana"), track("a2", "ANA"), track("b1", "Beto"), track("c1", "Caro"), track("b2", "Beto")];
        assert_eq!(ids(&latest_by_artist(&heard, 5)), ["a1", "b1", "c1"]);
        assert_eq!(ids(&latest_by_artist(&heard, 2)), ["a1", "b1"]);
        assert!(latest_by_artist(&[], 3).is_empty());
    }

    #[test]
    fn on_repeat_is_what_was_played_again_the_most_played_first() {
        let heard = [track("a", "Ana"), track("b", "Beto"), track("a", "Ana"), track("c", "Caro"), track("b", "Beto"), track("b", "Beto"), track("d", "Dani"), track("d", "Dani")];
        assert_eq!(ids(&on_repeat(&heard, 5)), ["b", "a", "d"], "three plays first, then the tie in the order last played; once is not a repeat");
        assert_eq!(ids(&on_repeat(&heard, 1)), ["b"]);
        assert!(on_repeat(&[track("a", "Ana")], 5).is_empty());
    }

    #[test]
    fn seeds_are_the_latest_track_of_each_artist_the_most_played_first() {
        let heard = [track("a1", "Ana"), track("b1", "Beto"), track("b2", "BETO"), track("c1", "Caro"), track("b3", "Beto")];
        assert_eq!(ids(&seeds(&heard, &[], 5)), ["b1", "a1", "c1"], "three plays outweigh the latest one");
        assert_eq!(ids(&seeds(&heard, &[track("c2", "Caro"), track("c3", "Caro")], 2)), ["c1", "b1"], "likes count, and the seed is still the latest play");
        assert!(seeds(&[], &[], 5).is_empty());
    }

    #[test]
    fn a_blend_takes_from_each_list_in_turn_and_no_artist_more_than_twice() {
        let first = vec![track("s", "Seed"), track("x1", "Xul"), track("x2", "Xul"), track("x3", "Xul"), track("y1", "Yma")];
        let second = vec![track("x1", "Xul"), track("z1", "Zoé"), track("z2", "Zoé")];
        let known = HashSet::from(["s".to_owned()]);
        assert_eq!(ids(&blend(&[first.clone(), second.clone()], &known, 10)), ["x1", "z1", "x2", "z2", "y1"]);
        assert_eq!(ids(&blend(&[first, second], &known, 2)), ["x1", "z1"]);
    }

    #[test]
    fn newcomers_are_the_artists_not_played_yet_the_most_named_first() {
        let lists = [vec![track("1", "Zoé"), track("2", "Mazzy Star"), track("3", "Beach House")], vec![track("4", "Beach House"), track("5", "zoe")]];
        let played = HashSet::from([plain("Zoé")]);
        assert_eq!(newcomers(&lists, &played, 5), ["Beach House", "Mazzy Star", "zoe"], "an accent makes another name");
        assert_eq!(newcomers(&lists, &played, 1), ["Beach House"]);
    }
}
