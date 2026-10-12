//! The item menus as the game shows them: the inventory (`inventorymenu.swf`),
//! a container, a body, a teammate's inventory or someone's pockets
//! (`containermenu.swf`), a merchant's wares (`bartermenu.swf`) and gifts
//! (`giftmenu.swf`: the giver's items only). They share
//! the categories on the left, the items of one beside them, the item card and
//! the bottom bar with the player's stats.
//!
//! The game fills a menu: `SetCategoriesList` (name, filter flag, always
//! shown, for each category; the container and barter menus list the other
//! side's categories, a divider (flag 0), then the player's), then the items
//! list's `entryList` and `InvalidateListData`. The menu asks for the
//! selected item's card (`RequestItemCardInfo`) and says what the player does
//! (`ItemSelect`, `ItemDrop`, `ItemTransfer`, `CloseMenu`...). Equipping with
//! the mouse, dropping, taking all and marking favorites are the game's keys,
//! which it passes on as `AttemptEquip`, `DropItem` and `XButtonPress` or
//! handles itself. What is guessed is in `known_gaps/swf-hud.md`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use esp::FormId;
use esp::actor_value as av;
use ui_swf::{GameCall, Menu, UiSwf, Value};

use crate::barter::Vendor;
use crate::crime::GOLD;
use crate::engine::{Engine, PLAYER_REF};
use crate::items::{ListedItem, Menu as Shown};
use crate::world::inventory::ItemKind;

use super::{Buttons, num, text};

/// The menu's root clip (`InventoryMenu`, `ContainerMenu`, `BarterMenu`) and
/// its lists.
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
const ALL: u32 = 0x3FE;
const CATEGORIES: &[(&str, &str, u32)] = &[
    ("sFavorites", "Favorites", FAVORITES),
    ("sAll", "All", ALL),
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

/// The other side's categories (a container's, a merchant's) are the
/// player's moved up past them.
const THEIRS: u32 = 10;

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

/// Which menu, and whose items are on the other side.
#[derive(Debug, Clone)]
enum Kind {
    Inventory,
    /// A container or a body; a living actor's inventory is a teammate's to
    /// trade with (the menu's NPC mode: Give in place of Store).
    Container {
        owner: FormId,
        npc: bool,
    },
    /// Someone's pockets.
    Pickpocket {
        victim: FormId,
    },
    /// A merchant: their wares are in `stock` (the merchant chest).
    Barter {
        vendor: FormId,
        stock: FormId,
        info: Option<Vendor>,
    },
    /// Gifts to or from an actor (`Engine::gift`): only the giver's items.
    Gift {
        actor: FormId,
        giving: bool,
    },
}

impl Kind {
    fn of(engine: &Engine, shown: Shown) -> Option<Kind> {
        Some(match shown {
            Shown::Inventory => Kind::Inventory,
            Shown::Container(owner) => {
                let container = engine
                    .base_of(owner)
                    .and_then(|b| engine.lo.tag_of(b))
                    .is_some_and(|t| t.0 == *b"CONT");
                Kind::Container {
                    owner,
                    npc: !container && !engine.is_dead(owner),
                }
            }
            Shown::Pickpocket(victim) => Kind::Pickpocket { victim },
            Shown::Gift(actor) => Kind::Gift {
                actor,
                giving: engine.gift.as_ref()?.giving,
            },
            Shown::Barter(vendor) => Kind::Barter {
                vendor,
                stock: engine.vendor_stock(vendor),
                info: engine.vendor_of(vendor),
            },
            _ => return None,
        })
    }

    fn movie(&self) -> &'static str {
        match self {
            Kind::Inventory => "inventorymenu.swf",
            Kind::Container { .. } | Kind::Pickpocket { .. } => "containermenu.swf",
            Kind::Barter { .. } => "bartermenu.swf",
            Kind::Gift { .. } => "giftmenu.swf",
        }
    }

    /// Whose items are on the other side (the gift menu has one side).
    fn other(&self) -> Option<FormId> {
        match self {
            Kind::Inventory | Kind::Gift { .. } => None,
            Kind::Container { owner, .. } => Some(*owner),
            Kind::Pickpocket { victim } => Some(*victim),
            Kind::Barter { stock, .. } => Some(*stock),
        }
    }
}

/// A row of the list: the other side's (a container's...) or the player's.
#[derive(Debug, Clone)]
struct Row {
    item: ListedItem,
    theirs: bool,
}

/// What the list shows, to know when it must be filled again: each row's side,
/// item, owner and count, what is worn, the favorites.
type Listed = (
    Vec<(bool, FormId, Option<FormId>, i32)>,
    Vec<FormId>,
    Vec<FormId>,
);

pub(super) struct ItemMenu {
    pub menu: Menu,
    /// The engine's menu this shows.
    pub shows: Shown,
    kind: Kind,
    /// The rows, in the order of the menu's `entryList`.
    rows: Vec<Row>,
    listed: Option<Listed>,
    /// Each row's item card, for the menu's `RequestItemCardInfo`, which is
    /// answered while it waits.
    cards: Arc<Mutex<Vec<Value>>>,
    /// The merchant's gold, for the warning when they can't pay.
    vendor_gold: Arc<Mutex<i32>>,
    /// The player info last sent.
    player: Option<Vec<Value>>,
    /// Closing: the inventory fades out, then calls `CloseMenu`.
    pub closing: bool,
    /// Closing for the tween menu (`ShowTweenMenu`), not the world.
    to_tween: bool,
}

pub(super) fn object(fields: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect::<BTreeMap<_, _>>(),
    )
}

/// The categories for `SetCategoriesList`: (name, flag, always listed) each.
fn categories(engine: &Engine, theirs: bool) -> Vec<Value> {
    let mut out = Vec::new();
    for &(setting, english, flag) in CATEGORIES {
        // The other side has no favorites.
        if theirs && flag == FAVORITES {
            continue;
        }
        let name = engine
            .gmst_string(setting)
            .unwrap_or_else(|| english.to_string());
        out.push(text(&name));
        out.push(num((if theirs { flag << THEIRS } else { flag }) as f32));
        out.push(Value::Bool(flag == ALL));
    }
    out
}

impl ItemMenu {
    /// Whether `shown` is one of these menus.
    pub fn shows(shown: Shown) -> bool {
        matches!(
            shown,
            Shown::Inventory
                | Shown::Container(_)
                | Shown::Pickpocket(_)
                | Shown::Barter(_)
                | Shown::Gift(_)
        )
    }

    /// Opens the menu for `shown`: its extensions, the PC platform, the
    /// categories, a merchant's prices, then the items and the player's
    /// stats. `indices` are the inventory's list positions saved when it last
    /// closed (`SaveIndices`).
    pub fn open(
        ui: &UiSwf,
        size: (u32, u32),
        engine: &mut Engine,
        shown: Shown,
        indices: &[Value],
        buttons: Arc<Buttons>,
    ) -> anyhow::Result<Self> {
        let kind = Kind::of(engine, shown).ok_or_else(|| anyhow::anyhow!("not an item menu"))?;
        let menu = ui.open(kind.movie(), size)?;
        let cards: Arc<Mutex<Vec<Value>>> = Default::default();
        let vendor_gold: Arc<Mutex<i32>> = Default::default();
        let (row_cards, gold) = (cards.clone(), vendor_gold.clone());
        let raw_deal = engine
            .gmst_string("sNotEnoughVendorGold")
            .unwrap_or_else(|| "Transaction value: %d gold.\nVendor only has %d gold.".into())
            .replace("\r\n", "\n");
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
            // Selling for more than the merchant has: the warning to confirm.
            "GetRawDealWarningString" => {
                let total = match call.args.first() {
                    Some(Value::Number(n)) => *n as i64,
                    _ => 0,
                };
                let warning = raw_deal.replacen("%d", &total.to_string(), 1).replacen(
                    "%d",
                    &gold.lock().unwrap().to_string(),
                    1,
                );
                Some(vec![text(&warning)])
            }
            _ => super::respond(&buttons, call),
        });
        // The NPC mode: a teammate's inventory gives in place of storing, and
        // pockets have no Take All.
        let npc = match kind {
            Kind::Container { npc, .. } => npc,
            Kind::Pickpocket { .. } => true,
            _ => false,
        };
        if npc {
            menu.set(&format!("{M}.bNPCMode"), Value::Bool(true));
        }
        // The gift menu's root has no functions of its own: its menu clip's.
        let root = match kind {
            Kind::Gift { .. } => M,
            _ => "_root",
        };
        menu.invoke(&format!("{root}.InitExtensions"), &[]);
        menu.invoke(
            &format!("{root}.SetPlatform"),
            &[num(0.0), Value::Bool(false)],
        );
        let mut list = Vec::new();
        if kind.other().is_some() {
            list.extend(categories(engine, true));
            // The divider between the other side's and the player's.
            list.extend([text(""), num(0.0), Value::Bool(true)]);
        }
        list.extend(categories(engine, false));
        menu.callback("SetCategoriesList", &list);
        if let Kind::Barter { vendor, .. } = kind {
            let (buy, sell) = engine.barter_multipliers(vendor);
            menu.callback("SetBarterMultipliers", &[num(buy), num(sell)]);
        }
        // Give or take, and whether favor points count (the bottom bar's
        // stats are hidden when they don't).
        if let Some(gift) = engine
            .gift
            .as_ref()
            .filter(|_| matches!(kind, Kind::Gift { .. }))
        {
            menu.callback(
                "SetMenuInfo",
                &[Value::Bool(gift.giving), Value::Bool(gift.favor)],
            );
        }
        let mut m = ItemMenu {
            menu,
            shows: shown,
            kind,
            rows: Vec::new(),
            listed: None,
            cards,
            vendor_gold,
            player: None,
            closing: false,
            to_tween: false,
        };
        m.update(engine);
        if m.shows == Shown::Inventory && !indices.is_empty() {
            m.menu.callback("RestoreIndices", indices);
        }
        Ok(m)
    }

    /// The rows: the other side's, then the player's; the giver's in the
    /// gift menu.
    fn rows(&self, engine: &mut Engine) -> Vec<Row> {
        let mut rows = Vec::new();
        if let Kind::Gift { actor, giving } = self.kind {
            let Some(gift) = engine.gift.clone() else {
                return rows;
            };
            let giver = if giving { PLAYER_REF } else { actor };
            let mut items = engine.listed_inventory(giver);
            items.retain(|r| engine.giftable(&gift, r));
            // The one list is the player's side, whoever gives.
            rows.extend(items.into_iter().map(|item| Row {
                item,
                theirs: false,
            }));
            return rows;
        }
        if let Some(other) = self.kind.other() {
            let mut theirs = engine.listed_inventory(other);
            match &self.kind {
                // What isn't worn or wielded can be picked (and what is,
                // with the perks for it).
                Kind::Pickpocket { victim } => {
                    theirs.retain(|x| engine.can_pickpocket_item(*victim, x.item));
                }
                // Gold is the bottom bar's.
                Kind::Barter { .. } => theirs.retain(|x| x.item != GOLD),
                _ => {}
            }
            rows.extend(theirs.into_iter().map(|item| Row { item, theirs: true }));
        }
        let mut mine = engine.listed_inventory(PLAYER_REF);
        match &self.kind {
            Kind::Inventory => mine.retain(|r| r.item != GOLD),
            // What the merchant trades in.
            Kind::Barter { info, .. } => mine.retain(|r| {
                info.as_ref()
                    .is_some_and(|v| engine.vendor_trades(v, r.item, r.owner.is_some()))
            }),
            _ => {}
        }
        rows.extend(mine.into_iter().map(|item| Row {
            item,
            theirs: false,
        }));
        rows
    }

    /// Fills the list again when the items changed, and the player's stats
    /// when they did.
    pub fn update(&mut self, engine: &mut Engine) {
        // The stats first: the bottom bar compares the card's item with them.
        let stats = player_info(engine);
        let player = match &self.kind {
            // The player's and the merchant's gold, the merchant's name, then
            // the stats.
            Kind::Barter { vendor, stock, .. } => {
                let gold = engine.gold(*stock);
                *self.vendor_gold.lock().unwrap() = gold;
                vec![
                    num(engine.gold(PLAYER_REF) as f32),
                    num(gold as f32),
                    text(&engine.actor_name(*vendor).unwrap_or_default()),
                    stats,
                ]
            }
            // The favor points (the bottom bar only shows that it's a gift).
            Kind::Gift { .. } => {
                vec![num(engine.gift.as_ref().map_or(0, |g| g.points) as f32)]
            }
            _ => vec![stats],
        };
        if self.player.as_ref() != Some(&player) {
            self.menu.callback("UpdatePlayerInfo", &player);
            self.player = Some(player);
        }
        let rows = self.rows(engine);
        let worn = engine
            .inventories
            .get(&PLAYER_REF)
            .map(|i| i.equipped.clone())
            .unwrap_or_default();
        let listed: Listed = (
            rows.iter()
                .map(|r| (r.theirs, r.item.item, r.item.owner, r.item.count))
                .collect(),
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
            *self.cards.lock().unwrap() = rows.iter().map(|r| self.card(engine, r)).collect();
            let list = format!("{LISTS}.ItemsList.entryList");
            self.menu.invoke(&format!("{list}.splice"), &[num(0.0)]);
            if !entries.is_empty() {
                self.menu.invoke(&format!("{list}.push"), &entries);
            }
            self.menu.callback("InvalidateListData", &[]);
            self.rows = rows;
            self.listed = Some(listed);
        }
    }

    /// A row as the list's entry: its name, count, equip icon, categories,
    /// favorite star and whether taking it is stealing (red); `vrmRow` is its
    /// place in `rows`.
    fn entry(&self, engine: &Engine, i: usize, r: &Row, worn: &[FormId]) -> Value {
        let item = &r.item;
        let favorite = !r.theirs && engine.favorites.contains(&item.item);
        let equip = match !r.theirs && worn.contains(&item.item) {
            false => 0.0,
            true if item.info.kind == ItemKind::Weapon => ES_RIGHT,
            true => ES_EQUIPPED,
        };
        let mut flags = category(engine, item) | if favorite { FAVORITES } else { 0 };
        if r.theirs {
            flags <<= THEIRS;
        }
        let stealing = r.theirs
            && match &self.kind {
                Kind::Pickpocket { .. } => true,
                Kind::Container { owner, .. } => {
                    engine.stolen_from(item.item, *owner, true).is_some()
                }
                _ => false,
            };
        object([
            ("text", text(&item.info.name)),
            ("count", num(item.count as f32)),
            ("equipState", Value::Number(equip)),
            ("filterFlag", num(flags as f32)),
            ("favorite", Value::Bool(favorite)),
            ("enabled", Value::Bool(true)),
            ("isStealing", Value::Bool(stealing)),
            ("vrmRow", num(i as f32)),
        ])
    }

    /// A row's item card: the item's, with whether it's a favorite and, when
    /// pickpocketing, the chance to take one (or place one: the menu says
    /// "TO PLACE" on the player's side).
    fn card(&self, engine: &Engine, r: &Row) -> Value {
        let mut card = item_card(engine, &r.item);
        if let Value::Object(fields) = &mut card {
            let favorite = !r.theirs && engine.favorites.contains(&r.item.item);
            fields.insert("favorite".into(), Value::Bool(favorite));
            if let Kind::Pickpocket { victim } = &self.kind {
                let chance = engine.pickpocket_chance(*victim, r.item.item, 1);
                fields.insert("pickpocketChance".into(), num(chance.round()));
            }
        }
        card
    }

    /// The row selected in the items list.
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
        let number = |i: usize| match call.args.get(i) {
            Some(Value::Number(n)) => Some(*n),
            _ => None,
        };
        let amount = || number(0).map_or(1, |n| n as i32);
        match call.name.as_str() {
            // The inventory: Accept on an item, or a hand's equip button (its
            // argument). Bartering: buying or selling (amount, price each,
            // buying).
            "ItemSelect" => {
                let Some(row) = self.selected() else {
                    return true;
                };
                if let Kind::Gift { .. } = self.kind {
                    engine.give_gift(&row.item, amount());
                } else if let Kind::Barter { vendor, .. } = self.kind {
                    let price = number(1).unwrap_or(0.0).round() as i32;
                    let buying = matches!(call.args.get(2), Some(Value::Bool(true)));
                    let (item, owner) = (row.item.item, row.item.owner);
                    if let Err(e) = engine.barter(vendor, item, owner, amount(), price, buying) {
                        engine.scripts.notify(e);
                    }
                    engine.unequip_if_gone(PLAYER_REF, item);
                } else {
                    engine.use_listed_item(&row.item);
                }
            }
            "ItemDrop" => {
                if let Some(row) = self.selected() {
                    engine.drop_item(PLAYER_REF, row.item.item, amount());
                    engine.unequip_if_gone(PLAYER_REF, row.item.item);
                }
            }
            // Taking (from the other side) or storing so many.
            "ItemTransfer" => {
                if let Some(row) = self.selected() {
                    self.transfer(engine, &row, amount());
                }
            }
            "TakeAllItems" => {
                for row in self.rows.clone().into_iter().filter(|r| r.theirs) {
                    self.transfer(engine, &row, row.item.count);
                }
            }
            // Equipping from the container menu (hand, then the amount when
            // taken first).
            "EquipItem" => {
                if let Some(row) = self.selected() {
                    if row.theirs {
                        self.transfer(engine, &row, number(1).map_or(1, |n| n as i32));
                    }
                    engine.use_listed_item(&row.item);
                }
            }
            "SaveIndices" => {
                if let Some(Value::List(list)) = call.args.first() {
                    *indices = list.clone();
                }
            }
            "CloseMenu" => {
                if engine.menu == Some(self.shows) {
                    engine.menu = self.to_tween.then_some(crate::items::Menu::Tween);
                }
            }
            // Leaving the inventory for the tween menu or the world: it fades
            // out and then calls `CloseMenu`.
            "ShowTweenMenu" => {
                self.closing = true;
                self.to_tween = true;
            }
            "CloseTweenMenu" => self.closing = true,
            // The item's model (not shown yet), a disabled item, the
            // quantity slider: nothing to do.
            "UpdateItem3D" | "StartMouseRotation" | "StopMouseRotation" | "ZoomItemModel"
            | "DisabledItemSelect" | "QuantitySliderOpen" => {}
            _ => return false,
        }
        true
    }

    /// Moves `count` of a row across: taken from the other side (picked from
    /// pockets), or stored there.
    fn transfer(&self, engine: &mut Engine, row: &Row, count: i32) {
        let Some(other) = self.kind.other() else {
            return;
        };
        let (item, owner) = (row.item.item, Some(row.item.owner));
        match (&self.kind, row.theirs) {
            (Kind::Pickpocket { victim }, true) => {
                engine.try_pickpocket(*victim, item, count, owner);
            }
            (Kind::Pickpocket { victim }, false) => {
                engine.try_place_in_pocket(*victim, item, count, owner);
            }
            (_, true) => {
                engine.transfer_item(other, PLAYER_REF, item, count, owner);
            }
            (_, false) => {
                engine.transfer_item(PLAYER_REF, other, item, count, owner);
                engine.unequip_if_gone(PLAYER_REF, item);
                engine.take_up_if_better(other, item);
            }
        }
    }

    /// The control map's Ready Weapon key: the inventory drops
    /// (`DropItem`), the container menu takes all or stores
    /// (`XButtonPress`).
    pub fn x_button(&self) {
        match self.kind {
            Kind::Inventory => self.menu.callback("DropItem", &[]),
            Kind::Container { .. } | Kind::Pickpocket { .. } => {
                self.menu.callback("XButtonPress", &[])
            }
            Kind::Barter { .. } | Kind::Gift { .. } => Value::Undefined,
        };
    }

    /// The control map's Toggle POV key: marks one of the player's items a
    /// favorite or not (not while bartering).
    pub fn favorite_key(&mut self, engine: &mut Engine) {
        let Some(row) = self.selected().filter(|r| !r.theirs) else {
            return;
        };
        if matches!(self.kind, Kind::Barter { .. } | Kind::Gift { .. }) {
            return;
        }
        let item = row.item.item;
        engine.toggle_favorite(item);
        self.update(engine);
        // The bottom bar's Favorite / Unfavorite.
        match self.kind {
            Kind::Inventory => self
                .menu
                .invoke(&format!("{M}.UpdateBottomBarButtons"), &[]),
            _ => self.menu.invoke(&format!("{M}.updateButtons"), &[]),
        };
    }

    /// A mouse button's equip (`AttemptEquip`, the hand's slot); the gift
    /// menu equips nothing.
    pub fn equip_button(&self, slot: f32) {
        if matches!(self.kind, Kind::Gift { .. }) {
            return;
        }
        self.menu
            .callback("AttemptEquip", &[num(slot), Value::Bool(true)]);
    }

    /// Esc: the inventory fades out as on Tab; the others close on Tab.
    pub fn close(&mut self) {
        if self.closing {
            return;
        }
        if self.shows == Shown::Inventory {
            self.closing = true;
            self.menu.invoke(&format!("{M}.StartMenuFade"), &[]);
        } else {
            use ui_swf::PlayerEvent;
            for key in ui_swf::input::keys(winit::keyboard::KeyCode::Tab) {
                self.menu.handle_event(PlayerEvent::KeyDown { key });
                self.menu.handle_event(PlayerEvent::KeyUp { key });
            }
        }
    }
}

/// The weapon the player has equipped (not the one they'd fight with when
/// none is).
fn held_weapon(engine: &Engine) -> Option<FormId> {
    let inv = engine.inventories.get(&PLAYER_REF)?;
    inv.weapon(&engine.lo).filter(|&w| inv.count(w) > 0)
}

/// The bottom bar's player stats: armor rating, the weapon's damage, carry
/// weight, gold, and health, magicka and stamina.
pub(super) fn player_info(engine: &Engine) -> Value {
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
    let damage =
        held_weapon(engine).map_or(0.0, |w| crate::ai::combat::weapon_damage(&engine.lo, w));
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
                let now = held_weapon(engine)
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
