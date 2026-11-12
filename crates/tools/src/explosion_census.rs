//! `explosion-census <data dir> [edid substring]`: every `EXPL` record's
//! `DATA` fields (layout: xEdit `wbDefinitionsTES5.pas`, UESP's EXPL page),
//! its enchantment's effects, and who refers to it: magic effects (`MGEF`
//! `DATA` 0x4C), projectiles (`PROJ` `DATA` 0x24), destruction stages
//! (`DSTD` 0x08), dual-cast data (`DUAL` `DATA` 0x04), body parts (`BPTD`
//! explodable / severable) and script properties (`VMAD`). Plugin records
//! only.
use anyhow::Result;
use esp::FormId;
use std::collections::BTreeMap;

pub fn run(data: &str, want: Option<&str>) -> Result<()> {
    let data = std::path::Path::new(data);
    let names = esp::LoadOrder::default_plugin_list(data, None);
    let lo = esp::LoadOrder::load(data, &names)?;
    let edid = |id: FormId| {
        lo.get(id)
            .and_then(|r| r.editor_id().map(|e| e.to_string()))
            .unwrap_or_else(|| id.to_string())
    };
    let explosions: std::collections::HashSet<FormId> =
        lo.ids_of_type(b"EXPL").iter().copied().collect();
    // Users of each explosion.
    let mut users: BTreeMap<FormId, Vec<String>> = BTreeMap::new();
    let mut note = |e: FormId, what: String| {
        if explosions.contains(&e) {
            users.entry(e).or_default().push(what);
        }
    };
    let form_at = |r: &esp::LoadedRecord<'_>, d: &[u8], o: usize| {
        d.get(o..o + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .filter(|&v| v != 0)
            .map(|v| r.fid(FormId(v)))
    };
    for (tag, off) in [(b"MGEF", 0x4C), (b"PROJ", 0x24), (b"DUAL", 0x04)] {
        for &id in lo.ids_of_type(tag) {
            let Some(r) = lo.get(id) else { continue };
            if let Some(e) = r.get(b"DATA").and_then(|d| form_at(&r, d, off)) {
                note(e, format!("{} {}", r.tag(), edid(id)));
            }
        }
    }
    let tags: &[&[u8; 4]] = &[
        b"ACTI", b"CONT", b"DOOR", b"FURN", b"MSTT", b"STAT", b"TREE", b"FLOR", b"LIGH", b"MISC",
        b"WEAP", b"ARMO", b"NPC_", b"QUST", b"MGEF", b"SPEL", b"ENCH", b"SCRL", b"PROJ", b"TACT",
        b"HAZD", b"ALCH", b"BOOK", b"AMMO", b"INGR",
    ];
    for tag in tags {
        for &id in lo.ids_of_type(tag) {
            let Some(r) = lo.get(id) else { continue };
            for s in r.subrecords() {
                if s.tag.0 == *b"DSTD"
                    && let Some(e) = form_at(&r, s.data, 8)
                {
                    note(e, format!("DEST {} {}", r.tag(), edid(id)));
                }
            }
            if let Some(v) = crate::vmad::parse(&r) {
                for sc in v.scripts {
                    for (prop, value) in &sc.properties {
                        let mut forms = Vec::new();
                        collect_forms(value, &mut forms);
                        for f in forms {
                            note(f, format!("VMAD {} {}.{prop}", edid(id), sc.name));
                        }
                    }
                }
            }
        }
    }
    for &id in lo.ids_of_type(b"BPTD") {
        let Some(r) = lo.get(id) else { continue };
        for s in r.subrecords() {
            if s.tag.0 == *b"BPND" {
                for (o, kind) in [(0x0C, "explodable"), (0x1C, "severable")] {
                    if let Some(e) = form_at(&r, s.data, o) {
                        note(e, format!("BPTD {} {kind}", edid(id)));
                    }
                }
            }
        }
    }
    for &id in lo.ids_of_type(b"EXPL") {
        let Some(r) = lo.get(id) else { continue };
        let name = edid(id);
        if want.is_some_and(|w| !name.to_ascii_lowercase().contains(&w.to_ascii_lowercase())) {
            continue;
        }
        let Some(d) = r.get(b"DATA") else {
            println!("EXPL {id} {name} no DATA");
            continue;
        };
        let f = |o: usize| {
            d.get(o..o + 4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        };
        let u = |o: usize| {
            d.get(o..o + 4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        };
        let form = |o: usize| {
            form_at(&r, d, o).map(|f| {
                format!(
                    "{}({})",
                    edid(f),
                    lo.tag_of(f).map(|t| t.to_string()).unwrap_or_default()
                )
            })
        };
        let model = r.get(b"MODL").map(|m| {
            String::from_utf8_lossy(m)
                .trim_end_matches('\0')
                .to_string()
        });
        let ench = r.get(b"EITM").and_then(|e| form_at(&r, e, 0));
        let imad = r.get(b"MNAM").and_then(|e| form_at(&r, e, 0));
        println!(
            "EXPL {id} {name} len={} model={model:?} ench={:?} imad={:?} light={:?} snd1={:?} snd2={:?} ipds={:?} placed={:?} spawn={:?} force={:?} damage={:?} radius={:?} is_radius={:?} vmult={:?} flags={:?} level={:?}",
            d.len(),
            ench.map(edid),
            imad.map(edid),
            form(0),
            form(4),
            form(8),
            form(12),
            form(16),
            form(20),
            f(24),
            f(28),
            f(32),
            f(36),
            f(40),
            u(44).map(|v| format!("{v:#x}")),
            u(48),
        );
        if let Some(e) = ench.and_then(|e| lo.get(e)) {
            let mut effect = None;
            for s in e.subrecords() {
                match &s.tag.0 {
                    b"EFID" => effect = form_at(&e, s.data, 0),
                    b"EFIT" if s.data.len() >= 12 => println!(
                        "  effect {} magnitude={} area={} duration={}",
                        effect.map(edid).unwrap_or_default(),
                        s.f32(0),
                        u32::from_le_bytes(s.data[4..8].try_into().unwrap()),
                        u32::from_le_bytes(s.data[8..12].try_into().unwrap())
                    ),
                    _ => {}
                }
            }
        }
        for u in users.get(&id).into_iter().flatten() {
            println!("  used by {u}");
        }
    }
    Ok(())
}

fn collect_forms(v: &crate::vmad::PropValue, out: &mut Vec<FormId>) {
    match v {
        crate::vmad::PropValue::Object { form, .. } => out.push(*form),
        crate::vmad::PropValue::Array(items) => {
            items.iter().for_each(|i| collect_forms(i, out));
        }
        _ => {}
    }
}
