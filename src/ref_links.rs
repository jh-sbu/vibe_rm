//! Placed references by what they link to (`XLKR`) and by the ref types
//! they carry (`XLRT`, and the locations' lists of them), read once over the
//! whole load order. They narrow the references an alias fill tests
//! `IsLinkedTo` and `HasRefType` on; the conditions themselves still decide.

use std::collections::HashMap;

use esp::FormId;

use crate::engine::Engine;

#[derive(Default)]
pub struct RefLinks {
    /// Target -> the references with a linked reference (any keyword) to it.
    linked_from: HashMap<FormId, Vec<FormId>>,
    /// Ref type -> the references that have it.
    by_ref_type: HashMap<FormId, Vec<FormId>>,
}

impl RefLinks {
    fn build(e: &Engine) -> Self {
        use rayon::prelude::*;
        let lo = &e.lo;
        let refs: Vec<FormId> = [b"REFR", b"ACHR"]
            .into_iter()
            .flat_map(|tag| lo.ids_of_type(tag).iter().copied())
            .collect();
        // Read in parallel (878,000 vanilla references), then gathered.
        let read: Vec<(FormId, Vec<FormId>, Option<FormId>)> = refs
            .par_iter()
            .filter_map(|&r| {
                let rec = lo.get(r)?;
                let mut targets = Vec::new();
                let mut ty = None;
                for sr in rec.subrecords() {
                    match &sr.tag.0 {
                        b"XLKR" if sr.data.len() >= 8 => targets.push(rec.fid(sr.form_id(4))),
                        b"XLKR" if sr.data.len() >= 4 => targets.push(rec.fid(sr.form_id(0))),
                        b"XLRT" if sr.data.len() >= 4 => ty = Some(rec.fid(sr.form_id(0))),
                        _ => {}
                    }
                }
                (!targets.is_empty() || ty.is_some()).then_some((r, targets, ty))
            })
            .collect();
        let mut out = RefLinks::default();
        for (r, targets, ty) in read {
            for t in targets {
                let from = out.linked_from.entry(t).or_default();
                if from.last() != Some(&r) {
                    from.push(r);
                }
            }
            if let Some(ty) = ty {
                out.by_ref_type.entry(ty).or_default().push(r);
            }
        }
        for (&r, types) in e.location_ref_types() {
            for &ty in types {
                out.by_ref_type.entry(ty).or_default().push(r);
            }
        }
        for v in out.by_ref_type.values_mut() {
            v.sort_unstable();
            v.dedup();
        }
        out
    }

    /// Plugin references with a linked reference to `target`.
    pub fn linked_from(&self, target: FormId) -> &[FormId] {
        self.linked_from.get(&target).map_or(&[], Vec::as_slice)
    }

    /// Plugin references with ref type `ty`.
    pub fn of_ref_type(&self, ty: FormId) -> &[FormId] {
        self.by_ref_type.get(&ty).map_or(&[], Vec::as_slice)
    }
}

impl Engine {
    pub(crate) fn ref_links(&self) -> &RefLinks {
        self.ref_links.get_or_init(|| RefLinks::build(self))
    }
}
