//! The inventory as the game shows it: `inventorymenu.swf`, its categories on
//! the left, the items of one beside them, the item card and the bottom bar
//! with the player's stats.
//!
//! The game fills the menu: `SetCategoriesList` (name, filter flag, always
//! shown, for each category), then the items list's `entryList` and
//! `InvalidateListData`. The menu asks for the selected item's card
//! (`RequestItemCardInfo`) and says what the player does (`ItemSelect`,
//! `ItemDrop`, `CloseMenu`...). Equipping with the mouse, dropping and marking
//! favorites are the game's keys, which it passes on as `AttemptEquip` and
//! `DropItem` or handles itself. What is guessed is in `known_gaps/swf-hud.md`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use esp::FormId;
use esp::actor_value as av;
use ui_swf::{GameCall, Menu, UiSwf, Value};

use crate::engine::{Engine, PLAYER_REF};
use crate::items::ListedItem;
use crate::world::inventory::ItemKind;

use super::{Buttons, num, text};

/// The menu's root clip (`InventoryMenu`) and its lists.
const M: &str = "_root.Menu_mc";
const LISTS: &str = "_root.Menu_mc.InventoryLists_mc";

/// The item card's kinds (`InventoryDefines.ICT_*`).
mod card {
    pub const ARMOR: f64 = 1.0;
    pub const WEAPON: f64 = 2.0;
    pub const MISC: f64 = 3.0;
    pub const BOOK: f64 = 4.0;
    pub const FOOD: f64 = 5.0;
    pub const POTION: f64 = 6.0;
    pub const INGREDIENT: f64 = 8.0;
    pub const KEY: f64 = 9.0;
}

/// Equip icons (`InventoryDefines.ES_*`): worn, and wielded in the right hand.
const ES_EQUIPPED: f64 = 1.0;
const ES_RIGHT: f64 = 3.0;

/// The categories, in the game's order: their names' game settings, English
/// fallbacks and filter flags. The flags are the game's to choose; the menu
/// only tests them against each other, and takes the first category's for the
/// favorites'. "All" shows every item and stays listed when empty.
const FAVORITES: u32 = 0x1;
const CATEGORIES: &[(&str, &str, u32)] = &[
    ("sFavorites", "Favorites", FAVORITES),
    ("sAll", "All", 0x3FE),
    ("sWeapons", "Weapons", 0x2),
    ("sApparel", "Apparel", 0x4),
    ("sPotions", "Potions", 0x8),
    ("sScrolls", "Scrolls", 0x10),
    ("sFood", "Food", 0x20),
    ("sIngredients", "Ingredients", 0x40),
    ("sBooks", "Books", 0x80),
    ("sKeys", "Keys", 0x100),
    ("sMisc", "Misc", 0x200),
];

/// The category an item is listed under.
fn category(engine: &Engine, row: &ListedItem) -> u32 {
    match row.info.kind {
        ItemKind::Weapon | ItemKind::Ammo => 0x2,
        ItemKind::Armor => 0x4,
        ItemKind::Potion if engine.is_food(row.item) => 0x20,
        ItemKind::Potion => 0x8,
        ItemKind::Scroll => 0x10,
        ItemKind::Ingredient => 0x40,
        ItemKind::Book => 0x80,
        ItemKind::Key => 0x100,
        ItemKind::SoulGem | ItemKind::Light | ItemKind::Misc => 0x200,
    }
}

/// What the list shows, to know when it must be filled again: each row's item,
/// owner and count, what is worn, the favorites.
type Listed = (Vec<(FormId, Option<FormId>, i32)>, Vec<FormId>, Vec<FormId>);

pub(super) struct ItemMenu {
    pub menu: Menu,
    /// The rows, in the order of the menu's `entryList`.
    rows: Vec<ListedItem>,
    listed: Option<Listed>,
    /// Each row's item card, for the menu's `RequestItemCardInfo`, which is
    /// answered while it waits.
    cards: Arc<Mutex<Vec<Value>>>,
    /// The player info last sent.
    player: Option<Value>,
    /// Closing: the menu fades out, then calls `CloseMenu`.
    pub closing: bool,
}

fn object(fields: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect::<BTreeMap<_, _>>(),
    )
}

impl ItemMenu {
    /// Opens the inventory: the menu's extensions, the PC platform, the
    /// categories, then the items and the player's stats. `indices` are the
    /// list positions the menu saved when last closed (`SaveIndices`).
    pub fn open(
        ui: &UiSwf,
        size: (u32, u32),
        engine: &mut Engine,
        indices: &[Value],
        buttons: Arc<Buttons>,
    ) -> anyhow::Result<Self> {
        let menu = ui.open("inventorymenu.swf", size)?;
        let cards: Arc<Mutex<Vec<Value>>> = Default::default();
        let row_cards = cards.clone();
        menu.set_responder(move |call, movie| match call.name.as_str() {
            // The selected item's card.
            "RequestItemCardInfo" => {
                let row = movie.get(&format!("{LISTS}.ItemsList.selectedEntry.vrmRow"));
                let Value::Number(row) = row else {
                    return None;
                };
                let card = row_cards.lock().unwrap().get(row as usize).cloned()?;
                Some(vec![card])
            }
            // A click on the item's model equips it, as the button clicked
            // would on the list (`AttemptEquip`).
            "CheckForMouseEquip" => {
                let button = call.args.first().cloned().unwrap_or(num(0.0));
                Some(vec![button, Value::Bool(false)])
            }
            _ => super::respond(&buttons, call),
        });
        menu.invoke("_root.InitExtensions", &[]);
        menu.invoke("_root.SetPlatform", &[num(0.0), Value::Bool(false)]);
        let mut categories = Vec::new();
        for &(setting, english, flag) in CATEGORIES {
            let name = engine
                .gmst_string(setting)
                .unwrap_or_else(|| english.to_string());
            categories.push(text(&name));
            categories.push(num(flag as f32));
            categories.push(Value::Bool(flag == 0x3FE));
        }
        menu.callback("SetCategoriesList", &categories);
        let mut m = ItemMenu {
            menu,
            rows: Vec::new(),
            listed: None,
            cards,
            player: None,
            closing: false,
        };
        m.update(engine);
        if !indices.is_empty() {
            m.menu.callback("RestoreIndices", indices);
        }
        Ok(m)
    }

    /// Fills the list again when the inventory changed, and the player's
    /// stats when they did.
    pub fn update(&mut self, engine: &mut Engine) {
        let mut rows = engine.listed_inventory(PLAYER_REF);
        // Gold is in the bottom bar, not the list.
        rows.retain(|r| r.item != crate::crime::GOLD);
        let worn = engine
            .inventories
            .get(&PLAYER_REF)
            .map(|i| i.equipped.clone())
            .unwrap_or_default();
        let listed: Listed = (
            rows.iter().map(|r| (r.item, r.owner, r.count)).collect(),
            worn.clone(),
            engine.favorites.clone(),
        );
        if self.listed.as_ref() != Some(&listed) {
            let entries: Vec<Value> = rows
                .iter()
                .enumerate()
                .map(|(i, r)| self.entry(engine, i, r, &worn))
                .collect();
            // The cards first: filling the list asks for the selected one's.
            *self.cards.lock().unwrap() = rows.iter().map(|r| item_card(engine, r)).collect();
            let list = format!("{LISTS}.ItemsList.entryList");
            self.menu.invoke(&format!("{list}.splice"), &[num(0.0)]);
            if !entries.is_empty() {
                self.menu.invoke(&format!("{list}.push"), &entries);
            }
            self.menu.callback("InvalidateListData", &[]);
            self.rows = rows;
            self.listed = Some(listed);
        }
        let player = player_info(engine);
        if self.player.as_ref() != Some(&player) {
            self.menu.callback("UpdatePlayerInfo", &[player.clone()]);
            self.player = Some(player);
        }
    }

    /// A row as the list's entry: its name, count, equip icon, categories and
    /// favorite star; `vrmRow` is its place in `rows`.
    fn entry(&self, engine: &Engine, i: usize, r: &ListedItem, worn: &[FormId]) -> Value {
        let favorite = engine.favorites.contains(&r.item);
        let equip = match worn.contains(&r.item) {
            false => 0.0,
            true if r.info.kind == ItemKind::Weapon => ES_RIGHT,
            true => ES_EQUIPPED,
        };
        let flags = category(engine, r) | if favorite { FAVORITES } else { 0 };
        object([
            ("text", text(&r.info.name)),
            ("count", num(r.count as f32)),
            ("equipState", Value::Number(equip)),
            ("filterFlag", num(flags as f32)),
            ("favorite", Value::Bool(favorite)),
            ("enabled", Value::Bool(true)),
            ("vrmRow", num(i as f32)),
        ])
    }

    /// The row selected in the items list.
    fn selected(&self) -> Option<&ListedItem> {
        match self
            .menu
            .get(&format!("{LISTS}.ItemsList.selectedEntry.vrmRow"))
        {
            Value::Number(n) if n >= 0.0 => self.rows.get(n as usize),
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
        let amount = || match call.args.first() {
            Some(Value::Number(n)) => *n as i32,
            _ => 1,
        };
        match call.name.as_str() {
            // Accept on an item, or a hand's equip button (its argument).
            "ItemSelect" => {
                if let Some(row) = self.selected().cloned() {
                    self.select(engine, &row);
                }
            }
            "ItemDrop" => {
                if let Some(row) = self.selected().cloned() {
                    engine.drop_item(PLAYER_REF, row.item, amount());
                }
            }
            "SaveIndices" => {
                if let Some(Value::List(list)) = call.args.first() {
                    *indices = list.clone();
                }
            }
            "CloseMenu" => {
                if engine.menu == Some(crate::items::Menu::Inventory) {
                    engine.menu = None;
                }
            }
            // Leaving for the tween menu (not built) or the world: the menu
            // fades out and then calls `CloseMenu`.
            "ShowTweenMenu" | "CloseTweenMenu" => self.closing = true,
            // The item's model (not shown yet), a disabled item, the
            // quantity slider: nothing to do.
            "UpdateItem3D" | "StartMouseRotation" | "StopMouseRotation" | "ZoomItemModel"
            | "DisabledItemSelect" | "QuantitySliderOpen" => {}
            _ => return false,
        }
        true
    }

    /// Accept on an item: weapons and apparel are equipped or taken off,
    /// potions, food and ingredients used, books read.
    fn select(&mut self, engine: &mut Engine, row: &ListedItem) {
        let worn = engine
            .inventories
            .get(&PLAYER_REF)
            .is_some_and(|i| i.equipped.contains(&row.item));
        let result = match row.info.kind {
            ItemKind::Weapon | ItemKind::Ammo | ItemKind::Armor => {
                engine.equip_item(PLAYER_REF, row.item, !worn)
            }
            ItemKind::Potion | ItemKind::Ingredient => engine.consume(PLAYER_REF, row.item),
            ItemKind::Book => {
                engine.menu = Some(crate::items::Menu::Book {
                    book: row.item,
                    reference: None,
                });
                engine.read_book(row.item);
                Ok(())
            }
            _ => Ok(()),
        };
        if let Err(e) = result {
            engine.scripts.notify(e);
        }
    }

    /// The game's item menu keys: drop (`DropItem`), mark a favorite, equip with
    /// a mouse button (`AttemptEquip`, the hand's slot). Closing on Esc fades
    /// the menu out first, as Tab does.
    pub fn drop_key(&self) {
        self.menu.callback("DropItem", &[]);
    }

    pub fn favorite_key(&mut self, engine: &mut Engine) {
        let Some(item) = self.selected().map(|r| r.item) else {
            return;
        };
        if let Some(i) = engine.favorites.iter().position(|&f| f == item) {
            engine.favorites.remove(i);
        } else {
            engine.favorites.push(item);
        }
        self.update(engine);
        // The bottom bar's Favorite / Unfavorite.
        self.menu
            .invoke(&format!("{M}.UpdateBottomBarButtons"), &[]);
    }

    pub fn equip_button(&self, slot: f32) {
        self.menu
            .callback("AttemptEquip", &[num(slot), Value::Bool(true)]);
    }

    pub fn close(&mut self) {
        if !self.closing {
            self.closing = true;
            self.menu.invoke(&format!("{M}.StartMenuFade"), &[]);
        }
    }
}

/// The bottom bar's player stats: armor rating, the weapon's damage, carry
/// weight, gold, and health, magicka and stamina.
fn player_info(engine: &Engine) -> Value {
    let weight: f32 = engine
        .inventories
        .get(&PLAYER_REF)
        .map(|inv| {
            inv.items
                .iter()
                .filter_map(|&(f, n)| {
                    crate::world::inventory::item_info(&engine.lo, f).map(|i| i.weight * n as f32)
                })
                .sum()
        })
        .unwrap_or(0.0);
    let damage = engine
        .player_weapon()
        .map_or(0.0, |w| crate::ai::combat::weapon_damage(&engine.lo, w));
    let stat = |index| {
        (
            num(engine.actor_value(PLAYER_REF, index)),
            num(engine.av_max(PLAYER_REF, index)),
        )
    };
    let (health, max_health) = stat(av::HEALTH);
    let (magicka, max_magicka) = stat(av::MAGICKA);
    let (stamina, max_stamina) = stat(av::STAMINA);
    object([
        ("armor", num(engine.protection(PLAYER_REF).rating.floor())),
        ("damage", num(damage)),
        ("encumbrance", num(weight)),
        (
            "maxEncumbrance",
            num(engine.actor_value(PLAYER_REF, av::CARRY_WEIGHT)),
        ),
        (
            "gold",
            num(engine.item_count(PLAYER_REF, crate::crime::GOLD) as f32),
        ),
        ("health", health),
        ("maxHealth", max_health),
        ("magicka", magicka),
        ("maxMagicka", max_magicka),
        ("stamina", stamina),
        ("maxStamina", max_stamina),
    ])
}

/// The item card: name, count, value and weight, and by kind the armor rating,
/// damage, effects or description, with how much worn armor or the weapon
/// held would change.
fn item_card(engine: &Engine, row: &ListedItem) -> Value {
    let item = row.item;
    let mut f: Vec<(&'static str, Value)> = vec![
        ("name", text(&row.info.name)),
        ("count", num(row.count as f32)),
        ("value", num(row.info.value as f32)),
        ("weight", num(row.info.weight)),
        ("stolen", Value::Bool(row.owner.is_some())),
    ];
    let worn = engine
        .inventories
        .get(&PLAYER_REF)
        .is_some_and(|i| i.equipped.contains(&item));
    let kind = match row.info.kind {
        ItemKind::Armor => {
            let none = engine.protection_of(PLAYER_REF, &[]).rating;
            let rating = engine.protection_of(PLAYER_REF, &[item]).rating - none;
            f.push(("armor", num(rating.round())));
            if !worn {
                let now = engine.protection(PLAYER_REF).rating;
                let with = engine
                    .protection_of(PLAYER_REF, &engine.worn_with(PLAYER_REF, item))
                    .rating;
                f.push(("armorChange", num(with - now)));
            }
            f.push(("effects", text(&engine.effects_text(item))));
            f.push(("skillText", text("")));
            card::ARMOR
        }
        ItemKind::Weapon | ItemKind::Ammo => {
            let damage = crate::ai::combat::weapon_damage(&engine.lo, item);
            f.push(("damage", num(damage.round())));
            if row.info.kind == ItemKind::Weapon && !worn {
                let now = engine
                    .player_weapon()
                    .map_or(0.0, |w| crate::ai::combat::weapon_damage(&engine.lo, w));
                f.push(("damageChange", num(damage - now)));
            }
            f.push(("effects", text(&engine.effects_text(item))));
            card::WEAPON
        }
        ItemKind::Potion => {
            f.push(("effects", text(&engine.effects_text(item))));
            // Which meter the bottom bar shows: the first effect's (health 0,
            // magicka 1, stamina 2), else health's.
            let restores = engine
                .magic_item(item)
                .and_then(|m| m.effects.first().map(|e| e.effect))
                .and_then(|e| engine.magic_effect(e))
                .and_then(|e| e.primary);
            let meter = match restores {
                Some(av::MAGICKA) => 1.0,
                Some(av::STAMINA) => 2.0,
                _ => 0.0,
            };
            f.push(("potionType", num(meter)));
            if engine.is_food(item) {
                card::FOOD
            } else {
                card::POTION
            }
        }
        ItemKind::Ingredient => {
            // Effects the player hasn't learned show as unknown.
            let n = engine.magic_item(item).map_or(0, |m| m.effects.len());
            f.push(("numItemEffects", num(n as f32)));
            card::INGREDIENT
        }
        ItemKind::Book => {
            let description = engine
                .lo
                .get(item)
                .and_then(|r| r.get(b"CNAM").map(|d| engine.lo.lstring(&r, d)))
                .unwrap_or_default();
            f.push(("description", text(&description)));
            card::BOOK
        }
        ItemKind::Key => card::KEY,
        ItemKind::Scroll | ItemKind::SoulGem | ItemKind::Light | ItemKind::Misc => card::MISC,
    };
    f.push(("type", Value::Number(kind)));
    object(f)
}
