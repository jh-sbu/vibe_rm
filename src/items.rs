//! The player's items: taking things from the world and from containers, and the
//! menus that list them.

use esp::FormId;

use crate::engine::{Engine, PLAYER_REF};
use crate::world::inventory::{ItemInfo, ItemKind, armor_slots, equip_slot, item_info};

/// A menu taking the mouse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    /// The player's inventory.
    Inventory,
    /// A container (or another inventory) being searched.
    Container(FormId),
    /// A book being read: the book, and the reference it lies as in the world.
    Book {
        book: FormId,
        reference: Option<FormId>,
    },
    /// Picking a lock (`Engine::lockpick` holds the state).
    Lockpick,
    /// Picking someone's pocket.
    Pickpocket(FormId),
    /// Asking whether to serve the jail sentence (`sServeSentenceQuestion`).
    ServeSentence,
    /// The quest journal.
    Journal,
    /// The player's skills and level, where level ups are taken.
    Skills,
    /// Bartering with a merchant.
    Barter(FormId),
    /// Giving an actor gifts, or taking theirs (`Engine::gift` holds the
    /// state).
    Gift(FormId),
    /// The player's spells, powers and active effects.
    Magic,
    /// The player's favorite items, spells and powers.
    Favorites,
    /// Tab's menu of menus (Skills, Magic, Items, Map).
    Tween,
    /// How long to sleep (in a bed) or wait (`Engine::waiting` while the
    /// hours pass).
    SleepWait { sleeping: bool },
    /// A trainer's training menu.
    Training(FormId),
}

/// A book's text as pages of plain text: Skyrim's HTML-like markup (`<p>`, `<br>`,
/// `<font>`, `<img>`...) reduced to paragraphs, `[pagebreak]` starting a new page.
pub fn book_pages(text: &str) -> Vec<String> {
    let mut pages = Vec::new();
    for raw in text.split("[pagebreak]") {
        let mut out = String::new();
        let mut rest = raw;
        while let Some(i) = rest.find('<') {
            out.push_str(&rest[..i]);
            let Some(j) = rest[i..].find('>') else {
                rest = &rest[i + 1..];
                continue;
            };
            let tag = rest[i + 1..i + j].trim().to_ascii_lowercase();
            if tag.starts_with("br") || tag.starts_with("/p") || tag.starts_with("p ") || tag == "p"
            {
                out.push('\n');
            }
            // Illuminated first letters are pictures of the letter (`.../T_kells.png`).
            if let Some(k) = tag.find("illuminated_letters/")
                && let Some(c) = tag[k + "illuminated_letters/".len()..].chars().next()
            {
                out.push(c.to_ascii_uppercase());
            }
            rest = &rest[i + j + 1..];
        }
        out.push_str(rest);
        let text = out
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&")
            .replace("&quot;", "\"")
            .replace("\r", "");
        // Collapse runs of blank lines.
        let mut page = String::new();
        let mut blank = 0;
        for line in text.lines().map(str::trim) {
            if line.is_empty() {
                blank += 1;
                if blank > 1 || page.is_empty() {
                    continue;
                }
            } else {
                blank = 0;
            }
            page.push_str(line);
            page.push('\n');
        }
        let page = page.trim_end().to_owned();
        if !page.is_empty() {
            pages.push(page);
        }
    }
    pages
}

/// A row of an inventory: so many of an item, belonging to `owner` if it is
/// someone else's (stolen).
#[derive(Debug, Clone)]
pub struct ListedItem {
    pub item: FormId,
    pub owner: Option<FormId>,
    pub count: i32,
    pub info: ItemInfo,
}

impl Engine {
    /// Whether a reference is an item lying about that can be picked up.
    pub fn is_item_ref(&self, r: FormId) -> bool {
        self.base_of(r)
            .is_some_and(|b| item_info(&self.lo, b).is_some())
            && match self.created(r) {
                Some(c) => !c.actor && c.container.is_none(),
                None => self.lo.tag_of(r).map(|t| t.0) == Some(*b"REFR"),
            }
    }

    /// Pick up an item reference: into the player's inventory (as many as the
    /// reference stands for, `XCNT`) and gone from the world.
    pub fn take_item(&mut self, r: FormId) -> bool {
        if self.is_disabled(r) {
            return false;
        }
        let Some(base) = self.base_of(r) else {
            return false;
        };
        let Some(info) = item_info(&self.lo, base) else {
            return false;
        };
        let count = self
            .lo
            .get(r)
            .and_then(|rec| {
                rec.get(b"XCNT")
                    .filter(|d| d.len() >= 4)
                    .map(|d| i32::from_le_bytes(d[0..4].try_into().unwrap()))
            })
            .or_else(|| self.created(r).map(|c| c.count))
            .unwrap_or(1)
            .max(1);
        let stolen = self.stolen_from(base, r, false);
        self.inventory_mut(PLAYER_REF)
            .add_owned(base, stolen, count);
        if stolen.is_some() {
            self.increment_stat("Items Stolen", count);
        }
        if base == crate::crime::GOLD {
            self.increment_stat("Gold Found", count);
        }
        self.inventory_event(PLAYER_REF, true, base, count, None);
        self.set_disabled(r, true);
        self.send_player_add_item(base, count, r, false);
        self.scripts.notify(if count > 1 {
            format!("{} ({count}) added", info.name)
        } else {
            format!("{} added", info.name)
        });
        log::info!("took {r} ({base} x{count})");
        let player = self.object_value(PLAYER_REF);
        self.send_script_event(r, "OnContainerChanged", vec![player, papyrus::Value::None]);
        true
    }

    /// Whether taking `item` from something `owner` owns is stealing. What
    /// another person owns always is. What a faction owns is when the faction
    /// can own things (`DATA` flag 0x8000, "can be owner") and the item's base
    /// value is over the favor cap of its living members the player is on good
    /// terms with: the lowest of their ranks' caps (`iFavorFriendValue` 25,
    /// `iFavorConfidantValue` 50, `iFavorAllyValue` 100, `iFavorLoverValue`
    /// 500); no such member, no allowance. What the player or one of their
    /// factions owns never is.
    pub(crate) fn is_stealing(&self, item: FormId, owner: FormId) -> bool {
        if owner == FormId(0x7) || self.player_factions().contains(&owner) {
            return false;
        }
        let Some(rec) = self.lo.get(owner) else {
            return false;
        };
        if rec.tag().0 != *b"FACT" {
            return true;
        }
        let flags = rec
            .get(b"DATA")
            .filter(|d| d.len() >= 4)
            .map_or(0, |d| u32::from_le_bytes(d[0..4].try_into().unwrap()));
        if flags & 0x8000 == 0 {
            return false;
        }
        let value = item_info(&self.lo, item).map_or(0, |i| i.value);
        let cap = self.faction_favor_cap(owner);
        log::debug!("{item} (worth {value}) owned by faction {owner}: favor cap {cap:?}");
        cap.is_none_or(|cap| value > cap)
    }

    /// Who owns a reference (or cell): as a script set it
    /// (`SetActorOwner` / `SetFactionOwner`), else its `XOWN`, else its
    /// cell's.
    pub(crate) fn ref_owner(&self, r: FormId) -> Option<FormId> {
        match self.scripts.owners.get(&r) {
            Some(owner) => *owner,
            None => crate::ai::furniture::owner_of(&self.lo, r),
        }
    }

    /// Whether what `owner` owns isn't the player's to take freely: another
    /// person's, or a faction's that can own things and the player isn't in
    /// (whatever its members' favor allows; see `is_stealing`).
    pub(crate) fn owned_by_other(&self, owner: FormId) -> bool {
        if owner == FormId(0x7) || self.player_factions().contains(&owner) {
            return false;
        }
        match self.lo.get(owner) {
            Some(rec) if rec.tag().0 == *b"FACT" => rec
                .get(b"DATA")
                .filter(|d| d.len() >= 4)
                .is_some_and(|d| u32::from_le_bytes(d[0..4].try_into().unwrap()) & 0x8000 != 0),
            Some(_) => true,
            None => false,
        }
    }

    /// The gold value up to which a faction's members let the player take its
    /// things: the lowest cap among living members ranked friend or better
    /// with the player (those at acquaintance or below don't count).
    fn faction_favor_cap(&self, faction: FormId) -> Option<i32> {
        let gmst = |name: &str, default: i32| crate::ai::combat::gmst_i32(&self.lo, name, default);
        let mut cap: Option<i32> = None;
        for &npc in self.faction_members(faction) {
            let alive = self
                .npc_refs_index()
                .get(&npc)
                .is_none_or(|&r| !self.is_dead(r));
            let rank = self.relationship_rank(npc, PLAYER_REF);
            if !alive || rank <= 0 {
                continue;
            }
            let c = match rank {
                1 => gmst("iFavorFriendValue", 25),
                2 => gmst("iFavorConfidantValue", 50),
                3 => gmst("iFavorAllyValue", 100),
                _ => gmst("iFavorLoverValue", 500),
            };
            cap = Some(cap.map_or(c, |x| x.min(c)));
        }
        cap
    }

    /// A book's title and text.
    pub fn book_text(&self, book: FormId) -> (String, String) {
        let Some(rec) = self.lo.get(book) else {
            return Default::default();
        };
        let name = rec
            .get(b"FULL")
            .map(|d| self.lo.lstring(&rec, d))
            .unwrap_or_default();
        let text = rec
            .get(b"DESC")
            .map(|d| self.lo.lstring(&rec, d))
            .unwrap_or_default();
        (name, text)
    }

    /// Whether a book may be taken (`DATA` flag 0x02: can't be taken).
    pub fn book_takeable(&self, book: FormId) -> bool {
        !self
            .lo
            .get(book)
            .and_then(|r| r.get(b"DATA").and_then(|d| d.first().copied()))
            .is_some_and(|f| f & 0x2 != 0)
    }

    /// Move up to `n` of `item` from one inventory to another (of those
    /// belonging to `stack`'s owner, if given: a row of the item menu);
    /// returns how many moved. What the player steals is marked as the
    /// owner's.
    pub fn transfer_item(
        &mut self,
        from: FormId,
        to: FormId,
        item: FormId,
        n: i32,
        stack: Option<Option<FormId>>,
    ) -> i32 {
        let mark = if to == PLAYER_REF {
            self.stolen_from(item, from, true)
        } else {
            None
        };
        let moved: i32 = self
            .remove_stack(from, item, n, stack, Some(to), mark)
            .iter()
            .map(|(_, n)| n)
            .sum();
        if moved > 0 && to == PLAYER_REF {
            self.count_taken(from, item, moved, mark.is_some());
            self.send_player_add_item(item, moved, from, true);
            let name = item_info(&self.lo, item)
                .map(|i| i.name)
                .unwrap_or_default();
            self.scripts.notify(if moved > 1 {
                format!("{name} ({moved}) added")
            } else {
                format!("{name} added")
            });
        }
        moved
    }

    /// The statistics for what the player takes from a container or someone
    /// else's inventory: Items Stolen; from containers and bodies (not the
    /// living), Gold Found, and Chests Looted the first time anything is
    /// taken from a container.
    fn count_taken(&mut self, from: FormId, item: FormId, n: i32, stolen: bool) {
        if stolen {
            self.increment_stat("Items Stolen", n);
        }
        let living = self.actor_ref(from).is_some_and(|a| !a.dead);
        if living {
            return;
        }
        if item == crate::crime::GOLD {
            self.increment_stat("Gold Found", n);
        }
        let chest = self
            .base_of(from)
            .and_then(|b| self.lo.tag_of(b))
            .is_some_and(|t| t.0 == *b"CONT");
        if chest && self.scripts.looted.insert(from) {
            self.increment_stat("Chests Looted", 1);
        }
    }

    /// Marks an item, spell or power one of the player's favorites, or not
    /// (losing its hotkey).
    pub fn toggle_favorite(&mut self, form: FormId) {
        if let Some(i) = self.favorites.iter().position(|&f| f == form) {
            self.favorites.remove(i);
            for key in self.hotkeys.iter_mut().filter(|k| **k == Some(form)) {
                *key = None;
            }
        } else {
            self.favorites.push(form);
        }
    }

    /// Accept on one of the player's items (in the inventory, the favorites
    /// menu, on a hotkey), or an attack button's equip in its hand (`hand`):
    /// weapons and apparel are equipped or taken off (a weapon in that hand,
    /// or out of it when it is there already), potions, food and ingredients
    /// used, books read.
    pub fn use_listed_item(&mut self, row: &ListedItem, hand: Option<usize>) {
        // A weapon by the hand asked for, the rest wherever they are.
        let hand_of_weapon = hand.filter(|_| row.info.kind == ItemKind::Weapon);
        let worn = self
            .inventories
            .get(&PLAYER_REF)
            .is_some_and(|i| match hand_of_weapon {
                Some(h) => i.held(h) == Some(row.item),
                None => i.is_equipped(row.item),
            });
        let result = match row.info.kind {
            ItemKind::Weapon | ItemKind::Ammo | ItemKind::Armor if worn => match hand_of_weapon {
                Some(h) => {
                    let lo = &self.lo;
                    if let Some(inv) = self.inventories.get_mut(&PLAYER_REF) {
                        inv.release(lo, h);
                    }
                    Ok(())
                }
                None => self.equip_item(PLAYER_REF, row.item, false),
            },
            ItemKind::Weapon | ItemKind::Ammo | ItemKind::Armor => {
                self.equip_in(PLAYER_REF, row.item, hand)
            }
            ItemKind::Potion | ItemKind::Ingredient => self.consume(PLAYER_REF, row.item),
            ItemKind::Book if let Some(spell) = self.tome_spell(row.item) => {
                self.read_spell_tome(row.item, spell);
                Ok(())
            }
            ItemKind::Book => {
                self.menu = Some(Menu::Book {
                    book: row.item,
                    reference: None,
                });
                self.read_book(row.item);
                Ok(())
            }
            _ => Ok(()),
        };
        if let Err(e) = result {
            self.scripts.notify(e);
        }
        log::debug!(
            "player uses {} ({hand:?}): holds {:?}",
            row.item,
            self.inventories.get(&PLAYER_REF).map(|i| i.held_all())
        );
    }

    /// Hotkey `key` (0 to 7) in the world: its favorite used as Accept in the
    /// favorites menu would (a spell readied in the right hand).
    pub fn use_hotkey(&mut self, key: usize) {
        let Some(form) = self.hotkeys.get(key).copied().flatten() else {
            return;
        };
        if self.actor_spells(PLAYER_REF).contains(&form)
            || self.actor_shouts(PLAYER_REF).contains(&form)
        {
            self.toggle_spell(PLAYER_REF, form, crate::spells::RIGHT);
        } else if let Some(row) = self
            .listed_inventory(PLAYER_REF)
            .into_iter()
            .find(|i| i.item == form)
        {
            self.use_listed_item(&row, None);
        }
    }

    /// Equip or take off an item the actor carries: armor (see `equip_armor`),
    /// a weapon, shield or torch where it goes first (see `equip_in`), or
    /// ammunition (in place of the other).
    pub fn equip_item(&mut self, r: FormId, item: FormId, on: bool) -> Result<(), String> {
        if on {
            return self.equip_in(r, item, None);
        }
        let tag = self.lo.tag_of(item).map(|t| t.0);
        if !matches!(tag, Some(t) if t == *b"ARMO" || t == *b"AMMO")
            && equip_slot(&self.lo, item).is_none_or(|s| s.voice)
        {
            return Err(format!("{item} isn't worn or held"));
        }
        self.inventory_mut(r).take_off(item);
        Ok(())
    }

    /// Equip an item the actor carries: armor (see `equip_armor`), ammunition
    /// in place of the other, or a weapon, shield or torch in the hands its
    /// equip slot (`ETYP`) gives it, in place of what they hold: `hand`
    /// (`RIGHT` / `LEFT`) for a one-handed weapon (any of them, though most
    /// name only the right hand), else the right hand if it may go there;
    /// all its hands for a two-handed one (a bow puts the shield away).
    pub fn equip_in(&mut self, r: FormId, item: FormId, hand: Option<usize>) -> Result<(), String> {
        let tag = self.lo.tag_of(item).map(|t| t.0);
        let slot = equip_slot(&self.lo, item);
        let weapon = tag == Some(*b"WEAP");
        match tag {
            Some(t) if t == *b"ARMO" && slot.is_none() => return self.equip_armor(r, item, true),
            Some(t) if t == *b"AMMO" => {}
            Some(t) if t != *b"SPEL" && t != *b"SHOU" && slot.is_some_and(|s| !s.voice) => {}
            _ => return Err(format!("{item} isn't worn or held")),
        }
        self.inventory_mut(r);
        let lo = &self.lo;
        let Some(inv) = self.inventories.get_mut(&r) else {
            return Err(format!("{r} has no inventory"));
        };
        if inv.count(item) <= 0 {
            return Err(format!("{r} doesn't carry {item}"));
        }
        match slot {
            Some(s) => {
                inv.hold(lo, item, &s.hands(hand, weapon));
            }
            None => {
                inv.take_off_where(|f| f != item && lo.tag_of(f).map(|t| t.0) == Some(*b"AMMO"));
                inv.wear(item);
            }
        }
        Ok(())
    }

    /// Wear (taking off whatever covers the same body slots) or take off a piece of
    /// armor the actor carries; a shield is held (see `equip_in`).
    pub fn equip_armor(&mut self, r: FormId, armor: FormId, on: bool) -> Result<(), String> {
        let slots = match self.lo.get(armor).filter(|rec| rec.tag().0 == *b"ARMO") {
            Some(rec) => armor_slots(&rec),
            None => return Err(format!("{armor} isn't armor")),
        };
        if on && equip_slot(&self.lo, armor).is_some() {
            return self.equip_in(r, armor, None);
        }
        self.inventory_mut(r);
        let lo = &self.lo;
        let Some(inv) = self.inventories.get_mut(&r) else {
            return Err(format!("{r} has no inventory"));
        };
        if on {
            if inv.count(armor) <= 0 {
                return Err(format!("{r} doesn't carry {armor}"));
            }
            inv.take_off_where(|f| {
                f != armor
                    && lo.get(f).is_some_and(|rec| {
                        rec.tag().0 == *b"ARMO" && armor_slots(&rec) & slots != 0
                    })
            });
            inv.wear(armor);
        } else {
            inv.take_off(armor);
        }
        Ok(())
    }

    /// What an item's magic does, as the item card says it: each effect's
    /// description (`MGEF` `DNAM`) with the magnitude and duration in place of
    /// `<mag>` and `<dur>`; a weapon or armor's enchantment's (`EITM`). Effects
    /// hidden from the UI (flag 0x8000) are left out, and what only Survival
    /// Mode says (`[SURV=...]`).
    pub fn effects_text(&self, item: FormId) -> String {
        let magic = self.item_enchantment(item).unwrap_or(item);
        let Some(m) = self.magic_item(magic) else {
            return String::new();
        };
        let mut out: Vec<String> = Vec::new();
        for e in &m.effects {
            let Some(rec) = self.lo.get(e.effect) else {
                continue;
            };
            let hidden = rec
                .get(b"DATA")
                .filter(|d| d.len() >= 4)
                .is_some_and(|d| u32::from_le_bytes(d[0..4].try_into().unwrap()) & 0x8000 != 0);
            let text = rec
                .get(b"DNAM")
                .map(|d| self.lo.lstring(&rec, d))
                .unwrap_or_default();
            if hidden || text.trim().is_empty() {
                continue;
            }
            out.push(
                without_survival(&text)
                    .replace("<mag>", &format!("{:.0}", e.magnitude))
                    .replace("<dur>", &e.duration.to_string())
                    .trim()
                    .to_owned(),
            );
        }
        out.join(" ")
    }

    /// Whether a potion (`ALCH`) is food (`ENIT` flag 0x2).
    pub fn is_food(&self, item: FormId) -> bool {
        self.lo
            .get(item)
            .filter(|r| r.tag().0 == *b"ALCH")
            .and_then(|r| {
                r.get(b"ENIT")
                    .filter(|d| d.len() >= 8)
                    .map(|d| u32::from_le_bytes(d[4..8].try_into().unwrap()) & 0x2 != 0)
            })
            .unwrap_or(false)
    }

    /// Takes an item off once the actor has none of it left (out of the
    /// left hand when one of a pair is left).
    pub fn unequip_if_gone(&mut self, actor: FormId, item: FormId) {
        if let Some(inv) = self.inventories.get_mut(&actor) {
            inv.unequip_if_short(item);
        }
    }

    /// What the actor would wear with a piece of armor put on: it in place of
    /// whatever covers the same body slots (as `equip_armor`).
    pub fn worn_with(&self, actor: FormId, armor: FormId) -> Vec<FormId> {
        let slots = self.lo.get(armor).map_or(0, |r| armor_slots(&r));
        let mut worn: Vec<FormId> = self
            .inventories
            .get(&actor)
            .map(|i| i.equipped().to_vec())
            .unwrap_or_default();
        worn.retain(|&f| {
            f != armor
                && !self
                    .lo
                    .get(f)
                    .is_some_and(|r| r.tag().0 == *b"ARMO" && armor_slots(&r) & slots != 0)
        });
        worn.push(armor);
        worn
    }

    /// An inventory's items with what they are, by kind then name; those
    /// belonging to someone (stolen) in rows of their own after the rest.
    pub fn listed_inventory(&mut self, r: FormId) -> Vec<ListedItem> {
        let inv = self.inventory_mut(r).clone();
        let mut out: Vec<ListedItem> = Vec::new();
        for &(item, _) in &inv.items {
            let Some(info) = item_info(&self.lo, item) else {
                continue;
            };
            let free = inv.count_owned(item, None);
            if free > 0 {
                out.push(ListedItem {
                    item,
                    owner: None,
                    count: free,
                    info: info.clone(),
                });
            }
            for &(_, owner, count) in inv.owned.iter().filter(|(f, _, n)| *f == item && *n > 0) {
                out.push(ListedItem {
                    item,
                    owner: Some(owner),
                    count,
                    info: info.clone(),
                });
            }
        }
        out.sort_by(|a, b| {
            (a.info.kind, &a.info.name, a.owner.is_some()).cmp(&(
                b.info.kind,
                &b.info.name,
                b.owner.is_some(),
            ))
        });
        out
    }

    /// Send a Papyrus event to the scripts on a reference and on the quest aliases
    /// it fills (delivered with the next script update).
    pub fn send_script_event(&mut self, r: FormId, event: &str, args: Vec<papyrus::Value>) {
        for obj in self.objects_of_ref(r) {
            self.scripts
                .pending_events
                .push((obj, event.to_owned(), args.clone()));
        }
    }

    /// A reference's script objects: itself and the running quests' aliases it fills.
    pub(crate) fn objects_of_ref(&self, r: FormId) -> Vec<papyrus::ObjectId> {
        let mut out = vec![papyrus::ObjectId::Form(r.0)];
        // Magic effects on it hear its events (`OnHit`...).
        out.extend(self.effect_objects(r));
        for (q, st) in &self.scripts.quests {
            if !st.running {
                continue;
            }
            for (&alias, &filled) in &st.aliases {
                if filled == r {
                    out.push(papyrus::ObjectId::Alias { quest: q.0, alias });
                }
            }
        }
        out
    }

    /// `OnItemAdded` / `OnItemRemoved` on a container (or actor) for `count` of `item`,
    /// from or to `other`, to the objects whose inventory filters let it through.
    pub fn inventory_event(
        &mut self,
        owner: FormId,
        added: bool,
        item: FormId,
        count: i32,
        other: Option<FormId>,
    ) {
        if count <= 0 {
            return;
        }
        if owner == PLAYER_REF && added && item == crate::crime::GOLD {
            let gold = self.item_count(PLAYER_REF, item);
            self.raise_stat_to("Most Gold Carried", gold);
        }
        let event = if added {
            "OnItemAdded"
        } else {
            "OnItemRemoved"
        };
        let args = vec![
            self.object_value(item),
            papyrus::Value::Int(count),
            papyrus::Value::None,
            other.map_or(papyrus::Value::None, |o| self.object_value(o)),
        ];
        for obj in self.objects_of_ref(owner) {
            let heard = match self.scripts.inventory_filters.get(&obj) {
                Some(f) if !f.is_empty() => {
                    f.contains(&item) || f.iter().any(|&l| self.formlist(l).contains(&item))
                }
                _ => true,
            };
            if heard {
                self.scripts
                    .pending_events
                    .push((obj, event.to_owned(), args.clone()));
            }
        }
    }
}

/// A description without its Survival Mode parts (`[SURV=...]`), as the game
/// shows it with Survival Mode off.
fn without_survival(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find("[SURV=") {
        out.push_str(&rest[..i]);
        rest = rest[i..].find(']').map_or("", |j| &rest[i + j + 1..]);
    }
    out.push_str(rest);
    out
}
