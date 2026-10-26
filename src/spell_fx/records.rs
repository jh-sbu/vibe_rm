//! Serialized visual records. Field order checked against the public xEdit
//! TES5 definitions: https://raw.githubusercontent.com/TES5Edit/TES5Edit/dev-4.1.6/Core/wbDefinitionsTES5.pas
//! See known_ambiguities/spell-visuals.md and
//! known_ambiguities/effect-shaders.md for research and license context.
//! UESP EFSH revision 3089075: https://en.uesp.net/w/index.php?oldid=3089075
//! (its DATA layout is undocumented). No xEdit code is used.
#![allow(dead_code)]
use esp::{FormId, LoadOrder, LoadedRecord};

pub fn optional_u32(d: &[u8], offset: usize) -> u32 {
    d.get(offset..offset + 4)
        .map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()))
}
pub fn optional_float(d: &[u8], offset: usize) -> f32 {
    f32::from_bits(optional_u32(d, offset))
}
pub fn optional_form(rec: &LoadedRecord<'_>, d: &[u8], offset: usize) -> FormId {
    match optional_u32(d, offset) {
        0 => FormId::NULL,
        v => rec.fid(FormId(v)),
    }
}
fn text(rec: &LoadedRecord<'_>, tag: &[u8; 4], prefix: &str) -> Option<String> {
    let d = rec.get(tag)?;
    let s = String::from_utf8_lossy(d)
        .trim_end_matches('\0')
        .replace('\\', "/")
        .to_ascii_lowercase();
    if s.is_empty() {
        return None;
    }
    Some(if s.starts_with(prefix) {
        s
    } else {
        format!("{prefix}{s}")
    })
}
#[derive(Debug, Clone)]
pub struct ArtObject {
    pub model: Option<String>,
    /// 0 casting, 1 hit, 2 enchantment; preserve unknown values.
    pub kind: u32,
}
impl ArtObject {
    pub fn load(lo: &LoadOrder, id: FormId) -> Option<Self> {
        let rec = lo.get(id).filter(|r| r.tag().0 == *b"ARTO")?;
        Some(Self {
            model: text(&rec, b"MODL", "meshes/"),
            kind: optional_u32(rec.get(b"DNAM")?, 0),
        })
    }
}
#[derive(Debug, Clone)]
pub struct VisualEffect {
    pub art: FormId,
    pub shader: FormId,
    pub flags: u32,
}
impl VisualEffect {
    pub fn load(lo: &LoadOrder, id: FormId) -> Option<Self> {
        let r = lo.get(id).filter(|r| r.tag().0 == *b"RFCT")?;
        let d = r.get(b"DATA").filter(|d| d.len() >= 12)?;
        Some(Self {
            art: optional_form(&r, d, 0),
            shader: optional_form(&r, d, 4),
            flags: optional_u32(d, 8),
        })
    }
}
#[derive(Debug, Clone)]
pub struct DualCastData {
    pub projectile: FormId,
    pub explosion: FormId,
    pub shader: FormId,
    pub hit_art: FormId,
    pub impacts: FormId,
    pub inherit_scale: u32,
}
impl DualCastData {
    pub fn load(lo: &LoadOrder, id: FormId) -> Option<Self> {
        let r = lo.get(id).filter(|r| r.tag().0 == *b"DUAL")?;
        let d = r.get(b"DATA").filter(|d| d.len() >= 24)?;
        Some(Self {
            projectile: optional_form(&r, d, 0),
            explosion: optional_form(&r, d, 4),
            shader: optional_form(&r, d, 8),
            hit_art: optional_form(&r, d, 12),
            impacts: optional_form(&r, d, 16),
            inherit_scale: optional_u32(d, 20),
        })
    }
}
/// Complete serialized DATA, including optional older tails and unknown bits.
/// Accessors never read beyond the actual record; absent fields are zero.
#[derive(Debug, Clone)]
pub struct EffectShader {
    pub textures: [Option<String>; 5],
    pub data: Vec<u8>,
    pub addon_models: FormId,
    pub ambient_sound: FormId,
}
impl EffectShader {
    pub fn load(lo: &LoadOrder, id: FormId) -> Option<Self> {
        let r = lo.get(id).filter(|r| r.tag().0 == *b"EFSH")?;
        let d = r.get(b"DATA")?;
        Some(Self {
            textures: [b"ICON", b"ICO2", b"NAM7", b"NAM8", b"NAM9"]
                .map(|t| text(&r, t, "textures/")),
            data: d.to_vec(),
            addon_models: optional_form(&r, d, 0xF4),
            ambient_sound: optional_form(&r, d, 0x134),
        })
    }
    pub fn u32(&self, offset: usize) -> u32 {
        optional_u32(&self.data, offset)
    }
    pub fn float(&self, offset: usize) -> f32 {
        optional_float(&self.data, offset)
    }
    pub fn flags(&self) -> u32 {
        if self.data.len() >= 0x184 {
            self.u32(0x180)
        } else {
            self.data.first().copied().unwrap_or(0) as u32
        }
    }
    pub fn color(&self, offset: usize) -> [f32; 3] {
        std::array::from_fn(|i| self.data.get(offset + i).copied().unwrap_or(0) as f32 / 255.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp::writer::{Node, RecordBuilder, write_plugin};
    #[test]
    fn records_remap_references_and_tolerate_old_shader_tails() {
        let dir = std::env::temp_dir().join(format!(
            "vrm-spell-records-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let fields = |values: &[u32]| {
            values
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<_>>()
        };
        let mut shader = vec![0; 400];
        shader[0xF4..0xF8].copy_from_slice(&0x810u32.to_le_bytes());
        shader[0x134..0x138].copy_from_slice(&0x811u32.to_le_bytes());
        shader[0x180..0x184].copy_from_slice(&0x10400u32.to_le_bytes());
        let groups = [
            Node::top(
                b"ARTO",
                vec![
                    RecordBuilder::new(b"ARTO", FormId(0x800))
                        .zstring(b"MODL", "Magic\\Test.nif")
                        .u32(b"DNAM", 2),
                ],
            ),
            Node::top(
                b"RFCT",
                vec![
                    RecordBuilder::new(b"RFCT", FormId(0x801))
                        .field(b"DATA", &fields(&[0x800, 0x803, 7])),
                ],
            ),
            Node::top(
                b"DUAL",
                vec![
                    RecordBuilder::new(b"DUAL", FormId(0x802))
                        .field(b"DATA", &fields(&[0x810, 0, 0x803, 0x800, 0x811, 3])),
                ],
            ),
            Node::top(
                b"EFSH",
                vec![
                    RecordBuilder::new(b"EFSH", FormId(0x803))
                        .field(b"DATA", &shader)
                        .zstring(b"ICON", "Effects\\Fill.dds"),
                    RecordBuilder::new(b"EFSH", FormId(0x804)).field(b"DATA", &[8]),
                    RecordBuilder::new(b"EFSH", FormId(0x805)).field(b"DATA", &[]),
                ],
            ),
        ];
        std::fs::write(
            dir.join("Empty.esp"),
            write_plugin(0, &[], "test", "", 0x800, &[]),
        )
        .unwrap();
        std::fs::write(
            dir.join("Fx.esp"),
            write_plugin(0, &[], "test", "", 0x900, &groups),
        )
        .unwrap();
        let lo = LoadOrder::load(&dir, &["Empty.esp".into(), "Fx.esp".into()]).unwrap();
        let art = ArtObject::load(&lo, FormId(0x01000800)).unwrap();
        assert_eq!(art.model.as_deref(), Some("meshes/magic/test.nif"));
        assert_eq!(art.kind, 2);
        let rfct = VisualEffect::load(&lo, FormId(0x01000801)).unwrap();
        assert_eq!(rfct.art, FormId(0x01000800));
        assert_eq!(rfct.shader, FormId(0x01000803));
        assert_eq!(rfct.flags, 7);
        let dual = DualCastData::load(&lo, FormId(0x01000802)).unwrap();
        assert_eq!(dual.explosion, FormId::NULL);
        assert_eq!(dual.projectile, FormId(0x01000810));
        assert_eq!(dual.shader, rfct.shader);
        assert_eq!(dual.hit_art, rfct.art);
        assert_eq!(dual.impacts, FormId(0x01000811));
        assert_eq!(dual.inherit_scale, 3);
        let efsh = EffectShader::load(&lo, FormId(0x01000803)).unwrap();
        assert_eq!(
            efsh.textures[0].as_deref(),
            Some("textures/effects/fill.dds")
        );
        assert_eq!(efsh.addon_models, FormId(0x01000810));
        assert_eq!(efsh.ambient_sound, FormId(0x01000811));
        assert_eq!(efsh.flags(), 0x10400);
        for (id, flags) in [(0x01000804, 8), (0x01000805, 0)] {
            let old = EffectShader::load(&lo, FormId(id)).unwrap();
            assert_eq!(old.flags(), flags);
            assert_eq!(old.float(0x188), 0.0);
            assert_eq!(old.addon_models, FormId::NULL);
        }
        assert!(ArtObject::load(&lo, FormId(0x01000803)).is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
