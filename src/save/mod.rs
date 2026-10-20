//! Saving and loading the game in this engine's own format (`file`: the
//! container; `state`: what it holds; `codec`: the encoding). Reading the
//! original game's `.ess` saves is the `ess` crate's separate, read-only job.
//! `known_ambiguities/saves.md`.

pub mod codec;
pub mod file;
pub mod state;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::engine::Engine;
use file::SaveFile;
use state::Snapshot;

const EXTENSION: &str = "vrs";

/// Where saves live: `$VRM_SAVES`, else `vibe_rm/saves` under the user's data
/// directory (`$XDG_DATA_HOME`, else `~/.local/share`).
pub fn save_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("VRM_SAVES") {
        return d.into();
    }
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    data.join("vibe_rm/saves")
}

/// A save's file in `dir`; names with anything but letters, digits, spaces,
/// `-` and `_` are refused, so a name can't leave the directory.
pub fn save_path_in(dir: &Path, name: &str) -> Option<PathBuf> {
    let ok = !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_'));
    ok.then(|| dir.join(format!("{name}.{EXTENSION}")))
}

/// The saves in `dir`, newest first.
pub fn list_saves_in(dir: &Path) -> Vec<String> {
    let mut found: Vec<(std::time::SystemTime, String)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            (path.extension()? == EXTENSION).then_some(())?;
            let name = path.file_stem()?.to_str()?.to_string();
            Some((entry.metadata().ok()?.modified().ok()?, name))
        })
        .collect();
    found.sort_by(|a, b| b.cmp(a));
    found.into_iter().map(|(_, n)| n).collect()
}

impl Engine {
    /// Write the game to `path` (to a temporary file first, so a crash
    /// leaves the old save whole).
    pub fn save_game_to(&self, path: &Path) -> Result<()> {
        let bytes = Snapshot::capture(self).write().to_bytes();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, &bytes).with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))?;
        log::info!("saved {} ({} bytes)", path.display(), bytes.len());
        Ok(())
    }

    /// Load the save at `path`: all of it reads before the game changes.
    pub fn load_game_from(&mut self, path: &Path) -> Result<()> {
        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        let snapshot = Snapshot::read(&SaveFile::from_bytes(&bytes)?)?;
        snapshot.apply(self)?;
        log::info!("loaded {}", path.display());
        Ok(())
    }

    /// Save as `name` in the saves directory.
    pub fn save_game(&self, name: &str) -> Result<PathBuf> {
        let Some(path) = save_path_in(&save_dir(), name) else {
            bail!("'{name}' isn't a save name (letters, digits, spaces, - and _)");
        };
        self.save_game_to(&path)?;
        Ok(path)
    }

    pub fn load_game(&mut self, name: &str) -> Result<()> {
        let Some(path) = save_path_in(&save_dir(), name) else {
            bail!("'{name}' isn't a save name");
        };
        self.load_game_from(&path)
    }

    /// `Game.RequestSave` / `RequestAutoSave`: the game is written when the
    /// script has finished running (next update), as `quicksave` or `autosave`.
    pub fn request_save(&mut self, auto: bool) {
        let name = if auto { "autosave" } else { "quicksave" };
        if !self.save_requests.iter().any(|(n, _)| n == name) {
            self.save_requests.push((name.into(), auto));
        }
    }

    pub(crate) fn flush_save_requests(&mut self) {
        for (name, auto) in std::mem::take(&mut self.save_requests) {
            if self.player_dead() {
                let msg = self.gmst_string("sCantSaveNow");
                self.scripts
                    .notify(msg.unwrap_or_else(|| "You cannot save right now.".into()));
                continue;
            }
            if auto {
                let msg = self.gmst_string("sAutoSaving");
                self.scripts
                    .notify(msg.unwrap_or_else(|| "Autosaving...".into()));
            }
            if let Err(err) = self.save_game(&name) {
                log::warn!("saving {name}: {err:#}");
            }
        }
    }
}
