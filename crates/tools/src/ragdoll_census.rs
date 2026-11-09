//! Survey only plugin records, skeletons and behavior assets; never engine binaries.
use anyhow::Result;
use havok::behavior::Generator;
use std::collections::{BTreeMap, HashSet};

pub fn run(data: &str) -> Result<()> {
    let data = std::path::Path::new(data);
    let names: Vec<_> = esp::BASE_MASTERS.iter().map(|s| s.to_string()).collect();
    let lo = esp::LoadOrder::load(data, &names)?;
    let v = vfs::Vfs::new(data, &names);
    let mesh = |s: String| format!("meshes/{}", s.replace('\\', "/").to_ascii_lowercase());
    let mut projects = BTreeMap::new();
    for &id in lo.ids_of_type(b"RACE") {
        let Some(r) = lo.get(id) else { continue };
        let flags = r
            .get(b"DATA")
            .filter(|d| d.len() >= 36)
            .map_or(0, |d| u32::from_le_bytes(d[32..36].try_into().unwrap()));
        let mut skeletons = Vec::new();
        let mut behavior_section = false;
        for s in r.subrecords() {
            if s.tag.0 == *b"ANAM" {
                let path = mesh(s.zstring());
                let bodies = v
                    .read(&path)
                    .and_then(|b| nif::Nif::parse(&b).ok())
                    .map(|n| {
                        n.blocks
                            .iter()
                            .filter(|b| matches!(b, nif::Block::RigidBody(_)))
                            .count()
                    });
                skeletons.push((path, bodies));
            }
            if s.tag.0 == *b"NAM3" {
                behavior_section = true;
            }
            if matches!(&s.tag.0, b"NAM4" | b"NAM5") {
                behavior_section = false;
            }
            if behavior_section && s.tag.0 == *b"MODL" {
                let path = mesh(s.zstring());
                projects
                    .entry(path)
                    .or_insert_with(Vec::new)
                    .push(r.editor_id().unwrap_or_default().to_owned());
            }
        }
        println!(
            "RACE {id} {} no_knockdowns={} ragdoll_collision={} skeletons={skeletons:?}",
            r.editor_id().unwrap_or_default(),
            flags & 0x400000 != 0,
            flags & 0x40000 != 0
        );
        let mut knockdown = 0.0;
        for s in r.subrecords() {
            if s.tag.0 == *b"ATKD" && s.data.len() >= 36 {
                knockdown = s.f32(32);
            }
            if s.tag.0 == *b"ATKE" && knockdown != 0.0 {
                println!(
                    "ATKD {} {} knockdown={knockdown}",
                    r.editor_id().unwrap_or_default(),
                    s.zstring()
                );
            }
        }
    }
    for &id in lo.ids_of_type(b"EXPL") {
        let Some(r) = lo.get(id) else { continue };
        if let Some(d) = r.get(b"DATA").filter(|d| d.len() >= 48) {
            let flags = u32::from_le_bytes(d[44..48].try_into().unwrap());
            if flags & 12 != 0 {
                println!(
                    "EXPL {id} {} knock_flags={:x}",
                    r.editor_id().unwrap_or_default(),
                    flags & 12
                );
            }
        }
    }
    for tag in [b"MGEF", b"SPEL"] {
        for &id in lo.ids_of_type(tag) {
            let Some(r) = lo.get(id) else { continue };
            if r.get(b"VMAD").is_some() {
                if let Some(scripts) = crate::vmad::parse(&r) {
                    for s in scripts.scripts {
                        if s.name.eq_ignore_ascii_case("VoicePushEffectScript")
                            || s.name.eq_ignore_ascii_case("werewolfPushEffectScript")
                        {
                            println!("PUSH {id} {} {s:?}", r.editor_id().unwrap_or_default());
                        }
                    }
                }
            }
        }
    }
    let mut seen = HashSet::new();
    for (path, races) in projects {
        let Some((dir, file)) = path.rsplit_once('/') else {
            continue;
        };
        let Ok(p) =
            havok::behavior::Project::load_project(file, |rel| v.read(&format!("{dir}/{rel}")))
        else {
            println!("PROJECT {path} missing");
            continue;
        };
        let up = p
            .graphs
            .iter()
            .flat_map(|(_, g)| &g.generators)
            .any(|g| match g {
                Generator::StateMachine { states, .. } => states.iter().any(|s| {
                    s.name
                        .to_ascii_lowercase()
                        .replace([' ', '_'], "")
                        .contains("getupfromragdoll")
                }),
                _ => false,
            });
        println!("PROJECT {path} get_up={up} races={races:?}");
        for (file, g) in &p.graphs {
            let full = format!("{dir}/{}", file.replace('\\', "/").to_ascii_lowercase());
            if !seen.insert(full.clone()) {
                continue;
            }
            for (i, n) in g.generators.iter().enumerate() {
                if n.name().contains("PoseMatch") || n.name().contains("GetUp") {
                    println!(
                        "GEN {full} {} {:?} bindings={:?}",
                        n.name(),
                        n,
                        g.bindings.get(i)
                    );
                    if let Generator::Selector { children, .. } = n {
                        println!(
                            "SELECT {} {:?}",
                            n.name(),
                            children
                                .iter()
                                .map(|&i| g.generators[i].name())
                                .collect::<Vec<_>>()
                        );
                    }
                }
            }
            for (i, m) in g.modifiers.iter().enumerate() {
                if matches!(
                    m,
                    havok::behavior::Modifier::GetUp(_)
                        | havok::behavior::Modifier::PoweredRagdoll(_)
                        | havok::behavior::Modifier::RigidBodyRagdoll(_)
                        | havok::behavior::Modifier::KeyframeBones(_)
                        | havok::behavior::Modifier::ContactListener(_)
                ) {
                    println!(
                        "PARSED_CONTROL {full} {m:?} bindings={:?} properties={:?}",
                        g.modifier_bindings.get(i),
                        g.modifier_property_bindings.get(i)
                    );
                }
            }
            if let Some(bytes) = v.read(&full) {
                let pack = havok::Packfile::parse(&bytes)?;
                for o in &pack.objects {
                    if let Some(fields) = decode(&pack, o.offset, &o.class) {
                        println!(
                            "CONTROL {full} {} {} {fields}",
                            o.class,
                            pack.string(o.offset + 0x38).unwrap_or_default()
                        );
                    }
                }
            }
        }
    }
    // Summarise constant arguments of calls, without extracting script assets.
    for path in v
        .list("scripts/")
        .into_iter()
        .filter(|p| p.ends_with(".pex"))
    {
        let Some(p) = v.read(&path).and_then(|b| papyrus::pex::parse(&b).ok()) else {
            continue;
        };
        for line in papyrus::pex::disassemble(&p).lines().filter(|l| {
            l.to_ascii_lowercase()
                .contains("callmethod knockareaeffect,")
        }) {
            println!("AREA {path} {}", line.trim());
        }
    }
    Ok(())
}

/// Named serialized fields, using public Behavior Editor member order and the
/// 64-bit SE assets' alignment. These are format observations, not engine code.
pub fn decode(p: &havok::Packfile, o: u32, class: &str) -> Option<String> {
    let f = |a| p.f32(o + a);
    let i = |a| p.i16(o + a);
    let vec = |a| [f(a), f(a + 4), f(a + 8), f(a + 12)];
    Some(match class {
        "hkbPoseMatchingGenerator" => format!(
            "rotation={:?} blend={} min_speed={} switch_times={:?} events={:?} bones={:?} mode={}",
            vec(0xa0),
            f(0xb0),
            f(0xb4),
            [f(0xb8), f(0xbc)],
            [p.i32(o + 0xc0), p.i32(o + 0xc4)],
            [i(0xc8), i(0xca), i(0xcc), i(0xce)],
            p.u8(o + 0xd0)
        ),
        "hkbGetUpModifier" => format!(
            "normal={:?} duration={} align={} bones={:?}",
            vec(0x50),
            f(0x60),
            f(0x64),
            [i(0x68), i(0x6a), i(0x6c)]
        ),
        "hkbPoweredRagdollControlsModifier" => format!(
            "max_force={} tau={} damping={} proportional={} constant={} bones={:?} mode={} weights={:?}",
            f(0x50),
            f(0x54),
            f(0x58),
            f(0x5c),
            f(0x60),
            [i(0x78), i(0x7a), i(0x7c)],
            p.u8(o + 0x7e),
            p.ptr(o + 0x80)
        ),
        "hkbRigidBodyRagdollControlsModifier" => format!(
            "gains={:?} blend={}",
            (0..12).map(|n| f(0x50 + n * 4)).collect::<Vec<_>>(),
            f(0x80)
        ),
        "hkbKeyframeBonesModifier" => format!(
            "info_count={} bones={:?}",
            p.array(o + 0x50).1,
            p.ptr(o + 0x60)
        ),
        "BSRagdollContactListenerModifier" => {
            format!("event={} bones={:?}", p.i32(o + 0x58), p.ptr(o + 0x68))
        }
        _ => return None,
    })
}
