//! Inventories: what actors and containers carry (`CNTO`, leveled items, outfits) and
//! what actors have equipped.

use esp::{FormId, LoadOrder, LoadedRecord};

use super::actor::{Globals, resolve_items};

/// What an item is, for listing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemKind {
    Weapon,
    Armor,
    Ammo,
    Potion,
    Ingredient,
    Book,
    Scroll,
    Key,
    SoulGem,
    Light,
    Misc,
}

#[derive(Debug, Clone)]
pub struct ItemInfo {
    pub name: String,
    pub kind: ItemKind,
    pub value: i32,
    pub weight: f32,
}

/// Weapon restrictions authored in `WEAP`: `DNAM` flags at byte 12
/// (xEdit's TES5 record definitions; CommonLibSSE's `TESObjectWEAP::Data::Flag`).
/// Keep these separate from `item_info`: hidden weapons still equip and
/// contribute to combat and inventory counts.
fn weapon_restrictions(lo: &LoadOrder, form: FormId) -> (bool, bool) {
    let Some(rec) = lo.get(form).filter(|r| r.tag().0 == *b"WEAP") else {
        return (false, false);
    };
    let flags = rec
        .get(b"DNAM")
        .and_then(|d| d.get(12))
        .copied()
        .unwrap_or(0);
    (
        flags & 0x08 != 0,
        flags & 0x80 != 0 || rec.flags() & 0x04 != 0,
    )
}

/// Whether an item can be shown and selected in inventory menus. Bound
/// weapons and NPC-only weapons are non-playable, not absent from inventory.
pub fn item_playable(lo: &LoadOrder, form: FormId) -> bool {
    !weapon_restrictions(lo, form).1
}

/// Whether a weapon's Can't Drop flag allows it to be dropped.
pub fn item_droppable(lo: &LoadOrder, form: FormId) -> bool {
    !weapon_restrictions(lo, form).0
}

/// Whether a weapon is a bound weapon (`DNAM` flags2 bit 13, "Bound Weapon" in
/// xEdit's TES5 definitions): the Disarm shout leaves these in the hand.
pub fn weapon_bound(lo: &LoadOrder, form: FormId) -> bool {
    lo.get(form)
        .filter(|r| r.tag().0 == *b"WEAP")
        .and_then(|r| {
            r.get(b"DNAM")
                .and_then(|d| d.get(40..44))
                .map(|d| d.to_vec())
        })
        .is_some_and(|d| u32::from_le_bytes(d[..4].try_into().unwrap()) & 0x2000 != 0)
}

/// Name, kind, value and weight of an inventory item; `None` for forms that aren't
/// items (and lights that can't be carried).
pub fn item_info(lo: &LoadOrder, form: FormId) -> Option<ItemInfo> {
    let rec = lo.get(form)?;
    let tag = rec.tag().0;
    let data = rec.get(b"DATA").unwrap_or(&[]);
    let f32_at = |d: &[u8], o: usize| {
        d.get(o..o + 4)
            .map_or(0.0, |b| f32::from_le_bytes(b.try_into().unwrap()))
    };
    let i32_at = |d: &[u8], o: usize| {
        d.get(o..o + 4)
            .map_or(0, |b| i32::from_le_bytes(b.try_into().unwrap()))
    };
    let (kind, value, weight) = match &tag {
        b"WEAP" => (ItemKind::Weapon, i32_at(data, 0), f32_at(data, 4)),
        b"ARMO" => (ItemKind::Armor, i32_at(data, 0), f32_at(data, 4)),
        b"MISC" => (ItemKind::Misc, i32_at(data, 0), f32_at(data, 4)),
        b"KEYM" => (ItemKind::Key, i32_at(data, 0), f32_at(data, 4)),
        b"SLGM" => (ItemKind::SoulGem, i32_at(data, 0), f32_at(data, 4)),
        b"INGR" => (ItemKind::Ingredient, i32_at(data, 0), f32_at(data, 4)),
        // Value in the enchanted item data (ENIT).
        b"ALCH" => (
            ItemKind::Potion,
            i32_at(rec.get(b"ENIT").unwrap_or(&[]), 0),
            f32_at(data, 0),
        ),
        b"SCRL" => (ItemKind::Scroll, i32_at(data, 0), f32_at(data, 4)),
        b"BOOK" => (ItemKind::Book, i32_at(data, 8), f32_at(data, 12)),
        // SSE: projectile, flags, damage, value, weight.
        b"AMMO" => (ItemKind::Ammo, i32_at(data, 12), f32_at(data, 16)),
        b"LIGH" => {
            let l = super::records::light_data(&rec)?;
            if l.flags & 0x2 == 0 {
                return None;
            }
            (ItemKind::Light, i32_at(data, 40), f32_at(data, 44))
        }
        _ => return None,
    };
    let name = rec
        .get(b"FULL")
        .map(|d| lo.lstring(&rec, d))
        .unwrap_or_default();
    Some(ItemInfo {
        name: if name.is_empty() {
            rec.editor_id().unwrap_or_default().to_owned()
        } else {
            name
        },
        kind,
        value,
        weight,
    })
}

/// Biped slot 39 (shield).
const SLOT_SHIELD: u32 = 1 << 9;

/// What an actor holds (`Actor.GetEquippedSpell`'s slots): the right hand,
/// the left, and the voice (a power, lesser power or shout).
pub const RIGHT: usize = 0;
pub const LEFT: usize = 1;
pub const VOICE: usize = 2;

/// The equip slots (`EQUP`) the others are built from: Skyrim.esm's
/// `RightHand`, `LeftHand` and `Voice`.
const RIGHT_HAND_SLOT: FormId = FormId(0x13F42);
const LEFT_HAND_SLOT: FormId = FormId(0x13F43);
const VOICE_SLOT: FormId = FormId(0x25BEE);
/// `EQUP` `DATA` flag: the item fills all of the parent slots at once
/// (`BothHands`, and `Shield` with `LeftHand` its only parent).
const USE_ALL_PARENTS: u32 = 0x1;

/// Where an item or spell is held, from its equip slot (`ETYP`): the hands it
/// may go in, whether it fills all of them at once, or the voice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EquipSlot {
    pub right: bool,
    pub left: bool,
    pub all: bool,
    pub voice: bool,
}

impl EquipSlot {
    const EITHER: EquipSlot = EquipSlot {
        right: true,
        left: true,
        all: false,
        voice: false,
    };
    const BOTH: EquipSlot = EquipSlot {
        all: true,
        ..EquipSlot::EITHER
    };
    const LEFT: EquipSlot = EquipSlot {
        right: false,
        left: true,
        all: true,
        voice: false,
    };

    /// The hands it takes when put in `want` (`RIGHT` / `LEFT`, or wherever it
    /// goes first: the right hand if it can): all of them for two-handed
    /// items, else the one asked for if it may go there. `any_hand` lets an
    /// item go in a hand its slot doesn't name, as one-handed weapons (most
    /// of them `RightHand`) go in the left from the menus.
    pub fn hands(self, want: Option<usize>, any_hand: bool) -> Vec<usize> {
        let mine = |h: usize| if h == RIGHT { self.right } else { self.left };
        let all: Vec<usize> = [RIGHT, LEFT].into_iter().filter(|&h| mine(h)).collect();
        if self.all || all.is_empty() {
            return all;
        }
        match want {
            Some(h) if h != VOICE && (mine(h) || any_hand) => vec![h],
            _ => vec![all[0]],
        }
    }
}

/// An item's or spell's equip slot: its `ETYP`'s hands (the slot's own, or
/// its parents', `PNAM`) or the voice; without one, what its kind takes
/// (two-handed weapons both hands, others the right, shields and torches the
/// left, spells either, powers and shouts the voice). `None` for what isn't
/// held: apparel, ammunition, potions.
pub fn equip_slot(lo: &LoadOrder, form: FormId) -> Option<EquipSlot> {
    let rec = lo.get(form)?;
    let tag = rec.tag().0;
    let shield = tag == *b"ARMO" && armor_slots(&rec) & SLOT_SHIELD != 0;
    if tag == *b"ARMO" && !shield {
        return None;
    }
    if let Some(d) = rec.get(b"ETYP").filter(|d| d.len() >= 4) {
        let slot = rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap())));
        let s = slot_hands(lo, slot, 0);
        if s != EquipSlot::default() {
            return Some(s);
        }
    }
    Some(match &tag {
        b"WEAP" => match weapon_anim_type(&rec) {
            5 | 6 | 7 | 9 => EquipSlot::BOTH,
            _ => EquipSlot {
                right: true,
                ..EquipSlot::default()
            },
        },
        b"ARMO" => EquipSlot::LEFT,
        b"LIGH" => {
            let carried = super::records::light_data(&rec).is_some_and(|l| l.flags & 0x2 != 0);
            if !carried {
                return None;
            }
            EquipSlot::LEFT
        }
        b"SHOU" => EquipSlot {
            voice: true,
            ..EquipSlot::default()
        },
        // SPIT spell type: 0 a spell, 2 / 3 powers, 11 a voice power.
        b"SPEL" => match rec.get(b"SPIT").and_then(|d| d.get(8)) {
            Some(2 | 3 | 11) => EquipSlot {
                voice: true,
                ..EquipSlot::default()
            },
            _ => EquipSlot::EITHER,
        },
        b"SCRL" => EquipSlot::EITHER,
        _ => return None,
    })
}

/// The hands (or voice) an `EQUP` stands for: its own, else its parents'.
fn slot_hands(lo: &LoadOrder, slot: FormId, depth: usize) -> EquipSlot {
    match slot {
        RIGHT_HAND_SLOT => {
            return EquipSlot {
                right: true,
                ..EquipSlot::default()
            };
        }
        LEFT_HAND_SLOT => {
            return EquipSlot {
                left: true,
                ..EquipSlot::default()
            };
        }
        VOICE_SLOT => {
            return EquipSlot {
                voice: true,
                ..EquipSlot::default()
            };
        }
        _ => {}
    }
    let Some(rec) = lo.get(slot).filter(|r| r.tag().0 == *b"EQUP") else {
        return EquipSlot::default();
    };
    if depth > 4 {
        return EquipSlot::default();
    }
    let mut out = EquipSlot::default();
    for p in rec.get(b"PNAM").unwrap_or(&[]).chunks_exact(4) {
        let p = slot_hands(
            lo,
            rec.fid(FormId(u32::from_le_bytes(p.try_into().unwrap()))),
            depth + 1,
        );
        out.right |= p.right;
        out.left |= p.left;
        out.voice |= p.voice;
    }
    out.all = rec
        .get(b"DATA")
        .filter(|d| d.len() >= 4)
        .is_some_and(|d| u32::from_le_bytes(d[0..4].try_into().unwrap()) & USE_ALL_PARENTS != 0);
    out
}

/// Whether a form is held without being carried (spells, powers, shouts).
pub(crate) fn is_magic(lo: &LoadOrder, form: FormId) -> bool {
    matches!(lo.tag_of(form).map(|t| t.0), Some(t) if t == *b"SPEL" || t == *b"SHOU")
}

#[derive(Debug, Clone, Default)]
pub struct Inventory {
    pub copies: Vec<crate::item_copies::ItemCopy>,
    pub equipped_copies: std::collections::HashMap<FormId, crate::item_copies::CopyState>,
    /// Item and count, in the order they were added.
    pub items: Vec<(FormId, i32)>,
    /// Worn and wielded items: armor, ammunition, and the items in hand.
    worn: Vec<FormId>,
    /// How many of an item belong to someone else (item, owner, count): the
    /// player's stolen goods, kept apart within `items`' counts.
    pub owned: Vec<(FormId, FormId, i32)>,
    /// How many of a soul gem hold a soul (1 petty .. 5 grand) their record
    /// doesn't (gem, soul, count): trapped into an empty gem, kept apart
    /// within `items`' counts and taken last.
    pub souls: Vec<(FormId, u8, i32)>,
    /// Weapons carrying a poison (weapon, poison, doses left): one of the
    /// weapons carried, kept apart within `items`' counts and taken last
    /// (the game's `ExtraPoison` on one instance).
    pub poisoned: Vec<(FormId, FormId, i32)>,
    /// Enchanted weapons with charge spent (weapon, charge left), one entry
    /// per depleted copy. Unlisted copies are full and taken first; the first
    /// listed copy is the one used by casting, combat and recharging.
    pub charges: Vec<(FormId, f32)>,
    /// What is held (`RIGHT`, `LEFT`, `VOICE`): weapons, spells, a shield, a
    /// torch; a power or shout. What fills both hands is in each.
    held: [Option<FormId>; 3],
    /// Whether what is in the right hand fills the left too (a two-handed
    /// weapon, a spell cast with both hands), rather than being one of a
    /// pair (two of the same sword).
    both: bool,
}

/// Items removed from an inventory, with the per-copy state that must travel
/// with them when transferred or dropped.
#[derive(Debug, Default, PartialEq)]
pub struct RemovedItems {
    pub copies: Vec<crate::item_copies::ItemCopy>,
    pub parts: Vec<(Option<FormId>, i32)>,
    pub souls: Vec<(u8, i32)>,
    pub charges: Vec<f32>,
}

/// What an actor holds in a hand, as `GetEquippedItemType` and the behaviour
/// graph's `iLeftHandType` / `iRightHandType` number it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandType {
    Empty = 0,
    Sword = 1,
    Dagger = 2,
    Axe = 3,
    Mace = 4,
    Greatsword = 5,
    Battleaxe = 6,
    Bow = 7,
    Staff = 8,
    Spell = 9,
    Shield = 10,
    Torch = 11,
    Crossbow = 12,
}

impl HandType {
    /// From a weapon's animation type (`DNAM`).
    fn of_weapon(anim: u8) -> HandType {
        match anim {
            1 => HandType::Sword,
            2 => HandType::Dagger,
            3 => HandType::Axe,
            4 => HandType::Mace,
            5 => HandType::Greatsword,
            6 => HandType::Battleaxe,
            7 => HandType::Bow,
            8 => HandType::Staff,
            9 => HandType::Crossbow,
            _ => HandType::Empty,
        }
    }

    pub fn one_handed(self) -> bool {
        matches!(
            self,
            HandType::Sword | HandType::Dagger | HandType::Axe | HandType::Mace
        )
    }

    /// As the behaviour graph's `iLeftHandType` / `iRightHandType` get it
    /// (spells 9, as the game sets them: `known_ambiguities/player-body.md`).
    pub fn for_graph(self) -> f32 {
        self as i32 as f32
    }
}

impl Inventory {
    pub fn count(&self, form: FormId) -> i32 {
        self.items
            .iter()
            .filter(|(f, _)| *f == form)
            .map(|(_, n)| n)
            .sum()
    }

    pub fn add(&mut self, form: FormId, n: i32) {
        if n <= 0 || form.is_null() {
            return;
        }
        match self.items.iter_mut().find(|(f, _)| *f == form) {
            Some((_, c)) => *c += n,
            None => self.items.push((form, n)),
        }
    }

    /// Add `n` of an item that belongs to `owner` (stolen); `None` is the same
    /// as `add`. Gold is never marked.
    pub fn add_owned(&mut self, form: FormId, owner: Option<FormId>, n: i32) {
        if n <= 0 || form.is_null() {
            return;
        }
        self.add(form, n);
        let Some(owner) = owner.filter(|_| form != FormId(0xF)) else {
            return;
        };
        match self
            .owned
            .iter_mut()
            .find(|(f, o, _)| *f == form && *o == owner)
        {
            Some((_, _, c)) => *c += n,
            None => self.owned.push((form, owner, n)),
        }
    }

    /// How many of an item belong to `owner`; with `None`, how many belong to nobody.
    pub fn count_owned(&self, form: FormId, owner: Option<FormId>) -> i32 {
        let owned = |o: Option<FormId>| {
            self.owned
                .iter()
                .filter(|(f, x, _)| *f == form && o.is_none_or(|o| *x == o))
                .map(|(_, _, n)| n)
                .sum::<i32>()
        };
        match owner {
            Some(o) => owned(Some(o)),
            None => self.count(form) - owned(None),
        }
    }

    /// Take up to `n` of `form` away, with whom each part belonged to: those
    /// belonging to nobody first, then the owned (stolen) ones; or only those
    /// `only` names (`Some(None)`: nobody's). The last one taken is unequipped.
    pub fn remove_split(
        &mut self,
        form: FormId,
        n: i32,
        only: Option<Option<FormId>>,
    ) -> Vec<(Option<FormId>, i32)> {
        self.remove_with_souls(form, n, only).0
    }

    /// Remove items together with their per-instance souls. Callers moving
    /// items to another inventory must restore these souls there after adding
    /// the item counts. Weapon charge is discarded; transfers needing it
    /// must use `remove_with_extras`. Destructive removal discards all extras.
    pub fn remove_with_souls(
        &mut self,
        form: FormId,
        n: i32,
        only: Option<Option<FormId>>,
    ) -> (Vec<(Option<FormId>, i32)>, Vec<(u8, i32)>) {
        let removed = self.remove_with_extras(form, n, only);
        (removed.parts, removed.souls)
    }

    /// Remove items and their sparse per-copy souls and weapon charges.
    /// Full-charge copies leave first, then depleted copies in reverse order
    /// so the copy selected by `charge_left` remains until the last removal.
    pub fn remove_with_extras(
        &mut self,
        form: FormId,
        n: i32,
        only: Option<Option<FormId>>,
    ) -> RemovedItems {
        let Some(i) = self.items.iter().position(|(f, _)| *f == form) else {
            return RemovedItems::default();
        };
        let mut plain = vec![(None, self.plain_copies(form, None))];
        plain.extend(
            self.owned
                .iter()
                .filter(|(f, _, _)| *f == form)
                .map(|(_, o, _)| (Some(*o), self.plain_copies(form, Some(*o)))),
        );
        let mut left = n.max(0);
        let mut out = Vec::new();
        if only.is_none_or(|o| o.is_none()) {
            let free = self.count_owned(form, None).min(left);
            if free > 0 {
                out.push((None, free));
                left -= free;
            }
        }
        for (_, o, c) in self.owned.iter_mut().filter(|(f, _, _)| *f == form) {
            if left <= 0 {
                break;
            }
            if only.is_some_and(|w| w != Some(*o)) {
                continue;
            }
            let t = (*c).min(left);
            *c -= t;
            left -= t;
            out.push((Some(*o), t));
        }
        self.owned.retain(|(_, _, c)| *c > 0);
        let taken: i32 = out.iter().map(|(_, n)| n).sum();
        self.items[i].1 -= taken;
        if self.items[i].1 <= 0 {
            self.items.remove(i);
        }
        // Those holding a soul go last.
        let mut souls = Vec::new();
        let mut extra = self.souls_in(form) - self.count(form);
        for (_, soul, c) in self.souls.iter_mut().filter(|(f, _, _)| *f == form) {
            let t = (*c).min(extra.max(0));
            if t > 0 {
                souls.push((*soul, t));
            }
            *c -= t;
            extra -= t;
        }
        self.souls.retain(|(_, _, c)| *c > 0);
        let mut extra =
            self.charges.iter().filter(|(w, _)| *w == form).count() as i32 - self.count(form);
        let mut charges = Vec::new();
        for i in (0..self.charges.len()).rev() {
            if extra > 0 && self.charges[i].0 == form {
                charges.push(self.charges.remove(i).1);
                extra -= 1;
            }
        }
        charges.reverse();
        // The poisoned one goes last.
        if self.count(form) <= 0 {
            self.poisoned.retain(|(w, _, _)| *w != form);
        }
        self.unequip_if_short(form);
        let copies = self.detach_copies(form, &out, &plain);
        RemovedItems {
            copies,
            parts: out,
            souls,
            charges,
        }
    }

    /// The poison on one of the carried `weapon`s, with its doses left.
    pub fn poison_on(&self, weapon: FormId) -> Option<(FormId, i32)> {
        if let Some(poison) = self.copy_state(weapon).poison {
            return Some(poison);
        }
        self.poisoned
            .iter()
            .find(|(w, _, _)| *w == weapon)
            .map(|&(_, p, n)| (p, n))
    }

    /// One of the carried `weapon`s takes `doses` of `poison`.
    pub fn poison(&mut self, weapon: FormId, poison: FormId, doses: i32) {
        let old = self.copy_state(weapon);
        if old != Default::default() {
            if let Some(copy) = self
                .copies
                .iter()
                .find(|c| c.item == weapon && c.state == old)
                .cloned()
            {
                let mut new = old.clone();
                new.poison = (doses > 0).then_some((poison, doses));
                self.replace_copy_state(weapon, copy.owner, &old, new);
                return;
            }
        }
        self.poisoned.retain(|(w, _, _)| *w != weapon);
        if doses > 0 && self.count(weapon) > 0 {
            self.poisoned.push((weapon, poison, doses));
        }
    }

    /// A dose of the poison on `weapon` used: the poison, if it had one.
    pub fn take_poison_dose(&mut self, weapon: FormId) -> Option<FormId> {
        if let Some((poison, doses)) = self.copy_state(weapon).poison {
            self.change_equipped_copy(weapon, |s| {
                s.poison = (doses > 1).then_some((poison, doses - 1))
            });
            return Some(poison);
        }
        let i = self.poisoned.iter().position(|(w, _, _)| *w == weapon)?;
        let poison = self.poisoned[i].1;
        self.poisoned[i].2 -= 1;
        if self.poisoned[i].2 <= 0 {
            self.poisoned.remove(i);
        }
        Some(poison)
    }

    /// The charge left on one of the carried `weapon`s, if any was spent.
    pub fn charge_left(&self, weapon: FormId) -> Option<f32> {
        self.charges
            .iter()
            .find(|(w, _)| *w == weapon)
            .map(|&(_, c)| c)
    }

    /// Update the selected carried copy's charge, leaving other copies alone.
    /// A fully recharged copy needs no sparse entry; the next depleted copy
    /// becomes selected until inventory rows can identify individual copies.
    pub fn set_charge(&mut self, weapon: FormId, left: f32, full: f32) {
        if self.copy_state(weapon).capacity > 0.
            && self.change_equipped_copy(weapon, |s| s.charge = left.clamp(0.0, full))
        {
            return;
        }

        if let Some(i) = self.charges.iter().position(|(w, _)| *w == weapon) {
            if left < full {
                self.charges[i].1 = left.max(0.0);
            } else {
                self.charges.remove(i);
            }
        } else {
            self.add_charge(weapon, left, full);
        }
    }

    /// Restore another depleted copy after adding its item count. Unlike
    /// `set_charge`, this preserves any depleted copies already carried.
    pub fn add_charge(&mut self, weapon: FormId, left: f32, full: f32) {
        let depleted = self.charges.iter().filter(|(w, _)| *w == weapon).count() as i32;
        if left < full && depleted < self.count(weapon) {
            self.charges.push((weapon, left.max(0.0)));
        }
    }

    /// How many of a soul gem hold a soul of the side list's.
    fn souls_in(&self, form: FormId) -> i32 {
        self.copies
            .iter()
            .filter(|c| c.item == form && c.state.soul.is_some_and(|s| s > 0))
            .map(|c| c.count)
            .sum::<i32>()
            + self
                .souls
                .iter()
                .filter(|(f, _, _)| *f == form)
                .map(|(_, _, n)| n)
                .sum::<i32>()
    }

    /// Of the soul gems `form` carried, how many hold no soul but their
    /// record's.
    pub fn without_soul(&self, form: FormId) -> i32 {
        self.count(form) - self.souls_in(form)
    }

    /// One of the carried `form` gems holding `soul` gives it up: a soul of
    /// the side list's, else (`record`: the soul its record holds) one of
    /// those holding none of the side list's. The gem is used up unless
    /// `keep` (reusable gems, which then hold nothing; only side-list souls
    /// can be taken from them). Whether there was one.
    pub fn spend_soul(&mut self, form: FormId, soul: u8, record: u8, keep: bool) -> bool {
        if let Some(copy) = self
            .copies
            .iter()
            .find(|c| c.item == form && c.state.soul == Some(soul) && soul > 0 && c.count > 0)
            .cloned()
        {
            if keep {
                let mut empty = copy.state.clone();
                empty.soul = Some(0);
                self.replace_copy_state(form, copy.owner, &copy.state, empty);
            } else {
                self.remove_copy(form, 1, copy.owner, &copy.state);
            }
            return true;
        }
        if let Some(i) = self
            .souls
            .iter()
            .position(|&(f, s, n)| f == form && s == soul && n > 0)
        {
            self.souls[i].2 -= 1;
            self.souls.retain(|(_, _, c)| *c > 0);
        } else if soul == 0 || soul != record || keep || self.without_soul(form) <= 0 {
            return false;
        }
        if !keep {
            // Souls left are counted against those still carried: none go.
            self.remove_split(form, 1, None);
        }
        true
    }

    /// One of the carried `form` gems takes a `soul` (1 petty .. 5 grand).
    pub fn put_soul(&mut self, form: FormId, soul: u8) {
        if let Some(copy) = self
            .copies
            .iter()
            .find(|c| c.item == form && c.state.soul == Some(0) && c.count > 0)
            .cloned()
        {
            let mut filled = copy.state.clone();
            filled.soul = Some(soul);
            self.replace_copy_state(form, copy.owner, &copy.state, filled);
            return;
        }
        match self
            .souls
            .iter_mut()
            .find(|(f, s, _)| *f == form && *s == soul)
        {
            Some((_, _, c)) => *c += 1,
            None => self.souls.push((form, soul, 1)),
        }
    }

    /// What is held in each hand and the voice, and whether the right hand's
    /// item fills both (what saves keep, with `equipped`).
    pub fn hands(&self) -> ([Option<FormId>; 3], bool) {
        (self.held, self.both)
    }

    /// This inventory with the equipment a save recorded (`equipped`, `hands`).
    pub fn with_equipment(
        mut self,
        worn: Vec<FormId>,
        held: [Option<FormId>; 3],
        both: bool,
    ) -> Self {
        self.worn = worn;
        self.held = held;
        self.both = both;
        self
    }

    /// What is worn and wielded: armor, ammunition, the items in hand.
    pub fn equipped(&self) -> &[FormId] {
        &self.worn
    }

    pub fn is_equipped(&self, form: FormId) -> bool {
        self.worn.contains(&form)
    }

    /// What is held in a slot (`RIGHT`, `LEFT`, `VOICE`).
    pub fn held(&self, slot: usize) -> Option<FormId> {
        self.held.get(slot).copied().flatten()
    }

    /// The right hand, the left and the voice.
    pub fn held_all(&self) -> [Option<FormId>; 3] {
        self.held
    }

    /// Whether what is in the right hand fills both.
    pub fn both_hands(&self) -> bool {
        self.both && self.held[RIGHT].is_some()
    }

    /// Wear a piece of apparel or ammunition (in hand: see `hold`).
    pub fn wear(&mut self, form: FormId) {
        if !self.worn.contains(&form) {
            self.worn.push(form);
        }
    }

    /// Wear it, or hold it where it goes first (`equip_slot`).
    pub fn equip(&mut self, lo: &LoadOrder, form: FormId) {
        match equip_slot(lo, form) {
            Some(s) => {
                self.hold(lo, form, &s.hands(None, false));
            }
            None => self.wear(form),
        }
    }

    /// Take something off, out of every hand that holds it.
    pub fn take_off(&mut self, form: FormId) {
        self.take_off_where(|f| f == form);
    }

    /// Take off what `off` picks, worn or held.
    pub fn take_off_where(&mut self, off: impl Fn(FormId) -> bool) {
        self.worn.retain(|&f| !off(f));
        for h in &mut self.held {
            if h.is_some_and(&off) {
                *h = None;
            }
        }
        if self.held[RIGHT].is_none() || self.held[LEFT].is_none() {
            self.both = false;
        }
    }

    /// Hold an item or spell in `hands` (or a power or shout as the voice),
    /// in place of what is there; what filled both hands leaves both. The
    /// only one of an item, held in the other hand, moves over; a second of
    /// it can go in each. Returns the items no longer held at all.
    pub fn hold(&mut self, lo: &LoadOrder, form: FormId, hands: &[usize]) -> Vec<FormId> {
        self.hold_by(form, hands, |f| is_magic(lo, f))
    }

    /// `hold`, with `is_magic` telling spells from items.
    fn hold_by(
        &mut self,
        form: FormId,
        hands: &[usize],
        is_magic: impl Fn(FormId) -> bool,
    ) -> Vec<FormId> {
        let before = self.held;
        let magic = is_magic(form);
        if hands == [VOICE] {
            self.held[VOICE] = Some(form);
            return Vec::new();
        }
        let all = hands.len() > 1;
        // What filled both hands leaves both, else only the hands it goes in.
        if self.both {
            self.held[RIGHT] = None;
            self.held[LEFT] = None;
        }
        if !magic && !all && self.count(form) < 2 {
            for h in [RIGHT, LEFT] {
                if !hands.contains(&h) && self.held[h] == Some(form) {
                    self.held[h] = None;
                }
            }
        }
        for &h in hands {
            self.held[h] = Some(form);
        }
        self.both = all;
        if !magic {
            self.wear(form);
        }
        let mut gone = Vec::new();
        for f in before.into_iter().take(2).flatten() {
            if !self.held.contains(&Some(f)) && !gone.contains(&f) && !is_magic(f) {
                self.worn.retain(|&w| w != f);
                gone.push(f);
            }
        }
        gone
    }

    /// Empty a hand (both, when what is in it fills them), or the voice.
    /// Returns what is no longer held at all.
    pub fn release(&mut self, lo: &LoadOrder, slot: usize) -> Option<FormId> {
        self.release_by(slot, |f| is_magic(lo, f))
    }

    fn release_by(&mut self, slot: usize, is_magic: impl Fn(FormId) -> bool) -> Option<FormId> {
        let f = self.held.get(slot).copied().flatten()?;
        if slot != VOICE && self.both {
            self.held[RIGHT] = None;
            self.held[LEFT] = None;
            self.both = false;
        } else {
            self.held[slot] = None;
        }
        if self.held.contains(&Some(f)) {
            return None;
        }
        if !is_magic(f) {
            self.worn.retain(|&w| w != f);
        }
        Some(f)
    }

    /// Fewer of an item than the hands holding it: none left takes it off,
    /// one left of a pair leaves the left hand.
    pub fn unequip_if_short(&mut self, form: FormId) {
        let n = self.count(form);
        if n <= 0 && self.worn.contains(&form) {
            self.take_off(form);
        } else if n == 1
            && !self.both
            && self.held[RIGHT] == Some(form)
            && self.held[LEFT] == Some(form)
        {
            self.held[LEFT] = None;
        }
    }

    /// What is in a hand (`left` or right).
    pub fn hand(&self, lo: &LoadOrder, left: bool) -> HandType {
        let Some(rec) = self.held[if left { LEFT } else { RIGHT }].and_then(|f| lo.get(f)) else {
            return HandType::Empty;
        };
        match &rec.tag().0 {
            b"WEAP" => HandType::of_weapon(weapon_anim_type(&rec)),
            b"ARMO" => HandType::Shield,
            b"LIGH" => HandType::Torch,
            b"SPEL" | b"SCRL" => HandType::Spell,
            _ => HandType::Empty,
        }
    }

    /// The weapon in the right hand (a two-handed one too), if any: what
    /// the right attack fights with.
    pub fn weapon(&self, lo: &LoadOrder) -> Option<FormId> {
        self.weapon_in(lo, RIGHT)
    }

    /// The weapon in a hand (`RIGHT` / `LEFT`).
    pub fn weapon_in(&self, lo: &LoadOrder, hand: usize) -> Option<FormId> {
        self.held(hand)
            .filter(|&f| lo.tag_of(f).map(|t| t.0) == Some(*b"WEAP"))
    }

    /// The shield held, if any.
    pub fn shield(&self, lo: &LoadOrder) -> Option<FormId> {
        self.held(LEFT)
            .filter(|&f| lo.tag_of(f).map(|t| t.0) == Some(*b"ARMO"))
    }
}

/// The items listed in a record's `CNTO` subrecords (NPC_, CONT), leveled lists
/// resolved by `seed`.
pub fn listed_items(
    lo: &LoadOrder,
    rec: &LoadedRecord<'_>,
    seed: u64,
    globals: &Globals,
) -> Vec<(FormId, i32)> {
    rec.subrecords()
        .filter(|s| s.tag.0 == *b"CNTO" && s.data.len() >= 8)
        .flat_map(|s| {
            resolve_items(
                lo,
                rec.fid(s.form_id(0)),
                i32::from_le_bytes(s.data[4..8].try_into().unwrap()),
                seed,
                globals,
                0,
            )
        })
        .collect()
}

/// A container's starting contents.
pub fn container_inventory(
    lo: &LoadOrder,
    cont: FormId,
    seed: u64,
    globals: &Globals,
) -> Inventory {
    let mut inv = Inventory::default();
    if let Some(rec) = lo.get(cont).filter(|r| r.tag().0 == *b"CONT") {
        for (f, n) in listed_items(lo, &rec, seed, globals) {
            inv.add(f, n);
        }
    }
    inv
}

pub fn weapon_anim_type(rec: &LoadedRecord<'_>) -> u8 {
    rec.get(b"DNAM")
        .and_then(|d| d.first().copied())
        .unwrap_or(0)
}

pub fn armor_slots(rec: &LoadedRecord<'_>) -> u32 {
    rec.get(b"BOD2")
        .or_else(|| rec.get(b"BODT"))
        .filter(|d| d.len() >= 4)
        .map_or(0, |d| u32::from_le_bytes(d[0..4].try_into().unwrap()))
}

/// The model of a weapon (following its template when it has none of its own).
pub fn weapon_model(lo: &LoadOrder, weap: FormId) -> Option<String> {
    let mut id = weap;
    for _ in 0..4 {
        let rec = lo.get(id)?;
        if let Some(m) = rec
            .get(b"MODL")
            .map(esp::decode_zstring)
            .filter(|m| !m.is_empty())
        {
            return Some(super::records::mesh_path(&m));
        }
        let t = rec.get(b"CNAM").filter(|d| d.len() >= 4)?;
        id = rec.fid(FormId(u32::from_le_bytes(t[0..4].try_into().unwrap())));
    }
    None
}

/// NPC skill values (`DNAM`): one-handed, two-handed, archery, block.
fn skills(npc: &LoadedRecord<'_>) -> [f32; 4] {
    let d = npc.get(b"DNAM").unwrap_or(&[]);
    std::array::from_fn(|i| d.get(i).copied().unwrap_or(15) as f32)
}

/// A combat style's equipment score multipliers (`CSGD`): melee, ranged.
fn style_weights(lo: &LoadOrder, style: FormId) -> (f32, f32) {
    let csgd = lo
        .get(style)
        .and_then(|r| r.get(b"CSGD").filter(|d| d.len() >= 24).map(|d| d.to_vec()));
    let f = |d: &[u8], o: usize| f32::from_le_bytes(d[o..o + 4].try_into().unwrap());
    csgd.map_or((1.0, 1.0), |d| (f(&d, 12), f(&d, 20)))
}

/// A combat style's staff score multiplier (`CSGD` 0x20; 1 without one).
fn style_staff_weight(lo: &LoadOrder, style: FormId) -> f32 {
    lo.get(style)
        .and_then(|r| {
            let d = r.get(b"CSGD")?.get(0x20..0x24)?;
            Some(f32::from_le_bytes(d.try_into().unwrap()))
        })
        .unwrap_or(1.0)
}

/// The enchanted staff an NPC would take up: the most valuable it carries.
fn best_staff(lo: &LoadOrder, inv: &Inventory) -> Option<FormId> {
    inv.items
        .iter()
        .filter_map(|&(f, _)| {
            let rec = lo.get(f).filter(|r| r.tag().0 == *b"WEAP")?;
            let staff = HandType::of_weapon(weapon_anim_type(&rec)) == HandType::Staff;
            (staff && rec.get(b"EITM").is_some() && weapon_model(lo, f).is_some())
                .then(|| (item_info(lo, f).map_or(0, |i| i.value), f))
        })
        .max_by_key(|s| s.0)
        .map(|s| s.1)
}

/// Combat style `DATA` flag: allow dual wielding (`csMercerFreyMelee`,
/// `csDraugrMagicAllowDual`...).
const STYLE_DUAL_WIELD: u32 = 0x4;

/// Whether a combat style allows dual wielding.
fn style_dual_wields(lo: &LoadOrder, style: FormId) -> bool {
    lo.get(style)
        .and_then(|r| {
            r.get(b"DATA")
                .filter(|d| d.len() >= 4)
                .map(|d| u32::from_le_bytes(d[0..4].try_into().unwrap()))
        })
        .is_some_and(|f| f & STYLE_DUAL_WIELD != 0)
}

/// Choose what an NPC wields from what it carries, as the game does when nothing
/// was equipped by hand: a melee weapon unless its combat style favours ranged
/// ones (guards carry their bow but wear their sword), the one that scores best in
/// its hands (base damage scaled by the NPC's skill with it), and with a one-handed
/// weapon (or none) its best shield; or, when its style allows dual wielding
/// and it carries no shield, its next best one-handed weapon in the left hand
/// (a second of the same one too). A shield wins over a second weapon (`test-mod
/// autotest4`: a dual wielding NPC given two swords and a shield held one
/// sword and the shield). Staffs, crossbows and weapons without a model (creature attacks) are
/// left alone; unplayable ones with a model are the NPCs' own (giants' clubs,
/// Dremora greatswords, the Keepers' dragonbone). An enchanted staff (the
/// most valuable) is taken up instead, in the right hand with the left left
/// free for spells, when the style weighs staffs (`CSGD` 0x20) at least as
/// much as melee and ranged weapons (warlocks, dragon priests, Falmer
/// shamans; `known_ambiguities/staffs.md`).
pub fn equip_weapons(lo: &LoadOrder, inv: &mut Inventory, skill_npc: FormId, style: FormId) {
    let skill = lo.get(skill_npc).map_or([15.0; 4], |r| skills(&r));
    let (melee, ranged) = style_weights(lo, style);
    let archer = ranged > melee;
    if style_staff_weight(lo, style) >= melee.max(ranged)
        && let Some(staff) = best_staff(lo, inv)
    {
        log::debug!("{skill_npc} takes up its staff {staff}");
        inv.hold(lo, staff, &[RIGHT]);
        return;
    }
    let mut best: Option<(bool, f32, FormId, HandType)> = None;
    let mut one_handed: Vec<(f32, FormId, i32)> = Vec::new();
    for &(f, n) in &inv.items {
        let Some(rec) = lo.get(f).filter(|r| r.tag().0 == *b"WEAP") else {
            continue;
        };
        let hand = HandType::of_weapon(weapon_anim_type(&rec));
        if matches!(hand, HandType::Empty | HandType::Staff | HandType::Crossbow)
            || weapon_model(lo, f).is_none()
        {
            continue;
        }
        let damage = rec
            .get(b"DATA")
            .filter(|d| d.len() >= 10)
            .map_or(0.0, |d| u16::from_le_bytes([d[8], d[9]]) as f32);
        let s = match hand {
            HandType::Greatsword | HandType::Battleaxe => skill[1],
            HandType::Bow => skill[2],
            _ => skill[0],
        };
        let preferred = (hand == HandType::Bow) == archer;
        let score = (damage + 1.0) * (0.5 + s / 100.0);
        log::trace!(
            "{skill_npc} could wield {f} ({hand:?}, damage {damage}, skill {s}, preferred {preferred}): {score:.1}"
        );
        if best.is_none_or(|b| (preferred, score) > (b.0, b.1)) {
            best = Some((preferred, score, f, hand));
        }
        if hand.one_handed() {
            one_handed.push((score, f, n));
        }
    }
    let free_hand = best.is_none_or(|b| b.3.one_handed());
    // Two hands on the weapon: a shield from the outfit goes unworn.
    if let Some((_, _, f, _)) = best {
        inv.equip(lo, f);
    }
    let carries_shield = inv.items.iter().any(|&(f, _)| {
        lo.get(f)
            .is_some_and(|r| r.tag().0 == *b"ARMO" && armor_slots(&r) & SLOT_SHIELD != 0)
    });
    // A second weapon for the left hand, with no shield to hold.
    let second = best
        .filter(|b| b.3.one_handed() && style_dual_wields(lo, style) && !carries_shield)
        .and_then(|b| {
            one_handed
                .iter()
                .filter(|&&(_, f, n)| f != b.2 || n >= 2)
                .max_by(|x, y| x.0.total_cmp(&y.0))
                .map(|&(_, f, _)| f)
        });
    if let Some(f) = second {
        log::debug!(
            "{skill_npc} dual wields {} and {f}",
            best.map_or(FormId::NULL, |b| b.2)
        );
        inv.hold(lo, f, &[LEFT]);
        return;
    }
    if free_hand && inv.shield(lo).is_none() && skill[3] > 0.0 {
        let rating = |r: &LoadedRecord<'_>| {
            r.get(b"DNAM")
                .filter(|d| d.len() >= 4)
                .map_or(0, |d| i32::from_le_bytes(d[0..4].try_into().unwrap()))
        };
        let shield = inv
            .items
            .iter()
            .filter_map(|&(f, _)| {
                lo.get(f)
                    .filter(|r| r.tag().0 == *b"ARMO" && armor_slots(r) & SLOT_SHIELD != 0)
                    .map(|r| (rating(&r), f))
            })
            .max_by_key(|s| s.0);
        if let Some((_, f)) = shield {
            inv.equip(lo, f);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SWORD: FormId = FormId(0x100);
    const DAGGER: FormId = FormId(0x101);
    const GREATSWORD: FormId = FormId(0x102);
    const FLAMES: FormId = FormId(0x200);

    fn magic(f: FormId) -> bool {
        f == FLAMES
    }

    #[test]
    fn spending_a_soul_uses_up_the_gem_unless_it_is_reusable() {
        let gem = FormId(0x300);
        let mut inv = carrying(&[(gem, 3)]);
        inv.put_soul(gem, 2);
        // An empty one has none to give; the one holding a lesser soul does.
        assert!(!inv.spend_soul(gem, 0, 0, false));
        assert!(!inv.spend_soul(gem, 5, 0, false));
        assert!(inv.spend_soul(gem, 2, 0, false));
        assert_eq!(inv.count(gem), 2);
        assert!(inv.souls.is_empty());
        // A reusable gem stays, empty.
        inv.put_soul(gem, 5);
        assert!(inv.spend_soul(gem, 5, 0, true));
        assert_eq!(inv.count(gem), 2);
        assert!(inv.souls.is_empty());
        // Gems filled by their record give their record's soul.
        let filled = FormId(0x301);
        let mut inv = carrying(&[(filled, 1)]);
        assert!(inv.spend_soul(filled, 1, 1, false));
        assert_eq!(inv.count(filled), 0);
    }

    fn carrying(items: &[(FormId, i32)]) -> Inventory {
        let mut inv = Inventory::default();
        for &(f, n) in items {
            inv.add(f, n);
        }
        inv
    }

    #[test]
    fn charge_removal_keeps_full_copies_first_and_selected_copy_last() {
        let mut inv = carrying(&[(SWORD, 4), (DAGGER, 1)]);
        inv.set_charge(SWORD, 10.0, 100.0);
        inv.add_charge(SWORD, 65.0, 100.0);
        inv.set_charge(DAGGER, 5.0, 50.0);
        assert!(inv.remove_with_extras(SWORD, 2, None).charges.is_empty());
        assert_eq!(inv.remove_with_extras(SWORD, 1, None).charges, vec![65.0]);
        assert_eq!(inv.charge_left(SWORD), Some(10.0));
        assert_eq!(inv.remove_with_extras(SWORD, 99, None).charges, vec![10.0]);
        assert_eq!(inv.charge_left(SWORD), None);
        assert_eq!(inv.charge_left(DAGGER), Some(5.0));
    }

    #[test]
    fn charge_transfer_preserves_distinct_copies_and_full_charge_implicit() {
        let mut from = carrying(&[(SWORD, 3)]);
        from.add_charge(SWORD, 0.0, 100.0);
        from.add_charge(SWORD, 60.0, 100.0);
        let mut to = carrying(&[(SWORD, 1)]);
        to.set_charge(SWORD, 30.0, 100.0);
        let removed = from.remove_with_extras(SWORD, 99, None);
        for (owner, n) in removed.parts {
            to.add_owned(SWORD, owner, n);
        }
        for left in removed.charges {
            to.add_charge(SWORD, left, 100.0);
        }
        assert_eq!(to.count(SWORD), 4);
        assert_eq!(to.charges, vec![(SWORD, 30.0), (SWORD, 0.0), (SWORD, 60.0)]);
        assert!(from.charges.is_empty());
        // Spending and recharging only change the selected copy.
        to.set_charge(SWORD, 20.0, 100.0);
        assert_eq!(to.charges, vec![(SWORD, 20.0), (SWORD, 0.0), (SWORD, 60.0)]);
        to.set_charge(SWORD, 100.0, 100.0);
        assert_eq!(to.charges, vec![(SWORD, 0.0), (SWORD, 60.0)]);
        assert!(to.remove_with_extras(SWORD, 2, None).charges.is_empty());
        assert_eq!(
            to.remove_with_extras(SWORD, 2, None).charges,
            vec![0.0, 60.0]
        );
    }

    #[test]
    fn failed_and_destructive_removal_never_leave_or_duplicate_charges() {
        let mut inv = carrying(&[(SWORD, 1)]);
        inv.set_charge(SWORD, 0.0, 100.0);
        inv.add_charge(SWORD, 25.0, 100.0); // No spare copy to carry it.
        for n in [0, -1] {
            assert_eq!(
                inv.remove_with_extras(SWORD, n, None),
                RemovedItems::default()
            );
        }
        assert_eq!(
            inv.remove_with_extras(SWORD, 1, Some(Some(DAGGER))),
            RemovedItems::default()
        );
        assert_eq!(inv.charges, vec![(SWORD, 0.0)]);
        inv.remove_split(SWORD, 1, None);
        assert!(inv.charges.is_empty());
        inv.set_charge(SWORD, 0.0, 100.0); // An absent copy cannot acquire charge.
        assert!(inv.charges.is_empty());
    }

    #[test]
    fn soul_removal_takes_empty_gems_first_and_returns_each_size() {
        let gem = FormId(0x300);
        let mut inv = carrying(&[(gem, 5), (SWORD, 1)]);
        inv.put_soul(gem, 1);
        inv.put_soul(gem, 1);
        inv.put_soul(gem, 4);
        assert!(inv.remove_with_souls(gem, 1, None).1.is_empty());
        let (parts, souls) = inv.remove_with_souls(gem, 3, None);
        assert_eq!(parts, vec![(None, 3)]);
        assert_eq!(souls, vec![(1, 2)]);
        assert_eq!(inv.souls, vec![(gem, 4, 1)]);
        let (parts, souls) = inv.remove_with_souls(gem, 99, None);
        assert_eq!(parts, vec![(None, 1)]);
        assert_eq!(souls, vec![(4, 1)]);
        assert!(inv.souls.is_empty());
        assert_eq!(inv.count(SWORD), 1);
    }

    #[test]
    fn soul_transfer_round_trip_conserves_counts_and_sizes() {
        let gem = FormId(0x300);
        let owner = FormId(0x400);
        let mut source = Inventory::default();
        source.add_owned(gem, Some(owner), 2);
        source.put_soul(gem, 2);
        source.put_soul(gem, 5);
        let mut destination = Inventory::default();
        let transfer = |from: &mut Inventory, to: &mut Inventory| {
            let (parts, souls) = from.remove_with_souls(gem, 2, Some(Some(owner)));
            for (owner, n) in parts {
                to.add_owned(gem, owner, n);
            }
            for (size, n) in souls {
                for _ in 0..n {
                    to.put_soul(gem, size);
                }
            }
        };
        transfer(&mut source, &mut destination);
        transfer(&mut destination, &mut source);
        assert_eq!(source.count_owned(gem, Some(owner)), 2);
        assert_eq!(source.souls, vec![(gem, 2, 1), (gem, 5, 1)]);
        assert!(destination.souls.is_empty());
        assert_eq!(source.count(gem) + destination.count(gem), 2);
        assert_eq!(source.souls_in(gem) + destination.souls_in(gem), 2);
    }

    #[test]
    fn failed_removal_and_destroying_gems_do_not_leave_or_duplicate_souls() {
        let gem = FormId(0x300);
        let mut inv = carrying(&[(gem, 1)]);
        inv.put_soul(gem, 3);
        for n in [0, -1] {
            assert_eq!(inv.remove_with_souls(gem, n, None), (vec![], vec![]));
        }
        assert_eq!(
            inv.remove_with_souls(gem, 1, Some(Some(SWORD))),
            (vec![], vec![])
        );
        assert_eq!(inv.souls, vec![(gem, 3, 1)]);
        inv.remove_split(gem, 1, None);
        assert!(inv.souls.is_empty());
        assert_eq!(inv.remove_with_souls(gem, 1, None), (vec![], vec![]));
    }

    #[test]
    fn slot_hands() {
        let right = EquipSlot {
            right: true,
            ..EquipSlot::default()
        };
        // A one-handed weapon (`RightHand`) goes in the left when asked.
        assert_eq!(right.hands(Some(LEFT), true), vec![LEFT]);
        assert_eq!(right.hands(None, true), vec![RIGHT]);
        // A left-hand spell stays there.
        assert_eq!(EquipSlot::LEFT.hands(Some(RIGHT), false), vec![LEFT]);
        assert_eq!(EquipSlot::EITHER.hands(Some(LEFT), false), vec![LEFT]);
        assert_eq!(EquipSlot::BOTH.hands(Some(LEFT), true), vec![RIGHT, LEFT]);
    }

    #[test]
    fn two_weapons_then_a_two_handed_one() {
        let mut inv = carrying(&[(SWORD, 1), (DAGGER, 1), (GREATSWORD, 1)]);
        inv.hold_by(SWORD, &[RIGHT], magic);
        inv.hold_by(DAGGER, &[LEFT], magic);
        assert_eq!(inv.held_all(), [Some(SWORD), Some(DAGGER), None]);
        assert!(inv.is_equipped(SWORD) && inv.is_equipped(DAGGER));
        let gone = inv.hold_by(GREATSWORD, &[RIGHT, LEFT], magic);
        assert_eq!(gone, vec![SWORD, DAGGER]);
        assert!(inv.both_hands());
        assert_eq!(inv.equipped(), &[GREATSWORD]);
        // A one-handed weapon in either hand takes the two-handed one away.
        assert_eq!(inv.hold_by(DAGGER, &[LEFT], magic), vec![GREATSWORD]);
        assert_eq!(inv.held_all(), [None, Some(DAGGER), None]);
        assert!(!inv.both_hands());
    }

    #[test]
    fn the_only_one_moves_a_second_goes_in_each() {
        let mut inv = carrying(&[(SWORD, 1)]);
        inv.hold_by(SWORD, &[RIGHT], magic);
        inv.hold_by(SWORD, &[LEFT], magic);
        assert_eq!(inv.held_all(), [None, Some(SWORD), None]);
        inv.add(SWORD, 1);
        inv.hold_by(SWORD, &[RIGHT], magic);
        assert_eq!(inv.held_all(), [Some(SWORD), Some(SWORD), None]);
        assert!(!inv.both_hands());
        // One taken away: it leaves the left hand.
        inv.remove_split(SWORD, 1, None);
        assert_eq!(inv.held_all(), [Some(SWORD), None, None]);
        inv.remove_split(SWORD, 1, None);
        assert_eq!(inv.held_all(), [None, None, None]);
        assert!(inv.equipped().is_empty());
    }

    #[test]
    fn spells_share_the_hands() {
        let mut inv = carrying(&[(SWORD, 1), (GREATSWORD, 1)]);
        inv.hold_by(SWORD, &[RIGHT], magic);
        inv.hold_by(FLAMES, &[LEFT], magic);
        assert_eq!(inv.held_all(), [Some(SWORD), Some(FLAMES), None]);
        assert_eq!(inv.equipped(), &[SWORD]);
        // In the right hand too, the sword goes; the spell isn't worn.
        assert_eq!(inv.hold_by(FLAMES, &[RIGHT], magic), vec![SWORD]);
        assert!(inv.equipped().is_empty());
        inv.hold_by(GREATSWORD, &[RIGHT, LEFT], magic);
        assert_eq!(inv.held_all(), [Some(GREATSWORD), Some(GREATSWORD), None]);
        // Emptying either hand puts the two-handed weapon away.
        assert_eq!(inv.release_by(LEFT, magic), Some(GREATSWORD));
        assert_eq!(inv.held_all(), [None, None, None]);
        assert!(inv.equipped().is_empty());
    }

    #[test]
    fn poison_doses_run_out_and_go_with_the_last_weapon() {
        const POISON: FormId = FormId(0x300);
        let mut inv = carrying(&[(SWORD, 2)]);
        inv.poison(SWORD, POISON, 2);
        assert_eq!(inv.poison_on(SWORD), Some((POISON, 2)));
        assert_eq!(inv.take_poison_dose(SWORD), Some(POISON));
        assert_eq!(inv.take_poison_dose(SWORD), Some(POISON));
        assert_eq!(inv.take_poison_dose(SWORD), None);
        // The poisoned one is taken last.
        inv.poison(SWORD, POISON, 1);
        inv.remove_split(SWORD, 1, None);
        assert_eq!(inv.poison_on(SWORD), Some((POISON, 1)));
        inv.remove_split(SWORD, 1, None);
        assert_eq!(inv.poison_on(SWORD), None);
        // None carried, none poisoned.
        inv.poison(SWORD, POISON, 1);
        assert_eq!(inv.poison_on(SWORD), None);
    }
}
