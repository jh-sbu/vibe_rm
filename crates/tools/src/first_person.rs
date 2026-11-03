//! Survey record and animation assets used by the first-person character.
use std::collections::BTreeMap;

use anyhow::Result;
use esp::{FormId, LoadOrder};

pub fn run(data: &str) -> Result<()> {
    let data = std::path::Path::new(data);
    let names = LoadOrder::default_plugin_list(data, None);
    let lo = LoadOrder::load(data, &names)?;
    let vfs = vfs::Vfs::new(data, &names);
    let mesh = |p: String| format!("meshes/{}", p.replace('\\', "/").to_ascii_lowercase());
    let mut counts = BTreeMap::<String, usize>::new();
    for &id in lo.ids_of_type(b"ARMA") {
        let Some(r) = lo.get(id) else { continue };
        let male = r
            .get(b"MOD4")
            .is_some_and(|d| !esp::decode_zstring(d).is_empty());
        let female = r
            .get(b"MOD5")
            .is_some_and(|d| !esp::decode_zstring(d).is_empty());
        *counts
            .entry(format!("ARMA MOD4={male} MOD5={female}"))
            .or_default() += 1;
        if !male && !female {
            let slots = r
                .get(b"BOD2")
                .or_else(|| r.get(b"BODT"))
                .filter(|d| d.len() >= 4)
                .map_or(0, |d| u32::from_le_bytes(d[..4].try_into().unwrap()));
            *counts
                .entry(format!(
                    "ARMA without first-person models slots={slots:08x}"
                ))
                .or_default() += 1;
            println!(
                "omitted {id} {} slots={slots:08x}",
                r.editor_id().unwrap_or_default()
            );
        }
    }
    for &id in lo.ids_of_type(b"WEAP") {
        let Some(r) = lo.get(id) else { continue };
        let target = r
            .get(b"WNAM")
            .filter(|d| d.len() >= 4)
            .map(|d| r.fid(FormId(u32::from_le_bytes(d[..4].try_into().unwrap()))))
            .filter(|f| !f.is_null());
        let status = match target {
            None => "WEAP without WNAM",
            Some(f) => {
                let model = lo
                    .get(f)
                    .filter(|s| s.tag().0 == *b"STAT")
                    .and_then(|s| s.get(b"MODL").map(esp::decode_zstring));
                if model.is_some_and(|m| !m.is_empty() && vfs.read(&mesh(m)).is_some()) {
                    "WEAP WNAM STAT model exists"
                } else {
                    println!(
                        "missing WNAM model {id} {} -> {f}",
                        r.editor_id().unwrap_or_default()
                    );
                    "WEAP WNAM missing STAT/model"
                }
            }
        };
        *counts.entry(status.into()).or_default() += 1;
    }
    // Search plugin subrecords only, never executables or engine memory.
    for tag in [b"RACE", b"ARMA", b"WEAP", b"STAT", b"IDLE", b"GMST"] {
        for &id in lo.ids_of_type(tag) {
            let Some(r) = lo.get(id) else { continue };
            for s in r.subrecords() {
                if s.data
                    .windows(10)
                    .any(|w| w.eq_ignore_ascii_case(b"_1stperson"))
                {
                    println!(
                        "path {} {id} {} {} {:?}",
                        String::from_utf8_lossy(tag),
                        r.editor_id().unwrap_or_default(),
                        s.tag,
                        s.zstring()
                    );
                }
            }
        }
    }
    if let Some(bytes) = vfs.read("meshes/animationdata/firstperson.txt") {
        let text = String::from_utf8_lossy(&bytes);
        let lines: Vec<_> = text.lines().map(str::trim).collect();
        let mut i = 3 + lines
            .get(1)
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
        while i + 5 < lines.len() {
            if lines[i].is_empty() {
                i += 1;
                continue;
            }
            let Ok(n) = lines[i + 5].parse::<usize>() else {
                break;
            };
            for line in &lines[i + 6..(i + 6 + n).min(lines.len())] {
                if let Some((event, time)) = line.rsplit_once(':') {
                    println!("trigger {} {time} {event}", lines[i]);
                    *counts
                        .entry(format!(
                            "trigger {}",
                            event.split('.').next().unwrap_or(event)
                        ))
                        .or_default() += 1;
                }
            }
            i += 6 + n;
        }
    }
    for (key, count) in counts {
        println!("{count:5} {key}");
    }
    Ok(())
}
