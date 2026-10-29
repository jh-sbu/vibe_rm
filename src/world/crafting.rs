//! Authored crafting recipes. Station keywords select recipes; CTDA controls
//! visibility, separately from whether the player has the required materials.
use std::collections::HashMap;

use esp::{FormId, LoadOrder, LoadedRecord};

use crate::condition::{self, Condition, Context};
use crate::engine::{Engine, PLAYER_REF};

#[derive(Debug, Clone)]
pub struct Recipe {
    pub id: FormId,
    pub inputs: Vec<(FormId, u32)>,
    pub conditions: Vec<Condition>,
    pub output: FormId,
    pub count: u16,
    pub bench: FormId,
}

fn form(r: &LoadedRecord<'_>, tag: &[u8; 4]) -> Option<FormId> {
    let d = r.get(tag)?.get(..4)?;
    let f = r.fid(FormId(u32::from_le_bytes(d.try_into().ok()?)));
    (!f.is_null()).then_some(f)
}

impl Recipe {
    pub fn parse(r: &LoadedRecord<'_>) -> Option<Self> {
        if r.tag().0 != *b"COBJ" {
            return None;
        }
        let mut inputs: Vec<(FormId, u32)> = Vec::new();
        for s in r.subrecords().filter(|s| s.tag.0 == *b"CNTO") {
            let d = s.data.get(..8)?;
            let item = r.fid(FormId(u32::from_le_bytes(d[..4].try_into().ok()?)));
            let count = u32::from_le_bytes(d[4..8].try_into().ok()?);
            if item.is_null() || count == 0 || count > i32::MAX as u32 {
                return None;
            }
            if let Some((_, n)) = inputs.iter_mut().find(|(f, _)| *f == item) {
                *n = n.checked_add(count).filter(|n| *n <= i32::MAX as u32)?;
            } else {
                inputs.push((item, count));
            }
        }
        let count = u16::from_le_bytes(r.get(b"NAM1")?.get(..2)?.try_into().ok()?);
        if count == 0 {
            return None;
        }
        Some(Self {
            id: r.form_id,
            inputs,
            conditions: condition::parse_all(r),
            output: form(r, b"CNAM")?,
            count,
            bench: form(r, b"BNAM")?,
        })
    }

    pub fn visible(&self, e: &Engine) -> bool {
        condition::evaluate(
            e,
            &self.conditions,
            Context {
                subject: Some(PLAYER_REF),
                target: Some(self.output),
                ..Default::default()
            },
        )
    }

    pub fn affordable(&self, inv: &super::inventory::Inventory) -> bool {
        self.inputs.iter().all(|&(f, n)| inv.count(f) >= n as i32)
    }
}

#[derive(Default)]
pub struct Recipes {
    pub by_bench: HashMap<FormId, Vec<Recipe>>,
}

impl Recipes {
    pub fn load(lo: &LoadOrder) -> Self {
        let mut out = Self::default();
        for &id in lo.ids_of_type(b"COBJ") {
            if let Some(r) = lo.get(id).as_ref().and_then(Recipe::parse) {
                out.by_bench.entry(r.bench).or_default().push(r);
            }
        }
        for recipes in out.by_bench.values_mut() {
            recipes.sort_by_key(|r| r.id);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp::writer::{Node, RecordBuilder, write_plugin};

    #[test]
    fn recipes_preserve_counts_conditions_and_combine_duplicate_inputs() {
        let dir = std::env::temp_dir().join(format!("vrm-crafting-records-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input = [0x11u32.to_le_bytes(), 2u32.to_le_bytes()].concat();
        let mut c = [0u8; 32];
        c[8..10].copy_from_slice(&448u16.to_le_bytes());
        c[12..16].copy_from_slice(&0x123u32.to_le_bytes());
        let r = RecordBuilder::new(b"COBJ", FormId(0x800))
            .field(b"CNTO", &input)
            .field(b"CNTO", &input)
            .field(b"CTDA", &c)
            .form(b"CNAM", FormId(0x22))
            .form(b"BNAM", FormId(0x33))
            .field(b"NAM1", &3u16.to_le_bytes());
        std::fs::write(
            dir.join("Test.esp"),
            write_plugin(
                0,
                &[],
                "",
                "",
                0x900,
                &[
                    Node::top(b"COBJ", vec![r]),
                    Node::top(
                        b"FURN",
                        vec![
                            RecordBuilder::new(b"FURN", FormId(0x801))
                                .field(b"WBDT", &[1, 17])
                                .form(b"KWDA", FormId(0x33)),
                            RecordBuilder::new(b"FURN", FormId(0x802)).field(b"WBDT", &[4, 255]),
                            RecordBuilder::new(b"FURN", FormId(0x803)).field(b"WBDT", &[0, 255]),
                            RecordBuilder::new(b"FURN", FormId(0x804)).field(b"WBDT", &[1]),
                        ],
                    ),
                ],
            ),
        )
        .unwrap();
        let lo = LoadOrder::load(&dir, &["Test.esp".into()]).unwrap();
        let station =
            super::super::records::Workbench::parse(&lo.get(FormId(0x801)).unwrap()).unwrap();
        assert_eq!(station.kind, 1);
        assert_eq!(station.skill, Some(17));
        assert_eq!(station.keywords, vec![FormId(0x33)]);
        assert_eq!(station.frame(), Some("ConstructibleObject"));
        let experiment =
            super::super::records::Workbench::parse(&lo.get(FormId(0x802)).unwrap()).unwrap();
        assert_eq!(experiment.skill, None);
        assert_eq!(experiment.frame(), None);
        for id in [0x803, 0x804] {
            assert!(
                super::super::records::Workbench::parse(&lo.get(FormId(id)).unwrap()).is_none()
            );
        }
        let index = Recipes::load(&lo);
        let r = &index.by_bench[&FormId(0x33)][0];
        assert_eq!(r.inputs, vec![(FormId(0x11), 4)]);
        assert_eq!((r.output, r.count), (FormId(0x22), 3));
        assert_eq!((r.conditions[0].func, r.conditions[0].p1), (448, 0x123));
        let mut inv = super::super::inventory::Inventory::default();
        inv.add(FormId(0x11), 3);
        assert!(!r.affordable(&inv));
        inv.add(FormId(0x11), 1);
        assert!(r.affordable(&inv));
        drop(lo);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    #[ignore = "requires installed Skyrim assets"]
    fn vanilla_recipe_survey() {
        let data = vfs::locate_data_dir().unwrap();
        let names: Vec<_> = esp::BASE_MASTERS.iter().map(|s| (*s).to_owned()).collect();
        let lo = LoadOrder::load(data, &names).unwrap();
        let recipes = Recipes::load(&lo);
        for (name, count) in [
            ("CraftingSmithingForge", 241),
            ("CraftingSmithingSharpeningWheel", 204),
            ("CraftingSmithingArmorTable", 313),
            ("DLC2StaffEnchanter", 43),
        ] {
            assert_eq!(
                recipes.by_bench[&lo.find_editor_id(name).unwrap()].len(),
                count,
                "{name}"
            );
        }
    }
}
