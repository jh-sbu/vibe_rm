//! Reference spatial queries. Public sources and unresolved details: research/pex-natives/batch06-spatial.md.
use crate::engine::Engine;
use esp::FormId;
use papyrus::Value;

fn float_at(data: &[u8], offset: usize) -> Option<f32> {
    Some(f32::from_le_bytes(
        data.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn scale(e: &Engine, reference: FormId) -> f32 {
    if let Some(actor) = e.actor_ref(reference) {
        return actor.scale;
    }
    let placed = e.reference_of(reference).map_or(1.0, |r| r.scale);
    let Some(sources) = e.templates_of(reference) else {
        return placed;
    };
    let Some(traits) = e.lo.get(sources.of(crate::world::template::TRAITS)) else {
        return placed;
    };
    let height = traits
        .get(b"NAM6")
        .and_then(|d| float_at(d, 0))
        .unwrap_or(1.0);
    let female = traits
        .get(b"ACBS")
        .and_then(|d| d.first())
        .is_some_and(|b| b & 1 != 0);
    let race_height = e
        .race_of(reference)
        .and_then(|race| e.lo.get(race))
        .and_then(|race| float_at(race.get(b"DATA")?, if female { 20 } else { 16 }))
        .unwrap_or(1.0);
    placed * height * race_height
}

pub(super) fn call(e: &Engine, func: &str, reference: Option<FormId>) -> Value {
    if func == "getworldspace" {
        let world = reference
            .and_then(|r| e.current_place_of(r))
            .and_then(|(p, _)| match p {
                crate::ai::schedule::Place::Exterior(w, _) => Some(w),
                _ => None,
            });
        return world.map_or(Value::None, |w| e.object_value(w));
    }
    if func == "getscale" {
        return Value::Float(reference.map_or(0.0, |r| scale(e, r)));
    }
    let axis = match func {
        "getwidth" => 0,
        "getlength" => 1,
        _ => 2,
    };
    let extent = reference
        .and_then(|r| {
            let base = e.lo.get(e.base_of(r)?)?;
            let data = base.get(b"OBND")?.get(..12)?;
            let coordinate = |offset| i16::from_le_bytes([data[offset], data[offset + 1]]) as f32;
            Some((coordinate(axis * 2 + 6) - coordinate(axis * 2)).max(0.0) * scale(e, r))
        })
        .unwrap_or(0.0);
    Value::Float(extent)
}
