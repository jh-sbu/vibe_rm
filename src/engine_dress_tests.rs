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

/// Exterior cells streamed while their objects' models are parsed in the
/// background load once those are in, drawn as synchronously loaded ones.
fn streamed(background: bool) -> (Engine, usize) {
    let data = vfs::locate_data_dir().expect("installed vanilla Skyrim SE assets");
    let names: Vec<_> = esp::BASE_MASTERS.iter().map(|s| s.to_string()).collect();
    let mut e = crate::stats::action_tests::headless_engine_plugins(&data, &names);
    e.vfs = vfs::Vfs::new(&data, &names).into();
    if background {
        let vfs = e.vfs.clone();
        e.renderer.load_textures_in_background(vfs);
    }
    e.radius = 2;
    let tamriel = FormId(0x3C);
    // Riverwood, cell (5, -12); then seven cells east, queuing all 25 about
    // (12, -12): the nine nearest load at once, the rest when their models
    // are in.
    e.enter_exterior(
        tamriel,
        glam::Vec3::new(5.5 * 4096.0, -11.5 * 4096.0, 0.0),
        0.0,
    )
    .unwrap();
    e.player.position = glam::Vec3::new(12.5 * 4096.0, -11.5 * 4096.0, 0.0);
    let (mut frames, mut slowest) = (0, std::time::Duration::ZERO);
    loop {
        let start = std::time::Instant::now();
        e.dress_waiting_actors();
        e.update_streaming();
        slowest = slowest.max(start.elapsed());
        frames += 1;
        if e.pending_loads.is_empty() {
            break;
        }
        assert!(frames < 10_000, "cells never loaded");
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    eprintln!("background {background}: {frames} frames, slowest {slowest:?}");
    (e, frames)
}

/// Each exterior cell's drawn instances.
fn instances(e: &Engine) -> std::collections::BTreeMap<(i32, i32), usize> {
    e.scene
        .cells
        .iter()
        .filter_map(|(k, rc)| match *k {
            CellKey::Exterior(x, y) => Some(((x, y), rc.instances.len())),
            _ => None,
        })
        .collect()
}

#[test]
#[ignore = "needs Skyrim SE Data and a headless GPU adapter"]
fn streamed_cells_wait_for_their_models_then_load_as_synchronously() {
    let (sync, sync_frames) = streamed(false);
    let (e, frames) = streamed(true);
    eprintln!("synchronous {sync_frames} frames, background {frames}");
    assert!(instances(&sync).keys().any(|&(x, _)| x == 14));
    assert_eq!(instances(&e), instances(&sync));
    assert!(e.pending_models.is_empty());
}
