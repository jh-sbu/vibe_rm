//! Read-only surveys of authored animation assets; no engine binaries.
use anyhow::Result;
use havok::behavior::{Modifier, Project};

pub fn run(data: &str) -> Result<()> {
    let data = std::path::Path::new(data);
    let plugins = esp::LoadOrder::default_plugin_list(data, None);
    let v = vfs::Vfs::new(data, &plugins);
    let mut projects = 0;
    let mut controls = 0;
    let mut legs = 0;
    let mut ungrounded = 0;
    for file in v.list("meshes/actors/").into_iter().filter(|p| {
        p.ends_with(".hkx")
            && !p.contains("/animations/")
            && !p.contains("/behaviors/")
            && !p.contains("/character assets/")
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
        let project = Project::load_project(name, |rel| v.read(&format!("{dir}/{rel}")))?;
        projects += 1;
        println!("project {file}");
        for (rel, graph) in &project.graphs {
            for (index, modifier) in graph.modifiers.iter().enumerate() {
                let label = match modifier {
                    Modifier::FootIkControls(g) => {
                        controls += 1;
                        println!(
                            "  {rel} foot IK unlock={} align={} hip={} knee={}",
                            g.foot_unlock,
                            g.align_world_from_model,
                            g.hip_orientation,
                            g.max_knee_angle_difference
                        );
                        "foot IK"
                    }
                    Modifier::LookAt(g) => {
                        println!(
                            "  {rel} look-at tracking={} bones={}",
                            g.look_at_target,
                            g.bones.len()
                        );
                        "look-at"
                    }
                    _ => continue,
                };
                for binding in &graph.modifier_bindings[index] {
                    println!(
                        "    {label} .{} <- {} (default {})",
                        binding.member,
                        graph.variables[binding.variable],
                        graph.variable_default(binding.variable)
                    );
                }
            }
            let path = format!("{dir}/{}", rel.replace('\\', "/"));
            let Some(bytes) = v.read(&path) else { continue };
            let pack = havok::Packfile::parse(&bytes)?;
            for object in &pack.objects {
                if object.class == "hkbFootIkControlsModifier" {
                    let (base, count) = pack.array(object.offset + 0x80);
                    legs += count;
                    println!("    controls legs={count}");
                    if let Some(base) = base {
                        for i in 0..count as u32 {
                            // Vector4, aligned event (id, payload), error and bools.
                            let event = pack.i32(base + i * 0x30 + 0x10);
                            if event >= 0 {
                                ungrounded += 1;
                                println!(
                                    "    leg {i} ungroundedEvent={event} {:?}",
                                    graph.events.get(event as usize)
                                );
                            }
                        }
                    }
                } else if object.class == "hkbLookAtModifier" {
                    println!("    hkbLookAtModifier (not BSLookAtModifier)");
                }
            }
        }
    }
    println!(
        "total: {projects} projects, {controls} controls modifiers, {legs} control legs, {ungrounded} ungrounded events"
    );
    Ok(())
}
