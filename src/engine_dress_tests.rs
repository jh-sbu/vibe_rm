//! Actors spawned while their models are parsed in the background: drawn
//! with nothing until every model is in, then dressed as a synchronous spawn
//! would have them. Reads installed assets in place, never copies them.

use super::*;
use crate::render::CellKey;

const CELL: FormId = FormId(0x133C9);

fn engine(background: bool) -> Engine {
    let data = vfs::locate_data_dir().expect("installed vanilla Skyrim SE assets");
    let names: Vec<_> = esp::BASE_MASTERS.iter().map(|s| s.to_string()).collect();
    let mut e = crate::stats::action_tests::headless_engine_plugins(&data, &names);
    e.vfs = vfs::Vfs::new(&data, &names).into();
    if background {
        let vfs = e.vfs.clone();
        e.renderer.load_textures_in_background(vfs);
    }
    e.enter_interior(CELL, None).unwrap();
    e
}

/// Each spawned actor's drawn meshes, by reference.
fn meshes(e: &Engine) -> std::collections::BTreeMap<FormId, usize> {
    let key = CellKey::Interior(CELL);
    e.cells[&key]
        .actors
        .iter()
        .zip(&e.scene.cells[&key].actors)
        .map(|(a, inst)| (a.ref_id, inst.meshes.len()))
        .collect()
}

#[test]
#[ignore = "needs Skyrim SE Data and a headless GPU adapter"]
fn actors_waiting_for_models_are_dressed_like_a_synchronous_spawn() {
    let expected = meshes(&engine(false));
    assert!(expected.values().any(|&n| n > 0), "{expected:?}");
    let mut e = engine(true);
    let waiting: Vec<FormId> = e.undressed.keys().copied().collect();
    assert!(!waiting.is_empty(), "nothing was parsed in the background");
    for (actor, n) in meshes(&e) {
        if waiting.contains(&actor) {
            assert_eq!(n, 0, "{actor} drawn before its models are in");
        }
    }
    let vfs = e.vfs.clone();
    e.models.finish(&mut e.renderer, &vfs);
    e.dress_waiting_actors();
    assert!(e.undressed.is_empty());
    assert_eq!(meshes(&e), expected);
}
