//! Synced lyrics: the lines of a song and when each one starts.

/// A line and the moment it starts being sung.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub at_ms: u64,
    pub text: String,
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
            (!text.is_empty()).then(|| Line { at_ms: at.max(0) as u64, text })
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
    fn the_active_line_is_the_last_that_started() {
        let lines = parse_lrc("[00:05.00]a\n[00:10.00]b");
        assert_eq!(active(&lines, 4_999), None);
        assert_eq!(active(&lines, 5_000), Some(0));
        assert_eq!(active(&lines, 60_000), Some(1));
        assert_eq!(active(&[], 0), None);
    }
}
