use super::codec::{Corrupt, Reader, Result, Writer};
use crate::ai::away_practice::Progress;

pub(super) fn put(w: &mut Writer, p: Progress) {
    w.u64(p.seed);
    w.bool(p.magic);
    w.f64(p.elapsed);
    w.f64(p.credit);
    w.f64(p.wait);
    w.bool(p.magic_left.is_some());
    if let Some(left) = p.magic_left {
        w.u32(left);
    }
    w.u32(p.attacks_left);
    w.u32(p.barrages);
    w.bool(p.target.is_some());
    if let Some(target) = p.target {
        w.u32(target);
    }
    w.bool(p.action);
    w.bool(p.finished);
}
pub(super) fn get(r: &mut Reader) -> Result<Progress> {
    let p = Progress {
        seed: r.u64()?,
        magic: r.bool()?,
        elapsed: r.f64()?,
        credit: r.f64()?,
        wait: r.f64()?,
        magic_left: if r.bool()? { Some(r.u32()?) } else { None },
        attacks_left: r.u32()?,
        barrages: r.u32()?,
        target: if r.bool()? { Some(r.u32()?) } else { None },
        action: r.bool()?,
        finished: r.bool()?,
    };
    if p.seed == 0
        || [p.elapsed, p.credit, p.wait]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.0 || *v > f32::MAX as f64)
        || p.target.is_some_and(|i| i >= 3)
    {
        return Err(Corrupt("invalid away practice progress".into()));
    }
    Ok(p)
}
