//! Station recipe queries and crafting transactions.
use crate::engine::Engine;
use crate::world::crafting::Recipe;
use crate::world::inventory::item_info;
use esp::FormId;

pub(crate) fn constructible_xp(lo: &esp::LoadOrder, value: f32) -> f32 {
    let g = |n, d| crate::ai::combat::gmst_f32(lo, n, d);
    g("fConstructibleSkillUseMult", 3.) * value.max(0.).powf(g("fConstructibleSkilluseExp", 0.65))
        + g("fConstructibleSkillUseConst", 25.)
}

#[derive(Clone)]
pub(crate) struct Improvement {
    pub row: crate::items::ListedItem,
    pub recipe: Recipe,
}
impl Engine {
    pub(crate) fn improvements_at(&mut self, station: FormId) -> Vec<Improvement> {
        let Some(bench) = self
            .base_of(station)
            .and_then(|b| self.lo.get(b))
            .as_ref()
            .and_then(crate::world::records::Workbench::parse)
            .filter(|b| matches!(b.kind, 2 | 7))
        else {
            return Vec::new();
        };
        let recipes = self.recipes_at(station);
        self.listed_inventory(crate::engine::PLAYER_REF)
            .into_iter()
            .filter(|row| match bench.kind {
                2 => row.info.kind == crate::world::inventory::ItemKind::Weapon,
                _ => row.info.kind == crate::world::inventory::ItemKind::Armor,
            })
            .filter_map(|row| {
                let recipe = recipes
                    .iter()
                    .find(|r| r.output == row.item && r.visible_for(self, Some(&row.copy)))?
                    .clone();
                Some(Improvement { row, recipe })
            })
            .collect()
    }

    pub(crate) fn craft_event(&self, station: FormId, item: FormId) -> crate::story::StoryEvent {
        let mut event = crate::story::StoryEvent::new(b"CRFT");
        event.refs[0] = station;
        event.locs[0] = self.ref_current_location(station).unwrap_or(FormId::NULL);
        event.form = item;
        event
    }
    pub(crate) fn craft_recipe(
        &mut self,
        station: FormId,
        recipe: FormId,
    ) -> Result<FormId, String> {
        use crate::engine::PLAYER_REF;
        if self.menu != Some(crate::items::Menu::Crafting { station, kind: 1 }) {
            return Err("".into());
        }
        let Some(r) = self
            .recipes_at(station)
            .into_iter()
            .find(|r| r.id == recipe)
        else {
            return Err("".into());
        };
        let inv = self.inventory_mut(PLAYER_REF);
        if !r.affordable(inv) || inv.count(r.output).checked_add(r.count as i32).is_none() {
            return Err(self
                .gmst_string("sLackRequiredToCreate")
                .unwrap_or_else(|| "You lack the required components.".into()));
        }
        for &(item, n) in &r.inputs {
            self.remove_stack(PLAYER_REF, item, n as i32, None, None, None);
        }
        self.inventory_mut(PLAYER_REF).add(r.output, r.count as i32);
        self.inventory_event(PLAYER_REF, true, r.output, r.count as i32, None);
        match self.lo.tag_of(r.output).map(|t| t.0) {
            Some(t) if t == *b"WEAP" => self.increment_stat("Weapons Made", r.count as i32),
            Some(t) if t == *b"ARMO" => self.increment_stat("Armor Made", r.count as i32),
            _ => {}
        }
        let forge = [
            "CraftingSmithingForge",
            "CraftingSmithingSkyforge",
            "DLC1CraftingDawnguard",
            "DLC1LD_CraftingForgeAetherium",
        ]
        .iter()
        .any(|name| self.lo.find_editor_id(name) == Some(r.bench));
        if forge {
            let value =
                item_info(&self.lo, r.output).map_or(0, |i| i.value).max(0) as f32 * r.count as f32;
            self.use_skill(
                esp::actor_value::SMITHING,
                constructible_xp(&self.lo, value),
            );
        }
        let event = self.craft_event(station, r.output);
        self.send_story_event(event);
        Ok(r.output)
    }
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
                        .field(b"DATA", &data)
                        .field(
                            b"VMAD",
                            &esp::writer::vmad(&[esp::writer::AttachedScript {
                                name: "VrmCraftOutput".into(),
                                properties: Vec::new(),
                            }]),
                        ),
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
        std::fs::create_dir_all(dir.join("scripts")).unwrap();
        let mut script = papyrus::build::ScriptBuilder::new("VrmCraftOutput", "Form");
        let seen = script.variable("seen", "Bool", false.into());
        let mut on = papyrus::build::FunctionBuilder::new("None");
        on.param("newContainer", "ObjectReference");
        on.param("oldContainer", "ObjectReference");
        on.assign(seen, true.into());
        on.ret(papyrus::build::Arg::None);
        script.function("OnContainerChanged", on);
        std::fs::write(
            dir.join("scripts/VrmCraftOutput.pex"),
            papyrus::pex::write(&script.build().unwrap()).unwrap(),
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

#[cfg(test)]
mod transaction_tests {
    use super::*;
    use crate::engine::PLAYER_REF;
    #[test]
    fn crafting_xp_uses_cumulative_output_value() {
        let lo = esp::LoadOrder::load(std::env::temp_dir(), &[]).unwrap();
        assert!((constructible_xp(&lo, 10.) - 38.4).abs() < 0.1);
        assert!(constructible_xp(&lo, 20.) > constructible_xp(&lo, 10.));
    }
    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic generated records and PEX only"]
    fn crafting_delivers_container_changes_to_form_derived_output_scripts() {
        let (dir, mut e) = super::tests::fixture();
        e.open_crafting_station(FormId(0x810));
        e.inventory_mut(PLAYER_REF).add(FormId(0x802), 2);
        e.craft_recipe(FormId(0x810), FormId(0x811)).unwrap();
        let mut vm = std::mem::take(&mut e.vm);
        let events = std::mem::take(&mut e.scripts.pending_events);
        {
            let mut host = crate::script::EngineHost { engine: &mut e };
            for (obj, event, args) in events {
                crate::script::attach_form_event_scripts(&mut vm, &mut host, obj);
                vm.send_event(&mut host, obj, &event, args);
            }
            vm.run(&mut host, 0., 2000);
        }
        assert_eq!(
            vm.get_var(papyrus::ObjectId::Form(0x803), "seen"),
            Some(papyrus::Value::Bool(true))
        );
        e.vm = vm;
        drop(e);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn crafting_consumes_exact_counts_rechecks_and_emits_inventory_events() {
        let (dir, mut e) = super::tests::fixture();
        let station = FormId(0x810);
        let recipe = FormId(0x811);
        e.open_crafting_station(station);
        let owner = Some(FormId(0x999));
        e.inventory_mut(PLAYER_REF).add(FormId(0x802), 1);
        e.inventory_mut(PLAYER_REF)
            .add_owned(FormId(0x802), owner, 2);
        assert_eq!(e.craft_recipe(station, recipe).unwrap(), FormId(0x803));
        let inv = &e.inventories[&PLAYER_REF];
        assert_eq!(inv.count(FormId(0x802)), 1);
        assert_eq!(inv.count_owned(FormId(0x802), owner), 1);
        assert_eq!(inv.count_owned(FormId(0x803), None), 2);
        assert!(
            e.scripts
                .pending_events
                .iter()
                .any(|(_, event, _)| event == "OnItemAdded")
        );
        let event = e.craft_event(station, FormId(0x803));
        assert_eq!(event.code, *b"CRFT");
        assert_eq!(event.refs[0], station);
        assert_eq!(event.form, FormId(0x803));
        assert!(e.craft_recipe(station, recipe).is_err());
        assert_eq!(e.inventories[&PLAYER_REF].count(FormId(0x803)), 2);
        e.menu = None;
        assert!(e.craft_recipe(station, recipe).is_err());
        drop(e);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
#[path = "crafting/smithing_tests.rs"]
mod smithing_tests;
