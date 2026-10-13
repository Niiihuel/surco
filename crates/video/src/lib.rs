//! A song's short video. YouTube serves a picture-only rendition as fragmented MP4: a header
//! that says how to decode, an index of where each few seconds lie, and the fragments
//! themselves. This reads the three apart and decodes the fragments to frames; fetching them
//! is the caller's.

mod boxes;
mod decode;

pub use boxes::{fragments, header, samples, Fragment, Header, Sample};
pub use decode::{Decoder, Frame};

/// A stretch of a video ready to be played over and over: what the decoder is given
/// first, and then its pictures.
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub parameters: Vec<u8>,
    pub samples: Vec<Sample>,
}

/// A whole video as it is fetched a few seconds at a time: how to decode it, and where
/// each of its fragments lies.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Film {
    pub header: Header,
    pub fragments: Vec<Fragment>,
    pub width: usize,
    pub height: usize,
}

impl Film {
    /// The fragment that holds the picture for `seconds` into the video, and when that
    /// fragment starts. None past the end.
    pub fn at(&self, seconds: f64) -> Option<(usize, f64)> {
        fragment_at(&self.fragments, seconds)
    }

    pub fn seconds(&self) -> f64 {
        self.fragments.iter().map(|fragment| fragment.seconds).sum()
    }
}

fn fragment_at(fragments: &[Fragment], seconds: f64) -> Option<(usize, f64)> {
    let mut starts = 0.0;
    let index = fragments.iter().position(|fragment| {
        let holds = seconds < starts + fragment.seconds;
        if !holds {
            starts += fragment.seconds;
        }
        holds
    })?;
    Some((index, starts))
}

/// The first and last byte to fetch for a clip of about `seconds`. It starts a third of
/// the way in, where a video has left its opening behind, and takes whole fragments,
/// since each begins with a picture that stands alone.
pub fn stretch(fragments: &[Fragment], seconds: f64) -> Option<(u64, u64)> {
    let total: f64 = fragments.iter().map(|fragment| fragment.seconds).sum();
    let mut at = 0.0;
    // A video too short to have a fragment that late is taken from its start.
    let from = fragments
        .iter()
        .position(|fragment| {
            let starts = at;
            at += fragment.seconds;
            starts >= total / 3.0
        })
        .unwrap_or(0);
    let mut taken = 0.0;
    let chosen: Vec<&Fragment> = fragments[from..]
        .iter()
        .take_while(|fragment| {
            let more = taken < seconds;
            taken += fragment.seconds;
            more
        })
        .collect();
    let (first, last) = (chosen.first()?, chosen.last()?);
    Some((first.start, last.start + last.size.checked_sub(1)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clip_takes_whole_fragments_from_a_third_of_the_way_in() {
        let fragments: Vec<Fragment> = (0..9).map(|index| Fragment { start: 1000 + index * 100, size: 100, seconds: 5.0 }).collect();
        // 45 s in all: the fourth fragment starts at 15 s, and two of them make 8 s.
        assert_eq!(stretch(&fragments, 8.0), Some((1300, 1499)));
        assert_eq!(stretch(&fragments[..1], 8.0), Some((1000, 1099)), "a video of one fragment gives it");
        assert_eq!(stretch(&[], 8.0), None);
    }

    #[test]
    fn a_moment_of_a_film_is_in_the_fragment_that_starts_before_it() {
        let fragments: Vec<Fragment> = (0..3).map(|index| Fragment { start: index * 100, size: 100, seconds: 5.0 }).collect();
        assert_eq!(fragment_at(&fragments, 0.0), Some((0, 0.0)));
        assert_eq!(fragment_at(&fragments, 7.5), Some((1, 5.0)));
        assert_eq!(fragment_at(&fragments, 15.0), None, "past its end");
    }
}
