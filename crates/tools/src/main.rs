use anyhow::{Context, Result, bail};

#[path = "../../../src/condition/functions.rs"]
#[allow(dead_code)]
mod functions;

/// A behaviour project named by its directory (humanoids: `behaviors/0_master.hkx`)
/// or by its project file (`meshes/actors/canine/dogproject.hkx`), with its directory.
fn load_project(v: &vfs::Vfs, arg: &str) -> Result<(String, havok::behavior::Project)> {
    let arg = arg.trim_end_matches('/').to_ascii_lowercase();
    if let Some((dir, file)) = arg.rsplit_once('/').filter(|_| arg.ends_with(".hkx")) {
        let p =
            havok::behavior::Project::load_project(file, |rel| v.read(&format!("{dir}/{rel}")))?;
        if let Some(c) = &p.character {
            eprintln!(
                "character {} rig {} behaviour {}, {} graphs",
                c.name,
                c.rig,
                c.behavior,
                p.graphs.len()
            );
            if let Some(f) = &c.foot_ik {
                eprintln!("foot IK: {f:?}");
            }
        }
        return Ok((dir.to_owned(), p));
    }
    let p = havok::behavior::Project::load("behaviors/0_master.hkx", |rel| {
        v.read(&format!("{arg}/{rel}"))
    });
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
                println!(
                    "{:?} {} masters={:?}",
                    p.slot,
                    p.plugin.name(),
                    p.plugin.header().masters
                );
            }
            for tag in [
                b"CELL", b"WRLD", b"STAT", b"REFR", b"ACHR", b"NPC_", b"LAND",
            ] {
                println!(
                    "{}: {}",
                    String::from_utf8_lossy(tag),
                    lo.ids_of_type(tag).len()
                );
            }
            let t = std::time::Instant::now();
            let id = lo.find_editor_id("WhiterunBanneredMare");
            println!("edid lookup {:?} took {:?}", id, t.elapsed());
            if let Some(id) = id {
                let c = lo.cell(id).unwrap();
                println!(
                    "persistent {} temporary {}",
                    c.persistent.len(),
                    c.temporary.len()
                );
            }
        }
        Some("esp-dump") => {
            // esp-dump <data dir> <form id | editor id>: subrecords, the first 48
            // bytes of each (FULL=1: all of them); LSTR=1 adds the localised string
            // of 4-byte FULL / NNAM / CNAM / DESC / ITXT / RNAM fields.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let mut lo = esp::LoadOrder::load(data, &names)?;
            if std::env::var("LSTR").is_ok() {
                let vfs = vfs::Vfs::new(data, &names);
                lo.load_strings("english", |p| vfs.read(p));
            }
            let id = match u32::from_str_radix(&args[2], 16) {
                Ok(v) if args[2].len() == 8 => esp::FormId(v),
                _ => lo.find_editor_id(&args[2]).context("editor id not found")?,
            };
            let rec = lo.get(id).context("record not found")?;
            println!(
                "{} {} flags={:08X} from {}",
                rec.tag(),
                id,
                rec.flags(),
                rec.plugin.plugin.name()
            );
            for sr in rec.subrecords() {
                let hex: String = sr
                    .data
                    .iter()
                    .take(std::env::var("FULL").map(|_| 100000).unwrap_or(48))
                    .map(|b| format!("{b:02x}"))
                    .collect();
                let txt: String = sr
                    .data
                    .iter()
                    .take(std::env::var("FULL").map(|_| 100000).unwrap_or(48))
                    .map(|&b| {
                        if (32..127).contains(&b) {
                            b as char
                        } else {
                            '.'
                        }
                    })
                    .collect();
                println!("  {} [{}] {hex} {txt}", sr.tag, sr.data.len());
                let lstr = [b"FULL", b"NNAM", b"CNAM", b"DESC", b"ITXT", b"RNAM"];
                if std::env::var("LSTR").is_ok()
                    && sr.data.len() == 4
                    && lstr.iter().any(|t| sr.tag.0 == **t)
                {
                    println!("    = {:?}", lo.lstring(&rec, sr.data));
                }
            }
        }
        Some("gmst") => {
            // Game settings whose editor id contains the pattern (case-insensitive).
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let mut lo = esp::LoadOrder::load(data, &names)?;
            let vfs = vfs::Vfs::new(data, &names);
            lo.load_strings("english", |p| vfs.read(p));
            let pat = args
                .get(2)
                .map_or(String::new(), |p| p.to_ascii_lowercase());
            let mut rows: Vec<(String, String)> = Vec::new();
            for &id in lo.ids_of_type(b"GMST") {
                let Some(rec) = lo.get(id) else { continue };
                let Some(name) = rec.editor_id().map(|e| e.to_string()) else {
                    continue;
                };
                if !name.to_ascii_lowercase().contains(&pat) {
                    continue;
                }
                let d = rec.get(b"DATA").unwrap_or(&[]);
                let value = match (name.as_bytes().first(), d.get(0..4)) {
                    (Some(b'f'), Some(b)) => {
                        format!("{}", f32::from_le_bytes(b.try_into().unwrap()))
                    }
                    (Some(b'i' | b'u'), Some(b)) => {
                        format!("{}", i32::from_le_bytes(b.try_into().unwrap()))
                    }
                    (Some(b'b'), Some(b)) => {
                        format!("{}", u32::from_le_bytes(b.try_into().unwrap()) != 0)
                    }
                    (Some(b's'), Some(_)) => format!("{:?}", lo.lstring(&rec, d)),
                    _ => format!("{} bytes", d.len()),
                };
                rows.push((name, value));
            }
            rows.sort();
            for (n, v) in rows {
                println!("{n} = {v}");
            }
        }
        Some("envmap-shapes") => {
            // envmap-shapes <bsa>...: lighting shaders with environment
            // mapping, counted by shader type, whether they name a cube map
            // (slot 4) and an environment mask (slot 5); the cube maps used.
            let mut kinds = std::collections::BTreeMap::<(u32, bool, bool), usize>::new();
            let mut cubes = std::collections::BTreeMap::<String, usize>::new();
            for path in &args[1..] {
                let a = bsa::Archive::open(path)?;
                let paths: Vec<String> = a
                    .paths()
                    .filter(|p| p.ends_with(".nif"))
                    .map(str::to_owned)
                    .collect();
                for p in paths {
                    let Ok(n) = nif::Nif::parse(&a.read(&p)?.unwrap()) else {
                        continue;
                    };
                    for b in &n.blocks {
                        let nif::Block::LightingShader(s) = b else {
                            continue;
                        };
                        if s.flags1
                            & (nif::sf1::ENVIRONMENT_MAPPING | nif::sf1::EYE_ENVIRONMENT_MAPPING)
                            == 0
                        {
                            continue;
                        }
                        let t = match n.get(s.texture_set) {
                            Some(nif::Block::TextureSet(t)) => t.clone(),
                            _ => Vec::new(),
                        };
                        let slot = |i: usize| {
                            t.get(i)
                                .map(|s| s.trim().to_lowercase())
                                .filter(|s| !s.is_empty())
                        };
                        let (cube, mask) = (slot(4), slot(5));
                        *kinds
                            .entry((s.shader_type, cube.is_some(), mask.is_some()))
                            .or_default() += 1;
                        if let Some(c) = cube {
                            *cubes.entry(c).or_default() += 1;
                        }
                    }
                }
            }
            for ((ty, c, m), k) in kinds {
                println!("type {ty:2} cube {c:5} mask {m:5}: {k}");
            }
            for (c, k) in cubes {
                println!("{k:6} {c}");
            }
        }
        Some("sf1-files") => {
            // sf1-files <bit> <bsa>...: models with shader properties (lighting
            // or effect) that have shader flags 1 bit <bit>, and how many.
            let bit: u32 = args[1].parse()?;
            let mut files = std::collections::BTreeMap::<String, usize>::new();
            for path in &args[2..] {
                let a = bsa::Archive::open(path)?;
                let paths: Vec<String> = a
                    .paths()
                    .filter(|p| p.ends_with(".nif"))
                    .map(str::to_owned)
                    .collect();
                for p in paths {
                    let Ok(n) = nif::Nif::parse(&a.read(&p)?.unwrap()) else {
                        continue;
                    };
                    for b in &n.blocks {
                        let flags1 = match b {
                            nif::Block::LightingShader(s) => s.flags1,
                            nif::Block::EffectShader(s) => s.flags1,
                            _ => continue,
                        };
                        if flags1 & (1 << bit) != 0 {
                            *files.entry(p.clone()).or_default() += 1;
                        }
                    }
                }
            }
            println!("{} files", files.len());
            for (f, k) in files {
                println!("{k:4} {f}");
            }
        }
        Some("msn-shapes") => {
            // msn-shapes <bsa>...: shapes whose lighting shader has model-space
            // normals, counted by whether they're drawn skinned; the files of
            // those drawn rigid (unskinned, or in models hung from a bone or trees).
            let (mut skinned, mut rigid) = (0usize, 0usize);
            let mut files = std::collections::BTreeMap::<String, usize>::new();
            for path in &args[1..] {
                let a = bsa::Archive::open(path)?;
                let paths: Vec<String> = a
                    .paths()
                    .filter(|p| p.ends_with(".nif"))
                    .map(str::to_owned)
                    .collect();
                for p in paths {
                    let Ok(n) = nif::Nif::parse(&a.read(&p)?.unwrap()) else {
                        continue;
                    };
                    for b in &n.blocks {
                        let (shader, skin) = match b {
                            nif::Block::TriShape(t) => (t.shader, t.skin),
                            nif::Block::NiTriShape(g) | nif::Block::NiTriStrips(g) => {
                                (g.shader, g.skin)
                            }
                            _ => continue,
                        };
                        let Some(nif::Block::LightingShader(s)) = n.get(shader) else {
                            continue;
                        };
                        if s.flags1 & nif::sf1::MODEL_SPACE_NORMALS == 0 {
                            continue;
                        }
                        // Skinned shapes of models hung from a bone (`Prn`) or
                        // trees are drawn rigid too.
                        let root = n.roots.first().and_then(|&r| n.get(nif::Ref(r as i32)));
                        let prn = root.and_then(|b| b.av()).is_some_and(|av| {
                            av.net.extra_data.iter().any(|&e| matches!(n.get(e),
                                Some(nif::Block::ExtraData(nif::ExtraData::String { name, .. })) if name == "Prn"))
                        });
                        let tree = matches!(root, Some(nif::Block::Node(r)) if r.kind == nif::NodeKind::Tree);
                        if skin.is_none() || prn || tree {
                            rigid += 1;
                            *files.entry(p.clone()).or_default() += 1;
                        } else {
                            skinned += 1;
                        }
                    }
                }
            }
            println!("skinned {skinned} rigid {rigid} in {} files", files.len());
            for (f, k) in files {
                println!("{k:4} {f}");
            }
        }
        Some("nif-blocks") => {
            // nif-blocks <bsa>... -- <block type> [n]: the parsed blocks of a type
            // across every NIF in the archives (up to n, default 50), with their file.
            let sep = args.iter().position(|a| a == "--").context("-- <type>")?;
            let ty = &args[sep + 1];
            let max: usize = args.get(sep + 2).and_then(|n| n.parse().ok()).unwrap_or(50);
            let mut shown = 0;
            'files: for path in &args[1..sep] {
                let a = bsa::Archive::open(path)?;
                let mut paths: Vec<String> = a
                    .paths()
                    .filter(|p| p.ends_with(".nif"))
                    .map(str::to_owned)
                    .collect();
                paths.sort();
                for p in paths {
                    let Ok(n) = nif::Nif::parse(&a.read(&p)?.unwrap()) else {
                        continue;
                    };
                    for (i, b) in n.blocks.iter().enumerate() {
                        if n.block_type_name(i) == ty {
                            println!("{p} [{i}] {b:?}");
                            shown += 1;
                            if shown >= max {
                                break 'files;
                            }
                        }
                    }
                }
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
                let mut paths: Vec<String> = a
                    .paths()
                    .filter(|p| p.ends_with(".nif"))
                    .map(str::to_owned)
                    .collect();
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
                                    examples
                                        .entry(name)
                                        .or_insert_with(|| format!("{p} block {i} {st:?}"));
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
            println!(
                "{:<40} {:>8} {:>8} {:>8} {:>8}",
                "type", "exact", "skipped", "mismatch", "failed"
            );
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
            let fid = |r: &esp::LoadedRecord<'_>, d: &[u8]| {
                r.fid(esp::FormId(u32::from_le_bytes(d[..4].try_into().unwrap())))
            };
            let counts: Vec<usize> = rec
                .get(b"XCNT")
                .context("no XCNT")?
                .chunks_exact(4)
                .map(|c| u32::from_le_bytes(c.try_into().unwrap()) as usize)
                .collect();
            let steps: Vec<esp::FormId> = rec
                .get(b"DATA")
                .context("no DATA")?
                .chunks_exact(4)
                .map(|c| fid(&rec, c))
                .collect();
            // The counts run walk to swim; the footsteps are stored swim first.
            let mut next = steps.into_iter();
            for (g, n) in ["walk", "run", "sprint", "sneak", "swim"]
                .iter()
                .zip(counts)
                .rev()
            {
                println!("{g}:");
                for s in next.by_ref().take(n) {
                    let Some(st) = lo.get(s) else { continue };
                    let tag = st.get(b"ANAM").map(esp::decode_zstring).unwrap_or_default();
                    let ipds = st.get(b"DATA").map(|d| fid(&st, d)).unwrap_or_default();
                    let ipds_name = lo.get(ipds).and_then(|r| r.editor_id()).unwrap_or_default();
                    println!(
                        "  {s} {:<40} {tag:<16} {ipds} {ipds_name}",
                        st.editor_id().unwrap_or_default()
                    );
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
                for p in a
                    .paths()
                    .filter(|p| p.ends_with(".nif"))
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
                {
                    let Ok(n) = nif::Nif::parse(&a.read(&p)?.unwrap()) else {
                        continue;
                    };
                    let mut add = |m: u32, k: usize| {
                        let e = counts.entry(m).or_insert_with(|| (0, p.clone()));
                        e.0 += k;
                    };
                    for b in &n.blocks {
                        match b {
                            nif::Block::Shape(
                                nif::Shape::CompressedMeshData { materials, .. }
                                | nif::Shape::PackedTriStripsData { materials, .. },
                            ) => materials.iter().for_each(|&m| add(m, 1)),
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
        Some("nif-verts") => {
            // nif-verts <data dir> <vfs path>: each shape's vertices (position,
            // uv, colour).
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let v = vfs::Vfs::new(data, &names);
            let n = nif::Nif::parse(&v.read(&args[2]).context("not found")?)?;
            for b in &n.blocks {
                let g = match b {
                    nif::Block::TriShape(t) => &t.geometry,
                    _ => continue,
                };
                for (i, p) in g.positions.iter().enumerate() {
                    println!(
                        "{i:3} pos {:7.2} {:7.2} {:7.2} uv {:?} color {:?}",
                        p.x,
                        p.y,
                        p.z,
                        g.uvs.get(i),
                        g.colors.get(i)
                    );
                }
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
                        let (mut lo, mut hi) =
                            (glam::Vec3::splat(f32::MAX), glam::Vec3::splat(f32::MIN));
                        for p in &t.geometry.positions {
                            lo = lo.min(*p);
                            hi = hi.max(*p);
                        }
                        format!(
                            "TriShape name={} verts={} tris={} skin={:?} shader={:?} alpha={:?} desc={:016x} flags={:03x} xf={:?} bounds={lo:?}..{hi:?}",
                            t.av.net.name,
                            t.geometry.positions.len(),
                            t.geometry.triangles.len(),
                            t.skin,
                            t.shader,
                            t.alpha,
                            t.vertex_desc,
                            t.vertex_flags(),
                            t.av.transform.translation
                        )
                    }
                    nif::Block::SkinPartition(p) => format!(
                        "SkinPartition verts={} desc={:016x} partitions={:?}",
                        p.geometry.positions.len(),
                        p.vertex_desc,
                        p.partitions
                            .iter()
                            .map(|q| (
                                q.num_vertices,
                                q.bones.len(),
                                q.triangles.len(),
                                q.vertex_map.len(),
                                q.weights_per_vertex
                            ))
                            .collect::<Vec<_>>()
                    ),
                    nif::Block::SkinData(d) => format!(
                        "SkinData skin_xf={:?} bones={} first={:?}",
                        d.skin_transform.translation,
                        d.bones.len(),
                        d.bones.first().map(|b| b.transform.translation)
                    ),
                    nif::Block::TriShapeData(d) => {
                        let (mut lo, mut hi) =
                            (glam::Vec3::splat(f32::MAX), glam::Vec3::splat(f32::MIN));
                        for p in &d.geometry.positions {
                            lo = lo.min(*p);
                            hi = hi.max(*p);
                        }
                        format!(
                            "TriShapeData verts={} tris={} bounds={lo:?}..{hi:?}",
                            d.geometry.positions.len(),
                            d.geometry.triangles.len()
                        )
                    }
                    nif::Block::Shape(nif::Shape::CompressedMeshData {
                        vertices,
                        triangles,
                        ..
                    }) => {
                        format!(
                            "CompressedMeshData verts={} tris={}",
                            vertices.len(),
                            triangles.len()
                        )
                    }
                    other => format!("{other:?}"),
                };
                let s: String = s.chars().take(600).collect();
                println!("[{i}] {}: {s}", n.block_type_name(i));
            }
        }
        Some("nif-skin") => {
            // nif-skin <data dir> <vfs path>: skin partitions' bone lists and the vertex
            // bone indices they use.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let v = vfs::Vfs::new(data, &names);
            let n = nif::Nif::parse(&v.read(&args[2]).context("not found")?)?;
            for (i, b) in n.blocks.iter().enumerate() {
                let nif::Block::SkinPartition(p) = b else {
                    continue;
                };
                let g = &p.geometry;
                println!(
                    "[{i}] {} vertices, {} partitions",
                    g.positions.len(),
                    p.partitions.len()
                );
                for (k, part) in p.partitions.iter().enumerate() {
                    let mut used = std::collections::BTreeSet::new();
                    for t in &part.triangles {
                        for &vi in t {
                            if let Some(bi) = g.bone_indices.get(vi as usize) {
                                for (j, &x) in bi.iter().enumerate() {
                                    if g.bone_weights[vi as usize][j] > 0.0 {
                                        used.insert(x);
                                    }
                                }
                            }
                        }
                    }
                    println!(
                        "  {k}: {} bones {:?}, {} vertices, vertex map {}, indices used {:?}",
                        part.bones.len(),
                        part.bones,
                        part.num_vertices,
                        part.vertex_map.len(),
                        used
                    );
                }
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
                    let u = u32::from_le_bytes([
                        c[0],
                        *c.get(1).unwrap_or(&0),
                        *c.get(2).unwrap_or(&0),
                        *c.get(3).unwrap_or(&0),
                    ]);
                    println!(
                        "  +{:3}: {:08x} {:>14.6} {}",
                        j * 4,
                        u,
                        f32::from_bits(u),
                        u as i32
                    );
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
            println!(
                "{} objects: {:?}",
                p.version,
                p.objects
                    .iter()
                    .map(|o| o.class.as_str())
                    .collect::<Vec<_>>()
            );
            let c = havok::AnimationContainer::parse(&bytes)?;
            for s in &c.skeletons {
                println!("skeleton {} with {} bones", s.name, s.bones.len());
                let shown = if std::env::var_os("HKX_ALL").is_some() {
                    usize::MAX
                } else {
                    8
                };
                for (i, b) in s.bones.iter().enumerate().take(shown) {
                    println!(
                        "  [{i}] {} parent {:?} t {:?}",
                        b.name, b.parent, b.reference.translation
                    );
                }
            }
            let t: f32 = args.get(3).map(|s| s.parse()).transpose()?.unwrap_or(0.0);
            for a in &c.animations {
                println!(
                    "animation {:.3}s {} tracks, binding {:?}",
                    a.duration,
                    a.num_tracks,
                    a.binding
                        .as_ref()
                        .map(|b| (&b.skeleton_name, b.track_to_bone.len()))
                );
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
                    println!(
                        "  track {i}: t={:?} r={:?} |r|={:.4} s={:?}",
                        q.translation,
                        q.rotation,
                        q.rotation.length(),
                        q.scale
                    );
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
                let clips = g
                    .generators
                    .iter()
                    .filter(|g| matches!(g, havok::behavior::Generator::Clip { .. }))
                    .count();
                println!(
                    "{rel}: {} generators, {clips} clips, {} events",
                    g.generators.len(),
                    g.events.len()
                );
            }
            // `name*` lists the events starting with `name` (any case).
            let mut wanted: Vec<String> = Vec::new();
            for e in &args[3..] {
                match e.strip_suffix('*') {
                    Some(prefix) => {
                        let prefix = prefix.to_ascii_lowercase();
                        let mut names: Vec<String> = project
                            .graphs
                            .iter()
                            .flat_map(|(_, g)| g.events.iter())
                            .filter(|n| n.to_ascii_lowercase().starts_with(&prefix))
                            .cloned()
                            .collect();
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
                        let havok::behavior::Generator::StateMachine {
                            name,
                            states,
                            wildcards,
                            ..
                        } = node
                        else {
                            continue;
                        };
                        for (from, t) in
                            wildcards
                                .iter()
                                .map(|t| ("*", t))
                                .chain(states.iter().flat_map(|s| {
                                    s.transitions.iter().map(move |t| (s.name.as_str(), t))
                                }))
                        {
                            if t.event == id {
                                let to = states
                                    .iter()
                                    .find(|s| s.id == t.to_state)
                                    .map(|s| s.name.as_str())
                                    .unwrap_or("?");
                                println!(
                                    "  {gpath} sm#{gi} {name}: {from} -> {to} nested {:?} flags {:#x}",
                                    t.to_nested, t.flags
                                );
                            }
                        }
                    }
                }
                for pb in project.play_event(e) {
                    println!(
                        "  {:?}",
                        pb.clips
                            .iter()
                            .map(|c| format!("{} ({:?})", c.animation, c.mode))
                            .collect::<Vec<_>>()
                    );
                    for e in pb.events.iter().filter(|e| {
                        e.payload.is_some() || e.event.to_ascii_lowercase().contains("animobj")
                    }) {
                        println!(
                            "    clip {} @{:.3}{} {} {:?}",
                            e.clip,
                            e.time,
                            if e.from_end { " from end" } else { "" },
                            e.event,
                            e.payload
                        );
                    }
                    if let Some(exit) = project
                        .then_event(&pb, "IdleChairExitStart")
                        .or_else(|| project.then_event(&pb, "IdleStop"))
                    {
                        println!(
                            "    exit {:?}",
                            exit.clips
                                .iter()
                                .map(|c| c.animation.as_str())
                                .collect::<Vec<_>>()
                        );
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
            let bytes = v
                .read(&format!("{dir}/behaviors/{}", args[3]))
                .context("graph not found")?;
            let g = havok::behavior::BehaviorGraph::parse(&bytes)?;
            let max: usize = args.get(5).map(|s| s.parse()).transpose()?.unwrap_or(6);
            use havok::behavior::Generator as G;
            fn show(g: &havok::behavior::BehaviorGraph, id: usize, depth: usize, max: usize) {
                let pad = "  ".repeat(depth);
                match &g.generators[id] {
                    G::Clip {
                        name,
                        animation,
                        mode,
                        speed,
                        triggers,
                        ..
                    } => {
                        let t: Vec<String> = triggers
                            .iter()
                            .map(|t| {
                                format!(
                                    "{}@{:.2}{}",
                                    g.event_name(t.event).unwrap_or("?"),
                                    t.time,
                                    if t.from_end { "e" } else { "" }
                                )
                            })
                            .collect();
                        println!("{pad}clip {name} {animation} {mode:?} x{speed} {t:?}");
                    }
                    G::StateMachine {
                        name,
                        start,
                        start_variable,
                        start_mode,
                        states,
                        wildcards,
                    } => {
                        let var = start_variable.map(|v| {
                            format!(
                                " (bound to {} = {:?})",
                                g.variables.get(v).map_or("?", String::as_str),
                                g.variable_defaults.get(v)
                            )
                        });
                        let mode = match start_mode {
                            havok::behavior::StartMode::Default => String::new(),
                            havok::behavior::StartMode::Sync(v) => format!(
                                " synced with {}",
                                g.variables.get(*v).map_or("?", String::as_str)
                            ),
                            havok::behavior::StartMode::Random => " random".to_owned(),
                        };
                        println!(
                            "{pad}sm {name} start {start}{}{mode}",
                            var.unwrap_or_default()
                        );
                        let tr = |t: &havok::behavior::Transition| {
                            let mut s = format!(
                                "--{}--> {} nested {:?}",
                                g.event_name(t.event).unwrap_or("?"),
                                t.to_state,
                                t.to_nested
                            );
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
                    other @ (G::Blender { .. }
                    | G::Selector { .. }
                    | G::Wrap { .. }
                    | G::BoneSwitch { .. }) => {
                        let vars: Vec<String> = g.bindings[id]
                            .iter()
                            .map(|b| {
                                format!(
                                    "{}={}",
                                    b.member,
                                    g.variables.get(b.variable).map_or("?", String::as_str)
                                )
                            })
                            .collect();
                        let what = match other {
                            G::Blender {
                                name,
                                parameter,
                                flags,
                                children,
                                ..
                            } => {
                                let w: Vec<f32> = children.iter().map(|c| c.weight).collect();
                                format!(
                                    "blend {name} param {parameter} flags {flags:#x} weights {w:?}"
                                )
                            }
                            G::Selector { name, index, .. } => {
                                format!("select {name} index {index}")
                            }
                            G::Wrap {
                                name,
                                class,
                                modifier,
                                ..
                            } => {
                                let m = modifier
                                    .map(|m| format!(" modifier {:?}", g.modifiers[m]))
                                    .unwrap_or_default();
                                format!("{class} {name}{m}")
                            }
                            G::BoneSwitch { name, .. } => format!("boneswitch {name}"),
                            _ => unreachable!(),
                        };
                        println!(
                            "{pad}{what} {}",
                            if vars.is_empty() {
                                String::new()
                            } else {
                                format!("{vars:?}")
                            }
                        );
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
                    for st in states
                        .iter()
                        .filter(|s| s.name.to_ascii_lowercase() == want)
                    {
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
                    if filter
                        .as_ref()
                        .is_some_and(|f| !var.to_ascii_lowercase().contains(f.as_str()))
                    {
                        continue;
                    }
                    println!(
                        "{rel}: {var} ({:?}, default {})",
                        g.variable_types.get(vi),
                        g.variable_default(vi)
                    );
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
                            for l in lines
                                .iter()
                                .filter(|l| l.to_ascii_lowercase().contains(&lower))
                            {
                                println!("    expr {l}");
                            }
                        }
                    }
                    for node in &g.generators {
                        if let G::StateMachine {
                            name,
                            states,
                            wildcards,
                            ..
                        } = node
                        {
                            for t in states
                                .iter()
                                .flat_map(|s| s.transitions.iter())
                                .chain(wildcards)
                            {
                                if let Some(c) = t
                                    .condition
                                    .as_ref()
                                    .filter(|c| c.to_ascii_lowercase().contains(&lower))
                                {
                                    let to = states
                                        .iter()
                                        .find(|s| s.id == t.to_state)
                                        .map(|s| s.name.as_str())
                                        .unwrap_or("?");
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
                    if let havok::behavior::Generator::StateMachine {
                        states, wildcards, ..
                    } = node
                    {
                        let from = states
                            .iter()
                            .flat_map(|s| s.transitions.iter().map(move |t| (s.name.as_str(), t)));
                        for (from, t) in from.chain(wildcards.iter().map(|t| ("*", t))) {
                            total += 1;
                            if t.trigger.is_some() || t.initiate.is_some() || t.uninterruptible() {
                                let ev = |e: i32| g.event_name(e).unwrap_or("-");
                                let iv = |i: &havok::behavior::Interval| {
                                    format!(
                                        "[{} .. {} | {}s .. {}s]",
                                        ev(i.enter_event),
                                        ev(i.exit_event),
                                        i.enter_time,
                                        i.exit_time
                                    )
                                };
                                let to = states
                                    .iter()
                                    .find(|s| s.id == t.to_state)
                                    .map_or("?", |s| s.name.as_str());
                                println!(
                                    "{}: {from} -> {to} on {}{}{}{}",
                                    node.name(),
                                    ev(t.event),
                                    t.trigger
                                        .as_ref()
                                        .map(|i| format!(" trigger {}", iv(i)))
                                        .unwrap_or_default(),
                                    t.initiate
                                        .as_ref()
                                        .map(|i| format!(" initiate {}", iv(i)))
                                        .unwrap_or_default(),
                                    if t.uninterruptible() {
                                        format!(" uninterruptible (blend {:?})", t.blend)
                                    } else {
                                        String::new()
                                    },
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
                let Some(bytes) = v.read(&format!("{dir}/{}", rel.replace('\\', "/"))) else {
                    continue;
                };
                let p = havok::Packfile::parse(&bytes)?;
                for o in p.objects_of("hkbBlendingTransitionEffect") {
                    let key = format!(
                        "end mode {} start fraction {} flags {:#x} self mode {} event mode {}",
                        p.u8(o + 0x5A) as i8,
                        p.f32(o + 0x54),
                        p.u16(o + 0x58),
                        p.u8(o + 0x48) as i8,
                        p.u8(o + 0x49) as i8
                    );
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
                let bytes = v
                    .read(&format!("{dir}/{}", rel.replace('\\', "/")))
                    .context("graph vanished")?;
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
                let end = starts
                    .iter()
                    .copied()
                    .find(|&s| s > o.offset)
                    .unwrap_or(p.data.len() as u32);
                println!("{} @{:#x} ({} bytes)", o.class, o.offset, end - o.offset);
                let mut at = o.offset;
                while at < end {
                    let rel = at - o.offset;
                    if let Some(t) = p.ptr(at) {
                        let what = p.object_class(t).map(str::to_owned).unwrap_or_else(|| {
                            let s = p.string(at).unwrap_or_default();
                            if s.chars().all(|c| c.is_ascii_graphic() || c == ' ') && !s.is_empty()
                            {
                                format!("{s:?}")
                            } else {
                                format!("data @{t:#x}")
                            }
                        });
                        println!("  {rel:#05x} ptr -> {what}");
                        at += 8;
                        continue;
                    }
                    let w = p.u32(at);
                    println!(
                        "  {rel:#05x} {w:08x} {:>12} {:>14}",
                        p.i32(at),
                        format!("{:.4}", f32::from_bits(w))
                    );
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
                    let path = format!(
                        "{}/{}",
                        self.dir,
                        anim.to_ascii_lowercase().replace('\\', "/")
                    );
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
            let mut clips = ToolClips {
                v: &v,
                dir: dir.clone(),
                cache: Default::default(),
            };
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
                        *c += s.weight
                            * s.mask
                                .as_ref()
                                .map_or(1.0, |m| m.get(b).copied().unwrap_or(0.0));
                    }
                }
                let bare: Vec<usize> = cover
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| **c < 1e-3)
                    .map(|(b, _)| b)
                    .collect();
                if !bare.is_empty() {
                    println!("  uncovered bones {bare:?}");
                }
                for s in samples {
                    let mask = s.mask.as_ref().map(|m| {
                        let on: Vec<usize> = m
                            .iter()
                            .enumerate()
                            .filter(|(_, w)| **w > 0.0)
                            .map(|(i, _)| i)
                            .collect();
                        format!(
                            " mask {} of {} bones {:?}",
                            on.len(),
                            m.len(),
                            &on[..on.len().min(8)]
                        )
                    });
                    println!(
                        "  {:5.2}{} {} t={:.2}{}",
                        s.weight,
                        if s.additive { " +" } else { "" },
                        s.animation,
                        s.time,
                        mask.unwrap_or_default()
                    );
                }
                for r in inst.take_raised() {
                    println!(
                        "  raised {}{}",
                        r.event,
                        r.payload.map(|p| format!(" ({p})")).unwrap_or_default()
                    );
                }
                for l in inst.look_ats() {
                    let bones: Vec<String> = l
                        .bones
                        .iter()
                        .map(|b| {
                            format!(
                                "{}{} {:.0}deg",
                                b.index,
                                if b.enabled { "" } else { " off" },
                                b.limit_degrees
                            )
                        })
                        .collect();
                    println!(
                        "  look-at{} limit {:.0}deg bones {bones:?} eyes {}",
                        if l.look_at_target { " (tracking)" } else { "" },
                        l.limit_degrees,
                        l.eye_bones.len()
                    );
                }
                if let Some(g) = inst.foot_ik_controls() {
                    println!(
                        "  foot IK on/off {:.2} feedback {:.2} bias {:.2}",
                        g.on_off, g.world_from_model_feedback, g.error_up_down_bias
                    );
                }
            };
            for step in &args[3..] {
                if let Some(ev) = step.strip_prefix('!') {
                    let took = inst.handle_event(ev, &mut clips);
                    show(
                        &mut inst,
                        &format!("{ev} ({})", if took { "taken" } else { "ignored" }),
                    );
                } else if let Some((name, value)) = step.split_once('=') {
                    let ok = inst.set_variable(name, value.parse()?);
                    println!(
                        "== {name} = {value}{}",
                        if ok { "" } else { " (unknown variable)" }
                    );
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
            let events: std::collections::HashSet<String> = project
                .graphs
                .iter()
                .flat_map(|(_, g)| g.events.iter().map(|e| e.to_ascii_lowercase()))
                .collect();
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
                let Some(bytes) = v.read(&format!("{dir}/{a}")) else {
                    continue;
                };
                let Ok(c) = havok::AnimationContainer::parse(&bytes) else {
                    continue;
                };
                for an in c.animations.iter().flat_map(|x| &x.annotations) {
                    let key = an.text.split('.').next().unwrap_or("").to_owned();
                    *counts.entry(key).or_default() += 1;
                }
            }
            for (k, n) in counts {
                println!(
                    "{n:6} {k}{}",
                    if events.contains(&k.to_ascii_lowercase()) {
                        "  [event]"
                    } else {
                        ""
                    }
                );
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
                println!(
                    "duration {} tracks {} additive {:?} bound tracks {:?}",
                    a.duration,
                    a.num_tracks,
                    b.map(|b| b.additive),
                    b.map(|b| b.track_to_bone.len())
                );
            }
        }
        Some("hkb-clips") => {
            // hkb-clips <data dir> <project dir> <graph file>: every clip generator with mode and speed.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let v = vfs::Vfs::new(data, &names);
            let dir = args[2].trim_end_matches('/');
            let bytes = v
                .read(&format!("{dir}/behaviors/{}", args[3]))
                .context("graph not found")?;
            let g = havok::behavior::BehaviorGraph::parse(&bytes)?;
            for node in &g.generators {
                if let havok::behavior::Generator::Clip {
                    name,
                    animation,
                    mode,
                    speed,
                    ..
                } = node
                {
                    println!("{name}\t{animation}\t{mode:?}\t{speed}");
                }
            }
        }
        Some("idle-roots") => {
            // idle-roots <data dir>: IDLE records without a parent, with their graph and size.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let mut parent: std::collections::HashMap<esp::FormId, esp::FormId> =
                Default::default();
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
                let graph = rec
                    .get(b"DNAM")
                    .map(esp::decode_zstring)
                    .unwrap_or_default();
                println!(
                    "{id} {:5} {} {graph}",
                    n,
                    rec.editor_id().unwrap_or_default()
                );
            }
        }
        Some("idle-tree") => {
            // idle-tree <data dir> <IDLE editor id>: the idle and its descendants with raw conditions.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let mut children: std::collections::HashMap<esp::FormId, Vec<esp::FormId>> =
                Default::default();
            let mut previous: std::collections::HashMap<esp::FormId, esp::FormId> =
                Default::default();
            for &id in lo.ids_of_type(b"IDLE") {
                let Some(rec) = lo.get(id) else { continue };
                if let Some(d) = rec.get(b"ANAM").filter(|d| d.len() >= 8) {
                    let v = u32::from_le_bytes(d[0..4].try_into().unwrap());
                    let prev = u32::from_le_bytes(d[4..8].try_into().unwrap());
                    previous.insert(
                        id,
                        if prev == 0 {
                            esp::FormId::NULL
                        } else {
                            rec.fid(esp::FormId(prev))
                        },
                    );
                    if v != 0 {
                        children
                            .entry(rec.fid(esp::FormId(v)))
                            .or_default()
                            .push(id);
                    }
                }
            }
            // Authored order: follow previous-sibling links.
            for kids in children.values_mut() {
                let mut ordered = Vec::new();
                let mut cur = esp::FormId::NULL;
                while let Some(&n) = kids.iter().find(|k| {
                    previous.get(k).copied().unwrap_or_default() == cur && !ordered.contains(*k)
                }) {
                    ordered.push(n);
                    cur = n;
                }
                let rest: Vec<_> = kids
                    .iter()
                    .filter(|k| !ordered.contains(*k))
                    .copied()
                    .collect();
                ordered.extend(rest);
                *kids = ordered;
            }
            let root = lo.find_editor_id(&args[2]).context("editor id not found")?;
            let mut stack = vec![(root, 0usize)];
            while let Some((id, depth)) = stack.pop() {
                let Some(rec) = lo.get(id) else { continue };
                let text = |t: &[u8; 4]| rec.get(t).map(esp::decode_zstring).unwrap_or_default();
                let pad = "  ".repeat(depth);
                println!(
                    "{pad}{id} {} event={:?}",
                    rec.editor_id().unwrap_or_default(),
                    text(b"ENAM")
                );
                for c in rec
                    .subrecords()
                    .filter(|s| s.tag.0 == *b"CTDA" && s.data.len() >= 24)
                {
                    let d = c.data;
                    let f = u16::from_le_bytes([d[8], d[9]]);
                    let p1 = u32::from_le_bytes(d[12..16].try_into().unwrap());
                    let p1s = lo
                        .get(rec.fid(esp::FormId(p1)))
                        .and_then(|r| r.editor_id())
                        .unwrap_or_default();
                    let p2 = u32::from_le_bytes(d[16..20].try_into().unwrap());
                    let v = f32::from_le_bytes(d[4..8].try_into().unwrap());
                    println!(
                        "{pad}    ctda op {:#04x} func {f} p1 {p1:#x} {p1s} p2 {p2:#x} value {v} run {}",
                        d[0],
                        u32::from_le_bytes(d[20..24].try_into().unwrap())
                    );
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
                    if let havok::behavior::Generator::StateMachine {
                        name,
                        start,
                        start_variable: Some(v),
                        ..
                    } = node
                    {
                        println!(
                            "{rel} sm {name} start {start} bound to {} = {:?}",
                            g.variables.get(*v).map_or("?", String::as_str),
                            g.variable_defaults.get(*v)
                        );
                    }
                    if let havok::behavior::Generator::StateMachine { name, states, .. } = node {
                        for st in states {
                            for (when, e) in st
                                .enter_events
                                .iter()
                                .map(|e| ("enter", e))
                                .chain(st.exit_events.iter().map(|e| ("exit", e)))
                            {
                                println!(
                                    "{rel} sm {name} state {} {when} {} {:?}",
                                    st.name,
                                    g.event_name(e.event).unwrap_or("?"),
                                    e.payload.as_deref().unwrap_or("")
                                );
                            }
                        }
                    }
                    let havok::behavior::Generator::Clip {
                        name,
                        animation,
                        triggers,
                        ..
                    } = node
                    else {
                        continue;
                    };
                    for t in triggers.iter().filter(|t| t.payload.is_some()) {
                        let end = if t.from_end { " from end" } else { "" };
                        println!(
                            "{rel} {name} ({animation}) @{:.3}{end} {} {:?}",
                            t.time,
                            g.event_name(t.event).unwrap_or("?"),
                            t.payload.as_deref().unwrap_or("")
                        );
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
            let project = havok::behavior::Project::load("behaviors/0_master.hkx", |rel| {
                v.read(&format!("meshes/actors/character/{rel}"))
            });
            let (mut ok, mut bad) = (0, 0);
            for &m in lo.ids_of_type(b"IDLM") {
                let Some(rec) = lo.get(m) else { continue };
                let Some(list) = rec.get(b"IDLA") else {
                    continue;
                };
                for c in list.chunks_exact(4) {
                    let idle = rec.fid(esp::FormId(u32::from_le_bytes(c.try_into().unwrap())));
                    let Some(i) = lo.get(idle) else { continue };
                    let dnam = i
                        .get(b"DNAM")
                        .map(esp::decode_zstring)
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    if !dnam.is_empty() && !dnam.starts_with("actors\\character\\") {
                        continue;
                    }
                    let event = i.get(b"ENAM").map(esp::decode_zstring).unwrap_or_default();
                    let seqs = project.clips_for_event(&event);
                    if seqs.is_empty() {
                        bad += 1;
                        println!(
                            "unresolved {} {} event {event:?}",
                            rec.editor_id().unwrap_or_default(),
                            i.editor_id().unwrap_or_default()
                        );
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
                    let Some(pack) = lo.get(rec.fid(s.form_id(0))) else {
                        continue;
                    };
                    let Some(cu) = pack.get(b"PKCU").filter(|d| d.len() >= 8) else {
                        continue;
                    };
                    let t = pack.fid(esp::FormId(u32::from_le_bytes(
                        cu[4..8].try_into().unwrap(),
                    )));
                    let name = lo.get(t).and_then(|r| r.editor_id()).unwrap_or_default();
                    // SHOW=<template>: the NPCs and packages using it.
                    if std::env::var("SHOW").is_ok_and(|v| v.eq_ignore_ascii_case(&name)) {
                        println!(
                            "{npc} {} -> {} {}",
                            rec.editor_id().unwrap_or_default(),
                            pack.form_id(),
                            pack.editor_id().unwrap_or_default()
                        );
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
        Some("pack-tree") => {
            // pack-tree <data dir> <package or template>: a package template's
            // procedure tree (branches, their conditions, procedures and the inputs
            // they take) with its input names; for a package, its template's tree
            // and its own input values.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let id = match u32::from_str_radix(&args[2], 16) {
                Ok(v) if args[2].len() == 8 => esp::FormId(v),
                _ => lo.find_editor_id(&args[2]).context("editor id not found")?,
            };
            let rec = lo.get(id).context("record not found")?;
            let template = rec
                .get(b"PKCU")
                .filter(|d| d.len() >= 8)
                .map(|d| rec.fid(esp::FormId(u32::from_le_bytes(d[4..8].try_into().unwrap()))));
            let t = template.filter(|t| *t != id).and_then(|t| lo.get(t));
            let tree = t.as_ref().unwrap_or(&rec);
            println!(
                "template {} {}",
                tree.form_id(),
                tree.editor_id().unwrap_or_default()
            );
            // Input names follow the tree (UNAM index, BNAM name).
            let mut input_names: std::collections::BTreeMap<u8, String> = Default::default();
            let (mut in_tree, mut idx) = (false, None);
            for sr in tree.subrecords() {
                match &sr.tag.0 {
                    b"XNAM" => in_tree = true,
                    b"UNAM" if in_tree && !sr.data.is_empty() => idx = Some(sr.data[0]),
                    b"BNAM" if in_tree => {
                        if let Some(i) = idx.take() {
                            input_names.insert(i, sr.zstring());
                        }
                    }
                    _ => {}
                }
            }
            // Input types before the tree, in UNAM order.
            let (mut kinds, mut indices) = (Vec::new(), Vec::new());
            for sr in tree.subrecords() {
                match &sr.tag.0 {
                    b"XNAM" => break,
                    b"ANAM" => kinds.push(sr.zstring()),
                    b"UNAM" if !sr.data.is_empty() => indices.push(sr.data[0]),
                    _ => {}
                }
            }
            for (i, k) in indices.iter().zip(&kinds) {
                println!(
                    "  input {i:3} {k:14} {}",
                    input_names.get(i).map_or("", String::as_str)
                );
            }
            const OPS: [&str; 6] = ["==", "!=", ">", ">=", "<", "<="];
            let mut in_tree = false;
            for sr in tree.subrecords() {
                match &sr.tag.0 {
                    b"XNAM" => in_tree = true,
                    _ if !in_tree => {}
                    // The input names follow the tree.
                    b"UNAM" => break,
                    b"ANAM" => println!("  {}", sr.zstring()),
                    b"PRCB" if sr.data.len() >= 8 => {
                        println!("      children {} flags {:08X}", sr.u32(0), sr.u32(4))
                    }
                    b"PNAM" => println!("      procedure {}", sr.zstring()),
                    b"PKC2" if !sr.data.is_empty() => {
                        let i = sr.data[0];
                        println!(
                            "        input {i} {}",
                            input_names.get(&i).map_or("", String::as_str)
                        );
                    }
                    b"CTDA" if sr.data.len() >= 24 => {
                        let d = sr.data;
                        let f = u16::from_le_bytes([d[8], d[9]]);
                        let value = if d[0] & 0x04 != 0 {
                            "global".to_owned()
                        } else {
                            format!("{}", f32::from_le_bytes(d[4..8].try_into().unwrap()))
                        };
                        println!(
                            "      if {}({:08X}, {:08X}) run_on {} ref {:X} {} {value}{}",
                            functions::name(f),
                            sr.u32(12),
                            sr.u32(16),
                            sr.u32(20),
                            if d.len() >= 28 { sr.u32(24) } else { 0 },
                            OPS.get((d[0] >> 5) as usize).unwrap_or(&"?"),
                            if d[0] & 1 != 0 { " OR" } else { "" }
                        );
                    }
                    _ => {}
                }
            }
        }
        Some("pack-lists") => {
            // pack-lists <data dir>: NPCs' default package lists (DPLT) and override
            // package lists (SPOR spectator, OCOR observe corpse, GWOR guard warn, ECOR
            // combat): which form lists, how many NPCs name each, and what is in them.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            for tag in [b"DPLT", b"SPOR", b"OCOR", b"GWOR", b"ECOR"] {
                let mut lists: std::collections::BTreeMap<esp::FormId, (usize, usize)> =
                    Default::default();
                for &npc in lo.ids_of_type(b"NPC_") {
                    let Some(rec) = lo.get(npc) else { continue };
                    let Some(d) = rec.get(tag).filter(|d| d.len() >= 4) else {
                        continue;
                    };
                    let l = rec.fid(esp::FormId(u32::from_le_bytes(d[0..4].try_into().unwrap())));
                    let own = rec.subrecords().filter(|s| s.tag.0 == *b"PKID").count();
                    let e = lists.entry(l).or_default();
                    e.0 += 1;
                    e.1 += (own == 0) as usize;
                }
                println!("{}: {} lists", String::from_utf8_lossy(tag), lists.len());
                let mut v: Vec<_> = lists.into_iter().collect();
                v.sort_by(|a, b| b.1.0.cmp(&a.1.0));
                for (l, (n, no_own)) in v.iter().take(12) {
                    let Some(r) = lo.get(*l) else { continue };
                    let packs: Vec<String> = r
                        .subrecords()
                        .filter(|s| s.tag.0 == *b"LNAM")
                        .map(|s| {
                            lo.get(r.fid(s.form_id(0)))
                                .and_then(|p| p.editor_id())
                                .unwrap_or_default()
                        })
                        .collect();
                    println!(
                        "  {n:6} NPCs ({no_own} with no packages of their own) {l} {}: {packs:?}",
                        r.editor_id().unwrap_or_default()
                    );
                }
            }
        }
        Some("force-greets") => {
            // force-greets <data dir>: ForceGreet / ForceGreetFromSitting packages with their
            // topic, trigger location, force greet distance, "must be detected" and "sandbox
            // while waiting" inputs, the NPCs using them and where those are placed.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let mut users: std::collections::HashMap<esp::FormId, Vec<esp::FormId>> =
                Default::default();
            for &npc in lo.ids_of_type(b"NPC_") {
                let Some(rec) = lo.get(npc) else { continue };
                for s in rec.subrecords().filter(|s| s.tag.0 == *b"PKID") {
                    users.entry(rec.fid(s.form_id(0))).or_default().push(npc);
                }
            }
            let mut placed: std::collections::HashMap<esp::FormId, Vec<esp::FormId>> =
                Default::default();
            for &r in lo.ids_of_type(b"ACHR") {
                let Some(rec) = lo.get(r) else { continue };
                let Some(b) = rec.get(b"NAME").filter(|d| d.len() >= 4) else {
                    continue;
                };
                placed
                    .entry(rec.fid(esp::FormId(u32::from_le_bytes(b[0..4].try_into().unwrap()))))
                    .or_default()
                    .push(r);
            }
            for &id in lo.ids_of_type(b"PACK") {
                let Some(rec) = lo.get(id) else { continue };
                let Some(cu) = rec.get(b"PKCU").filter(|d| d.len() >= 8) else {
                    continue;
                };
                let t = rec.fid(esp::FormId(u32::from_le_bytes(
                    cu[4..8].try_into().unwrap(),
                )));
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
                        b"CNAM" if !d.is_empty() => {
                            *values.last_mut().unwrap() = format!("{}", d[0] != 0)
                        }
                        b"PDTO" if d.len() >= 8 && !values.is_empty() => {
                            let v = u32::from_le_bytes(d[4..8].try_into().unwrap());
                            *values.last_mut().unwrap() = if d[0] == 0 {
                                let f = rec.fid(esp::FormId(v));
                                format!(
                                    "{f} {}",
                                    lo.get(f).and_then(|r| r.editor_id()).unwrap_or_default()
                                )
                            } else {
                                String::from_utf8_lossy(&d[4..8]).into_owned()
                            };
                        }
                        b"PLDT" if d.len() >= 12 && !values.is_empty() => {
                            let k = u32::from_le_bytes(d[0..4].try_into().unwrap());
                            let v = rec
                                .fid(esp::FormId(u32::from_le_bytes(d[4..8].try_into().unwrap())));
                            *values.last_mut().unwrap() = format!(
                                "kind {k} {v} r {}",
                                i32::from_le_bytes(d[8..12].try_into().unwrap())
                            );
                        }
                        b"UNAM" if !d.is_empty() => indices.push(d[0]),
                        _ => {}
                    }
                }
                let input = |i: u8| {
                    indices
                        .iter()
                        .position(|&x| x == i)
                        .and_then(|p| values.get(p).cloned())
                        .unwrap_or_else(|| "-".into())
                };
                println!(
                    "{id} {} ({template}): topic {} | trigger {} | distance {} | detect {} | sandbox {}",
                    rec.editor_id().unwrap_or_default(),
                    input(0x07),
                    input(0x3e),
                    input(0x4b),
                    if input(0x4f) == "-" {
                        input(0x4d)
                    } else {
                        input(0x4f)
                    },
                    input(0x28)
                );
                for npc in users.get(&id).into_iter().flatten() {
                    let edid = lo.get(*npc).and_then(|r| r.editor_id()).unwrap_or_default();
                    let refs: Vec<String> = placed
                        .get(npc)
                        .into_iter()
                        .flatten()
                        .map(|r| {
                            let cell = lo
                                .cell_of_ref(*r)
                                .and_then(|c| lo.get(c).and_then(|c| c.editor_id()))
                                .unwrap_or_default();
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
                                .and_then(|p| {
                                    p.get(b"PKCU").filter(|d| d.len() >= 8).map(|cu| {
                                        p.fid(esp::FormId(u32::from_le_bytes(
                                            cu[4..8].try_into().unwrap(),
                                        )))
                                    })
                                })
                                .and_then(|t| lo.get(t).and_then(|t| t.editor_id()))
                                .unwrap_or_default();
                            if std::env::var("SHOW").is_ok_and(|v| v.eq_ignore_ascii_case(&t)) {
                                println!(
                                    "{q} {} -> {pack} {}",
                                    rec.editor_id().unwrap_or_default(),
                                    lo.get(pack).and_then(|p| p.editor_id()).unwrap_or_default()
                                );
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
            let (mut pldt, mut ptda): (
                std::collections::BTreeMap<u32, usize>,
                std::collections::BTreeMap<u32, usize>,
            ) = Default::default();
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
        Some("alias-fills") => {
            // alias-fills <data dir>: reference and location aliases by fill type, for all
            // quests, start-game-enabled ones, and aliases with packages (ALPC); FNAM flags.
            // SHOW=<fill type> lists the aliases of that type.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let show = std::env::var("SHOW").ok();
            // fill type -> (all, start game enabled, with packages)
            let mut kinds: std::collections::BTreeMap<String, [usize; 3]> = Default::default();
            let mut flags: std::collections::BTreeMap<u32, usize> = Default::default();
            let mut funcs: std::collections::BTreeMap<String, usize> = Default::default();
            for &q in lo.ids_of_type(b"QUST") {
                let Some(rec) = lo.get(q) else { continue };
                let sge = rec.get(b"DNAM").is_some_and(|d| d[0] & 1 != 0);
                let mut cur: Option<(bool, String, Vec<&'static str>, bool, u32)> = None;
                for sr in rec.subrecords() {
                    let tag = &sr.tag.0;
                    match tag {
                        b"ALST" | b"ALLS" => {
                            cur = Some((tag == b"ALLS", String::new(), Vec::new(), false, 0))
                        }
                        _ => {}
                    }
                    let Some(c) = cur.as_mut() else { continue };
                    match tag {
                        b"ALID" => c.1 = sr.zstring(),
                        b"FNAM" => c.4 = sr.u32(0),
                        b"ALFR" => c.2.push("forced"),
                        b"ALUA" => c.2.push("unique"),
                        b"ALFL" => c.2.push("specific location"),
                        b"ALFA" => c.2.push("from alias"),
                        b"ALRT" => c.2.push("ref type"),
                        b"ALEQ" => c.2.push("external"),
                        b"ALCO" => {
                            c.2.push("create");
                            let obj = rec.fid(sr.form_id(0));
                            let tag = lo.tag_of(obj).map(|t| t.to_string()).unwrap_or_default();
                            *funcs.entry(format!("create {tag}")).or_default() += 1;
                        }
                        b"ALCA" => {
                            let v = sr.u32(0);
                            *funcs
                                .entry(format!(
                                    "create {} level {}",
                                    if v & 0x8000_0000 != 0 { "in" } else { "at" },
                                    (v >> 16) & 0x7fff
                                ))
                                .or_default() += 1;
                        }
                        b"ALNA" => c.2.push("near alias"),
                        b"ALNT" => {
                            *funcs
                                .entry(format!("near alias type {}", sr.u32(0)))
                                .or_default() += 1
                        }
                        b"ALFE" => c.2.push("from event"),
                        b"CTDA" => {
                            if !c.2.contains(&"conditions") {
                                c.2.push("conditions");
                            }
                            let d = sr.data;
                            let f = u16::from_le_bytes([d[8], d[9]]);
                            let run_on = u32::from_le_bytes(d[20..24].try_into().unwrap());
                            *funcs
                                .entry(format!(
                                    "{} {} run_on {run_on}",
                                    if c.0 { "loc" } else { "ref" },
                                    functions::name(f)
                                ))
                                .or_default() += 1;
                        }
                        b"ALPC" => c.3 = true,
                        b"ALED" => {
                            let (loc, name, k, packs, fl) = cur.take().unwrap();
                            let mut k = k;
                            k.dedup();
                            let key = format!(
                                "{} {}",
                                if loc { "loc" } else { "ref" },
                                if k.is_empty() {
                                    "-".into()
                                } else {
                                    k.join("+")
                                }
                            );
                            let e = kinds.entry(key.clone()).or_default();
                            e[0] += 1;
                            e[1] += sge as usize;
                            e[2] += packs as usize;
                            for b in 0..32 {
                                if fl & (1 << b) != 0 {
                                    *flags.entry(1 << b).or_default() += 1;
                                }
                            }
                            if show.as_deref() == Some(key.as_str()) {
                                println!(
                                    "{q} {} {name} flags={fl:x}{}{}",
                                    rec.editor_id().unwrap_or_default(),
                                    if sge { " SGE" } else { "" },
                                    if packs { " ALPC" } else { "" }
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }
            println!("{:>6} {:>6} {:>6}  fill", "all", "sge", "alpc");
            for (k, [a, s, p]) in &kinds {
                println!("{a:6} {s:6} {p:6}  {k}");
            }
            println!("flags {flags:x?}");
            let mut v: Vec<_> = funcs.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1));
            for (n, c) in v.iter().take(60) {
                println!("{c:6} {n}");
            }
        }
        Some("addons") => {
            // addons <data dir> [nif]: addon nodes (ADDN: index, model, flags)
            // and the meshes' BSValueNodes naming them (how many models,
            // values without an addon); with a model, its value nodes.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let v = vfs::Vfs::new(data, &names);
            let mut by_index = std::collections::BTreeMap::new();
            for &id in lo.ids_of_type(b"ADDN") {
                let Some(rec) = lo.get(id) else { continue };
                let index = rec
                    .get(b"DATA")
                    .and_then(|d| d.get(..4))
                    .map(|d| i32::from_le_bytes(d.try_into().unwrap()));
                let model = rec.get(b"MODL").map(esp::decode_zstring);
                let dnam = rec.get(b"DNAM").map(|d| d.to_vec());
                println!(
                    "{id} {:<32} index {index:?} model {model:?} DNAM {dnam:02x?}",
                    rec.editor_id().unwrap_or_default()
                );
                if let Some(i) = index {
                    by_index.insert(i, rec.editor_id().unwrap_or_default().to_owned());
                }
            }
            let mut paths: Vec<String> = match args.get(2) {
                Some(p) => vec![p.clone()],
                None => v
                    .list("meshes/")
                    .into_iter()
                    .filter(|p| p.ends_with(".nif"))
                    .collect(),
            };
            paths.sort();
            let (mut users, mut missing) = (0, std::collections::BTreeMap::new());
            for p in &paths {
                let Some(n) = v.read(p).and_then(|b| nif::Nif::parse(&b).ok()) else {
                    continue;
                };
                let mut found = Vec::new();
                for b in &n.blocks {
                    if let nif::Block::Node(node) = b
                        && let nif::NodeKind::Value { value, flags } = node.kind
                    {
                        found.push(value);
                        if args.get(2).is_some() {
                            println!(
                                "{:<24} value {value} flags {flags:02x} -> {:?}",
                                node.av.net.name,
                                by_index.get(&value)
                            );
                        }
                        if !by_index.contains_key(&value) {
                            *missing.entry(value).or_insert(0) += 1;
                        }
                    }
                }
                if !found.is_empty() {
                    users += 1;
                    if args.get(2).is_none() {
                        println!("{p}: {found:?}");
                    }
                }
            }
            println!("{users} models with value nodes; values without an addon: {missing:?}");
        }
        Some("impacts") => {
            // impacts <data dir>: each impact (IPCT) with its model and the
            // model's block types (how many of each), and how many impacts use
            // each block type.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let v = vfs::Vfs::new(data, &names);
            let mut totals = std::collections::BTreeMap::<String, usize>::new();
            for &id in lo.ids_of_type(b"IPCT") {
                let Some(rec) = lo.get(id) else { continue };
                let model = rec.get(b"MODL").map(esp::decode_zstring);
                // DATA: duration, orientation (0 surface normal, 1 projectile
                // vector, 2 projectile reflection).
                let (duration, orientation) = rec
                    .get(b"DATA")
                    .filter(|d| d.len() >= 8)
                    .map(|d| {
                        (
                            f32::from_le_bytes(d[0..4].try_into().unwrap()),
                            u32::from_le_bytes(d[4..8].try_into().unwrap()),
                        )
                    })
                    .unwrap_or_default();
                print!(
                    "{id} {:<40} {duration:.2}s orientation {orientation} {model:?}",
                    rec.editor_id().unwrap_or_default()
                );
                let Some(m) = model.filter(|m| !m.is_empty()) else {
                    println!();
                    continue;
                };
                let path = format!("meshes/{}", m.to_ascii_lowercase().replace('\\', "/"));
                let Some(n) = v.read(&path).and_then(|b| nif::Nif::parse(&b).ok()) else {
                    println!(" (missing)");
                    continue;
                };
                let mut counts = std::collections::BTreeMap::<String, usize>::new();
                for i in 0..n.blocks.len() {
                    *counts.entry(n.block_type_name(i).to_owned()).or_default() += 1;
                }
                for t in counts.keys() {
                    *totals.entry(t.clone()).or_default() += 1;
                }
                println!(" {counts:?}");
                // The blocks with controllers of their own, and those controllers.
                for (i, b) in n.blocks.iter().enumerate() {
                    let Some(av) = b.av() else { continue };
                    let mut c = av.net.controller;
                    while let Some(nif::Block::NodeController(nc)) = n.get(c) {
                        println!(
                            "    {} {:?}: {:?} flags {:#x} {}..{}",
                            n.block_type_name(i),
                            av.net.name,
                            nc.kind,
                            nc.timing.flags,
                            nc.timing.start,
                            nc.timing.stop
                        );
                        c = nc.next;
                    }
                }
            }
            println!("impacts using each block type: {totals:#?}");
        }
        Some("decals") => {
            // decals <data dir> [texture set]: texture sets with decal data
            // (DODT) and the references placing them: how many, the subrecords
            // those carry, a few samples (or every one of the named texture
            // set) with their cell, position and scale.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let mut with_dodt = 0;
            for &id in lo.ids_of_type(b"TXST") {
                let Some(rec) = lo.get(id) else { continue };
                if let Some(d) = rec.get(b"DODT") {
                    with_dodt += 1;
                    if with_dodt <= 6
                        || args.get(2).is_some_and(|w| {
                            rec.editor_id().is_some_and(|e| e.eq_ignore_ascii_case(w))
                        })
                    {
                        let f = |i: usize| f32::from_le_bytes(d[i..i + 4].try_into().unwrap());
                        println!(
                            "{id} {:<28} DODT {} bytes: min w {} max w {} min h {} max h {} depth {} shin {} par scale {} passes {} flags {:02x} color {:?} tx00 {:?} DNAM {:?}",
                            rec.editor_id().unwrap_or_default(),
                            d.len(),
                            f(0),
                            f(4),
                            f(8),
                            f(12),
                            f(16),
                            f(20),
                            f(24),
                            d[28],
                            d[29],
                            &d[32..36],
                            rec.get(b"TX00").map(esp::decode_zstring),
                            rec.get(b"DNAM"),
                        );
                    }
                }
            }
            println!("{with_dodt} texture sets with DODT");
            let mut n = 0;
            let mut ground = (0, 0);
            let mut tags = std::collections::BTreeMap::<String, usize>::new();
            let mut bases = std::collections::BTreeMap::<String, usize>::new();
            for &r in lo.ids_of_type(b"REFR") {
                let Some(rec) = lo.get(r) else { continue };
                let Some(name) = rec.get(b"NAME") else {
                    continue;
                };
                let base = rec.fid(esp::FormId(u32::from_le_bytes(
                    name[..4].try_into().unwrap(),
                )));
                let Some(b) = lo.get(base) else { continue };
                if b.tag().0 != *b"TXST" {
                    continue;
                }
                n += 1;
                let edid = b.editor_id().unwrap_or_default();
                *bases.entry(edid.clone()).or_default() += 1;
                // Which way the reference's +Y points, for ground decals.
                if edid.contains("Ground") || edid.contains("FlameBurn") {
                    let d = rec.get(b"DATA").unwrap_or_default();
                    if d.len() >= 24 {
                        let f = |i: usize| f32::from_le_bytes(d[i..i + 4].try_into().unwrap());
                        let q = glam::Quat::from_euler(glam::EulerRot::XYZ, -f(12), -f(16), -f(20));
                        let y = q * glam::Vec3::Y;
                        ground.0 += 1;
                        if y.z < -0.5 {
                            ground.1 += 1;
                        }
                    }
                }
                for sr in rec.subrecords() {
                    *tags.entry(sr.tag.to_string()).or_default() += 1;
                }
                let wanted = args.get(2).is_none_or(|w| edid.eq_ignore_ascii_case(w));
                if wanted && (n <= 6 || args.get(2).is_some()) {
                    let data = rec.get(b"DATA").unwrap_or_default();
                    let fl: Vec<f32> = data
                        .chunks_exact(4)
                        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
                        .collect();
                    println!(
                        "{r} {} in {:?} DATA {fl:?} XSCL {:?}",
                        b.editor_id().unwrap_or_default(),
                        lo.cell_of_ref(r),
                        rec.get(b"XSCL")
                            .map(|x| f32::from_le_bytes(x[..4].try_into().unwrap())),
                    );
                }
            }
            println!("{n} references to texture sets; subrecords {tags:?}");
            println!(
                "ground / burn decals: {} of {} have +Y pointing down",
                ground.1, ground.0
            );
            let mut v: Vec<_> = bases.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1));
            for (k, c) in v.iter().take(15) {
                println!("{c:6} {k}");
            }
        }
        Some("ref-types") => {
            // ref-types <data dir> [ref type]: references carrying location ref types
            // (XLRT), by type, with how long reading every reference takes.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let want = args.get(2).and_then(|a| lo.find_editor_id(a));
            let t = std::time::Instant::now();
            let (mut n, mut with) = (0, 0);
            let mut types: std::collections::BTreeMap<String, usize> = Default::default();
            for tag in [b"REFR", b"ACHR"] {
                for &r in lo.ids_of_type(tag) {
                    let Some(rec) = lo.get(r) else { continue };
                    n += 1;
                    for sr in rec.subrecords().filter(|sr| sr.tag.0 == *b"XLRT") {
                        with += 1;
                        let ty = rec.fid(sr.form_id(0));
                        if want == Some(ty) {
                            println!("{r} {} in {:?}", rec.tag(), lo.cell_of_ref(r));
                        }
                        *types
                            .entry(lo.get(ty).and_then(|t| t.editor_id()).unwrap_or_default())
                            .or_default() += 1;
                    }
                }
            }
            println!(
                "{n} references, {with} ref types, read in {:?}",
                t.elapsed()
            );
            let mut v: Vec<_> = types.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1));
            for (k, c) in v.iter().take(25) {
                println!("{c:6} {k}");
            }
        }
        Some("npc-templates") => {
            // npc-templates <data dir>: NPCs with a template (TPLT), by what it is (NPC_ or a
            // leveled list), how often each "Use ..." flag (ACBS) is set, and ACHRs placing
            // templated NPCs through a leveled template.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            const FLAGS: [&str; 13] = [
                "traits",
                "stats",
                "factions",
                "spells",
                "ai data",
                "ai packages",
                "model/anim (unused)",
                "base data",
                "inventory",
                "script",
                "def pack list",
                "attack data",
                "keywords",
            ];
            let (mut to_npc, mut to_lvln, mut flags) = (0, 0, [0usize; 13]);
            let mut lvln_flags = [0usize; 13];
            for &npc in lo.ids_of_type(b"NPC_") {
                let Some(rec) = lo.get(npc) else { continue };
                let Some(t) = rec.get(b"TPLT").filter(|d| d.len() >= 4) else {
                    continue;
                };
                let t = rec.fid(esp::FormId(u32::from_le_bytes(t[0..4].try_into().unwrap())));
                let Some(tr) = lo.get(t) else { continue };
                let lvln = tr.tag().0 == *b"LVLN";
                if lvln {
                    to_lvln += 1
                } else {
                    to_npc += 1
                }
                let f = rec
                    .get(b"ACBS")
                    .filter(|d| d.len() >= 20)
                    .map_or(0, |d| u16::from_le_bytes([d[18], d[19]]));
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
            let has = |tag: &[u8; 4]| {
                lo.ids_of_type(b"NPC_")
                    .iter()
                    .filter(|&&n| lo.get(n).is_some_and(|r| r.get(tag).is_some()))
                    .count()
            };
            println!(
                "NPCs with an attack race (ATKR) {}, own attacks (ATKD) {}",
                has(b"ATKR"),
                has(b"ATKD")
            );
            for (i, n) in FLAGS.iter().enumerate() {
                println!(
                    "  0x{:04X} use {n:22} {:6} ({} through a leveled list)",
                    1 << i,
                    flags[i],
                    lvln_flags[i]
                );
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
                let Some(d) = rec.get(b"PKDT").filter(|d| d.len() >= 8) else {
                    continue;
                };
                let flags = u32::from_le_bytes(d[0..4].try_into().unwrap());
                let key = format!(
                    "type {:2} preferred {} speed {} sneak {}",
                    d[4],
                    flags >> 13 & 1,
                    d[6],
                    flags >> 17 & 1
                );
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
            let mut by: std::collections::BTreeMap<String, (usize, Vec<String>)> =
                Default::default();
            for &id in lo.ids_of_type(b"DIAL") {
                let Some(rec) = lo.get(id) else { continue };
                let sub = rec
                    .get(b"SNAM")
                    .and_then(|d| d.get(0..4))
                    .map(|d| String::from_utf8_lossy(d).into_owned())
                    .unwrap_or_default();
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
            let mut counts: BTreeMap<(String, u8, u8, bool), (usize, esp::FormId)> =
                BTreeMap::new();
            for &c in lo.ids_of_type(b"CELL") {
                let Some(idx) = lo.cell(c) else { continue };
                for &r in idx.persistent.iter().chain(&idx.temporary) {
                    let Some(rec) = lo.get(r) else { continue };
                    let Some(x) = rec.get(b"XLOC").filter(|d| d.len() >= 9) else {
                        continue;
                    };
                    let base = rec.get(b"NAME").filter(|d| d.len() >= 4).map(|b| {
                        rec.fid(esp::FormId(u32::from_le_bytes(b[0..4].try_into().unwrap())))
                    });
                    let tag = base
                        .and_then(|b| lo.tag_of(b))
                        .map(|t| t.to_string())
                        .unwrap_or_default();
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
                rec.get(tag)
                    .filter(|d| d.len() >= 4)
                    .map(|d| rec.fid(esp::FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
                    .filter(|f| !f.is_null())
            };
            let owner = |r: esp::FormId| {
                let rec = lo.get(r)?;
                fid(&rec, b"XOWN").map(|o| ("door", o)).or_else(|| {
                    lo.get(lo.cell_of_ref(r)?)
                        .and_then(|c| fid(&c, b"XOWN"))
                        .map(|o| ("cell", o))
                })
            };
            let mut owned: BTreeMap<String, (usize, esp::FormId)> = BTreeMap::new();
            for &c in lo.ids_of_type(b"CELL") {
                let Some(idx) = lo.cell(c) else { continue };
                for &r in idx.persistent.iter().chain(&idx.temporary) {
                    let Some(rec) = lo.get(r) else { continue };
                    if rec.get(b"XLOC").is_none() {
                        continue;
                    }
                    let Some(partner) = fid(&rec, b"XTEL") else {
                        continue;
                    };
                    let near = owner(r);
                    let far = owner(partner);
                    let desc = |o: Option<(&str, esp::FormId)>| match o {
                        Some((at, f)) => format!(
                            "{at} {}",
                            lo.tag_of(f).map(|t| t.to_string()).unwrap_or_default()
                        ),
                        None => "none".into(),
                    };
                    let e = owned
                        .entry(format!("this side {} / far side {}", desc(near), desc(far)))
                        .or_insert((0, r));
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
                let actors = idx
                    .persistent
                    .iter()
                    .filter(|&&r| lo.tag_of(r).is_some_and(|t| t.0 == *b"ACHR"))
                    .count();
                if actors == 0 {
                    continue;
                }
                let doors: Vec<String> = idx
                    .persistent
                    .iter()
                    .chain(&idx.temporary)
                    .filter(|&&r| {
                        lo.get(r).is_some_and(|rec| {
                            rec.get(b"XLOC").is_some() && rec.get(b"XTEL").is_none()
                        })
                    })
                    .filter(|&&r| {
                        lo.get(r)
                            .and_then(|rec| fid(&rec, b"NAME"))
                            .and_then(|b| lo.tag_of(b))
                            .is_some_and(|t| t.0 == *b"DOOR")
                    })
                    .map(|&r| format!("{r} (owner {:?})", owner(r).map(|o| o.1)))
                    .collect();
                if !doors.is_empty() {
                    let name = lo.get(c).and_then(|r| r.editor_id()).unwrap_or_default();
                    println!(
                        "{name} ({c}), {actors} actors: locked doors {}",
                        doors.join(", ")
                    );
                }
            }
        }
        Some("cell-refs") => {
            // cell-refs <data dir> <cell editor id | hex | [world:]x,y> [base tag]: a cell's
            // references with their base records, positions and rotations (degrees).
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let cell =
                if let Some((x, y)) = args[2].rsplit(':').next().and_then(|g| g.split_once(',')) {
                    let world = args[2].split_once(':').map_or("Tamriel", |(w, _)| w);
                    let w = lo.find_editor_id(world).context("world not found")?;
                    let g = (x.trim().parse::<i32>()?, y.trim().parse::<i32>()?);
                    *lo.world(w)
                        .and_then(|w| w.cells.get(&g))
                        .context("no cell there")?
                } else {
                    match u32::from_str_radix(&args[2], 16) {
                        Ok(v) if args[2].len() == 8 => esp::FormId(v),
                        _ => lo.find_editor_id(&args[2]).context("cell not found")?,
                    }
                };
            let idx = lo.cell(cell).context("not a cell")?;
            for &r in idx.persistent.iter().chain(&idx.temporary) {
                let Some(rec) = lo.get(r) else { continue };
                let Some(b) = rec.get(b"NAME").filter(|d| d.len() >= 4) else {
                    continue;
                };
                let base = rec.fid(esp::FormId(u32::from_le_bytes(b[0..4].try_into().unwrap())));
                let tag = lo.tag_of(base).map(|t| t.to_string()).unwrap_or_default();
                if args
                    .get(3)
                    .is_some_and(|want| !want.eq_ignore_ascii_case(&tag))
                {
                    continue;
                }
                let edid = lo
                    .get(base)
                    .and_then(|b| b.editor_id().map(|e| e.to_string()))
                    .unwrap_or_default();
                let placement = rec
                    .get(b"DATA")
                    .filter(|d| d.len() >= 24)
                    .map(|d| {
                        let f =
                            |i: usize| f32::from_le_bytes(d[i * 4..i * 4 + 4].try_into().unwrap());
                        format!(
                            " @ {:.0},{:.0},{:.0} rot {:.1},{:.1},{:.1}",
                            f(0),
                            f(1),
                            f(2),
                            f(3).to_degrees(),
                            f(4).to_degrees(),
                            f(5).to_degrees()
                        )
                    })
                    .unwrap_or_default();
                println!("{r} {} -> {base} {tag} {edid}{placement}", rec.tag());
            }
        }
        Some("unplayable-weapons") => {
            // unplayable-weapons <data dir>: non-playable weapons (DNAM flag 0x80) with
            // their animation type and model.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            for &id in lo.ids_of_type(b"WEAP") {
                let Some(rec) = lo.get(id) else { continue };
                let Some(d) = rec.get(b"DNAM").filter(|d| d.len() > 12) else {
                    continue;
                };
                if d[12] & 0x80 == 0 {
                    continue;
                }
                let model = rec
                    .get(b"MODL")
                    .map(esp::decode_zstring)
                    .unwrap_or_default();
                println!(
                    "{id} {:28} anim {} {}",
                    rec.editor_id().unwrap_or_default(),
                    d[0],
                    model
                );
            }
        }
        Some("esp-list") => {
            // esp-list <data dir> <TAG> [edid substring]: records of a type with their text subrecords.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let tag: [u8; 4] = args[2]
                .as_bytes()
                .try_into()
                .context("TAG must be 4 bytes")?;
            let filter = args.get(3).map(|f| f.to_ascii_lowercase());
            for &id in lo.ids_of_type(&tag) {
                let Some(rec) = lo.get(id) else { continue };
                let edid = rec.editor_id().unwrap_or_default();
                if filter
                    .as_ref()
                    .is_some_and(|f| !edid.to_ascii_lowercase().contains(f.as_str()))
                {
                    continue;
                }
                let text: Vec<String> = rec
                    .subrecords()
                    .filter(|s| {
                        s.tag.0 != *b"EDID"
                            && s.data.len() > 1
                            && s.data[..s.data.len() - 1]
                                .iter()
                                .all(|&b| (32..127).contains(&b))
                            && s.data.last() == Some(&0)
                    })
                    .map(|s| {
                        format!(
                            "{}={}",
                            s.tag,
                            String::from_utf8_lossy(&s.data[..s.data.len() - 1])
                        )
                    })
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
                let end = offs
                    .iter()
                    .copied()
                    .find(|&x| x > o)
                    .unwrap_or(p.data.len() as u32);
                println!("{} @{o:#x} size {:#x}", args[3], end - o);
                for slot in (o..end).step_by(4) {
                    let rel = slot - o;
                    if let Some(t) = p.ptr(slot) {
                        let what = match p.object_class(t) {
                            Some(c) => format!("-> {c} @{t:#x}"),
                            None => {
                                let s: String = p.data[t as usize..]
                                    .iter()
                                    .take(60)
                                    .take_while(|&&b| b != 0)
                                    .map(|&b| b as char)
                                    .collect();
                                format!("-> {t:#x} {s:?}")
                            }
                        };
                        println!("  +{rel:#04x} ptr {what}  (next i32 {})", p.i32(slot + 8));
                    } else if rel % 4 == 0 {
                        let x = p.u32(slot);
                        if x != 0 {
                            println!(
                                "  +{rel:#04x} {x:#010x} ({} / {:.3})",
                                x as i32,
                                f32::from_bits(x)
                            );
                        }
                    }
                }
            }
        }
        Some("pex-events") => {
            // pex-events <data dir> <prefix>: how many scripts define each function
            // (any state) whose name starts with the prefix (`OnStory`), and which.
            let data = std::path::Path::new(&args[1]);
            let prefix = args
                .get(2)
                .map_or("on", |p| p.as_str())
                .to_ascii_lowercase();
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let v = vfs::Vfs::new(data, &names);
            let mut by: std::collections::BTreeMap<String, Vec<String>> = Default::default();
            for path in v.list("scripts/") {
                if !path.ends_with(".pex") {
                    continue;
                }
                let Some(p) = v.read(&path).and_then(|b| papyrus::pex::parse(&b).ok()) else {
                    continue;
                };
                for o in &p.objects {
                    let mut seen = std::collections::HashSet::new();
                    for st in &o.states {
                        for (n, _) in &st.functions {
                            let name = p.str(*n).to_string();
                            if name.to_ascii_lowercase().starts_with(&prefix)
                                && seen.insert(name.clone())
                            {
                                by.entry(name).or_default().push(p.str(o.name).to_string());
                            }
                        }
                    }
                }
            }
            for (name, scripts) in by {
                println!(
                    "{name}: {} {:?}",
                    scripts.len(),
                    &scripts[..scripts.len().min(8)]
                );
            }
        }
        Some("pex-calls") => {
            // pex-calls <data dir> <method>: the scripts calling a method or
            // global function (any class) by that name, with each call's line.
            anyhow::ensure!(args.len() > 2, "usage: pex-calls <data dir> <method>");
            let data = std::path::Path::new(&args[1]);
            let name = args[2].to_ascii_lowercase();
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let v = vfs::Vfs::new(data, &names);
            for path in v.list("scripts/") {
                if !path.ends_with(".pex") {
                    continue;
                }
                let Some(p) = v.read(&path).and_then(|b| papyrus::pex::parse(&b).ok()) else {
                    continue;
                };
                let text = papyrus::pex::disassemble(&p);
                let script = p.objects.first().map_or("", |o| p.str(o.name));
                for line in text.lines() {
                    let mut words = line.split_whitespace().skip(1);
                    let (Some(op), Some(called)) = (words.next(), words.next()) else {
                        continue;
                    };
                    let called = called.trim_end_matches(',').to_ascii_lowercase();
                    let hit = match op {
                        "callmethod" => called == name,
                        // callstatic <class>, <name>, ...
                        "callstatic" => words
                            .next()
                            .is_some_and(|w| w.trim_end_matches(',').eq_ignore_ascii_case(&name)),
                        _ => false,
                    };
                    if hit {
                        println!("{script}: {}", line.trim());
                    }
                }
            }
        }
        Some("pex-dump") => {
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let v = vfs::Vfs::new(data, &names);
            let bytes = v
                .read(&format!("scripts/{}.pex", args[2]))
                .context("script not found")?;
            let p = papyrus::pex::parse(&bytes)?;
            print!("{}", papyrus::pex::disassemble(&p));
        }
        Some("pex-verify") => {
            let a = bsa::Archive::open(&args[1])?;
            let mut ok = 0;
            let mut bad = 0;
            let mut ops = [0usize; 36];
            for p in a
                .paths()
                .filter(|p| p.ends_with(".pex"))
                .map(str::to_owned)
                .collect::<Vec<_>>()
            {
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
        Some("trespass-cells") => {
            // trespass-cells <data dir>: interior cells by the flags trespass
            // reads: Off Limits (record flag 0x20000), Public Area (DATA 0x20),
            // Warn To Leave (DATA 0x200), and their owner (XOWN).
            anyhow::ensure!(args.len() > 1, "usage: trespass-cells <data dir>");
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let edid = |f: esp::FormId| {
                lo.get(f)
                    .and_then(|r| r.editor_id().map(|e| e.to_string()))
                    .unwrap_or_else(|| format!("{f}"))
            };
            let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
            for &c in lo.ids_of_type(b"CELL") {
                let Some(r) = lo.get(c) else { continue };
                let data_flags = r.get(b"DATA").map_or(0, |d| {
                    d.iter()
                        .take(2)
                        .enumerate()
                        .fold(0u32, |a, (i, b)| a | (*b as u32) << (8 * i))
                });
                if data_flags & 1 == 0 {
                    continue;
                }
                let owner = r
                    .get(b"XOWN")
                    .filter(|d| d.len() >= 4)
                    .map(|d| r.fid(esp::FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))));
                let off = r.flags() & 0x20000 != 0;
                let public = data_flags & 0x20 != 0;
                let warn = data_flags & 0x200 != 0;
                let key = format!(
                    "{}{}{}{}",
                    if off { "off-limits " } else { "" },
                    if public { "public " } else { "" },
                    if warn { "warn-to-leave " } else { "" },
                    if owner.is_some() { "owned" } else { "unowned" }
                );
                *counts.entry(key).or_default() += 1;
                if off || warn {
                    println!(
                        "{} {}{}{} owner {}",
                        r.editor_id().unwrap_or_default(),
                        if off { "off-limits " } else { "" },
                        if public { "public " } else { "" },
                        if warn { "warn-to-leave" } else { "" },
                        owner.map(edid).unwrap_or_default()
                    );
                }
            }
            for (k, n) in counts {
                println!("{n:6} {k}");
            }
        }
        Some("topic-lines") => {
            // topic-lines <data dir> <subtype>: the responses of every topic of
            // a subtype (SNAM, e.g. TRES, PICN) in order, with their
            // conditions (function index, parameters, run-on, comparison).
            anyhow::ensure!(args.len() > 2, "usage: topic-lines <data dir> <subtype>");
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let mut lo = esp::LoadOrder::load(data, &names)?;
            let vfs = vfs::Vfs::new(data, &names);
            lo.load_strings("english", |p| vfs.read(p));
            const OPS: [&str; 6] = ["==", "!=", ">", ">=", "<", "<="];
            for &t in lo.ids_of_type(b"DIAL") {
                let Some(dial) = lo.get(t) else { continue };
                if dial.get(b"SNAM") != Some(args[2].as_bytes()) {
                    continue;
                }
                println!("{t} {}", dial.editor_id().unwrap_or_default());
                for &i in lo.topic_infos(t) {
                    let Some(r) = lo.get(i) else { continue };
                    let line = r
                        .get(b"NAM1")
                        .map(|n| lo.lstring(&r, n))
                        .unwrap_or_default();
                    let conds: Vec<String> = r
                        .subrecords()
                        .filter(|s| s.tag.0 == *b"CTDA" && s.data.len() >= 24)
                        .map(|s| {
                            let d = s.data;
                            let p = |o: usize| {
                                let v = u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
                                lo.get(r.fid(esp::FormId(v)))
                                    .and_then(|x| x.editor_id().map(|e| e.to_string()))
                                    .unwrap_or_else(|| v.to_string())
                            };
                            let f = u16::from_le_bytes([d[8], d[9]]);
                            let run_on = u32::from_le_bytes(d[20..24].try_into().unwrap());
                            format!(
                                "{f}({}, {}) on {run_on} {} {}{}",
                                p(12),
                                p(16),
                                OPS[(d[0] >> 5) as usize % 6],
                                f32::from_le_bytes(d[4..8].try_into().unwrap()),
                                if d[0] & 1 != 0 { " OR" } else { "" }
                            )
                        })
                        .collect();
                    println!("  {i} \"{line}\" [{}]", conds.join("; "));
                }
            }
        }
        Some("quest-topics") => {
            // quest-topics <data dir> <quest editor id>: the quest's dialogue
            // topics (subtype, branch), each response with its conditions, the
            // topics it links to (TCLT) and its fragment script (TIF__...).
            anyhow::ensure!(args.len() > 2, "usage: quest-topics <data dir> <quest>");
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let mut lo = esp::LoadOrder::load(data, &names)?;
            let vfs = vfs::Vfs::new(data, &names);
            lo.load_strings("english", |p| vfs.read(p));
            let quest = lo.find_editor_id(&args[2]).context("quest not found")?;
            const OPS: [&str; 6] = ["==", "!=", ">", ">=", "<", "<="];
            let edid = |f: esp::FormId| {
                lo.get(f)
                    .and_then(|x| x.editor_id().map(|e| e.to_string()))
                    .unwrap_or_else(|| f.to_string())
            };
            for &t in lo.ids_of_type(b"DIAL") {
                let Some(dial) = lo.get(t) else { continue };
                if dial
                    .get(b"QNAM")
                    .filter(|d| d.len() >= 4)
                    .map(|d| dial.fid(esp::FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
                    != Some(quest)
                {
                    continue;
                }
                let sub = dial
                    .get(b"SNAM")
                    .map(|d| String::from_utf8_lossy(d).into_owned())
                    .unwrap_or_default();
                let branch = dial
                    .get(b"BNAM")
                    .filter(|d| d.len() >= 4)
                    .map(|d| {
                        edid(dial.fid(esp::FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
                    })
                    .unwrap_or_default();
                let full = dial
                    .get(b"FULL")
                    .map(|n| lo.lstring(&dial, n))
                    .unwrap_or_default();
                println!(
                    "{t} {} [{sub}] {branch} \"{full}\"",
                    dial.editor_id().unwrap_or_default()
                );
                for &i in lo.topic_infos(t) {
                    let Some(r) = lo.get(i) else { continue };
                    let line = r
                        .get(b"NAM1")
                        .map(|n| lo.lstring(&r, n))
                        .unwrap_or_default();
                    let prompt = r
                        .get(b"RNAM")
                        .map(|n| lo.lstring(&r, n))
                        .unwrap_or_default();
                    let conds: Vec<String> = r
                        .subrecords()
                        .filter(|s| s.tag.0 == *b"CTDA" && s.data.len() >= 24)
                        .map(|s| {
                            let d = s.data;
                            let p = |o: usize| {
                                let v = u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
                                lo.get(r.fid(esp::FormId(v)))
                                    .and_then(|x| x.editor_id().map(|e| e.to_string()))
                                    .unwrap_or_else(|| v.to_string())
                            };
                            let f = u16::from_le_bytes([d[8], d[9]]);
                            let name = functions::FUNCTIONS
                                .iter()
                                .find(|x| x.0 == f)
                                .map_or(String::new(), |x| x.1.to_string());
                            let run_on = u32::from_le_bytes(d[20..24].try_into().unwrap());
                            format!(
                                "{name}({}, {}) on {run_on} {} {}{}",
                                p(12),
                                p(16),
                                OPS[(d[0] >> 5) as usize % 6],
                                f32::from_le_bytes(d[4..8].try_into().unwrap()),
                                if d[0] & 1 != 0 { " OR" } else { "" }
                            )
                        })
                        .collect();
                    let links: Vec<String> = r
                        .subrecords()
                        .filter(|s| s.tag.0 == *b"TCLT" && s.data.len() >= 4)
                        .map(|s| edid(r.fid(s.form_id(0))))
                        .collect();
                    let script = r
                        .get(b"VMAD")
                        .and_then(|v| {
                            v.windows(5).position(|w| w == b"TIF__").map(|p| {
                                String::from_utf8_lossy(&v[p..(p + 13).min(v.len())]).into_owned()
                            })
                        })
                        .unwrap_or_default();
                    let flags = r
                        .get(b"ENAM")
                        .filter(|d| d.len() >= 2)
                        .map_or(0, |d| u16::from_le_bytes([d[0], d[1]]));
                    println!(
                        "  {i} {prompt:?} \"{line}\" flags {flags:#x} [{}] -> {links:?} {script}",
                        conds.join("; ")
                    );
                }
            }
        }
        Some("ctda-uses") => {
            // ctda-uses <data dir> <func index>...: every condition calling one of
            // these functions, with the record holding it, its parameters (forms
            // by editor id), run-on and comparison.
            anyhow::ensure!(
                args.len() > 2,
                "usage: ctda-uses <data dir> <func index>..."
            );
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let mut lo = esp::LoadOrder::load(data, &names)?;
            let vfs = vfs::Vfs::new(data, &names);
            lo.load_strings("english", |p| vfs.read(p));
            let mut topic_of: std::collections::HashMap<esp::FormId, String> = Default::default();
            for &t in lo.ids_of_type(b"DIAL") {
                let name = lo
                    .get(t)
                    .and_then(|r| r.editor_id().map(|e| e.to_string()))
                    .unwrap_or_else(|| format!("{t}"));
                for &i in lo.topic_infos(t) {
                    topic_of.insert(i, name.clone());
                }
            }
            let funcs: Vec<u16> = args[2..]
                .iter()
                .map(|a| a.parse())
                .collect::<Result<_, _>>()?;
            const OPS: [&str; 6] = ["==", "!=", ">", ">=", "<", "<="];
            for tag in [
                b"INFO", b"PACK", b"IDLE", b"QUST", b"PERK", b"MGEF", b"SPEL", b"SCEN", b"FACT",
                b"DIAL", b"MESG", b"SMQN", b"SMBN", b"SMEN", b"COBJ",
            ] {
                for &id in lo.ids_of_type(tag) {
                    let Some(r) = lo.get(id) else { continue };
                    for s in r
                        .subrecords()
                        .filter(|s| s.tag.0 == *b"CTDA" && s.data.len() >= 24)
                    {
                        let d = s.data;
                        let f = u16::from_le_bytes([d[8], d[9]]);
                        if !funcs.contains(&f) {
                            continue;
                        }
                        let p = |o: usize| {
                            let v = u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
                            lo.get(r.fid(esp::FormId(v)))
                                .and_then(|x| x.editor_id().map(|e| e.to_string()))
                                .unwrap_or_else(|| v.to_string())
                        };
                        let value = f32::from_le_bytes(d[4..8].try_into().unwrap());
                        let run_on = u32::from_le_bytes(d[20..24].try_into().unwrap());
                        let owner = r
                            .editor_id()
                            .map(|e| e.to_string())
                            .unwrap_or_else(|| format!("{id}"));
                        let info_of = if tag == b"INFO" {
                            let line = r
                                .get(b"NAM1")
                                .map(|n| lo.lstring(&r, n))
                                .unwrap_or_default();
                            format!(
                                "{} \"{}\"",
                                topic_of.get(&id).map_or("?", |t| t.as_str()),
                                line.chars().take(80).collect::<String>()
                            )
                        } else {
                            String::new()
                        };
                        println!(
                            "{} {owner} [{f}({}, {}) run_on {run_on} {} {value}] {info_of}",
                            String::from_utf8_lossy(tag),
                            p(12),
                            p(16),
                            OPS[(d[0] >> 5) as usize % 6]
                        );
                    }
                }
            }
        }
        Some("av-conditions") => {
            // av-conditions <data dir>: conditions on actor values (GetActorValue,
            // GetBaseActorValue, GetPermanentActorValue, GetActorValuePercent) by value,
            // with the record types they gate and the values compared against.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            const FUNCS: [(u16, &str); 4] = [
                (14, "GetAV"),
                (277, "GetBaseAV"),
                (494, "GetPermAV"),
                (640, "GetAVPercent"),
            ];
            const OPS: [&str; 6] = ["==", "!=", ">", ">=", "<", "<="];
            type Uses = (
                usize,
                std::collections::BTreeMap<String, usize>,
                std::collections::BTreeSet<String>,
            );
            let mut uses: std::collections::BTreeMap<(u32, &str), Uses> = Default::default();
            for tag in [
                b"INFO", b"PACK", b"IDLE", b"QUST", b"PERK", b"MGEF", b"SPEL", b"SCEN", b"FACT",
                b"DIAL", b"LVLI", b"COBJ", b"MESG", b"LSCR", b"SMQN", b"SMBN", b"SMEN", b"ENCH",
                b"ALCH",
            ] {
                for &id in lo.ids_of_type(tag) {
                    let Some(r) = lo.get(id) else { continue };
                    for s in r
                        .subrecords()
                        .filter(|s| s.tag.0 == *b"CTDA" && s.data.len() >= 24)
                    {
                        let d = s.data;
                        let f = u16::from_le_bytes([d[8], d[9]]);
                        let Some(&(_, fname)) = FUNCS.iter().find(|x| x.0 == f) else {
                            continue;
                        };
                        let av = u32::from_le_bytes(d[12..16].try_into().unwrap());
                        let e = uses.entry((av, fname)).or_default();
                        e.0 += 1;
                        *e.1.entry(String::from_utf8_lossy(tag).into_owned())
                            .or_default() += 1;
                        let value = if d[0] & 0x04 != 0 {
                            "global".to_owned()
                        } else {
                            format!("{}", f32::from_le_bytes(d[4..8].try_into().unwrap()))
                        };
                        if e.2.len() < 8 {
                            e.2.insert(format!(
                                "{} {value}",
                                OPS.get((d[0] >> 5) as usize).unwrap_or(&"?")
                            ));
                        }
                    }
                }
            }
            for ((av, f), (n, tags, values)) in &uses {
                let name =
                    esp::actor_value::name(*av).map_or_else(|| format!("#{av}"), str::to_owned);
                println!("{n:>6} {f:13} {name:22} {tags:?} {values:?}");
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
                    && let Some(m) = r
                        .get(b"NVNM")
                        .and_then(|d| esp::navmesh::NavMesh::parse(d, |f| r.fid(f)))
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
                            let back = (0..3)
                                .filter_map(|e| tri.link(e))
                                .any(|i| t.edge_links[i].navmesh == id);
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
        Some("story") => {
            // story <data dir> [event code]: per event type, its nodes and quests and
            // the event members (R1, L1...) conditions and "from event" aliases use,
            // with alias names as hints; with a code, that event's tree (CONDS=1:
            // with each node's conditions). OWN=1 lists quests with their own
            // event conditions.
            use std::collections::{BTreeMap, BTreeSet, HashMap};
            anyhow::ensure!(args.len() > 1, "usage: story <data dir> [event code]");
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let edid = |f: esp::FormId| {
                lo.get(f)
                    .and_then(|r| r.editor_id())
                    .unwrap_or_else(|| f.to_string())
            };
            let mut nodes: HashMap<esp::FormId, esp::story::Node> = HashMap::new();
            for tag in [b"SMEN", b"SMBN", b"SMQN"] {
                for &id in lo.ids_of_type(tag) {
                    if let Some(n) = lo.get(id).and_then(|r| esp::story::parse(&r, id)) {
                        nodes.insert(id, n);
                    }
                }
            }
            // Children in order: the first has no previous sibling, each next names it.
            let mut children: HashMap<esp::FormId, Vec<esp::FormId>> = HashMap::new();
            for n in nodes.values() {
                children.entry(n.parent).or_default().push(n.id);
            }
            for kids in children.values_mut() {
                let mut ordered = Vec::new();
                let mut prev = esp::FormId::NULL;
                while let Some(i) = kids.iter().position(|&k| nodes[&k].previous == prev) {
                    prev = kids.remove(i);
                    ordered.push(prev);
                }
                ordered.extend(kids.drain(..));
                *kids = ordered;
            }
            let members_of = |n: &esp::story::Node, out: &mut BTreeSet<String>| {
                for c in &n.conditions {
                    let d = &c.ctda;
                    if d.len() < 32 {
                        continue;
                    }
                    let func = u16::from_le_bytes([d[8], d[9]]);
                    let run_on = u32::from_le_bytes(d[20..24].try_into().unwrap());
                    if run_on == 7 {
                        out.insert(format!("{} (run on)", esp::story::code(&d[28..30])));
                    }
                    if func == 576 {
                        out.insert(format!(
                            "{} (GetEventData fn {})",
                            esp::story::code(&d[14..16]),
                            u16::from_le_bytes([d[12], d[13]])
                        ));
                    }
                }
            };
            fn walk(
                id: esp::FormId,
                depth: usize,
                nodes: &HashMap<esp::FormId, esp::story::Node>,
                children: &HashMap<esp::FormId, Vec<esp::FormId>>,
                f: &mut dyn FnMut(&esp::story::Node, usize),
            ) {
                f(&nodes[&id], depth);
                for &k in children.get(&id).map(|v| v.as_slice()).unwrap_or(&[]) {
                    walk(k, depth + 1, nodes, children, f);
                }
            }
            // Quests' "from event" aliases: (event, member) -> alias names.
            let mut alias_members: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
            let mut quest_events: BTreeMap<String, usize> = BTreeMap::new();
            for &q in lo.ids_of_type(b"QUST") {
                let Some(rec) = lo.get(q) else { continue };
                if let Some(e) = rec.get(b"ENAM") {
                    *quest_events.entry(esp::story::code(e)).or_default() += 1;
                }
                let (mut name, mut ev) = (String::new(), None);
                for sr in rec.subrecords() {
                    match &sr.tag.0 {
                        b"ALST" | b"ALLS" => (name, ev) = (String::new(), None),
                        b"ALID" => name = sr.zstring(),
                        b"ALFE" => ev = Some(esp::story::code(sr.data)),
                        b"ALFD" => {
                            if let Some(e) = &ev {
                                alias_members
                                    .entry((e.clone(), esp::story::code(sr.data)))
                                    .or_default()
                                    .insert(name.clone());
                            }
                        }
                        _ => {}
                    }
                }
            }
            // Quests' own event conditions: CTDAs after NEXT, before the stages.
            let mut own: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for &q in lo.ids_of_type(b"QUST") {
                let Some(rec) = lo.get(q) else { continue };
                let (mut after_next, mut funcs) = (false, Vec::new());
                for sr in rec.subrecords() {
                    match &sr.tag.0 {
                        b"NEXT" => after_next = true,
                        b"INDX" | b"QOBJ" | b"ALST" | b"ALLS" | b"ANAM" => break,
                        b"CTDA" if after_next && sr.data.len() >= 32 => funcs.push(format!(
                            "{}/{}",
                            u16::from_le_bytes([sr.data[8], sr.data[9]]),
                            u32::from_le_bytes(sr.data[20..24].try_into().unwrap())
                        )),
                        _ => {}
                    }
                }
                if !funcs.is_empty() {
                    let e = rec.get(b"ENAM").map(esp::story::code).unwrap_or_default();
                    own.entry(e)
                        .or_default()
                        .push(format!("{} {funcs:?}", rec.editor_id().unwrap_or_default()));
                }
            }
            if std::env::var("OWN").is_ok() {
                for (e, qs) in &own {
                    println!("{e}: {} quests with event conditions", qs.len());
                    for q in qs.iter().take(8) {
                        println!("  {q}");
                    }
                }
            }
            let mut roots: Vec<&esp::story::Node> =
                nodes.values().filter(|n| n.event().is_some()).collect();
            roots.sort_by_key(|n| n.event());
            if let Some(code) = args.get(2) {
                for r in roots
                    .iter()
                    .filter(|r| esp::story::code(&r.event().unwrap()) == *code)
                {
                    walk(r.id, 0, &nodes, &children, &mut |n, d| {
                        let what = match &n.kind {
                            esp::story::NodeKind::Quest { quests, num_to_run } => format!(
                                "quests {:?}{}",
                                quests
                                    .iter()
                                    .map(|q| format!(
                                        "{}{}",
                                        edid(q.quest),
                                        if q.reset_hours > 0.0 {
                                            format!(" reset {}h", q.reset_hours)
                                        } else {
                                            String::new()
                                        }
                                    ))
                                    .collect::<Vec<_>>(),
                                if *num_to_run > 0 {
                                    format!(" run {num_to_run}")
                                } else {
                                    String::new()
                                }
                            ),
                            esp::story::NodeKind::Branch => "branch".into(),
                            esp::story::NodeKind::Event(_) => "event".into(),
                        };
                        let mut m = BTreeSet::new();
                        members_of(n, &mut m);
                        println!(
                            "{}{} {} flags {:#x}/{:#x} max {} conditions {} {what} {m:?}",
                            "  ".repeat(d),
                            n.editor_id,
                            n.id,
                            n.node_flags,
                            n.quest_flags,
                            n.max_concurrent,
                            n.conditions.len()
                        );
                        // CONDS=1: each condition's function, operator, value, parameters and run-on.
                        if std::env::var("CONDS").is_ok() {
                            for c in &n.conditions {
                                let x = &c.ctda;
                                if x.len() < 32 {
                                    continue;
                                }
                                let u =
                                    |o: usize| u32::from_le_bytes(x[o..o + 4].try_into().unwrap());
                                let p1 = esp::FormId(u(12));
                                println!(
                                    "{}  - fn {} op {:#04x} value {} p1 {} ({}) p2 {:08X} run_on {} ref {:08X}",
                                    "  ".repeat(d),
                                    u16::from_le_bytes([x[8], x[9]]),
                                    x[0],
                                    f32::from_le_bytes(x[4..8].try_into().unwrap()),
                                    p1,
                                    edid(p1),
                                    u(16),
                                    u(20),
                                    u(24)
                                );
                            }
                        }
                    });
                }
                return Ok(());
            }
            println!("{} nodes, {} event roots", nodes.len(), roots.len());
            for r in &roots {
                let (mut qnodes, mut quests) = (0, 0);
                let mut members = BTreeSet::new();
                walk(r.id, 0, &nodes, &children, &mut |n, _| {
                    members_of(n, &mut members);
                    if let esp::story::NodeKind::Quest { quests: q, .. } = &n.kind {
                        qnodes += 1;
                        quests += q.len();
                    }
                });
                let code = esp::story::code(&r.event().unwrap());
                println!(
                    "{code} {}: {qnodes} quest nodes, {quests} quests ({} with this event type); members {members:?}",
                    r.id,
                    quest_events.get(&code).copied().unwrap_or(0)
                );
            }
            println!("from-event aliases (event, member: names):");
            for ((e, m), names) in &alias_members {
                let v: Vec<&String> = names.iter().take(6).collect();
                println!("  {e} {m}: {} {v:?}", names.len());
            }
        }
        Some("faction-owners") => {
            // faction-owners <data dir>: factions that own references or cells
            // (XOWN), by their DATA flags, against all factions' flags.
            use std::collections::{BTreeMap, HashSet};
            anyhow::ensure!(args.len() > 1, "usage: faction-owners <data dir>");
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let flags_of = |f: esp::FormId| {
                lo.get(f).and_then(|r| {
                    r.get(b"DATA")
                        .filter(|d| d.len() >= 4)
                        .map(|d| u32::from_le_bytes(d[0..4].try_into().unwrap()))
                })
            };
            let mut owners: HashSet<esp::FormId> = HashSet::new();
            let mut owned = 0usize;
            for tag in [b"REFR", b"CELL", b"ACHR"] {
                for &id in lo.ids_of_type(tag) {
                    let Some(rec) = lo.get(id) else { continue };
                    if let Some(d) = rec.get(b"XOWN").filter(|d| d.len() >= 4) {
                        let o =
                            rec.fid(esp::FormId(u32::from_le_bytes(d[0..4].try_into().unwrap())));
                        if lo.tag_of(o).is_some_and(|t| t.0 == *b"FACT") {
                            owners.insert(o);
                            owned += 1;
                        }
                    }
                }
            }
            println!(
                "{owned} references / cells owned by {} factions",
                owners.len()
            );
            // Per flag bit: factions with it, owning factions with it.
            let mut bits: BTreeMap<u32, (usize, usize)> = BTreeMap::new();
            for &f in lo.ids_of_type(b"FACT") {
                let fl = flags_of(f).unwrap_or(0);
                for b in 0..32 {
                    if fl & (1 << b) != 0 {
                        let e = bits.entry(1 << b).or_default();
                        e.0 += 1;
                        e.1 += owners.contains(&f) as usize;
                    }
                }
            }
            for (b, (all, own)) in bits {
                println!("  flag {b:#07x}: {all} factions, {own} of them owners");
            }
            let missing: Vec<String> = owners
                .iter()
                .filter(|&&f| flags_of(f).unwrap_or(0) & 0x8000 == 0)
                .map(|&f| lo.get(f).and_then(|r| r.editor_id()).unwrap_or_default())
                .collect();
            println!(
                "owners without 0x8000: {} {:?}",
                missing.len(),
                missing.iter().take(10).collect::<Vec<_>>()
            );
        }
        Some("crime-factions") => {
            // crime-factions <data dir>: factions that track crime, their crime
            // gold (CRVA, or "use defaults"), shared crime groups (CRGR), how
            // many NPCs name each as their crime faction (CRIF) and locations
            // naming it as their unreported crime faction (FNAM).
            use std::collections::BTreeMap;
            anyhow::ensure!(args.len() > 1, "usage: crime-factions <data dir>");
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let form = |rec: &esp::LoadedRecord<'_>, tag: &[u8; 4]| {
                rec.get(tag)
                    .filter(|d| d.len() >= 4)
                    .map(|d| rec.fid(esp::FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
            };
            let edid = |f: esp::FormId| {
                lo.get(f)
                    .and_then(|r| r.editor_id())
                    .unwrap_or_else(|| format!("{f}"))
            };
            let mut crif: BTreeMap<esp::FormId, usize> = BTreeMap::new();
            for &n in lo.ids_of_type(b"NPC_") {
                if let Some(f) = lo.get(n).and_then(|r| form(&r, b"CRIF")) {
                    *crif.entry(f).or_default() += 1;
                }
            }
            let mut fnam: BTreeMap<esp::FormId, usize> = BTreeMap::new();
            for &l in lo.ids_of_type(b"LCTN") {
                if let Some(f) = lo.get(l).and_then(|r| form(&r, b"FNAM")) {
                    *fnam.entry(f).or_default() += 1;
                }
            }
            for &f in lo.ids_of_type(b"FACT") {
                let Some(rec) = lo.get(f) else { continue };
                let flags = rec
                    .get(b"DATA")
                    .filter(|d| d.len() >= 4)
                    .map_or(0, |d| u32::from_le_bytes(d[0..4].try_into().unwrap()));
                if flags & 0x40 == 0 && !crif.contains_key(&f) {
                    continue;
                }
                let crva = rec.get(b"CRVA").map(|d| {
                    let u16_at = |o: usize| d.get(o..o + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]));
                    let mult = d.get(12..16).map_or(0.0, |b| f32::from_le_bytes(b.try_into().unwrap()));
                    format!("arrest {} aos {} murder {} assault {} trespass {} pickpocket {} steal x{mult} escape {} werewolf {} ({} bytes)", d[0], d.get(1).copied().unwrap_or(0), u16_at(2), u16_at(4), u16_at(6), u16_at(8), u16_at(16), u16_at(18), d.len())
                });
                let group = form(&rec, b"CRGR").map(|g| {
                    let list = lo
                        .get(g)
                        .map(|r| r.subrecords().filter(|s| s.tag.0 == *b"LNAM").count())
                        .unwrap_or(0);
                    format!("{} ({list} factions)", edid(g))
                });
                println!(
                    "{} {f} flags {flags:#07x}{} crif {} locs {}\n    {}\n    group {}  jail {:?} stolen {:?}",
                    edid(f),
                    if flags & 0x1000 != 0 {
                        " (defaults)"
                    } else {
                        ""
                    },
                    crif.get(&f).copied().unwrap_or(0),
                    fnam.get(&f).copied().unwrap_or(0),
                    crva.unwrap_or_else(|| "no CRVA".into()),
                    group.unwrap_or_else(|| "-".into()),
                    form(&rec, b"JAIL").map(edid),
                    form(&rec, b"STOL").map(edid),
                );
            }
        }
        Some("first-of") => {
            // first-of <data dir> <TYPE> [n]: editor ids and form ids of the first n records of a type.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let tag: [u8; 4] = args[2].as_bytes().try_into().context("4-char type")?;
            let n: usize = args.get(3).and_then(|n| n.parse().ok()).unwrap_or(10);
            println!("{} records", lo.ids_of_type(&tag).len());
            for &id in lo.ids_of_type(&tag).iter().take(n) {
                println!(
                    "{id} {}",
                    lo.get(id).and_then(|r| r.editor_id()).unwrap_or_default()
                );
            }
        }
        Some("full-lod") => {
            // full-lod <data dir> [world]: the worldspace's references flagged
            // 0x10000 (full LOD), counted by where they live (persistent cell or
            // not) and their base's model folder.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let world = lo
                .find_editor_id(args.get(2).map_or("Tamriel", |s| s.as_str()))
                .context("world not found")?;
            let wi = lo.world(world).context("not a worldspace")?;
            let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
            let mut count = |cell: esp::FormId, persistent: bool| {
                let Some(idx) = lo.cell(cell) else { return };
                for &r in idx.persistent.iter().chain(idx.temporary.iter()) {
                    let Some(rec) = lo.get(r) else { continue };
                    if rec.flags() & 0x10000 == 0 {
                        continue;
                    }
                    let base = rec.get(b"NAME").filter(|d| d.len() >= 4).map(|d| {
                        rec.fid(esp::FormId(u32::from_le_bytes(d[0..4].try_into().unwrap())))
                    });
                    let model = base
                        .and_then(|b| lo.get(b))
                        .and_then(|b| {
                            b.get(b"MODL")
                                .map(|m| String::from_utf8_lossy(m).to_ascii_lowercase())
                        })
                        .unwrap_or_default();
                    let folder = model.rsplit_once('\\').map_or("", |(f, _)| f).to_string();
                    *counts
                        .entry(format!(
                            "{} {folder}",
                            if persistent {
                                "persistent"
                            } else {
                                "temporary "
                            }
                        ))
                        .or_default() += 1;
                }
            };
            if let Some(pc) = wi.persistent_cell {
                count(pc, true);
            }
            for &c in wi.cells.values() {
                count(c, false);
            }
            for (k, n) in counts {
                println!("{n:6} {k}");
            }
        }
        Some("weathers") => {
            // weathers <data dir>: every weather's DATA wind (speed, direction byte 17,
            // range byte 18), flags (classification, aurora bits), aurora model,
            // sky statics (TNAM) and precipitation (MNAM).
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            for &id in lo.ids_of_type(b"WTHR") {
                let Some(rec) = lo.get(id) else { continue };
                let d = rec.get(b"DATA").unwrap_or_default();
                let b = |i: usize| d.get(i).copied().unwrap_or(0);
                let statics = rec.subrecords().filter(|s| s.tag.0 == *b"TNAM").count();
                let aurora = rec
                    .get(b"MODL")
                    .map(|m| {
                        String::from_utf8_lossy(m)
                            .trim_end_matches('\0')
                            .to_string()
                    })
                    .unwrap_or_default();
                let mnam = rec
                    .get(b"MNAM")
                    .filter(|m| m.len() >= 4)
                    .map(|m| u32::from_le_bytes(m[0..4].try_into().unwrap()))
                    .unwrap_or(0);
                println!(
                    "{id} {:<32} wind {:3} dir {:3} range {:3} flags {:02x} statics {:3} precip {mnam:08X} {aurora}",
                    rec.editor_id().unwrap_or_default(),
                    b(0),
                    b(17),
                    b(18),
                    b(11),
                    statics,
                );
            }
        }
        Some("grass") => {
            // grass <data dir>: every grass (GRAS) with its DATA (density, min /
            // max slope, units from water and its type, position / height / colour
            // range, wave period, flags), model and how many landscape textures
            // (LTEX GNAM) list it; then the most grasses one LTEX lists.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let mut users = std::collections::HashMap::<esp::FormId, usize>::new();
            let mut most = 0;
            for &id in lo.ids_of_type(b"LTEX") {
                let Some(rec) = lo.get(id) else { continue };
                let mut n = 0;
                for s in rec.subrecords().filter(|s| s.tag.0 == *b"GNAM") {
                    *users.entry(rec.fid(s.form_id(0))).or_default() += 1;
                    n += 1;
                }
                most = most.max(n);
            }
            for &id in lo.ids_of_type(b"GRAS") {
                let Some(rec) = lo.get(id) else { continue };
                let d = rec.get(b"DATA").unwrap_or_default();
                if d.len() < 32 {
                    println!("{id} DATA {} bytes", d.len());
                    continue;
                }
                let f = |i: usize| f32::from_le_bytes(d[i..i + 4].try_into().unwrap());
                let model = rec
                    .get(b"MODL")
                    .map(|m| {
                        String::from_utf8_lossy(m)
                            .trim_end_matches('\0')
                            .to_string()
                    })
                    .unwrap_or_default();
                println!(
                    "{id} {:<28} dens {:3} slope {:2}-{:2} water {:4} type {} pos {:5.1} h {:4.2} col {:4.2} wave {:5.1} flags {:02x} ltex {:3} {model}",
                    rec.editor_id().unwrap_or_default(),
                    d[0],
                    d[1],
                    d[2],
                    u16::from_le_bytes([d[4], d[5]]),
                    u32::from_le_bytes(d[8..12].try_into().unwrap()),
                    f(12),
                    f(16),
                    f(20),
                    f(24),
                    d[28],
                    users.get(&id).copied().unwrap_or(0),
                );
            }
            println!("most grasses on one LTEX: {most}");
        }
        Some("image-spaces") => {
            // image-spaces <data dir> [imgs]: one image space's values, or over all of
            // them: HNAM / CNAM / TNAM / DNAM ranges, which cells and weathers use
            // them, and interiors without one (XCIM).
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let floats = |d: &[u8]| -> Vec<f32> {
                d.chunks_exact(4)
                    .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                    .collect()
            };
            let fields: [(&[u8; 4], &[&str]); 4] = [
                (
                    b"HNAM",
                    &[
                        "eye adapt speed",
                        "bloom blur radius",
                        "bloom threshold",
                        "bloom scale",
                        "receive bloom threshold",
                        "white",
                        "sunlight scale",
                        "sky scale",
                        "eye adapt strength",
                    ],
                ),
                (b"CNAM", &["saturation", "brightness", "contrast"]),
                (b"TNAM", &["tint amount", "tint r", "tint g", "tint b"]),
                (b"DNAM", &["dof strength", "dof distance", "dof range"]),
            ];
            if let Some(name) = args.get(2) {
                let id = match u32::from_str_radix(name, 16) {
                    Ok(v) if name.len() == 8 => esp::FormId(v),
                    _ => lo.find_editor_id(name).context("editor id not found")?,
                };
                let r = lo.get(id).context("record not found")?;
                println!("{} {id}", r.editor_id().unwrap_or_default());
                for (tag, labels) in fields {
                    let Some(d) = r.get(tag) else { continue };
                    for (l, v) in labels.iter().zip(floats(d)) {
                        println!("  {l}: {v}");
                    }
                }
                return Ok(());
            }
            let mut ranges: std::collections::BTreeMap<String, (f32, f32, usize)> =
                Default::default();
            let ids = lo.ids_of_type(b"IMGS");
            println!("{} image spaces", ids.len());
            for &id in ids {
                let Some(r) = lo.get(id) else { continue };
                for (tag, labels) in fields {
                    let Some(d) = r.get(tag) else {
                        ranges
                            .entry(format!("no {tag:?}", tag = String::from_utf8_lossy(tag)))
                            .or_insert((0.0, 0.0, 0))
                            .2 += 1;
                        continue;
                    };
                    for (l, v) in labels.iter().zip(floats(d)) {
                        let e = ranges
                            .entry(l.to_string())
                            .or_insert((f32::MAX, f32::MIN, 0));
                        e.0 = e.0.min(v);
                        e.1 = e.1.max(v);
                        e.2 += 1;
                    }
                }
            }
            for (k, (lo, hi, n)) in &ranges {
                println!("  {k}: {lo} .. {hi} ({n})");
            }
            let (mut with, mut without) = (0, Vec::new());
            for &id in lo.ids_of_type(b"CELL") {
                let Some(r) = lo.get(id) else { continue };
                if r.get(b"DATA").is_none_or(|d| d[0] & 1 == 0) {
                    continue;
                }
                if r.get(b"XCIM").is_some() {
                    with += 1;
                } else {
                    without.push(r.editor_id().unwrap_or_default());
                }
            }
            println!(
                "interiors: {with} with XCIM, {} without (e.g. {:?})",
                without.len(),
                &without[..without.len().min(8)]
            );
            let mut weathers = 0;
            let mut no_imsp = Vec::new();
            for &id in lo.ids_of_type(b"WTHR") {
                let Some(r) = lo.get(id) else { continue };
                weathers += 1;
                if r.get(b"IMSP").is_none() {
                    no_imsp.push(r.editor_id().unwrap_or_default());
                }
            }
            println!("weathers: {weathers}, without IMSP {no_imsp:?}");
            for &id in lo.ids_of_type(b"WRLD") {
                let Some(r) = lo.get(id) else { continue };
                let tags: Vec<String> = r
                    .subrecords()
                    .map(|s| s.tag.to_string())
                    .filter(|t| ["INAM", "XCIM", "ZNAM"].contains(&t.as_str()))
                    .collect();
                if !tags.is_empty() {
                    println!("  world {} has {tags:?}", r.editor_id().unwrap_or_default());
                }
            }
        }
        Some("model-users") => {
            // model-users <data dir> <model path substring> [n]: base records whose
            // model's path has the substring, and up to n placed references of them
            // with their positions.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let want = args[2].to_ascii_lowercase();
            let n: usize = args.get(3).and_then(|n| n.parse().ok()).unwrap_or(10);
            let mut bases = std::collections::HashSet::new();
            for tag in [
                b"STAT", b"MSTT", b"ACTI", b"CONT", b"MISC", b"FURN", b"DOOR", b"TACT",
            ] {
                for &id in lo.ids_of_type(tag) {
                    let Some(rec) = lo.get(id) else { continue };
                    let Some(m) = rec.get(b"MODL") else { continue };
                    let path = String::from_utf8_lossy(m)
                        .trim_end_matches('\0')
                        .to_ascii_lowercase();
                    if path.contains(&want) {
                        println!(
                            "base {id} {} {} {path}",
                            rec.tag(),
                            rec.editor_id().unwrap_or_default()
                        );
                        bases.insert(id);
                    }
                }
            }
            let mut shown = 0;
            for &id in lo.ids_of_type(b"REFR") {
                let Some(rec) = lo.get(id) else { continue };
                let Some(name) = rec.get(b"NAME").filter(|d| d.len() >= 4) else {
                    continue;
                };
                let base = rec.fid(esp::FormId(u32::from_le_bytes(
                    name[0..4].try_into().unwrap(),
                )));
                if !bases.contains(&base) {
                    continue;
                }
                let cell = lo
                    .cell_of_ref(id)
                    .and_then(|c| lo.get(c).and_then(|r| r.editor_id().map(|e| e.to_string())))
                    .unwrap_or_default();
                let pos = rec
                    .get(b"DATA")
                    .filter(|d| d.len() >= 12)
                    .map(|d| {
                        let f = |i: usize| f32::from_le_bytes(d[i..i + 4].try_into().unwrap());
                        format!(" at {:.0} {:.0} {:.0}", f(0), f(4), f(8))
                    })
                    .unwrap_or_default();
                println!("ref {id} -> {base} in {cell}{pos}");
                shown += 1;
                if shown >= n {
                    break;
                }
            }
        }
        Some("script-users") => {
            // script-users <data dir> <script> [n]: records (NPC_, ACHR, REFR, QUST...)
            // whose VMAD names the script.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let want = args[2].to_ascii_lowercase().into_bytes();
            let n: usize = args.get(3).and_then(|n| n.parse().ok()).unwrap_or(20);
            let mut shown = 0;
            for tag in [
                b"NPC_", b"ACHR", b"REFR", b"QUST", b"ACTI", b"MGEF", b"FURN", b"CONT", b"DOOR",
            ] {
                for &id in lo.ids_of_type(tag) {
                    let Some(rec) = lo.get(id) else { continue };
                    let Some(vmad) = rec.get(b"VMAD") else {
                        continue;
                    };
                    let lower = vmad.to_ascii_lowercase();
                    if !lower.windows(want.len()).any(|w| w == want.as_slice()) {
                        continue;
                    }
                    let cell = lo
                        .cell_of_ref(id)
                        .and_then(|c| lo.get(c).and_then(|r| r.editor_id().map(|e| e.to_string())))
                        .unwrap_or_default();
                    println!(
                        "{id} {} {} {cell}",
                        rec.tag(),
                        rec.editor_id().unwrap_or_default()
                    );
                    shown += 1;
                    if shown >= n {
                        return Ok(());
                    }
                }
            }
        }
        Some("triggers") => {
            // triggers <data dir>: references with a primitive (XPRM) by shape, the
            // scripts on them (or their base) by name, and their bases' types.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
            let scripts_of = |rec: &esp::LoadedRecord<'_>| -> Vec<String> {
                // VMAD: version, format, count, then (name, ...) — names only of the first script.
                let Some(d) = rec.get(b"VMAD") else {
                    return Vec::new();
                };
                if d.len() < 8 {
                    return Vec::new();
                }
                let n = u16::from_le_bytes([d[4], d[5]]);
                let len = u16::from_le_bytes([d[6], d[7]]) as usize;
                if n == 0 || d.len() < 8 + len {
                    return Vec::new();
                }
                vec![format!(
                    "{}{}",
                    esp::decode_zstring(&d[8..8 + len]),
                    if n > 1 { " (+more)" } else { "" }
                )]
            };
            for &id in lo.ids_of_type(b"REFR") {
                let Some(rec) = lo.get(id) else { continue };
                let Some(p) = rec.get(b"XPRM") else { continue };
                let shape = p
                    .get(28..32)
                    .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                    .unwrap_or(0);
                *counts.entry(format!("shape {shape}")).or_default() += 1;
                let base = rec
                    .get(b"NAME")
                    .map(|d| rec.fid(esp::FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
                    .unwrap_or_default();
                let tag = lo.tag_of(base).map(|t| t.to_string()).unwrap_or_default();
                *counts.entry(format!("base {tag}")).or_default() += 1;
                let mut s = scripts_of(&rec);
                if let Some(b) = lo.get(base) {
                    s.extend(scripts_of(&b).into_iter().map(|x| format!("{x} (base)")));
                }
                if s.is_empty() {
                    *counts.entry("no script".into()).or_default() += 1;
                }
                for x in s {
                    *counts.entry(format!("script {x}")).or_default() += 1;
                }
            }
            let mut v: Vec<_> = counts.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1));
            for (k, n) in v.iter().take(60) {
                println!("{n:>7} {k}");
            }
        }
        Some("scenes") => {
            // scenes <data dir> [scene]: one scene's phases, actors and actions, or
            // counts over all scenes: flags, action kinds, package actions' templates
            // and the phases only a package action (of that template) ends.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            let edid = |f: esp::FormId| {
                lo.get(f)
                    .and_then(|r| r.editor_id())
                    .unwrap_or_else(|| f.to_string())
            };
            let template = |p: esp::FormId| {
                let r = lo.get(p)?;
                let t = r
                    .get(b"PKCU")
                    .filter(|d| d.len() >= 8)
                    .map(|d| r.fid(esp::FormId(u32::from_le_bytes(d[4..8].try_into().unwrap()))))?;
                Some(edid(t))
            };
            if let Some(name) = args.get(2) {
                let id = match u32::from_str_radix(name, 16) {
                    Ok(v) if name.len() == 8 => esp::FormId(v),
                    _ => lo.find_editor_id(name).context("editor id not found")?,
                };
                let s = esp::scene::parse(&lo.get(id).context("no record")?, id)
                    .context("not a scene")?;
                println!(
                    "{} {} quest {} flags {:#x} conditions {}",
                    s.editor_id,
                    s.id,
                    edid(s.quest),
                    s.flags,
                    s.conditions.len()
                );
                for (i, p) in s.phases.iter().enumerate() {
                    println!(
                        "  phase {i} {:?}: start conditions {}, completion conditions {}",
                        p.name,
                        p.start.len(),
                        p.completion.len()
                    );
                }
                for a in &s.actors {
                    println!(
                        "  actor alias {} flags {:#x} behaviour {:#x}",
                        a.alias, a.flags, a.behaviour
                    );
                }
                for a in &s.actions {
                    let what = match &a.kind {
                        esp::scene::ActionKind::Dialogue {
                            topic,
                            headtrack,
                            loop_min,
                            loop_max,
                            ..
                        } => {
                            format!(
                                "say {} headtrack {headtrack:?} loop {loop_min}..{loop_max}",
                                if topic.is_null() {
                                    "-".into()
                                } else {
                                    edid(*topic)
                                }
                            )
                        }
                        esp::scene::ActionKind::Package { packages } => {
                            format!(
                                "packages {:?}",
                                packages
                                    .iter()
                                    .map(|&p| format!(
                                        "{} ({})",
                                        edid(p),
                                        template(p).unwrap_or_default()
                                    ))
                                    .collect::<Vec<_>>()
                            )
                        }
                        esp::scene::ActionKind::Timer { seconds } => format!("timer {seconds}s"),
                    };
                    println!(
                        "  action {} {:?} alias {} phases {}..{} flags {:#x}: {what}",
                        a.index, a.name, a.actor, a.start_phase, a.end_phase, a.flags
                    );
                }
                return Ok(());
            }
            if std::env::var("SGE").is_ok() {
                // Scenes that begin with a start-game-enabled quest.
                for &id in lo.ids_of_type(b"SCEN") {
                    let Some(s) = lo.get(id).and_then(|r| esp::scene::parse(&r, id)) else {
                        continue;
                    };
                    let sge = lo
                        .get(s.quest)
                        .and_then(|q| q.get(b"DNAM").map(|d| d[0] & 1 != 0))
                        .unwrap_or(false);
                    if sge && s.flags & esp::scene::flags::BEGIN_ON_QUEST_START != 0 {
                        println!(
                            "{} {} quest {} phases {} actions {}",
                            s.editor_id,
                            s.id,
                            edid(s.quest),
                            s.phases.len(),
                            s.actions.len()
                        );
                    }
                }
                return Ok(());
            }
            let mut n = 0;
            let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
            let mut only_ender: std::collections::BTreeMap<String, usize> = Default::default();
            for &id in lo.ids_of_type(b"SCEN") {
                let Some(s) = lo.get(id).and_then(|r| esp::scene::parse(&r, id)) else {
                    continue;
                };
                n += 1;
                let mut c = |k: String| *counts.entry(k).or_default() += 1;
                for bit in 0..8 {
                    if s.flags & (1 << bit) != 0 {
                        c(format!("scene flag {:#x}", 1 << bit));
                    }
                }
                c(format!("scene conditions: {}", !s.conditions.is_empty()));
                for p in &s.phases {
                    c(format!("phase start conditions: {}", !p.start.is_empty()));
                    c(format!(
                        "phase completion conditions: {}",
                        !p.completion.is_empty()
                    ));
                }
                for a in &s.actors {
                    c(format!("actor flags {:#x}", a.flags));
                    for bit in 0..8 {
                        if a.behaviour & (1 << bit) != 0 {
                            c(format!("actor behaviour {:#x}", 1 << bit));
                        }
                    }
                }
                for a in &s.actions {
                    let looping = a.flags & esp::scene::action_flags::LOOPING != 0;
                    c(format!("action flags {:#x}", a.flags));
                    match &a.kind {
                        esp::scene::ActionKind::Dialogue { topic, .. } => c(format!(
                            "dialogue{}{}",
                            if topic.is_null() { " (no topic)" } else { "" },
                            if looping { " looping" } else { "" }
                        )),
                        esp::scene::ActionKind::Timer { .. } => c("timer".into()),
                        esp::scene::ActionKind::Package { packages } => {
                            c(format!("package action with {} packages", packages.len()));
                            for &p in packages {
                                c(format!(
                                    "package template {}",
                                    template(p).unwrap_or_else(|| "(own tree)".into())
                                ));
                            }
                            // The only action ending its end phase, with no completion conditions?
                            let others = s
                                .actions
                                .iter()
                                .filter(|o| {
                                    o.index != a.index
                                        && o.end_phase == a.end_phase
                                        && o.flags & esp::scene::action_flags::LOOPING == 0
                                })
                                .count();
                            let cond = s
                                .phases
                                .get(a.end_phase as usize)
                                .is_some_and(|p| !p.completion.is_empty());
                            if others == 0 && !cond {
                                let t = packages
                                    .first()
                                    .and_then(|&p| template(p))
                                    .unwrap_or_default();
                                *only_ender.entry(t).or_default() += 1;
                            }
                        }
                    }
                }
            }
            println!("{n} scenes");
            for (k, v) in counts {
                println!("  {k}: {v}");
            }
            println!(
                "package actions that alone end their end phase, by first package's template:"
            );
            for (k, v) in only_ender {
                println!("  {k}: {v}");
            }
        }
        Some("messages") => {
            // messages <data dir> [message]: one message's flags, title, text and
            // buttons, or counts over all messages: flags, buttons, conditions on
            // buttons and the tags their text uses (`<Alias=...>`, `%.0f`, `[...]`).
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let mut lo = esp::LoadOrder::load(data, &names)?;
            let vfs = vfs::Vfs::new(data, &names);
            lo.load_strings("english", |p| vfs.read(p));
            let show = |id: esp::FormId| -> Option<()> {
                let r = lo.get(id)?;
                let flags = r.get(b"DNAM").filter(|d| d.len() >= 4).map_or(0, |d| d[0]);
                let text = |t: &[u8; 4]| r.get(t).map(|d| lo.lstring(&r, d)).unwrap_or_default();
                println!(
                    "{} {id} flags {flags:#x} time {:?} title {:?} quest {}",
                    r.editor_id().unwrap_or_default(),
                    r.get(b"TNAM")
                        .map(|d| u32::from_le_bytes(d[..4].try_into().unwrap())),
                    text(b"FULL"),
                    r.get(b"QNAM")
                        .map(|d| r.fid(esp::FormId(u32::from_le_bytes(d[..4].try_into().unwrap()))))
                        .and_then(|q| lo.get(q))
                        .and_then(|q| q.editor_id())
                        .unwrap_or_default()
                );
                println!("  {:?}", text(b"DESC"));
                let mut conds = 0;
                for sr in r.subrecords() {
                    match &sr.tag.0 {
                        b"ITXT" => {
                            println!("  button {:?}", lo.lstring(&r, sr.data));
                            conds = 0;
                        }
                        b"CTDA" => {
                            conds += 1;
                            println!("    condition {conds}");
                        }
                        _ => {}
                    }
                }
                Some(())
            };
            if let Some(name) = args.get(2) {
                let id = match u32::from_str_radix(name, 16) {
                    Ok(v) if name.len() == 8 => esp::FormId(v),
                    _ => lo.find_editor_id(name).context("editor id not found")?,
                };
                show(id).context("no record")?;
                return Ok(());
            }
            let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
            let mut n = 0;
            for &id in lo.ids_of_type(b"MESG") {
                let Some(r) = lo.get(id) else { continue };
                n += 1;
                let mut c = |k: String| *counts.entry(k).or_default() += 1;
                let flags = r.get(b"DNAM").filter(|d| d.len() >= 4).map_or(0, |d| d[0]);
                c(format!("flags {flags:#x}"));
                let buttons = r.subrecords().filter(|s| &s.tag.0 == b"ITXT").count();
                c(format!("buttons {}", buttons.min(5)));
                if r.subrecords().any(|s| &s.tag.0 == b"CTDA") {
                    c("conditions on buttons".into());
                }
                c(format!("owner quest {}", r.get(b"QNAM").is_some()));
                let desc = r
                    .get(b"DESC")
                    .map(|d| lo.lstring(&r, d))
                    .unwrap_or_default();
                // GREP=<text>: also show the messages whose text has it.
                if std::env::var("GREP").is_ok_and(|g| desc.contains(&g)) {
                    show(id);
                }
                let mut rest = desc.as_str();
                while let Some(i) = rest.find(['<', '%', '[']) {
                    rest = &rest[i..];
                    let end = match rest.as_bytes()[0] {
                        b'<' => rest.find('>').map(|e| rest[..e].find('=').unwrap_or(e)),
                        b'[' => rest.find(']').map(|e| e + 1),
                        _ => rest[1..]
                            .find(|ch: char| ch.is_ascii_alphabetic() || ch == '%')
                            .map(|e| e + 2),
                    }
                    .unwrap_or(1)
                    .min(24);
                    c(format!("tag {}", &rest[..end]));
                    rest = &rest[end.max(1)..];
                }
            }
            println!("{n} messages");
            for (k, v) in counts {
                println!("  {k}: {v}");
            }
        }
        Some("perks") => {
            // perks <data dir> [perk]: one perk's sections, or for every entry point
            // used: its functions, tab counts and the condition functions on each tab
            // (with how many perks use them), and the other section types.
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let lo = esp::LoadOrder::load(data, &names)?;
            // `ep <n>`: every perk with entry point n, in full.
            let ep_filter: Option<u8> = (args.get(2).map(String::as_str) == Some("ep"))
                .then(|| args.get(3).and_then(|n| n.parse().ok()))
                .flatten();
            let one = args.get(2).filter(|_| ep_filter.is_none()).map(|name| {
                match u32::from_str_radix(name, 16) {
                    Ok(v) if name.len() == 8 => Some(esp::FormId(v)),
                    _ => lo.find_editor_id(name),
                }
            });
            let ids: Vec<esp::FormId> = match one {
                Some(id) => vec![id.context("editor id not found")?],
                None => lo.ids_of_type(b"PERK").to_vec(),
            };
            let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
            for id in ids {
                let Some(r) = lo.get(id) else { continue };
                let edid = r.editor_id().unwrap_or_default();
                let one = one.is_some()
                    || ep_filter.is_some_and(|n| {
                        let mut kind = None;
                        r.subrecords().any(|sr| match &sr.tag.0 {
                            b"PRKE" => {
                                kind = Some(sr.u8(0));
                                false
                            }
                            b"DATA" => kind == Some(2) && sr.data.first() == Some(&n),
                            _ => false,
                        })
                    });
                let one = one.then_some(());
                if one.is_some() && ep_filter.is_some() {
                    for sr in r.subrecords().take_while(|sr| sr.tag.0 != *b"PRKE") {
                        if sr.tag.0 == *b"CTDA" && sr.data.len() >= 24 {
                            println!(
                                "{edid} {id}: needs {} {:08x} {:08x} op {} {}",
                                functions::name(sr.u16(8)),
                                sr.u32(12),
                                sr.u32(16),
                                sr.u8(0) >> 5,
                                sr.f32(4)
                            );
                        }
                    }
                }
                let mut c = |k: String| *counts.entry(k).or_default() += 1;
                let mut kind = None;
                let mut ep = 0u8;
                let mut tab = -1i32;
                for sr in r.subrecords() {
                    match &sr.tag.0 {
                        b"PRKE" => {
                            kind = Some(sr.u8(0));
                            tab = -1;
                            if kind != Some(2) {
                                c(format!("section type {}", sr.u8(0)));
                            }
                        }
                        b"DATA" if kind == Some(2) && sr.data.len() >= 3 => {
                            ep = sr.u8(0);
                            c(format!("ep {ep:3} fn {} tabs {}", sr.u8(1), sr.u8(2)));
                            if one.is_some() {
                                println!(
                                    "{edid}: entry point {ep} function {} tabs {}",
                                    sr.u8(1),
                                    sr.u8(2)
                                );
                            }
                        }
                        b"DATA" if kind.is_some() && one.is_some() => {
                            println!("{edid}: section {kind:?} data {:02x?}", sr.data)
                        }
                        b"PRKC" => tab = sr.u8(0) as i32,
                        b"CTDA" if kind == Some(2) && sr.data.len() >= 24 => {
                            let f = sr.u16(8);
                            c(format!(
                                "ep {ep:3} tab {tab} {} run_on {}",
                                functions::name(f),
                                sr.u32(20)
                            ));
                            if one.is_some() {
                                println!(
                                    "  tab {tab} {} {:08x} {:08x} op {} {}",
                                    functions::name(f),
                                    sr.u32(12),
                                    sr.u32(16),
                                    sr.u8(0) >> 5,
                                    sr.f32(4)
                                );
                            }
                        }
                        b"EPFT" => c(format!("ep {ep:3} param type {}", sr.u8(0))),
                        b"EPFD" if one.is_some() => println!("  EPFD {:02x?}", sr.data),
                        b"PRKF" => kind = None,
                        _ => {}
                    }
                }
            }
            for (k, n) in counts {
                println!("{n:5} {k}");
            }
        }
        Some("quest-log") => {
            // quest-log <data dir> [quest]: one quest's type, stages with their log
            // entries (flags, conditions, text) and objectives, or counts over all
            // quests: types, log entry flags, objective flags and the tags of their
            // text (`<Alias=...>`, `<Alias.ShortName=...>`, `<Global=...>`).
            let data = std::path::Path::new(&args[1]);
            let names = esp::LoadOrder::default_plugin_list(data, None);
            let mut lo = esp::LoadOrder::load(data, &names)?;
            let vfs = vfs::Vfs::new(data, &names);
            lo.load_strings("english", |p| vfs.read(p));
            let one = args.get(2).map(|name| match u32::from_str_radix(name, 16) {
                Ok(v) if name.len() == 8 => Some(esp::FormId(v)),
                _ => lo.find_editor_id(name),
            });
            let ids: Vec<esp::FormId> = match one {
                Some(id) => vec![id.context("editor id not found")?],
                None => lo.ids_of_type(b"QUST").to_vec(),
            };
            let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
            for id in ids {
                let Some(r) = lo.get(id) else { continue };
                let mut c = |k: String| *counts.entry(k).or_default() += 1;
                let qtype = r.get(b"DNAM").filter(|d| d.len() >= 12).map_or(0, |d| d[8]);
                c(format!("quest type {qtype}"));
                let tags = |text: &str, c: &mut dyn FnMut(String)| {
                    let mut rest = text;
                    while let Some(i) = rest.find('<') {
                        rest = &rest[i..];
                        let end = rest.find(['=', '>']).unwrap_or(rest.len()).min(32);
                        c(format!("tag {}", &rest[..end]));
                        rest = &rest[1..];
                    }
                };
                let full = r
                    .get(b"FULL")
                    .map(|d| lo.lstring(&r, d))
                    .unwrap_or_default();
                tags(&full, &mut c);
                if one.is_some() {
                    println!(
                        "{} {id} type {qtype} {full:?}",
                        r.editor_id().unwrap_or_default()
                    );
                }
                let mut conds = 0;
                let mut in_objectives = false;
                for sr in r.subrecords() {
                    match &sr.tag.0 {
                        b"INDX" => {
                            in_objectives = false;
                            if one.is_some() {
                                println!("  stage {} flags {:#x}", sr.u16(0), sr.u8(2));
                            }
                        }
                        b"QSDT" => {
                            c(format!("log entry flags {:#x}", sr.u8(0)));
                            conds = 0;
                            if one.is_some() {
                                println!("    entry flags {:#x}", sr.u8(0));
                            }
                        }
                        b"CTDA" => conds += 1,
                        b"CNAM" if !in_objectives => {
                            let text = lo.lstring(&r, sr.data);
                            tags(&text, &mut c);
                            c(format!("log entry with conditions: {}", conds > 0));
                            if one.is_some() {
                                println!("      ({conds} conditions) {text:?}");
                            }
                        }
                        b"QOBJ" => {
                            in_objectives = true;
                            if one.is_some() {
                                println!("  objective {}", sr.u16(0));
                            }
                        }
                        b"FNAM" if in_objectives && sr.data.len() >= 4 => {
                            c(format!("objective flags {:#x}", sr.u32(0)));
                        }
                        b"NNAM" if in_objectives => {
                            let text = lo.lstring(&r, sr.data);
                            tags(&text, &mut c);
                            if one.is_some() {
                                println!("    {text:?}");
                            }
                        }
                        b"QSTA" => c("objective targets".into()),
                        b"ALST" | b"ALLS" => in_objectives = false,
                        _ => {}
                    }
                }
            }
            if one.is_none() {
                for (k, v) in counts {
                    println!("  {k}: {v}");
                }
            }
        }
        _ => bail!("usage: vrm-tool <bsa-list|bsa-extract|bsa-verify> ..."),
    }
    Ok(())
}
