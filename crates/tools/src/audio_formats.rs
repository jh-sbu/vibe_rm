//! Bounded diagnostics for the lip research gate. Unknown track layouts stay unknown.
use anyhow::{Context, Result, ensure};
fn word(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}
fn lip_payload(d: &[u8]) -> Result<&[u8]> {
    ensure!(d.get(..4) == Some(b"FUZE"), "not a FUZE container");
    ensure!(word(d, 4) == Some(1), "unsupported FUZE version");
    let size = word(d, 8).context("truncated FUZE header")? as usize;
    d.get(12..12usize.checked_add(size).context("lip size overflow")?)
        .context("lip chunk exceeds FUZE container")
}
fn diagnostics(d: &[u8]) {
    println!(
        "bytes={}; raw header words (interpretations only, not decoded fields):",
        d.len()
    );
    for at in (0..d.len().min(64)).step_by(4) {
        if let Some(v) = word(d, at) {
            println!("  {at:04X}: {v:08X} u32={v} f32={}", f32::from_bits(v));
        }
    }
    let mut names = 0;
    for (i, chunk) in d.split(|b| *b == 0).enumerate() {
        if chunk.len() >= 3
            && chunk.len() <= 64
            && chunk
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || b"_ -".contains(b))
            && chunk.iter().any(u8::is_ascii_alphabetic)
        {
            println!("  text[{i}] {:?}", String::from_utf8_lossy(chunk));
            names += 1;
            if names >= 80 {
                break;
            }
        }
    }
}
/// Each morph's name and largest offset component (`nif::tri`).
fn tri_summary(d: &[u8]) -> Result<Vec<(String, f32)>> {
    let tri = nif::tri::Tri::parse(d)?;
    Ok(tri
        .morphs
        .into_iter()
        .map(|m| {
            let peak = m
                .deltas
                .iter()
                .map(|v| v.abs().max_element())
                .fold(0.0, f32::max);
            (m.name, peak)
        })
        .collect())
}

pub fn run(kind: &str, data: &str, path: Option<&str>) -> Result<()> {
    let data = std::path::Path::new(data);
    let names = esp::LoadOrder::default_plugin_list(data, None);
    let v = vfs::Vfs::new(data, &names);
    let suffix = if kind == "lip-dump" { ".fuz" } else { ".tri" };
    let Some(path) = path else {
        let mut paths: Vec<_> = v
            .list(if suffix == ".fuz" {
                "sound/voice/"
            } else {
                "meshes/"
            })
            .into_iter()
            .filter(|p| p.ends_with(suffix))
            .collect();
        paths.sort();
        println!(
            "{} {suffix} assets; first 12 (pass a path to inspect):",
            paths.len()
        );
        for p in paths.iter().take(12) {
            println!("{p}");
        }
        return Ok(());
    };
    if kind == "tri-dump" && path == "--survey" {
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        let mut decoded = 0;
        let mut failed = 0;
        for path in v
            .list("meshes/")
            .into_iter()
            .filter(|p| p.ends_with(".tri"))
        {
            let Some(bytes) = v.read(&path) else { continue };
            match tri_summary(&bytes) {
                Ok(morphs) => {
                    decoded += 1;
                    for (name, _) in morphs {
                        *counts.entry(name).or_default() += 1;
                    }
                }
                Err(e) => {
                    failed += 1;
                    if failed <= 8 {
                        println!("unsupported {path}: {e}");
                    }
                }
            }
        }
        println!("decoded={decoded} unsupported={failed}; morph name counts: {counts:?}");
        return Ok(());
    }
    let bytes = v.read(path).context("asset not found")?;
    if kind == "lip-dump" {
        let lip = if path.to_ascii_lowercase().ends_with(".fuz") {
            let lip = lip_payload(&bytes)?;
            println!(
                "FUZE version=1 lip_bytes={} audio_bytes={}",
                lip.len(),
                bytes.len() - 12 - lip.len()
            );
            lip
        } else {
            bytes.as_slice()
        };
        diagnostics(lip);
        println!("LIP track layout is not decoded; no phoneme/morph mapping is inferred.");
    } else {
        println!(
            "TRI signature {:?}",
            String::from_utf8_lossy(bytes.get(..8).unwrap_or(&bytes))
        );
        diagnostics(&bytes);
        match tri_summary(&bytes) {
            Ok(morphs) => {
                println!(
                    "vertices={} triangles={} UVs={} morphs={}",
                    word(&bytes, 8).unwrap(),
                    word(&bytes, 12).unwrap(),
                    word(&bytes, 28).unwrap(),
                    morphs.len()
                );
                for (name, peak) in morphs {
                    println!("morph {name} peak_delta={peak}");
                }
            }
            Err(e) => println!("TRI morph layout remains unsupported: {e}"),
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tri_morph_diagnostics_require_a_complete_supported_layout() {
        let mut d = vec![0; 64];
        d[..8].copy_from_slice(b"FRTRI003");
        d[8..12].copy_from_slice(&1u32.to_le_bytes());
        d[36..40].copy_from_slice(&1u32.to_le_bytes());
        d.extend([0; 12]);
        d.extend(4u32.to_le_bytes());
        d.extend(b"Aah\0");
        d.extend(0.5f32.to_le_bytes());
        for value in [-2i16, 4, 0] {
            d.extend(value.to_le_bytes());
        }
        assert_eq!(tri_summary(&d).unwrap(), vec![("Aah".into(), 2.0)]);
        assert!(tri_summary(&d[..d.len() - 1]).is_err());
        d.push(0);
        assert!(tri_summary(&d).is_err());
    }
    #[test]
    fn fuz_lip_envelope_is_bounded_and_version_checked() {
        let mut d = b"FUZE".to_vec();
        d.extend(1u32.to_le_bytes());
        d.extend(3u32.to_le_bytes());
        d.extend(b"lipaudio");
        assert_eq!(lip_payload(&d).unwrap(), b"lip");
        d[8..12].copy_from_slice(&999u32.to_le_bytes());
        assert!(lip_payload(&d).is_err());
        assert!(lip_payload(b"FUZE").is_err());
        d[4..8].copy_from_slice(&2u32.to_le_bytes());
        assert!(lip_payload(&d).is_err());
    }
}
