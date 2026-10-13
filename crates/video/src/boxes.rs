//! The boxes of a fragmented MP4 that carries one H.264 track, as far as playing it needs.

/// The body of the first box of `kind` among the boxes laid end to end in `data`, and
/// where that box starts.
fn find<'a>(data: &'a [u8], kind: &[u8; 4]) -> Option<(usize, &'a [u8])> {
    let mut at = 0;
    while let Some(head) = data.get(at..at + 8) {
        let size = u32::from_be_bytes(head[..4].try_into().ok()?) as usize;
        // A size of 1 keeps the real one in the next eight bytes; 0 runs to the end.
        let (size, body) = match size {
            1 => (u64::from_be_bytes(data.get(at + 8..at + 16)?.try_into().ok()?) as usize, at + 16),
            0 => (data.len() - at, at + 8),
            size => (size, at + 8),
        };
        let end = at.checked_add(size).filter(|end| *end <= data.len() && *end >= body)?;
        if &head[4..8] == kind {
            return Some((at, &data[body..end]));
        }
        at = end;
    }
    None
}

/// The body of the box at the end of `path`, each box inside the one before it.
fn inside<'a>(data: &'a [u8], path: &[&[u8; 4]]) -> Option<&'a [u8]> {
    path.iter().try_fold(data, |data, kind| find(data, kind).map(|(_, body)| body))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn u64_at(data: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(data.get(at..at + 8)?.try_into().ok()?))
}

/// What the header of a rendition says about decoding it.
#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    /// The parameter sets the decoder is given before any picture, each with its start code.
    pub parameters: Vec<u8>,
    /// How many bytes carry the length of each unit of a sample.
    length_bytes: usize,
    /// Ticks in a second, and how many a sample lasts when its fragment does not say.
    timescale: u32,
    sample_ticks: u32,
}

const START: [u8; 4] = [0, 0, 0, 1];

/// Reads a rendition's header: its `moov`, up to the end of its `initRange`.
pub fn header(init: &[u8]) -> Option<Header> {
    let moov = inside(init, &[b"moov"])?;
    let media = inside(moov, &[b"trak", b"mdia"])?;
    let mdhd = inside(media, &[b"mdhd"])?;
    // Version 1 keeps its two dates in eight bytes each.
    let timescale = u32_at(mdhd, if *mdhd.first()? == 1 { 20 } else { 12 })?;
    let stsd = inside(media, &[b"minf", b"stbl", b"stsd"])?;
    // Past the table's own eight bytes, the entry's size and name, and the 78 bytes every
    // picture entry starts with, are the entry's boxes.
    let avcc = inside(stsd.get(8 + 8 + 78..)?, &[b"avcC"])?;
    let length_bytes = usize::from(avcc.get(4)? & 3) + 1;
    let mut parameters = Vec::new();
    let mut at = 5;
    // Sequence sets, counted in five bits, then picture sets, each behind its length.
    for mask in [0x1f, 0xff] {
        let count = avcc.get(at)? & mask;
        at += 1;
        for _ in 0..count {
            let length = usize::from(u16::from_be_bytes(avcc.get(at..at + 2)?.try_into().ok()?));
            parameters.extend(START);
            parameters.extend(avcc.get(at + 2..at + 2 + length)?);
            at += 2 + length;
        }
    }
    let sample_ticks = inside(moov, &[b"mvex", b"trex"]).and_then(|trex| u32_at(trex, 12)).unwrap_or(0);
    (timescale > 0 && !parameters.is_empty()).then_some(Header { parameters, length_bytes, timescale, sample_ticks })
}

/// A few seconds of the rendition: where its bytes are, and how long it lasts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fragment {
    pub start: u64,
    pub size: u64,
    pub seconds: f64,
}

/// Reads a rendition's index, its `sidx`: the bytes of its `indexRange`, which ends at
/// byte `index_end` of the file.
pub fn fragments(index: &[u8], index_end: u64) -> Vec<Fragment> {
    let read = || {
        let sidx = inside(index, &[b"sidx"])?;
        let timescale = f64::from(u32_at(sidx, 8)?);
        // After the earliest time comes how far past the index the first fragment starts.
        let (first, mut at) = if *sidx.first()? == 0 { (u64::from(u32_at(sidx, 16)?), 20) } else { (u64_at(sidx, 20)?, 28) };
        let count = usize::from(u16::from_be_bytes(sidx.get(at + 2..at + 4)?.try_into().ok()?));
        at += 4;
        let mut start = index_end + 1 + first;
        let mut list = Vec::with_capacity(count);
        for _ in 0..count {
            // The top bit says the reference is to another index, which YouTube does not use.
            let size = u64::from(u32_at(sidx, at)? & 0x7fff_ffff);
            let seconds = f64::from(u32_at(sidx, at + 4)?) / timescale;
            list.push(Fragment { start, size, seconds });
            start += size;
            at += 12;
        }
        (timescale > 0.0).then_some(list)
    };
    read().unwrap_or_default()
}

/// One picture as the decoder takes it, and how long it shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    /// Its units, each behind a start code.
    pub data: Vec<u8>,
    pub seconds: f64,
}

/// Reads the pictures out of fragments laid end to end, each a `moof` and its `mdat`.
pub fn samples(mut data: &[u8], header: &Header) -> Vec<Sample> {
    let mut list = Vec::new();
    while let Some((moof_at, moof)) = find(data, b"moof") {
        let fragment = &data[moof_at..];
        if read_fragment(fragment, moof, header, &mut list).is_none() {
            break;
        }
        // On to whatever follows this fragment's pictures.
        let Some((mdat_at, mdat)) = find(fragment, b"mdat") else { break };
        let after = mdat_at + (mdat.as_ptr() as usize - fragment[mdat_at..].as_ptr() as usize) + mdat.len();
        data = &fragment[after..];
    }
    list
}

/// The pictures of one fragment, which starts at its `moof`.
fn read_fragment(fragment: &[u8], moof: &[u8], header: &Header, list: &mut Vec<Sample>) -> Option<()> {
    let traf = inside(moof, &[b"traf"])?;
    let tfhd = inside(traf, &[b"tfhd"])?;
    let flags = u32_at(tfhd, 0)? & 0x00ff_ffff;
    // The fields a `tfhd` may carry, in order, each behind its flag.
    let mut at = 8;
    let mut field = |flag: u32, bytes: usize| {
        let value = (flags & flag != 0).then(|| if bytes == 8 { u64_at(tfhd, at) } else { u32_at(tfhd, at).map(u64::from) });
        at += if flags & flag != 0 { bytes } else { 0 };
        value
    };
    // Offsets count from the `moof` unless a base is given, which YouTube does not do.
    if field(0x01, 8).is_some() {
        return None;
    }
    field(0x02, 4);
    let ticks = field(0x08, 4).flatten().map_or(header.sample_ticks, |ticks| ticks as u32);
    let size = field(0x10, 4).flatten().map_or(0, |size| size as u32);

    let trun = inside(traf, &[b"trun"])?;
    let flags = u32_at(trun, 0)? & 0x00ff_ffff;
    let count = u32_at(trun, 4)? as usize;
    let mut at = 8;
    let mut offset = if flags & 0x01 != 0 {
        at += 4;
        u32_at(trun, 8)? as usize
    } else {
        // Without one, the pictures follow the `moof` and the eight bytes that open `mdat`.
        find(fragment, b"mdat")?.0 + 8
    };
    at += if flags & 0x04 != 0 { 4 } else { 0 };
    for _ in 0..count {
        let mut next = |flag: u32, otherwise: u32| {
            let value = if flags & flag != 0 { u32_at(trun, at) } else { Some(otherwise) };
            at += if flags & flag != 0 { 4 } else { 0 };
            value
        };
        let ticks = next(0x100, ticks)?;
        let size = next(0x200, size)? as usize;
        next(0x400, 0)?;
        next(0x800, 0)?;
        let picture = fragment.get(offset..offset + size)?;
        offset += size;
        // Each unit swaps its length for a start code.
        let mut data = Vec::with_capacity(size + 16);
        let mut unit = 0;
        while let Some(length) = picture.get(unit..unit + header.length_bytes) {
            let length = length.iter().fold(0usize, |length, byte| (length << 8) | usize::from(*byte));
            data.extend(START);
            data.extend(picture.get(unit + header.length_bytes..unit + header.length_bytes + length)?);
            unit += header.length_bytes + length;
        }
        list.push(Sample { data, seconds: f64::from(ticks) / f64::from(header.timescale) });
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boxed(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut bytes = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        bytes.extend(kind);
        bytes.extend(body);
        bytes
    }

    fn nested(path: &[&[u8; 4]], body: &[u8]) -> Vec<u8> {
        path.iter().rev().fold(body.to_vec(), |body, kind| boxed(kind, &body))
    }

    /// A header with one sequence set `[7, 1]`, one picture set `[8]`, four-byte lengths,
    /// 30 000 ticks a second and 1001 a sample.
    fn init() -> Vec<u8> {
        let mut mdhd = vec![0; 12];
        mdhd.extend(30_000u32.to_be_bytes());
        let avcc = boxed(b"avcC", &[1, 0x4d, 0x40, 0x1e, 0xff, 0xe1, 0, 2, 7, 1, 1, 0, 1, 8]);
        let mut entry = vec![0; 78];
        entry.extend(avcc);
        let mut stsd = vec![0, 0, 0, 0, 0, 0, 0, 1];
        stsd.extend(boxed(b"avc1", &entry));
        let media = [boxed(b"mdhd", &mdhd), nested(&[b"minf", b"stbl", b"stsd"], &stsd)].concat();
        let mut trex = vec![0; 12];
        trex.extend(1001u32.to_be_bytes());
        let moov = [nested(&[b"trak", b"mdia"], &media), nested(&[b"mvex", b"trex"], &trex)].concat();
        [boxed(b"ftyp", b"dash"), boxed(b"moov", &moov)].concat()
    }

    #[test]
    fn a_header_gives_the_parameter_sets_and_the_clock() {
        let header = header(&init()).unwrap();
        assert_eq!(header.parameters, [0, 0, 0, 1, 7, 1, 0, 0, 0, 1, 8]);
        assert_eq!((header.length_bytes, header.timescale, header.sample_ticks), (4, 30_000, 1001));
        assert_eq!(super::header(b"not a header"), None);
        assert_eq!(super::header(&init()[..40]), None, "cut short");
    }

    #[test]
    fn an_index_places_each_fragment_after_the_one_before() {
        let mut sidx = vec![0; 8];
        sidx.extend(1000u32.to_be_bytes());
        sidx.extend([0; 4]);
        sidx.extend(10u32.to_be_bytes());
        sidx.extend([0, 0, 0, 2]);
        for (size, ticks) in [(500u32, 5000u32), (700, 4000)] {
            sidx.extend(size.to_be_bytes());
            sidx.extend(ticks.to_be_bytes());
            sidx.extend([0x90, 0, 0, 0]);
        }
        let list = fragments(&boxed(b"sidx", &sidx), 1275);
        assert_eq!(list, [Fragment { start: 1286, size: 500, seconds: 5.0 }, Fragment { start: 1786, size: 700, seconds: 4.0 }]);
        assert!(fragments(b"nothing", 0).is_empty());
    }

    #[test]
    fn a_fragment_gives_its_pictures_with_start_codes_and_their_time() {
        let header = header(&init()).unwrap();
        // Two pictures: one of two units, one of a single unit.
        let first = [0, 0, 0, 2, 0x65, 0xaa, 0, 0, 0, 1, 0x06];
        let second = [0, 0, 0, 3, 0x41, 0xbb, 0xcc];
        let fragment = |offset: u32| {
            let tfhd = [0, 2, 0, 0, 0, 0, 0, 1];
            let mut trun = vec![0, 0, 2, 1, 0, 0, 0, 2];
            trun.extend(offset.to_be_bytes());
            trun.extend((first.len() as u32).to_be_bytes());
            trun.extend((second.len() as u32).to_be_bytes());
            let moof = boxed(b"moof", &boxed(b"traf", &[boxed(b"tfhd", &tfhd), boxed(b"trun", &trun)].concat()));
            [moof, boxed(b"mdat", &[first.as_slice(), second.as_slice()].concat())].concat()
        };
        // The offset is counted from the start of the `moof`, to the first byte in `mdat`.
        let offset = fragment(0).len() - first.len() - second.len();
        let one = fragment(offset as u32);
        let list = samples(&[one.clone(), one].concat(), &header);
        assert_eq!(list.len(), 4, "both fragments are read");
        assert_eq!(list[0].data, [0, 0, 0, 1, 0x65, 0xaa, 0, 0, 0, 1, 0x06]);
        assert_eq!(list[1].data, [0, 0, 0, 1, 0x41, 0xbb, 0xcc]);
        assert_eq!(list[3], list[1]);
        assert!((list[0].seconds - 1001.0 / 30_000.0).abs() < 1e-9, "the header's own pace");
        assert!(samples(b"no fragments here", &header).is_empty());
        assert_eq!(samples(&fragment(9_999), &header).len(), 0, "pictures that are not there are not made up");
    }
}
