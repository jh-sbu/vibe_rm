//! Bounded, versioned AEXC procedure progress. Trees are re-read from PACK data.
use super::codec::{Corrupt, Reader, Result, Writer};
use crate::ai::procedure::{Cursor, Path, Progress};

const MAX_NODES: usize = 4096;
const MAX_DEPTH: usize = 64;

fn put_path(w: &mut Writer, path: &[u32]) {
    w.len(path.len());
    for &i in path {
        w.u32(i);
    }
}
fn get_path(r: &mut Reader) -> Result<Path> {
    let n = r.count(4)?;
    if n > MAX_DEPTH {
        return Err(Corrupt("procedure path too deep".into()));
    }
    (0..n).map(|_| r.u32()).collect()
}
fn put_index(w: &mut Writer, v: Option<u32>) {
    w.bool(v.is_some());
    if let Some(i) = v {
        w.u32(i);
    }
}
fn get_index(r: &mut Reader) -> Result<Option<u32>> {
    Ok(if r.bool()? { Some(r.u32()?) } else { None })
}

pub(super) fn put(w: &mut Writer, c: &Cursor) {
    w.str(&c.tree_key);
    w.bool(c.completed);
    w.len(c.states.len());
    for (path, p) in &c.states {
        put_path(w, path);
        w.bool(p.started);
        w.bool(p.finished);
        w.u32(p.next);
        put_index(w, p.selected);
        put_index(w, p.last);
        w.f64(p.elapsed);
        w.u32(p.count);
        w.bool(p.arrived);
        w.bool(p.spoken);
    }
    w.len(c.active.len());
    for path in &c.active {
        put_path(w, path);
    }
}

pub(super) fn get(r: &mut Reader) -> Result<Cursor> {
    let key = r.bytes()?;
    if key.len() > 4096 {
        return Err(Corrupt("procedure tree key too long".into()));
    }
    let tree_key = std::str::from_utf8(key)
        .map_err(|_| Corrupt("procedure tree key isn't UTF-8".into()))?
        .to_owned();
    let completed = r.bool()?;
    let n = r.count(26)?;
    if n > MAX_NODES {
        return Err(Corrupt("too many procedure states".into()));
    }
    let mut states = std::collections::BTreeMap::new();
    let mut previous: Option<Path> = None;
    for _ in 0..n {
        let path = get_path(r)?;
        if previous.as_ref().is_some_and(|p| p >= &path) {
            return Err(Corrupt("unsorted procedure states".into()));
        }
        previous = Some(path.clone());
        let p = Progress {
            started: r.bool()?,
            finished: r.bool()?,
            next: r.u32()?,
            selected: get_index(r)?,
            last: get_index(r)?,
            elapsed: r.f64()?,
            count: r.u32()?,
            arrived: r.bool()?,
            spoken: r.bool()?,
        };
        if !p.elapsed.is_finite() || p.elapsed < 0.0 || (p.finished && !p.started) {
            return Err(Corrupt("invalid procedure progress".into()));
        }
        states.insert(path, p);
    }
    let n = r.count(4)?;
    if n > MAX_NODES {
        return Err(Corrupt("too many active procedures".into()));
    }
    let mut active = Vec::with_capacity(n);
    let mut previous: Option<Path> = None;
    for _ in 0..n {
        let path = get_path(r)?;
        if previous.as_ref().is_some_and(|p| p >= &path)
            || states.get(&path).is_none_or(|p| !p.started || p.finished)
        {
            return Err(Corrupt("invalid active procedure".into()));
        }
        previous = Some(path.clone());
        active.push(path);
    }
    if completed && !active.is_empty() {
        return Err(Corrupt("completed procedure tree still active".into()));
    }
    Ok(Cursor {
        tree_key,
        completed,
        states,
        active,
    })
}
