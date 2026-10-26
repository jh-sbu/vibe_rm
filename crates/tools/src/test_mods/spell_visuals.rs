//! Manual visual save/load probe. No Papyrus query exposes an active membrane.
//! Play once, save, reload that save, compare; the persisted Ran guard prevents
//! this quest from starting the shader again. Builds references to vanilla
//! records, and exports no vanilla meshes, textures or scripts.
//!
//! Build `vrm-tools` first, then generate with:
//! `target/debug/vrm-tool test-mod spell-visuals "<Skyrim SE>/Data" tests/assets/VrmSpellVisuals.esp`.
//! The PEX is emitted beside the plugin in `tests/assets/Scripts/`. Do not install
//! or run the game here; the user performs this probe on a disposable SE save.
//!
//! | Checkpoint | Predicted outcome (not an observed SE result) |
//! | --- | --- |
//! | First update | Oakflesh sheen appears; one indefinite Play, then RequestSave. |
//! | Reload: SE persists shaders | Sheen remains without replay; contradicts our transient choice. |
//! | Reload: SE discards shaders | Sheen absent without replay; supports our transient choice. |
//! | `cqf VrmSpellVisualsQuest StopVisual` | Remaining sheen fades/stops; no replay. |
//!
//! Missing initial sheen invalidates the comparison. Reload the requested numbered
//! save without calling Play again. Human observation is required; save inspection
//! cannot report membrane visibility. See `plans/spell-visuals.md` for the protocol.
use super::{TestMod, records, script::Code};
use anyhow::{Context, Result};
use esp::{
    FormId, LoadOrder,
    writer::{AttachedScript, Node, PropertyValue, write_plugin},
};
use papyrus::build::{Arg, ScriptBuilder, auto_var, id, string};
const SCRIPT: &str = "VrmSpellVisualsProbe";
fn script() -> Result<Vec<u8>> {
    let mut s = ScriptBuilder::new(SCRIPT, "Quest");
    s.auto_property("Player", "Actor");
    s.auto_property("Shader", "EffectShader");
    s.variable("Ran", "Bool", false.into());
    let mut init = Code::new("None");
    init.method(
        "Form",
        id("self"),
        "RegisterForSingleUpdate",
        [3.0f32.into()],
    );
    s.function("OnInit", init.finish(Arg::None));
    let mut run = Code::new("None");
    let go = run.label();
    run.jmpf(id("Ran"), go)
        .ret(Arg::None)
        .bind(go)
        .assign(id("Ran"), true.into());
    run.global("Game", "ForceThirdPerson", []);
    run.method(
        "EffectShader",
        auto_var("Shader"),
        "Play",
        [auto_var("Player"), (-1.0f32).into()],
    );
    run.global("Debug","Notification",[string("Spell visual probe: note Oakflesh sheen, then reload the new numbered save. Do not run Play again.")]);
    run.wait(3.0);
    run.global("Game", "RequestSave", []);
    s.function("OnUpdate", run.finish(Arg::None));
    let mut stop = Code::new("None");
    stop.method(
        "EffectShader",
        auto_var("Shader"),
        "Stop",
        [auto_var("Player")],
    );
    s.function("StopVisual", stop.finish(Arg::None));
    Ok(papyrus::pex::write(&s.build()?)?)
}
pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let shader = lo
        .find_editor_id("MagicArmorOakFleshFXS")
        .context("vanilla Oakflesh shader missing")?;
    let quest = records::Quest {
        object_id: 0x800,
        edid: "VrmSpellVisualsQuest",
        flags: 0x11,
        priority: 0,
        kind: 0,
        script: Some(AttachedScript {
            name: SCRIPT.into(),
            properties: vec![
                ("Player".into(), PropertyValue::Object(FormId(0x14))),
                ("Shader".into(), PropertyValue::Object(shader)),
            ],
        }),
        stages: &[],
        aliases: Vec::new(),
    }
    .record();
    Ok(TestMod {
        plugin: write_plugin(
            0,
            &["Skyrim.esm"],
            "vibe_rm",
            "Spell shader persistence: manual screenshot comparison after reload",
            0x801,
            &[Node::top(b"QUST", vec![quest])],
        ),
        scripts: vec![(SCRIPT.into(), script()?)],
    })
}
#[cfg(test)]
mod tests {
    #[test]
    fn probe_script_uses_declared_native_signatures() {
        let bytes = super::script().unwrap();
        let p = papyrus::pex::parse(&bytes).unwrap();
        assert!(super::super::script::mistyped_calls(&p).is_empty());
    }
}
