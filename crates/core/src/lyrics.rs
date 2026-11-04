//! Synced lyrics: the lines of a song and when each one starts, and its words when they are
//! timed too.

/// A line and the moment it starts being sung.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub at_ms: u64,
    pub text: String,
    /// Its words and when each is sung, `text` being them joined by spaces; none when only
    /// the line is timed.
    pub words: Vec<Word>,
}

/// A word of a line and when it is sung.
#[derive(Clone, Debug, PartialEq)]
pub struct Word {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

/// Glues timed runs, syllables or words as a provider wrote them, `(start, end, text)`, into
/// words, as BitChord's do: a run is the front of a word that carries on when it neither ends
/// in whitespace nor is followed by one that starts with it, the way Apple's and QQ's
/// syllables are written, and a blank run ends the word in hand. A writer that spaces nothing,
/// and scripts written without spaces, keep each run a word.
pub fn words_from_runs(runs: &[(u64, u64, &str)]) -> Vec<Word> {
    let unspaced = |c: char| matches!(c, '\u{3000}'..='\u{303F}' | '\u{3040}'..='\u{30FF}' | '\u{3400}'..='\u{9FFF}' | '\u{FF00}'..='\u{FFEF}' | '\u{0E00}'..='\u{0E7F}');
    let glue = runs.iter().any(|(_, _, raw)| raw.contains(char::is_whitespace));
    let mut words: Vec<Word> = Vec::new();
    let mut open = false;
    for &(start, end, raw) in runs {
        let content = raw.trim();
        if content.is_empty() {
            open = false;
            continue;
        }
        let joins = open && glue && !raw.starts_with(char::is_whitespace)
            && words.last().is_some_and(|word| !word.text.ends_with(unspaced)) && !content.starts_with(unspaced);
        match words.last_mut().filter(|_| joins) {
            Some(word) => {
                word.text.push_str(content);
                word.end_ms = word.end_ms.max(end);
            }
            None => words.push(Word { start_ms: start, end_ms: end.max(start), text: content.to_owned() }),
        }
        open = glue && !raw.ends_with(char::is_whitespace);
    }
    words
}

/// How far into its text the singing of `line` has got at `at_ms`, in characters, as
/// BitChord's: within a word across its own span, so a held note draws slowly and a quick one
/// snaps, and the space between two words across the rest between them. A line timed only
/// as a whole is sung all at once.
pub fn revealed(line: &Line, at_ms: u64) -> f32 {
    let length = line.text.chars().count() as f32;
    if line.words.is_empty() {
        return if at_ms >= line.at_ms { length } else { 0.0 };
    }
    let share = |from: u64, to: u64| if to > from { (at_ms.saturating_sub(from) as f32 / (to - from) as f32).min(1.0) } else { 1.0 };
    let mut start = 0.0;
    for (index, word) in line.words.iter().enumerate() {
        let end = start + word.text.chars().count() as f32;
        if at_ms < word.start_ms {
            return start;
        }
        if at_ms < word.end_ms {
            return start + share(word.start_ms, word.end_ms) * (end - start);
        }
        if let Some(next) = line.words.get(index + 1).filter(|next| at_ms < next.start_ms) {
            return end + share(word.end_ms, next.start_ms);
        }
        start = end + 1.0;
    }
    length
}

/// `m:ss`, `m:ss.xx` or `m:ss.xxx` in milliseconds.
fn stamp(text: &str) -> Option<u64> {
    let (minutes, rest) = text.split_once(':')?;
    let (seconds, fraction) = rest.split_once(['.', ':']).unwrap_or((rest, ""));
    // The fraction may be in hundredths (`.85`) or thousandths (`.850`).
    let fraction = if fraction.is_empty() { 0 } else { format!("{fraction:0<3}").get(..3)?.parse::<u64>().ok()? };
    Some(minutes.parse::<u64>().ok()? * 60_000 + seconds.parse::<u64>().ok()? * 1000 + fraction)
}

/// The `[mm:ss.xx] text` lines of an LRC file, in the order they are sung. `[offset:±ms]`
/// shifts them all, earlier when positive as the format has it, and the `<mm:ss.xx>` word
/// marks of enhanced LRC are dropped. Lines without words are left out.
pub fn parse_lrc(lrc: &str) -> Vec<Line> {
    let tag = |line: &str| line.trim().strip_prefix('[').and_then(|rest| rest.split_once(']')).map(|(tag, text)| (tag.to_owned(), text.to_owned()));
    let offset = lrc.lines().filter_map(tag).find_map(|(tag, _)| tag.strip_prefix("offset:")?.trim().parse::<i64>().ok()).unwrap_or(0);
    let mut lines: Vec<Line> = lrc
        .lines()
        .filter_map(tag)
        .filter_map(|(tag, text)| {
            let at = stamp(&tag)? as i64 - offset;
            // A word mark is a stamp between angle brackets; anything else in them is text.
            let mut words = String::new();
            let mut rest = text.as_str();
            while let Some((before, after)) = rest.split_once('<') {
                words.push_str(before);
                match after.split_once('>') {
                    Some((mark, tail)) if stamp(mark).is_some() => rest = tail,
                    _ => {
                        words.push('<');
                        rest = after;
                    }
                }
            }
            words.push_str(rest);
            let text = words.split_whitespace().collect::<Vec<_>>().join(" ");
            (!text.is_empty()).then(|| Line { at_ms: at.max(0) as u64, text, words: Vec::new() })
        })
        .collect();
    lines.sort_by_key(|line| line.at_ms);
    lines
}

/// The line being sung at `at_ms`: the last one that has started.
pub fn active(lines: &[Line], at_ms: u64) -> Option<usize> {
    lines.partition_point(|line| line.at_ms <= at_ms).checked_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_lrc_file_becomes_its_lines_in_the_order_they_are_sung() {
        let lines = parse_lrc("[ar: someone]\n[00:12.50] second  line\n[00:01.5]first <00:02.00>line\n[00:20.00]\nnot a line\n[1:02:250] a < b\n");
        let read: Vec<(u64, &str)> = lines.iter().map(|line| (line.at_ms, line.text.as_str())).collect();
        assert_eq!(read, [(1_500, "first line"), (12_500, "second line"), (62_250, "a < b")]);
    }

    #[test]
    fn an_offset_moves_every_line_and_none_before_the_start() {
        let lines = parse_lrc("[offset: +2000]\n[00:01.00]a\n[00:05.00]b");
        assert_eq!(lines.iter().map(|line| line.at_ms).collect::<Vec<_>>(), [0, 3_000]);
    }

    #[test]
    fn syllables_glue_into_the_words_their_spacing_says() {
        let runs = [(100, 200, "Ti-"), (200, 300, "ri, "), (300, 400, "Daddy"), (400, 400, " "), (500, 600, "yeah")];
        let words = words_from_runs(&runs);
        let read: Vec<(u64, u64, &str)> = words.iter().map(|word| (word.start_ms, word.end_ms, word.text.as_str())).collect();
        assert_eq!(read, [(100, 300, "Ti-ri,"), (300, 400, "Daddy"), (500, 600, "yeah")]);
        let unspaced = words_from_runs(&[(0, 10, "a"), (10, 20, "b")]);
        assert_eq!(unspaced.len(), 2, "a writer that spaces nothing states no words");
        assert_eq!(words_from_runs(&[(0, 10, "我"), (10, 20, "爱 ")]).len(), 2, "Han is written without spaces");
    }

    #[test]
    fn the_singing_reveals_each_word_across_its_span_and_the_space_across_the_rest() {
        let words = words_from_runs(&[(1_000, 1_400, "uno "), (1_600, 2_000, "dos")]);
        let line = Line { at_ms: 1_000, text: "uno dos".into(), words };
        assert_eq!(revealed(&line, 900), 0.0);
        assert_eq!(revealed(&line, 1_200), 1.5, "half way through \"uno\"");
        assert_eq!(revealed(&line, 1_500), 3.5, "half way through the rest after it");
        assert_eq!(revealed(&line, 1_800), 5.5);
        assert_eq!(revealed(&line, 2_500), 7.0);
        let whole = Line { at_ms: 1_000, text: "uno dos".into(), words: Vec::new() };
        assert_eq!((revealed(&whole, 999), revealed(&whole, 1_000)), (0.0, 7.0), "timed as a whole, sung at once");
    }

    #[test]
    fn the_active_line_is_the_last_that_started() {
        let lines = parse_lrc("[00:05.00]a\n[00:10.00]b");
        assert_eq!(active(&lines, 4_999), None);
        assert_eq!(active(&lines, 5_000), Some(0));
        assert_eq!(active(&lines, 60_000), Some(1));
        assert_eq!(active(&[], 0), None);
    }
}
