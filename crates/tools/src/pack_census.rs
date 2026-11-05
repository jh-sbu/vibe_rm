//! All PACK records, including quest, scene and alias packages (not just NPC slots).

use std::collections::BTreeMap;
use std::fmt::Write;

use anyhow::{Context, Result, ensure};
use esp::{FormId, LoadOrder};

use crate::package;

#[derive(Default)]
struct Template {
    count: usize,
    behaviours: BTreeMap<String, usize>,
    users: Vec<(FormId, String)>,
}

#[derive(Default)]
struct Census {
    packages: usize,
    templates: BTreeMap<String, Template>,
    procedures: BTreeMap<String, usize>,
    locations: BTreeMap<u32, usize>,
    targets: BTreeMap<u32, usize>,
    acquire_flags: BTreeMap<String, usize>,
}

impl Census {
    fn read(lo: &LoadOrder) -> Self {
        let mut out = Self::default();
        for &id in lo.ids_of_type(b"PACK") {
            let Some(rec) = lo.get(id) else { continue };
            out.packages += 1;
            let template = rec
                .get(b"PKCU")
                .filter(|d| d.len() >= 8)
                .map(|d| rec.fid(FormId(u32::from_le_bytes(d[4..8].try_into().unwrap()))))
                .filter(|t| !t.is_null());
            let acquire = template.and_then(|t| lo.get(t)).is_some_and(|t| {
                t.subrecords()
                    .any(|s| s.tag.0 == *b"PNAM" && s.zstring() == "Acquire")
            });
            let name = template.map_or_else(
                || "<own tree>".to_owned(),
                |t| {
                    lo.get(t)
                        .and_then(|r| r.editor_id())
                        .unwrap_or_else(|| t.to_string())
                },
            );
            let group = out.templates.entry(name).or_default();
            group.count += 1;
            group.users.push((id, rec.editor_id().unwrap_or_default()));
            // Count each resulting branch, while counting the PACK itself only once.
            for p in package::expand(lo, id) {
                *group
                    .behaviours
                    .entry(format!("{:?}", p.behaviour))
                    .or_default() += 1;
            }
            let mut tree = false;
            for sr in rec.subrecords() {
                match &sr.tag.0 {
                    b"XNAM" => tree = true,
                    b"UNAM" if tree => break,
                    b"PNAM" if tree => *out.procedures.entry(sr.zstring()).or_default() += 1,
                    b"PLDT" if sr.data.len() >= 12 => {
                        *out.locations.entry(sr.u32(0)).or_default() += 1
                    }
                    b"PTDA" if sr.data.len() >= 8 => {
                        *out.targets.entry(sr.u32(0)).or_default() += 1
                    }
                    _ => {}
                }
            }
            // Named Bool inputs; count explicitly enabled permissions, not defaults.
            let source = template.unwrap_or(id);
            let mut names = BTreeMap::new();
            let (mut tree, mut index) = (false, None);
            if let Some(t) = lo.get(source) {
                for sr in t.subrecords() {
                    match &sr.tag.0 {
                        b"XNAM" => tree = true,
                        b"UNAM" if tree => index = sr.data.first().copied(),
                        b"BNAM" if tree => {
                            if let Some(i) = index.take() {
                                let name: String = sr
                                    .zstring()
                                    .chars()
                                    .filter(char::is_ascii_alphanumeric)
                                    .flat_map(char::to_lowercase)
                                    .collect();
                                names.insert(i, name);
                            }
                        }
                        _ => {}
                    }
                }
            }
            let (mut enabled, mut indices, mut kind) = (Vec::new(), Vec::new(), String::new());
            for sr in rec.subrecords() {
                match &sr.tag.0 {
                    b"XNAM" => break,
                    b"ANAM" => {
                        kind = sr.zstring();
                        enabled.push(false);
                    }
                    b"CNAM" if kind == "Bool" && !enabled.is_empty() => {
                        *enabled.last_mut().unwrap() = sr.data.first().is_some_and(|v| *v != 0)
                    }
                    b"UNAM" if !sr.data.is_empty() => indices.push(sr.data[0]),
                    _ => {}
                }
            }
            for (i, on) in indices.into_iter().zip(enabled) {
                if let Some(name) = names.get(&i).filter(|_| on) {
                    if acquire
                        && ["allowstealing", "allowpickpocketing", "allowkilling"]
                            .contains(&name.as_str())
                    {
                        *out.acquire_flags.entry(name.clone()).or_default() += 1;
                    }
                }
            }
        }
        for t in out.templates.values_mut() {
            t.users.sort();
        }
        out
    }

    fn report(&self, show: Option<&str>) -> String {
        let mut s = format!(
            "{} PACK records\nTemplates (PACK count; expanded branch behaviours):\n",
            self.packages
        );
        let mut templates: Vec<_> = self.templates.iter().collect();
        templates.sort_by(|a, b| b.1.count.cmp(&a.1.count).then(a.0.cmp(b.0)));
        for (name, t) in templates {
            writeln!(s, "{:6} {name}: {:?}", t.count, t.behaviours).unwrap();
            if show.is_some_and(|v| v.eq_ignore_ascii_case(name)) {
                for (id, name) in &t.users {
                    writeln!(s, "         {id} {name}").unwrap();
                }
            }
        }
        s.push_str("Procedures in records' own trees (each tree counted once):\n");
        for (name, count) in &self.procedures {
            writeln!(s, "{count:6} {name}").unwrap();
        }
        s.push_str("Location kinds (PLDT):\n");
        for (kind, count) in &self.locations {
            writeln!(s, "{count:6} {kind}").unwrap();
        }
        s.push_str("Target kinds (PTDA):\n");
        for (kind, count) in &self.targets {
            writeln!(s, "{count:6} {kind}").unwrap();
        }
        s.push_str("Explicitly enabled concrete Acquire permissions:\n");
        for name in ["allowstealing", "allowpickpocketing", "allowkilling"] {
            writeln!(
                s,
                "{:6} {name}",
                self.acquire_flags.get(name).copied().unwrap_or(0)
            )
            .unwrap();
        }
        s
    }
}

pub fn run(args: &[String]) -> Result<()> {
    let data = std::path::Path::new(
        args.first()
            .context("usage: pack-census <data> [--masters]")?,
    );
    ensure!(
        args.len() <= 2 && args.get(1).is_none_or(|s| s == "--masters"),
        "usage: pack-census <data> [--masters]"
    );
    let names = if args.get(1).is_some() {
        [
            "Skyrim.esm",
            "Update.esm",
            "Dawnguard.esm",
            "HearthFires.esm",
            "Dragonborn.esm",
        ]
        .map(str::to_owned)
        .to_vec()
    } else {
        LoadOrder::default_plugin_list(data, None)
    };
    let lo = LoadOrder::load(data, &names)?;
    print!(
        "{}",
        Census::read(&lo).report(std::env::var("SHOW").ok().as_deref())
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp::writer::{Node, RecordBuilder, write_plugin};

    #[test]
    fn counts_scene_and_alias_packages_without_npc_slots_and_uses_engine_parser() {
        let dir = std::env::temp_dir().join(format!("vrm-pack-census-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let template = RecordBuilder::new(b"PACK", FormId(0x800))
            .zstring(b"EDID", "Travel")
            .field(b"XNAM", &[])
            .zstring(b"ANAM", "Procedure")
            .zstring(b"PNAM", "Travel");
        let mut cu = 1u32.to_le_bytes().to_vec();
        cu.extend_from_slice(&0x800u32.to_le_bytes());
        let p = RecordBuilder::new(b"PACK", FormId(0x801))
            .zstring(b"EDID", "SceneTravel")
            .field(b"PKCU", &cu)
            .zstring(b"ANAM", "TargetSelector")
            .field(b"PTDA", &[2, 0, 0, 0, 10, 0, 0, 0])
            .field(b"UNAM", &[0]);
        std::fs::write(
            dir.join("Census.esp"),
            write_plugin(
                0,
                &[],
                "test",
                "",
                0x900,
                &[Node::top(b"PACK", vec![template, p])],
            ),
        )
        .unwrap();
        let lo = LoadOrder::load(&dir, &["Census.esp".into()]).unwrap();
        let c = Census::read(&lo);
        assert_eq!(c.packages, 2);
        assert_eq!(c.templates["Travel"].count, 1);
        assert_eq!(c.templates["Travel"].behaviours["Travel"], 1);
        assert_eq!(c.procedures["Travel"], 1);
        assert_eq!(c.targets[&2], 1);
        assert!(c.report(Some("travel")).contains("SceneTravel"));
        assert!(!c.report(None).contains("SceneTravel"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
