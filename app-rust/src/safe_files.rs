//! Small helpers for publishing local files without replacing existing data.

use crate::model::{Result, error};
use std::{fs::File, io, io::Write, path::Path};
use tempfile::NamedTempFile;

/// Writes `bytes` to a new file, atomically publishing it without overwriting.
///
/// The temporary file is created in the destination directory so publication
/// stays on the same filesystem. It is fully written and synced before the
/// destination name becomes visible.
pub fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    write_new_with(
        path,
        bytes,
        |file, bytes| file.write_all(bytes),
        File::sync_all,
    )
}

fn write_new_with<W, S>(path: &Path, bytes: &[u8], write: W, sync: S) -> Result<()>
where
    W: FnOnce(&mut File, &[u8]) -> io::Result<()>,
    S: FnOnce(&File) -> io::Result<()>,
{
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = NamedTempFile::new_in(parent).map_err(|_| {
        error(
            "FILE_CREATE",
            "Could not create a temporary file in the destination folder.",
        )
    })?;

    write(temporary.as_file_mut(), bytes)
        .map_err(|_| error("FILE_WRITE", "Could not write the complete file contents."))?;
    sync(temporary.as_file())
        .map_err(|_| error("FILE_SYNC", "Could not sync the completed file contents."))?;

    temporary.persist_noclobber(path).map_err(|_| {
        error(
            "FILE_PUBLISH",
            "Could not publish the file without overwriting an existing destination.",
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::write_new_with;
    use std::{fs, io, io::Write};
    use tempfile::tempdir;

    fn assert_failed_attempt_cleans_up(write_fails: bool) {
        let directory = tempdir().expect("temporary test directory");
        let destination = directory.path().join("policy.json");

        let result = if write_fails {
            write_new_with(
                &destination,
                b"complete contents",
                |file, bytes| {
                    file.write_all(&bytes[..4])?;
                    Err(io::Error::other("injected write failure"))
                },
                |file| file.sync_all(),
            )
        } else {
            write_new_with(
                &destination,
                b"complete contents",
                |file, bytes| file.write_all(bytes),
                |_| Err(io::Error::other("injected sync failure")),
            )
        };

        assert!(result.is_err());
        assert!(!destination.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
    }

    #[test]
    fn write_failure_leaves_no_destination_or_temporary_file() {
        assert_failed_attempt_cleans_up(true);
    }

    #[test]
    fn sync_failure_leaves_no_destination_or_temporary_file() {
        assert_failed_attempt_cleans_up(false);
    }
}
