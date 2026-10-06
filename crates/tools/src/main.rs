use anyhow::{Context, Result, bail};

/// A behaviour project named by its directory (humanoids: `behaviors/0_master.hkx`)
/// or by its project file (`meshes/actors/canine/dogproject.hkx`), with its directory.
fn load_project(v: &vfs::Vfs, arg: &str) -> Result<(String, havok::behavior::Project)> {
    let arg = arg.trim_end_matches('/').to_ascii_lowercase();
    if let Some((dir, file)) = arg.rsplit_once('/').filter(|_| arg.ends_with(".hkx")) {
        let p = havok::behavior::Project::load_project(file, |rel| v.read(&format!("{dir}/{rel}")))?;
        if let Some(c) = &p.character {
            eprintln!("character {} rig {} behaviour {}, {} graphs", c.name, c.rig, c.behavior, p.graphs.len());
            if let Some(f) = &c.foot_ik {
                eprintln!("foot IK: {f:?}");
            }
        }
        return Ok((dir.to_owned(), p));
    }
    let p = havok::behavior::Project::load("behaviors/0_master.hkx", |rel| v.read(&format!("{arg}/{rel}")));
    Ok((arg, p))
}

fn main() -> Result<()> {
    env_logger::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("bsa-list") => {
            let a = bsa::Archive::open(&args[1])?;
            let mut paths: Vec<_> = a.paths().collect();
            paths.sort();
            for p in paths {
                println!("{p}");
            }
        }
        Some("bsa-extract") => {
            let a = bsa::Archive::open(&args[1])?;
            let data = a.read(&args[2])?.context("file not found in archive")?;
            std::fs::write(&args[3], data)?;
        }
        Some("bsa-verify") => {
            for path in &args[1..] {
                let a = bsa::Archive::open(path)?;
                let mut n = 0usize;
                let mut bytes = 0usize;
                let paths: Vec<String> = a.paths().map(str::to_owned).collect();
                for p in &paths {
                    bytes += a.read(p)?.unwrap().len();
                    n += 1;
                }
                println!("{path}: v{} {n} files, {bytes} bytes OK", a.version());
            }
        }
        Some("esp-info") => {
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let t = std::time::Instant::now();
            let lo = esp::LoadOrder::load(data, &names)?;
            println!("indexed {} records in {:?}", lo.record_count(), t.elapsed());
            for p in lo.plugins() {
                println!("{:?} {} masters={:?}", p.slot, p.plugin.name(), p.plugin.header().masters);
            }
            for tag in [b"CELL", b"WRLD", b"STAT", b"REFR", b"ACHR", b"NPC_", b"LAND"] {
                println!("{}: {}", String::from_utf8_lossy(tag), lo.ids_of_type(tag).len());
            }
            let t = std::time::Instant::now();
            let id = lo.find_editor_id("WhiterunBanneredMare");
            println!("edid lookup {:?} took {:?}", id, t.elapsed());
            if let Some(id) = id {
                let c = lo.cell(id).unwrap();
                println!("persistent {} temporary {}", c.persistent.len(), c.temporary.len());
            }
        }
        Some("esp-dump") => {
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let id = match u32::from_str_radix(&args[2], 16) {
                Ok(v) if args[2].len() == 8 => esp::FormId(v),
                _ => lo.find_editor_id(&args[2]).context("editor id not found")?,
            };
            let rec = lo.get(id).context("record not found")?;
            println!("{} {} flags={:08X} from {}", rec.tag(), id, rec.flags(), rec.plugin.plugin.name());
            for sr in rec.subrecords() {
                let hex: String = sr.data.iter().take(48).map(|b| format!("{b:02x}")).collect();
                let txt: String = sr.data.iter().take(48).map(|&b| if (32..127).contains(&b) { b as char } else { '.' }).collect();
                println!("  {} [{}] {hex} {txt}", sr.tag, sr.data.len());
            }
        }
        Some("gmst") => {
            // Game settings whose editor id contains the pattern (case-insensitive).
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let pat = args.get(2).map_or(String::new(), |p| p.to_ascii_lowercase());
            let mut rows: Vec<(String, String)> = Vec::new();
            for &id in lo.ids_of_type(b"GMST") {
                let Some(rec) = lo.get(id) else { continue };
                let Some(name) = rec.editor_id().map(|e| e.to_string()) else { continue };
                if !name.to_ascii_lowercase().contains(&pat) {
                    continue;
                }
                let d = rec.get(b"DATA").unwrap_or(&[]);
                let value = match (name.as_bytes().first(), d.get(0..4)) {
                    (Some(b'f'), Some(b)) => format!("{}", f32::from_le_bytes(b.try_into().unwrap())),
                    (Some(b'i' | b'u'), Some(b)) => format!("{}", i32::from_le_bytes(b.try_into().unwrap())),
                    (Some(b'b'), Some(b)) => format!("{}", u32::from_le_bytes(b.try_into().unwrap()) != 0),
                    _ => format!("{} bytes", d.len()),
                };
                rows.push((name, value));
            }
            rows.sort();
            for (n, v) in rows {
                println!("{n} = {v}");
            }
        }
        Some("nif-verify") => {
            // Parse every NIF in the given archives; report blocks whose parse
            // didn't consume exactly the declared size.
            use std::collections::BTreeMap;
            let mut stats: BTreeMap<String, [usize; 4]> = BTreeMap::new();
            let mut files = 0usize;
            let mut failed_files = 0usize;
            let mut examples: BTreeMap<String, String> = BTreeMap::new();
            for path in &args[1..] {
                let a = bsa::Archive::open(path)?;
                let mut paths: Vec<String> = a.paths().filter(|p| p.ends_with(".nif")).map(str::to_owned).collect();
                paths.sort();
                for p in paths {
                    let data = a.read(&p)?.unwrap();
                    files += 1;
                    match nif::Nif::parse_with_report(&data) {
                        Ok((n, rep)) => {
                            for (i, st) in rep.iter().enumerate() {
                                let name = n.block_type_name(i).to_owned();
                                let e = stats.entry(name.clone()).or_default();
                                let k = match st {
                                    nif::BlockParse::Exact => 0,
                                    nif::BlockParse::Skipped => 1,
                                    nif::BlockParse::SizeMismatch { .. } => 2,
                                    nif::BlockParse::Failed => 3,
                                };
                                e[k] += 1;
                                if k >= 2 {
                                    examples.entry(name).or_insert_with(|| format!("{p} block {i} {st:?}"));
                                }
                            }
                        }
                        Err(e) => {
                            failed_files += 1;
                            if failed_files < 10 {
                                println!("FAILED {p}: {e}");
                            }
                        }
                    }
                }
            }
            println!("{files} files, {failed_files} failed");
            println!("{:<40} {:>8} {:>8} {:>8} {:>8}", "type", "exact", "skipped", "mismatch", "failed");
            for (k, v) in &stats {
                println!("{k:<40} {:>8} {:>8} {:>8} {:>8}", v[0], v[1], v[2], v[3]);
            }
            for (k, v) in &examples {
                println!("example {k}: {v}");
            }
        }
        Some("fsts") => {
            // fsts <data dir> <footstep set>: its groups (walk, run, sprint, sneak,
            // swim): each footstep's tag and impact data set.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let id = match u32::from_str_radix(&args[2], 16) {
                Ok(v) if args[2].len() == 8 => esp::FormId(v),
                _ => lo.find_editor_id(&args[2]).context("editor id not found")?,
            };
            let rec = lo.get(id).context("record not found")?;
            let fid = |r: &esp::LoadedRecord<'_>, d: &[u8]| r.fid(esp::FormId(u32::from_le_bytes(d[..4].try_into().unwrap())));
            let counts: Vec<usize> = rec.get(b"XCNT").context("no XCNT")?.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap()) as usize).collect();
            let steps: Vec<esp::FormId> = rec.get(b"DATA").context("no DATA")?.chunks_exact(4).map(|c| fid(&rec, c)).collect();
            // The counts run walk to swim; the footsteps are stored swim first.
            let mut next = steps.into_iter();
            for (g, n) in ["walk", "run", "sprint", "sneak", "swim"].iter().zip(counts).rev() {
                println!("{g}:");
                for s in next.by_ref().take(n) {
                    let Some(st) = lo.get(s) else { continue };
                    let tag = st.get(b"ANAM").map(esp::decode_zstring).unwrap_or_default();
                    let ipds = st.get(b"DATA").map(|d| fid(&st, d)).unwrap_or_default();
                    let ipds_name = lo.get(ipds).and_then(|r| r.editor_id()).unwrap_or_default();
                    println!("  {s} {:<40} {tag:<16} {ipds} {ipds_name}", st.editor_id().unwrap_or_default());
                }
            }
        }
        Some("nif-materials") => {
            // nif-materials <bsa>...: Havok materials across the archives' collision
            // shapes (triangles for meshes, else shapes), with a model using each.
            use std::collections::BTreeMap;
            let mut counts: BTreeMap<u32, (usize, String)> = BTreeMap::new();
            for path in &args[1..] {
                let a = bsa::Archive::open(path)?;
                for p in a.paths().filter(|p| p.ends_with(".nif")).map(str::to_owned).collect::<Vec<_>>() {
                    let Ok(n) = nif::Nif::parse(&a.read(&p)?.unwrap()) else { continue };
                    let mut add = |m: u32, k: usize| {
                        let e = counts.entry(m).or_insert_with(|| (0, p.clone()));
                        e.0 += k;
                    };
                    for b in &n.blocks {
                        match b {
                            nif::Block::Shape(nif::Shape::CompressedMeshData { materials, .. } | nif::Shape::PackedTriStripsData { materials, .. }) => {
                                materials.iter().for_each(|&m| add(m, 1))
                            }
                            nif::Block::Shape(
                                nif::Shape::NiTriStrips { material, .. }
                                | nif::Shape::ConvexVertices { material, .. }
                                | nif::Shape::Box { material, .. }
                                | nif::Shape::Sphere { material, .. }
                                | nif::Shape::Capsule { material, .. },
                            ) => add(*material, 1),
                            _ => {}
                        }
                    }
                }
            }
            let mut v: Vec<_> = counts.into_iter().collect();
            v.sort_by_key(|e| std::cmp::Reverse(e.1.0));
            for (m, (n, p)) in v {
                println!("{m:>10} {n:>9} {p}");
            }
        }
        Some("nif-dump") => {
            // nif-dump <data dir> <vfs path>
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let v = vfs::Vfs::new(data, &names);
            let bytes = v.read(&args[2]).context("not found")?;
            let n = nif::Nif::parse(&bytes)?;
            println!("bs version {} roots {:?}", n.header.bs_version, n.roots);
            for (i, b) in n.blocks.iter().enumerate() {
                let s = match b {
                    nif::Block::TriShape(t) => {
                        let (mut lo, mut hi) = (glam::Vec3::splat(f32::MAX), glam::Vec3::splat(f32::MIN));
                        for p in &t.geometry.positions {
                            lo = lo.min(*p);
                            hi = hi.max(*p);
                        }
                        format!(
                            "TriShape name={} verts={} tris={} skin={:?} shader={:?} alpha={:?} desc={:016x} flags={:03x} xf={:?} bounds={lo:?}..{hi:?}",
                            t.av.net.name, t.geometry.positions.len(), t.geometry.triangles.len(), t.skin, t.shader, t.alpha,
                            t.vertex_desc, t.vertex_flags(), t.av.transform.translation
                        )
                    }
                    nif::Block::SkinPartition(p) => format!(
                        "SkinPartition verts={} desc={:016x} partitions={:?}",
                        p.geometry.positions.len(),
                        p.vertex_desc,
                        p.partitions.iter().map(|q| (q.num_vertices, q.bones.len(), q.triangles.len(), q.vertex_map.len(), q.weights_per_vertex)).collect::<Vec<_>>()
                    ),
                    nif::Block::SkinData(d) => format!(
                        "SkinData skin_xf={:?} bones={} first={:?}",
                        d.skin_transform.translation, d.bones.len(), d.bones.first().map(|b| b.transform.translation)
                    ),
                    nif::Block::TriShapeData(d) => {
                        let (mut lo, mut hi) = (glam::Vec3::splat(f32::MAX), glam::Vec3::splat(f32::MIN));
                        for p in &d.geometry.positions {
                            lo = lo.min(*p);
                            hi = hi.max(*p);
                        }
                        format!("TriShapeData verts={} tris={} bounds={lo:?}..{hi:?}", d.geometry.positions.len(), d.geometry.triangles.len())
                    }
                    nif::Block::Shape(nif::Shape::CompressedMeshData { vertices, triangles, .. }) => {
                        format!("CompressedMeshData verts={} tris={}", vertices.len(), triangles.len())
                    }
                    other => format!("{other:?}"),
                };
                let s: String = s.chars().take(600).collect();
                println!("[{i}] {}: {s}", n.block_type_name(i));
            }
        }
        Some("nif-raw") => {
            // nif-raw <data dir> <vfs path> <block type>: hex dump blocks of a type as floats
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let v = vfs::Vfs::new(data, &names);
            let bytes = v.read(&args[2]).context("not found")?;
            let n = nif::Nif::parse(&bytes)?;
            for i in 0..n.blocks.len() {
                if n.block_type_name(i) != args[3] {
                    continue;
                }
                let off = n.header.block_offsets[i];
                let sz = n.header.block_sizes[i] as usize;
                println!("[{i}] {} size {sz}", args[3]);
                let b = &bytes[off..off + sz];
                for (j, c) in b.chunks(4).enumerate() {
                    let u = u32::from_le_bytes([c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0), *c.get(3).unwrap_or(&0)]);
                    println!("  +{:3}: {:08x} {:>14.6} {}", j * 4, u, f32::from_bits(u), u as i32);
                }
            }
        }
        Some("hkx-dump") => {
            // hkx-dump <data dir> <vfs path> [time]
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let v = vfs::Vfs::new(data, &names);
            let bytes = v.read(&args[2]).context("not found")?;
            let p = havok::Packfile::parse(&bytes)?;
            println!("{} objects: {:?}", p.version, p.objects.iter().map(|o| o.class.as_str()).collect::<Vec<_>>());
            let c = havok::AnimationContainer::parse(&bytes)?;
            for s in &c.skeletons {
                println!("skeleton {} with {} bones", s.name, s.bones.len());
                let shown = if std::env::var_os("HKX_ALL").is_some() { usize::MAX } else { 8 };
                for (i, b) in s.bones.iter().enumerate().take(shown) {
                    println!("  [{i}] {} parent {:?} t {:?}", b.name, b.parent, b.reference.translation);
                }
            }
            let t: f32 = args.get(3).map(|s| s.parse()).transpose()?.unwrap_or(0.0);
            for a in &c.animations {
                println!("animation {:.3}s {} tracks, binding {:?}", a.duration, a.num_tracks, a.binding.as_ref().map(|b| (&b.skeleton_name, b.track_to_bone.len())));
                for an in a.annotations.iter().take(10) {
                    println!("  @{:.3} {}", an.time, an.text);
                }
                let mut out = Vec::new();
                a.sample(t, &mut out);
                let tracks: Vec<usize> = std::env::var("HKX_TRACKS")
                    .ok()
                    .map(|v| v.split(',').filter_map(|s| s.parse().ok()).collect())
                    .unwrap_or_else(|| (0..6).collect());
                for (i, q) in out.iter().enumerate().filter(|(i, _)| tracks.contains(i)) {
                    println!("  track {i}: t={:?} r={:?} |r|={:.4} s={:?}", q.translation, q.rotation, q.rotation.length(), q.scale);
                }
            }
        }
        Some("hkb-events") => {
            // hkb-events <data dir> <project dir, e.g. meshes/actors/character> <event>...
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let v = vfs::Vfs::new(data, &names);
            let (_, project) = load_project(&v, &args[2])?;
            for (rel, g) in &project.graphs {
                let clips = g.generators.iter().filter(|g| matches!(g, havok::behavior::Generator::Clip { .. })).count();
                println!("{rel}: {} generators, {clips} clips, {} events", g.generators.len(), g.events.len());
            }
            // `name*` lists the events starting with `name` (any case).
            let mut wanted: Vec<String> = Vec::new();
            for e in &args[3..] {
                match e.strip_suffix('*') {
                    Some(prefix) => {
                        let prefix = prefix.to_ascii_lowercase();
                        let mut names: Vec<String> = project.graphs.iter().flat_map(|(_, g)| g.events.iter()).filter(|n| n.to_ascii_lowercase().starts_with(&prefix)).cloned().collect();
                        names.sort();
                        names.dedup();
                        wanted.extend(names);
                    }
                    None => wanted.push(e.clone()),
                }
            }
            for e in &wanted {
                println!("{e}:");
                for (gpath, g) in &project.graphs {
                    let Some(id) = g.event_id(e) else { continue };
                    for (gi, node) in g.generators.iter().enumerate() {
                        let havok::behavior::Generator::StateMachine { name, states, wildcards, .. } = node else { continue };
                        for (from, t) in wildcards.iter().map(|t| ("*", t)).chain(states.iter().flat_map(|s| s.transitions.iter().map(move |t| (s.name.as_str(), t)))) {
                            if t.event == id {
                                let to = states.iter().find(|s| s.id == t.to_state).map(|s| s.name.as_str()).unwrap_or("?");
                                println!("  {gpath} sm#{gi} {name}: {from} -> {to} nested {:?} flags {:#x}", t.to_nested, t.flags);
                            }
                        }
                    }
                }
                for pb in project.play_event(e) {
                    println!("  {:?}", pb.clips.iter().map(|c| format!("{} ({:?})", c.animation, c.mode)).collect::<Vec<_>>());
                    for e in pb.events.iter().filter(|e| e.payload.is_some() || e.event.to_ascii_lowercase().contains("animobj")) {
                        println!("    clip {} @{:.3}{} {} {:?}", e.clip, e.time, if e.from_end { " from end" } else { "" }, e.event, e.payload);
                    }
                    if let Some(exit) = project.then_event(&pb, "IdleChairExitStart").or_else(|| project.then_event(&pb, "IdleStop")) {
                        println!("    exit {:?}", exit.clips.iter().map(|c| c.animation.as_str()).collect::<Vec<_>>());
                    }
                }
            }
        }
        Some("hkb-tree") => {
            // hkb-tree <data dir> <project dir> <graph file> <state or generator name> [depth]
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let v = vfs::Vfs::new(data, &names);
            let dir = args[2].trim_end_matches('/');
            let bytes = v.read(&format!("{dir}/behaviors/{}", args[3])).context("graph not found")?;
            let g = havok::behavior::BehaviorGraph::parse(&bytes)?;
            let max: usize = args.get(5).map(|s| s.parse()).transpose()?.unwrap_or(6);
            use havok::behavior::Generator as G;
            fn show(g: &havok::behavior::BehaviorGraph, id: usize, depth: usize, max: usize) {
                let pad = "  ".repeat(depth);
                match &g.generators[id] {
                    G::Clip { name, animation, mode, speed, triggers, .. } => {
                        let t: Vec<String> = triggers.iter().map(|t| format!("{}@{:.2}{}", g.event_name(t.event).unwrap_or("?"), t.time, if t.from_end { "e" } else { "" })).collect();
                        println!("{pad}clip {name} {animation} {mode:?} x{speed} {t:?}");
                    }
                    G::StateMachine { name, start, start_variable, start_mode, states, wildcards } => {
                        let var = start_variable.map(|v| format!(" (bound to {} = {:?})", g.variables.get(v).map_or("?", String::as_str), g.variable_defaults.get(v)));
                        let mode = match start_mode {
                            havok::behavior::StartMode::Default => String::new(),
                            havok::behavior::StartMode::Sync(v) => format!(" synced with {}", g.variables.get(*v).map_or("?", String::as_str)),
                            havok::behavior::StartMode::Random => " random".to_owned(),
                        };
                        println!("{pad}sm {name} start {start}{}{mode}", var.unwrap_or_default());
                        let tr = |t: &havok::behavior::Transition| {
                            let mut s = format!("--{}--> {} nested {:?}", g.event_name(t.event).unwrap_or("?"), t.to_state, t.to_nested);
                            if let Some(b) = t.blend {
                                s += &format!(" blend {b}");
                            }
                            if let Some(c) = &t.condition {
                                s += &format!(" if {c:?}");
                            }
                            if t.flags & !0x2000 != 0 {
                                s += &format!(" flags {:#x}", t.flags);
                            }
                            s
                        };
                        for w in wildcards {
                            println!("{pad}  * {}", tr(w));
                        }
                        for st in states {
                            println!("{pad}  state {} {}", st.id, st.name);
                            for t in &st.transitions {
                                println!("{pad}    {}", tr(t));
                            }
                            if depth < max {
                                if let Some(c) = st.generator {
                                    show(g, c, depth + 2, max);
                                }
                            }
                        }
                    }
                    other @ (G::Blender { .. } | G::Selector { .. } | G::Wrap { .. } | G::BoneSwitch { .. }) => {
                        let vars: Vec<String> = g.bindings[id].iter().map(|b| format!("{}={}", b.member, g.variables.get(b.variable).map_or("?", String::as_str))).collect();
                        let what = match other {
                            G::Blender { name, parameter, flags, children, .. } => {
                                let w: Vec<f32> = children.iter().map(|c| c.weight).collect();
                                format!("blend {name} param {parameter} flags {flags:#x} weights {w:?}")
                            }
                            G::Selector { name, index, .. } => format!("select {name} index {index}"),
                            G::Wrap { name, class, modifier, .. } => {
                                let m = modifier.map(|m| format!(" modifier {:?}", g.modifiers[m])).unwrap_or_default();
                                format!("{class} {name}{m}")
                            }
                            G::BoneSwitch { name, .. } => format!("boneswitch {name}"),
                            _ => unreachable!(),
                        };
                        println!("{pad}{what} {}", if vars.is_empty() { String::new() } else { format!("{vars:?}") });
                        if depth < max {
                            for c in other.children() {
                                show(g, c, depth + 1, max);
                            }
                        }
                    }
                    G::Reference { name, behavior } => println!("{pad}ref {name} -> {behavior}"),
                    G::Other(c) => println!("{pad}other {c}"),
                }
            }
            let want = args[4].to_ascii_lowercase();
            if want == "root"
                && let Some(r) = g.root
            {
                show(&g, r, 0, max);
            }
            for (i, node) in g.generators.iter().enumerate() {
                if node.name().to_ascii_lowercase() == want {
                    show(&g, i, 0, max);
                }
                if let G::StateMachine { states, .. } = node {
                    for st in states.iter().filter(|s| s.name.to_ascii_lowercase() == want) {
                        println!("state {} {} in sm {}", st.id, st.name, node.name());
                        if let Some(c) = st.generator {
                            show(&g, c, 1, max);
                        }
                    }
                }
            }
        }
        Some("hkb-vars") => {
            // hkb-vars <data dir> <project> [name filter]: variables, their defaults and
            // what reads or writes them (bound members, expressions, conditions).
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let v = vfs::Vfs::new(data, &names);
            let (_, project) = load_project(&v, &args[2])?;
            let filter = args.get(3).map(|f| f.to_ascii_lowercase());
            use havok::behavior::{Generator as G, Modifier as M};
            for (rel, g) in &project.graphs {
                for (vi, var) in g.variables.iter().enumerate() {
                    if filter.as_ref().is_some_and(|f| !var.to_ascii_lowercase().contains(f.as_str())) {
                        continue;
                    }
                    println!("{rel}: {var} ({:?}, default {})", g.variable_types.get(vi), g.variable_default(vi));
                    for (gi, b) in g.bindings.iter().enumerate() {
                        for b in b.iter().filter(|b| b.variable == vi) {
                            println!("    gen {} .{}", g.generators[gi].name(), b.member);
                        }
                    }
                    for (mi, b) in g.modifier_bindings.iter().enumerate() {
                        for b in b.iter().filter(|b| b.variable == vi) {
                            let kind = match &g.modifiers[mi] {
                                M::Other(c) => c.clone(),
                                m => format!("{m:?}").chars().take(60).collect(),
                            };
                            println!("    mod #{mi} {kind} .{}", b.member);
                        }
                    }
                    let lower = var.to_ascii_lowercase();
                    for m in &g.modifiers {
                        if let M::Expressions(lines) = m {
                            for l in lines.iter().filter(|l| l.to_ascii_lowercase().contains(&lower)) {
                                println!("    expr {l}");
                            }
                        }
                    }
                    for node in &g.generators {
                        if let G::StateMachine { name, states, wildcards, .. } = node {
                            for t in states.iter().flat_map(|s| s.transitions.iter()).chain(wildcards) {
                                if let Some(c) = t.condition.as_ref().filter(|c| c.to_ascii_lowercase().contains(&lower)) {
                                    let to = states.iter().find(|s| s.id == t.to_state).map(|s| s.name.as_str()).unwrap_or("?");
                                    println!("    cond in {name} -> {to}: {c}");
                                }
                            }
                        }
                    }
                }
            }
        }
        Some("hkb-flags") => {
            // hkb-flags <data dir> <project>: how often each transition flag and kind of
            // blending effect occurs.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let v = vfs::Vfs::new(data, &names);
            let (dir, project) = load_project(&v, &args[2])?;
            let mut counts = [0usize; 16];
            let mut total = 0;
            for (_, g) in &project.graphs {
                for node in &g.generators {
                    if let havok::behavior::Generator::StateMachine { states, wildcards, .. } = node {
                        let from = states.iter().flat_map(|s| s.transitions.iter().map(move |t| (s.name.as_str(), t)));
                        for (from, t) in from.chain(wildcards.iter().map(|t| ("*", t))) {
                            total += 1;
                            if t.trigger.is_some() || t.initiate.is_some() || t.uninterruptible() {
                                let ev = |e: i32| g.event_name(e).unwrap_or("-");
                                let iv = |i: &havok::behavior::Interval| format!("[{} .. {} | {}s .. {}s]", ev(i.enter_event), ev(i.exit_event), i.enter_time, i.exit_time);
                                let to = states.iter().find(|s| s.id == t.to_state).map_or("?", |s| s.name.as_str());
                                println!(
                                    "{}: {from} -> {to} on {}{}{}{}",
                                    node.name(),
                                    ev(t.event),
                                    t.trigger.as_ref().map(|i| format!(" trigger {}", iv(i))).unwrap_or_default(),
                                    t.initiate.as_ref().map(|i| format!(" initiate {}", iv(i))).unwrap_or_default(),
                                    if t.uninterruptible() { format!(" uninterruptible (blend {:?})", t.blend) } else { String::new() },
                                );
                            }
                            for (b, c) in counts.iter_mut().enumerate() {
                                if t.flags & (1 << b) != 0 {
                                    *c += 1;
                                }
                            }
                        }
                    }
                }
            }
            // Blending effects: end mode, start-time fraction, flags.
            let mut effects: std::collections::BTreeMap<String, usize> = Default::default();
            for (rel, _) in &project.graphs {
                let Some(bytes) = v.read(&format!("{dir}/{}", rel.replace('\\', "/"))) else { continue };
                let p = havok::Packfile::parse(&bytes)?;
                for o in p.objects_of("hkbBlendingTransitionEffect") {
                    let key = format!("end mode {} start fraction {} flags {:#x} self mode {} event mode {}", p.u8(o + 0x5A) as i8, p.f32(o + 0x54), p.u16(o + 0x58), p.u8(o + 0x48) as i8, p.u8(o + 0x49) as i8);
                    *effects.entry(key).or_default() += 1;
                }
            }
            for (k, n) in effects {
                println!("  effect {k}: {n}");
            }
            println!("{total} transitions");
            for (b, c) in counts.iter().enumerate().filter(|(_, c)| **c > 0) {
                println!("  {:#06x}: {c}", 1 << b);
            }
        }
        Some("hkb-classes") => {
            // hkb-classes <data dir> <project dir>: object classes across the project's graphs.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let v = vfs::Vfs::new(data, &names);
            let (dir, project) = load_project(&v, &args[2])?;
            let dir = dir.as_str();
            let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
            for (rel, _) in &project.graphs {
                let bytes = v.read(&format!("{dir}/{}", rel.replace('\\', "/"))).context("graph vanished")?;
                for o in havok::Packfile::parse(&bytes)?.objects {
                    *counts.entry(o.class).or_default() += 1;
                }
            }
            for (class, n) in counts {
                println!("{n:6} {class}");
            }
        }
        Some("hkb-obj") => {
            // hkb-obj <data dir> <project dir> <graph file> <class> [count]: raw fields of
            // objects of a class (words as hex / float, pointers with their target).
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let v = vfs::Vfs::new(data, &names);
            let dir = args[2].trim_end_matches('/');
            // A graph under behaviors/, or a project-relative path starting with `./`.
            let file = match args[3].strip_prefix("./") {
                Some(rel) => format!("{dir}/{rel}"),
                None => format!("{dir}/behaviors/{}", args[3]),
            };
            let bytes = v.read(&file).context("graph not found")?;
            let p = havok::Packfile::parse(&bytes)?;
            let count: usize = args.get(5).map(|s| s.parse()).transpose()?.unwrap_or(2);
            let mut starts: Vec<u32> = p.objects.iter().map(|o| o.offset).collect();
            starts.sort();
            for o in p.objects.iter().filter(|o| o.class == args[4]).take(count) {
                let end = starts.iter().copied().find(|&s| s > o.offset).unwrap_or(p.data.len() as u32);
                println!("{} @{:#x} ({} bytes)", o.class, o.offset, end - o.offset);
                let mut at = o.offset;
                while at < end {
                    let rel = at - o.offset;
                    if let Some(t) = p.ptr(at) {
                        let what = p.object_class(t).map(str::to_owned).unwrap_or_else(|| {
                            let s = p.string(at).unwrap_or_default();
                            if s.chars().all(|c| c.is_ascii_graphic() || c == ' ') && !s.is_empty() { format!("{s:?}") } else { format!("data @{t:#x}") }
                        });
                        println!("  {rel:#05x} ptr -> {what}");
                        at += 8;
                        continue;
                    }
                    let w = p.u32(at);
                    println!("  {rel:#05x} {w:08x} {:>12} {:>14}", p.i32(at), format!("{:.4}", f32::from_bits(w)));
                    at += 4;
                }
            }
        }
        Some("hkb-run") => {
            // hkb-run <data dir> <project dir> <step>...: run a character's behaviour graphs.
            // Steps: `<seconds>` advances time, `!Event` handles an event (reporting whether
            // the graph took it), `Var=value` sets a variable.
            use havok::behavior::runtime::{Instance, Shared};
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let v = vfs::Vfs::new(data, &names);
            let (dir, project) = load_project(&v, &args[2])?;
            let project = std::sync::Arc::new(project);
            let shared = Shared::new(project);
            /// Clip lengths and blend hints, read from the data.
            struct ToolClips<'a> {
                v: &'a vfs::Vfs,
                dir: String,
                cache: std::collections::HashMap<String, Option<(f32, bool)>>,
            }
            impl ToolClips<'_> {
                fn info(&mut self, anim: &str) -> Option<(f32, bool)> {
                    let path = format!("{}/{}", self.dir, anim.to_ascii_lowercase().replace('\\', "/"));
                    let v = self.v;
                    *self.cache.entry(path.clone()).or_insert_with(|| {
                        let c = havok::AnimationContainer::parse(&v.read(&path)?).ok()?;
                        let a = c.animations.first()?;
                        Some((a.duration, a.binding.as_ref().is_some_and(|b| b.additive)))
                    })
                }
            }
            impl havok::behavior::runtime::ClipSource for ToolClips<'_> {
                fn duration(&mut self, anim: &str) -> Option<f32> {
                    self.info(anim).map(|i| i.0)
                }
                fn additive(&mut self, anim: &str) -> bool {
                    self.info(anim).is_some_and(|i| i.1)
                }
            }
            let mut clips = ToolClips { v: &v, dir: dir.clone(), cache: Default::default() };
            let mut inst = Instance::new(shared, 1);
            inst.set_tracing(true);
            let show = |inst: &mut Instance, label: &str| {
                println!("== {label}");
                for t in inst.take_trace() {
                    println!("  > {t}");
                }
                println!("  states: {}", inst.active_states().join(" > "));
                let samples = inst.samples();
                let mut cover = vec![0.0f32; 99];
                for s in &samples {
                    for (b, c) in cover.iter_mut().enumerate() {
                        *c += s.weight * s.mask.as_ref().map_or(1.0, |m| m.get(b).copied().unwrap_or(0.0));
                    }
                }
                let bare: Vec<usize> = cover.iter().enumerate().filter(|(_, c)| **c < 1e-3).map(|(b, _)| b).collect();
                if !bare.is_empty() {
                    println!("  uncovered bones {bare:?}");
                }
                for s in samples {
                    let mask = s.mask.as_ref().map(|m| {
                        let on: Vec<usize> = m.iter().enumerate().filter(|(_, w)| **w > 0.0).map(|(i, _)| i).collect();
                        format!(" mask {} of {} bones {:?}", on.len(), m.len(), &on[..on.len().min(8)])
                    });
                    println!("  {:5.2}{} {} t={:.2}{}", s.weight, if s.additive { " +" } else { "" }, s.animation, s.time, mask.unwrap_or_default());
                }
                for r in inst.take_raised() {
                    println!("  raised {}{}", r.event, r.payload.map(|p| format!(" ({p})")).unwrap_or_default());
                }
                for l in inst.look_ats() {
                    let bones: Vec<String> = l.bones.iter().map(|b| format!("{}{} {:.0}deg", b.index, if b.enabled { "" } else { " off" }, b.limit_degrees)).collect();
                    println!("  look-at{} limit {:.0}deg bones {bones:?} eyes {}", if l.look_at_target { " (tracking)" } else { "" }, l.limit_degrees, l.eye_bones.len());
                }
            };
            for step in &args[3..] {
                if let Some(ev) = step.strip_prefix('!') {
                    let took = inst.handle_event(ev, &mut clips);
                    show(&mut inst, &format!("{ev} ({})", if took { "taken" } else { "ignored" }));
                } else if let Some((name, value)) = step.split_once('=') {
                    let ok = inst.set_variable(name, value.parse()?);
                    println!("== {name} = {value}{}", if ok { "" } else { " (unknown variable)" });
                } else {
                    let secs: f32 = step.parse()?;
                    let mut t = 0.0;
                    while t < secs - 1e-4 {
                        let dt = (secs - t).min(1.0 / 30.0);
                        inst.update(dt, &mut clips);
                        t += dt;
                    }
                    show(&mut inst, &format!("+{secs} s"));
                }
            }
        }
        Some("hkb-annotations") => {
            // hkb-annotations <data dir> <project file>: annotation texts across the
            // project's clips (prefix before any '.'), with whether a graph has an event
            // of that name.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let v = vfs::Vfs::new(data, &names);
            let (dir, project) = load_project(&v, &args[2])?;
            let events: std::collections::HashSet<String> = project.graphs.iter().flat_map(|(_, g)| g.events.iter().map(|e| e.to_ascii_lowercase())).collect();
            let mut anims = std::collections::BTreeSet::new();
            for (_, g) in &project.graphs {
                for gn in &g.generators {
                    if let havok::behavior::Generator::Clip { animation, .. } = gn {
                        anims.insert(animation.to_ascii_lowercase().replace('\\', "/"));
                    }
                }
            }
            let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
            for a in &anims {
                let Some(bytes) = v.read(&format!("{dir}/{a}")) else { continue };
                let Ok(c) = havok::AnimationContainer::parse(&bytes) else { continue };
                for an in c.animations.iter().flat_map(|x| &x.annotations) {
                    let key = an.text.split('.').next().unwrap_or("").to_owned();
                    *counts.entry(key).or_default() += 1;
                }
            }
            for (k, n) in counts {
                println!("{n:6} {k}{}", if events.contains(&k.to_ascii_lowercase()) { "  [event]" } else { "" });
            }
        }
        Some("hkx-binding") => {
            // hkx-binding <data dir> <animation path>: how a clip binds to its skeleton.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let v = vfs::Vfs::new(data, &names);
            let c = havok::AnimationContainer::parse(&v.read(&args[2]).context("not found")?)?;
            for a in &c.animations {
                let b = a.binding.as_ref();
                println!("duration {} tracks {} additive {:?} bound tracks {:?}", a.duration, a.num_tracks, b.map(|b| b.additive), b.map(|b| b.track_to_bone.len()));
            }
        }
        Some("hkb-clips") => {
            // hkb-clips <data dir> <project dir> <graph file>: every clip generator with mode and speed.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let v = vfs::Vfs::new(data, &names);
            let dir = args[2].trim_end_matches('/');
            let bytes = v.read(&format!("{dir}/behaviors/{}", args[3])).context("graph not found")?;
            let g = havok::behavior::BehaviorGraph::parse(&bytes)?;
            for node in &g.generators {
                if let havok::behavior::Generator::Clip { name, animation, mode, speed, .. } = node {
                    println!("{name}\t{animation}\t{mode:?}\t{speed}");
                }
            }
        }
        Some("idle-roots") => {
            // idle-roots <data dir>: IDLE records without a parent, with their graph and size.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let mut parent: std::collections::HashMap<esp::FormId, esp::FormId> = Default::default();
            for &id in lo.ids_of_type(b"IDLE") {
                let Some(rec) = lo.get(id) else { continue };
                if let Some(d) = rec.get(b"ANAM").filter(|d| d.len() >= 4) {
                    let v = u32::from_le_bytes(d[0..4].try_into().unwrap());
                    if v != 0 {
                        parent.insert(id, rec.fid(esp::FormId(v)));
                    }
                }
            }
            let mut size: std::collections::HashMap<esp::FormId, usize> = Default::default();
            for &id in lo.ids_of_type(b"IDLE") {
                let mut cur = id;
                while let Some(&p) = parent.get(&cur) {
                    cur = p;
                }
                *size.entry(cur).or_default() += 1;
            }
            let mut roots: Vec<_> = size.into_iter().collect();
            roots.sort_by_key(|r| std::cmp::Reverse(r.1));
            for (id, n) in roots {
                let Some(rec) = lo.get(id) else { continue };
                let graph = rec.get(b"DNAM").map(esp::decode_zstring).unwrap_or_default();
                println!("{id} {:5} {} {graph}", n, rec.editor_id().unwrap_or_default());
            }
        }
        Some("idle-tree") => {
            // idle-tree <data dir> <IDLE editor id>: the idle and its descendants with raw conditions.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let mut children: std::collections::HashMap<esp::FormId, Vec<esp::FormId>> = Default::default();
            let mut previous: std::collections::HashMap<esp::FormId, esp::FormId> = Default::default();
            for &id in lo.ids_of_type(b"IDLE") {
                let Some(rec) = lo.get(id) else { continue };
                if let Some(d) = rec.get(b"ANAM").filter(|d| d.len() >= 8) {
                    let v = u32::from_le_bytes(d[0..4].try_into().unwrap());
                    let prev = u32::from_le_bytes(d[4..8].try_into().unwrap());
                    previous.insert(id, if prev == 0 { esp::FormId::NULL } else { rec.fid(esp::FormId(prev)) });
                    if v != 0 {
                        children.entry(rec.fid(esp::FormId(v))).or_default().push(id);
                    }
                }
            }
            // Authored order: follow previous-sibling links.
            for kids in children.values_mut() {
                let mut ordered = Vec::new();
                let mut cur = esp::FormId::NULL;
                while let Some(&n) = kids.iter().find(|k| previous.get(k).copied().unwrap_or_default() == cur && !ordered.contains(*k)) {
                    ordered.push(n);
                    cur = n;
                }
                let rest: Vec<_> = kids.iter().filter(|k| !ordered.contains(*k)).copied().collect();
                ordered.extend(rest);
                *kids = ordered;
            }
            let root = lo.find_editor_id(&args[2]).context("editor id not found")?;
            let mut stack = vec![(root, 0usize)];
            while let Some((id, depth)) = stack.pop() {
                let Some(rec) = lo.get(id) else { continue };
                let text = |t: &[u8; 4]| rec.get(t).map(esp::decode_zstring).unwrap_or_default();
                let pad = "  ".repeat(depth);
                println!("{pad}{id} {} event={:?}", rec.editor_id().unwrap_or_default(), text(b"ENAM"));
                for c in rec.subrecords().filter(|s| s.tag.0 == *b"CTDA" && s.data.len() >= 24) {
                    let d = c.data;
                    let f = u16::from_le_bytes([d[8], d[9]]);
                    let p1 = u32::from_le_bytes(d[12..16].try_into().unwrap());
                    let p1s = lo.get(rec.fid(esp::FormId(p1))).and_then(|r| r.editor_id()).unwrap_or_default();
                    let p2 = u32::from_le_bytes(d[16..20].try_into().unwrap());
                    let v = f32::from_le_bytes(d[4..8].try_into().unwrap());
                    println!("{pad}    ctda op {:#04x} func {f} p1 {p1:#x} {p1s} p2 {p2:#x} value {v} run {}", d[0], u32::from_le_bytes(d[20..24].try_into().unwrap()));
                }
                for &k in children.get(&id).into_iter().flatten().rev() {
                    stack.push((k, depth + 1));
                }
            }
        }
        Some("hkb-payloads") => {
            // hkb-payloads <data dir> <project dir>: clip triggers carrying string payloads.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let v = vfs::Vfs::new(data, &names);
            let (_, project) = load_project(&v, &args[2])?;
            for (rel, g) in &project.graphs {
                for node in &g.generators {
                    if let havok::behavior::Generator::StateMachine { name, start, start_variable: Some(v), .. } = node {
                        println!("{rel} sm {name} start {start} bound to {} = {:?}", g.variables.get(*v).map_or("?", String::as_str), g.variable_defaults.get(*v));
                    }
                    if let havok::behavior::Generator::StateMachine { name, states, .. } = node {
                        for st in states {
                            for (when, e) in st.enter_events.iter().map(|e| ("enter", e)).chain(st.exit_events.iter().map(|e| ("exit", e))) {
                                println!("{rel} sm {name} state {} {when} {} {:?}", st.name, g.event_name(e.event).unwrap_or("?"), e.payload.as_deref().unwrap_or(""));
                            }
                        }
                    }
                    let havok::behavior::Generator::Clip { name, animation, triggers, .. } = node else { continue };
                    for t in triggers.iter().filter(|t| t.payload.is_some()) {
                        let end = if t.from_end { " from end" } else { "" };
                        println!("{rel} {name} ({animation}) @{:.3}{end} {} {:?}", t.time, g.event_name(t.event).unwrap_or("?"), t.payload.as_deref().unwrap_or(""));
                    }
                }
            }
        }
        Some("idlm-check") => {
            // idlm-check <data dir>: resolve the humanoid idles of every idle marker.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let v = vfs::Vfs::new(data, &names);
            let project = havok::behavior::Project::load("behaviors/0_master.hkx", |rel| v.read(&format!("meshes/actors/character/{rel}")));
            let (mut ok, mut bad) = (0, 0);
            for &m in lo.ids_of_type(b"IDLM") {
                let Some(rec) = lo.get(m) else { continue };
                let Some(list) = rec.get(b"IDLA") else { continue };
                for c in list.chunks_exact(4) {
                    let idle = rec.fid(esp::FormId(u32::from_le_bytes(c.try_into().unwrap())));
                    let Some(i) = lo.get(idle) else { continue };
                    let dnam = i.get(b"DNAM").map(esp::decode_zstring).unwrap_or_default().to_ascii_lowercase();
                    if !dnam.is_empty() && !dnam.starts_with("actors\\character\\") {
                        continue;
                    }
                    let event = i.get(b"ENAM").map(esp::decode_zstring).unwrap_or_default();
                    let seqs = project.clips_for_event(&event);
                    if seqs.is_empty() {
                        bad += 1;
                        println!("unresolved {} {} event {event:?}", rec.editor_id().unwrap_or_default(), i.editor_id().unwrap_or_default());
                    } else {
                        ok += 1;
                    }
                }
            }
            println!("{ok} resolved, {bad} unresolved");
        }
        Some("pack-templates") => {
            // pack-templates <data dir>: how many NPC package slots use each procedure template.
            // SHOW=<template> lists the NPCs and packages using one.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let mut count: std::collections::BTreeMap<String, usize> = Default::default();
            for &npc in lo.ids_of_type(b"NPC_") {
                let Some(rec) = lo.get(npc) else { continue };
                for s in rec.subrecords().filter(|s| s.tag.0 == *b"PKID") {
                    let Some(pack) = lo.get(rec.fid(s.form_id(0))) else { continue };
                    let Some(cu) = pack.get(b"PKCU").filter(|d| d.len() >= 8) else { continue };
                    let t = pack.fid(esp::FormId(u32::from_le_bytes(cu[4..8].try_into().unwrap())));
                    let name = lo.get(t).and_then(|r| r.editor_id()).unwrap_or_default();
                    // SHOW=<template>: the NPCs and packages using it.
                    if std::env::var("SHOW").is_ok_and(|v| v.eq_ignore_ascii_case(&name)) {
                        println!("{npc} {} -> {} {}", rec.editor_id().unwrap_or_default(), pack.form_id(), pack.editor_id().unwrap_or_default());
                    }
                    *count.entry(name).or_default() += 1;
                }
            }
            let mut v: Vec<_> = count.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1));
            for (n, c) in v.iter().take(40) {
                println!("{c:6} {n}");
            }
        }
        Some("force-greets") => {
            // force-greets <data dir>: ForceGreet / ForceGreetFromSitting packages with their
            // topic, trigger location, force greet distance, "must be detected" and "sandbox
            // while waiting" inputs, the NPCs using them and where those are placed.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let mut users: std::collections::HashMap<esp::FormId, Vec<esp::FormId>> = Default::default();
            for &npc in lo.ids_of_type(b"NPC_") {
                let Some(rec) = lo.get(npc) else { continue };
                for s in rec.subrecords().filter(|s| s.tag.0 == *b"PKID") {
                    users.entry(rec.fid(s.form_id(0))).or_default().push(npc);
                }
            }
            let mut placed: std::collections::HashMap<esp::FormId, Vec<esp::FormId>> = Default::default();
            for &r in lo.ids_of_type(b"ACHR") {
                let Some(rec) = lo.get(r) else { continue };
                let Some(b) = rec.get(b"NAME").filter(|d| d.len() >= 4) else { continue };
                placed.entry(rec.fid(esp::FormId(u32::from_le_bytes(b[0..4].try_into().unwrap())))).or_default().push(r);
            }
            for &id in lo.ids_of_type(b"PACK") {
                let Some(rec) = lo.get(id) else { continue };
                let Some(cu) = rec.get(b"PKCU").filter(|d| d.len() >= 8) else { continue };
                let t = rec.fid(esp::FormId(u32::from_le_bytes(cu[4..8].try_into().unwrap())));
                let template = lo.get(t).and_then(|r| r.editor_id()).unwrap_or_default();
                if !template.to_ascii_lowercase().starts_with("forcegreet") {
                    continue;
                }
                // Input values in record order, then their indices (UNAM).
                let (mut values, mut indices) = (Vec::<String>::new(), Vec::<u8>::new());
                for sr in rec.subrecords() {
                    let d = sr.data;
                    match &sr.tag.0 {
                        b"XNAM" => break,
                        b"ANAM" => values.push(String::new()),
                        b"CNAM" if !d.is_empty() => *values.last_mut().unwrap() = format!("{}", d[0] != 0),
                        b"PDTO" if d.len() >= 8 && !values.is_empty() => {
                            let v = u32::from_le_bytes(d[4..8].try_into().unwrap());
                            *values.last_mut().unwrap() = if d[0] == 0 {
                                let f = rec.fid(esp::FormId(v));
                                format!("{f} {}", lo.get(f).and_then(|r| r.editor_id()).unwrap_or_default())
                            } else {
                                String::from_utf8_lossy(&d[4..8]).into_owned()
                            };
                        }
                        b"PLDT" if d.len() >= 12 && !values.is_empty() => {
                            let k = u32::from_le_bytes(d[0..4].try_into().unwrap());
                            let v = rec.fid(esp::FormId(u32::from_le_bytes(d[4..8].try_into().unwrap())));
                            *values.last_mut().unwrap() = format!("kind {k} {v} r {}", i32::from_le_bytes(d[8..12].try_into().unwrap()));
                        }
                        b"UNAM" if !d.is_empty() => indices.push(d[0]),
                        _ => {}
                    }
                }
                let input = |i: u8| indices.iter().position(|&x| x == i).and_then(|p| values.get(p).cloned()).unwrap_or_else(|| "-".into());
                println!(
                    "{id} {} ({template}): topic {} | trigger {} | distance {} | detect {} | sandbox {}",
                    rec.editor_id().unwrap_or_default(),
                    input(0x07),
                    input(0x3e),
                    input(0x4b),
                    if input(0x4f) == "-" { input(0x4d) } else { input(0x4f) },
                    input(0x28)
                );
                for npc in users.get(&id).into_iter().flatten() {
                    let edid = lo.get(*npc).and_then(|r| r.editor_id()).unwrap_or_default();
                    let refs: Vec<String> = placed
                        .get(npc)
                        .into_iter()
                        .flatten()
                        .map(|r| {
                            let cell = lo.cell_of_ref(*r).and_then(|c| lo.get(c).and_then(|c| c.editor_id())).unwrap_or_default();
                            format!("{r} in {cell}")
                        })
                        .collect();
                    println!("    {npc} {edid}: {}", refs.join(", "));
                }
            }
        }
        Some("alias-packages") => {
            // alias-packages <data dir>: quest alias packages (ALPC) by procedure template, how
            // many aliases carry them, and the location (PLDT) / target (PTDA) kinds all
            // packages use, with whether they have an owner quest (QNAM).
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let (mut aliases, mut slots) = (0, 0);
            let mut templates: std::collections::BTreeMap<String, usize> = Default::default();
            for &q in lo.ids_of_type(b"QUST") {
                let Some(rec) = lo.get(q) else { continue };
                let mut has = false;
                for sr in rec.subrecords() {
                    match &sr.tag.0 {
                        b"ALST" | b"ALLS" => has = false,
                        b"ALPC" => {
                            if !has {
                                aliases += 1;
                                has = true;
                            }
                            slots += 1;
                            let pack = rec.fid(sr.form_id(0));
                            let t = lo
                                .get(pack)
                                .and_then(|p| p.get(b"PKCU").filter(|d| d.len() >= 8).map(|cu| p.fid(esp::FormId(u32::from_le_bytes(cu[4..8].try_into().unwrap())))))
                                .and_then(|t| lo.get(t).and_then(|t| t.editor_id()))
                                .unwrap_or_default();
                            if std::env::var("SHOW").is_ok_and(|v| v.eq_ignore_ascii_case(&t)) {
                                println!("{q} {} -> {pack} {}", rec.editor_id().unwrap_or_default(), lo.get(pack).and_then(|p| p.editor_id()).unwrap_or_default());
                            }
                            *templates.entry(t).or_default() += 1;
                        }
                        _ => {}
                    }
                }
            }
            println!("{slots} alias package slots on {aliases} aliases");
            let mut v: Vec<_> = templates.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1));
            for (n, c) in v.iter().take(30) {
                println!("{c:6} {n}");
            }
            let (mut pldt, mut ptda): (std::collections::BTreeMap<u32, usize>, std::collections::BTreeMap<u32, usize>) = Default::default();
            let (mut owned, mut packs) = (0, 0);
            for &id in lo.ids_of_type(b"PACK") {
                let Some(rec) = lo.get(id) else { continue };
                packs += 1;
                owned += rec.get(b"QNAM").is_some() as usize;
                for sr in rec.subrecords() {
                    match &sr.tag.0 {
                        b"XNAM" => break,
                        b"PLDT" if sr.data.len() >= 4 => *pldt.entry(sr.u32(0)).or_default() += 1,
                        b"PTDA" if sr.data.len() >= 4 => *ptda.entry(sr.u32(0)).or_default() += 1,
                        _ => {}
                    }
                }
            }
            println!("{packs} packages, {owned} with an owner quest");
            println!("location kinds {pldt:?}");
            println!("target kinds {ptda:?}");
        }
        Some("npc-templates") => {
            // npc-templates <data dir>: NPCs with a template (TPLT), by what it is (NPC_ or a
            // leveled list), how often each "Use ..." flag (ACBS) is set, and ACHRs placing
            // templated NPCs through a leveled template.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            const FLAGS: [&str; 13] = [
                "traits", "stats", "factions", "spells", "ai data", "ai packages", "model/anim (unused)", "base data",
                "inventory", "script", "def pack list", "attack data", "keywords",
            ];
            let (mut to_npc, mut to_lvln, mut flags) = (0, 0, [0usize; 13]);
            let mut lvln_flags = [0usize; 13];
            for &npc in lo.ids_of_type(b"NPC_") {
                let Some(rec) = lo.get(npc) else { continue };
                let Some(t) = rec.get(b"TPLT").filter(|d| d.len() >= 4) else { continue };
                let t = rec.fid(esp::FormId(u32::from_le_bytes(t[0..4].try_into().unwrap())));
                let Some(tr) = lo.get(t) else { continue };
                let lvln = tr.tag().0 == *b"LVLN";
                if lvln { to_lvln += 1 } else { to_npc += 1 }
                let f = rec.get(b"ACBS").filter(|d| d.len() >= 20).map_or(0, |d| u16::from_le_bytes([d[18], d[19]]));
                for (i, n) in flags.iter_mut().enumerate() {
                    if f & (1 << i) != 0 {
                        *n += 1;
                        if lvln {
                            lvln_flags[i] += 1;
                        }
                    }
                }
            }
            println!("templated NPCs: {to_npc} on an NPC, {to_lvln} on a leveled list");
            let has = |tag: &[u8; 4]| lo.ids_of_type(b"NPC_").iter().filter(|&&n| lo.get(n).is_some_and(|r| r.get(tag).is_some())).count();
            println!("NPCs with an attack race (ATKR) {}, own attacks (ATKD) {}", has(b"ATKR"), has(b"ATKD"));
            for (i, n) in FLAGS.iter().enumerate() {
                println!("  0x{:04X} use {n:22} {:6} ({} through a leveled list)", 1 << i, flags[i], lvln_flags[i]);
            }
        }
        Some("pack-speeds") => {
            // pack-speeds <data dir>: packages by type, preferred speed flag / speed, sneak flag.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
            for &id in lo.ids_of_type(b"PACK") {
                let Some(rec) = lo.get(id) else { continue };
                let Some(d) = rec.get(b"PKDT").filter(|d| d.len() >= 8) else { continue };
                let flags = u32::from_le_bytes(d[0..4].try_into().unwrap());
                let key = format!("type {:2} preferred {} speed {} sneak {}", d[4], flags >> 13 & 1, d[6], flags >> 17 & 1);
                if flags >> 13 & 1 != 0 && d[6] >= 2 && std::env::var_os("SHOW").is_some() {
                    println!("{id} {}", rec.editor_id().unwrap_or_default());
                }
                *counts.entry(key).or_default() += 1;
            }
            for (k, n) in counts {
                println!("{n:6} {k}");
            }
        }
        Some("dial-subtypes") => {
            // dial-subtypes <data dir>: dialogue topic subtypes (SNAM) with counts and examples.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let mut by: std::collections::BTreeMap<String, (usize, Vec<String>)> = Default::default();
            for &id in lo.ids_of_type(b"DIAL") {
                let Some(rec) = lo.get(id) else { continue };
                let sub = rec.get(b"SNAM").and_then(|d| d.get(0..4)).map(|d| String::from_utf8_lossy(d).into_owned()).unwrap_or_default();
                let e = by.entry(sub).or_default();
                e.0 += 1;
                if e.1.len() < 3 {
                    e.1.push(rec.editor_id().unwrap_or_default().to_string());
                }
            }
            for (k, (n, ex)) in by {
                println!("{n:6} {k} {ex:?}");
            }
        }
        Some("locks") => {
            // locks <data dir>: lock levels and flags (`XLOC`) over every reference,
            // by base record type, with an example of each.
            use std::collections::BTreeMap;
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let mut counts: BTreeMap<(String, u8, u8, bool), (usize, esp::FormId)> = BTreeMap::new();
            for &c in lo.ids_of_type(b"CELL") {
                let Some(idx) = lo.cell(c) else { continue };
                for &r in idx.persistent.iter().chain(&idx.temporary) {
                    let Some(rec) = lo.get(r) else { continue };
                    let Some(x) = rec.get(b"XLOC").filter(|d| d.len() >= 9) else { continue };
                    let base = rec.get(b"NAME").filter(|d| d.len() >= 4).map(|b| rec.fid(esp::FormId(u32::from_le_bytes(b[0..4].try_into().unwrap()))));
                    let tag = base.and_then(|b| lo.tag_of(b)).map(|t| t.to_string()).unwrap_or_default();
                    let key = u32::from_le_bytes(x[4..8].try_into().unwrap()) != 0;
                    let e = counts.entry((tag, x[0], x[8], key)).or_insert((0, r));
                    e.0 += 1;
                }
            }
            for ((tag, level, flags, key), (n, example)) in counts {
                println!("{tag} level {level:3} flags {flags:02X} key {key}: {n} (e.g. {example})");
            }
            // Who owns locked load doors: the door (`XOWN`), its cell, the far side or
            // the far side's cell, owned by an NPC or a faction.
            let fid = |rec: &esp::LoadedRecord<'_>, tag: &[u8; 4]| {
                rec.get(tag).filter(|d| d.len() >= 4).map(|d| rec.fid(esp::FormId(u32::from_le_bytes(d[0..4].try_into().unwrap())))).filter(|f| !f.is_null())
            };
            let owner = |r: esp::FormId| {
                let rec = lo.get(r)?;
                fid(&rec, b"XOWN").map(|o| ("door", o)).or_else(|| lo.get(lo.cell_of_ref(r)?).and_then(|c| fid(&c, b"XOWN")).map(|o| ("cell", o)))
            };
            let mut owned: BTreeMap<String, (usize, esp::FormId)> = BTreeMap::new();
            for &c in lo.ids_of_type(b"CELL") {
                let Some(idx) = lo.cell(c) else { continue };
                for &r in idx.persistent.iter().chain(&idx.temporary) {
                    let Some(rec) = lo.get(r) else { continue };
                    if rec.get(b"XLOC").is_none() {
                        continue;
                    }
                    let Some(partner) = fid(&rec, b"XTEL") else { continue };
                    let near = owner(r);
                    let far = owner(partner);
                    let desc = |o: Option<(&str, esp::FormId)>| match o {
                        Some((at, f)) => format!("{at} {}", lo.tag_of(f).map(|t| t.to_string()).unwrap_or_default()),
                        None => "none".into(),
                    };
                    let e = owned.entry(format!("this side {} / far side {}", desc(near), desc(far))).or_insert((0, r));
                    e.0 += 1;
                }
            }
            for (k, (n, example)) in owned {
                println!("locked load doors, {k}: {n} (e.g. {example})");
            }
            // Locked doors within a cell (no teleport) in cells with persistent actors:
            // those NPCs may walk up to.
            for &c in lo.ids_of_type(b"CELL") {
                let Some(idx) = lo.cell(c) else { continue };
                let actors = idx.persistent.iter().filter(|&&r| lo.tag_of(r).is_some_and(|t| t.0 == *b"ACHR")).count();
                if actors == 0 {
                    continue;
                }
                let doors: Vec<String> = idx
                    .persistent
                    .iter()
                    .chain(&idx.temporary)
                    .filter(|&&r| lo.get(r).is_some_and(|rec| rec.get(b"XLOC").is_some() && rec.get(b"XTEL").is_none()))
                    .filter(|&&r| lo.get(r).and_then(|rec| fid(&rec, b"NAME")).and_then(|b| lo.tag_of(b)).is_some_and(|t| t.0 == *b"DOOR"))
                    .map(|&r| format!("{r} (owner {:?})", owner(r).map(|o| o.1)))
                    .collect();
                if !doors.is_empty() {
                    let name = lo.get(c).and_then(|r| r.editor_id()).unwrap_or_default();
                    println!("{name} ({c}), {actors} actors: locked doors {}", doors.join(", "));
                }
            }
        }
        Some("cell-refs") => {
            // cell-refs <data dir> <cell editor id | hex | [world:]x,y> [base tag]: a cell's
            // references with their base records, positions and rotations (degrees).
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let cell = if let Some((x, y)) = args[2].rsplit(':').next().and_then(|g| g.split_once(',')) {
                let world = args[2].split_once(':').map_or("Tamriel", |(w, _)| w);
                let w = lo.find_editor_id(world).context("world not found")?;
                let g = (x.trim().parse::<i32>()?, y.trim().parse::<i32>()?);
                *lo.world(w).and_then(|w| w.cells.get(&g)).context("no cell there")?
            } else {
                match u32::from_str_radix(&args[2], 16) {
                    Ok(v) if args[2].len() == 8 => esp::FormId(v),
                    _ => lo.find_editor_id(&args[2]).context("cell not found")?,
                }
            };
            let idx = lo.cell(cell).context("not a cell")?;
            for &r in idx.persistent.iter().chain(&idx.temporary) {
                let Some(rec) = lo.get(r) else { continue };
                let Some(b) = rec.get(b"NAME").filter(|d| d.len() >= 4) else { continue };
                let base = rec.fid(esp::FormId(u32::from_le_bytes(b[0..4].try_into().unwrap())));
                let tag = lo.tag_of(base).map(|t| t.to_string()).unwrap_or_default();
                if args.get(3).is_some_and(|want| !want.eq_ignore_ascii_case(&tag)) {
                    continue;
                }
                let edid = lo.get(base).and_then(|b| b.editor_id().map(|e| e.to_string())).unwrap_or_default();
                let placement = rec
                    .get(b"DATA")
                    .filter(|d| d.len() >= 24)
                    .map(|d| {
                        let f = |i: usize| f32::from_le_bytes(d[i * 4..i * 4 + 4].try_into().unwrap());
                        format!(" @ {:.0},{:.0},{:.0} rot {:.1},{:.1},{:.1}", f(0), f(1), f(2), f(3).to_degrees(), f(4).to_degrees(), f(5).to_degrees())
                    })
                    .unwrap_or_default();
                println!("{r} {} -> {base} {tag} {edid}{placement}", rec.tag());
            }
        }
        Some("esp-list") => {
            // esp-list <data dir> <TAG> [edid substring]: records of a type with their text subrecords.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let tag: [u8; 4] = args[2].as_bytes().try_into().context("TAG must be 4 bytes")?;
            let filter = args.get(3).map(|f| f.to_ascii_lowercase());
            for &id in lo.ids_of_type(&tag) {
                let Some(rec) = lo.get(id) else { continue };
                let edid = rec.editor_id().unwrap_or_default();
                if filter.as_ref().is_some_and(|f| !edid.to_ascii_lowercase().contains(f.as_str())) {
                    continue;
                }
                let text: Vec<String> = rec
                    .subrecords()
                    .filter(|s| s.tag.0 != *b"EDID" && s.data.len() > 1 && s.data[..s.data.len() - 1].iter().all(|&b| (32..127).contains(&b)) && s.data.last() == Some(&0))
                    .map(|s| format!("{}={}", s.tag, String::from_utf8_lossy(&s.data[..s.data.len() - 1])))
                    .collect();
                println!("{id} {edid} {}", text.join(" "));
            }
        }
        Some("hkx-probe") => {
            // hkx-probe <data dir> <vfs path> <class> [max objects]: pointer slots of each object.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let v = vfs::Vfs::new(data, &names);
            let bytes = v.read(&args[2]).context("not found")?;
            let p = havok::Packfile::parse(&bytes)?;
            let max: usize = args.get(4).map(|s| s.parse()).transpose()?.unwrap_or(3);
            let mut offs: Vec<u32> = p.objects.iter().map(|o| o.offset).collect();
            offs.sort();
            for o in p.objects_of(&args[3]).take(max) {
                let end = offs.iter().copied().find(|&x| x > o).unwrap_or(p.data.len() as u32);
                println!("{} @{o:#x} size {:#x}", args[3], end - o);
                for slot in (o..end).step_by(4) {
                    let rel = slot - o;
                    if let Some(t) = p.ptr(slot) {
                        let what = match p.object_class(t) {
                            Some(c) => format!("-> {c} @{t:#x}"),
                            None => {
                                let s: String = p.data[t as usize..].iter().take(60).take_while(|&&b| b != 0).map(|&b| b as char).collect();
                                format!("-> {t:#x} {s:?}")
                            }
                        };
                        println!("  +{rel:#04x} ptr {what}  (next i32 {})", p.i32(slot + 8));
                    } else if rel % 4 == 0 {
                        let x = p.u32(slot);
                        if x != 0 {
                            println!("  +{rel:#04x} {x:#010x} ({} / {:.3})", x as i32, f32::from_bits(x));
                        }
                    }
                }
            }
        }
        Some("pex-dump") => {
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let v = vfs::Vfs::new(data, &names);
            let bytes = v.read(&format!("scripts/{}.pex", args[2])).context("script not found")?;
            let p = papyrus::pex::parse(&bytes)?;
            print!("{}", papyrus::pex::disassemble(&p));
        }
        Some("pex-verify") => {
            let a = bsa::Archive::open(&args[1])?;
            let mut ok = 0;
            let mut bad = 0;
            let mut ops = [0usize; 36];
            for p in a.paths().filter(|p| p.ends_with(".pex")).map(str::to_owned).collect::<Vec<_>>() {
                match papyrus::pex::parse(&a.read(&p)?.unwrap()) {
                    Ok(x) => {
                        ok += 1;
                        for o in &x.objects {
                            for s in &o.states {
                                for (_, f) in &s.functions {
                                    for i in &f.code {
                                        ops[i.op as usize] += 1;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        bad += 1;
                        if bad < 10 {
                            println!("{p}: {e}");
                        }
                    }
                }
            }
            println!("{ok} ok, {bad} failed");
            for (i, c) in ops.iter().enumerate() {
                println!("{:>20} {c}", papyrus::pex::OP_NAMES[i]);
            }
        }
        Some("nif-points") => {
            // nif-points <data dir> <vfs path>: print vertices of the first TriShape near given xy
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let v = vfs::Vfs::new(data, &names);
            let n = nif::Nif::parse(&v.read(&args[2]).context("not found")?)?;
            for b in &n.blocks {
                if let nif::Block::TriShape(t) = b {
                    let mut pts = t.geometry.positions.clone();
                    pts.sort_by(|a, b| (a.x + a.y * 10000.0).total_cmp(&(b.x + b.y * 10000.0)));
                    for p in pts.iter().take(5) {
                        println!("{p:?}");
                    }
                    break;
                }
            }
        }
        Some("ctda-stats") => {
            // ctda-stats <data dir> <TYPE>: condition function usage counts
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let tag: [u8; 4] = args[2].as_bytes().try_into().context("4-char type")?;
            let mut counts: std::collections::HashMap<(u16, u32), usize> = Default::default();
            for &id in lo.ids_of_type(&tag) {
                if let Some(r) = lo.get(id) {
                    for s in r.subrecords().filter(|s| s.tag.0 == *b"CTDA") {
                        let f = u16::from_le_bytes([s.data[8], s.data[9]]);
                        let run_on = u32::from_le_bytes(s.data[20..24].try_into().unwrap());
                        *counts.entry((f, run_on)).or_default() += 1;
                    }
                }
            }
            let mut v: Vec<_> = counts.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1));
            for ((f, run_on), c) in v.iter().take(60) {
                println!("{c:>7} func {f:>4} run_on {run_on}");
            }
        }
        Some("navm-verify") => {
            // navm-verify <data dir>: parse every navmesh and check its indices
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let (mut ok, mut bad, mut missing) = (0usize, 0usize, 0usize);
            let mut versions: std::collections::BTreeMap<u32, usize> = Default::default();
            for &id in lo.ids_of_type(b"NAVM") {
                let Some(r) = lo.get(id) else { continue };
                let Some(d) = r.get(b"NVNM") else {
                    missing += 1;
                    continue;
                };
                match esp::navmesh::NavMesh::parse(d, |f| r.fid(f)) {
                    Some(m) => {
                        *versions.entry(m.version).or_default() += 1;
                        match m.validate() {
                            Ok(()) => ok += 1,
                            Err(e) => {
                                bad += 1;
                                if bad < 10 {
                                    println!("{id}: {e}");
                                }
                            }
                        }
                    }
                    None => {
                        bad += 1;
                        if bad < 10 {
                            println!("{id}: parse failed ({} bytes)", d.len());
                        }
                    }
                }
            }
            println!("{ok} ok, {bad} bad, {missing} without NVNM; versions {versions:?}");
            // Cross-mesh links must land on an existing triangle that links back.
            let mut meshes = std::collections::HashMap::new();
            for &id in lo.ids_of_type(b"NAVM") {
                if let Some(r) = lo.get(id)
                    && let Some(m) = r.get(b"NVNM").and_then(|d| esp::navmesh::NavMesh::parse(d, |f| r.fid(f)))
                {
                    meshes.insert(id, m);
                }
            }
            let (mut links, mut dangling, mut one_way) = (0usize, 0usize, 0usize);
            for (&id, m) in &meshes {
                for l in &m.edge_links {
                    links += 1;
                    match meshes.get(&l.navmesh) {
                        Some(t) if (l.triangle as usize) < t.triangles.len() => {
                            let tri = &t.triangles[l.triangle as usize];
                            let back = (0..3).filter_map(|e| tri.link(e)).any(|i| t.edge_links[i].navmesh == id);
                            if !back {
                                one_way += 1;
                            }
                        }
                        _ => dangling += 1,
                    }
                }
            }
            println!("{links} edge links: {dangling} dangling, {one_way} without a link back");
        }
        _ => bail!("usage: vrm-tool <bsa-list|bsa-extract|bsa-verify> ..."),
    }
    Ok(())
}
