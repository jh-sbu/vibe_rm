//! Virtual file system: loose files in the Data directory override files in
//! archives; later archives override earlier ones.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub use bsa::normalize_path;

/// Archives listed by `sResourceArchiveList`/`sResourceArchiveList2` in a stock Skyrim.ini.
pub const DEFAULT_ARCHIVES: &[&str] = &[
    "Skyrim - Misc.bsa",
    "Skyrim - Shaders.bsa",
    "Skyrim - Interface.bsa",
    "Skyrim - Animations.bsa",
    "Skyrim - Meshes0.bsa",
    "Skyrim - Meshes1.bsa",
    "Skyrim - Sounds.bsa",
    "Skyrim - Voices_en0.bsa",
    "Skyrim - Textures0.bsa",
    "Skyrim - Textures1.bsa",
    "Skyrim - Textures2.bsa",
    "Skyrim - Textures3.bsa",
    "Skyrim - Textures4.bsa",
    "Skyrim - Textures5.bsa",
    "Skyrim - Textures6.bsa",
    "Skyrim - Textures7.bsa",
    "Skyrim - Textures8.bsa",
    "Skyrim - Patch.bsa",
];

pub struct Vfs {
    data_dir: PathBuf,
    archives: Vec<bsa::Archive>,
    /// Normalised path -> (archive index) for the winning archive.
    index: HashMap<String, usize>,
    /// Normalised path -> loose file path on disk.
    loose: HashMap<String, PathBuf>,
}

impl Vfs {
    /// Build a VFS for `data_dir`. `plugins` is the load order; each plugin's
    /// companion archives (`<name>.bsa`, `<name> - Textures.bsa`) are loaded after
    /// the default archives in plugin order.
    pub fn new(data_dir: impl AsRef<Path>, plugins: &[String]) -> Self {
        let data_dir = data_dir.as_ref().to_path_buf();
        let mut names: Vec<String> = DEFAULT_ARCHIVES.iter().map(|s| s.to_string()).collect();
        for p in plugins {
            let stem = p.rsplit_once('.').map(|x| x.0).unwrap_or(p);
            names.push(format!("{stem}.bsa"));
            names.push(format!("{stem} - Textures.bsa"));
        }
        let mut vfs = Vfs { data_dir, archives: Vec::new(), index: HashMap::new(), loose: HashMap::new() };
        let mut seen = std::collections::HashSet::new();
        for n in names {
            if !seen.insert(n.to_ascii_lowercase()) {
                continue;
            }
            let path = find_case_insensitive(&vfs.data_dir, &n);
            let Some(path) = path else { continue };
            match bsa::Archive::open(&path) {
                Ok(a) => vfs.add_archive(a),
                Err(e) => log::error!("failed to open archive {}: {e}", path.display()),
            }
        }
        vfs.scan_loose();
        vfs
    }

    fn add_archive(&mut self, a: bsa::Archive) {
        let idx = self.archives.len();
        for p in a.paths() {
            self.index.insert(p.to_owned(), idx);
        }
        log::info!("archive {} ({} files)", a.path().display(), a.len());
        self.archives.push(a);
    }

    fn scan_loose(&mut self) {
        // Only the asset folders; plugins/archives at the root are not resources.
        let Ok(rd) = std::fs::read_dir(&self.data_dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, &self.data_dir, &mut self.loose);
            }
        }
        if !self.loose.is_empty() {
            log::info!("{} loose files", self.loose.len());
        }
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn exists(&self, path: &str) -> bool {
        let n = normalize_path(path);
        self.loose.contains_key(&n) || self.index.contains_key(&n)
    }

    pub fn read(&self, path: &str) -> Option<Vec<u8>> {
        let n = normalize_path(path);
        if let Some(p) = self.loose.get(&n) {
            match std::fs::read(p) {
                Ok(d) => return Some(d),
                Err(e) => log::warn!("failed to read {}: {e}", p.display()),
            }
        }
        let &a = self.index.get(&n)?;
        match self.archives[a].read(&n) {
            Ok(d) => d,
            Err(e) => {
                log::error!("failed to read {n} from {}: {e}", self.archives[a].path().display());
                None
            }
        }
    }

    /// All known paths (loose and archived) under a prefix.
    pub fn list(&self, prefix: &str) -> Vec<String> {
        let prefix = normalize_path(prefix);
        let mut v: Vec<String> = self
            .index
            .keys()
            .chain(self.loose.keys())
            .filter(|k| k.starts_with(&prefix))
            .cloned()
            .collect();
        v.sort();
        v.dedup();
        v
    }
}

fn walk(dir: &Path, root: &Path, out: &mut HashMap<String, PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, root, out);
        } else if let Ok(rel) = p.strip_prefix(root) {
            out.insert(normalize_path(&rel.to_string_lossy()), p);
        }
    }
}

fn find_case_insensitive(dir: &Path, name: &str) -> Option<PathBuf> {
    let direct = dir.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.file_name().is_some_and(|f| f.to_string_lossy().eq_ignore_ascii_case(name)))
}

/// Locate a Skyrim Special Edition Data directory: `$SKYRIM_DATA`, then common Steam paths.
pub fn locate_data_dir() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("SKYRIM_DATA") {
        let p = PathBuf::from(p);
        if p.is_dir() {
            return Some(p);
        }
    }
    let home = std::env::var("HOME").ok()?;
    let candidates = [
        ".local/share/Steam/steamapps/common/Skyrim Special Edition/Data",
        ".steam/steam/steamapps/common/Skyrim Special Edition/Data",
        ".var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps/common/Skyrim Special Edition/Data",
    ];
    candidates.iter().map(|c| Path::new(&home).join(c)).find(|p| p.join("Skyrim.esm").is_file())
}
