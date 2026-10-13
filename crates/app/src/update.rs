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
