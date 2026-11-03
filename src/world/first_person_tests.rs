use super::*;

#[test]
#[ignore = "requires installed Skyrim SE records and assets"]
fn first_person_models_use_skin_armor_and_weapon_record_pairs() {
    let data = vfs::locate_data_dir().unwrap();
    let names = esp::LoadOrder::default_plugin_list(&data, None);
    let lo = LoadOrder::load(&data, &names).unwrap();
    let vfs = vfs::Vfs::new(&data, &names);
    let race = |name| lo.find_editor_id(name).unwrap();
    for (name, female, expected) in [
        ("ImperialRace", false, "1stpersonmalehands"),
        ("ArgonianRace", true, "1stpersonfemalehandsargonian"),
    ] {
        let race = race(name);
        let (_, skin, _) = race_info(&lo, race, female).unwrap();
        let models = armor_models(&lo, skin, race, female, Viewpoint::First);
        assert!(
            models.iter().any(|(m, _)| m.contains(expected)),
            "{models:?}"
        );
        assert!(models.iter().all(|(m, _)| vfs.read(m).is_some()));
    }
    let armor = lo.find_editor_id("ArmorIronBandedCuirass").unwrap();
    let models = armor_models(&lo, armor, race("ImperialRace"), false, Viewpoint::First);
    assert!(
        models.iter().any(|(m, _)| m.contains("1stperson")),
        "{models:?}"
    );
    let weapon = lo.find_editor_id("IronSword").unwrap();
    let r = lo.get(weapon).unwrap();
    let stat = lo.get(fid_at(&r, r.get(b"WNAM").unwrap())).unwrap();
    let model = weapon_first_person_model(&lo, weapon).unwrap();
    assert_eq!(Some(model.clone()), records::model_path(&stat));
    assert!(vfs.read(&model).is_some());
}
