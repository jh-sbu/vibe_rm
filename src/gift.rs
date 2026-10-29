//! Gifts (`Actor.ShowGiftMenu`): the player giving an actor items, or taking
//! what the actor gives, in the gift menu. The script waits until the menu
//! closes and gets the favor points the gifts earned (`FavorDialogueScript`
//! turns them into Speech XP).
//!
//! What a gift is worth in favor points isn't published: see
//! `known_ambiguities/gifts.md`.

use esp::FormId;
use papyrus::{NativeResult, Value};

use crate::crime::GOLD;
use crate::engine::{Engine, PLAYER_REF};
use crate::items::ListedItem;

/// The gift menu's state while it is open.
#[derive(Debug, Clone)]
pub struct Gift {
    pub actor: FormId,
    /// The player gives (else the actor gives to the player).
    pub giving: bool,
    /// The items that can be given (`FLST` of items or keywords); all when
    /// none.
    pub filter: Option<FormId>,
    /// Stolen items are listed too.
    pub stolen: bool,
    /// Gifts earn favor points.
    pub favor: bool,
    /// The waiting script's key (`Vm::signal_with`).
    key: u64,
    /// The favor points earned so far.
    pub points: i32,
}

impl Engine {
    /// `Actor.ShowGiftMenu(abGivingGift, apFilterList, abShowStolenItems,
    /// abUseFavorPoints)`: opens the gift menu; the script carries on with
    /// the favor points once it closes.
    pub fn show_gift_menu(
        &mut self,
        actor: FormId,
        giving: bool,
        filter: Option<FormId>,
        stolen: bool,
        favor: bool,
    ) -> NativeResult {
        if self.gift.is_some() {
            return NativeResult::Value(Value::Int(0));
        }
        self.messages.next_key += 1;
        let key = 0x4749_4654_0000_0000 | self.messages.next_key;
        log::info!(
            "gift menu: {} {actor} (filter {filter:?}, stolen {stolen}, favor {favor})",
            if giving { "giving to" } else { "taking from" }
        );
        self.gift = Some(Gift {
            actor,
            giving,
            filter,
            stolen,
            favor,
            key,
            points: 0,
        });
        self.menu = Some(crate::items::Menu::Gift(actor));
        NativeResult::WaitFor {
            key,
            timeout: f32::MAX,
            value: Value::Int(0),
        }
    }

    /// Whether a row of the giver's items can be given: the filter's items,
    /// or items with one of its keywords; stolen ones only when allowed.
    /// Never gold.
    pub fn giftable(&self, gift: &Gift, row: &ListedItem) -> bool {
        if row.item == GOLD || (row.owner.is_some() && !gift.stolen) {
            return false;
        }
        let Some(filter) = gift.filter else {
            return true;
        };
        self.formlist(filter)
            .into_iter()
            .any(|f| f == row.item || self.has_keyword(row.item, f))
    }

    /// The gift menu's Give / Take: `count` of the item moves across, and a
    /// gift to the actor earns its value in favor points.
    pub fn give_gift(&mut self, row: &ListedItem, count: i32) {
        let Some(gift) = self.gift.clone() else {
            return;
        };
        let (from, to) = if gift.giving {
            (PLAYER_REF, gift.actor)
        } else {
            (gift.actor, PLAYER_REF)
        };
        let moved =
            self.transfer_item_copy(from, to, row.item, count, Some(row.owner), Some(&row.copy));
        log::info!("gift: {moved} of {} from {from} to {to}", row.item);
        if gift.giving {
            self.unequip_if_gone(PLAYER_REF, row.item);
            if gift.favor
                && let Some(g) = &mut self.gift
            {
                g.points += row.info.value.max(0) * moved;
            }
        }
    }

    /// The gift menu closed (or never opened): the script gets its favor
    /// points.
    pub fn close_gift_menu(&mut self) {
        let Some(gift) = self.gift.take() else {
            return;
        };
        log::info!("gift menu closed: {} favor points", gift.points);
        self.vm.signal_with(gift.key, Value::Int(gift.points));
    }
}
