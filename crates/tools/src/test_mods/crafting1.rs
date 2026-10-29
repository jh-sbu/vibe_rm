//! Manual crafting probes. See research/crafting/manual-test.md for console commands and
//! the observations to collect. This generator never installs or runs them.
use super::script::Code;
use super::{TestMod, own};
use anyhow::{Context, Result};
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, vmad, write_plugin};
use esp::{FormId, LoadOrder};
use papyrus::build::{Arg, ScriptBuilder, auto_var, id, string};
use papyrus::pex::op;

const SCRIPT: &str = "VrmCrafting1Script";
const ALIAS: &str = "VrmCrafting1Alias";
const ITEMS: &[(&str, &str)] = &[
    ("IronSword", "IronSword"),
    ("SteelSword", "SteelSword"),
    ("IronArmor", "ArmorIronCuirass"),
    ("SteelArmor", "ArmorSteelCuirassA"),
    ("Iron", "IngotIron"),
    ("Steel", "IngotSteel"),
    ("Leather", "LeatherStrips"),
    ("Wheat", "Wheat"),
    ("Mushroom", "Mushroom06"),
    ("Petty", "SoulGemPettyFilled"),
    ("Lesser", "SoulGemLesserFilled"),
    ("Common", "SoulGemCommonFilled"),
    ("Greater", "SoulGemGreaterFilled"),
    ("Grand", "SoulGemGrandFilled"),
    ("Dagger", "IronDagger"),
    ("SteelPerk", "SteelSmithing"),
    ("Alchemist", "Alchemist80"),
    ("Physician", "Physician"),
    ("Benefactor", "Benefactor"),
    ("Fortify", "FortifySkillSmithing04"),
    ("Disenchant", "EnchIronSwordFire01"),
];
const REPORTS: &[&str] = &[
    "Temper15",
    "Temper40",
    "Temper57",
    "Temper74",
    "Temper100",
    "Steel15",
    "Steel40",
    "Steel57",
    "Steel74",
    "Steel100",
    "Fortify100",
];

fn add(c: &mut Code, property: &str, count: i32) {
    c.method(
        "ObjectReference",
        auto_var("Player"),
        "AddItem",
        [auto_var(property), count.into(), true.into()],
    );
}
fn note(c: &mut Code, text: &str) {
    c.global("Debug", "MessageBox", [string(text)]);
}
fn scripts() -> Result<Vec<(String, Vec<u8>)>> {
    let mut s = ScriptBuilder::new(SCRIPT, "Quest");
    s.auto_property("Player", "Actor");
    s.auto_property("Observer", ALIAS);
    for &(name, _) in ITEMS {
        s.auto_property(name, "Form");
    }
    s.variable("ReportSlot", "Int", 0.into());
    s.variable("Ready", "Bool", false.into());
    s.variable(
        "ResultEnchanting",
        "String",
        string("Item cards: capture magnitude, charge and name before/after reload."),
    );
    for report in REPORTS {
        s.variable(&format!("Result{report}"), "String", string(""));
    }
    let mut init = Code::new("None");
    init.method("Form", id("self"), "RegisterForSingleUpdate", [5f32.into()]);
    s.function("OnInit", init.finish(Arg::None));
    let mut init = Code::new("None");
    let ready = init.label();
    init.jmpf(id("Ready"), ready)
        .ret(Arg::None)
        .bind(ready)
        .assign(id("Ready"), true.into());
    note(
        &mut init,
        "crafting1 ready. Use the commands in research/crafting/manual-test.md. Use a disposable save: the probes change skills, perks and inventory.",
    );
    s.function("OnUpdate", init.finish(Arg::None));
    let mut setup = Code::new("None");
    let skill = setup.param("skill", "Float");
    let steel = setup.param("steel", "Bool");
    let slot = setup.param("slot", "Int");
    setup.assign(id("ReportSlot"), slot);
    setup.method(
        "Actor",
        auto_var("Player"),
        "SetActorValue",
        [string("Smithing"), skill],
    );
    setup.method(
        "Actor",
        auto_var("Player"),
        "RemovePerk",
        [auto_var("SteelPerk")],
    );
    let no_perk = setup.label();
    setup.jmpf(steel, no_perk);
    setup.method(
        "Actor",
        auto_var("Player"),
        "AddPerk",
        [auto_var("SteelPerk")],
    );
    setup.bind(no_perk);
    for name in ["IronSword", "SteelSword", "IronArmor", "SteelArmor"] {
        let count = setup.method(
            "ObjectReference",
            auto_var("Player"),
            "GetItemCount",
            [auto_var(name)],
        );
        setup.method(
            "ObjectReference",
            auto_var("Player"),
            "RemoveItem",
            [auto_var(name), count, true.into(), Arg::None],
        );
        add(&mut setup, name, 1);
    }
    for name in ["Iron", "Steel", "Leather"] {
        add(&mut setup, name, 100);
    }
    note(
        &mut setup,
        "Improve the iron/steel sword at a grindstone and both cuirasses at an armor workbench. Then close the menu and run cqf VrmCrafting1Quest ReadTemper.",
    );
    s.function("SetupTemper", setup.finish(Arg::None));
    for (index, report) in REPORTS.iter().enumerate() {
        let skill = [15., 40., 57., 74., 100.][index.min(9) % 5];
        let mut c = Code::new("None");
        c.own(
            "SetupTemper",
            "None",
            [skill.into(), (index >= 5).into(), (index as i32).into()],
        );
        if index == 10 {
            add(&mut c, "Fortify", 1);
            note(
                &mut c,
                "Drink the supplied Fortify Smithing potion immediately before improving. Record its percentage and whether all four improvements finish before it expires.",
            );
        }
        s.function(report, c.finish(Arg::None));
    }
    let mut read = Code::new("None");
    let text = read.local("report", "String");
    read.assign(
        text.clone(),
        string("GetItemHealthPercent (untouched = 1):\n"),
    );
    for name in ["IronSword", "SteelSword", "IronArmor", "SteelArmor"] {
        let dropped = read.method(
            "ObjectReference",
            auto_var("Player"),
            "DropObject",
            [auto_var(name), 1.into()],
        );
        let health = read.method(
            "ObjectReference",
            dropped.clone(),
            "GetItemHealthPercent",
            [],
        );
        read.append(
            text.clone(),
            [string(name), string(": "), health, string("\n")],
        );
        read.method(
            "ObjectReference",
            dropped.clone(),
            "Disable",
            [false.into()],
        );
        read.method("ObjectReference", dropped, "Delete", []);
    }
    for (index, report) in REPORTS.iter().enumerate() {
        let next = read.label();
        let yes = read.calc(op::CMP_EQ, "Bool", id("ReportSlot"), (index as i32).into());
        read.jmpf(yes, next)
            .assign(id(&format!("Result{report}")), text.clone())
            .bind(next);
    }
    read.global("Debug", "MessageBox", [text]);
    s.function("ReadTemper", read.finish(Arg::None));

    let mut potions = Code::new("None");
    let skill = potions.param("skill", "Float");
    let perks = potions.param("perks", "Bool");
    potions.method(
        "Actor",
        auto_var("Player"),
        "SetActorValue",
        [string("Alchemy"), skill],
    );
    for name in ["Alchemist", "Physician", "Benefactor"] {
        potions.method("Actor", auto_var("Player"), "RemovePerk", [auto_var(name)]);
    }
    let skip = potions.label();
    potions.jmpf(perks, skip);
    for name in ["Alchemist", "Physician", "Benefactor"] {
        potions.method("Actor", auto_var("Player"), "AddPerk", [auto_var(name)]);
    }
    potions.bind(skip);
    add(&mut potions, "Wheat", 20);
    add(&mut potions, "Mushroom", 20);
    note(
        &mut potions,
        "Mix Wheat + Blisterwort once. Screenshot the result name/effects/value. OnItemAdded records its FormID and GetGoldValue on the player alias; names have no vanilla native. Repeat at Alchemy 15/50/100, with and without perks.",
    );
    s.function("Potions", potions.finish(Arg::None));
    let mut enchant = Code::new("None");
    enchant.method(
        "Actor",
        auto_var("Player"),
        "SetActorValue",
        [string("Enchanting"), 100f32.into()],
    );
    for name in ["Petty", "Lesser", "Common", "Greater", "Grand"] {
        add(&mut enchant, name, 4);
    }
    add(&mut enchant, "Dagger", 20);
    add(&mut enchant, "Disenchant", 1);
    note(
        &mut enchant,
        "Disenchant the supplied fire sword. Enchant a fresh iron dagger with each soul size, at both slider ends. Rename one. Screenshot every card and compare after save/reload. Record reusable-gem behavior separately with Azura's Star or the Black Star.",
    );
    s.function("Enchanting", enchant.finish(Arg::None));
    let mut xp = Code::new("None");
    xp.method(
        "Actor",
        auto_var("Player"),
        "SetActorValue",
        [string("Smithing"), 15f32.into()],
    );
    add(&mut xp, "Iron", 100);
    add(&mut xp, "Leather", 100);
    xp.call_method(
        auto_var("Observer"),
        "BeginXP",
        papyrus::build::discard(),
        [],
    );
    note(
        &mut xp,
        "Craft iron daggers until Smithing rises from 15. Remove XP bonuses (stones, sleep, equipment) first. The alias records the number made and base Smithing after each. Stop crafting as soon as it rises; then run Finish.",
    );
    s.function("SmithingXP", xp.finish(Arg::None));
    let mut finish = Code::new("None");
    finish.global("Game", "RequestSave", []);
    note(
        &mut finish,
        "Reports saved. Copy the resulting save into tests/results and run vrm-tool test-results tests/results. Keep item-card screenshots alongside it.",
    );
    s.function("Finish", finish.finish(Arg::None));

    let mut a = ScriptBuilder::new(ALIAS, "ReferenceAlias");
    a.auto_property("Player", "Actor");
    a.auto_property("Dagger", "Form");
    a.variable("Made", "Int", 0.into());
    a.variable("Lines", "Int", 0.into());
    a.variable("TrackXP", "Bool", false.into());
    a.variable(
        "ResultPotions",
        "String",
        string("Created potion FormID / gold value:\n"),
    );
    a.variable(
        "ResultSmithingXP",
        "String",
        string("Daggers / base Smithing:\n"),
    );
    let mut begin = Code::new("None");
    begin
        .assign(id("Made"), 0.into())
        .assign(id("TrackXP"), true.into());
    a.function("BeginXP", begin.finish(Arg::None));
    let mut added = Code::new("None");
    let item = added.param("akBaseItem", "Form");
    let count = added.param("aiItemCount", "Int");
    added.param("akItemReference", "ObjectReference");
    added.param("akSourceContainer", "ObjectReference");
    let potion = added.temp("Potion");
    added.cast(potion.clone(), item.clone());
    let skip = added.label();
    added.jmpf(potion, skip);
    let more = added.calc(op::CMP_LT, "Bool", id("Lines"), 18.into());
    added.jmpf(more, skip);
    let gold = added.method("Form", item.clone(), "GetGoldValue", []);
    let fid = added.method("Form", item.clone(), "GetFormID", []);
    added.append(
        id("ResultPotions"),
        [fid, string(" / "), gold, string("\n")],
    );
    added
        .op(op::IADD, [id("Lines"), id("Lines"), 1.into()])
        .bind(skip);
    let done = added.label();
    added.jmpf(id("TrackXP"), done);
    let matches = added.calc(op::CMP_EQ, "Bool", item, auto_var("Dagger"));
    added.jmpf(matches, done);
    added.op(op::IADD, [id("Made"), id("Made"), count]);
    let skill = added.method(
        "Actor",
        auto_var("Player"),
        "GetBaseActorValue",
        [string("Smithing")],
    );
    let cap = added.calc(op::CMP_LE, "Bool", id("Made"), 18.into());
    let no_line = added.label();
    added.jmpf(cap, no_line);
    added
        .append(
            id("ResultSmithingXP"),
            [id("Made"), string(" / "), skill.clone(), string("\n")],
        )
        .bind(no_line);
    let raised = added.calc(op::CMP_GT, "Bool", skill, 15f32.into());
    added.jmpf(raised, done);
    added.assign(id("TrackXP"), false.into());
    added
        .append(
            id("ResultSmithingXP"),
            [
                string("First increase after "),
                id("Made"),
                string(" daggers.\n"),
            ],
        )
        .bind(done);
    a.function("OnItemAdded", added.finish(Arg::None));
    Ok(vec![
        (SCRIPT.into(), papyrus::pex::write(&s.build()?)?),
        (ALIAS.into(), papyrus::pex::write(&a.build()?)?),
    ])
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let player = FormId(0x14);
    let mut properties = vec![
        ("Player".into(), PropertyValue::Object(player)),
        (
            "Observer".into(),
            PropertyValue::Alias {
                quest: own(0x800),
                alias: 0,
            },
        ),
    ];
    for &(name, edid) in ITEMS {
        properties.push((
            name.into(),
            PropertyValue::Object(
                lo.find_editor_id(edid)
                    .with_context(|| format!("missing crafting probe form {edid}"))?,
            ),
        ));
    }
    let dagger = lo.find_editor_id("IronDagger").unwrap();
    let mut data = vmad(&[AttachedScript {
        name: SCRIPT.into(),
        properties,
    }]);
    data.extend_from_slice(&[2, 0, 0, 0, 0]); // quest fragments: v2, no entries, empty script
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&[0, 0, 0, 0]); // object format 2: padding, alias 0
    data.extend_from_slice(&own(0x800).0.to_le_bytes());
    data.extend_from_slice(&vmad(&[AttachedScript {
        name: ALIAS.into(),
        properties: vec![
            ("Player".into(), PropertyValue::Object(player)),
            ("Dagger".into(), PropertyValue::Object(dagger)),
        ],
    }]));
    let quest = RecordBuilder::new(b"QUST", own(0x800))
        .zstring(b"EDID", "VrmCrafting1Quest")
        .zstring(b"FULL", "Crafting probes")
        .field(b"VMAD", &data)
        .field(b"DNAM", &[0x11, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
        .field(b"NEXT", &[])
        .u32(b"ANAM", 1)
        .u32(b"ALST", 0)
        .zstring(b"ALID", "Player")
        .u32(b"FNAM", 0)
        .form(b"ALFR", player)
        .field(b"ALED", &[]);
    Ok(TestMod {
        plugin: write_plugin(
            0,
            &["Skyrim.esm"],
            "vibe_rm",
            "Manual crafting probes; research/crafting/manual-test.md",
            0x801,
            &[Node::top(b"QUST", vec![quest])],
        ),
        scripts: scripts()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crafting_probe_scripts_have_typed_vanilla_calls_and_bounded_pages() {
        let built = scripts().unwrap();
        for (name, bytes) in &built {
            let pex = papyrus::pex::parse(bytes).unwrap();
            assert!(
                super::super::script::mistyped_calls(&pex).is_empty(),
                "{name}"
            );
        }
        let shown = super::super::script::dry_run::run(built, &[(own(0x800).0, SCRIPT, vec![])], 1);
        assert_eq!(shown.boxes.len(), 1);
        assert!(shown.boxes[0].lines().count() <= 20);
        assert_eq!(shown.saved_at_box, None); // Only Finish requests a save.
    }
}
