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
                    nif::Block::TriShape(t) => format!(
                        "TriShape name={} verts={} tris={} skin={:?} shader={:?} alpha={:?} desc={:016x} flags={:03x} xf={:?}",
                        t.av.net.name, t.geometry.positions.len(), t.geometry.triangles.len(), t.skin, t.shader, t.alpha,
                        t.vertex_desc, t.vertex_flags(), t.av.transform.translation
                    ),
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
                for (i, b) in s.bones.iter().enumerate().take(8) {
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
                for (i, q) in out.iter().enumerate().take(6) {
                    println!("  track {i}: t={:?} r={:?} |r|={:.4} s={:?}", q.translation, q.rotation, q.rotation.length(), q.scale);
                }
            }
        }
        _ => bail!("usage: vrm-tool <bsa-list|bsa-extract|bsa-verify> ..."),
    }
    Ok(())
}
