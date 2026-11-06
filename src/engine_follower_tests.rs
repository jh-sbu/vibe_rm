//! Actual vanilla follower alias scripts, executed through death event delivery.
use super::*;

#[test]
#[ignore = "requires installed Skyrim SE scripts and a headless GPU"]
fn vanilla_follower_alias_death_frees_the_follower_slot_through_its_script() {
    let data = vfs::locate_data_dir().unwrap();
    let names: Vec<_> = esp::BASE_MASTERS.iter().map(|s| s.to_string()).collect();
    let mut e = crate::stats::action_tests::headless_engine_plugins(&data, &names);
    e.vfs = vfs::Vfs::new(&data, &names).into();
    let quest = FormId(0x750BA);
    let follower = FormId(0xA2C94);
    let vmad = crate::script::vmad::parse(&e.lo.get(quest).unwrap()).unwrap();
    let (alias, scripts) = vmad
        .alias_scripts
        .iter()
        .find(|(_, scripts)| {
            scripts
                .iter()
                .any(|s| s.name.eq_ignore_ascii_case("FollowerAliasScript"))
        })
        .unwrap();
    let script = scripts
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case("FollowerAliasScript"))
        .unwrap();
    let property = |name: &str| {
        script
            .properties
            .iter()
            .find_map(|(n, v)| {
                if n.eq_ignore_ascii_case(name) {
                    if let crate::script::vmad::PropValue::Object { form, .. } = v {
                        Some(*form)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap()
    };
    let count = property("PlayerFollowerCount");
    let hireling = property("CurrentHirelingFaction");
    let alias = *alias;
    assert!(e.start_quest(quest));
    e.update_scripts(0.01);
    let arg = e.object_value(follower);
    let mut vm = std::mem::take(&mut e.vm);
    assert!(vm.call_method(
        &mut crate::script::EngineHost { engine: &mut e },
        papyrus::ObjectId::Form(quest.0),
        "DialogueFollowerScript",
        "SetFollower",
        vec![arg]
    ));
    e.vm = vm;
    e.update_scripts(0.01);
    assert_eq!(e.alias_ref(quest, alias), Some(follower));
    assert_eq!(e.global_value(count), 1.);
    e.set_faction_rank(follower, hireling, 0);
    let mut x = crate::ai::exec::ActorExec::new(vec![], Vec3::ZERO, 0., 0.);
    x.place = Some(crate::ai::schedule::Place::Interior(FormId(0x165A4)));
    x.health = 100.;
    e.away.insert(follower, x);
    assert!(e.kill_actor_by(follower, None));
    e.update_scripts(0.01);
    assert_eq!(e.global_value(count), 0.);
    assert_eq!(e.alias_ref(quest, alias), None);
    assert!(!e.is_member(follower, hireling));
    assert!(
        e.is_player_teammate(follower),
        "FollowerAliasScript does not clear this flag on death; do not invent extra dismissal"
    );
}
