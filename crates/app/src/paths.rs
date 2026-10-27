//! Persistent app files use the platform's private application-support directory.

use std::io;
use std::path::{Path, PathBuf};

pub fn data_dir(app_id: &str) -> io::Result<PathBuf> {
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        Ok(eframe::storage_dir(app_id).unwrap_or_else(std::env::temp_dir))
    }
    // eframe knows no folder of Android's: without the activity's own the library would be
    // kept in a cache the system empties.
    #[cfg(target_os = "android")]
    {
        let _ = app_id;
        crate::android::data_dir().ok_or_else(|| io::Error::other("Android's app files directory is unavailable"))
    }
    #[cfg(target_os = "ios")]
    {
        use std::ffi::{CStr, CString, OsStr};
        use std::os::unix::ffi::OsStrExt;

        unsafe extern "C" {
            fn surco_application_support_dir() -> *mut std::ffi::c_char;
            fn surco_exclude_from_backup(path: *const std::ffi::c_char) -> i32;
            fn free(pointer: *mut std::ffi::c_void);
        }

        // The host returns an owned, NUL-terminated copy of Foundation's container path.
        let support = unsafe { surco_application_support_dir() };
        if support.is_null() {
            return Err(io::Error::other(
                "iOS application-support directory is unavailable",
            ));
        }
        let bytes = unsafe { CStr::from_ptr(support) }.to_bytes().to_vec();
        unsafe { free(support.cast()) };
        let data = ios_data_dir(Path::new(OsStr::from_bytes(&bytes)), app_id)?;
        let offline = CString::new(data.join("offline").as_os_str().as_bytes())?;
        // Downloaded audio must persist between launches without entering iCloud backups.
        if unsafe { surco_exclude_from_backup(offline.as_ptr()) } == 0 {
            return Err(io::Error::other(
                "could not exclude offline audio from iOS backups",
            ));
        }
        Ok(data)
    }
}

pub fn persistence_path(data: &Path) -> Option<PathBuf> {
    cfg!(any(target_os = "ios", target_os = "android")).then(|| data.join("app.ron"))
}

#[cfg(any(target_os = "ios", test))]
fn ios_data_dir(support: &Path, app_id: &str) -> io::Result<PathBuf> {
    if !support.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "iOS container path must be absolute",
        ));
    }
    let data = support.join(app_id);
    std::fs::create_dir_all(data.join("offline"))?;
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn ios_state_and_downloads_survive_reopening_the_same_container() {
        let support = std::env::temp_dir().join(format!("surco-ios-paths-{}", std::process::id()));
        let first = ios_data_dir(&support, "surco").unwrap();
        std::fs::write(first.join("app.ron"), b"saved settings").unwrap();
        std::fs::write(first.join("offline/song"), b"saved audio").unwrap();

        let reopened = ios_data_dir(&support, "surco").unwrap();
        assert_eq!(
            std::fs::read(reopened.join("app.ron")).unwrap(),
            b"saved settings"
        );
        assert_eq!(
            std::fs::read(reopened.join("offline/song")).unwrap(),
            b"saved audio"
        );
        std::fs::remove_dir_all(support).unwrap();
    }

    #[test]
    fn ios_rejects_missing_or_relative_container_paths() {
        for path in ["", "Library/Application Support"] {
            assert_eq!(
                ios_data_dir(Path::new(path), "surco").unwrap_err().kind(),
                std::io::ErrorKind::InvalidInput
            );
        }
    }

    #[test]
    #[cfg(not(target_os = "ios"))]
    fn desktop_keeps_eframes_existing_storage_and_default_persistence_path() {
        let expected = eframe::storage_dir("surco").unwrap_or_else(std::env::temp_dir);
        assert_eq!(data_dir("surco").unwrap(), expected);
        assert_eq!(persistence_path(&expected), None);
    }
}
