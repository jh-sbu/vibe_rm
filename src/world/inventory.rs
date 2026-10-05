//! Inventories: what actors and containers carry (`CNTO`, leveled items, outfits) and
//! what actors have equipped.

use esp::{FormId, LoadOrder, LoadedRecord};

use super::actor::resolve_items;

/// Biped slot 39 (shield).
const SLOT_SHIELD: u32 = 1 << 9;

#[derive(Debug, Clone, Default)]
pub struct Inventory {
    /// Item and count, in the order they were added.
    pub items: Vec<(FormId, i32)>,
    /// Worn and wielded items: the outfit's armors, a weapon, a shield.
    pub equipped: Vec<FormId>,
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

    fn one_handed(self) -> bool {
        matches!(self, HandType::Sword | HandType::Dagger | HandType::Axe | HandType::Mace)
    }
}

impl Inventory {
    pub fn count(&self, form: FormId) -> i32 {
        self.items.iter().filter(|(f, _)| *f == form).map(|(_, n)| n).sum()
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

    /// Take up to `n` of `form` away; returns how many were taken. The last one
    /// taken is unequipped.
    pub fn remove(&mut self, form: FormId, n: i32) -> i32 {
        let Some(i) = self.items.iter().position(|(f, _)| *f == form) else { return 0 };
        let taken = n.clamp(0, self.items[i].1);
        self.items[i].1 -= taken;
        if self.items[i].1 <= 0 {
            self.items.remove(i);
            self.equipped.retain(|f| *f != form);
        }
        taken
    }

    pub fn is_equipped(&self, form: FormId) -> bool {
        self.equipped.contains(&form)
    }

    /// What is in a hand (`left` or right).
    pub fn hand(&self, lo: &LoadOrder, left: bool) -> HandType {
        for &f in &self.equipped {
            let Some(rec) = lo.get(f) else { continue };
            match &rec.tag().0 {
                b"WEAP" if !left => return HandType::of_weapon(weapon_anim_type(&rec)),
                b"ARMO" if left && armor_slots(&rec) & SLOT_SHIELD != 0 => return HandType::Shield,
                _ => {}
            }
        }
        HandType::Empty
    }

    /// The equipped weapon, if any.
    pub fn weapon(&self, lo: &LoadOrder) -> Option<FormId> {
        self.equipped.iter().copied().find(|&f| lo.tag_of(f).map(|t| t.0) == Some(*b"WEAP"))
    }
}

/// The items listed in a record's `CNTO` subrecords (NPC_, CONT), leveled lists
/// resolved by `seed`.
pub fn listed_items(lo: &LoadOrder, rec: &LoadedRecord<'_>, seed: u64) -> Vec<(FormId, i32)> {
    rec.subrecords()
        .filter(|s| s.tag.0 == *b"CNTO" && s.data.len() >= 8)
        .flat_map(|s| resolve_items(lo, rec.fid(s.form_id(0)), i32::from_le_bytes(s.data[4..8].try_into().unwrap()), seed, 0))
        .collect()
}

/// A container's starting contents.
pub fn container_inventory(lo: &LoadOrder, cont: FormId, seed: u64) -> Inventory {
    let mut inv = Inventory::default();
    if let Some(rec) = lo.get(cont).filter(|r| r.tag().0 == *b"CONT") {
        for (f, n) in listed_items(lo, &rec, seed) {
            inv.add(f, n);
        }
    }
    inv
}

fn weapon_anim_type(rec: &LoadedRecord<'_>) -> u8 {
    rec.get(b"DNAM").and_then(|d| d.first().copied()).unwrap_or(0)
}

pub fn armor_slots(rec: &LoadedRecord<'_>) -> u32 {
    rec.get(b"BOD2").or_else(|| rec.get(b"BODT")).filter(|d| d.len() >= 4).map_or(0, |d| u32::from_le_bytes(d[0..4].try_into().unwrap()))
}

/// The model of a weapon (following its template when it has none of its own).
pub fn weapon_model(lo: &LoadOrder, weap: FormId) -> Option<String> {
    let mut id = weap;
    for _ in 0..4 {
        let rec = lo.get(id)?;
        if let Some(m) = rec.get(b"MODL").map(esp::decode_zstring).filter(|m| !m.is_empty()) {
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
    let csgd = lo.get(style).and_then(|r| r.get(b"CSGD").filter(|d| d.len() >= 24).map(|d| d.to_vec()));
    let f = |d: &[u8], o: usize| f32::from_le_bytes(d[o..o + 4].try_into().unwrap());
    csgd.map_or((1.0, 1.0), |d| (f(&d, 12), f(&d, 20)))
}

/// Choose what an NPC wields from what it carries, as the game does when nothing
/// was equipped by hand: a melee weapon unless its combat style favours ranged
/// ones (guards carry their bow but wear their sword), the one that scores best in
/// its hands (base damage scaled by the NPC's skill with it), and with a one-handed
/// weapon (or none) its best shield. Staffs, crossbows and unplayable weapons
/// (creature attacks) are left alone.
pub fn equip_weapons(lo: &LoadOrder, inv: &mut Inventory, skill_npc: FormId, style: FormId) {
    let skill = lo.get(skill_npc).map_or([15.0; 4], |r| skills(&r));
    let (melee, ranged) = style_weights(lo, style);
    let archer = ranged > melee;
    let mut best: Option<(bool, f32, FormId, HandType)> = None;
    for &(f, _) in &inv.items {
        let Some(rec) = lo.get(f).filter(|r| r.tag().0 == *b"WEAP") else { continue };
        let hand = HandType::of_weapon(weapon_anim_type(&rec));
        // DNAM flags (offset 12): 0x80 non-playable.
        let unplayable = rec.get(b"DNAM").and_then(|d| d.get(12)).is_some_and(|f| f & 0x80 != 0);
        if unplayable || matches!(hand, HandType::Empty | HandType::Staff | HandType::Crossbow) || weapon_model(lo, f).is_none() {
            continue;
        }
        let damage = rec.get(b"DATA").filter(|d| d.len() >= 10).map_or(0.0, |d| u16::from_le_bytes([d[8], d[9]]) as f32);
        let s = match hand {
            HandType::Greatsword | HandType::Battleaxe => skill[1],
            HandType::Bow => skill[2],
            _ => skill[0],
        };
        let preferred = (hand == HandType::Bow) == archer;
        let score = (damage + 1.0) * (0.5 + s / 100.0);
        log::trace!("{skill_npc} could wield {f} ({hand:?}, damage {damage}, skill {s}, preferred {preferred}): {score:.1}");
        if best.is_none_or(|b| (preferred, score) > (b.0, b.1)) {
            best = Some((preferred, score, f, hand));
        }
    }
    let one_handed = best.is_none_or(|b| b.3.one_handed());
    if let Some((_, _, f, _)) = best {
        inv.equipped.push(f);
    }
    let has_shield = inv.equipped.iter().any(|&f| lo.get(f).is_some_and(|r| r.tag().0 == *b"ARMO" && armor_slots(&r) & SLOT_SHIELD != 0));
    if !one_handed {
        // Two hands on the weapon: a shield from the outfit goes unworn.
        inv.equipped.retain(|&f| !lo.get(f).is_some_and(|r| r.tag().0 == *b"ARMO" && armor_slots(&r) & SLOT_SHIELD != 0));
    } else if !has_shield && skill[3] > 0.0 {
        let rating = |r: &LoadedRecord<'_>| r.get(b"DNAM").filter(|d| d.len() >= 4).map_or(0, |d| i32::from_le_bytes(d[0..4].try_into().unwrap()));
        let shield = inv
            .items
            .iter()
            .filter_map(|&(f, _)| lo.get(f).filter(|r| r.tag().0 == *b"ARMO" && armor_slots(r) & SLOT_SHIELD != 0).map(|r| (rating(&r), f)))
            .max_by_key(|s| s.0);
        if let Some((_, f)) = shield {
            inv.equipped.push(f);
        }
    }
}
