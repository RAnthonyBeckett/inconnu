//! Account-local navigation history; never game data or an automatic open instruction.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
const MAX_ENTRIES: usize = 10;
const MAX_BYTES: u64 = 65536;
const UNAVAILABLE: &str = "Recent library history is unavailable; its existing file is preserved. Clear the list to reset it.";
const WRITE_FAILED: &str =
    "Recent library history could not be saved. Your library is still available.";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    version: u8,
    paths: Vec<PathBuf>,
}
#[derive(Default)]
pub struct RecentLibraries {
    file: Option<PathBuf>,
    paths: Vec<PathBuf>,
    blocked: bool,
    warning: Option<&'static str>,
}
impl RecentLibraries {
    pub fn load(file: PathBuf) -> Self {
        let mut history = Self {
            file: Some(file.clone()),
            ..Default::default()
        };
        let read = (|| -> Result<Option<Saved>, Box<dyn std::error::Error>> {
            let input = match fs::File::open(&file) {
                Ok(input) => input,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(e) => return Err(e.into()),
            };
            let mut bytes = Vec::new();
            input.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
            if bytes.len() as u64 > MAX_BYTES {
                return Err("history size".into());
            }
            let saved: Saved = serde_json::from_slice(&bytes)?;
            if saved.version != 1
                || saved.paths.len() > MAX_ENTRIES
                || saved
                    .paths
                    .iter()
                    .any(|p| !p.is_absolute() || p.as_os_str().len() > 32768)
            {
                return Err("history shape".into());
            }
            Ok(Some(saved))
        })();
        match read {
            Ok(Some(saved)) => {
                for path in saved.paths {
                    if !history.paths.iter().any(|p| same_path(p, &path)) {
                        history.paths.push(path);
                    }
                }
            }
            Ok(None) => {}
            Err(_) => {
                history.blocked = true;
                history.warning = Some(UNAVAILABLE);
            }
        }
        history
    }
    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }
    pub fn warning(&self) -> Option<&'static str> {
        self.warning
    }
    pub fn remember(&mut self, path: &Path) {
        let Ok(path) = std::path::absolute(path) else {
            self.warning = Some(WRITE_FAILED);
            return;
        };
        if path.as_os_str().len() > 32768 {
            self.warning = Some(WRITE_FAILED);
            return;
        }
        self.paths.retain(|p| !same_path(p, &path));
        self.paths.insert(0, path);
        self.paths.truncate(MAX_ENTRIES);
        self.save();
    }
    pub fn forget(&mut self, path: &Path) {
        self.paths.retain(|p| !same_path(p, path));
        self.save();
    }
    pub fn clear(&mut self) {
        self.paths.clear();
        self.blocked = false;
        self.save();
    }
    fn save(&mut self) {
        if self.blocked {
            return;
        }
        let Some(path) = &self.file else {
            return;
        };
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let parent = path.parent().ok_or("history parent")?;
            fs::create_dir_all(parent)?;
            let bytes = serde_json::to_vec(&Saved {
                version: 1,
                paths: self.paths.clone(),
            })?;
            if bytes.len() as u64 > MAX_BYTES {
                return Err("history size".into());
            }
            let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
            temporary.write_all(&bytes)?;
            temporary.as_file().sync_all()?;
            temporary.persist(path)?;
            Ok(())
        })();
        self.warning = if result.is_err() {
            Some(WRITE_FAILED)
        } else {
            None
        };
    }
}
fn same_path(a: &Path, b: &Path) -> bool {
    if cfg!(windows) {
        a.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(&b.as_os_str().to_string_lossy())
    } else {
        a == b
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordered_bounded_restart_and_forget_never_touch_libraries() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("state/history.json");
        let mut history = RecentLibraries::load(file.clone());
        for n in 0..12 {
            history.remember(&dir.path().join(format!("game{n}.gvlib")));
        }
        let game = dir.path().join("game8.gvlib");
        fs::write(&game, b"KEEP_LIBRARY").unwrap();
        history.remember(&game);
        assert_eq!(history.paths()[0], game);
        assert_eq!(history.paths().len(), 10);
        let mut reopened = RecentLibraries::load(file.clone());
        assert_eq!(reopened.paths(), history.paths());
        reopened.forget(&game);
        assert!(!reopened.paths().contains(&game));
        assert_eq!(fs::read(&game).unwrap(), b"KEEP_LIBRARY");
        reopened.clear();
        assert!(RecentLibraries::load(file).paths().is_empty());
        assert_eq!(fs::read(game).unwrap(), b"KEEP_LIBRARY");
    }
    #[test]
    fn invalid_history_is_preserved_until_explicit_clear() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("history.json");
        for bytes in [
            b"not json".to_vec(),
            vec![b'x'; 65537],
            br#"{"version":9,"paths":[]}"#.to_vec(),
            br#"{"version":1,"paths":["relative.gvlib"]}"#.to_vec(),
        ] {
            fs::write(&file, &bytes).unwrap();
            let mut history = RecentLibraries::load(file.clone());
            assert!(history.warning().is_some());
            history.remember(&dir.path().join("real.gvlib"));
            assert_eq!(fs::read(&file).unwrap(), bytes);
            history.clear();
            assert!(history.warning().is_none());
            assert!(RecentLibraries::load(file.clone()).paths().is_empty());
        }
    }
    #[test]
    fn write_failure_does_not_damage_destination_or_remembered_path() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("folder");
        fs::create_dir(&file).unwrap();
        let mut history = RecentLibraries::load(file.clone());
        history.clear();
        history.remember(&dir.path().join("game.gvlib"));
        assert!(history.warning().is_some());
        assert_eq!(history.paths().len(), 1);
        assert!(file.is_dir());
    }
    #[test]
    fn duplicates_and_unicode_paths_survive_without_probing_targets() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("history.json");
        let target = dir.path().join("Crônica inexistente.gvlib");
        let bytes = serde_json::to_vec(&Saved {
            version: 1,
            paths: vec![target.clone(), target.clone()],
        })
        .unwrap();
        fs::write(&file, bytes).unwrap();
        let mut history = RecentLibraries::load(file.clone());
        assert_eq!(history.paths(), std::slice::from_ref(&target));
        history.remember(&target);
        assert_eq!(
            RecentLibraries::load(file).paths(),
            std::slice::from_ref(&target)
        );
        assert!(!target.exists());
    }
}
