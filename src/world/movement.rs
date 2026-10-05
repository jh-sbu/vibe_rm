//! Movement types (`MOVT`): how fast an actor walks, runs and turns in each of its
//! behaviour graph's movement states.

use std::collections::HashMap;

use esp::LoadOrder;

/// Forward speeds (units per second) and turn rates (radians per second) of a
/// movement type (`SPED`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoveSpeeds {
    pub walk: f32,
    pub run: f32,
    /// Turning in place while walking / running, and turning on the move.
    pub turn_walk: f32,
    pub turn_run: f32,
    pub turn_moving: f32,
}

/// Movement types by name (`MNAM`, lowercase): the name a graph's `iState_<name>`
/// variable carries (`NPCDefault`, `DogDefault`...).
pub fn movement_types(lo: &LoadOrder) -> HashMap<String, MoveSpeeds> {
    let mut out = HashMap::new();
    for &id in lo.ids_of_type(b"MOVT") {
        let Some(rec) = lo.get(id) else { continue };
        let Some(name) = rec.get(b"MNAM").map(esp::decode_zstring) else { continue };
        // Left, right, forward and back (walk, run each), then the three turn rates.
        let Some(d) = rec.get(b"SPED").filter(|d| d.len() >= 44) else { continue };
        let f = |i: usize| f32::from_le_bytes(d[i * 4..i * 4 + 4].try_into().unwrap());
        out.insert(name.to_ascii_lowercase(), MoveSpeeds { walk: f(4), run: f(5), turn_walk: f(8), turn_run: f(9), turn_moving: f(10) });
    }
    out
}

/// The movement type names a graph's `iState_<name>` variables offer, with their
/// state values: (default, sneaking), given the variables and `iState`'s default.
/// `character` (the project's character name) settles ties (a cow's graph also
/// knows `iState_DeerDefault`).
pub fn graph_movement_types<'a>(vars: impl Iterator<Item = (&'a str, f32)>, character: &str) -> (Option<(String, f32)>, Option<(String, f32)>) {
    let vars: Vec<(&str, f32)> = vars.collect();
    let state = vars.iter().find(|(n, _)| n.eq_ignore_ascii_case("iState")).map(|v| v.1);
    let named: Vec<(String, f32)> = vars
        .iter()
        .filter_map(|(n, v)| Some((n.get(..7).filter(|p| p.eq_ignore_ascii_case("iState_"))?.len(), n, *v)))
        .map(|(skip, n, v)| (n[skip..].trim().to_ascii_lowercase(), v))
        .collect();
    let character = character.to_ascii_lowercase();
    let score = |n: &str| {
        let mut s = 0;
        if n.contains("default") {
            s += 4;
        }
        if n.contains("swim") || n.contains("sneak") || n.contains("sprint") {
            s -= 8;
        }
        if character.contains(n.trim_end_matches("_mt").trim_end_matches("default").trim_end_matches('_')) {
            s += 2;
        }
        s
    };
    let default = state.and_then(|st| named.iter().filter(|(_, v)| *v == st).max_by_key(|(n, _)| score(n)).cloned());
    let sneak = named.iter().find(|(n, _)| n.contains("sneaking")).cloned();
    (default, sneak)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_default_movement_type() {
        let vars = [("iState", 10.0), ("iState_DeerDefault", 10.0), ("iState_CowDefault", 10.0), ("iState_CowSwimDefault", 11.0)];
        let (d, s) = graph_movement_types(vars.iter().map(|(n, v)| (*n, *v)), "H_CowCharater");
        assert_eq!(d, Some(("cowdefault".into(), 10.0)));
        assert_eq!(s, None);
        let vars = [("iState", 0.0), ("iState_NPCDefault", 0.0), ("iState_NPCSneaking", 2.0), ("iState_NPCSprinting", 1.0)];
        let (d, s) = graph_movement_types(vars.iter().map(|(n, v)| (*n, *v)), "DefaultMale");
        assert_eq!(d, Some(("npcdefault".into(), 0.0)));
        assert_eq!(s, Some(("npcsneaking".into(), 2.0)));
    }
}
