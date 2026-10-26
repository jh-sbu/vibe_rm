//! Read-only survey of spell visuals. Loads assets in memory, exports none.
use anyhow::{Context, Result, bail};
use esp::{FormId, LoadOrder};
#[path = "../../../src/spell_fx/records.rs"]
mod records;

pub fn run(args: &[String]) -> Result<()> {
    let data = std::path::Path::new(
        args.first()
            .context("usage: spell-fx <data> [mgef|spell]")?,
    );
    let plugins = LoadOrder::default_plugin_list(data, None);
    let lo = LoadOrder::load(data, &plugins)?;
    let vfs = vfs::Vfs::new(data, &plugins);
    if let Some(key) = args.get(1) {
        let id = lo
            .find_editor_id(key)
            .or_else(|| {
                u32::from_str_radix(key.trim_start_matches("0x"), 16)
                    .ok()
                    .map(FormId)
            })
            .context("unknown form")?;
        let r = lo.get(id).context("form not found")?;
        let effects = match &r.tag().0 {
            b"MGEF" => vec![id],
            b"SPEL" | b"ENCH" | b"SCRL" => r
                .subrecords()
                .filter(|s| s.tag.0 == *b"EFID" && s.data.len() >= 4)
                .map(|s| r.fid(FormId(s.u32(0))))
                .collect(),
            _ => bail!("expected MGEF, SPEL, SCRL or ENCH"),
        };
        for effect in effects {
            describe(&lo, &vfs, effect)?;
        }
    } else {
        let fields = [
            (0x18, "casting light"),
            (0x20, "hit shader"),
            (0x24, "enchant shader"),
            (0x5c, "casting art"),
            (0x60, "hit art"),
            (0x6c, "dual cast"),
            (0x74, "enchant art"),
            (0x78, "hit visuals"),
            (0x7c, "enchant visuals"),
        ];
        println!("{} MGEFs", lo.ids_of_type(b"MGEF").len());
        for (offset, label) in fields {
            let count = lo
                .ids_of_type(b"MGEF")
                .iter()
                .filter(|&&id| {
                    lo.get(id)
                        .and_then(|r| {
                            r.get(b"DATA")
                                .map(|d| records::optional_u32(d, offset) != 0)
                        })
                        .unwrap_or(false)
                })
                .count();
            println!("{label}: {count}");
        }
        for tag in [b"ARTO", b"EFSH", b"RFCT", b"DUAL"] {
            println!(
                "{}: {}",
                String::from_utf8_lossy(tag),
                lo.ids_of_type(tag).len()
            );
        }
        let mut graphs = std::collections::BTreeMap::<String, usize>::new();
        let (mut models, mut tagged, mut multi, mut havok) = (0, 0, 0, 0);
        for &id in lo.ids_of_type(b"ARTO") {
            let Some(model) = records::ArtObject::load(&lo, id).and_then(|a| a.model) else {
                continue;
            };
            models += 1;
            let Some(n) = vfs.read(&model).and_then(|d| nif::Nif::parse(&d).ok()) else {
                continue;
            };
            let tags = nif::attach_tags(&n);
            tagged += usize::from(tags.values().any(|t| !t.named_nodes.is_empty()));
            multi += usize::from(tags.values().any(|t| t.multi_technique));
            havok += usize::from(tags.values().any(|t| t.havok_geometry));
            if let Some(g) = n.behavior_graph() {
                *graphs.entry(g.to_owned()).or_default() += 1;
            }
        }
        println!(
            "art models: {models}; NamedNode: {tagged}; MultiTechnique: {multi}; HavokGeometry: {havok}"
        );
        for (graph, count) in graphs {
            println!("graph {count}: {graph}");
        }
    }
    Ok(())
}
fn describe(lo: &LoadOrder, vfs: &vfs::Vfs, id: FormId) -> Result<()> {
    let r = lo.get(id).context("effect missing")?;
    let d = r.get(b"DATA").context("MGEF DATA missing")?;
    println!(
        "{id} {} flags {:08X}",
        r.editor_id().unwrap_or_default(),
        records::optional_u32(d, 0)
    );
    for (offset, label) in [
        (0x18, "casting light"),
        (0x20, "hit shader"),
        (0x24, "enchant shader"),
        (0x48, "projectile"),
        (0x5c, "casting art"),
        (0x60, "hit art"),
        (0x6c, "dual cast"),
        (0x74, "enchant art"),
        (0x78, "hit visuals"),
        (0x7c, "enchant visuals"),
    ] {
        let form = records::optional_form(&r, d, offset);
        if form.is_null() {
            continue;
        }
        println!(
            "  {label}: {form} {}",
            lo.get(form).and_then(|r| r.editor_id()).unwrap_or_default()
        );
        if let Some(art) = records::ArtObject::load(lo, form) {
            println!("    kind {}", art.kind);
            if let Some(model) = art.model {
                model_info(vfs, &model);
            }
        }
        if let Some(shader) = records::EffectShader::load(lo, form) {
            println!(
                "    textures {:?}; DATA {} bytes flags {:08X}",
                shader.textures,
                shader.data.len(),
                shader.flags()
            );
        }
        if let Some(dual) = records::DualCastData::load(lo, form) {
            println!("    {dual:?}; scale {}", records::optional_float(d, 0x70));
        }
        if let Some(visual) = records::VisualEffect::load(lo, form) {
            println!("    {visual:?}");
        }
        if let Some(proj) = lo.get(form).filter(|r| r.tag().0 == *b"PROJ") {
            if let Some(model) = proj.get(b"MODL") {
                model_info(
                    vfs,
                    &format!(
                        "meshes/{}",
                        String::from_utf8_lossy(model)
                            .trim_end_matches('\0')
                            .replace('\\', "/")
                    ),
                );
            }
            if let Some(data) = proj.get(b"DATA") {
                println!("    light {}", records::optional_form(&proj, data, 16));
            }
        }
    }
    Ok(())
}
fn model_info(vfs: &vfs::Vfs, model: &str) {
    println!("    model {model}");
    let Some(n) = vfs.read(model).and_then(|d| nif::Nif::parse(&d).ok()) else {
        println!("    model unavailable");
        return;
    };
    println!(
        "    graph {:?}; attach {:?}",
        n.behavior_graph(),
        nif::attach_tags(&n)
    );
    for b in &n.blocks {
        if let nif::Block::ControllerSequence(s) = b {
            println!(
                "    sequence {} [{}, {}] {} blocks",
                s.name,
                s.start,
                s.stop,
                s.blocks.len()
            );
        }
    }
}

#[cfg(test)]
mod graph_tests {
    use super::*;
    use havok::behavior::{
        Project,
        runtime::{Instance, Shared},
    };
    use std::sync::Arc;
    fn graph(name: &str) -> Instance {
        let data = vfs::locate_data_dir().expect("installed Skyrim Data");
        let vfs = vfs::Vfs::new(&data, &LoadOrder::default_plugin_list(&data, None));
        let project = Project::load_project(&format!("{name}.hkx"), |rel| {
            vfs.read(&format!("meshes/magic/{rel}"))
        })
        .unwrap();
        assert!(!project.graphs.is_empty());

        Instance::new(Shared::new(Arc::new(project)), 1)
    }
    fn names(i: &Instance) -> Vec<String> {
        i.sequence_samples()
            .iter()
            .map(|s| s.animation.to_string())
            .collect()
    }
    #[test]
    #[ignore = "requires installed Skyrim HKX assets"]
    fn casting_basic_authored_events_select_named_nif_sequences() {
        let mut i = graph("castingbasic");
        i.set_variable("bRightHand", 1.0);
        i.set_variable("iHandType", 1.0);
        i.update(0.0, &mut |_: &str| Some(10.0));
        for (event, sequence) in [
            ("MRh_SpellAimedStart", "mCharge"),
            ("MRh_SpellReady_Event", "mReady"),
            ("MRh_SpellRelease_Event", "mCast"),
            ("MRh_Equipped_Event", "mIdle"),
        ] {
            i.send_event(event);
            i.update(0.5, &mut |_: &str| Some(10.0));
            if event == "MRh_Equipped_Event" {
                i.send_event("end");
                i.update(0.5, &mut |_: &str| Some(10.0));
            }
            assert!(
                names(&i).iter().any(|n| n == sequence),
                "{event}: {:?}",
                names(&i)
            );
        }
    }
    #[test]
    #[ignore = "requires installed Skyrim HKX assets"]
    fn begin_loop_end_and_idle_on_load_accept_stop_effect() {
        for file in ["beginloopend", "idleonload"] {
            let mut i = graph(file);
            i.update(0.0, &mut |_: &str| Some(1.0));
            assert!(!i.sequence_samples().is_empty(), "{file}");
            i.send_event("end");
            i.update(0.5, &mut |_: &str| Some(1.0));
            i.send_event("StopEffect");
            i.update(0.5, &mut |_: &str| Some(1.0));
            assert!(
                names(&i).iter().any(|n| n
                    == if file == "beginloopend" {
                        "mEnd"
                    } else {
                        "mIdle"
                    }),
                "{file}: {:?}",
                names(&i)
            );
        }
    }
}
