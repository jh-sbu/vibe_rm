//! The player's items: taking things from the world and from containers, and the
//! menus that list them.

use esp::FormId;

use crate::engine::{Engine, PLAYER_REF};
use crate::world::inventory::{ItemInfo, armor_slots, item_info};

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

    /// Equip or take off an item the actor carries: armor (see `equip_armor`), a
    /// weapon (in place of the one held; a bow takes both hands, so the shield
    /// comes off) or ammunition (in place of the other).
    pub fn equip_item(&mut self, r: FormId, item: FormId, on: bool) -> Result<(), String> {
        let tag = self.lo.tag_of(item).map(|t| t.0);
        if !matches!(tag, Some(t) if t == *b"WEAP" || t == *b"AMMO") {
            return self.equip_armor(r, item, on);
        }
        let tag = tag.unwrap();
        let bow = crate::ai::archery::is_bow(&self.lo, item);
        self.inventory_mut(r);
        let lo = &self.lo;
        let Some(inv) = self.inventories.get_mut(&r) else {
            return Err(format!("{r} has no inventory"));
        };
        if !on {
            inv.equipped.retain(|&f| f != item);
            return Ok(());
        }
        if inv.count(item) <= 0 {
            return Err(format!("{r} doesn't carry {item}"));
        }
        inv.equipped.retain(|&f| {
            let rec = lo.get(f);
            let same_kind = rec.as_ref().is_some_and(|x| x.tag().0 == tag);
            let shield = bow
                && rec
                    .as_ref()
                    .is_some_and(|x| x.tag().0 == *b"ARMO" && armor_slots(x) & (1 << 9) != 0);
            !same_kind && !shield
        });
        inv.equipped.push(item);
        Ok(())
    }

    /// Wear (taking off whatever covers the same body slots) or take off a piece of
    /// armor the actor carries.
    pub fn equip_armor(&mut self, r: FormId, armor: FormId, on: bool) -> Result<(), String> {
        let slots = match self.lo.get(armor).filter(|rec| rec.tag().0 == *b"ARMO") {
            Some(rec) => armor_slots(&rec),
            None => return Err(format!("{armor} isn't armor")),
        };
        self.inventory_mut(r);
        let lo = &self.lo;
        let Some(inv) = self.inventories.get_mut(&r) else {
            return Err(format!("{r} has no inventory"));
        };
        if on {
            if inv.count(armor) <= 0 {
                return Err(format!("{r} doesn't carry {armor}"));
            }
            inv.equipped.retain(|&f| {
                f == armor
                    || !lo.get(f).is_some_and(|rec| {
                        rec.tag().0 == *b"ARMO" && armor_slots(&rec) & slots != 0
                    })
            });
            if !inv.equipped.contains(&armor) {
                inv.equipped.push(armor);
            }
        } else {
            inv.equipped.retain(|&f| f != armor);
        }
        Ok(())
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
