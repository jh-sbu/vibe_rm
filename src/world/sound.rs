//! Sound descriptors (SNDR), sound markers (SOUN) and music (MUSC/MUST).

use esp::{FormId, LoadOrder};

#[derive(Debug, Clone)]
pub struct SoundDesc {
    pub files: Vec<String>,
    pub looping: bool,
    pub min_dist: f32,
    pub max_dist: f32,
    /// Linear volume from the static attenuation (dB) in BNAM, times the
    /// sound's category's ([`category_volume`]).
    pub volume: f32,
    /// `BNAM`'s frequency shift (percent, signed), frequency variance
    /// (percent) and dB variance: how each playing differs from the recording.
    pub frequency_shift: f32,
    pub frequency_variance: f32,
    pub db_variance: f32,
}

/// A number in [0, 1) from the top bits of a random 64-bit one.
pub fn unit(r: u64) -> f32 {
    (r >> 40) as f32 / (1u64 << 24) as f32
}

/// What one playing of a descriptor changes in its recording: a gain and a
/// playback rate (both 1 for none).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Variation {
    pub gain: f32,
    pub pitch: f32,
}

impl SoundDesc {
    /// The variation for two random numbers in [0, 1): the playback rate is
    /// 1 plus the frequency shift plus up to the variance either way
    /// (percent), the gain up to the dB variance either way. Whether the
    /// variances are one-sided, or measured so, isn't public.
    pub fn variation(&self, roll: [f32; 2]) -> Variation {
        let signed = |r: f32| r.clamp(0.0, 1.0) * 2.0 - 1.0;
        Variation {
            gain: 10f32.powf(signed(roll[1]) * self.db_variance / 20.0),
            pitch: (1.0
                + (self.frequency_shift + signed(roll[0]) * self.frequency_variance) / 100.0)
                .max(0.1),
        }
    }
}

/// A sound category (SNCT): the broadest group a sound belongs to, itself in a
/// parent category up to `_AudioCategoryMaster`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Category {
    pub parent: Option<FormId>,
    /// The static volume multiplier (`VNAM`).
    pub volume: f32,
    /// The Audio menu's default for a category that has a slider there
    /// (`FNAM` bit 2, `UNAM`), else 1.
    pub menu: f32,
}

/// `FNAM` bit: the category has a slider in the Audio menu.
const CATEGORY_IN_MENU: u32 = 0x02;

/// A category's 16-bit float: 0 is 0.0 and 0xFFFF is 1.0, linearly between
/// (UESP's SNCT page).
fn category_float(v: u16) -> f32 {
    v as f32 / 65535.0
}

fn category(lo: &LoadOrder, id: FormId) -> Option<Category> {
    let rec = lo.get(id).filter(|r| r.tag().0 == *b"SNCT")?;
    let u16_of = |tag: &[u8; 4]| {
        rec.get(tag)
            .filter(|d| d.len() >= 2)
            .map(|d| u16::from_le_bytes([d[0], d[1]]))
    };
    let flags = rec
        .get(b"FNAM")
        .filter(|d| d.len() >= 4)
        .map_or(0, |d| u32::from_le_bytes(d[0..4].try_into().unwrap()));
    Some(Category {
        parent: rec
            .get(b"PNAM")
            .filter(|d| d.len() >= 4)
            .map(|d| rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
            .filter(|p| !p.is_null()),
        volume: u16_of(b"VNAM").map_or(1.0, category_float),
        menu: if flags & CATEGORY_IN_MENU != 0 {
            u16_of(b"UNAM").map_or(1.0, category_float)
        } else {
            1.0
        },
    })
}

/// What a category scales its sounds by at the Audio menu's defaults: its
/// volume and menu default times each parent's up to the master category.
pub fn category_volume(lo: &LoadOrder, id: FormId) -> f32 {
    chain_volume(id, |c| category(lo, c))
}

/// [`category_volume`] over any source of categories (a cycle, which no
/// record has, ends the walk).
fn chain_volume(id: FormId, get: impl Fn(FormId) -> Option<Category>) -> f32 {
    let mut volume = 1.0;
    let mut at = Some(id);
    let mut seen = Vec::new();
    while let Some(c) = at.filter(|c| !seen.contains(c)) {
        seen.push(c);
        let Some(cat) = get(c) else { break };
        volume *= cat.volume * cat.menu;
        at = cat.parent;
    }
    volume
}

/// The volume of the category named `edid` (`AudioCategoryMUS`...), 1 if the
/// data has no such category.
pub fn named_category_volume(lo: &LoadOrder, edid: &str) -> f32 {
    lo.find_editor_id(edid)
        .map_or(1.0, |c| category_volume(lo, c))
}

/// Dialogue lines have no sound descriptor: they're the voice category's.
pub fn voice_volume(lo: &LoadOrder) -> f32 {
    named_category_volume(lo, "AudioCategoryVOC")
}

/// Music tracks have no sound descriptor either: they're the music category's.
pub fn music_volume(lo: &LoadOrder) -> f32 {
    named_category_volume(lo, "AudioCategoryMUS")
}

/// Normalise a sound path from a record ("Data\\Sound\\FX\\x.wav") into a VFS path.
pub fn sound_path(s: &str, vfs: &vfs::Vfs) -> String {
    let n = vfs::normalize_path(s);
    let n = n.strip_prefix("data/").unwrap_or(&n).to_owned();
    let n = if n.starts_with("sound/") || n.starts_with("music/") {
        n
    } else {
        format!("sound/{n}")
    };
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
    let files: Vec<String> = rec
        .subrecords()
        .filter(|s| s.tag.0 == *b"ANAM")
        .map(|s| sound_path(&s.zstring(), vfs))
        .collect();
    let looping = rec
        .get(b"LNAM")
        .is_some_and(|l| l.len() >= 2 && l[1] & 0x38 != 0);
    let mut desc = SoundDesc {
        files,
        looping,
        min_dist: 200.0,
        max_dist: 2000.0,
        volume: 1.0,
        frequency_shift: 0.0,
        frequency_variance: 0.0,
        db_variance: 0.0,
    };
    if let Some(b) = rec.get(b"BNAM")
        && b.len() >= 6
    {
        let db = u16::from_le_bytes([b[4], b[5]]) as f32 / 100.0;
        desc.volume = 10f32.powf(-db / 20.0);
        desc.frequency_shift = b[0] as i8 as f32;
        desc.frequency_variance = b[1] as f32;
        desc.db_variance = b[3] as f32;
    }
    // The category (GNAM) scales it, with the categories above it.
    if let Some(g) = rec.get(b"GNAM")
        && g.len() >= 4
    {
        let c = rec.fid(FormId(u32::from_le_bytes(g[0..4].try_into().unwrap())));
        desc.volume *= category_volume(lo, c);
    }
    if let Some(o) = rec.get(b"ONAM")
        && let Some(sopm) = lo.get(rec.fid(FormId(u32::from_le_bytes(o[0..4].try_into().ok()?))))
        && let Some(a) = sopm.get(b"ANAM")
        && a.len() >= 12
    {
        desc.min_dist = f32::from_le_bytes(a[4..8].try_into().unwrap());
        desc.max_dist = f32::from_le_bytes(a[8..12].try_into().unwrap());
    }
    if desc.files.is_empty() {
        None
    } else {
        Some(desc)
    }
}

pub const MUST_SINGLE: u32 = 0x6ED7_E048;
pub const MUST_PALETTE: u32 = 0x23F6_78C3;
pub const MUST_SILENT: u32 = 0xA1A9_C4D5;

#[cfg(test)]
mod tests {
    use super::*;
    use esp::writer::{Node, RecordBuilder, write_plugin};
    use std::collections::HashMap;

    fn cat(parent: Option<u32>, volume: f32, menu: f32) -> Category {
        Category {
            parent: parent.map(FormId),
            volume,
            menu,
        }
    }

    #[test]
    fn a_category_scales_by_every_category_above_it() {
        let cats: HashMap<u32, Category> = [
            (1, cat(None, 0.9, 1.0)),
            (2, cat(Some(1), 0.9, 0.5)),
            (3, cat(Some(2), 0.5, 1.0)),
        ]
        .into();
        let get = |id: FormId| cats.get(&id.0).copied();
        assert!((chain_volume(FormId(1), get) - 0.9).abs() < 1e-6);
        assert!((chain_volume(FormId(2), get) - 0.9 * 0.9 * 0.5).abs() < 1e-6);
        assert!((chain_volume(FormId(3), get) - 0.9 * 0.9 * 0.5 * 0.5).abs() < 1e-6);
        // Not a category: unscaled.
        assert_eq!(chain_volume(FormId(9), get), 1.0);
    }

    #[test]
    fn a_loop_of_parents_is_walked_once() {
        let cats: HashMap<u32, Category> =
            [(1, cat(Some(2), 0.5, 1.0)), (2, cat(Some(1), 0.5, 1.0))].into();
        assert_eq!(chain_volume(FormId(1), |id| cats.get(&id.0).copied()), 0.25);
    }

    fn varying(shift: f32, variance: f32, db: f32) -> SoundDesc {
        SoundDesc {
            files: vec![],
            looping: false,
            min_dist: 0.0,
            max_dist: 0.0,
            volume: 1.0,
            frequency_shift: shift,
            frequency_variance: variance,
            db_variance: db,
        }
    }

    #[test]
    fn variation_spans_the_variances_around_the_shift() {
        let d = varying(10.0, 8.0, 6.0);
        let low = d.variation([0.0, 0.0]);
        let mid = d.variation([0.5, 0.5]);
        let high = d.variation([1.0, 1.0]);
        // Rate: +10% shift, plus or minus 8%.
        assert!((low.pitch - 1.02).abs() < 1e-5);
        assert!((mid.pitch - 1.10).abs() < 1e-5);
        assert!((high.pitch - 1.18).abs() < 1e-5);
        // Gain: up to 6 dB either way.
        assert!((mid.gain - 1.0).abs() < 1e-5);
        assert!((high.gain - 10f32.powf(6.0 / 20.0)).abs() < 1e-4);
        assert!((low.gain - 10f32.powf(-6.0 / 20.0)).abs() < 1e-4);
        // A descriptor with none plays as recorded; shifts can be negative.
        assert_eq!(
            varying(0.0, 0.0, 0.0).variation([0.3, 0.9]),
            Variation {
                gain: 1.0,
                pitch: 1.0
            }
        );
        assert!((varying(-10.0, 0.0, 0.0).variation([0.7, 0.7]).pitch - 0.9).abs() < 1e-5);
    }

    #[test]
    fn category_floats_run_from_zero_to_one() {
        assert_eq!(category_float(0), 0.0);
        assert_eq!(category_float(0xFFFF), 1.0);
        // The values the base game uses (UESP's SNCT page: 0.9, 0.7, 0.5, 0.8).
        for (raw, want) in [(0xE665, 0.9), (0xB332, 0.7), (0x7FFF, 0.5), (0xCCCC, 0.8)] {
            assert!((category_float(raw) - want).abs() < 1e-4);
        }
    }

    #[test]
    fn descriptors_take_their_categories_volume_at_the_menus_defaults() {
        let dir = std::env::temp_dir().join(format!("vrm-sound-categories-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let snct = |id: u32, parent: Option<u32>, flags: u32, volume: u16, menu: Option<u16>| {
            let mut r = RecordBuilder::new(b"SNCT", FormId(id)).u32(b"FNAM", flags);
            if let Some(p) = parent {
                r = r.u32(b"PNAM", p);
            }
            r = r.field(b"VNAM", &volume.to_le_bytes());
            match menu {
                Some(m) => r.field(b"UNAM", &m.to_le_bytes()),
                None => r,
            }
        };
        let sndr = |id: u32, category: u32, db_times_100: u16| {
            // A shift of -5%, variance 3%, priority, dB variance 6.
            let mut bnam = [(-5i8) as u8, 3, 128, 6, 0, 0];
            bnam[4..6].copy_from_slice(&db_times_100.to_le_bytes());
            RecordBuilder::new(b"SNDR", FormId(id))
                .u32(b"GNAM", category)
                .zstring(b"ANAM", "fx/test.wav")
                .field(b"BNAM", &bnam)
        };
        let records = [
            Node::top(
                b"SNCT",
                vec![
                    snct(0x800, None, 0, 0xE665, None),
                    snct(0x801, Some(0x800), 2, 0xE665, Some(0x7FFF)),
                ],
            ),
            Node::top(
                b"SNDR",
                vec![sndr(0x810, 0x801, 0), sndr(0x811, 0x801, 600)],
            ),
        ];
        std::fs::write(
            dir.join("Sounds.esp"),
            write_plugin(0, &[], "test", "synthetic", 0x820, &records),
        )
        .unwrap();
        let names = vec!["Sounds.esp".to_string()];
        let lo = LoadOrder::load(&dir, &names).unwrap();
        let vfs = vfs::Vfs::new(&dir, &names);
        let _ = std::fs::remove_dir_all(&dir);
        let near = |a: f32, b: f32| (a - b).abs() < 1e-3;
        // The menu category: its multiplier and its default, then the master's.
        let want = 0.9 * 0.5 * 0.9;
        let d = descriptor(&lo, &vfs, FormId(0x810)).unwrap();
        assert!(near(d.volume, want), "{}", d.volume);
        assert_eq!(
            (d.frequency_shift, d.frequency_variance, d.db_variance),
            (-5.0, 3.0, 6.0)
        );
        // Static attenuation of 6 dB stacks on top.
        let d = descriptor(&lo, &vfs, FormId(0x811)).unwrap();
        assert!(
            near(d.volume, want * 10f32.powf(-6.0 / 20.0)),
            "{}",
            d.volume
        );
        assert!(near(named_category_volume(&lo, "AudioCategoryNone"), 1.0));
        assert!(near(category_volume(&lo, FormId(0x800)), 0.9));
    }
}
