//! The magic menu as the game shows it (`magicmenu.swf`): the player's
//! spells by school, shouts, powers and active effects, each with its card
//! (cost, level, effects, the school's skill on the bottom bar; a shout's
//! words, those unlocked, and the dragon souls), readied in a hand with the
//! attack buttons or Accept (a shout or power as the voice), and marked
//! favorites. Ready Weapon (R) spends a dragon soul on a shout's next word.
//!
//! It is the inventory's `ItemMenu` with `AttemptEquip` (a mouse button's
//! hand) answered by `ItemSelect(slot)`. The game fills it as the item menus
//! (`SetCategoriesList`, the list's `entryList`, `InvalidateListData`, the
//! card on `RequestItemCardInfo`). Choices: `known_ambiguities/magic-menu.md`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use esp::FormId;
use ui_swf::{GameCall, Menu, UiSwf, Value};

use crate::engine::{Engine, PLAYER_REF};
use crate::magic::{flags, spell_type};
use crate::spells::{CONCENTRATION, LEFT, RIGHT, SCHOOLS, VOICE};

use super::item_menu::{object, player_info};
use super::{Buttons, num, text};

const M: &str = "_root.Menu_mc";
const LISTS: &str = "_root.Menu_mc.InventoryLists_mc";

/// The categories' filter flags: the game's to choose (the menu only tests
/// them against each other and takes the first category's for favorites).
const FAVORITES: u32 = 0x1;
/// Alteration, Illusion, Destruction, Conjuration, Restoration.
const SCHOOL_FLAGS: [u32; 5] = [0x2, 0x4, 0x8, 0x10, 0x20];
const SHOUTS: u32 = 0x40;
const POWERS: u32 = 0x80;
const ACTIVE: u32 = 0x100;
/// Every spell, power and shout; not the active effects.
const ALL: u32 = 0xFE;

/// The item card's kinds (`InventoryDefines.ICT_*`).
const ICT_SPELL: f64 = 7.0;
const ICT_SHOUT: f64 = 10.0;
const ICT_ACTIVE_EFFECT: f64 = 11.0;
const ICT_SPELL_DEFAULT: f64 = 13.0;

/// Equip icons (`InventoryDefines.ES_*`).
const ES_EQUIPPED: f64 = 1.0;
const ES_LEFT: f64 = 2.0;
const ES_RIGHT: f64 = 3.0;
const ES_BOTH: f64 = 4.0;

/// A row of the list.
#[derive(Debug, Clone, PartialEq)]
enum Row {
    /// A spell or power the player knows.
    Spell(FormId),
    /// A shout the player has.
    Shout(FormId),
    /// The effects of one item (a spell, potion, ability...) on the player.
    Effects(FormId),
}

pub(super) struct MagicMenu {
    pub menu: Menu,
    rows: Vec<Row>,
    /// What the list shows, to fill it again when it changes: the rows, the
    /// slots and favorites.
    listed: Option<(Vec<Row>, [Option<FormId>; 3], Vec<FormId>)>,
    /// Each row's card, for `RequestItemCardInfo`.
    cards: Arc<Mutex<Vec<Value>>>,
    player: Option<Value>,
    /// Closing: it fades out, then calls `CloseMenu`.
    pub closing: bool,
    /// Closing for the tween menu (`ShowTweenMenu`), not the world.
    to_tween: bool,
}

/// The categories for `SetCategoriesList`: (name, flag, always listed) each.
fn categories(engine: &Engine) -> Vec<Value> {
    let mut list: Vec<(String, u32)> = vec![
        (setting(engine, "sFavorites", "FAVORITES"), FAVORITES),
        (setting(engine, "sAll", "ALL"), ALL),
    ];
    for (skill, flag) in SCHOOLS.iter().zip(SCHOOL_FLAGS) {
        list.push((engine.skill_name(*skill).to_uppercase(), flag));
    }
    list.push((setting(engine, "sShouts", "SHOUTS"), SHOUTS));
    list.push((setting(engine, "sPowers", "POWERS"), POWERS));
    list.push((setting(engine, "sActiveEffects", "ACTIVE EFFECTS"), ACTIVE));
    list.into_iter()
        .flat_map(|(name, flag)| [text(&name), num(flag as f32), Value::Bool(flag == ALL)])
        .collect()
}

fn setting(engine: &Engine, name: &str, english: &str) -> String {
    engine
        .gmst_string(name)
        .unwrap_or_else(|| english.to_string())
}

/// A spell's category flag: its school's, or the powers'.
fn spell_flag(engine: &Engine, spell: FormId) -> Option<u32> {
    let item = engine.magic_item(spell)?;
    match item.spell_type {
        spell_type::POWER | spell_type::LESSER_POWER => Some(POWERS),
        spell_type::SPELL => {
            let school = engine.spell_school(spell)?;
            let i = SCHOOLS.iter().position(|&s| s == school)?;
            Some(SCHOOL_FLAGS[i])
        }
        _ => None,
    }
}

impl MagicMenu {
    pub fn open(
        ui: &UiSwf,
        size: (u32, u32),
        engine: &mut Engine,
        indices: &[Value],
        buttons: Arc<Buttons>,
    ) -> anyhow::Result<Self> {
        let menu = ui.open("magicmenu.swf", size)?;
        let cards: Arc<Mutex<Vec<Value>>> = Default::default();
        let row_cards = cards.clone();
        menu.set_responder(move |call, movie| match call.name.as_str() {
            "RequestItemCardInfo" => {
                let row = movie.get(&format!("{LISTS}.ItemsList.selectedEntry.vrmRow"));
                let Value::Number(row) = row else {
                    return None;
                };
                let card = row_cards.lock().unwrap().get(row as usize).cloned()?;
                Some(vec![card])
            }
            _ => super::respond(&buttons, call),
        });
        // The movie's root has no functions of its own: its menu clip's.
        menu.invoke(&format!("{M}.InitExtensions"), &[]);
        menu.invoke(&format!("{M}.SetPlatform"), &[num(0.0), Value::Bool(false)]);
        // The active effects have no Equip / Favorite.
        menu.set(&format!("{M}.hideButtonFlag"), num(ACTIVE as f32));
        menu.callback("SetCategoriesList", &categories(engine));
        let mut m = MagicMenu {
            menu,
            rows: Vec::new(),
            listed: None,
            cards,
            player: None,
            closing: false,
            to_tween: false,
        };
        m.update(engine);
        if !indices.is_empty() {
            m.menu.callback("RestoreIndices", indices);
        }
        Ok(m)
    }

    /// The rows: the spells and powers the player knows, then the items with
    /// effects on them.
    fn rows(engine: &Engine) -> Vec<Row> {
        let mut rows: Vec<Row> = engine
            .actor_spells(PLAYER_REF)
            .into_iter()
            .filter(|&s| spell_flag(engine, s).is_some())
            .map(Row::Spell)
            .collect();
        rows.extend(engine.actor_shouts(PLAYER_REF).into_iter().map(Row::Shout));
        for x in engine.magic.effects.iter() {
            if x.target != PLAYER_REF || !x.active || x.done {
                continue;
            }
            let hidden = engine
                .magic_effect(x.effect)
                .is_none_or(|m| m.has(flags::HIDE_IN_UI));
            if !hidden && !rows.contains(&Row::Effects(x.item)) {
                rows.push(Row::Effects(x.item));
            }
        }
        rows
    }

    /// Fills the list again when it changed, and the bottom bar's stats.
    pub fn update(&mut self, engine: &mut Engine) {
        let player = player_info(engine);
        if self.player.as_ref() != Some(&player) {
            self.menu.callback("UpdatePlayerInfo", &[player.clone()]);
            self.player = Some(player);
        }
        let rows = Self::rows(engine);
        let slots = engine
            .equipped_spells
            .get(&PLAYER_REF)
            .copied()
            .unwrap_or_default();
        let listed = (rows.clone(), slots, engine.favorites.clone());
        if self.listed.as_ref() == Some(&listed) {
            return;
        }
        let entries: Vec<Value> = rows
            .iter()
            .enumerate()
            .map(|(i, r)| entry(engine, i, r, &slots))
            .collect();
        *self.cards.lock().unwrap() = rows.iter().map(|r| card(engine, r)).collect();
        let list = format!("{LISTS}.ItemsList.entryList");
        self.menu.invoke(&format!("{list}.splice"), &[num(0.0)]);
        if !entries.is_empty() {
            self.menu.invoke(&format!("{list}.push"), &entries);
        }
        self.menu.callback("InvalidateListData", &[]);
        self.rows = rows;
        self.listed = Some(listed);
    }

    fn selected(&self) -> Option<Row> {
        match self
            .menu
            .get(&format!("{LISTS}.ItemsList.selectedEntry.vrmRow"))
        {
            Value::Number(n) if n >= 0.0 => self.rows.get(n as usize).cloned(),
            _ => None,
        }
    }

    /// The menu's calls this answers; false for those it leaves to the
    /// menus' shared ones.
    pub fn answer(
        &mut self,
        engine: &mut Engine,
        call: &GameCall,
        indices: &mut Vec<Value>,
    ) -> bool {
        match call.name.as_str() {
            // A hand's equip (`AttemptEquip`: 0 right, 1 left), or Accept
            // (the right hand): readies the spell there, or puts it away when
            // it already is.
            "ItemSelect" => {
                let (Some(Row::Spell(spell)) | Some(Row::Shout(spell))) = self.selected() else {
                    return true;
                };
                let slot = match call.args.first() {
                    Some(Value::Number(n)) if *n as usize == LEFT => LEFT,
                    _ => RIGHT,
                };
                engine.toggle_spell(PLAYER_REF, spell, slot);
            }
            "SaveIndices" => {
                if let Some(Value::List(list)) = call.args.first() {
                    *indices = list.clone();
                }
            }
            "CloseMenu" => {
                if engine.menu == Some(crate::items::Menu::Magic) {
                    engine.menu = self.to_tween.then_some(crate::items::Menu::Tween);
                }
            }
            // Back to the tween menu, or the world.
            "ShowTweenMenu" => {
                self.closing = true;
                self.to_tween = true;
            }
            "CloseTweenMenu" => self.closing = true,
            // A disabled entry (none are), the item's model (none for spells).
            "ShowShoutFail" | "UpdateItem3D" | "StartMouseRotation" | "StopMouseRotation"
            | "ZoomItemModel" | "DisabledItemSelect" => {}
            _ => return false,
        }
        true
    }

    /// Ready Weapon on a shout: a dragon soul unlocks its next word known.
    /// The card plays the word being learned.
    pub fn unlock_key(&mut self, engine: &mut Engine) {
        let Some(Row::Shout(shout)) = self.selected() else {
            return;
        };
        if !engine.spend_dragon_soul(shout) {
            return;
        }
        self.menu.callback("DragonSoulSpent", &[]);
        let row = Row::Shout(shout);
        let mut card = card(engine, &row);
        if let Value::Object(f) = &mut card {
            f.insert("soulSpent".into(), Value::Bool(true));
        }
        self.menu
            .invoke(&format!("{M}.UpdateItemCardInfo"), &[card]);
        let info = player_info(engine);
        self.menu.callback("UpdatePlayerInfo", &[info.clone()]);
        self.player = Some(info);
        // The cards are made again.
        self.listed = None;
        self.update(engine);
    }

    /// The control map's Toggle POV key: a spell, shout or power marked a
    /// favorite or not.
    pub fn favorite_key(&mut self, engine: &mut Engine) {
        let (Some(Row::Spell(spell)) | Some(Row::Shout(spell))) = self.selected() else {
            return;
        };
        engine.toggle_favorite(spell);
        self.update(engine);
        self.menu.invoke(&format!("{M}.UpdateButtonText"), &[]);
    }

    /// A mouse button's equip (`AttemptEquip`, the hand's slot).
    pub fn equip_button(&self, slot: f32) {
        self.menu.callback("AttemptEquip", &[num(slot)]);
    }

    /// Esc: it fades out as on Tab.
    pub fn close(&mut self) {
        if self.closing {
            return;
        }
        self.closing = true;
        self.menu.invoke(&format!("{M}.StartMenuFade"), &[]);
    }
}

/// A row as the list's entry: its name, category flags (and the favorites'),
/// the hand it is in; `vrmRow` is its place in the rows.
fn entry(engine: &Engine, i: usize, row: &Row, slots: &[Option<FormId>; 3]) -> Value {
    let (name, flag, equip, favorite) = match *row {
        Row::Spell(spell) => {
            let equip = equip_state(slots, spell);
            let favorite = engine.favorites.contains(&spell);
            let flag =
                spell_flag(engine, spell).unwrap_or(0) | if favorite { FAVORITES } else { 0 };
            let name = engine.magic_item(spell).map(|m| m.name.clone());
            (name, flag, equip, favorite)
        }
        Row::Shout(shout) => {
            let favorite = engine.favorites.contains(&shout);
            let equip = if slots[VOICE] == Some(shout) {
                ES_EQUIPPED
            } else {
                0.0
            };
            let flag = SHOUTS | if favorite { FAVORITES } else { 0 };
            (
                engine.shout(shout).map(|s| s.name.clone()),
                flag,
                equip,
                favorite,
            )
        }
        Row::Effects(item) => (effects_name(engine, item), ACTIVE, 0.0, false),
    };
    object([
        ("text", text(&name.unwrap_or_default())),
        ("filterFlag", num(flag as f32)),
        ("equipState", Value::Number(equip)),
        ("favorite", Value::Bool(favorite)),
        ("enabled", Value::Bool(true)),
        ("vrmRow", num(i as f32)),
    ])
}

/// A spell's equip icon: the hands it is in, or readied as a power.
pub(super) fn equip_state(slots: &[Option<FormId>; 3], spell: FormId) -> f64 {
    let held = |s: usize| slots[s] == Some(spell);
    match (held(RIGHT), held(LEFT), held(VOICE)) {
        (true, true, _) => ES_BOTH,
        (true, false, _) => ES_RIGHT,
        (false, true, _) => ES_LEFT,
        (_, _, true) => ES_EQUIPPED,
        _ => 0.0,
    }
}

/// An item's effects on the player are listed under the item's name, else
/// its first effect's.
fn effects_name(engine: &Engine, item: FormId) -> Option<String> {
    let m = engine.magic_item(item)?;
    if !m.name.is_empty() {
        return Some(m.name.clone());
    }
    let first = m.effects.first()?;
    engine.magic_effect(first.effect).map(|e| e.name.clone())
}

/// A row's item card.
fn card(engine: &Engine, row: &Row) -> Value {
    let mut f: BTreeMap<String, Value> = BTreeMap::new();
    let mut put = |k: &str, v: Value| {
        f.insert(k.to_string(), v);
    };
    match *row {
        Row::Spell(spell) => {
            let Some(item) = engine.magic_item(spell) else {
                return Value::Null;
            };
            put("name", text(&item.name));
            put("effects", text(&engine.effects_text(spell)));
            let cost = engine.spell_cost(PLAYER_REF, spell).round();
            if item.spell_type == spell_type::SPELL {
                put("type", Value::Number(ICT_SPELL));
                put("spellCost", num(cost));
                // 0 is the card's cost per second.
                let concentration = item.cast_type == CONCENTRATION;
                put("castTime", num(if concentration { 0.0 } else { 1.0 }));
                put("castLevel", text(&engine.spell_level_name(spell)));
                // The bottom bar's school meter.
                if let Some(school) = engine.spell_school(spell) {
                    put("magicSchoolName", text(&engine.skill_name(school)));
                    put(
                        "magicSchoolLevel",
                        num(engine.av_base(PLAYER_REF, school).floor()),
                    );
                    put("magicSchoolPct", num(engine.skill_progress(school) * 100.0));
                }
            } else {
                put("type", Value::Number(ICT_SPELL_DEFAULT));
                put("spellCost", num(0.0));
            }
        }
        Row::Shout(shout) => {
            let Some(s) = engine.shout(shout) else {
                return Value::Null;
            };
            put("type", Value::Number(ICT_SHOUT));
            put("name", text(&s.name));
            // The words known, in the dragon tongue and their meaning, and
            // whether each is unlocked.
            for (i, w) in s.words.iter().enumerate().take(3) {
                let word = engine.word(w.word);
                let known = engine.shouts.known.contains(&w.word);
                let (dragon, meaning) = if known {
                    (word.dragon.as_str(), word.translation.as_str())
                } else {
                    ("", "")
                };
                put(&format!("dragonWord{i}"), text(dragon));
                put(&format!("word{i}"), text(meaning));
                put(
                    &format!("unlocked{i}"),
                    Value::Bool(engine.is_word_unlocked(w.word)),
                );
            }
            // What the shout does, and the rest it needs after its words
            // unlocked.
            let level = engine.shout_level(&s).max(1);
            let word = &s.words[level.min(s.words.len()) - 1];
            let effects = if s.description.is_empty() {
                engine.effects_text(word.spell)
            } else {
                s.description.clone()
            };
            put("effects", text(&effects));
            put("spellCost", num(word.recovery.round()));
            put(
                "showUnlocked",
                Value::Bool(engine.next_word_to_unlock(shout).is_some()),
            );
        }
        Row::Effects(item) => {
            put("type", Value::Number(ICT_ACTIVE_EFFECT));
            put(
                "name",
                text(&effects_name(engine, item).unwrap_or_default()),
            );
            put("effects", text(&engine.effects_text(item)));
            // The soonest of its effects to end; none for constant ones.
            let left = engine
                .magic
                .effects
                .iter()
                .filter(|x| x.target == PLAYER_REF && x.item == item && !x.done)
                .filter(|x| x.duration.is_finite() && x.duration > 0.0)
                .map(|x| x.duration - x.elapsed)
                .fold(f32::INFINITY, f32::min);
            put(
                "timeRemaining",
                num(if left.is_finite() { left.max(0.0) } else { 0.0 }),
            );
            let negative = engine.magic_item(item).is_some_and(|m| {
                m.effects.iter().any(|e| {
                    engine
                        .magic_effect(e.effect)
                        .is_some_and(|x| x.has(flags::HOSTILE) || x.has(flags::DETRIMENTAL))
                })
            });
            put("negativeEffect", Value::Bool(negative));
        }
    }
    Value::Object(f)
}
