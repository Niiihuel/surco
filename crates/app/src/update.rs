//! Taking a later version of the app in: which of a release's files is this system's
//! program, whether what was fetched is that file, and putting it where the running
//! program is.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use sha2::{Digest, Sha256};

/// Where the program that runs was started from. Asked once, at startup: after the file
/// is replaced the system no longer says.
pub fn running() -> Option<&'static Path> {
    static RUNNING: OnceLock<Option<PathBuf>> = OnceLock::new();
    RUNNING.get_or_init(|| std::env::current_exe().ok()).as_deref()
}

/// How a release names this system's program, after its version.
fn target() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("linux-x86_64"),
        ("windows", "x86_64") => Some("windows-x86_64.exe"),
        ("macos", "aarch64") => Some("macos-arm64"),
        _ => None,
    }
}

/// The name of this system's program among the files of the release of `version`.
pub fn program_name(version: &str) -> Option<String> {
    Some(format!("surco-{version}-{}", target()?))
}

/// The oldest version a release says may keep running, as its notes carry it in a
/// comment: `<!-- surco:minimum=1.2.0 -->`.
pub fn minimum(notes: &str) -> Option<&str> {
    let after = notes.split("surco:minimum=").nth(1)?;
    let version = after.split(|c: char| !(c.is_ascii_digit() || c == '.')).next()?;
    (!version.is_empty()).then_some(version)
}

/// A part of what a release says of itself: text to read, as Markdown, or a picture or a
/// video by its address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Note {
    Text(String),
    Picture(String),
    Video(String),
}

/// Text of the notes, with each video in it apart: GitHub leaves one it was given as its
/// address alone on a line.
fn text_and_videos(notes: &mut Vec<Note>, text: &str) {
    let mut lines = String::new();
    let flush = |notes: &mut Vec<Note>, lines: &mut String| {
        if !lines.trim().is_empty() {
            notes.push(Note::Text(lines.trim().to_owned()));
        }
        lines.clear();
    };
    for line in text.lines() {
        let address = line.trim();
        let video = address.starts_with("https://") && !address.contains(char::is_whitespace)
            && (address.starts_with("https://github.com/user-attachments/assets/")
                || [".mp4", ".mov", ".webm"].iter().any(|kind| address.to_ascii_lowercase().ends_with(kind)));
        if video {
            flush(notes, &mut lines);
            notes.push(Note::Video(address.to_owned()));
        } else {
            lines.push_str(line);
            lines.push('\n');
        }
    }
    flush(notes, &mut lines);
}

/// What a release says of itself in its notes, in their order: its text between its
/// pictures, which GitHub writes as Markdown or as an `img`. What the notes say to
/// programs, in comments, is left out, and so is a picture not served over HTTPS.
pub fn notes(body: &str) -> Vec<Note> {
    let mut said = String::new();
    let mut rest = body;
    while let Some((before, comment)) = rest.split_once("<!--") {
        said.push_str(before);
        rest = comment.split_once("-->").map_or("", |(_, after)| after);
    }
    said.push_str(rest);

    let mut notes = Vec::new();
    let mut rest = said.as_str();
    loop {
        let picture = [("![", "](", ")"), ("<img", "src=\"", ">")].into_iter().filter_map(|(opens, before, closes)| {
            let at = rest.find(opens)?;
            let address = at + rest[at..].find(before)? + before.len();
            let end = address + rest[address..].find(closes)?;
            // The address ends where its quote or its title begins.
            let address = rest[address..end].split(['"', ' ', ')']).next().unwrap_or_default();
            Some((at, end + closes.len(), address))
        });
        let Some((at, end, address)) = picture.min_by_key(|(at, ..)| *at) else { break };
        text_and_videos(&mut notes, &rest[..at]);
        if address.starts_with("https://") {
            notes.push(Note::Picture(address.to_owned()));
        }
        rest = &rest[end..];
    }
    text_and_videos(&mut notes, rest);
    notes
}

/// The sum that a list of them, a `<sum>  <name>` to a line, gives for `name`.
pub fn sum_for<'a>(sums: &'a str, name: &str) -> Option<&'a str> {
    sums.lines().find_map(|line| {
        let (sum, file) = line.split_once(char::is_whitespace)?;
        (file.trim().trim_start_matches('*') == name).then_some(sum)
    })
}

/// Whether `bytes` are the file whose SHA-256 is `sum`.
pub fn matches(bytes: &[u8], sum: &str) -> bool {
    let own: String = Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect();
    own.eq_ignore_ascii_case(sum.trim())
}

/// Puts `program` where `running` is. The program that runs goes on from the file it was
/// started from; the next start is the new one.
pub fn install(program: &[u8], running: &Path) -> io::Result<()> {
    let fresh = running.with_extension("new");
    fs::write(&fresh, program)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&fresh, fs::Permissions::from_mode(0o755))?;
    }
    // Windows does not let a program that runs be written over, but lets it be renamed.
    #[cfg(windows)]
    {
        let old = running.with_extension("old");
        let _ = fs::remove_file(&old);
        fs::rename(running, &old)?;
    }
    fs::rename(&fresh, running)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_says_its_minimum_in_a_comment_of_its_notes() {
        assert_eq!(minimum("<!-- surco:minimum=1.2.0 -->\n## What's Changed"), Some("1.2.0"));
        assert_eq!(minimum("notes\n<!-- surco:minimum=0.10 -->"), Some("0.10"));
        assert_eq!(minimum("## What's Changed"), None);
        assert_eq!(minimum("surco:minimum= -->"), None);
    }

    #[test]
    fn a_releases_notes_are_its_text_between_its_pictures() {
        let body = "<!-- surco:minimum=0.1.0 -->\n## Novedades\n- Importar de Spotify\n\n![captura](https://github.com/user-attachments/assets/abc \"título\")\nY más.\n<img width=\"600\" alt=\"x\" src=\"https://example.com/b.png\" />\n![no](http://example.com/c.png)";
        let text = |text: &str| Note::Text(text.into());
        let picture = |address: &str| Note::Picture(address.into());
        assert_eq!(notes(body), [text("## Novedades\n- Importar de Spotify"), picture("https://github.com/user-attachments/assets/abc"), text("Y más."), picture("https://example.com/b.png")]);
        assert_eq!(notes("Solo texto, con un ![ suelto"), [text("Solo texto, con un ![ suelto")]);
        let video = "https://github.com/user-attachments/assets/9f1c-2b";
        assert_eq!(notes(&format!("Mirá:\n{video}\n\nY sigue [aquí](https://example.com/a.mp4).\nhttps://example.com/demo.MP4")),
            [text("Mirá:"), Note::Video(video.into()), text("Y sigue [aquí](https://example.com/a.mp4)."), Note::Video("https://example.com/demo.MP4".into())],
            "a video is an address alone on its line; a link in a sentence is text");
        assert!(notes("<!-- surco:minimum=0.1.0 -->\n").is_empty());
    }

    #[test]
    fn a_program_is_checked_against_the_sum_its_release_lists() {
        let sums = "aaaa  surco-1.0.0-linux-x86_64\nba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad *surco-1.0.0-macos-arm64\n";
        assert_eq!(sum_for(sums, "surco-1.0.0-linux-x86_64"), Some("aaaa"));
        let sum = sum_for(sums, "surco-1.0.0-macos-arm64").unwrap();
        assert!(matches(b"abc", sum) && !matches(b"abd", sum));
        assert_eq!(sum_for(sums, "surco-1.0.0-windows-x86_64.exe"), None);
    }

    #[test]
    fn the_new_program_takes_the_place_of_the_one_that_runs() {
        let directory = std::env::temp_dir().join(format!("surco-update-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let running = directory.join("surco");
        fs::write(&running, b"old").unwrap();
        install(b"new", &running).unwrap();
        assert_eq!(fs::read(&running).unwrap(), b"new");
        assert!(!running.with_extension("new").exists(), "nothing is left beside it");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&running).unwrap().permissions().mode() & 0o111, 0o111, "and it can be started");
        }
        fs::remove_dir_all(&directory).unwrap();
    }
}
