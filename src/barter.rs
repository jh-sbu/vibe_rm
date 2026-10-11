//! Bartering: what a merchant sells (the merchant chest of their vendor
//! faction), what they buy, and at what price.
//!
//! A vendor faction (`FACT` flag 0x4000) names the merchant chest (`VENC`),
//! the hours and whether stolen and non-stolen goods are bought (`VENV`), and a
//! buy / sell list of keywords (`VEND`, a form list; `VENV` can turn it into
//! the list of what is *not* traded). Prices follow UESP's Speech page: a
//! factor of `fBarterMax - (fBarterMax - fBarterMin) x min(Speech, 100) / 100`
//! the player buys at and sells at the inverse of, the perks' Mod Buy / Sell
//! Prices entry points applied, buying never under 1.05 x the value and
//! selling never over it. Open questions: `known_gaps/barter.md`.

use esp::FormId;
use esp::actor_value as av;

use crate::crime::GOLD;
use crate::engine::{Engine, PLAYER_REF};
use crate::perks::ep;

/// A merchant's vendor faction.
#[derive(Debug, Clone)]
pub struct Vendor {
    /// The merchant chest (`VENC`): what is sold, and the merchant's gold.
    pub chest: Option<FormId>,
    pub buys_stolen: bool,
    pub buys_nonstolen: bool,
    /// The keyword list is what isn't bought and sold.
    pub inverted: bool,
    /// The buy / sell list (`VEND`): keywords.
    pub list: Vec<FormId>,
}

impl Engine {
    /// The merchant's vendor faction, if they are one.
    pub fn vendor_of(&self, actor: FormId) -> Option<Vendor> {
        self.npc_factions(actor).into_iter().find_map(|(f, _)| {
            let r = self.lo.get(f)?;
            let flags = r
                .get(b"DATA")
                .filter(|d| d.len() >= 4)
                .map_or(0, |d| u32::from_le_bytes(d[0..4].try_into().unwrap()));
            if flags & 0x4000 == 0 {
                return None;
            }
            let form = |tag: &[u8; 4]| {
                r.get(tag)
                    .filter(|d| d.len() >= 4)
                    .map(|d| r.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
                    .filter(|f| !f.is_null())
            };
            // VENV: start and end hour, radius, then buys stolen, inverted
            // list, only buys stolen (a general merchant has 0 there).
            let venv = r.get(b"VENV").unwrap_or(&[]);
            let byte = |i: usize, default: bool| venv.get(i).map_or(default, |&b| b != 0);
            Some(Vendor {
                chest: form(b"VENC"),
                buys_stolen: byte(8, false),
                inverted: byte(9, false),
                buys_nonstolen: !byte(10, false),
                list: form(b"VEND").map(|l| self.formlist(l)).unwrap_or_default(),
            })
        })
    }

    /// Where a merchant's wares and gold are: their merchant chest, else what
    /// they carry.
    pub fn vendor_stock(&self, vendor: FormId) -> FormId {
        self.vendor_of(vendor)
            .and_then(|v| v.chest)
            .unwrap_or(vendor)
    }

    /// Whether a merchant trades in an item (stolen: belonging to someone).
    pub fn vendor_trades(&self, vendor: &Vendor, item: FormId, stolen: bool) -> bool {
        if item == GOLD || (stolen && !vendor.buys_stolen) || (!stolen && !vendor.buys_nonstolen) {
            return false;
        }
        if vendor.list.is_empty() {
            return !vendor.inverted;
        }
        let listed = vendor.list.iter().any(|&kw| self.has_keyword(item, kw));
        listed != vendor.inverted
    }

    /// What the player pays and gets for an item worth 1 with this merchant:
    /// (buy, sell).
    pub fn barter_multipliers(&self, vendor: FormId) -> (f32, f32) {
        let g = |name, default| crate::ai::combat::gmst_f32(&self.lo, name, default);
        let (min, max) = (g("fBarterMin", 2.0), g("fBarterMax", 3.3));
        let speech = self.actor_value(PLAYER_REF, av::SPEECHCRAFT).min(100.0);
        let factor = max - (max - min) * speech / 100.0;
        // Tabs: the player, the item (none: one rate for the menu), the
        // merchant.
        let subjects = [None, Some(vendor)];
        let buy = self.perk_entry_point(ep::MOD_BUY_PRICES, PLAYER_REF, &subjects, 1.0);
        let sell = self.perk_entry_point(ep::MOD_SELL_PRICES, PLAYER_REF, &subjects, 1.0);
        ((factor * buy).max(1.05), (sell / factor).min(1.0))
    }

    /// Gold a reference holds.
    pub fn gold(&self, r: FormId) -> i32 {
        self.item_count(r, GOLD)
    }

    /// The player buys (`buying`) or sells `count` of an item (the stack of
    /// `owner`'s) at `price` gold each. Buying needs the gold; a merchant short
    /// of gold pays what they have. The gold traded trains Speech.
    pub fn barter(
        &mut self,
        vendor: FormId,
        item: FormId,
        owner: Option<FormId>,
        count: i32,
        price: i32,
        buying: bool,
    ) -> Result<(), String> {
        let stock = self.vendor_stock(vendor);
        let total = price.max(0) * count.max(1);
        let (from, to) = if buying {
            if self.gold(PLAYER_REF) < total {
                return Err(self
                    .gmst_string("sItemTooExpensive")
                    .unwrap_or_else(|| "You don't have enough gold.".into()));
            }
            (stock, PLAYER_REF)
        } else {
            (PLAYER_REF, stock)
        };
        let moved: i32 = self
            .remove_stack(from, item, count, Some(owner), Some(to), None)
            .iter()
            .map(|(_, n)| n)
            .sum();
        if moved <= 0 {
            return Ok(());
        }
        let paid = (price * moved).min(self.gold(to));
        if paid > 0 {
            self.remove_stack(to, GOLD, paid, None, Some(from), None);
        }
        // A player add item event: bought (2), from the merchant.
        if buying {
            self.send_player_add_item_as(item, Some(vendor), stock, 2);
        }
        log::info!(
            "{} {moved} x {item} for {paid} gold with {vendor}",
            if buying { "bought" } else { "sold" }
        );
        self.use_skill(av::SPEECHCRAFT, paid as f32);
        self.increment_stat("Barters", 1);
        Ok(())
    }
}
