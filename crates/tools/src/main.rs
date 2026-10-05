use anyhow::{Context, Result, bail};

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
                    nif::Block::Shape(nif::Shape::CompressedMeshData { vertices, triangles }) => {
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
            let dir = args[2].trim_end_matches('/');
            let project = havok::behavior::Project::load("behaviors/0_master.hkx", |rel| v.read(&format!("{dir}/{rel}")));
            for (rel, g) in &project.graphs {
                let clips = g.generators.iter().filter(|g| matches!(g, havok::behavior::Generator::Clip { .. })).count();
                println!("{rel}: {} generators, {clips} clips, {} events", g.generators.len(), g.events.len());
            }
            for e in &args[3..] {
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
                    G::StateMachine { name, start, start_variable, states, wildcards } => {
                        let var = start_variable.map(|v| format!(" (bound to {} = {:?})", g.variables.get(v).map_or("?", String::as_str), g.variable_defaults.get(v)));
                        println!("{pad}sm {name} start {start}{}", var.unwrap_or_default());
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
        Some("hkb-classes") => {
            // hkb-classes <data dir> <project dir>: object classes across the project's graphs.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let v = vfs::Vfs::new(data, &names);
            let dir = args[2].trim_end_matches('/');
            let project = havok::behavior::Project::load("behaviors/0_master.hkx", |rel| v.read(&format!("{dir}/{rel}")));
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
            let bytes = v.read(&format!("{dir}/behaviors/{}", args[3])).context("graph not found")?;
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
            let dir = args[2].trim_end_matches('/').to_owned();
            let project = std::sync::Arc::new(havok::behavior::Project::load("behaviors/0_master.hkx", |rel| v.read(&format!("{dir}/{rel}"))));
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
            let dir = args[2].trim_end_matches('/');
            let project = havok::behavior::Project::load("behaviors/0_master.hkx", |rel| v.read(&format!("{dir}/{rel}")));
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
                    *count.entry(name).or_default() += 1;
                }
            }
            let mut v: Vec<_> = count.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1));
            for (n, c) in v.iter().take(40) {
                println!("{c:6} {n}");
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
