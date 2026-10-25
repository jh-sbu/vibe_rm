//! Resets scripts ask for: a reference back to how it was placed
//! (`ObjectReference.Reset`), a cell flagged to do that for each of its
//! references when it next loads (`Cell.Reset`), a quest back to nothing
//! done (`Quest.Reset`), and the text globals a running quest keeps copies of
//! (`Quest.UpdateCurrentInstanceGlobal`). Saved: `RSTC`, `QGLB`. Choices:
//! `known_ambiguities/remaining-natives.md`.

use std::collections::{HashMap, HashSet};

use esp::FormId;

use crate::engine::{Engine, PLAYER_REF};

#[derive(Default)]
pub struct Resets {
    /// Cells flagged to reset the next time they load.
    pub(crate) cells: HashSet<FormId>,
    /// The value of each text global a running quest displays, as the quest
    /// last took it: (quest, global).
    pub(crate) quest_globals: HashMap<(FormId, FormId), f32>,
}

impl Engine {
    /// The globals a quest's text may show (`QTGL`).
    pub(crate) fn quest_text_globals(&self, q: FormId) -> Vec<FormId> {
        let Some(rec) = self.lo.get(q) else {
            return Vec::new();
        };
        rec.subrecords()
            .filter(|s| s.tag.0 == *b"QTGL" && s.data.len() >= 4)
            .map(|s| rec.fid(s.form_id(0)))
            .collect()
    }

    /// The value a running quest's text shows for one of its text globals:
    /// the copy it took, made now if it hasn't one yet.
    pub(crate) fn quest_global_value(&mut self, q: FormId, g: FormId) -> Option<f32> {
        if !self.scripts.quests.get(&q).is_some_and(|s| s.running)
            || !self.quest_text_globals(q).contains(&g)
        {
            return None;
        }
        let now = self.global_value(g);
        Some(*self.resets.quest_globals.entry((q, g)).or_insert(now))
    }

    /// `Quest.UpdateCurrentInstanceGlobal`: the quest takes the global's value
    /// now, and the objectives it shows are worded again. False when the quest
    /// isn't running or doesn't show the global.
    pub fn update_quest_global(&mut self, q: FormId, g: FormId) -> bool {
        if !self.scripts.quests.get(&q).is_some_and(|s| s.running)
            || !self.quest_text_globals(q).contains(&g)
        {
            return false;
        }
        let now = self.global_value(g);
        self.resets.quest_globals.insert((q, g), now);
        let mut shown: Vec<i32> = self.scripts.quests[&q]
            .objectives_displayed
            .iter()
            .copied()
            .collect();
        shown.sort();
        for obj in shown {
            let raw = self.objective_text(q, obj);
            let text = self.replace_text_tags(&raw, Some(q));
            self.scripts
                .quests
                .entry(q)
                .or_default()
                .objective_texts
                .insert(obj, text);
        }
        self.scripts.journal_seq += 1;
        let seq = self.scripts.journal_seq;
        self.scripts.quests.entry(q).or_default().journal_seq = seq;
        true
    }

    /// A quest stopped or reset takes its text globals' copies with it.
    pub(crate) fn drop_quest_globals(&mut self, q: FormId) {
        self.resets
            .quest_globals
            .retain(|&(quest, _), _| quest != q);
    }

    /// `Quest.Reset`: stopped, with its stages, objectives, journal entries,
    /// alias fills and flags back to nothing done.
    pub fn reset_quest(&mut self, q: FormId) {
        if self.lo.tag_of(q).map(|t| t.0) != Some(*b"QUST") {
            return;
        }
        self.stop_quest(q);
        self.drop_quest_globals(q);
        self.scripts.pending_stages.retain(|&(quest, _)| quest != q);
        self.scripts.pending_quest_inits.retain(|&quest| quest != q);
        self.scripts.journal_seq += 1;
        let seq = self.scripts.journal_seq;
        let st = self.scripts.quests.entry(q).or_default();
        *st = crate::script::QuestState::default();
        st.journal_seq = seq;
        log::info!("quest {q} reset");
    }

    /// `Cell.Reset`: flagged to reset its references when it next loads.
    pub fn reset_cell(&mut self, cell: FormId) {
        if self.lo.tag_of(cell).map(|t| t.0) == Some(*b"CELL") {
            self.resets.cells.insert(cell);
        }
    }

    /// A cell is loading: if it was flagged, its references go back to how
    /// they were placed first.
    pub(crate) fn apply_cell_reset(&mut self, cell: FormId) {
        if !self.resets.cells.remove(&cell) {
            return;
        }
        let refs: Vec<FormId> = self
            .lo
            .cell(cell)
            .map(|c| c.persistent.iter().chain(&c.temporary).copied().collect())
            .unwrap_or_default();
        log::info!("{cell} resets: {} references", refs.len());
        for r in refs {
            self.restore_reference(r, false);
        }
    }

    /// `ObjectReference.Reset(akTarget)`: back to its placed place (or to
    /// `target`, if it is loaded), enabled or not as placed, unlocked or locked
    /// as placed, not destroyed, its inventory as first rolled, an actor alive
    /// and well. Factions, relationships and owners aren't touched (the CK
    /// wiki's note). Scripts hear `OnReset`.
    pub fn reset_reference(&mut self, r: FormId, target: Option<FormId>) {
        if r.is_null() || r == PLAYER_REF {
            return;
        }
        self.restore_reference(r, true);
        if let Some(t) = target.filter(|&t| t != r && self.is_loaded_ref(r)) {
            if let (Some(pos), rot) = (self.ref_position(t), self.ref_rotation(t)) {
                self.move_ref(r, t, pos, rot);
            }
        }
        self.send_script_event(r, "OnReset", Vec::new());
    }

    fn is_loaded_ref(&self, r: FormId) -> bool {
        self.actor_cells.contains_key(&r) || self.cells.values().any(|c| c.refs.contains(&r))
    }

    /// Put a reference's changed state back to the record's. An actor goes
    /// back to its placed place only when asked about it (`ObjectReference.
    /// Reset`) or if something moved it there: a cell's reset leaves the actors
    /// its schedules put elsewhere.
    fn restore_reference(&mut self, r: FormId, asked: bool) {
        // Made while playing: nothing it was placed as.
        if self.created(r).is_some() {
            return;
        }
        let Some(tag) = self.lo.tag_of(r).map(|t| t.0) else {
            return;
        };
        let base_tag = self.base_of(r).and_then(|b| self.lo.tag_of(b)).map(|t| t.0);
        let actor = tag == *b"ACHR";
        let loaded = self.is_loaded_ref(r);
        // Shut what is open before its record of being open goes.
        if loaded && base_tag == Some(*b"DOOR") {
            self.set_open(r, false);
        }
        let had_moved = self.world_state.moved.contains_key(&r);
        let ws = &mut self.world_state;
        ws.destruction.remove(&r);
        ws.broken_joints.remove(&r);
        ws.graph_open.remove(&r);
        ws.object_graphs.remove(&r);
        ws.open_doors.remove(&r);
        ws.actor_open_doors.remove(&r);
        ws.item_charges.remove(&r);
        if self.scripts.disabled.remove(&r).is_some() {
            self.scripts.visibility_dirty = true;
        }
        self.scripts.locked.remove(&r);
        self.scripts.lock_levels.remove(&r);
        self.harvests.harvested.remove(&r);
        if actor || matches!(base_tag, Some(t) if &t == b"CONT") {
            self.inventories.remove(&r);
            if loaded {
                self.inventory_mut(r);
            }
        }
        if actor {
            self.resurrect(r);
            self.reset_health_and_limbs(r);
            self.heal_wounds_away(r);
            self.actor_flags.critical.remove(&r);
        }
        self.reset_loose(r);
        if had_moved || (actor && asked) {
            self.move_to_editor_location(r);
            self.world_state.moved.remove(&r);
        }
    }

    /// A loose object's bodies back where it was placed, at rest.
    fn reset_loose(&mut self, r: FormId) {
        let mut put_back = Vec::new();
        for l in self
            .cells
            .values_mut()
            .flat_map(|rt| rt.loose.iter_mut())
            .filter(|l| l.ref_id == r)
        {
            l.transform = l.start;
            l.moved = false;
            for b in l.bodies.iter_mut() {
                b.world = l.start * b.frame;
                put_back.push((b.handle, b.world));
            }
        }
        for (h, m) in put_back {
            self.physics.set_body_pose(h, m);
        }
        self.moved_refs.remove(&r);
    }

    pub(crate) fn restore_resets(
        &mut self,
        cells: HashSet<FormId>,
        quest_globals: HashMap<(FormId, FormId), f32>,
    ) {
        self.resets.cells = cells;
        self.resets.quest_globals = quest_globals;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp::writer::{Node, RecordBuilder, write_plugin};
    use papyrus::{NativeResult, ObjectId, Value};

    const HOUSE: FormId = FormId(0x820);
    const ITEM: FormId = FormId(0x800);
    const BOX: FormId = FormId(0x801);
    const CHEST: FormId = FormId(0x810);
    const STATUE: FormId = FormId(0x811);
    const GLOBAL: FormId = FormId(0x830);
    const OTHER_GLOBAL: FormId = FormId(0x831);
    const QUEST: FormId = FormId(0x840);

    struct Fixture(std::path::PathBuf);

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn fixture() -> (Fixture, Engine) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let f = Fixture(
            std::env::temp_dir().join(format!("vrm-resets-{}-{stamp}", std::process::id())),
        );
        std::fs::create_dir(&f.0).unwrap();
        let cnto = [&ITEM.0.to_le_bytes()[..], &2i32.to_le_bytes()].concat();
        let records = [
            Node::top(b"MISC", vec![RecordBuilder::new(b"MISC", ITEM)]),
            Node::top(
                b"CONT",
                vec![RecordBuilder::new(b"CONT", BOX).field(b"CNTO", &cnto)],
            ),
            Node::top(
                b"GLOB",
                vec![
                    RecordBuilder::new(b"GLOB", GLOBAL)
                        .zstring(b"EDID", "ResetGlobal")
                        .f32(b"FLTV", 1.0),
                    RecordBuilder::new(b"GLOB", OTHER_GLOBAL).f32(b"FLTV", 1.0),
                ],
            ),
            Node::top(
                b"QUST",
                vec![
                    RecordBuilder::new(b"QUST", QUEST)
                        .form(b"QTGL", GLOBAL)
                        .field(b"QOBJ", &[1, 0])
                        .zstring(b"NNAM", "Logs: <Global=ResetGlobal>"),
                ],
            ),
            Node::Group {
                label: *b"CELL",
                group_type: 0,
                children: vec![
                    Node::Record(RecordBuilder::new(b"CELL", HOUSE).field(b"DATA", &[1, 0])),
                    Node::Group {
                        label: HOUSE.0.to_le_bytes(),
                        group_type: esp::group_type::CELL_CHILDREN,
                        children: vec![Node::Group {
                            label: HOUSE.0.to_le_bytes(),
                            group_type: esp::group_type::CELL_TEMPORARY,
                            children: vec![
                                Node::Record(RecordBuilder::new(b"REFR", CHEST).form(b"NAME", BOX)),
                                Node::Record(
                                    RecordBuilder::new(b"REFR", STATUE).form(b"NAME", ITEM),
                                ),
                            ],
                        }],
                    },
                ],
            },
        ];
        std::fs::write(
            f.0.join("Resets.esp"),
            write_plugin(0, &[], "test", "synthetic resets", 0x900, &records),
        )
        .unwrap();
        let e = crate::stats::action_tests::headless_engine(&f.0, "Resets.esp");
        (f, e)
    }

    fn call(e: &mut Engine, class: &str, func: &str, on: FormId, args: &[Value]) -> Value {
        let this = e.object_value(on);
        match crate::script::natives::call(e, class, func, Some(&this), args) {
            NativeResult::Value(v) => v,
            _ => panic!("{class}.{func} didn't return"),
        }
    }

    /// Make a reference look used: moved, disabled, locked, damaged, emptied.
    fn disturb(e: &mut Engine, r: FormId) {
        e.world_state.moved.insert(
            r,
            crate::world_state::Moved {
                place: crate::ai::schedule::Place::Interior(HOUSE),
                pos: glam::Vec3::new(5.0, 5.0, 5.0),
                rot: glam::Vec3::ZERO,
            },
        );
        e.scripts.disabled.insert(r, true);
        e.scripts.locked.insert(r, true);
        e.scripts.lock_levels.insert(r, 50);
        e.world_state.destruction.insert(
            r,
            crate::world::destruction::DestructionState {
                health: 0.0,
                stage: 2,
            },
        );
        e.inventory_mut(r).add(ITEM, 7);
    }

    fn untouched(e: &Engine, r: FormId) -> bool {
        !e.world_state.moved.contains_key(&r)
            && !e.scripts.disabled.contains_key(&r)
            && !e.scripts.locked.contains_key(&r)
            && !e.scripts.lock_levels.contains_key(&r)
            && !e.world_state.destruction.contains_key(&r)
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn a_reference_goes_back_to_how_it_was_placed_and_hears_of_it() {
        let (_f, mut e) = fixture();
        disturb(&mut e, CHEST);
        disturb(&mut e, STATUE);
        call(&mut e, "objectreference", "reset", CHEST, &[]);
        assert!(untouched(&e, CHEST));
        // A container's contents are rolled again from the record.
        assert_eq!(e.inventory_mut(CHEST).count(ITEM), 2);
        // Nothing else changed.
        assert!(!untouched(&e, STATUE));
        assert!(
            e.scripts
                .pending_events
                .iter()
                .any(|(o, ev, _)| *o == ObjectId::Form(CHEST.0) && ev == "OnReset")
        );
        // The player and the null reference have nothing to reset.
        e.reset_reference(crate::engine::PLAYER_REF, None);
        e.reset_reference(FormId::NULL, None);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn a_flagged_cell_resets_its_references_when_it_loads() {
        let (_f, mut e) = fixture();
        disturb(&mut e, CHEST);
        disturb(&mut e, STATUE);
        // Only cells can be flagged.
        call(&mut e, "cell", "reset", BOX, &[]);
        assert!(e.resets.cells.is_empty());
        call(&mut e, "cell", "reset", HOUSE, &[]);
        assert!(e.resets.cells.contains(&HOUSE));
        // Flagged, not yet reset.
        assert!(!untouched(&e, CHEST));
        e.apply_cell_reset(FormId(0x999)); // another cell
        assert!(!untouched(&e, CHEST));
        e.apply_cell_reset(HOUSE);
        assert!(untouched(&e, CHEST) && untouched(&e, STATUE));
        assert!(e.resets.cells.is_empty());
        // The cell resets once.
        disturb(&mut e, CHEST);
        e.apply_cell_reset(HOUSE);
        assert!(!untouched(&e, CHEST));
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn quest_reset_clears_everything_it_did() {
        let (_f, mut e) = fixture();
        e.start_quest(QUEST);
        e.set_objective_displayed(QUEST, 1, true, false);
        {
            let st = e.scripts.quests.get_mut(&QUEST).unwrap();
            st.done.insert(10);
            st.stage = 10;
            st.completed = true;
        }
        assert!(e.scripts.quests[&QUEST].running);
        call(&mut e, "quest", "reset", QUEST, &[]);
        let st = &e.scripts.quests[&QUEST];
        assert!(!st.running && st.done.is_empty() && st.stage == 0 && !st.completed);
        assert!(st.objectives_displayed.is_empty() && st.objective_texts.is_empty());
        assert_eq!(
            call(&mut e, "quest", "isstarting", QUEST, &[]),
            Value::Bool(false)
        );
        assert_eq!(
            call(&mut e, "quest", "isstopping", QUEST, &[]),
            Value::Bool(false)
        );
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn a_quest_keeps_its_text_global_until_told_to_update() {
        let (_f, mut e) = fixture();
        e.scripts.globals.insert(GLOBAL, 3.0);
        let global = e.object_value(GLOBAL);
        let other = e.object_value(OTHER_GLOBAL);
        // Not running: nothing to update.
        assert_eq!(
            call(
                &mut e,
                "quest",
                "updatecurrentinstanceglobal",
                QUEST,
                &[global.clone()]
            ),
            Value::Bool(false)
        );
        e.start_quest(QUEST);
        e.set_objective_displayed(QUEST, 1, true, false);
        assert_eq!(e.scripts.quests[&QUEST].objective_texts[&1], "Logs: 3");
        // The quest still shows 3 until it takes the new value.
        e.scripts.globals.insert(GLOBAL, 9.0);
        assert_eq!(
            e.replace_text_tags("<Global=ResetGlobal>", Some(QUEST)),
            "3"
        );
        assert_eq!(e.replace_text_tags("<Global=ResetGlobal>", None), "9");
        assert_eq!(
            call(
                &mut e,
                "quest",
                "updatecurrentinstanceglobal",
                QUEST,
                &[global]
            ),
            Value::Bool(true)
        );
        assert_eq!(e.scripts.quests[&QUEST].objective_texts[&1], "Logs: 9");
        // A global the quest doesn't list can't be updated.
        assert_eq!(
            call(
                &mut e,
                "quest",
                "updatecurrentinstanceglobal",
                QUEST,
                &[other]
            ),
            Value::Bool(false)
        );
        // Stopping drops the copies.
        e.stop_quest(QUEST);
        assert!(e.resets.quest_globals.is_empty());
    }
}
