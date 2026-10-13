//! Fetches a few seconds from the middle of a video, decodes them and writes one frame:
//! `cargo run -p surco-video --example clip_probe -- <video id> <frame.ppm>`

use std::io::Write;

type Failure = Box<dyn std::error::Error + Send + Sync>;

async fn range(youtube: &surco_youtube::YouTube, url: &str, from: u64, to: u64) -> Result<Vec<u8>, Failure> {
    let response = youtube.http().get(url).header("Range", format!("bytes={from}-{to}")).send().await?.error_for_status()?;
    Ok(response.bytes().await?.to_vec())
}

#[tokio::main]
async fn main() -> Result<(), Failure> {
    let id = std::env::args().nth(1).unwrap_or_else(|| "4NRXx6U8ABQ".into());
    let youtube = surco_youtube::YouTube::new()?;
    let stream = youtube.stream(&id).await?;
    let video = stream.video.iter().filter(|video| video.mime.contains("avc1")).min_by_key(|video| video.height.abs_diff(360)).ok_or("no H.264")?;
    println!("{}x{} itag {} {} bytes", video.width, video.height, video.itag, video.size);

    let head = range(&youtube, &video.url, 0, video.index.1).await?;
    let header = surco_video::header(&head[..=video.init.1 as usize]).ok_or("no header")?;
    let fragments = surco_video::fragments(&head[video.index.0 as usize..], video.index.1);
    let total: f64 = fragments.iter().map(|fragment| fragment.seconds).sum();
    println!("{} fragments, {total:.1} s, first lasts {:.2} s", fragments.len(), fragments.first().map_or(0.0, |fragment| fragment.seconds));

    // Eight seconds from a third of the way in.
    let mut at = 0.0;
    let mut chosen = Vec::new();
    for fragment in &fragments {
        if at >= total * 0.35 && chosen.iter().map(|fragment: &&surco_video::Fragment| fragment.seconds).sum::<f64>() < 8.0 {
            chosen.push(fragment);
        }
        at += fragment.seconds;
    }
    let (first, last) = (chosen.first().ok_or("no fragments")?, chosen.last().ok_or("no fragments")?);
    let bytes = range(&youtube, &video.url, first.start, last.start + last.size - 1).await?;
    let samples = surco_video::samples(&bytes, &header);
    println!("{} fragments chosen, {} bytes, {} pictures of {:.4} s", chosen.len(), bytes.len(), samples.len(), samples.first().map_or(0.0, |sample| sample.seconds));

    let started = std::time::Instant::now();
    let mut decoder = surco_video::Decoder::new(&header.parameters)?;
    let mut frames = Vec::new();
    let (mut waiting, mut failed) = (0, 0);
    for (index, sample) in samples.iter().enumerate() {
        match decoder.decode(&sample.data) {
            Ok(Some(frame)) => frames.push(frame),
            Ok(None) => waiting += 1,
            Err(error) => {
                if failed < 3 {
                    println!("picture {index} ({} bytes, unit type {}): {error}", sample.data.len(), sample.data.get(4).map_or(0, |byte| byte & 0x1f));
                }
                failed += 1;
            }
        }
    }
    println!("{} frames, {waiting} waiting, {failed} failed in {:?}", frames.len(), started.elapsed());
    if let (Some(frame), Some(path)) = (frames.get(frames.len() / 2), std::env::args().nth(2)) {
        let mut file = std::fs::File::create(path)?;
        write!(file, "P6\n{} {}\n255\n", frame.width, frame.height)?;
        file.write_all(&frame.rgba.chunks(4).flat_map(|pixel| [pixel[0], pixel[1], pixel[2]]).collect::<Vec<u8>>())?;
    }
    Ok(())
}
