//! The Interface files the menus load: movies, font libraries and text.

use std::collections::HashMap;
use std::sync::Arc;

/// The movies (`.swf`, `.gfx`) and text files (`fontconfig.txt`, the
/// translations) under `interface/`, read once from the Data directory (loose
/// files over the archives). Looked up case-insensitively, as the game does:
/// the archive's names are lower case and the menus load mixed-case ones
/// (`Inventory components/ItemCard.swf`, `TextEntry.swf`).
pub struct InterfaceFiles {
    files: HashMap<String, Arc<[u8]>>,
}

impl InterfaceFiles {
    pub fn load(vfs: &vfs::Vfs) -> Self {
        let files: HashMap<String, Arc<[u8]>> = vfs
            .list("interface/")
            .into_iter()
            .filter(|p| [".swf", ".gfx", ".txt"].iter().any(|e| p.ends_with(e)))
            .filter_map(|p| Some((normalize(&p), Arc::from(vfs.read(&p)?))))
            .collect();
        log::info!("{} interface files", files.len());
        InterfaceFiles { files }
    }

    /// A file by its path under `interface/` or from the Data directory
    /// (`hudmenu.swf`, `interface/fontconfig.txt`), either slash, any case.
    pub fn get(&self, path: &str) -> Option<Arc<[u8]>> {
        let p = normalize(path);
        self.files
            .get(&p)
            .or_else(|| self.files.get(&format!("interface/{p}")))
            .cloned()
    }
}

fn normalize(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches('/')
        .to_lowercase()
}
