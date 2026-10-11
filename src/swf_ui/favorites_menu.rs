//! The favorites menu as the game shows it (`favoritesmenu.swf`): the
//! player's favorite items, spells and powers in one list, sorted by name,
//! each with its hotkey and equip icon. Accept or an attack button equips or
//! uses the selected one, 1 to 8 put it on that hotkey; the menu stays up
//! until Tab, Esc or the Favorites key fades it out.
//!
//! The game fills the list itself (the movie's `PopulateItems` callback has
//! no function behind it): the entries go in `List_mc.entryList`, then
//! `InvalidateData` sorts and shows them. An entry's `index` is the game's
//! own number for it, given back by `FadeDone` and taken by
//! `SetSelectedItem`. Choices: `known_gaps/favorites.md`.

use esp::FormId;
use ui_swf::{GameCall, Menu, UiSwf, Value};

use crate::engine::{Engine, PLAYER_REF};
use crate::items::ListedItem;
use crate::spells::{LEFT, RIGHT};
use crate::world::inventory::ItemKind;

use super::item_menu::object;
use super::magic_menu::equip_state;
use super::{Buttons, num, text};

const M: &str = "_root.MenuHolder.Menu_mc";
const LIST: &str = "_root.MenuHolder.Menu_mc.List_mc";

/// Equip icons (`InventoryDefines.ES_*`): worn, and wielded in the right hand.
const ES_EQUIPPED: f64 = 1.0;
const ES_RIGHT: f64 = 3.0;

/// A row of the list.
#[derive(Debug, Clone)]
enum Row {
    Item(ListedItem),
    Spell(FormId),
}

impl Row {
    fn form(&self) -> FormId {
        match self {
            Row::Item(i) => i.item,
            Row::Spell(s) => *s,
        }
    }
}

/// What the list shows, to fill it again when it changes: the rows' forms
/// and counts, what is worn, the spells in hand, the hotkeys.
type Listed = (
    Vec<(FormId, i32)>,
    Vec<FormId>,
    [Option<FormId>; 3],
    [Option<FormId>; 8],
);

pub(super) struct FavoritesMenu {
    pub menu: Menu,
    rows: Vec<Row>,
    listed: Option<Listed>,
    /// Closing: it fades out, then calls `FadeDone`.
    pub closing: bool,
}

impl FavoritesMenu {
    /// Opens the menu with `last` (the favorite selected when it last
    /// closed) selected.
    pub fn open(
        ui: &UiSwf,
        size: (u32, u32),
        engine: &mut Engine,
        last: Option<FormId>,
        buttons: std::sync::Arc<Buttons>,
    ) -> anyhow::Result<Self> {
        let menu = ui.open("favoritesmenu.swf", size)?;
        menu.set_responder(move |call, _| super::respond(&buttons, call));
        menu.invoke(&format!("{M}.InitExtensions"), &[]);
        menu.invoke(&format!("{M}.SetPlatform"), &[num(0.0), Value::Bool(false)]);
        let mut m = FavoritesMenu {
            menu,
            rows: Vec::new(),
            listed: None,
            closing: false,
        };
        m.update(engine);
        if let Some(i) = last.and_then(|f| m.rows.iter().position(|r| r.form() == f)) {
            m.menu.callback("SetSelectedItem", &[num(i as f32)]);
        }
        Ok(m)
    }

    /// The rows: the favorite items the player carries, then the favorite
    /// spells and powers they know (the list sorts them by name).
    fn rows(engine: &mut Engine) -> Vec<Row> {
        let favorites = engine.favorites.clone();
        let mut rows: Vec<Row> = engine
            .listed_inventory(PLAYER_REF)
            .into_iter()
            .filter(|i| favorites.contains(&i.item))
            .map(Row::Item)
            .collect();
        let known = engine.actor_spells(PLAYER_REF);
        rows.extend(
            favorites
                .iter()
                .filter(|f| known.contains(f))
                .map(|&s| Row::Spell(s)),
        );
        rows
    }

    /// Fills the list again when it changed, keeping the selection.
    pub fn update(&mut self, engine: &mut Engine) {
        let rows = Self::rows(engine);
        let worn = engine
            .inventories
            .get(&PLAYER_REF)
            .map(|i| i.equipped.clone())
            .unwrap_or_default();
        let slots = engine
            .equipped_spells
            .get(&PLAYER_REF)
            .copied()
            .unwrap_or_default();
        let listed: Listed = (
            rows.iter()
                .map(|r| match r {
                    Row::Item(i) => (i.item, i.count),
                    Row::Spell(s) => (*s, 1),
                })
                .collect(),
            worn.clone(),
            slots,
            engine.hotkeys,
        );
        if self.listed.as_ref() == Some(&listed) {
            return;
        }
        let selected = self.selected().map(|r| r.form());
        let entries: Vec<Value> = rows
            .iter()
            .enumerate()
            .map(|(i, r)| entry(engine, i, r, &worn, &slots))
            .collect();
        let list = format!("{LIST}.entryList");
        self.menu.invoke(&format!("{list}.splice"), &[num(0.0)]);
        if !entries.is_empty() {
            self.menu.invoke(&format!("{list}.push"), &entries);
        }
        self.menu.invoke(&format!("{LIST}.InvalidateData"), &[]);
        self.rows = rows;
        self.listed = Some(listed);
        if let Some(i) = selected.and_then(|f| self.rows.iter().position(|r| r.form() == f)) {
            self.menu.callback("SetSelectedItem", &[num(i as f32)]);
        }
    }

    fn selected(&self) -> Option<Row> {
        match self.menu.get(&format!("{LIST}.selectedEntry.index")) {
            Value::Number(n) if n >= 0.0 => self.rows.get(n as usize).cloned(),
            _ => None,
        }
    }

    /// The menu's calls this answers; false for those it leaves to the
    /// menus' shared ones. `last` keeps the favorite selected on closing.
    pub fn answer(
        &mut self,
        engine: &mut Engine,
        call: &GameCall,
        last: &mut Option<FormId>,
    ) -> bool {
        match call.name.as_str() {
            // Accept: as the right hand's attack button.
            "ItemSelect" => self.equip(engine, RIGHT),
            "FadeDone" => {
                if let Some(Value::Number(n)) = call.args.first() {
                    *last = self.rows.get(*n as usize).map(|r| r.form());
                }
                if engine.menu == Some(crate::items::Menu::Favorites) {
                    engine.menu = None;
                }
            }
            _ => return false,
        }
        true
    }

    /// Equips or uses the selected favorite: a spell readied in `slot`'s hand
    /// (`RIGHT` / `LEFT`), a power as the voice, an item as the inventory
    /// does (one hand).
    pub fn equip(&mut self, engine: &mut Engine, slot: usize) {
        if self.closing {
            return;
        }
        match self.selected() {
            Some(Row::Item(item)) => engine.use_listed_item(&item),
            Some(Row::Spell(spell)) => engine.toggle_spell(PLAYER_REF, spell, slot),
            None => return,
        }
        self.update(engine);
    }

    /// An attack button's equip: slot 0 the right hand, 1 the left (as the
    /// item menus' `AttemptEquip`).
    pub fn equip_button(&mut self, engine: &mut Engine, slot: f32) {
        self.equip(engine, if slot as usize == 1 { LEFT } else { RIGHT });
    }

    /// 1 to 8: the selected favorite goes on hotkey `key` (0 to 7), off any
    /// other.
    pub fn hotkey(&mut self, engine: &mut Engine, key: usize) {
        let Some(row) = self.selected() else {
            return;
        };
        let form = row.form();
        for k in engine.hotkeys.iter_mut().filter(|k| **k == Some(form)) {
            *k = None;
        }
        engine.hotkeys[key] = Some(form);
        self.update(engine);
    }

    /// Esc or the Favorites key: it fades out as on Tab.
    pub fn close(&mut self) {
        if self.closing {
            return;
        }
        self.closing = true;
        self.menu.callback("StartFadeOut", &[]);
    }
}

/// A row as the list's entry: its name (with the count when more than one),
/// hotkey, equip icon; `index` is its place in the rows.
fn entry(
    engine: &Engine,
    i: usize,
    row: &Row,
    worn: &[FormId],
    slots: &[Option<FormId>; 3],
) -> Value {
    let (name, equip) = match row {
        Row::Item(item) => {
            let equip = match worn.contains(&item.item) {
                false => 0.0,
                true if item.info.kind == ItemKind::Weapon => ES_RIGHT,
                true => ES_EQUIPPED,
            };
            let name = if item.count > 1 {
                format!("{} ({})", item.info.name, item.count)
            } else {
                item.info.name.clone()
            };
            (name, equip)
        }
        Row::Spell(spell) => {
            let name = engine
                .magic_item(*spell)
                .map(|m| m.name.clone())
                .unwrap_or_default();
            (name, equip_state(slots, *spell))
        }
    };
    let hotkey = engine
        .hotkeys
        .iter()
        .position(|k| *k == Some(row.form()))
        .map_or(-1.0, |k| k as f32);
    object([
        ("text", text(&name)),
        ("hotkey", num(hotkey)),
        ("equipState", Value::Number(equip)),
        ("filterFlag", num(1.0)),
        ("index", num(i as f32)),
    ])
}
