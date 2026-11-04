//! `hkb-masks <data dir>`: every authored bone-weight array (blender children,
//! bone-switch children, character properties) in every behavior project, its
//! length against the project's Havok skeleton, and the shape of short ones.
//! With a name filter, lists the per-bone weights of matching short arrays.
//! Read-only survey of game assets; nothing is copied.
use anyhow::Result;
use havok::behavior::{Generator, Project};

#[derive(Default)]
struct Tally {
    full: usize,
    short: usize,
    longer: usize,
    empty: usize,
}

pub fn run(data: &str, detail: Option<&str>) -> Result<()> {
    let detail = detail.map(str::to_ascii_lowercase);
    let data = std::path::Path::new(data);
    let plugins = esp::LoadOrder::default_plugin_list(data, None);
    let v = vfs::Vfs::new(data, &plugins);
    let mut blend = Tally::default();
    let mut switch = Tally::default();
    let mut property = Tally::default();
    // Short arrays: how their last authored entry reads.
    let mut short_last = std::collections::BTreeMap::<String, usize>::new();
    for file in v.list("meshes/").into_iter().filter(|p| {
        p.ends_with(".hkx")
            && !p.contains("/animations/")
            && !p.contains("/behaviors/")
            && !p.contains("/characters/")
    }) {
        let Some(bytes) = v.read(&file) else { continue };
        let Ok(pack) = havok::Packfile::parse(&bytes) else {
            continue;
        };
        if !pack.objects.iter().any(|o| o.class == "hkbProjectData") {
            continue;
        }
        let (dir, name) = file.rsplit_once('/').unwrap();
        let Ok(project) = Project::load_project(name, |rel| v.read(&format!("{dir}/{rel}"))) else {
            continue;
        };
        let Some(character) = &project.character else {
            continue;
        };
        let rig = format!("{dir}/{}", character.rig.replace('\\', "/")).to_ascii_lowercase();
        let Some(skeleton) = v
            .read(&rig)
            .and_then(|b| havok::AnimationContainer::parse(&b).ok())
            .and_then(|c| c.skeletons.into_iter().next())
        else {
            println!("project {file}: no skeleton at {rig}");
            continue;
        };
        let bones = skeleton.bones.len();
        println!("project {file}: {bones} bones ({rig})");
        let mut report = |kind: &str, at: &str, w: &[f32], tally: &mut Tally| {
            if w.is_empty() {
                tally.empty += 1;
                return;
            }
            if w.len() == bones {
                tally.full += 1;
                return;
            }
            if w.len() > bones {
                tally.longer += 1;
                println!("  {kind} {at}: {} entries > {bones}", w.len());
                return;
            }
            tally.short += 1;
            let nonzero = w.iter().filter(|x| **x != 0.0).count();
            let last = *w.last().unwrap();
            let next = skeleton.bones.get(w.len()).map_or("-", |b| b.name.as_str());
            *short_last
                .entry(format!("{kind} last={last} all_zero={}", nonzero == 0))
                .or_default() += 1;
            println!(
                "  {kind} {at}: {}/{bones} entries, {nonzero} nonzero, last {last}, first omitted bone {next}; {:?}",
                w.len(),
                w.iter().take(12).collect::<Vec<_>>()
            );
            // `hkb-masks <data> <name filter>`: each authored bone's weight.
            if detail
                .as_ref()
                .is_some_and(|d| at.to_ascii_lowercase().contains(d))
            {
                for (b, x) in skeleton.bones.iter().zip(w) {
                    println!("    {x} {}", b.name);
                }
            }
        };
        let mut names: Vec<_> = character.bone_weights.keys().collect();
        names.sort();
        for name in names {
            report(
                "property",
                name,
                &character.bone_weights[name],
                &mut property,
            );
        }
        for (rel, graph) in &project.graphs {
            for g in &graph.generators {
                match g {
                    Generator::Blender { name, children, .. } => {
                        for (i, c) in children.iter().enumerate() {
                            if let Some(p) = &c.bone_weights_property {
                                println!("  blend {rel} {name}[{i}] uses property {p}");
                                continue; // its array is reported as a property
                            }
                            if let Some(w) = &c.bone_weights {
                                report("blend", &format!("{rel} {name}[{i}]"), w, &mut blend);
                            }
                        }
                    }
                    Generator::BoneSwitch {
                        name,
                        children,
                        properties,
                        ..
                    } => {
                        for (i, (_, w)) in children.iter().enumerate() {
                            if let Some(Some(p)) = properties.get(i) {
                                println!("  switch {rel} {name}[{i}] uses property {p}");
                                continue;
                            }
                            report("switch", &format!("{rel} {name}[{i}]"), w, &mut switch);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    for (kind, t) in [
        ("blend child", &blend),
        ("bone switch child", &switch),
        ("character property", &property),
    ] {
        println!(
            "{kind}: {} full, {} short, {} longer, {} empty",
            t.full, t.short, t.longer, t.empty
        );
    }
    for (k, n) in &short_last {
        println!("short {k}: {n}");
    }
    Ok(())
}
