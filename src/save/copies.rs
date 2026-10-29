//! Optional COPY section; legacy INVT layout stays byte-compatible.
use super::codec::{Corrupt, Reader, Result, Writer};
use super::file::SaveFile;
use super::state::Snapshot;
use crate::item_copies::{CopyState, ItemCopy};
use std::collections::{HashMap, HashSet};

fn put_state(w: &mut Writer, s: &CopyState) {
    w.f32(s.quality);
    w.opt_form(s.enchantment);
    w.f32(s.capacity);
    w.f32(s.charge);
    w.bool(s.name.is_some());
    if let Some(n) = &s.name {
        w.str(n);
    }
}
fn get_state(r: &mut Reader) -> Result<CopyState> {
    let s = CopyState {
        quality: r.f32()?,
        enchantment: r.opt_form()?,
        capacity: r.f32()?,
        charge: r.f32()?,
        name: if r.bool()? { Some(r.str()?) } else { None },
    };
    if !s.quality.is_finite()
        || s.quality < 1.
        || !s.capacity.is_finite()
        || s.capacity < 0.
        || !s.charge.is_finite()
        || s.charge < 0.
        || s.charge > s.capacity
        || s.name.as_ref().is_some_and(|n| n.chars().count() > 32)
    {
        return Err(Corrupt("invalid item copy state".into()));
    }
    Ok(s)
}
fn put_copies(w: &mut Writer, copies: &[ItemCopy]) {
    w.len(copies.len());
    for c in copies {
        w.form(c.item);
        w.opt_form(c.owner);
        put_state(w, &c.state);
        w.i32(c.count);
    }
}
fn get_copies(r: &mut Reader) -> Result<Vec<ItemCopy>> {
    let mut copies = Vec::new();
    for _ in 0..r.count(29)? {
        let c = ItemCopy {
            item: r.form()?,
            owner: r.opt_form()?,
            state: get_state(r)?,
            count: r.i32()?,
        };
        if c.item.is_null()
            || c.count <= 0
            || c.state == CopyState::default()
            || copies.iter().any(|old: &ItemCopy| {
                old.item == c.item && old.owner == c.owner && old.state == c.state
            })
        {
            return Err(Corrupt("invalid or duplicate item copy".into()));
        }
        copies.push(c);
    }
    Ok(copies)
}

pub(super) fn write(s: &Snapshot, file: &mut SaveFile) {
    let mut w = Writer::default();
    w.u8(1);
    let mut invs: Vec<_> = s
        .inventories
        .iter()
        .filter(|(_, i)| !i.copies.is_empty() || !i.equipped_copies.is_empty())
        .collect();
    invs.sort_by_key(|(f, _)| **f);
    w.len(invs.len());
    for (&holder, inv) in invs {
        w.form(holder);
        put_copies(&mut w, &inv.copies);
        let mut equipped: Vec<_> = inv
            .equipped_copies
            .iter()
            .filter(|(f, _)| inv.is_equipped(**f))
            .collect();
        equipped.sort_by_key(|(f, _)| **f);
        w.len(equipped.len());
        for (&item, state) in equipped {
            w.form(item);
            put_state(&mut w, state);
        }
    }
    let mut refs: Vec<_> = s.item_copies.iter().collect();
    refs.sort_by_key(|(f, _)| **f);
    w.len(refs.len());
    for (&reference, copies) in refs {
        w.form(reference);
        put_copies(&mut w, copies);
    }
    file.add(b"COPY", w.0);
}

pub(super) fn read(s: &mut Snapshot, file: &SaveFile) -> Result<()> {
    if file.sections.iter().filter(|(t, _)| t == b"COPY").count() > 1 {
        return Err(Corrupt("duplicate COPY section".into()));
    }
    let Some(data) = file.section(b"COPY") else {
        return Ok(());
    };
    let mut r = Reader::new(data);
    if r.u8()? != 1 {
        return Err(Corrupt("unsupported COPY version".into()));
    }
    let mut seen = HashSet::new();
    for _ in 0..r.count(12)? {
        let holder = r.form()?;
        if !seen.insert(holder) {
            return Err(Corrupt("duplicate copy inventory".into()));
        }
        let Some(inv) = s.inventories.get_mut(&holder) else {
            return Err(Corrupt("copy metadata without inventory".into()));
        };
        inv.copies = get_copies(&mut r)?;
        let mut counts = HashMap::new();
        for c in &inv.copies {
            let n = counts.entry((c.item, c.owner)).or_insert(0i64);
            *n += c.count as i64;
            if *n > inv.count_owned(c.item, c.owner) as i64 {
                return Err(Corrupt("copy count exceeds ownership stack".into()));
            }
        }
        for _ in 0..r.count(21)? {
            let item = r.form()?;
            let state = get_state(&mut r)?;
            if !inv.is_equipped(item)
                || inv.equipped_copies.insert(item, state.clone()).is_some()
                || if state == CopyState::default() {
                    inv.items
                        .iter()
                        .find(|(f, _)| *f == item)
                        .map_or(0, |(_, n)| *n)
                        <= inv
                            .copies
                            .iter()
                            .filter(|c| c.item == item)
                            .map(|c| c.count)
                            .sum()
                } else {
                    !inv.copies
                        .iter()
                        .any(|c| c.item == item && c.state == state)
                }
            {
                return Err(Corrupt("invalid equipped copy".into()));
            }
        }
    }
    for _ in 0..r.count(8)? {
        let reference = r.form()?;
        let copies = get_copies(&mut r)?;
        if reference.is_null() || s.item_copies.insert(reference, copies).is_some() {
            return Err(Corrupt("invalid or duplicate reference copies".into()));
        }
    }
    r.finish()
}
