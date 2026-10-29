//! Station recipe queries and crafting transactions.
use crate::engine::Engine;
use crate::world::crafting::Recipe;
use crate::world::inventory::item_info;
use esp::FormId;

impl Engine {
    pub(crate) fn recipes_at(&self, station: FormId) -> Vec<Recipe> {
        let Some(bench) = self
            .base_of(station)
            .and_then(|f| self.lo.get(f))
            .as_ref()
            .and_then(crate::world::records::Workbench::parse)
        else {
            return Vec::new();
        };
        let mut recipes: Vec<_> = bench
            .keywords
            .iter()
            .filter_map(|k| self.recipes.by_bench.get(k))
            .flatten()
            .filter(|r| r.visible(self) && item_info(&self.lo, r.output).is_some())
            .cloned()
            .collect();
        recipes.sort_by_key(|r| r.id);
        recipes.dedup_by_key(|r| r.id);
        recipes
    }
    /// The movie's material filter flags are bit positions, not item categories.
    pub(crate) fn crafting_category(&self, item: FormId) -> (String, u32) {
        const MATERIALS: &[&str] = &[
            "Wood", "Hide", "Draugr", "Iron", "Studded", "Imperial", "Steel", "Leather", "Dwarven",
            "Elven", "Falmer", "Orcish", "Ebony", "Glass", "Dragon", "Daedric", "Jewelry", "Misc",
        ];
        let keywords = self
            .lo
            .get(item)
            .and_then(|r| {
                r.get(b"KWDA").map(|d| {
                    d.chunks_exact(4)
                        .map(|b| r.fid(FormId(u32::from_le_bytes(b.try_into().unwrap()))))
                        .collect::<Vec<_>>()
                })
            })
            .unwrap_or_default();
        for k in keywords {
            let Some(r) = self.lo.get(k) else {
                continue;
            };
            let Some(edid) = r.editor_id() else {
                continue;
            };
            let lower = edid.to_ascii_lowercase();
            if let Some(index) = lower
                .strip_prefix("genericcraftablekeyword")
                .and_then(|n| n.parse::<u32>().ok())
                .filter(|n| (1..=8).contains(n))
            {
                let name = self
                    .gmst_string(&format!("sGenericCraftKeywordName{index:02}"))
                    .unwrap_or(edid);
                return (name, 1 << (17 + index));
            }
            for (i, material) in MATERIALS.iter().enumerate() {
                if lower.contains("material") && lower.ends_with(&material.to_ascii_lowercase())
                    || *material == "Jewelry" && lower == "vendoritemjewelry"
                {
                    return (
                        r.get(b"FULL")
                            .map(|d| self.lo.lstring(&r, d))
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| (*material).into()),
                        1 << i,
                    );
                }
            }
            for (i, material) in ["Bonemold", "Chitin", "Nordic", "Stalhrim"]
                .iter()
                .enumerate()
            {
                if lower.contains("material") && lower.contains(&material.to_ascii_lowercase()) {
                    return ((*material).into(), 1 << (26 + i));
                }
            }
        }
        (
            self.gmst_string("sMisc").unwrap_or_else(|| "Misc".into()),
            1 << 17,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::PLAYER_REF;
    use esp::writer::{Node, RecordBuilder, write_plugin};
    pub(super) fn fixture() -> (std::path::PathBuf, Engine) {
        let dir = std::env::temp_dir().join(format!(
            "vrm-crafting-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let data = [10u32.to_le_bytes(), 1f32.to_le_bytes()].concat();
        let input = [0x802u32.to_le_bytes(), 2u32.to_le_bytes()].concat();
        let recipe = |id| {
            RecordBuilder::new(b"COBJ", FormId(id))
                .field(b"CNTO", &input)
                .form(b"CNAM", FormId(0x803))
                .form(b"BNAM", FormId(0x801))
                .field(b"NAM1", &2u16.to_le_bytes())
        };
        let mut condition = [0u8; 32];
        condition[4..8].copy_from_slice(&1f32.to_le_bytes());
        condition[8..10].copy_from_slice(&448u16.to_le_bytes());
        condition[12..16].copy_from_slice(&0x820u32.to_le_bytes());
        let nodes = [
            Node::top(
                b"FURN",
                vec![
                    RecordBuilder::new(b"FURN", FormId(0x800))
                        .field(b"WBDT", &[1, 17])
                        .form(b"KWDA", FormId(0x801)),
                ],
            ),
            Node::top(
                b"KYWD",
                vec![
                    RecordBuilder::new(b"KYWD", FormId(0x801))
                        .zstring(b"EDID", "CraftingSmithingForge"),
                ],
            ),
            Node::top(
                b"REFR",
                vec![RecordBuilder::new(b"REFR", FormId(0x810)).form(b"NAME", FormId(0x800))],
            ),
            Node::top(
                b"MISC",
                vec![
                    RecordBuilder::new(b"MISC", FormId(0x802))
                        .zstring(b"FULL", "Material")
                        .field(b"DATA", &data),
                    RecordBuilder::new(b"MISC", FormId(0x803))
                        .zstring(b"FULL", "Result")
                        .field(b"DATA", &data),
                ],
            ),
            Node::top(
                b"COBJ",
                vec![recipe(0x811), recipe(0x812).field(b"CTDA", &condition)],
            ),
            Node::top(b"PERK", vec![RecordBuilder::new(b"PERK", FormId(0x820))]),
        ];
        std::fs::write(
            dir.join("Crafting.esp"),
            write_plugin(0, &[], "test", "synthetic crafting", 0x900, &nodes),
        )
        .unwrap();
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Crafting.esp");
        e.inventory_mut(PLAYER_REF);
        (dir, e)
    }
    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn crafting_recipe_visibility_is_separate_from_affordability() {
        let (dir, mut e) = fixture();
        let station = FormId(0x810);
        let recipes = e.recipes_at(station);
        assert_eq!(recipes.len(), 1);
        assert!(!recipes[0].affordable(&e.inventories[&PLAYER_REF]));
        assert!(e.add_perk(PLAYER_REF, FormId(0x820)));
        assert_eq!(e.recipes_at(station).len(), 2);
        e.inventory_mut(PLAYER_REF).add(FormId(0x802), 2);
        assert!(
            e.recipes_at(station)
                .iter()
                .all(|r| r.affordable(&e.inventories[&PLAYER_REF]))
        );
        assert!(e.recipes_at(FormId(0x999)).is_empty());
        drop(e);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
