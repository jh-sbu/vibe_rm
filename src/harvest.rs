//! Harvesting: plants (`FLOR`, and `TREE` with something to give) give their
//! produce (`PFIG`: an ingredient, an item or a leveled list) when activated,
//! with their harvest sound (`SNAM`); the
//! prompt is their own verb (`RNAM`: "Harvest", "Catch"...). A harvested
//! plant shows its harvested model (its switch nodes on their second child)
//! and grows back after `iHoursToRespawnCell`. Taking from someone else's
//! is stealing. Choices: `known_ambiguities/harvest.md`.

use std::borrow::Cow;
use std::collections::HashMap;

use esp::FormId;

use crate::engine::{Engine, PLAYER_REF};

/// What a plant gives.
#[derive(Debug, Clone)]
pub struct Flora {
    pub produce: FormId,
    pub sound: Option<FormId>,
    /// `RNAM`: the prompt's verb.
    pub verb: Option<String>,
}

#[derive(Default)]
pub struct Harvests {
    /// Plants harvested, with the game hour they were.
    pub harvested: HashMap<FormId, f64>,
}

impl Engine {
    /// A plant's produce, if the base is one (`FLOR` or `TREE` with `PFIG`).
    pub fn flora(&self, base: FormId) -> Option<Flora> {
        let rec = self.lo.get(base)?;
        if !matches!(&rec.tag().0, b"FLOR" | b"TREE") {
            return None;
        }
        let form = |d: &[u8]| rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap())));
        let produce = form(rec.get(b"PFIG").filter(|d| d.len() >= 4)?);
        if produce.is_null() {
            return None;
        }
        let sound = rec
            .get(b"SNAM")
            .filter(|d| d.len() >= 4)
            .map(form)
            .filter(|s| !s.is_null());
        let verb = rec
            .get(b"RNAM")
            .map(|d| self.lo.lstring(&rec, d))
            .filter(|v| !v.is_empty());
        Some(Flora {
            produce,
            sound,
            verb,
        })
    }

    /// Whether a plant has been harvested and hasn't grown back.
    pub fn is_harvested(&self, r: FormId) -> bool {
        let Some(&at) = self.harvests.harvested.get(&r) else {
            return false;
        };
        let regrow = crate::ai::combat::gmst_i32(&self.lo, "iHoursToRespawnCell", 240) as f64;
        self.game_hours_total() - at < regrow
    }

    /// The prompt's verb for a plant.
    pub(crate) fn flora_verb(&self, base: FormId) -> Option<Cow<'static, str>> {
        let f = self.flora(base)?;
        Some(match f.verb {
            Some(v) => Cow::Owned(v),
            None => Cow::Owned(
                self.gmst_string("sHarvest")
                    .unwrap_or_else(|| "Harvest".into()),
            ),
        })
    }

    /// The player harvests a plant: its produce is theirs ("Blue Mountain
    /// Flower added"), with the harvest sound. Returns whether it was a plant
    /// to harvest.
    pub(crate) fn harvest(&mut self, r: FormId, base: FormId) -> bool {
        let Some(f) = self.flora(base) else {
            return false;
        };
        if self.is_harvested(r) {
            return true;
        }
        self.harvests.harvested.insert(r, self.game_hours_total());
        self.show_harvested(r, true);
        let at = self.ref_position(r).unwrap_or(self.player.position);
        if let Some(s) = f.sound {
            self.play_sound(s, at);
        }
        let before: Vec<(FormId, i32)> = self.inventory_mut(PLAYER_REF).items.clone();
        self.add_item(PLAYER_REF, f.produce, 1);
        // What the produce (a leveled list, perhaps) came to.
        let after = self.inventory_mut(PLAYER_REF).items.clone();
        for (item, n) in after {
            let had = before.iter().find(|b| b.0 == item).map_or(0, |b| b.1);
            if n <= had {
                continue;
            }
            let got = n - had;
            let info = crate::world::inventory::item_info(&self.lo, item);
            let name = info.map(|i| i.name).unwrap_or_default();
            self.scripts.notify(if got > 1 {
                format!("{name} ({got}) added")
            } else {
                format!("{name} added")
            });
            if self.lo.tag_of(item).is_some_and(|t| t.0 == *b"INGR") {
                self.increment_stat("Ingredients Harvested", 1);
            }
            // Someone else's: stolen, a crime if seen (the plant's owner).
            let stolen = self.stolen_from(item, r, false);
            if let Some(owner) = stolen {
                let n = self
                    .inventory_mut(PLAYER_REF)
                    .remove_split(item, got, Some(None));
                let n: i32 = n.iter().map(|(_, k)| k).sum();
                self.inventory_mut(PLAYER_REF)
                    .add_owned(item, Some(owner), n);
                self.increment_stat("Items Stolen", got);
            }
            self.send_player_add_item(item, got, r, false);
        }
        log::info!("harvested {r} ({base})");
        true
    }

    /// Swaps a loaded plant's model for its harvested (or grown) look.
    pub(crate) fn show_harvested(&mut self, r: FormId, harvested: bool) {
        let Some(base) = self.base_of(r) else {
            return;
        };
        let Some(model) = self
            .lo
            .get(base)
            .and_then(|rec| crate::world::records::model_path(&rec))
        else {
            return;
        };
        let path = if harvested {
            format!("{model}#harvested")
        } else {
            model
        };
        self.models
            .load_all(&mut self.renderer, &self.vfs, std::slice::from_ref(&path));
        let Some(m) = self.models.get(&path) else {
            return;
        };
        for rc in self.scene.cells.values_mut() {
            for inst in rc.instances.iter_mut().filter(|i| i.ref_id == r.0) {
                inst.model = m.clone();
            }
        }
    }

    /// Plants grown back: shown whole again.
    pub(crate) fn update_harvests(&mut self) {
        let grown: Vec<FormId> = self
            .harvests
            .harvested
            .keys()
            .copied()
            .filter(|&r| !self.is_harvested(r))
            .collect();
        for r in grown {
            self.harvests.harvested.remove(&r);
            self.show_harvested(r, false);
        }
    }
}
