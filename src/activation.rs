//! Activate parents (`XAPR`): activating a reference also activates the
//! references naming it as a parent, each after its delay, with the parent as
//! their activator. Pressure plates and trip wires set off trap linkers this
//! way, and linkers their traps. It is part of an activation's default
//! processing, so a reference whose activation is blocked (`BlockActivation`)
//! passes nothing on.

use std::collections::HashMap;

use esp::FormId;

use crate::engine::Engine;

#[derive(Default)]
pub(crate) struct Activation {
    /// Parent -> its children and their delays (seconds), from loaded cells.
    children: HashMap<FormId, Vec<(FormId, f32)>>,
    /// Activations waiting out their delay: when (real time), what, by whom.
    pending: Vec<(f64, FormId, FormId)>,
}

/// A reference's activate parents (`XAPR`: parent, delay; repeated).
fn parents_of(lo: &esp::LoadOrder, r: FormId) -> Vec<(FormId, f32)> {
    let Some(rec) = lo.get(r) else {
        return Vec::new();
    };
    let Some(d) = rec.get(b"XAPR") else {
        return Vec::new();
    };
    d.chunks_exact(8)
        .map(|c| {
            let parent = rec.fid(FormId(u32::from_le_bytes(c[0..4].try_into().unwrap())));
            (parent, f32::from_le_bytes(c[4..8].try_into().unwrap()))
        })
        .collect()
}

/// Whether only a reference's activate parents can activate it ("Parent
/// Activate Only", `XAPD` bit 0): the player can't.
pub(crate) fn parent_activate_only(lo: &esp::LoadOrder, r: FormId) -> bool {
    lo.get(r)
        .and_then(|rec| rec.get(b"XAPD").and_then(|d| d.first().copied()))
        .is_some_and(|f| f & 1 != 0)
}

impl Engine {
    /// Note the activate parents of a loaded cell's references.
    pub(crate) fn load_activate_parents(&mut self, refs: &[FormId]) {
        for &r in refs {
            for (parent, delay) in parents_of(&self.lo, r) {
                let kids = self.activation.children.entry(parent).or_default();
                if !kids.iter().any(|(k, _)| *k == r) {
                    kids.push((r, delay));
                }
            }
        }
    }

    /// `target` activated by `by` (a script's `Activate`, a parent): its
    /// scripts hear `OnActivate` (unless `default_only`), and unless its
    /// activation is blocked its children are activated in turn.
    pub(crate) fn activate_ref(&mut self, target: FormId, by: Option<FormId>, default_only: bool) {
        if !default_only {
            let by = by.map_or(papyrus::Value::None, |b| self.object_value(b));
            self.send_script_event(target, "OnActivate", vec![by]);
        }
        if !self.scripts.blocked_activation.contains(&target) {
            self.activate_children(target);
        }
    }

    /// Activate the references `parent` is an activate parent of, after their
    /// delays.
    pub(crate) fn activate_children(&mut self, parent: FormId) {
        let Some(kids) = self.activation.children.get(&parent).cloned() else {
            return;
        };
        let now = self.scripts.real_time;
        for (child, delay) in kids {
            log::debug!("{parent} activates {child} (after {delay:.1}s)");
            self.activation
                .pending
                .push((now + delay.max(0.0) as f64, child, parent));
        }
    }

    /// Carry out the child activations whose delay is over.
    pub(crate) fn update_activations(&mut self) {
        if self.activation.pending.is_empty() {
            return;
        }
        let now = self.scripts.real_time;
        let (due, wait): (Vec<_>, Vec<_>) = std::mem::take(&mut self.activation.pending)
            .into_iter()
            .partition(|(t, _, _)| *t <= now);
        self.activation.pending = wait;
        for (_, child, parent) in due {
            self.activate_ref(child, Some(parent), false);
        }
    }
}
