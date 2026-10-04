//! Sound descriptors (SNDR), sound markers (SOUN) and music (MUSC/MUST).

use esp::{FormId, LoadOrder};

#[derive(Debug, Clone)]
pub struct SoundDesc {
    pub files: Vec<String>,
    pub looping: bool,
    pub min_dist: f32,
    pub max_dist: f32,
    /// Linear volume from the static attenuation (dB) in BNAM.
    pub volume: f32,
}

/// Normalise a sound path from a record ("Data\\Sound\\FX\\x.wav") into a VFS path.
pub fn sound_path(s: &str, vfs: &vfs::Vfs) -> String {
    let n = vfs::normalize_path(s);
    let n = n.strip_prefix("data/").unwrap_or(&n).to_owned();
    let n = if n.starts_with("sound/") || n.starts_with("music/") { n } else { format!("sound/{n}") };
    // Records name .wav files that ship as .xwm.
    if !vfs.exists(&n)
        && let Some(stem) = n.strip_suffix(".wav")
    {
        let x = format!("{stem}.xwm");
        if vfs.exists(&x) {
            return x;
        }
    }
    n
}

/// Resolve a SNDR (or a SOUN pointing at one).
pub fn descriptor(lo: &LoadOrder, vfs: &vfs::Vfs, id: FormId) -> Option<SoundDesc> {
    let rec = lo.get(id)?;
    if rec.tag().0 == *b"SOUN" {
        let d = rec.get(b"SDSC")?;
        let target = rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().ok()?)));
        return descriptor(lo, vfs, target);
    }
    if rec.tag().0 != *b"SNDR" {
        return None;
    }
    let files: Vec<String> = rec.subrecords().filter(|s| s.tag.0 == *b"ANAM").map(|s| sound_path(&s.zstring(), vfs)).collect();
    let looping = rec.get(b"LNAM").is_some_and(|l| l.len() >= 2 && l[1] & 0x38 != 0);
    let mut desc = SoundDesc { files, looping, min_dist: 200.0, max_dist: 2000.0, volume: 1.0 };
    if let Some(b) = rec.get(b"BNAM")
        && b.len() >= 6
    {
        let db = u16::from_le_bytes([b[4], b[5]]) as f32 / 100.0;
        desc.volume = 10f32.powf(-db / 20.0);
    }
    if let Some(o) = rec.get(b"ONAM")
        && let Some(sopm) = lo.get(rec.fid(FormId(u32::from_le_bytes(o[0..4].try_into().ok()?))))
        && let Some(a) = sopm.get(b"ANAM")
        && a.len() >= 12
    {
        desc.min_dist = f32::from_le_bytes(a[4..8].try_into().unwrap());
        desc.max_dist = f32::from_le_bytes(a[8..12].try_into().unwrap());
    }
    if desc.files.is_empty() { None } else { Some(desc) }
}

pub const MUST_SINGLE: u32 = 0x6ED7_E048;
pub const MUST_PALETTE: u32 = 0x23F6_78C3;
pub const MUST_SILENT: u32 = 0xA1A9_C4D5;
