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
fn is_magic(lo: &LoadOrder, form: FormId) -> bool {
    matches!(lo.tag_of(form).map(|t| t.0), Some(t) if t == *b"SPEL" || t == *b"SHOU")
}

#[derive(Debug, Clone, Default)]
pub struct Inventory {
    /// Item and count, in the order they were added.
    pub items: Vec<(FormId, i32)>,
    /// Worn and wielded items: armor, ammunition, and the items in hand.
    worn: Vec<FormId>,
    /// How many of an item belong to someone else (item, owner, count): the
    /// player's stolen goods, kept apart within `items`' counts.
    pub owned: Vec<(FormId, FormId, i32)>,
    /// What is held (`RIGHT`, `LEFT`, `VOICE`): weapons, spells, a shield, a
    /// torch; a power or shout. What fills both hands is in each.
    held: [Option<FormId>; 3],
    /// Whether what is in the right hand fills the left too (a two-handed
    /// weapon, a spell cast with both hands), rather than being one of a
    /// pair (two of the same sword).
    both: bool,
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

    /// As the behaviour graph's `iLeftHandType` / `iRightHandType` get it: a
    /// spell's hand empty, as casting doesn't go through the graph yet.
    pub fn for_graph(self) -> f32 {
        match self {
            HandType::Spell => 0.0,
            h => h as i32 as f32,
        }
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
        let Some(i) = self.items.iter().position(|(f, _)| *f == form) else {
            return Vec::new();
        };
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
        self.unequip_if_short(form);
        out
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

fn weapon_anim_type(rec: &LoadedRecord<'_>) -> u8 {
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

/// Choose what an NPC wields from what it carries, as the game does when nothing
/// was equipped by hand: a melee weapon unless its combat style favours ranged
/// ones (guards carry their bow but wear their sword), the one that scores best in
/// its hands (base damage scaled by the NPC's skill with it), and with a one-handed
/// weapon (or none) its best shield. Staffs, crossbows and weapons without a
/// model (creature attacks) are left alone; unplayable ones with a model are the
/// NPCs' own (giants' clubs, Dremora greatswords, the Keepers' dragonbone).
pub fn equip_weapons(lo: &LoadOrder, inv: &mut Inventory, skill_npc: FormId, style: FormId) {
    let skill = lo.get(skill_npc).map_or([15.0; 4], |r| skills(&r));
    let (melee, ranged) = style_weights(lo, style);
    let archer = ranged > melee;
    let mut best: Option<(bool, f32, FormId, HandType)> = None;
    for &(f, _) in &inv.items {
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
    }
    let one_handed = best.is_none_or(|b| b.3.one_handed());
    // Two hands on the weapon: a shield from the outfit goes unworn.
    if let Some((_, _, f, _)) = best {
        inv.equip(lo, f);
    }
    if one_handed && inv.shield(lo).is_none() && skill[3] > 0.0 {
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

    fn carrying(items: &[(FormId, i32)]) -> Inventory {
        let mut inv = Inventory::default();
        for &(f, n) in items {
            inv.add(f, n);
        }
        inv
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
}
