//! Quest alias packages (`ALPC`): an actor filling a reference alias of a running
//! quest runs that alias's packages ahead of its own, those of higher priority
//! quests first, at the same priority the alias filled last first (`test-mod
//! autotest4`). Their conditions, alias locations and alias targets are read in
//! the package's owner quest (`QNAM`), else the alias's quest.

use std::collections::HashMap;
use std::sync::Arc;

use esp::FormId;

use super::package::{self, Package};
use crate::engine::Engine;

/// Who fills which aliases, rebuilt when quests or alias fills change, and each
/// alias's packages.
#[derive(Default)]
pub struct AliasPackages {
    /// `ScriptState::alias_gen` the index was built at.
    built: Option<u64>,
    /// Actor -> (quest priority, when filled, quest, alias) it fills in
    /// running quests.
    fills: HashMap<FormId, Vec<(u8, u64, FormId, u32)>>,
    packages: HashMap<(FormId, u32), Arc<Vec<Package>>>,
}

/// The packages (`ALPC`) of each reference alias of a quest.
fn quest_alias_packages(quest: &esp::LoadedRecord<'_>) -> HashMap<u32, Vec<FormId>> {
    let mut out: HashMap<u32, Vec<FormId>> = HashMap::new();
    let mut alias = None;
    for sr in quest.subrecords() {
        match &sr.tag.0 {
            b"ALST" => alias = Some(sr.u32(0)),
            b"ALLS" | b"ALED" => alias = None,
            b"ALPC" => {
                if let Some(a) = alias {
                    out.entry(a).or_default().push(quest.fid(sr.form_id(0)));
                }
            }
            _ => {}
        }
    }
    out
}

impl Engine {
    /// The packages of the aliases `achr` fills in running quests, highest
    /// priority quest first.
    pub(crate) fn alias_packages(&mut self, achr: FormId) -> Vec<Package> {
        if self.alias_packs.built != Some(self.scripts.alias_gen) {
            let mut fills: HashMap<FormId, Vec<(u8, u64, FormId, u32)>> = HashMap::new();
            for (&q, st) in self
                .scripts
                .quests
                .iter()
                .filter(|(_, st)| st.running && !st.aliases.is_empty())
            {
                let priority = self
                    .lo
                    .get(q)
                    .and_then(|r| r.get(b"DNAM").and_then(|d| d.get(2).copied()))
                    .unwrap_or(0);
                for (&alias, &r) in &st.aliases {
                    let at = st.filled_at.get(&alias).copied().unwrap_or(0);
                    fills.entry(r).or_default().push((priority, at, q, alias));
                }
            }
            for v in fills.values_mut() {
                // Higher priority first; at the same, the latest filled (in
                // the game, of two quests' aliases forced to one actor the
                // second's package ran, either way round, and of one quest's
                // two the second forced).
                v.sort_by(|a, b| {
                    b.0.cmp(&a.0)
                        .then(b.1.cmp(&a.1))
                        .then(a.2.cmp(&b.2))
                        .then(a.3.cmp(&b.3))
                });
            }
            log::debug!("{} references fill aliases of running quests", fills.len());
            self.alias_packs.fills = fills;
            self.alias_packs.built = Some(self.scripts.alias_gen);
        }
        let Some(fills) = self.alias_packs.fills.get(&achr).cloned() else {
            return Vec::new();
        };
        log::trace!("{achr} fills {fills:?}");
        let mut out = Vec::new();
        for (_, _, q, alias) in fills {
            let packs = match self.alias_packs.packages.get(&(q, alias)) {
                Some(p) => p.clone(),
                None => {
                    let ids = self
                        .lo
                        .get(q)
                        .map(|r| quest_alias_packages(&r))
                        .and_then(|mut m| m.remove(&alias))
                        .unwrap_or_default();
                    let p: Arc<Vec<Package>> = Arc::new(
                        ids.into_iter()
                            .flat_map(|id| package::expand(&self.lo, id))
                            .map(|mut p| {
                                p.quest = p.quest.or(Some(q));
                                p
                            })
                            .collect(),
                    );
                    self.alias_packs.packages.insert((q, alias), p.clone());
                    p
                }
            };
            out.extend(packs.iter().cloned());
        }
        log::trace!(
            "{achr} alias packages {:?}",
            out.iter().map(|p| p.editor_id.as_str()).collect::<Vec<_>>()
        );
        out
    }

    /// An actor's whole package stack: its scene's packages, its aliases',
    /// then its own.
    pub(crate) fn actor_packages(&mut self, achr: FormId, npc: FormId) -> Vec<Package> {
        let mut out = self.scene_packages(achr);
        out.extend(self.alias_packages(achr));
        out.extend(self.npc_packages_cached(achr, npc).iter().cloned());
        out
    }

    /// Loaded actors whose aliases changed take up their new package stacks,
    /// keeping their current package if it is still on it.
    pub(crate) fn refresh_alias_packages(&mut self) {
        let generation = self.scripts.alias_gen;
        let stale: Vec<(FormId, FormId)> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| a.exec.alias_gen != generation)
            .map(|a| (a.ref_id, a.npc))
            .collect();
        for (r, npc) in stale {
            let packages = self.actor_packages(r, npc);
            let Some(a) = self.actor_mut(r) else { continue };
            a.exec.alias_gen = generation;
            if a.exec
                .packages
                .iter()
                .map(|p| p.id)
                .eq(packages.iter().map(|p| p.id))
            {
                continue;
            }
            log::debug!(
                "{r} packages now {:?}",
                packages
                    .iter()
                    .map(|p| p.editor_id.as_str())
                    .collect::<Vec<_>>()
            );
            let current = a
                .exec
                .current
                .and_then(|i| a.exec.packages.get(i))
                .map(|p| p.id);
            a.exec.current = current.and_then(|id| packages.iter().position(|p| p.id == id));
            a.exec.packages = packages;
            // Choose again now.
            a.exec.next_eval = 0.0;
        }
    }
}
