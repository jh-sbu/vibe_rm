//! Saving and loading the game in this engine's own format (`file`: the
//! container; `state`: what it holds; `codec`: the encoding). Reading the
//! original game's `.ess` saves is the `ess` crate's separate, read-only job.
//! `known_ambiguities/saves.md`.

pub mod codec;
mod continuity;
pub mod file;
pub mod metadata;
pub mod state;
#[cfg(test)]
mod tests;
pub mod vm;

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

/// One journal row, discovered without reading the simulation snapshot.
#[derive(Clone, Debug)]
pub struct SaveListing {
    pub name: String,
    pub metadata: Option<metadata::Metadata>,
    pub corrupt: bool,
    pub obsolete: bool,
    pub written: u64,
}
impl SaveListing {
    pub fn file_num(&self) -> u32 {
        self.metadata.as_ref().map_or(0, |m| m.file_num)
    }
}
/// Newest first, deterministic by name on timestamp ties. Bad files remain
/// visible so the player can delete them instead of losing the entire list.
pub fn save_list_in(dir: &Path, plugins: &[String]) -> Vec<SaveListing> {
    let mut found: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension()? != EXTENSION || !entry.file_type().ok()?.is_file() {
                return None;
            }
            let name = path.file_stem()?.to_str()?.to_string();
            save_path_in(dir, &name)?;
            let modified = entry
                .metadata()
                .ok()?
                .modified()
                .ok()?
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let header = (|| -> Result<(Option<metadata::Metadata>, bool)> {
                let prefix = SaveFile::read_prefix(std::fs::File::open(&path)?)?;
                if let Some(bytes) = prefix.section(b"META") {
                    let meta = metadata::Metadata::decode(bytes)?;
                    let obsolete = meta.plugins_hash != metadata::plugin_hash(plugins);
                    Ok((Some(meta), obsolete))
                } else {
                    let mut r =
                        codec::Reader::new(prefix.section(b"HEAD").context("missing HEAD")?);
                    let mut saved = Vec::new();
                    for _ in 0..r.count(4)? {
                        saved.push(r.str()?);
                    }
                    Ok((None, saved != plugins))
                }
            })();
            let (metadata, obsolete, corrupt) = match header {
                Ok((m, o)) => (m, o, false),
                Err(_) => (None, false, true),
            };
            let written = metadata.as_ref().map_or(modified, |m| m.written);
            Some(SaveListing {
                name,
                metadata,
                corrupt,
                obsolete,
                written,
            })
        })
        .collect();
    found.sort_by(|a, b| b.written.cmp(&a.written).then_with(|| b.name.cmp(&a.name)));
    found
}

/// The next append-only quicksave name in `dir`. Existing numbered quicksaves
/// are retained, including gaps left by a failed or removed save.
fn next_quicksave_name_in(dir: &Path) -> Result<String> {
    let mut highest = 0u64;
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok("quicksave 1".to_string());
        }
        Err(err) => return Err(err).with_context(|| format!("reading {}", dir.display())),
    };
    for entry in entries {
        let entry = entry.with_context(|| format!("reading entries in {}", dir.display()))?;
        let path = entry.path();
        if path
            .extension()
            .is_none_or(|extension| extension != EXTENSION)
        {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(number) = name
            .strip_prefix("quicksave ")
            .and_then(|suffix| suffix.parse::<u64>().ok())
        else {
            continue;
        };
        highest = highest.max(number);
    }
    let number = highest
        .checked_add(1)
        .context("quicksave number is exhausted")?;
    let name = format!("quicksave {number}");
    if save_path_in(dir, &name).is_none() {
        bail!("quicksave number is outside the supported save name range");
    }
    Ok(name)
}

/// Global display numbering survives overwrites and skips used numbers.
fn next_file_num_in(dir: &Path) -> Result<u32> {
    let mut highest = 0u32;
    for row in save_list_in(dir, &[]) {
        let from_name = row
            .name
            .strip_prefix("Save ")
            .and_then(|s| s.split(' ').next())
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        highest = highest.max(row.file_num()).max(from_name);
    }
    highest.checked_add(1).context("save numbering exhausted")
}

fn delete_save_in(dir: &Path, name: &str) -> Result<()> {
    let path = save_path_in(dir, name).context("invalid save name")?;
    std::fs::remove_file(&path).with_context(|| format!("deleting {}", path.display()))
}

fn write_save_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))?;
    Ok(())
}

impl Engine {
    pub fn new_save_name(&self) -> Result<String> {
        let number = next_file_num_in(&save_dir())?;
        let meta = metadata::Metadata::capture(self);
        let suffix: String = format!("{} {}", meta.name, meta.location)
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_') {
                    c
                } else {
                    '_'
                }
            })
            .take(40)
            .collect();
        Ok(format!("Save {number} - {suffix}"))
    }
    pub fn save_list(&self) -> Vec<SaveListing> {
        let plugins: Vec<_> = self
            .lo
            .plugins()
            .iter()
            .map(|p| p.plugin.name().to_string())
            .collect();
        save_list_in(&save_dir(), &plugins)
    }

    /// Write the game to `path` (to a temporary file first, so a crash
    /// leaves the old save whole).
    pub fn save_game_to(&self, path: &Path) -> Result<()> {
        let mut snapshot = Snapshot::capture(self);
        if let Some(meta) = &mut snapshot.metadata {
            meta.file_num = std::fs::File::open(path)
                .ok()
                .and_then(|f| SaveFile::read_prefix(f).ok())
                .and_then(|f| {
                    f.section(b"META")
                        .and_then(|b| metadata::Metadata::decode(b).ok())
                })
                .map(|m| m.file_num)
                .filter(|n| *n > 0)
                .map(Ok)
                .unwrap_or_else(|| next_file_num_in(path.parent().unwrap_or(Path::new("."))))?;
        }
        let bytes = snapshot.write().to_bytes();
        write_save_bytes(path, &bytes)?;
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
        if self.saving_disabled() {
            bail!("saving is disabled");
        }
        let Some(path) = save_path_in(&save_dir(), name) else {
            bail!("'{name}' isn't a save name (letters, digits, spaces, - and _)");
        };
        self.save_game_to(&path)?;
        Ok(path)
    }

    pub fn delete_save(&self, name: &str) -> Result<()> {
        delete_save_in(&save_dir(), name)
    }

    pub fn load_game(&mut self, name: &str) -> Result<()> {
        let Some(path) = save_path_in(&save_dir(), name) else {
            bail!("'{name}' isn't a save name");
        };
        self.load_game_from(&path)
    }

    /// `Game.RequestSave` / `RequestAutoSave`: the game is written when the
    /// script has finished running (next update). Native quicksaves are
    /// append-only (`quicksave 1`, `quicksave 2`, ...); autosaves use their
    /// single replaceable slot.
    pub fn request_save(&mut self, auto: bool) {
        let name = if auto { "autosave" } else { "quicksave" };
        if !self.save_requests.iter().any(|(n, _)| n == name) {
            self.save_requests.push((name.into(), auto));
        }
    }

    pub(crate) fn flush_save_requests(&mut self) {
        self.flush_save_requests_in(&save_dir());
    }

    fn flush_save_requests_in(&mut self, dir: &Path) {
        if !self.save_requests.is_empty() && !self.menu_up() {
            self.capture_save_thumbnail();
        }
        for (name, auto) in std::mem::take(&mut self.save_requests) {
            if self.player_dead() || self.saving_disabled() {
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
            let name = if auto {
                name
            } else {
                match next_quicksave_name_in(dir) {
                    Ok(name) => name,
                    Err(err) => {
                        log::warn!("choosing quicksave name: {err:#}");
                        continue;
                    }
                }
            };
            let Some(path) = save_path_in(dir, &name) else {
                log::warn!("saving {name}: invalid save name");
                continue;
            };
            if let Err(err) = self.save_game_to(&path) {
                log::warn!("saving {name}: {err:#}");
            }
        }
    }
}
