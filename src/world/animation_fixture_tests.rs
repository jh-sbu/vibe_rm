//! Installed-game assets only, loaded read-only; these are not SE parity tests.
use super::*;

fn project(file: &str) -> (vfs::Vfs, Arc<ProjectRuntime>) {
    let data = vfs::locate_data_dir().expect("install Skyrim SE or set SKYRIM_DATA");
    let plugins = esp::LoadOrder::default_plugin_list(&data, None);
    let vfs = vfs::Vfs::new(data, &plugins);
    let project = GraphLibrary::default()
        .project(&vfs, file)
        .expect("project");
    (vfs, project)
}

#[test]
#[ignore = "requires installed vanilla Skyrim SE assets"]
fn creature_look_at_uses_its_authored_target_binding() {
    for (file, variable) in [
        ("meshes/actors/bear/bearproject.hkx", "bHeadTrackingOn"),
        (
            "meshes/actors/ambient/chicken/chickenproject.hkx",
            "bHeadTracking",
        ),
        ("meshes/actors/dragon/dragonproject.hkx", "bLookAtTarget"),
    ] {
        let (_vfs, project) = project(file);
        let mut graph = GraphAnim::new(project, "", false, &Skeleton::default(), 1);
        assert!(graph.has_look_at);
        graph.aim_at(Some(Vec3::Y * 200.0));
        assert_eq!(graph.variable(variable), Some(1.0), "{file}");
        graph.aim_at(None);
        assert_eq!(graph.variable(variable), Some(0.0), "{file}");
    }
}

#[test]
#[ignore = "requires installed vanilla Skyrim SE assets"]
fn humanoid_swim_state_enters_moves_and_leaves() {
    let (vfs, project) = project("meshes/actors/character/defaultmale.hkx");
    let skeleton = Skeleton::default();
    let mut graph = GraphAnim::new(
        project,
        "meshes/actors/character/character assets/skeleton.nif",
        false,
        &skeleton,
        1,
    );
    let mut animations = AnimationLibrary::default();
    graph.set_variable("IsNPC", 1.0);
    graph.set_variable("i1stPerson", 0.0);
    graph.set_variable("IsFirstPerson", 0.0);
    graph.update(0.1, &vfs, &mut animations, &skeleton);
    assert!(graph.handle_event("SwimStart", &vfs, &mut animations, &skeleton));
    graph.update(0.6, &vfs, &mut animations, &skeleton);
    assert!(graph.active_states().iter().any(|s| s == "Swim_State"));
    assert!(graph.handle_event("moveStart", &vfs, &mut animations, &skeleton));
    graph.update(0.6, &vfs, &mut animations, &skeleton);
    assert!(
        graph
            .active_states()
            .iter()
            .any(|s| s == "SwimLocomotion_3rdP")
    );
    assert!(graph.handle_event("SwimStop", &vfs, &mut animations, &skeleton));
    graph.update(0.6, &vfs, &mut animations, &skeleton);
    assert!(!graph.active_states().iter().any(|s| s == "Swim_State"));
}

/// Vanilla object graphs saved part way through their events and restored
/// into fresh instances go on exactly as the originals do.
#[test]
#[ignore = "requires installed vanilla Skyrim SE assets"]
fn object_graph_snapshots_run_on_like_the_originals() {
    for file in [
        "meshes/dungeons/nordic/levers/ruinslever01/norlever01.hkx",
        "meshes/dungeons/nordic/levers/pullchain/norpullchain01.hkx",
        "meshes/dungeons/nordic/doors/animated/portcullis/norportcullisgate01.hkx",
        "meshes/dungeons/nordic/doors/animated/puzzledoor/puzzledoorbase01.hkx",
        "meshes/dungeons/dwemer/levers/ruinslever01/dwelever01.hkx",
        "meshes/dungeons/dwemer/animated/shafttrap/dweshafttrap01.hkx",
        "meshes/dungeons/imperial/portcullis/portgatepole01.hkx",
        "meshes/traps/riggedrockfall/riggedrockfall01.hkx",
        "meshes/traps/swingingblade/trapbladeswinging01.hkx",
        "meshes/traps/spiketrap/spiketrap01.hkx",
        "meshes/traps/beartrap/beartrap01.hkx",
        "meshes/traps/pressureplate/trapstonepressureplate01.hkx",
        "meshes/clutter/sfarmhousemill/sfarmhousemillproject.hkx",
    ] {
        let (vfs, project) = project(file);
        let skeleton = Skeleton::default();
        let mut anims = AnimationLibrary::default();
        let events: Vec<String> = project
            .shared
            .project
            .graphs
            .iter()
            .flat_map(|(_, g)| g.events.iter().cloned())
            .collect();
        let new = || GraphAnim::new(project.clone(), "", false, &skeleton, 7);
        let mut a = new();
        a.update(0.1, &vfs, &mut anims, &skeleton);
        let mut b = None;
        for (step, event) in events
            .iter()
            .cycle()
            .take(events.len().max(1) * 2)
            .enumerate()
        {
            a.handle_event(event, &vfs, &mut anims, &skeleton);
            a.update(0.37, &vfs, &mut anims, &skeleton);
            if step == events.len() / 2 {
                let mut fresh = new();
                fresh.update(0.1, &vfs, &mut anims, &skeleton);
                assert!(fresh.restore(&a.saved()), "{file}");
                b = Some(fresh);
            }
            if let Some(b) = b.as_mut().filter(|_| step > events.len() / 2) {
                b.handle_event(event, &vfs, &mut anims, &skeleton);
                b.update(0.37, &vfs, &mut anims, &skeleton);
                assert_eq!(a.active_states(), b.active_states(), "{file} {event}");
                assert_eq!(a.saved(), b.saved(), "{file} {event}");
            }
        }
        assert!(b.is_some(), "{file}");
    }
}
