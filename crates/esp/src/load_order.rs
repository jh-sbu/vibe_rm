use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::plugin::{GroupHeader, Item, Plugin, group_type};
use crate::record::{Record, RecordHeader};
use crate::strings::{StringTable, StringsKind};
use crate::{Error, FormId, Result, Tag};

/// Where a plugin lives in the global FormID space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Full(u8),
    Light(u16),
}

impl Slot {
    fn globalize(self, local: u32) -> u32 {
        match self {
            Slot::Full(i) => ((i as u32) << 24) | (local & 0x00FF_FFFF),
            Slot::Light(j) => 0xFE00_0000 | ((j as u32) << 12) | (local & 0xFFF),
        }
    }
}

pub struct PluginInfo {
    pub plugin: Plugin,
    pub slot: Slot,
    pub index: usize,
    /// Slot of each master, then own slot as the final element.
    resolve: Vec<Slot>,
}

impl PluginInfo {
    /// Convert a FormID read from this plugin's data into the global id space.
    pub fn globalize(&self, local: FormId) -> FormId {
        // Runtime records already carry global IDs, including cross-plugin
        // effect IDs. Never reinterpret their high byte as a master index.
        if self.slot == Slot::Full(0xFF) {
            return local;
        }
        if local.0 == 0 {
            return local;
        }
        let idx = (local.0 >> 24) as usize;
        let slot = self.resolve.get(idx).copied().unwrap_or(self.slot);
        FormId(slot.globalize(local.0))
    }
}

/// Location of a record version inside the load order.
#[derive(Debug, Clone, Copy)]
pub struct RecordRef {
    pub plugin: u16,
    pub offset: u32,
    pub tag: Tag,
}

#[derive(Debug, Default, Clone)]
pub struct CellIndex {
    pub world: Option<FormId>,
    pub grid: Option<(i32, i32)>,
    pub persistent: Vec<FormId>,
    pub temporary: Vec<FormId>,
    pub land: Option<FormId>,
    pub navmeshes: Vec<FormId>,
}

#[derive(Debug, Default, Clone)]
pub struct WorldIndex {
    pub persistent_cell: Option<FormId>,
    pub cells: HashMap<(i32, i32), FormId>,
}

/// A record fetched from the load order, carrying the plugin it came from so
/// that FormIDs inside its fields can be globalised.
pub struct LoadedRecord<'a> {
    pub record: Record<'a>,
    pub plugin: &'a PluginInfo,
    pub form_id: FormId,
}

impl<'a> std::ops::Deref for LoadedRecord<'a> {
    type Target = Record<'a>;
    fn deref(&self) -> &Record<'a> {
        &self.record
    }
}

impl LoadedRecord<'_> {
    pub fn fid(&self, local: FormId) -> FormId {
        self.plugin.globalize(local)
    }
}

pub struct LoadOrder {
    data_dir: PathBuf,
    plugins: Vec<PluginInfo>,
    records: HashMap<FormId, RecordRef>,
    by_type: HashMap<Tag, Vec<FormId>>,
    cells: HashMap<FormId, CellIndex>,
    worlds: HashMap<FormId, WorldIndex>,
    ref_cell: HashMap<FormId, FormId>,
    topic_infos: HashMap<FormId, Vec<FormId>>,
    editor_ids: OnceLock<HashMap<String, FormId>>,
    strings: HashMap<(usize, u32), String>,
}

/// The base game masters, in their mandatory order.
pub const BASE_MASTERS: &[&str] = &[
    "Skyrim.esm",
    "Update.esm",
    "Dawnguard.esm",
    "HearthFires.esm",
    "Dragonborn.esm",
];

impl LoadOrder {
    /// Determine the default load order for a data directory: base masters,
    /// Creation Club content from `Skyrim.ccc`, then any `plugins.txt` entries.
    pub fn default_plugin_list(data_dir: &Path, plugins_txt: Option<&Path>) -> Vec<String> {
        let mut list: Vec<String> = Vec::new();
        let exists = |n: &str| data_dir.join(n).is_file();
        let push = |list: &mut Vec<String>, n: &str| {
            if exists(n) && !list.iter().any(|x| x.eq_ignore_ascii_case(n)) {
                list.push(n.to_owned());
            }
        };
        for m in BASE_MASTERS {
            push(&mut list, m);
        }
        if let Some(game_dir) = data_dir.parent()
            && let Ok(ccc) = std::fs::read_to_string(game_dir.join("Skyrim.ccc"))
        {
            for line in ccc.lines() {
                let l = line.trim();
                if !l.is_empty() {
                    push(&mut list, l);
                }
            }
        }
        if let Some(p) = plugins_txt
            && let Ok(txt) = std::fs::read_to_string(p)
        {
            for line in txt.lines() {
                if let Some(n) = line.trim().strip_prefix('*') {
                    if !exists(n) {
                        log::warn!(
                            "{}: {n} is not in {}; skipping it",
                            p.display(),
                            data_dir.display()
                        );
                    }
                    push(&mut list, n);
                }
            }
        }
        list
    }

    pub fn load(data_dir: impl AsRef<Path>, names: &[String]) -> Result<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        let mut opened = Vec::new();
        for n in names {
            let p = Plugin::open(data_dir.join(n))?;
            opened.push(p);
        }
        // Masters (ESM flag or .esl/.esm extension) load before regular plugins.
        let is_masterish = |p: &Plugin| {
            let lower = p.name().to_ascii_lowercase();
            p.header().is_master() || lower.ends_with(".esm") || lower.ends_with(".esl")
        };
        opened.sort_by_key(|p| !is_masterish(p));

        let mut plugins: Vec<PluginInfo> = Vec::new();
        let mut next_full = 0u8;
        let mut next_light = 0u16;
        for p in opened {
            let light = p.header().is_light() || p.name().to_ascii_lowercase().ends_with(".esl");
            let slot = if light {
                next_light += 1;
                Slot::Light(next_light - 1)
            } else {
                next_full += 1;
                Slot::Full(next_full - 1)
            };
            let mut resolve = Vec::new();
            for m in &p.header().masters {
                let found = plugins
                    .iter()
                    .find(|x| x.plugin.name().eq_ignore_ascii_case(m))
                    .ok_or_else(|| Error::MissingMaster {
                        plugin: p.name().to_owned(),
                        master: m.clone(),
                    })?;
                resolve.push(found.slot);
            }
            resolve.push(slot);
            let index = plugins.len();
            plugins.push(PluginInfo {
                plugin: p,
                slot,
                index,
                resolve,
            });
        }

        let mut lo = LoadOrder {
            data_dir,
            plugins,
            records: HashMap::new(),
            by_type: HashMap::new(),
            cells: HashMap::new(),
            worlds: HashMap::new(),
            ref_cell: HashMap::new(),
            topic_infos: HashMap::new(),
            editor_ids: OnceLock::new(),
            strings: HashMap::new(),
        };
        for i in 0..lo.plugins.len() {
            lo.index_plugin(i)?;
        }
        let index = lo.plugins.len();
        lo.plugins.push(PluginInfo {
            plugin: Plugin::runtime(),
            slot: Slot::Full(0xFF),
            index,
            resolve: Vec::new(),
        });
        Ok(lo)
    }

    fn index_plugin(&mut self, pi: usize) -> Result<()> {
        // Collect items first to avoid borrowing self.plugins while mutating indices.
        let plugin = &self.plugins[pi].plugin;
        let tops: Vec<Item> = plugin.top_level().collect();
        for item in tops {
            if let Item::Group(gh, off) = item {
                self.walk_group(pi, gh, off, None, None)?;
            }
        }
        Ok(())
    }

    fn walk_group(
        &mut self,
        pi: usize,
        gh: GroupHeader,
        off: usize,
        world: Option<FormId>,
        cell: Option<(FormId, i32)>,
    ) -> Result<()> {
        let items: Vec<Item> = self.plugins[pi].plugin.group_items(&gh, off).collect();
        let mut world = world;
        for item in items {
            match item {
                Item::Group(child, coff) => match child.group_type {
                    group_type::WORLD_CHILDREN => {
                        let w = self.plugins[pi].globalize(FormId(child.label_u32()));
                        self.walk_group(pi, child, coff, Some(w), None)?;
                    }
                    group_type::CELL_CHILDREN => {
                        let c = self.plugins[pi].globalize(FormId(child.label_u32()));
                        self.walk_group(
                            pi,
                            child,
                            coff,
                            world,
                            Some((c, group_type::CELL_CHILDREN)),
                        )?;
                    }
                    group_type::TOPIC_CHILDREN => {
                        let topic = self.plugins[pi].globalize(FormId(child.label_u32()));
                        let items: Vec<Item> =
                            self.plugins[pi].plugin.group_items(&child, coff).collect();
                        for it in items {
                            if let Item::Record(rh, roff) = it {
                                let id = self.plugins[pi].globalize(rh.form_id);
                                self.add_record(pi, rh, roff, id, world, None)?;
                                let list = self.topic_infos.entry(topic).or_default();
                                if !list.contains(&id) {
                                    list.push(id);
                                }
                            }
                        }
                    }
                    group_type::CELL_PERSISTENT | group_type::CELL_TEMPORARY => {
                        let c = self.plugins[pi].globalize(FormId(child.label_u32()));
                        self.walk_group(pi, child, coff, world, Some((c, child.group_type)))?;
                    }
                    _ => self.walk_group(pi, child, coff, world, cell)?,
                },
                Item::Record(rh, roff) => {
                    let id = self.plugins[pi].globalize(rh.form_id);
                    self.add_record(pi, rh, roff, id, world, cell)?;
                    if rh.tag.0 == *b"WRLD" {
                        // The following world-children group carries the world id itself.
                        world = Some(id);
                    }
                }
            }
        }
        Ok(())
    }

    fn add_record(
        &mut self,
        pi: usize,
        rh: RecordHeader,
        roff: usize,
        id: FormId,
        world: Option<FormId>,
        cell: Option<(FormId, i32)>,
    ) -> Result<()> {
        let prev = self.records.insert(
            id,
            RecordRef {
                plugin: pi as u16,
                offset: roff as u32,
                tag: rh.tag,
            },
        );
        if prev.is_none() {
            self.by_type.entry(rh.tag).or_default().push(id);
        }
        match &rh.tag.0 {
            b"CELL" => {
                let rec = self.plugins[pi].plugin.record_at(roff)?;
                let entry = self.cells.entry(id).or_default();
                if let Some(w) = world {
                    entry.world = Some(w);
                    let grid = rec
                        .subrecords()
                        .find(|s| s.tag.0 == *b"XCLC")
                        .map(|s| (s.i32(0), s.i32(4)));
                    // DATA flag 0x1 = interior. The persistent cell of a world
                    // is flagged persistent (0x400) and has no meaningful grid.
                    let wi = self.worlds.entry(w).or_default();
                    if rh.flags & crate::record::record_flags::PERSISTENT != 0 {
                        wi.persistent_cell = Some(id);
                    } else if let Some(g) = grid {
                        entry.grid = Some(g);
                        wi.cells.insert(g, id);
                    }
                }
            }
            _ => {
                if let Some((cid, kind)) = cell {
                    let entry = self.cells.entry(cid).or_default();
                    match &rh.tag.0 {
                        b"LAND" => entry.land = Some(id),
                        b"NAVM" => {
                            if !entry.navmeshes.contains(&id) {
                                entry.navmeshes.push(id)
                            }
                        }
                        _ => {
                            let list = if kind == group_type::CELL_PERSISTENT {
                                &mut entry.persistent
                            } else {
                                &mut entry.temporary
                            };
                            if prev.is_none() {
                                list.push(id);
                            }
                            self.ref_cell.insert(id, cid);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }
    pub fn plugins(&self) -> &[PluginInfo] {
        // Runtime records are save state, never a load-order dependency.
        &self.plugins[..self.plugins.len() - 1]
    }

    /// Insert an owned runtime ALCH/ENCH. The caller allocates the ID from
    /// the same sequence as created references. Replacements are forbidden.
    pub fn insert_runtime_record(&mut self, bytes: &[u8]) -> Result<FormId> {
        use crate::record::RECORD_HEADER_SIZE;
        if bytes.len() < RECORD_HEADER_SIZE {
            return Err(Error::Corrupt("short runtime record".into()));
        }
        let h = RecordHeader::parse(bytes);
        if h.form_id.0 < 0xFF00_0800
            || h.is_compressed()
            || h.flags != 0
            || !matches!(&h.tag.0, b"ALCH" | b"ENCH")
            || bytes.len() != RECORD_HEADER_SIZE + h.data_size as usize
            || self.records.contains_key(&h.form_id)
        {
            return Err(Error::Corrupt("invalid or duplicate runtime record".into()));
        }
        // Validate every subrecord boundary before allowing save bytes into
        // ordinary record readers. Runtime builders don't need XXXX fields.
        let mut at = RECORD_HEADER_SIZE;
        while at < bytes.len() {
            let Some(head) = bytes.get(at..at + 6) else {
                return Err(Error::Corrupt("short runtime field".into()));
            };
            if &head[..4] == b"XXXX" {
                return Err(Error::Corrupt("oversized runtime field".into()));
            }
            at += 6 + u16::from_le_bytes(head[4..6].try_into().unwrap()) as usize;
            if at > bytes.len() {
                return Err(Error::Corrupt("runtime field overrun".into()));
            }
        }
        let pi = self.plugins.len() - 1;
        let offset = self.plugins[pi].plugin.append_runtime(bytes)?;
        self.records.insert(
            h.form_id,
            RecordRef {
                plugin: pi as u16,
                offset,
                tag: h.tag,
            },
        );
        self.by_type.entry(h.tag).or_default().push(h.form_id);
        self.editor_ids.take();
        Ok(h.form_id)
    }

    /// Serialized records in ID order, independent of insertion order.
    pub fn runtime_records(&self) -> Vec<Vec<u8>> {
        let pi = self.plugins.len() - 1;
        let mut records: Vec<_> = self
            .records
            .iter()
            .filter(|(_, r)| r.plugin as usize == pi)
            .collect();
        records.sort_by_key(|(id, _)| **id);
        records
            .into_iter()
            .map(|(_, r)| {
                let data = &self.plugins[pi].plugin.bytes()[r.offset as usize..];
                let h = RecordHeader::parse(data);
                data[..24 + h.data_size as usize].to_vec()
            })
            .collect()
    }

    pub fn clear_runtime_records(&mut self) {
        let pi = self.plugins.len() - 1;
        self.records.retain(|_, r| r.plugin as usize != pi);
        for ids in self.by_type.values_mut() {
            ids.retain(|id| id.0 >> 24 != 0xFF);
        }
        self.plugins[pi].plugin = Plugin::runtime();
        self.editor_ids.take();
    }
    /// The plugin that owns a global FormID's prefix, and the id local to it
    /// (as used in e.g. FaceGen file names).
    pub fn origin(&self, id: FormId) -> Option<(&str, u32)> {
        let top = id.0 >> 24;
        let (slot, local) = if top == 0xFE {
            (Slot::Light(((id.0 >> 12) & 0xFFF) as u16), id.0 & 0xFFF)
        } else {
            (Slot::Full(top as u8), id.0 & 0x00FF_FFFF)
        };
        self.plugins
            .iter()
            .find(|p| p.slot == slot)
            .map(|p| (p.plugin.name(), local))
    }

    pub fn record_count(&self) -> usize {
        self.records.len()
    }
    pub fn locate(&self, id: FormId) -> Option<RecordRef> {
        self.records.get(&id).copied()
    }
    pub fn tag_of(&self, id: FormId) -> Option<Tag> {
        self.records.get(&id).map(|r| r.tag)
    }

    /// Fetch the winning version of a record.
    pub fn get(&self, id: FormId) -> Option<LoadedRecord<'_>> {
        let r = self.records.get(&id)?;
        let info = &self.plugins[r.plugin as usize];
        match info.plugin.record_at(r.offset as usize) {
            Ok(record) => Some(LoadedRecord {
                record,
                plugin: info,
                form_id: id,
            }),
            Err(e) => {
                log::error!("failed to read record {id}: {e}");
                None
            }
        }
    }

    /// DOBJ is a keyed table: later plugins can contain sparse named updates.
    /// Returns each named entry's last value, including explicit null targets.
    pub fn default_objects(&self) -> HashMap<[u8; 4], FormId> {
        let mut objects = HashMap::new();
        for info in &self.plugins {
            for item in info.plugin.top_level() {
                let Item::Group(header, offset) = item else {
                    continue;
                };
                if header.label != *b"DOBJ" {
                    continue;
                }
                for item in info.plugin.group_items(&header, offset) {
                    let Item::Record(h, offset) = item else {
                        continue;
                    };
                    if h.tag.0 != *b"DOBJ" {
                        continue;
                    }
                    let Ok(rec) = info.plugin.record_at(offset) else {
                        continue;
                    };
                    for e in rec.get(b"DNAM").unwrap_or_default().chunks_exact(8) {
                        let key: [u8; 4] = e[..4].try_into().unwrap();
                        if key == [0; 4] {
                            continue;
                        }
                        objects.insert(
                            key,
                            info.globalize(FormId(u32::from_le_bytes(e[4..].try_into().unwrap()))),
                        );
                    }
                }
            }
        }
        objects
    }

    pub fn ids_of_type(&self, tag: &[u8; 4]) -> &[FormId] {
        self.by_type
            .get(&Tag(*tag))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
    pub fn cell(&self, id: FormId) -> Option<&CellIndex> {
        self.cells.get(&id)
    }
    pub fn world(&self, id: FormId) -> Option<&WorldIndex> {
        self.worlds.get(&id)
    }
    /// INFO records belonging to a dialogue topic, in file order.
    pub fn topic_infos(&self, topic: FormId) -> &[FormId] {
        self.topic_infos
            .get(&topic)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn cell_of_ref(&self, id: FormId) -> Option<FormId> {
        self.ref_cell.get(&id).copied()
    }

    fn editor_id_map(&self) -> &HashMap<String, FormId> {
        self.editor_ids.get_or_init(|| {
            let mut map = HashMap::new();
            for (&id, r) in &self.records {
                // References, landscape, navmeshes and dialogue infos are by far the most
                // numerous records and almost never carry useful editor ids.
                if matches!(
                    &r.tag.0,
                    b"REFR" | b"ACHR" | b"LAND" | b"NAVM" | b"INFO" | b"PGRE" | b"PHZD"
                ) {
                    continue;
                }
                if let Some(rec) = self.get(id)
                    && let Some(e) = rec.editor_id()
                {
                    map.insert(e.to_ascii_lowercase(), id);
                }
            }
            map
        })
    }

    pub fn find_editor_id(&self, edid: &str) -> Option<FormId> {
        self.editor_id_map()
            .get(&edid.to_ascii_lowercase())
            .copied()
    }

    /// Load localised string tables for every localised plugin. `read` should
    /// return a virtual file's contents (e.g. from loose files or archives).
    pub fn load_strings(&mut self, language: &str, read: impl Fn(&str) -> Option<Vec<u8>>) {
        for info in &self.plugins {
            if !info.plugin.header().is_localized() {
                continue;
            }
            let stem = info
                .plugin
                .name()
                .rsplit_once('.')
                .map(|x| x.0)
                .unwrap_or(info.plugin.name());
            for ext in ["strings", "dlstrings", "ilstrings"] {
                let path = format!("strings/{stem}_{language}.{ext}");
                if let Some(data) = read(&path.to_ascii_lowercase()) {
                    if let Some(t) = StringTable::parse(&data, StringsKind::from_extension(ext)) {
                        for (id, s) in t.entries {
                            self.strings.insert((info.index, id), s);
                        }
                    }
                } else {
                    log::warn!("missing string table {path}");
                }
            }
        }
    }

    /// Resolve a localisable string field from a record.
    pub fn lstring(&self, rec: &LoadedRecord<'_>, data: &[u8]) -> String {
        if rec.plugin.plugin.header().is_localized() && data.len() == 4 {
            let id = u32::from_le_bytes(data.try_into().unwrap());
            if id == 0 {
                return String::new();
            }
            self.strings
                .get(&(rec.plugin.index, id))
                .cloned()
                .unwrap_or_else(|| format!("<string {id:08X}>"))
        } else {
            crate::decode_zstring(data)
        }
    }
}

#[cfg(test)]
mod runtime_tests {
    use super::*;
    use crate::writer::RecordBuilder;

    #[test]
    fn runtime_forms_use_all_normal_lookup_paths_without_globalizing_again() {
        let mut lo = LoadOrder::load(std::env::temp_dir(), &[]).unwrap();
        let id = FormId(0xFF00_0801);
        let record = RecordBuilder::new(b"ALCH", id)
            .zstring(b"FULL", "Potion")
            .form(b"EFID", FormId(0x0400_1234));
        lo.insert_runtime_record(&record.to_bytes()).unwrap();
        assert!(lo.plugins().is_empty());
        assert_eq!(lo.tag_of(id), Some(Tag(*b"ALCH")));
        assert_eq!(lo.ids_of_type(b"ALCH"), &[id]);
        let r = lo.get(id).unwrap();
        assert_eq!(r.fid(FormId(0x0400_1234)), FormId(0x0400_1234));
        assert_eq!(lo.lstring(&r, r.get(b"FULL").unwrap()), "Potion");
        assert!(lo.insert_runtime_record(&record.to_bytes()).is_err());
        let second = RecordBuilder::new(b"ENCH", FormId(0xFF00_0800)).to_bytes();
        lo.insert_runtime_record(&second).unwrap();
        assert_eq!(lo.runtime_records(), vec![second, record.to_bytes()]);
        lo.clear_runtime_records();
        assert!(lo.get(id).is_none());
        assert!(lo.ids_of_type(b"ALCH").is_empty());
    }

    #[test]
    fn runtime_records_reject_bad_ids_tags_and_truncated_fields() {
        let mut lo = LoadOrder::load(std::env::temp_dir(), &[]).unwrap();
        for bytes in [
            vec![],
            vec![0; 24],
            RecordBuilder::new(b"ALCH", FormId(0x800)).to_bytes(),
            RecordBuilder::new(b"REFR", FormId(0xFF00_0800)).to_bytes(),
        ] {
            assert!(lo.insert_runtime_record(&bytes).is_err());
        }
        let mut bytes = RecordBuilder::new(b"ALCH", FormId(0xFF00_0800))
            .zstring(b"FULL", "Potion")
            .to_bytes();
        bytes[28..30].copy_from_slice(&100u16.to_le_bytes());
        assert!(lo.insert_runtime_record(&bytes).is_err());
        assert!(lo.runtime_records().is_empty());
    }
}
