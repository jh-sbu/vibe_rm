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
//! handles itself. What is guessed is in `known_ambiguities/swf-hud.md`.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use esp::FormId;
use esp::actor_value as av;
use ui_swf::{GameCall, Menu, UiSwf, Value};

use crate::barter::Vendor;
use crate::crime::GOLD;
use crate::engine::{Engine, PLAYER_REF};
use crate::items::{ListedItem, Menu as Shown};
use crate::world::inventory::{Inventory, ItemKind, LEFT, RIGHT};

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
    pub const SPELL: f64 = 7.0;
    pub const INGREDIENT: f64 = 8.0;
    pub const KEY: f64 = 9.0;
    pub const SOUL_GEMS: f64 = 12.0;
    /// The card's own list (the souls to charge a weapon with).
    pub const LIST: f64 = 14.0;
}

/// Equip icons (`InventoryDefines.ES_*`): worn, wielded in the left hand,
/// the right, both.
const ES_EQUIPPED: f64 = 1.0;
const ES_LEFT: f64 = 2.0;
const ES_RIGHT: f64 = 3.0;
const ES_BOTH: f64 = 4.0;

/// An item's equip icon: a weapon by the hands it is in (one that fills both
/// in the right, where the game wears it, the left hand being for a second
/// one-handed weapon), anything else worn or not.
pub(super) fn equip_state(engine: &Engine, inv: &Inventory, item: FormId) -> f64 {
    if !inv.is_equipped(item) {
        return 0.0;
    }
    if engine.lo.tag_of(item).map(|t| t.0) != Some(*b"WEAP") {
        return ES_EQUIPPED;
    }
    let right = inv.held(RIGHT) == Some(item);
    let left = inv.held(LEFT) == Some(item) && !inv.both_hands();
    match (right, left) {
        (true, true) => ES_BOTH,
        (false, true) => ES_LEFT,
        _ => ES_RIGHT,
    }
}

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

    /// The inventory a row comes from; gifts have one side even when the
    /// actor gives to the player.
    fn source(&self, theirs: bool) -> FormId {
        match self {
            Kind::Gift {
                actor,
                giving: false,
            } => *actor,
            _ if theirs => self.other().unwrap_or(PLAYER_REF),
            _ => PLAYER_REF,
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
/// item, owner and count, what is worn and what is in hand, the favorites,
/// the soul shown on each gem's card, the poison icon on each weapon's and
/// the best-in-class star.
type Listed = (
    Vec<(bool, FormId, Option<FormId>, i32)>,
    Vec<FormId>,
    [Option<FormId>; 3],
    Vec<FormId>,
    Vec<Option<u8>>,
    Vec<bool>,
    Vec<Option<i32>>,
    Vec<bool>,
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
    /// Charging: the weapon, and the souls listed on the card (gem, soul).
    charging: Option<(FormId, Vec<(FormId, u8)>)>,
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
        super::set_button_mapping(&menu, &buttons);
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
            charging: None,
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
        let inv = engine
            .inventories
            .get(&PLAYER_REF)
            .cloned()
            .unwrap_or_default();
        let best = best_in_class(engine, &rows);
        let listed = listed_state(
            &rows,
            &inv,
            &engine.favorites,
            |r| self.soul_level(engine, r),
            |r| row_poisoned(&self.kind, r, &engine.inventories),
            |r| self.charge(engine, r).map(|c| (c * 100.0).round() as i32),
            &best,
        );
        if self.listed.as_ref() != Some(&listed) {
            let entries: Vec<Value> = rows
                .iter()
                .enumerate()
                .map(|(i, r)| self.entry(engine, i, r, &inv, best[i]))
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
    fn entry(&self, engine: &Engine, i: usize, r: &Row, inv: &Inventory, best: bool) -> Value {
        let item = &r.item;
        let favorite = !r.theirs && engine.favorites.contains(&item.item);
        let equip = match r.theirs {
            true => 0.0,
            false => equip_state(engine, inv, item.item),
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
            // The list adds `BestIcon.png` after the name.
            ("bestInClass", Value::Bool(best)),
            ("vrmRow", num(i as f32)),
        ])
    }

    /// A row's item card: the item's, with whether it's a favorite and, when
    /// pickpocketing, the chance to take one (or place one: the menu says
    /// "TO PLACE" on the player's side).
    fn card(&self, engine: &Engine, r: &Row) -> Value {
        let mut card = item_card(
            engine,
            &r.item,
            self.soul_level(engine, r),
            row_poisoned(&self.kind, r, &engine.inventories),
        );
        if let Value::Object(fields) = &mut card {
            let favorite = !r.theirs && engine.favorites.contains(&r.item.item);
            fields.insert("favorite".into(), Value::Bool(favorite));
            if let Kind::Pickpocket { victim } = &self.kind {
                let chance = engine.pickpocket_chance(*victim, r.item.item, 1);
                fields.insert("pickpocketChance".into(), num(chance.round()));
            }
            // An enchanted weapon's charge meter (the percent left; the
            // bottom bar offers Charge under 100).
            if let Some(charge) = self.charge(engine, r) {
                fields.insert("charge".into(), num(charge));
                fields.insert("usedCharge".into(), num(charge));
            }
        }
        card
    }

    /// A weapon row's charge, as a percent of its most, if it holds one.
    fn charge(&self, engine: &Engine, r: &Row) -> Option<f32> {
        if r.item.info.kind != ItemKind::Weapon {
            return None;
        }
        engine.charge_percent(self.kind.source(r.theirs), r.item.item)
    }

    /// The control map's Charge Item key (the Wait key): the inventory's
    /// selected weapon is charged (`AttemptChargeItem` asks for the souls
    /// when it is short of its most).
    pub fn charge_key(&self) {
        if matches!(self.kind, Kind::Inventory) && self.charging.is_none() {
            self.menu.callback("AttemptChargeItem", &[]);
        }
    }

    /// The card's list of souls is up (Tab, not Esc, leaves it).
    pub fn listing(&self) -> bool {
        self.charging.is_some()
    }

    /// The souls to charge the selected weapon with, on the card's list:
    /// each gem and soul, with how much of the meter it fills.
    fn show_souls(&mut self, engine: &mut Engine) {
        let Some(row) = self.selected().filter(|r| !r.theirs) else {
            return;
        };
        let weapon = row.item.item;
        let Some(now) = engine.charge_percent(PLAYER_REF, weapon) else {
            return;
        };
        let souls = engine.charging_souls(PLAYER_REF);
        if souls.is_empty() {
            let msg = engine
                .gmst_string("sNoChargeItems")
                .unwrap_or_else(|| "No items to charge enchantment".into());
            engine.scripts.notify(msg);
            return;
        }
        let items: Vec<Value> = souls
            .iter()
            .map(|&(gem, soul)| {
                let name = engine.form_name(gem);
                let size = soul_name(Some(soul), |s| engine.gmst_string(s));
                object([
                    ("text", text(&format!("{name} ({size})"))),
                    (
                        "chargeAdded",
                        num(engine.charge_added_percent(PLAYER_REF, weapon, soul)),
                    ),
                ])
            })
            .collect();
        let card = object([
            ("type", Value::Number(card::LIST)),
            ("listItems", Value::List(items)),
            ("currentCharge", num(now)),
        ]);
        self.menu.callback("UpdateItemCardInfo", &[card]);
        self.charging = Some((weapon, souls));
    }

    fn soul_level(&self, engine: &Engine, r: &Row) -> Option<u8> {
        if r.item.info.kind != ItemKind::SoulGem {
            return None;
        }
        let base = engine
            .lo
            .get(r.item.item)
            .and_then(|rec| rec.get(b"SOUL").and_then(|d| d.first()).copied())
            .unwrap_or(0);
        row_soul_level(&self.kind, r, &engine.inventories, base)
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
                    engine.use_listed_item(&row.item, number(0).map(hand));
                }
            }
            // Charge Item on a weapon short of its most.
            "ShowSoulGemList" => self.show_souls(engine),
            // A soul picked from the card's list (its index), or the list
            // left (the card asks for itself again).
            "ItemCardListCallback" => {
                let Some((weapon, souls)) = self.charging.take() else {
                    return true;
                };
                let picked = number(0).and_then(|i| souls.get(i as usize).copied());
                if let Some((gem, soul)) = picked
                    && engine.recharge(PLAYER_REF, weapon, gem, soul)
                {
                    self.update(engine);
                    if let Some(card) = self.selected().map(|r| self.card(engine, &r)) {
                        self.menu.callback("UpdateItemCardInfo", &[card]);
                    }
                }
            }
            "QuantitySliderOpen" if self.charging.is_some() => {
                // The card's list closing without a pick (Tab).
                if matches!(call.args.first(), Some(Value::Bool(false))) {
                    self.charging = None;
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
                    engine.use_listed_item(&row.item, number(0).map(hand));
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

/// `AttemptEquip`'s slot as a hand: 0 the right (`ContainerMenu.RIGHT_HAND`),
/// 1 the left.
fn hand(slot: f64) -> usize {
    if slot as usize == 1 { LEFT } else { RIGHT }
}

/// The weapon in the player's right hand.
fn held_weapon(engine: &Engine) -> Option<FormId> {
    let inv = engine.inventories.get(&PLAYER_REF)?;
    inv.weapon(&engine.lo).filter(|&w| inv.count(w) > 0)
}

/// The bottom bar's player stats: armor rating, the weapon's damage, carry
/// weight, gold, and health, magicka and stamina.
pub(super) fn player_info(engine: &Engine) -> Value {
    let weight = engine.carried_weight(PLAYER_REF);
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
        ("dragonSoulText", text(&dragon_souls_text(engine))),
    ])
}

/// "3 Dragon Souls" (`sMultipleDragonSoulCount`, `sSingleDragonSoulCount`).
fn dragon_souls_text(engine: &Engine) -> String {
    let n = engine
        .actor_value(PLAYER_REF, av::DRAGON_SOULS)
        .max(0.0)
        .floor() as i64;
    let (name, english) = if n == 1 {
        ("sSingleDragonSoulCount", "%i Dragon Soul")
    } else {
        ("sMultipleDragonSoulCount", "%i Dragon Souls")
    };
    engine
        .gmst_string(name)
        .unwrap_or_else(|| english.into())
        .replacen("%i", &n.to_string(), 1)
}

/// The item card: name, count, value and weight, and by kind the armor rating,
/// damage, effects or description, with how much worn armor or the weapon
/// held would change.
fn item_card(engine: &Engine, row: &ListedItem, soul: Option<u8>, poisoned: bool) -> Value {
    let item = row.item;
    let mut f = common_card(row, poisoned);
    let worn = engine
        .inventories
        .get(&PLAYER_REF)
        .is_some_and(|i| i.equipped().contains(&item));
    let kind = match row.info.kind {
        ItemKind::Scroll => {
            let magic = engine.magic_item(item);
            let kind = scroll_fields(
                &mut f,
                magic.as_deref(),
                &engine.effects_text(item),
                &engine.spell_level_name(item),
            );
            // ICT_SPELL selects the school meter in the inventory's bottom
            // bar too. Clear it explicitly for a scroll without a school.
            let school = engine.spell_school(item);
            f.push((
                "magicSchoolName",
                text(&school.map_or_else(String::new, |s| engine.skill_name(s))),
            ));
            f.push((
                "magicSchoolLevel",
                num(school.map_or(0.0, |s| engine.av_base(PLAYER_REF, s).floor())),
            ));
            f.push((
                "magicSchoolPct",
                num(school.map_or(0.0, |s| engine.skill_progress(s) * 100.0)),
            ));
            kind
        }
        ItemKind::SoulGem => soul_fields(&mut f, soul, |name| engine.gmst_string(name)),
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
            // Effects the player hasn't learned show as unknown: the card
            // labels `itemEffect0..3` it has a name for, the rest of
            // `numItemEffects` "$UNKNOWN".
            let effects = engine
                .magic_item(item)
                .map(|m| m.effects.clone())
                .unwrap_or_default();
            let n = effects.len();
            let names: Vec<Option<String>> = effects
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    engine
                        .effect_learned(item, i)
                        .then(|| engine.magic_effect(e.effect).map(|m| m.name.clone()))
                        .flatten()
                })
                .collect();
            f.extend(ingredient_labels(&names));
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
        ItemKind::Light | ItemKind::Misc => card::MISC,
    };
    f.push(("type", Value::Number(kind)));
    object(f)
}

/// An ingredient card's `itemEffect0..3` for the effects the player has
/// learned (`None`: not yet); the card leaves the others "$UNKNOWN".
fn ingredient_labels(names: &[Option<String>]) -> Vec<(&'static str, Value)> {
    ["itemEffect0", "itemEffect1", "itemEffect2", "itemEffect3"]
        .into_iter()
        .zip(names)
        .filter_map(|(key, name)| name.as_ref().map(|n| (key, text(n))))
        .collect()
}

fn common_card(row: &ListedItem, poisoned: bool) -> Vec<(&'static str, Value)> {
    vec![
        ("name", text(&row.info.name)),
        ("count", num(row.count as f32)),
        ("value", num(row.info.value as f32)),
        ("weight", num(row.info.weight)),
        ("stolen", Value::Bool(row.owner.is_some())),
        // ItemCard.__set__itemInfo selects PoisonInstance's On / Off frame
        // from this boolean for weapon cards (all four item menus).
        ("poisoned", Value::Bool(poisoned)),
    ]
}

/// The class an item competes in for the best-in-class star: a weapon's
/// animation type (`WEAP` `DNAM`: 1 sword ... 7 bow, 9 crossbow; not fists or
/// staves), or an armor's slot. `None` for what is never marked.
fn weapon_class(anim: u8) -> Option<u32> {
    matches!(anim, 1..=7 | 9).then_some(0x100 + u32::from(anim))
}

/// An armor piece's class by its biped slots (`BOD2`): body, hands, feet,
/// shield, then head. Jewelry, cloaks and the rest have none.
fn armor_class(slots: u32) -> Option<u32> {
    [0x4, 0x8, 0x80, 0x200, 0x1]
        .into_iter()
        .find(|bit| slots & bit != 0)
}

/// Marks, of the player's own rows, those that are the best of their class
/// (see `known_ambiguities/inventory-cards.md`): the most armor rating for an
/// armor slot, the most base damage for a weapon type, ties all marked.
/// Enchantments, poisons and perks that don't change the card's number don't
/// count.
fn best_in_class(engine: &Engine, rows: &[Row]) -> Vec<bool> {
    let none = engine.protection_of(PLAYER_REF, &[]).rating;
    let scored: Vec<Option<(u32, f32)>> = rows
        .iter()
        .map(|r| {
            if r.theirs {
                return None;
            }
            let rec = engine.lo.get(r.item.item)?;
            match r.item.info.kind {
                ItemKind::Weapon => Some((
                    weapon_class(crate::world::inventory::weapon_anim_type(&rec))?,
                    crate::ai::combat::weapon_damage(&engine.lo, r.item.item),
                )),
                ItemKind::Armor => Some((
                    armor_class(crate::world::inventory::armor_slots(&rec))?,
                    engine.protection_of(PLAYER_REF, &[r.item.item]).rating - none,
                )),
                _ => None,
            }
        })
        .collect();
    marked_best(&scored)
}

/// Which (class, score) entries have their class's top score.
fn marked_best(scored: &[Option<(u32, f32)>]) -> Vec<bool> {
    let mut top: HashMap<u32, f32> = HashMap::new();
    for &(class, score) in scored.iter().flatten() {
        let t = top.entry(class).or_insert(score);
        *t = t.max(score);
    }
    scored
        .iter()
        .map(|s| s.is_some_and(|(class, score)| score >= top[&class]))
        .collect()
}

fn listed_state(
    rows: &[Row],
    inv: &Inventory,
    favorites: &[FormId],
    soul: impl Fn(&Row) -> Option<u8>,
    poisoned: impl Fn(&Row) -> bool,
    charge: impl Fn(&Row) -> Option<i32>,
    best: &[bool],
) -> Listed {
    (
        rows.iter()
            .map(|r| (r.theirs, r.item.item, r.item.owner, r.item.count))
            .collect(),
        inv.equipped().to_vec(),
        inv.held_all(),
        favorites.to_vec(),
        // Filling a gem can change only its soul metadata, leaving the
        // base form, count and ownership unchanged.
        rows.iter().map(soul).collect(),
        // Applying or exhausting a poison can leave the inventory's forms,
        // counts, ownership and equipped hands unchanged.
        rows.iter().map(poisoned).collect(),
        // Spending or restoring a weapon's charge changes only its card.
        rows.iter().map(charge).collect(),
        // A skill or a new item can change which one is best.
        best.to_vec(),
    )
}

/// The card represents the row's source inventory, including gifts received
/// from another actor. Poison metadata currently belongs to a weapon form,
/// not an individual instance or ownership stack; see poisons.md.
fn row_poisoned(kind: &Kind, row: &Row, inventories: &HashMap<FormId, Inventory>) -> bool {
    row.item.info.kind == ItemKind::Weapon
        && inventories
            .get(&kind.source(row.theirs))
            .is_some_and(|inv| {
                inv.count(row.item.item) > 0
                    && inv
                        .poison_on(row.item.item)
                        .is_some_and(|(_, doses)| doses > 0)
            })
}

fn row_soul_level(
    kind: &Kind,
    row: &Row,
    inventories: &HashMap<FormId, Inventory>,
    base: u8,
) -> Option<u8> {
    soul_level(
        inventories.get(&kind.source(row.theirs)),
        row.item.item,
        base,
    )
}

fn scroll_fields(
    fields: &mut Vec<(&'static str, Value)>,
    magic: Option<&crate::magic::MagicItem>,
    effects: &str,
    level: &str,
) -> f64 {
    fields.push(("effects", text(effects)));
    fields.push(("castLevel", text(level)));
    // The SWF uses zero for the per-second layout, any other value for the
    // normal layout. This is not the scroll's charge time in seconds.
    let concentration = magic.is_some_and(|m| m.cast_type == crate::spells::CONCENTRATION);
    fields.push(("castTime", num(if concentration { 0.0 } else { 1.0 })));
    // Scrolls consume the item, not the caster's magicka.
    fields.push(("spellCost", num(0.0)));
    card::SPELL
}

fn soul_fields(
    fields: &mut Vec<(&'static str, Value)>,
    level: Option<u8>,
    setting: impl FnOnce(&str) -> Option<String>,
) -> f64 {
    fields.push(("soulLVL", text(&soul_name(level, setting))));
    card::SOUL_GEMS
}

/// A homogeneous stack's actual soul, including per-item captures. Rows are
/// still grouped by form/owner, while soul metadata has no owner identity;
/// do not select an arbitrary soul when the form has several kinds of gem.
fn soul_level(inv: Option<&Inventory>, item: FormId, base: u8) -> Option<u8> {
    let Some(inv) = inv else {
        return (base <= 5).then_some(base);
    };
    let mut level = (inv.without_soul(item) > 0).then_some(base);
    for &(_, soul, _) in inv.souls.iter().filter(|&&(f, _, n)| f == item && n > 0) {
        if level.is_some_and(|x| x != soul) {
            return None;
        }
        level = Some(soul);
    }
    level.or(Some(base)).filter(|&x| x <= 5)
}

fn soul_name(level: Option<u8>, setting: impl FnOnce(&str) -> Option<String>) -> String {
    let (key, english) = match level {
        Some(0) => ("sEmpty", "Empty"),
        Some(1) => ("sSoulLevelNamePetty", "Petty"),
        Some(2) => ("sSoulLevelNameLesser", "Lesser"),
        Some(3) => ("sSoulLevelNameCommon", "Common"),
        Some(4) => ("sSoulLevelNameGreater", "Greater"),
        Some(5) => ("sSoulLevelNameGrand", "Grand"),
        // A neutral placeholder for unresolved mixed or invalid soul data.
        _ => return "—".into(),
    };
    setting(key).unwrap_or_else(|| english.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::magic::MagicItem;
    use crate::world::inventory::ItemInfo;

    const GEM: FormId = FormId(0x800);
    const ACTOR: FormId = FormId(0x900);
    const STOCK: FormId = FormId(0xA00);

    fn row(kind: ItemKind, theirs: bool) -> Row {
        Row {
            item: ListedItem {
                item: GEM,
                owner: Some(ACTOR),
                count: 2,
                info: ItemInfo {
                    name: "Synthetic item".into(),
                    kind,
                    value: 125,
                    weight: 0.5,
                },
            },
            theirs,
        }
    }

    fn gems(count: i32, soul: u8) -> Inventory {
        let mut inv = Inventory::default();
        inv.add(GEM, count);
        for _ in 0..count {
            inv.put_soul(GEM, soul);
        }
        inv
    }

    #[test]
    fn ingredient_card_labels_only_learned_effects() {
        let names = [
            Some("Restore Health".to_string()),
            None,
            Some("Frenzy".into()),
        ];
        let labels = ingredient_labels(&names);
        let keys: Vec<_> = labels.iter().map(|l| l.0).collect();
        assert_eq!(keys, ["itemEffect0", "itemEffect2"]);
    }

    #[test]
    fn best_in_class_marks_each_classs_top_score_with_ties() {
        let scored = [
            Some((weapon_class(1).unwrap(), 7.0)),
            Some((weapon_class(1).unwrap(), 9.0)),
            Some((weapon_class(1).unwrap(), 9.0)),
            Some((weapon_class(2).unwrap(), 4.0)),
            Some((armor_class(0x4).unwrap(), 20.0)),
            None,
        ];
        assert_eq!(marked_best(&scored), [false, true, true, true, true, false]);
    }

    #[test]
    fn classes_leave_out_staves_fists_and_jewelry() {
        assert_eq!(weapon_class(0), None);
        assert_eq!(weapon_class(8), None);
        assert_ne!(weapon_class(7), weapon_class(9));
        // A helmet covering head and hair, a body piece, jewelry.
        assert_eq!(armor_class(0x3), armor_class(0x1));
        assert_ne!(armor_class(0x4), armor_class(0x8));
        assert_eq!(armor_class(0x20), None);
        assert_eq!(armor_class(0x40), None);
    }

    #[test]
    fn scroll_payload_keeps_item_stats_and_has_no_magicka_cost() {
        let row = row(ItemKind::Scroll, false);
        for (cast_type, expected) in [(1, 1.0), (crate::spells::CONCENTRATION, 0.0)] {
            let magic = MagicItem {
                cast_type,
                cost: Some(500.0),
                charge_time: 3.5,
                ..Default::default()
            };
            let mut fields = common_card(&row.item, false);
            let kind = scroll_fields(&mut fields, Some(&magic), "Burns for 10 seconds.", "Adept");
            fields.push(("type", Value::Number(kind)));
            let Value::Object(fields) = object(fields) else {
                unreachable!()
            };
            assert_eq!(fields["name"], text("Synthetic item"));
            assert_eq!(fields["count"], num(2.0));
            assert_eq!(fields["value"], num(125.0));
            assert_eq!(fields["weight"], num(0.5));
            assert_eq!(fields["stolen"], Value::Bool(true));
            assert_eq!(fields["effects"], text("Burns for 10 seconds."));
            assert_eq!(fields["castLevel"], text("Adept"));
            assert_eq!(fields["castTime"], num(expected));
            assert_eq!(fields["spellCost"], num(0.0));
            assert_eq!(fields["type"], Value::Number(7.0));
        }
        let mut fields = Vec::new();
        scroll_fields(&mut fields, None, "", "Novice");
        let Value::Object(fields) = object(fields) else {
            unreachable!()
        };
        assert_eq!(fields["castTime"], num(1.0));
        assert_eq!(fields["effects"], text(""));
    }

    #[test]
    fn soul_labels_use_game_settings_and_cover_empty_through_grand() {
        for (level, key, fallback) in [
            (0, "sEmpty", "Empty"),
            (1, "sSoulLevelNamePetty", "Petty"),
            (2, "sSoulLevelNameLesser", "Lesser"),
            (3, "sSoulLevelNameCommon", "Common"),
            (4, "sSoulLevelNameGreater", "Greater"),
            (5, "sSoulLevelNameGrand", "Grand"),
        ] {
            assert_eq!(soul_name(Some(level), |_| None), fallback);
            assert_eq!(
                soul_name(Some(level), |setting| {
                    assert_eq!(setting, key);
                    Some("Localized soul".into())
                }),
                "Localized soul"
            );
            let row = row(ItemKind::SoulGem, false);
            let mut fields = common_card(&row.item, false);
            let kind = soul_fields(&mut fields, Some(level), |_| Some("Localized soul".into()));
            fields.push(("type", Value::Number(kind)));
            let Value::Object(fields) = object(fields) else {
                unreachable!()
            };
            assert_eq!(fields["type"], Value::Number(12.0));
            assert_eq!(fields["soulLVL"], text("Localized soul"));
            assert_eq!(fields["count"], num(2.0));
            assert_eq!(fields["value"], num(125.0));
            assert_eq!(fields["weight"], num(0.5));
        }
        assert_eq!(soul_name(None, |_| panic!("mixed is not empty")), "—");
        assert_eq!(soul_name(Some(6), |_| None), "—");
    }

    #[test]
    fn soul_state_uses_captured_souls_without_claiming_a_mixed_stack_is_empty() {
        let mut inv = Inventory::default();
        inv.add(GEM, 2);
        assert_eq!(soul_level(Some(&inv), GEM, 0), Some(0));
        assert_eq!(soul_level(Some(&inv), GEM, 5), Some(5));
        inv.put_soul(GEM, 1);
        assert_eq!(soul_level(Some(&inv), GEM, 0), None);
        inv.put_soul(GEM, 1);
        assert_eq!(soul_level(Some(&inv), GEM, 0), Some(1));
        inv.souls[0].2 = 1;
        inv.put_soul(GEM, 3);
        assert_eq!(soul_level(Some(&inv), GEM, 0), None);
        assert_eq!(soul_level(None, GEM, 5), Some(5));
        assert_eq!(soul_level(None, GEM, 6), None);
        assert_eq!(soul_level(Some(&gems(1, 6)), GEM, 0), None);
    }

    #[test]
    fn soul_cards_read_the_inventory_of_the_actual_giver_or_side() {
        let inventories = HashMap::from([
            (PLAYER_REF, gems(2, 1)),
            (ACTOR, gems(2, 3)),
            (STOCK, gems(2, 5)),
        ]);
        let mine = row(ItemKind::SoulGem, false);
        let theirs = row(ItemKind::SoulGem, true);
        for kind in [
            Kind::Inventory,
            Kind::Container {
                owner: ACTOR,
                npc: false,
            },
            Kind::Container {
                owner: ACTOR,
                npc: true,
            },
            Kind::Pickpocket { victim: ACTOR },
            Kind::Barter {
                vendor: ACTOR,
                stock: STOCK,
                info: None,
            },
            Kind::Gift {
                actor: ACTOR,
                giving: true,
            },
        ] {
            assert_eq!(row_soul_level(&kind, &mine, &inventories, 0), Some(1));
        }
        for kind in [
            Kind::Container {
                owner: ACTOR,
                npc: false,
            },
            Kind::Pickpocket { victim: ACTOR },
        ] {
            assert_eq!(row_soul_level(&kind, &theirs, &inventories, 0), Some(3));
        }
        assert_eq!(
            row_soul_level(
                &Kind::Barter {
                    vendor: ACTOR,
                    stock: STOCK,
                    info: None
                },
                &theirs,
                &inventories,
                0
            ),
            Some(5)
        );
        // Gifts received are on the menu's only (non-theirs) side.
        assert_eq!(
            row_soul_level(
                &Kind::Gift {
                    actor: ACTOR,
                    giving: false
                },
                &mine,
                &inventories,
                0
            ),
            Some(3)
        );
    }

    #[test]
    fn filling_in_place_changes_the_list_snapshot_and_refreshes_the_card() {
        let rows = [row(ItemKind::SoulGem, true)];
        let player = Inventory::default();
        let kind = Kind::Container {
            owner: ACTOR,
            npc: false,
        };
        let mut container = Inventory::default();
        container.add(GEM, 2);
        let mut inventories = HashMap::from([(ACTOR, container)]);
        let snapshot = |inventories: &HashMap<FormId, Inventory>| {
            listed_state(
                &rows,
                &player,
                &[],
                |r| row_soul_level(&kind, r, inventories, 0),
                |r| row_poisoned(&kind, r, inventories),
                |_| None,
                &[false],
            )
        };
        let empty = snapshot(&inventories);
        inventories.get_mut(&ACTOR).unwrap().put_soul(GEM, 1);
        let mixed = snapshot(&inventories);
        inventories.get_mut(&ACTOR).unwrap().put_soul(GEM, 1);
        let filled = snapshot(&inventories);
        assert_eq!(empty.0, filled.0, "same form, owner, count and side");
        assert_ne!(empty, mixed);
        assert_ne!(mixed, filled);
        assert_eq!(empty.4, vec![Some(0)]);
        assert_eq!(mixed.4, vec![None]);
        assert_eq!(filled.4, vec![Some(1)]);
    }

    #[test]
    fn poison_cards_use_the_rows_inventory_in_every_item_menu() {
        let poison = FormId(0xB00);
        for (kind, theirs, source) in [
            (Kind::Inventory, false, PLAYER_REF),
            (
                Kind::Container {
                    owner: ACTOR,
                    npc: false,
                },
                false,
                PLAYER_REF,
            ),
            (
                Kind::Container {
                    owner: ACTOR,
                    npc: false,
                },
                true,
                ACTOR,
            ),
            (
                Kind::Container {
                    owner: ACTOR,
                    npc: true,
                },
                true,
                ACTOR,
            ),
            (Kind::Pickpocket { victim: ACTOR }, false, PLAYER_REF),
            (Kind::Pickpocket { victim: ACTOR }, true, ACTOR),
            (
                Kind::Barter {
                    vendor: ACTOR,
                    stock: STOCK,
                    info: None,
                },
                false,
                PLAYER_REF,
            ),
            (
                Kind::Barter {
                    vendor: ACTOR,
                    stock: STOCK,
                    info: None,
                },
                true,
                STOCK,
            ),
            (
                Kind::Gift {
                    actor: ACTOR,
                    giving: true,
                },
                false,
                PLAYER_REF,
            ),
            (
                Kind::Gift {
                    actor: ACTOR,
                    giving: false,
                },
                false,
                ACTOR,
            ),
        ] {
            let row = row(ItemKind::Weapon, theirs);
            let mut inventories: HashMap<_, _> = [PLAYER_REF, ACTOR, STOCK]
                .into_iter()
                .map(|actor| {
                    let mut inv = Inventory::default();
                    inv.add(GEM, 2);
                    (actor, inv)
                })
                .collect();
            inventories.get_mut(&source).unwrap().poison(GEM, poison, 1);
            assert!(row_poisoned(&kind, &row, &inventories), "{kind:?}");
            let Value::Object(fields) = object(common_card(
                &row.item,
                row_poisoned(&kind, &row, &inventories),
            )) else {
                unreachable!()
            };
            assert_eq!(fields["poisoned"], Value::Bool(true));
            // The same base weapon in every other inventory must not
            // inherit the source's poison status, or the reverse.
            for (&actor, inv) in &mut inventories {
                inv.poison(GEM, poison, if actor == source { 0 } else { 1 });
            }
            assert!(!row_poisoned(&kind, &row, &inventories), "{kind:?}");
        }
    }

    #[test]
    fn applying_and_exhausting_poison_refreshes_the_card_without_inventory_changes() {
        let rows = [row(ItemKind::Weapon, false)];
        let mut inv = Inventory::default();
        inv.add(GEM, 2);
        let poison = FormId(0xB00);
        let mut inventories = HashMap::from([(PLAYER_REF, inv)]);
        let snapshot = |inventories: &HashMap<FormId, Inventory>| {
            listed_state(
                &rows,
                &inventories[&PLAYER_REF],
                &[],
                |_| None,
                |r| row_poisoned(&Kind::Inventory, r, inventories),
                |_| None,
                &[false],
            )
        };
        let clean = snapshot(&inventories);
        inventories
            .get_mut(&PLAYER_REF)
            .unwrap()
            .poison(GEM, poison, 2);
        let poisoned = snapshot(&inventories);
        assert_eq!(clean.0, poisoned.0, "same form, owner, count and side");
        assert_ne!(clean, poisoned);
        assert_eq!(poisoned.5, vec![true]);
        inventories
            .get_mut(&PLAYER_REF)
            .unwrap()
            .take_poison_dose(GEM);
        assert_eq!(snapshot(&inventories), poisoned, "one dose remains");
        inventories
            .get_mut(&PLAYER_REF)
            .unwrap()
            .take_poison_dose(GEM);
        assert_eq!(snapshot(&inventories), clean);
        let Value::Object(fields) = object(common_card(&rows[0].item, false)) else {
            unreachable!()
        };
        assert_eq!(fields["poisoned"], Value::Bool(false));
        assert!(!row_poisoned(&Kind::Inventory, &rows[0], &HashMap::new()));
        let inv = inventories.get_mut(&PLAYER_REF).unwrap();
        // Invalid/stale metadata must not put the icon on unrelated item
        // kinds or weapons no longer in the inventory.
        inv.poisoned.push((GEM, poison, 0));
        assert!(!row_poisoned(&Kind::Inventory, &rows[0], &inventories));
        inventories.get_mut(&PLAYER_REF).unwrap().poisoned[0].2 = 1;
        assert!(!row_poisoned(
            &Kind::Inventory,
            &row(ItemKind::Ammo, false),
            &inventories
        ));
        inventories
            .get_mut(&PLAYER_REF)
            .unwrap()
            .remove_split(GEM, 2, None);
        inventories
            .get_mut(&PLAYER_REF)
            .unwrap()
            .poisoned
            .push((GEM, poison, 1));
        assert!(!row_poisoned(&Kind::Inventory, &rows[0], &inventories));
    }
}
