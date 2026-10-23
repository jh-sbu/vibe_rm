//! World-map travel. Public evidence and native policies: `known_ambiguities/fast-travel.md`.

#[cfg(test)]
mod tests {
    use crate::engine::PLAYER_REF;
    use papyrus::{NativeResult, Value};

    #[test]
    #[ignore = "requires a headless GPU; synthetic records only"]
    fn fast_travel_script_gate_and_scripted_player_moves() {
        let dir = tempfile_dir("gate");
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Travel.esp");
        let enabled = |e: &mut crate::engine::Engine| match crate::script::natives::call(
            e,
            "game",
            "isfasttravelenabled",
            None,
            &[],
        ) {
            NativeResult::Value(Value::Bool(b)) => b,
            _ => panic!("boolean expected"),
        };
        assert!(enabled(&mut e));
        crate::script::natives::call(
            &mut e,
            "game",
            "enablefasttravel",
            None,
            &[Value::Bool(false)],
        );
        assert!(!enabled(&mut e));
        crate::script::natives::call(&mut e, "game", "enablefasttravel", None, &[]);
        assert!(enabled(&mut e));
        e.fast_travel_disabled = true;
        let player = e.object_value(PLAYER_REF);
        crate::script::natives::call(
            &mut e,
            "objectreference",
            "setposition",
            Some(&player),
            &[Value::Float(1.0), Value::Float(2.0), Value::Float(3.0)],
        );
        assert!(enabled(&mut e));
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn tempfile_dir(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("vrm-travel-{label}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("Travel.esp"),
            esp::writer::write_plugin(0, &[], "test", "synthetic travel", 0x900, &[]),
        )
        .unwrap();
        dir
    }
}
