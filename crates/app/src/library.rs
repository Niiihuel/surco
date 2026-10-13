//! What is kept on disk to listen to without a network: the audio of each saved track, its
//! cover, and an index that says what they are.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use dnm_core::Track;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const INDEX: &str = "index.json";

#[derive(Clone, Serialize, Deserialize)]
struct Saved {
    track: Track,
    mime: String,
    /// How much louder than the reference it is, as the catalog said when it was saved.
    #[serde(default)]
    loudness_db: Option<f64>,
}

pub struct Library {
    dir: PathBuf,
    saved: Vec<Saved>,
}

/// A name for a file that holds what `key` names, safe on every file system.
fn file_name(key: &str) -> String {
    Sha256::digest(key).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Whole or not at all: a crash mid-write must not leave half a file under the final name.
fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let partial = path.with_extension("partial");
    fs::write(&partial, bytes)?;
    fs::rename(partial, path)
}

impl Library {
    /// An index that is missing or unreadable is an empty library, not an error: the audio
    /// can always be downloaded again.
    pub fn open(dir: PathBuf) -> Self {
        let saved = fs::read(dir.join(INDEX)).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
        Self { dir, saved }
    }

    /// Newest first.
    pub fn tracks(&self) -> Vec<Track> {
        self.saved.iter().rev().map(|saved| saved.track.clone()).collect()
    }

    /// Where a saved track's audio is, its type and its loudness.
    pub fn audio(&self, track_id: &str) -> Option<(PathBuf, String, Option<f64>)> {
        let saved = self.saved.iter().find(|saved| saved.track.id == track_id)?;
        Some((self.dir.join(file_name(track_id)), saved.mime.clone(), saved.loudness_db))
    }

    /// The room the saved audio takes on disk.
    pub fn bytes(&self) -> u64 {
        let size = |saved: &Saved| fs::metadata(self.dir.join(file_name(&saved.track.id))).map_or(0, |file| file.len());
        self.saved.iter().map(size).sum()
    }

    pub fn cover(&self, url: &str) -> Option<Vec<u8>> {
        fs::read(self.dir.join(file_name(url))).ok()
    }

    pub fn add(&mut self, track: Track, mime: String, loudness_db: Option<f64>, audio: &[u8], cover: Option<&[u8]>) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        write(&self.dir.join(file_name(&track.id)), audio)?;
        if let (Some(url), Some(cover)) = (&track.artwork_url, cover) {
            write(&self.dir.join(file_name(url)), cover)?;
        }
        self.saved.retain(|saved| saved.track.id != track.id);
        self.saved.push(Saved { track, mime, loudness_db });
        self.store()
    }

    /// The cover stays: another saved track of the same album may show it.
    pub fn remove(&mut self, track_id: &str) -> io::Result<()> {
        self.saved.retain(|saved| saved.track.id != track_id);
        self.store()?;
        match fs::remove_file(self.dir.join(file_name(track_id))) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    }

    fn store(&self) -> io::Result<()> {
        write(&self.dir.join(INDEX), &serde_json::to_vec(&self.saved)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str) -> Track {
        Track {
            id: id.into(),
            title: id.into(),
            artist: "artist".into(),
            artist_id: None,
            album: None,
            album_id: None,
            artwork_url: Some(format!("https://img/{id}?size=1")),
            duration_ms: 200_000,
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("dnm-library-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn what_is_saved_is_there_after_reopening_and_gone_after_removing() {
        let dir = scratch("keeps");
        let mut library = Library::open(dir.clone());
        assert!(library.tracks().is_empty() && library.audio("a").is_none());
        library.add(track("a"), "audio/webm".into(), Some(3.0), b"aaa", Some(b"cover")).unwrap();
        library.add(track("b/..\\x"), "audio/mp4".into(), None, b"bbb", None).unwrap();
        library.add(track("a"), "audio/webm".into(), Some(5.5), b"again", None).unwrap();

        let library = Library::open(dir.clone());
        assert_eq!(library.tracks(), vec![track("a"), track("b/..\\x")], "newest first, each once");
        assert_eq!(library.bytes(), 8, "five bytes of one and three of the other");
        let (path, mime, loudness) = library.audio("a").unwrap();
        assert_eq!((fs::read(path).unwrap(), mime.as_str(), loudness), (b"again".to_vec(), "audio/webm", Some(5.5)));
        assert!(library.audio("b/..\\x").unwrap().0.starts_with(&dir), "an id never names a path outside the library");
        assert_eq!(library.cover("https://img/a?size=1"), Some(b"cover".to_vec()));
        assert_eq!(library.cover("https://img/none"), None);

        let mut library = library;
        library.remove("a").unwrap();
        library.remove("a").unwrap();
        assert!(Library::open(dir.clone()).audio("a").is_none());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_broken_index_is_an_empty_library() {
        let dir = scratch("broken");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(INDEX), b"{ not json").unwrap();
        assert!(Library::open(dir.clone()).tracks().is_empty());
        fs::remove_dir_all(dir).unwrap();
    }
}
