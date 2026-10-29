//! Authored station, recipe and script survey; never reads engine binaries.
use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use esp::{FormId, LoadOrder};

pub fn run(data: &str) -> Result<()> {
    let data = std::path::Path::new(data);
    // Deliberately exclude Creations and installed mods from the vanilla survey.
    let names: Vec<_> = esp::BASE_MASTERS
        .iter()
        .filter(|n| data.join(n).is_file())
        .map(|n| (*n).to_owned())
        .collect();
    let lo = LoadOrder::load(data, &names)?;
    let vfs = vfs::Vfs::new(data, &names);
    let label = |id| {
        lo.get(id)
            .and_then(|r| r.editor_id())
            .unwrap_or_else(|| format!("{id}"))
    };
    let mut stations = BTreeMap::<u8, usize>::new();
    let mut scripts = BTreeSet::new();
    let mut bases = BTreeSet::new();
    for &id in lo.ids_of_type(b"FURN") {
        let Some(r) = lo.get(id) else { continue };
        let Some(d) = r.get(b"WBDT").filter(|d| d.len() >= 2 && d[0] != 0) else {
            continue;
        };
        *stations.entry(d[0]).or_default() += 1;
        bases.insert(id);
        let keywords: Vec<_> = r
            .get(b"KWDA")
            .unwrap_or(&[])
            .chunks_exact(4)
            .map(|b| label(r.fid(FormId(u32::from_le_bytes(b.try_into().unwrap())))))
            .collect();
        println!(
            "station {id} {} type={} skill={} keywords={keywords:?}",
            label(id),
            d[0],
            d[1]
        );
        if let Some(v) = crate::vmad::parse(&r) {
            scripts.extend(v.scripts.into_iter().map(|s| s.name));
        }
    }
    // Placed stations can have scripts independent of their base furniture.
    for &id in lo.ids_of_type(b"REFR") {
        let Some(r) = lo.get(id) else { continue };
        let base = r
            .get(b"NAME")
            .and_then(|d| d.get(..4))
            .map(|d| r.fid(FormId(u32::from_le_bytes(d.try_into().unwrap()))));
        if base.is_some_and(|b| bases.contains(&b))
            && let Some(v) = crate::vmad::parse(&r)
        {
            for s in v.scripts {
                println!("placed station {id} script={}", s.name);
                scripts.insert(s.name);
            }
        }
    }
    let mut benches = BTreeMap::<String, usize>::new();
    let mut functions = BTreeMap::<u16, usize>::new();
    for &id in lo.ids_of_type(b"COBJ") {
        let Some(r) = lo.get(id) else { continue };
        if r.get(b"CNAM").is_none()
            || r.get(b"NAM1")
                .is_none_or(|d| d.len() < 2 || d[..2] == [0, 0])
        {
            println!(
                "incomplete recipe {id} {} CNAM={:?} NAM1={:?}",
                label(id),
                r.get(b"CNAM"),
                r.get(b"NAM1")
            );
        }
        if let Some(b) = r.get(b"BNAM").and_then(|d| d.get(..4)) {
            let bench = r.fid(FormId(u32::from_le_bytes(b.try_into().unwrap())));
            *benches.entry(label(bench)).or_default() += 1;
        }
        for s in r
            .subrecords()
            .filter(|s| s.tag.0 == *b"CTDA" && s.data.len() >= 10)
        {
            *functions
                .entry(u16::from_le_bytes([s.data[8], s.data[9]]))
                .or_default() += 1;
        }
    }
    println!("station totals: {stations:?}");
    for (bench, count) in benches {
        println!("recipes {bench}: {count}");
    }
    for (f, count) in functions {
        println!(
            "condition {f} {}: {count}",
            crate::functions::find(f).map_or("unknown", |f| f.1)
        );
    }
    for script in scripts {
        let path = format!("scripts/{}.pex", script.to_ascii_lowercase());
        let events: BTreeSet<_> = vfs
            .read(&path)
            .and_then(|b| papyrus::pex::parse(&b).ok())
            .map(|p| {
                p.objects
                    .iter()
                    .flat_map(|o| &o.states)
                    .flat_map(|s| &s.functions)
                    .map(|(n, _)| p.str(*n).to_owned())
                    .filter(|n| n.to_ascii_lowercase().starts_with("on"))
                    .collect()
            })
            .unwrap_or_default();
        println!("script {script}: {events:?}");
    }
    Ok(())
}
