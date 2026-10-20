//! `DEST` destructible-object definitions.

use crate::{FormId, LoadedRecord, Record};

/// The data shared by references of a destructible base object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DestructibleData {
    pub health: u32,
    pub stage_count: u8,
    pub vats_targetable: bool,
    pub stages: Vec<DestructionStage>,
}

/// One `DSTD` stage and its optional replacement model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DestructionStage {
    pub health_percent: u8,
    pub index: u8,
    pub damage_stage: u8,
    pub flags: u8,
    pub self_damage_per_second: u32,
    pub explosion: FormId,
    pub debris: FormId,
    pub debris_count: u32,
    pub replacement_model: Option<String>,
}

impl Record<'_> {
    /// Read the `DEST` definition, leaving form fields in this plugin's local ID space.
    pub fn destructible_data(&self) -> Option<DestructibleData> {
        let mut definition = None;
        let mut stage: Option<DestructionStage> = None;

        let finish_stage = |stage: &mut Option<DestructionStage>,
                            definition: &mut Option<DestructibleData>| {
            if let (Some(stage), Some(definition)) = (stage.take(), definition.as_mut()) {
                definition.stages.push(stage);
            }
        };

        for subrecord in self.subrecords() {
            match &subrecord.tag.0 {
                b"DEST" => {
                    if subrecord.data.len() < 8 {
                        return None;
                    }
                    finish_stage(&mut stage, &mut definition);
                    definition = Some(DestructibleData {
                        health: subrecord.u32(0),
                        stage_count: subrecord.u8(4),
                        vats_targetable: subrecord.u8(5) & 0x01 != 0,
                        stages: Vec::with_capacity(subrecord.u8(4) as usize),
                    });
                }
                b"DSTD" => {
                    finish_stage(&mut stage, &mut definition);
                    if definition
                        .as_ref()
                        .is_some_and(|d| d.stages.len() < d.stage_count as usize)
                        && subrecord.data.len() >= 20
                    {
                        let explosion = subrecord.form_id(8);
                        let debris = subrecord.form_id(12);
                        stage = Some(DestructionStage {
                            health_percent: subrecord.u8(0),
                            index: subrecord.u8(1),
                            damage_stage: subrecord.u8(2),
                            flags: subrecord.u8(3),
                            self_damage_per_second: subrecord.u32(4),
                            explosion,
                            debris,
                            debris_count: subrecord.u32(16),
                            replacement_model: None,
                        });
                    }
                }
                b"DMDL" => {
                    if let Some(stage) = stage.as_mut() {
                        let path = subrecord.zstring();
                        if !path.is_empty() {
                            stage.replacement_model = Some(path);
                        }
                    }
                }
                b"DSTF" => finish_stage(&mut stage, &mut definition),
                _ => {}
            }
        }
        finish_stage(&mut stage, &mut definition);
        definition
    }
}

impl LoadedRecord<'_> {
    /// Read the base object's `DEST` definition, globalizing its form fields.
    pub fn destructible_data(&self) -> Option<DestructibleData> {
        let mut definition = self.record.destructible_data()?;
        for stage in &mut definition.stages {
            if !stage.explosion.is_null() {
                stage.explosion = self.fid(stage.explosion);
            }
            if !stage.debris.is_null() {
                stage.debris = self.fid(stage.debris);
            }
        }
        Some(definition)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RecordHeader, Tag};
    use std::borrow::Cow;

    fn subrecord(tag: &[u8; 4], data: &[u8], out: &mut Vec<u8>) {
        out.extend_from_slice(tag);
        out.extend_from_slice(&(data.len() as u16).to_le_bytes());
        out.extend_from_slice(data);
    }

    fn record(data: Vec<u8>) -> Record<'static> {
        Record {
            header: RecordHeader {
                tag: Tag(*b"ACTI"),
                data_size: data.len() as u32,
                flags: 0,
                form_id: FormId(0x800),
                version: 0,
            },
            data: Cow::Owned(data),
        }
    }

    fn stage(health: u8, index: u8, damage: u8, flags: u8, explosion: u32) -> Vec<u8> {
        let mut d = vec![health, index, damage, flags];
        d.extend_from_slice(&12u32.to_le_bytes());
        d.extend_from_slice(&explosion.to_le_bytes());
        d.extend_from_slice(&0u32.to_le_bytes()); // debris
        d.extend_from_slice(&3u32.to_le_bytes());
        d
    }

    #[test]
    fn parses_dest_stages_and_replacement_model_without_crossing_stage_boundary() {
        let mut data = Vec::new();
        let mut dest = Vec::new();
        dest.extend_from_slice(&100u32.to_le_bytes());
        dest.extend_from_slice(&[2, 1, 0, 0]); // count, VATS flag, padding
        subrecord(b"DEST", &dest, &mut data);
        subrecord(b"DSTD", &stage(99, 0, 1, 1, 0x1234), &mut data);
        subrecord(b"DMDL", b"meshes\\broken.nif\0", &mut data);
        subrecord(b"DSTF", &[], &mut data);
        subrecord(b"DSTD", &stage(0, 1, 2, 6, 0x5678), &mut data);
        subrecord(b"DSTF", &[], &mut data);
        subrecord(b"DSTD", &stage(50, 2, 3, 0, 0), &mut data); // Beyond DEST count.
        subrecord(b"DSTF", &[], &mut data);

        let parsed = record(data).destructible_data().unwrap();
        assert_eq!(parsed.health, 100);
        assert_eq!(parsed.stage_count, 2);
        assert!(parsed.vats_targetable);
        assert_eq!(parsed.stages.len(), 2);
        assert_eq!(parsed.stages[0].health_percent, 99);
        assert_eq!(parsed.stages[0].index, 0);
        assert_eq!(parsed.stages[0].damage_stage, 1);
        assert_eq!(parsed.stages[0].flags, 1);
        assert_eq!(parsed.stages[0].self_damage_per_second, 12);
        assert_eq!(parsed.stages[0].explosion, FormId(0x1234));
        assert_eq!(parsed.stages[0].debris_count, 3);
        assert_eq!(
            parsed.stages[0].replacement_model.as_deref(),
            Some("meshes\\broken.nif")
        );
        assert_eq!(parsed.stages[1].replacement_model, None);
        assert_eq!(parsed.stages[1].explosion, FormId(0x5678));
    }

    #[test]
    fn malformed_dest_and_short_stages_are_bounded() {
        let mut data = Vec::new();
        subrecord(b"DEST", &[1, 2, 3], &mut data);
        assert!(record(data).destructible_data().is_none());

        let mut data = Vec::new();
        subrecord(b"DEST", &[100, 0, 0, 0, 1, 0, 0, 0], &mut data);
        subrecord(b"DSTD", &[99, 0, 1], &mut data);
        subrecord(b"DSTF", &[], &mut data);
        assert!(record(data).destructible_data().unwrap().stages.is_empty());
    }
}
