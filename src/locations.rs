//! Locations (`LCTN`): which location each reference belongs to in the editor,
//! the location ref types (`LCRT`) references carry there, and where actors are
//! now. Read from the locations' reference lists (UESP *Mod File Format/LCTN*:
//! `ACPR` / `LCPR` persistent references, `ACUN` / `LCUN` unique actors,
//! `ACSR` / `LCSR` static references with their ref types), the references' own
//! `XLCN` / `XLRT`, then their cells' `XLCN`.

use std::collections::HashMap;

use esp::FormId;

use crate::engine::{Engine, PLAYER_REF};
use crate::render::CellKey;
use crate::world::records;

#[derive(Default)]
pub struct LocationIndex {
    /// Reference -> its editor location, from the locations' lists.
    editor: HashMap<FormId, FormId>,
    /// Location -> its static references with their ref types (in record order).
    typed: HashMap<FormId, Vec<(FormId, FormId)>>,
    /// Reference -> the ref types it has.
    ref_types: HashMap<FormId, Vec<FormId>>,
    /// Location -> the locations whose parent (`PNAM`) it is.
    children: HashMap<FormId, Vec<FormId>>,
}

fn u32_at(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}

impl LocationIndex {
    fn build(lo: &esp::LoadOrder) -> Self {
        let mut ix = LocationIndex::default();
        for &l in lo.ids_of_type(b"LCTN") {
            let Some(rec) = lo.get(l) else { continue };
            let f = |v: u32| rec.fid(FormId(v));
            for sr in rec.subrecords() {
                let d = sr.data;
                match &sr.tag.0 {
                    b"ACPR" | b"LCPR" => {
                        for c in d.chunks_exact(12) {
                            ix.editor.entry(f(u32_at(c, 0))).or_insert(l);
                        }
                    }
                    // Unique actor, its reference, its editor location.
                    b"ACUN" | b"LCUN" => {
                        for c in d.chunks_exact(12) {
                            let at = f(u32_at(c, 8));
                            ix.editor.entry(f(u32_at(c, 4))).or_insert(if at.is_null() { l } else { at });
                        }
                    }
                    b"ACSR" | b"LCSR" => {
                        for c in d.chunks_exact(16) {
                            let (ty, r) = (f(u32_at(c, 0)), f(u32_at(c, 4)));
                            ix.editor.entry(r).or_insert(l);
                            ix.typed.entry(l).or_default().push((ty, r));
                            let types = ix.ref_types.entry(r).or_default();
                            if !types.contains(&ty) {
                                types.push(ty);
                            }
                        }
                    }
                    b"PNAM" if d.len() >= 4 => ix.children.entry(f(u32_at(d, 0))).or_default().push(l),
                    _ => {}
                }
            }
        }
        // References carrying their ref type (`XLRT`) belong to their own location
        // (`XLCN`) or their cell's.
        for tag in [b"ACHR", b"REFR"] {
            for &r in lo.ids_of_type(tag) {
                let Some(rec) = lo.get(r) else { continue };
                let Some(d) = rec.get(b"XLRT").filter(|d| d.len() >= 4) else { continue };
                let ty = rec.fid(FormId(u32_at(d, 0)));
                let own = rec.get(b"XLCN").filter(|d| d.len() >= 4).map(|d| rec.fid(FormId(u32_at(d, 0))));
                let pos = records::reference(&rec).position;
                drop(rec);
                let Some(l) = own.or_else(|| ref_cell_location(lo, r, pos)) else { continue };
                ix.editor.entry(r).or_insert(l);
                ix.typed.entry(l).or_default().push((ty, r));
                let types = ix.ref_types.entry(r).or_default();
                if !types.contains(&ty) {
                    types.push(ty);
                }
            }
        }
        log::debug!("location index: {} references placed, {} with ref types", ix.editor.len(), ix.ref_types.len());
        ix
    }
}

/// The location (`XLCN`) of the cell a reference is in: for worldspace-persistent
/// references, the exterior cell under them.
fn ref_cell_location(lo: &esp::LoadOrder, r: FormId, pos: glam::Vec3) -> Option<FormId> {
    let cell = lo.cell_of_ref(r)?;
    let idx = lo.cell(cell)?;
    let cell = match (idx.world, idx.grid) {
        (Some(w), None) => *lo.world(w)?.cells.get(&crate::engine::grid_of(pos.truncate()))?,
        _ => cell,
    };
    let rec = lo.get(cell)?;
    let d = rec.get(b"XLCN").filter(|d| d.len() >= 4)?;
    Some(rec.fid(FormId(u32_at(d, 0)))).filter(|f| !f.is_null())
}

impl Engine {
    pub(crate) fn locations(&self) -> &LocationIndex {
        self.location_index.get_or_init(|| LocationIndex::build(&self.lo))
    }

    /// A form field of a record.
    fn form_field(&self, r: FormId, tag: &[u8; 4]) -> Option<FormId> {
        let rec = self.lo.get(r)?;
        let d = rec.get(tag).filter(|d| d.len() >= 4)?;
        Some(rec.fid(FormId(u32_at(d, 0)))).filter(|f| !f.is_null())
    }

    /// The location (`XLCN`) of a cell.
    pub fn cell_location(&self, cell: FormId) -> Option<FormId> {
        self.form_field(cell, b"XLCN")
    }

    /// The location of the exterior cell at a grid square.
    fn grid_location(&self, world: FormId, grid: (i32, i32)) -> Option<FormId> {
        self.lo.world(world).and_then(|w| w.cells.get(&grid).copied()).and_then(|c| self.cell_location(c))
    }

    /// The location a reference belongs to in the editor.
    pub fn editor_location(&self, r: FormId) -> Option<FormId> {
        if r == PLAYER_REF {
            return self.current_location();
        }
        if let Some(c) = self.created(r) {
            return c.location;
        }
        if let Some(&l) = self.locations().editor.get(&r) {
            return Some(l);
        }
        if let Some(l) = self.form_field(r, b"XLCN") {
            return Some(l);
        }
        ref_cell_location(&self.lo, r, records::reference(&self.lo.get(r)?).position)
    }

    /// The location a reference is in now: the player's, a loaded actor's cell's,
    /// a persistent actor's scheduled place's, else its editor location.
    pub fn ref_current_location(&self, r: FormId) -> Option<FormId> {
        if r == PLAYER_REF {
            return self.current_location();
        }
        if let Some(key) = self.actor_cells.get(&r) {
            return match (*key, self.location) {
                (CellKey::Interior(c), _) => self.cell_location(c),
                (CellKey::Exterior(x, y), crate::engine::Location::Exterior { world, .. }) => self.grid_location(world, (x, y)),
                _ => None,
            };
        }
        if let Some((place, _)) = self.whereabouts.of.get(&r) {
            return match *place {
                crate::ai::schedule::Place::Interior(c) => self.cell_location(c),
                crate::ai::schedule::Place::Exterior(w, g) => self.grid_location(w, g),
            };
        }
        self.editor_location(r)
    }

    /// The location ref types of a reference.
    pub fn ref_types(&self, r: FormId) -> Vec<FormId> {
        let mut out = self.locations().ref_types.get(&r).cloned().unwrap_or_default();
        if let Some(t) = self.form_field(r, b"XLRT")
            && !out.contains(&t)
        {
            out.push(t);
        }
        out
    }

    /// References of ref type `ty` in location `l` or the locations within it
    /// (its own first, then its children's, depth first).
    pub fn location_refs_of_type(&self, l: FormId, ty: FormId) -> Vec<FormId> {
        let ix = self.locations();
        let mut out = Vec::new();
        let mut stack = vec![l];
        let mut seen = 0;
        while let Some(l) = stack.pop() {
            seen += 1;
            if seen > 4096 {
                break;
            }
            out.extend(ix.typed.get(&l).into_iter().flatten().filter(|(t, _)| *t == ty).map(|(_, r)| *r));
            if let Some(c) = ix.children.get(&l) {
                stack.extend(c.iter().rev());
            }
        }
        out
    }

    /// The first of `l` and its parents with keyword `kw` (`l` itself for none).
    pub fn location_with_keyword(&self, mut l: FormId, kw: FormId) -> Option<FormId> {
        if kw.is_null() {
            return Some(l);
        }
        for _ in 0..16 {
            if self.has_keyword(l, kw) {
                return Some(l);
            }
            l = self.form_field(l, b"PNAM")?;
        }
        None
    }

    /// A reference marking where a location is: its world location marker
    /// (`MNAM`), else its first static reference, else a child's.
    pub fn location_marker(&self, l: FormId) -> Option<FormId> {
        if let Some(m) = self.form_field(l, b"MNAM") {
            return Some(m);
        }
        let ix = self.locations();
        if let Some(&(_, r)) = ix.typed.get(&l).and_then(|v| v.first()) {
            return Some(r);
        }
        ix.children.get(&l).into_iter().flatten().find_map(|&c| self.form_field(c, b"MNAM"))
    }

    pub fn is_location(&self, f: FormId) -> bool {
        self.lo.tag_of(f).is_some_and(|t| t.0 == *b"LCTN")
    }
}
