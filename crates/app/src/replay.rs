//! What was listened to, summed up for Replay, as Opentune's Wrapped does: how long, who and
//! what most, when in the day, the busiest day, the longest run of days and the song played
//! most in a single day. Everything comes from the plays kept on the device.

use std::collections::HashMap;

use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, Timelike};
use serde::{Deserialize, Serialize};
use surco_core::Track;

/// A song listened to: from when, and for how many seconds really.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Play {
    pub track: Track,
    /// When it started, in seconds since 1970.
    pub at: i64,
    pub seconds: u32,
}

/// How long a song is listened to before it counts as played: 30 seconds, or half of a
/// shorter one.
pub fn counts_after(duration: f64) -> f64 {
    if duration > 0.0 { (duration / 2.0).min(30.0) } else { 30.0 }
}

/// The stretch Replay looks over.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Period {
    #[default]
    Month,
    Year,
    All,
}

impl Period {
    pub const ALL: [Period; 3] = [Period::Month, Period::Year, Period::All];

    pub fn slug(self) -> &'static str {
        match self { Period::Month => "month", Period::Year => "year", Period::All => "all" }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|period| period.slug() == slug)
    }

    /// When it starts, seen from `now`: the first of the month, of the year, or ever.
    fn since(self, now: DateTime<FixedOffset>) -> i64 {
        let day = match self {
            Period::Month => now.date_naive().with_day(1),
            Period::Year => now.date_naive().with_ordinal(1),
            Period::All => return i64::MIN,
        };
        day.and_then(|day| day.and_hms_opt(0, 0, 0)).and_then(|start| start.and_local_timezone(*now.offset()).single())
            .map_or(i64::MIN, |start| start.timestamp())
    }
}

/// A song or an artist and how much of it was heard.
#[derive(Clone, Debug, PartialEq)]
pub struct Ranked {
    pub track: Track,
    pub plays: usize,
    pub minutes: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Summary {
    pub minutes: u64,
    pub plays: usize,
    pub songs: usize,
    pub artists: usize,
    pub top_songs: Vec<Ranked>,
    /// Each by the track of theirs heard most, which lends its picture.
    pub top_artists: Vec<Ranked>,
    /// Minutes in each hour of the day.
    pub hours: [u64; 24],
    /// The same of each of the artists heard most, the first few, in their order; what is
    /// left of `hours` is everyone else's.
    pub hours_of: Vec<[u64; 24]>,
    pub busiest_day: Option<(NaiveDate, u64)>,
    /// The most days in a row with something heard.
    pub streak: u32,
    /// The song played most times in one day, when it was more than once.
    pub on_repeat: Option<(Track, usize)>,
}

/// How many songs and artists the tops hold.
const TOP: usize = 10;
/// How many of the artists heard most have hours of their own, the rest summed.
const STACKED: usize = 3;

/// A step of Replay told as a story, as Wrapped's are: one thing at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Intro,
    Minutes,
    TopArtist,
    Songs,
    Artists,
    Hours,
    Facts,
    Tops,
}

impl Summary {
    /// The steps there is something to tell in, in the order they are told; none without plays.
    pub fn story(&self) -> Vec<Step> {
        if self.plays == 0 { return Vec::new(); }
        let artists = !self.top_artists.is_empty();
        let facts = self.streak > 1 || self.busiest_day.is_some() || self.on_repeat.is_some();
        [(Step::Intro, true), (Step::Minutes, true), (Step::TopArtist, artists), (Step::Songs, !self.top_songs.is_empty()),
            (Step::Artists, artists), (Step::Hours, true), (Step::Facts, facts), (Step::Tops, true)]
            .into_iter().filter_map(|(step, told)| told.then_some(step)).collect()
    }
}

/// The plays of `period` seen from `now`, summed up.
pub fn summarize(plays: &[Play], period: Period, now: DateTime<FixedOffset>) -> Summary {
    let since = period.since(now);
    let zone = *now.offset();
    let window: Vec<&Play> = plays.iter().filter(|play| play.at >= since && play.at <= now.timestamp()).collect();
    let local = |play: &Play| DateTime::from_timestamp(play.at, 0).map(|at| at.with_timezone(&zone));
    let minutes = |list: &[&Play]| list.iter().map(|play| u64::from(play.seconds)).sum::<u64>() / 60;
    // Played most first; as often, heard longest.
    let rank = |groups: HashMap<String, Vec<&Play>>| {
        let mut ranked: Vec<Ranked> = groups.into_values().map(|list| Ranked {
            track: list[0].track.clone(), plays: list.len(), minutes: minutes(&list),
        }).collect();
        ranked.sort_by(|a, b| b.plays.cmp(&a.plays).then(b.minutes.cmp(&a.minutes)).then(a.track.title.cmp(&b.track.title)));
        ranked
    };
    let mut by_song: HashMap<String, Vec<&Play>> = HashMap::new();
    let mut by_artist: HashMap<String, Vec<&Play>> = HashMap::new();
    let mut by_day: HashMap<NaiveDate, Vec<&Play>> = HashMap::new();
    let mut hours = [0u64; 24];
    for play in &window {
        by_song.entry(play.track.id.clone()).or_default().push(play);
        let artist = main_artist(&play.track.artist);
        if !artist.is_empty() { by_artist.entry(artist.to_lowercase()).or_default().push(play); }
        if let Some(at) = local(play) {
            hours[at.hour() as usize] += u64::from(play.seconds);
            by_day.entry(at.date_naive()).or_default().push(play);
        }
    }
    let songs = by_song.len();
    let artists = by_artist.len();
    let on_repeat = by_day.values()
        .flat_map(|list| {
            let mut counts: HashMap<&str, (usize, &Play)> = HashMap::new();
            for play in list { counts.entry(play.track.id.as_str()).or_insert((0, play)).0 += 1; }
            counts.into_values().collect::<Vec<_>>()
        })
        .filter(|(count, _)| *count >= 2)
        .max_by(|a, b| a.0.cmp(&b.0).then(b.1.at.cmp(&a.1.at)))
        .map(|(count, play)| (play.track.clone(), count));
    let busiest_day = by_day.iter().map(|(day, list)| (*day, minutes(list))).max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)));
    let mut days: Vec<NaiveDate> = by_day.keys().copied().collect();
    days.sort();
    let (mut streak, mut run) = (0u32, 0u32);
    for (index, day) in days.iter().enumerate() {
        run = if index > 0 && days[index - 1].succ_opt() == Some(*day) { run + 1 } else { 1 };
        streak = streak.max(run);
    }
    let mut top_artists = rank(by_artist);
    for artist in &mut top_artists { artist.track.artist = main_artist(&artist.track.artist).to_owned(); }
    top_artists.truncate(TOP);
    let mut top_songs = rank(by_song);
    top_songs.truncate(TOP);
    let hours_of = top_artists.iter().take(STACKED).map(|artist| {
        let name = artist.track.artist.to_lowercase();
        let mut seconds = [0u64; 24];
        for play in window.iter().filter(|play| main_artist(&play.track.artist).to_lowercase() == name) {
            if let Some(at) = local(play) { seconds[at.hour() as usize] += u64::from(play.seconds); }
        }
        seconds.map(|seconds| seconds / 60)
    }).collect();
    Summary {
        minutes: minutes(&window),
        plays: window.len(),
        songs,
        artists,
        top_songs,
        top_artists,
        hours: hours.map(|seconds| seconds / 60),
        hours_of,
        busiest_day,
        streak,
        on_repeat,
    }
}

/// The first of the artists a song's credit names.
fn main_artist(credit: &str) -> &str {
    let mut first = credit;
    // Not " y " or " & ": "Ciro y los Persas" and "Simon & Garfunkel" are one each.
    for separator in [", ", " feat. ", " ft. ", " featuring "] {
        if let Some((before, _)) = first.split_once(separator) { first = before; }
    }
    first.trim()
}

/// The part of the day most was heard in, by the hour it peaks at: night, morning, day or
/// evening, as Opentune names them.
pub fn part_of_day(hours: &[u64; 24]) -> Option<&'static str> {
    let parts: [(&str, &[usize]); 4] = [
        ("common-replay-night", &[22, 23, 0, 1, 2, 3, 4]),
        ("common-replay-morning", &[5, 6, 7, 8, 9, 10]),
        ("common-replay-day", &[11, 12, 13, 14, 15, 16]),
        ("common-replay-evening", &[17, 18, 19, 20, 21]),
    ];
    parts.iter().map(|(key, part)| (*key, part.iter().map(|hour| hours[*hour]).sum::<u64>()))
        .filter(|(_, minutes)| *minutes > 0).max_by_key(|(_, minutes)| *minutes).map(|(key, _)| key)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_story_tells_only_what_there_is_to_tell() {
        use super::Step::*;
        assert!(super::Summary::default().story().is_empty(), "nothing heard, nothing told");
        let lone = super::Summary { plays: 1, ..Default::default() };
        assert_eq!(lone.story(), [Intro, Minutes, Hours, Tops]);
    }

    use super::*;

    fn track(id: &str, artist: &str) -> Track {
        Track { id: id.into(), title: id.into(), artist: artist.into(), artist_id: None, album: None, album_id: None, artwork_url: None, duration_ms: 200_000 }
    }

    fn at(day: u32, hour: u32) -> i64 {
        // In Buenos Aires, three hours behind.
        chrono::NaiveDate::from_ymd_opt(2026, 10, day).unwrap().and_hms_opt(hour, 0, 0).unwrap().and_utc().timestamp() + 3 * 3600
    }

    #[test]
    fn a_month_of_plays_is_summed_up_who_and_what_most_and_when() {
        let zone = FixedOffset::west_opt(3 * 3600).unwrap();
        let now = DateTime::from_timestamp(at(21, 12), 0).unwrap().with_timezone(&zone);
        let play = |id: &str, artist: &str, at: i64, seconds: u32| Play { track: track(id, artist), at, seconds };
        let plays = vec![
            play("old", "Cerati", self::at(1, 10) - 40 * 86_400, 600),
            play("a", "Soda Stereo", self::at(18, 22), 240),
            play("a", "Soda Stereo", self::at(19, 23), 240),
            play("a", "Soda Stereo", self::at(19, 23), 240),
            play("b", "Cerati, Soda Stereo", self::at(20, 9), 180),
            play("c", "Cerati", self::at(20, 22), 120),
        ];
        let summary = summarize(&plays, Period::Month, now);
        assert_eq!((summary.plays, summary.songs, summary.artists, summary.minutes), (5, 3, 2, 17));
        assert_eq!(summary.top_songs[0].track.id, "a");
        assert_eq!((summary.top_songs[0].plays, summary.top_songs[0].minutes), (3, 12));
        assert_eq!(summary.top_artists.iter().map(|artist| (artist.track.artist.as_str(), artist.plays)).collect::<Vec<_>>(), [("Soda Stereo", 3), ("Cerati", 2)]);
        assert_eq!(summary.streak, 3, "the 18th to the 20th");
        assert_eq!(summary.on_repeat.as_ref().map(|(track, count)| (track.id.as_str(), *count)), Some(("a", 2)));
        assert_eq!(summary.busiest_day.map(|(day, _)| day.day()), Some(19));
        assert_eq!(summary.hours[23], 8, "in local hours");
        // Each artist heard most has their own hours, never more than all of them together.
        assert_eq!(summary.hours_of.len(), 2);
        assert!((0..24).all(|hour| summary.hours_of.iter().map(|hours| hours[hour]).sum::<u64>() <= summary.hours[hour]));
        assert_eq!(part_of_day(&summary.hours), Some("common-replay-night"));
        assert_eq!(summarize(&plays, Period::All, now).plays, 6);
        assert_eq!(summarize(&[], Period::Year, now), Summary::default());
    }

    #[test]
    fn a_song_counts_as_played_after_thirty_seconds_or_half_a_short_one() {
        assert_eq!(counts_after(200.0), 30.0);
        assert_eq!(counts_after(40.0), 20.0);
        assert_eq!(counts_after(0.0), 30.0);
        assert_eq!(main_artist("Cerati, Soda Stereo"), "Cerati");
        assert_eq!(main_artist("Ciro y los Persas"), "Ciro y los Persas");
        assert_eq!(main_artist("Bizarrap feat. Shakira"), "Bizarrap");
    }
}
