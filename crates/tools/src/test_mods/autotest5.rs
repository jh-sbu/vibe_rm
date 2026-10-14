//! `test-mod autotest5`: `autotest4`'s follow-ups, the tests that didn't
//! work as built there, rebuilt from what its run showed, and the open ends
//! of `taper`. As the other autotests: loading a save with the plugin for
//! the first time starts `VrmAutoTest5Quest`, whose script runs the tests,
//! keeps each report in a `Result<Name>` variable, asks the game to save and
//! shows the reports; `vrm-tool test-results` reads them.
//!
//! Load the usual save (outdoors near Riverwood, without the test plugins)
//! and leave the game alone for about six minutes. The run changes the save
//! for good only by the perks and spells it adds and takes back.
//!
//! What went wrong in `autotest4`, and what is done instead:
//!
//! - The potions' fortify skill effects (`AlchFortifyOneHanded`) raise the
//!   skill's damage modifier (actor value 135), not the skill, so every
//!   fortify read 5 before and after. **AVPercent**, **Recheck** and
//!   **HasShout** now put their conditions on an aimed frost spell (frost
//!   has no taper) and read the health lost; **Ability** copies the potion
//!   effect onto `OneHanded` itself.
//! - The player's spells cast by script didn't reach the target:
//!   **MagnitudePerk** and **PerkPriority** use target actor delivery (no
//!   projectile), with an NPC's cast first as a check.
//! - Held (`SetRestrained`) NPCs neither fought nor regenerated:
//!   **CombatRegen** compares a free NPC, one held by `SetDontMove`, a
//!   restrained one and one fighting (held by `SetDontMove`), and reads the
//!   rates.
//! - **FriendlyHits** logs the struck one's health: did the casts land?
//! - **Scenes**: both phase tests ended 0.32 s into the poll, which can't
//!   tell a phase completing on its conditions from one never starting. Now
//!   on a global (the quest stage can't go back), with `IsPlaying` read
//!   before the global is set, and a scene without conditions to compare.
//! - **AreaRadius**: the NPCs stood in a line, each behind the last, and
//!   their distances were read after the blast had moved them. Now spread
//!   around the centre, read before; a pair one behind the other on purpose.
//! - **Traps**: our scripted bowl sent nothing. Now the bowl's script also
//!   logs `OnLoad`, the bowl's place after the fall is read, and a copy of
//!   the game's falling rock (an activator, `PhysicsTrapHit` in the game)
//!   with our script is dropped too, made dynamic as `PhysicsTrap` does.
//! - **Taper**: four copied effects never landed. Two carry a condition
//!   (`GetLevel` of the player >= 30, the traps' level variants); dropped
//!   here. All are cast with target actor delivery, the actors'
//!   regeneration rates set to 0, so absorbs' gain and dual effects' second
//!   values can be read.
//!
//! Not yet run in the game.

use anyhow::Result;
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, write_plugin};
use esp::{FormId, LoadOrder};
use papyrus::build::{Arg, FunctionBuilder, ScriptBuilder, auto_var, id, string};
use papyrus::pex::op;

use super::autotest2::{Npc, Vanilla, line, place_fn, put, remove};
use super::autotest4::{
    TRAP_PARAMS, cast, fresh, health, now, num_of, poll, put_num, ref_alias, relay, report,
    restrain, self_ref, set_av,
};
use super::records::{
    self, Alias, Effect, Fill, Quest, Section, alias_flags, ctda, func, reaction, run_on, spell,
};
use super::script::{Code, num, on_update};
use super::taper::timeline;
use super::{TestMod, own};

// Object ids: the quest, factions, NPCs, effects, spells, perks, the scene
// quest and its parts, the traps, then the taper copies.
const QUEST: u32 = 0x800;
const VICTIMS: u32 = 0x801;
const ENEMIES: u32 = 0x802;
const GROUP: u32 = 0x803;

const DUMMY: u32 = 0x808;
const CASTER: u32 = 0x809;
const VICTIM: u32 = 0x80A;
const ENEMY: u32 = 0x80B;
const GROUP_MEMBER: u32 = 0x80C;
const SCENE_ACTOR: u32 = 0x80D;

const FIRE_TA: u32 = 0x810;
const FROST_TA: u32 = 0x811;
const FORTIFY_AB: u32 = 0x812;
const FORTIFY_FF: u32 = 0x813;

const AV_PERCENT: u32 = 0x820;
const RECHECK: u32 = 0x821;
const HAS_SHOUT: u32 = 0x822;
const ABILITY: u32 = 0x823;
const FORTIFY: u32 = 0x824;
const PLAYER_FIRE: u32 = 0x825;
const PLAYER_FROST: u32 = 0x826;
const BOLT: u32 = 0x827;

const PERK_ADD: u32 = 0x830;
const PERK_MUL: u32 = 0x831;
const PERK_ADD_LATE: u32 = 0x832;
const PERK_MUL_EARLY: u32 = 0x833;
const PERK_ABILITY: u32 = 0x834;

const SCENE_QUEST: u32 = 0x838;
const SCENE_GLOBAL: u32 = 0x839;
const SCENE_CONTROL: u32 = 0x83A;
const SCENE_COMPLETE: u32 = 0x83B;
const SCENE_SKIP: u32 = 0x83C;

const TRAP_BOWL: u32 = 0x840;
const TRAP_ROCK: u32 = 0x841;

/// The taper copies' effects, then their spells (one each, by index).
const TAPER_EFFECTS: u32 = 0x850;
const TAPER_SPELLS: u32 = 0x860;
const NEXT_OBJECT_ID: u32 = 0x870;

const SCRIPT: &str = "VrmAutoTest5Script";
const TRAP_SCRIPT: &str = "VrmAutoTest5TrapScript";
const PLAYER_REF: FormId = FormId(0x14);

/// Mod Spell Magnitude's entry point.
const MOD_SPELL_MAGNITUDE: u8 = 29;
/// `OneHanded`'s actor value index.
const ONE_HANDED: u32 = 6;
/// `SetMotionType`'s dynamic.
const MOTION_DYNAMIC: i32 = 1;

/// Vanilla forms the scripts use: property, type, editor id.
const VANILLA: &[(&str, &str, &str)] = &[
    ("AugmentedFlames", "Perk", "AugmentedFlames"),
    ("Fireball", "Spell", "Fireball"),
    ("UnrelentingForce", "Shout", "UnrelentingForceShout"),
];

/// Our own forms the scripts use: property, type, object id.
const OWN: &[(&str, &str, u32)] = &[
    ("Dummy", "ActorBase", DUMMY),
    ("Caster", "ActorBase", CASTER),
    ("Victim", "ActorBase", VICTIM),
    ("Enemy", "ActorBase", ENEMY),
    ("GroupMember", "ActorBase", GROUP_MEMBER),
    ("SceneActor", "ActorBase", SCENE_ACTOR),
    ("AvPercent", "Spell", AV_PERCENT),
    ("Recheck", "Spell", RECHECK),
    ("HasShoutSpell", "Spell", HAS_SHOUT),
    ("Ability", "Spell", ABILITY),
    ("Fortify", "Spell", FORTIFY),
    ("PlayerFire", "Spell", PLAYER_FIRE),
    ("PlayerFrost", "Spell", PLAYER_FROST),
    ("Bolt", "Spell", BOLT),
    ("PerkAdd", "Perk", PERK_ADD),
    ("PerkMul", "Perk", PERK_MUL),
    ("PerkAddLate", "Perk", PERK_ADD_LATE),
    ("PerkMulEarly", "Perk", PERK_MUL_EARLY),
    ("PerkAbility", "Perk", PERK_ABILITY),
    ("SceneQuest", "Quest", SCENE_QUEST),
    ("SceneGlobal", "GlobalVariable", SCENE_GLOBAL),
    ("SceneControl", "Scene", SCENE_CONTROL),
    ("SceneComplete", "Scene", SCENE_COMPLETE),
    ("SceneSkip", "Scene", SCENE_SKIP),
    ("TrapBowl", "MiscObject", TRAP_BOWL),
    ("TrapRock", "Form", TRAP_ROCK),
];

const TESTS: &[(&str, &str)] = &[
    ("AVPercent", "GetActorValuePercent's scale"),
    ("Recheck", "Conditions looked at again"),
    ("HasShout", "HasShout"),
    ("Ability", "Fortify effects"),
    ("MagnitudePerk", "Augmented Flames"),
    ("PerkPriority", "Perk priority"),
    ("CombatRegen", "Health coming back"),
    ("FriendlyHits", "Friendly hits"),
    ("Scenes", "Scene phases"),
    ("AreaRadius", "Fireball's area"),
    ("Traps", "Trap hits"),
    ("Taper", "Taper's open ends"),
];

/// A taper case: name, the vanilla effect, the magnitude, the value watched,
/// the second value read, whether the caster's gain is read (its value
/// first lowered by 500), whether the effect's conditions are dropped,
/// label.
struct Case {
    name: &'static str,
    effect: &'static str,
    magnitude: f32,
    av: &'static str,
    second: &'static str,
    caster: bool,
    drop_conditions: bool,
    label: &'static str,
}

const fn case(
    name: &'static str,
    effect: &'static str,
    av: &'static str,
    second: &'static str,
    label: &'static str,
) -> Case {
    Case {
        name,
        effect,
        magnitude: 50.0,
        av,
        second,
        caster: false,
        drop_conditions: false,
        label,
    }
}

const CASES: &[Case] = &[
    case(
        "FireTA",
        "FireDamageFFAimed",
        "Health",
        "Stamina",
        "fire (0.3, 2, 1 s; 4.875 by aimed delivery)",
    ),
    Case {
        caster: true,
        ..case(
            "AbsorbMagicka",
            "MG07StaffEffect",
            "Magicka",
            "Stamina",
            "absorb magicka (1, 0, 1 s)",
        )
    },
    Case {
        caster: true,
        ..case(
            "AbsorbHealth",
            "DraugrMagicAxeStreak",
            "Health",
            "Stamina",
            "absorb health (0.2, 2, 1 s)",
        )
    },
    case(
        "StormBolt",
        "ShockDamageBoltStormAimed",
        "Health",
        "Magicka",
        "Storm Call's bolt (1, 1, 1 s; health and magicka)",
    ),
    case(
        "Frost",
        "FrostDamageFFAimed",
        "Health",
        "Stamina",
        "frost (no taper; health and stamina)",
    ),
    case(
        "Shock",
        "ShockDamageFFAimed",
        "Health",
        "Magicka",
        "shock (0.01, 0, 0.1 s; health and half to magicka)",
    ),
    Case {
        drop_conditions: true,
        ..case(
            "RuneShock",
            "TrapRuneLightningFFLocation02",
            "Health",
            "Magicka",
            "lightning rune (0.3, 2, 3 s; health and magicka)",
        )
    },
    case(
        "Swarm",
        "SprigganBugDamageConcAimed",
        "Health",
        "Stamina",
        "poison (0.5, 2, 3 s)",
    ),
    Case {
        drop_conditions: true,
        ..case(
            "TrapFire",
            "TrapExplGasEnchFireDamageFFContact06",
            "Health",
            "Stamina",
            "fire (0.2, 2, 1 s)",
        )
    },
];

/// The regeneration rates set to 0 where only spells may change values.
const RATES: &[&str] = &["HealRate", "MagickaRate", "StaminaRate"];

/// Where the area test's NPCs stand: the angle from the centre (degrees, 0
/// away from the caster), the distance, label.
const RING: &[(f32, f32, &str)] = &[
    (0.0, 150.0, "150 units off"),
    (60.0, 250.0, "250 units off"),
    (90.0, 300.0, "300 units off"),
    (120.0, 320.0, "320 units off"),
    (240.0, 340.0, "340 units off"),
    (270.0, 360.0, "360 units off"),
    (300.0, 400.0, "400 units off"),
    (30.0, 100.0, "100 units off, in front of the next"),
    (30.0, 220.0, "220 units off, behind the last"),
];

fn as_text(c: &mut Code, f: Arg) -> Arg {
    let s = c.temp("String");
    c.cast(s.clone(), f);
    s
}

fn damage(c: &mut Code, a: &Arg, av: &str, amount: f32) {
    c.method(
        "Actor",
        a.clone(),
        "DamageActorValue",
        [string(av), amount.into()],
    );
}

fn restore(c: &mut Code, a: &Arg, av: &str, amount: f32) {
    c.method(
        "Actor",
        a.clone(),
        "RestoreActorValue",
        [string(av), amount.into()],
    );
}

fn dont_move(c: &mut Code, a: &Arg) {
    c.method("Actor", a.clone(), "SetDontMove", [true.into()]);
}

/// A held caster (150 units out) and a held target (450) of `hp` health,
/// neither regenerating.
fn pair(c: &mut Code, hp: f32) -> (Arg, Arg) {
    let t = fresh(c, "t", "Dummy", 450.0, 0.0);
    let caster = fresh(c, "caster", "Caster", 150.0, 0.0);
    set_av(c, &t, "Health", hp);
    for a in [&t, &caster] {
        for rate in RATES {
            set_av(c, a, rate, 0.0);
        }
        restrain(c, a);
    }
    c.wait(1.5);
    (caster, t)
}

/// Health lost by `t` in the 3 s after `caster` casts `spell` at it, as a
/// line.
fn lost_line(c: &mut Code, r: &Arg, spell: &str, caster: &Arg, t: &Arg, label: &str) {
    let before = health(c, t);
    let before_v = c.temp("Float");
    c.assign(before_v.clone(), before);
    cast(c, spell, caster, t);
    c.wait(3.0);
    let after = health(c, t);
    let lost = c.calc(op::FSUB, "Float", before_v, after);
    put_num(c, r, &format!("{label}: health lost "), lost);
    line(c, r, "\n");
}

fn remove_all(c: &mut Code, actors: &[&Arg]) {
    for a in actors {
        c.method("Actor", (*a).clone(), "StopCombat", []);
        remove(c, a);
    }
}

fn test_av_percent() -> FunctionBuilder {
    let (mut c, r) = report();
    line(
        &mut c,
        &r,
        "an aimed frost spell (no taper): 7 if the target's GetActorValuePercent(Health) \
         >= 0.5, 13 if >= 50 (20 lost at full health and 7 at 40% if the scale is 0..100; \
         7 and 0 if 0..1):\n",
    );
    let (caster, t) = pair(&mut c, 500.0);
    lost_line(&mut c, &r, "AvPercent", &caster, &t, "  at full health");
    restore(&mut c, &t, "Health", 1000.0);
    damage(&mut c, &t, "Health", 300.0);
    c.wait(0.5);
    let pct = c.method(
        "Actor",
        t.clone(),
        "GetActorValuePercentage",
        [string("Health")],
    );
    put_num(
        &mut c,
        &r,
        "  health down to (GetActorValuePercentage) ",
        pct,
    );
    line(&mut c, &r, "\n");
    lost_line(&mut c, &r, "AvPercent", &caster, &t, "  then");
    remove_all(&mut c, &[&caster, &t]);
    c.finish(r)
}

fn test_recheck() -> FunctionBuilder {
    let (mut c, r) = report();
    line(
        &mut c,
        &r,
        "an aimed frost spell, 20 a second for 30 s while the target's GetActorValue(Health) \
         >= 960, cast at 1000 health; health lost every 0.25 s (200 restored after 6 s):",
    );
    let (caster, t) = pair(&mut c, 1000.0);
    cast(&mut c, "Recheck", &caster, &t);
    let i = c.local("i", "Int");
    c.assign(i.clone(), 0.into());
    let (top, done) = (c.label(), c.label());
    c.bind(top);
    let h = health(&mut c, &t);
    let lost = c.calc(op::FSUB, "Float", 1000.0f32.into(), h);
    put_num(&mut c, &r, " ", lost);
    let enough = c.calc(op::CMP_GE, "Bool", i.clone(), 48.into());
    c.jmpt(enough, done);
    let at_6 = c.calc(op::CMP_EQ, "Bool", i.clone(), 24.into());
    let skip = c.label();
    c.jmpf(at_6, skip);
    restore(&mut c, &t, "Health", 200.0);
    line(&mut c, &r, " (restored)");
    c.bind(skip);
    c.op(op::IADD, [i.clone(), i.clone(), 1.into()]);
    c.wait(0.25);
    c.jmp(top);
    c.bind(done);
    line(&mut c, &r, "\n");
    remove_all(&mut c, &[&caster, &t]);
    c.finish(r)
}

fn test_has_shout() -> FunctionBuilder {
    let (mut c, r) = report();
    line(
        &mut c,
        &r,
        "an aimed frost spell, 10 if the target HasShout(Unrelenting Force):\n",
    );
    let (caster, t) = pair(&mut c, 500.0);
    lost_line(&mut c, &r, "HasShoutSpell", &caster, &t, "  without it");
    c.method(
        "Actor",
        t.clone(),
        "AddShout",
        [auto_var("UnrelentingForce")],
    );
    c.wait(0.5);
    lost_line(
        &mut c,
        &r,
        "HasShoutSpell",
        &caster,
        &t,
        "  after AddShout(Unrelenting Force)",
    );
    remove_all(&mut c, &[&caster, &t]);
    c.finish(r)
}

fn test_ability() -> FunctionBuilder {
    let (mut c, r) = report();
    let player = auto_var("Player");
    let state = |c: &mut Code, r: &Arg, label: &str| {
        let one = c.method(
            "Actor",
            player.clone(),
            "GetActorValue",
            [string("OneHanded")],
        );
        let has = c.method("Actor", player.clone(), "HasSpell", [auto_var("Ability")]);
        put_num(c, r, &format!("{label}: the player's OneHanded "), one);
        put(c, r, ", has the ability ", has);
        line(c, r, "\n");
    };
    line(
        &mut c,
        &r,
        "the potions' fortify one-handed effect copied onto OneHanded itself:\n",
    );
    state(&mut c, &r, "  before");
    c.method(
        "Actor",
        player.clone(),
        "AddSpell",
        [auto_var("Ability"), false.into()],
    );
    c.wait(1.0);
    state(&mut c, &r, "  AddSpell(our ability, +7)");
    c.method(
        "Actor",
        player.clone(),
        "RemoveSpell",
        [auto_var("Ability")],
    );
    c.wait(1.0);
    state(&mut c, &r, "  RemoveSpell");
    cast(&mut c, "Fortify", &player, &player);
    c.wait(1.5);
    state(&mut c, &r, "  a self spell, +10 for 60 s, cast");
    c.method(
        "Actor",
        player.clone(),
        "DispelSpell",
        [auto_var("Fortify")],
    );
    c.wait(1.0);
    state(&mut c, &r, "  dispelled");
    c.method(
        "Actor",
        player.clone(),
        "AddPerk",
        [auto_var("PerkAbility")],
    );
    c.wait(1.0);
    state(&mut c, &r, "  a perk with the ability as its section added");
    c.method(
        "Actor",
        player.clone(),
        "RemovePerk",
        [auto_var("PerkAbility")],
    );
    c.wait(1.0);
    state(&mut c, &r, "  removed");
    c.finish(r)
}

fn test_magnitude_perk() -> FunctionBuilder {
    let (mut c, r) = report();
    let (caster, t) = pair(&mut c, 5000.0);
    let player = auto_var("Player");
    line(
        &mut c,
        &r,
        "a 50 point fire spell, target actor delivery (4.875 taper at 50):\n",
    );
    lost_line(&mut c, &r, "PlayerFire", &caster, &t, "  an NPC's");
    lost_line(&mut c, &r, "PlayerFire", &player, &t, "  the player's");
    c.method(
        "Actor",
        player.clone(),
        "AddPerk",
        [auto_var("AugmentedFlames")],
    );
    lost_line(
        &mut c,
        &r,
        "PlayerFire",
        &player,
        &t,
        "  the player's with Augmented Flames",
    );
    c.method("Actor", player, "RemovePerk", [auto_var("AugmentedFlames")]);
    remove_all(&mut c, &[&caster, &t]);
    c.finish(r)
}

fn test_perk_priority() -> FunctionBuilder {
    let (mut c, r) = report();
    let (caster, t) = pair(&mut c, 5000.0);
    let player = auto_var("Player");
    line(
        &mut c,
        &r,
        "a 50 point frost spell (no taper), target actor delivery:\n",
    );
    lost_line(&mut c, &r, "PlayerFrost", &caster, &t, "  an NPC's");
    lost_line(&mut c, &r, "PlayerFrost", &player, &t, "  the player's");
    for (perks, label) in [
        (
            ["PerkAdd", "PerkMul"],
            "  Mod Spell Magnitude +10 at priority 0, x2 at 1 (60 x 2 = 120 or 50 x 2 + 10 = 110)",
        ),
        (
            ["PerkAddLate", "PerkMulEarly"],
            "  +10 at priority 1, x2 at 0",
        ),
    ] {
        for p in perks {
            c.method("Actor", player.clone(), "AddPerk", [auto_var(p)]);
        }
        lost_line(&mut c, &r, "PlayerFrost", &player, &t, label);
        for p in perks {
            c.method("Actor", player.clone(), "RemovePerk", [auto_var(p)]);
        }
    }
    remove_all(&mut c, &[&caster, &t]);
    c.finish(r)
}

fn test_combat_regen() -> FunctionBuilder {
    let (mut c, r) = report();
    let free = fresh(&mut c, "free", "Dummy", 300.0, -200.0);
    let held = fresh(&mut c, "held", "Dummy", 300.0, 0.0);
    let restrained = fresh(&mut c, "restrained", "Dummy", 300.0, 200.0);
    let fighting = fresh(&mut c, "fighting", "Victim", 300.0, 400.0);
    let e = fresh(&mut c, "e", "Enemy", 300.0, 1900.0);
    dont_move(&mut c, &held);
    restrain(&mut c, &restrained);
    dont_move(&mut c, &fighting);
    dont_move(&mut c, &e);
    let four = [&free, &held, &restrained, &fighting];
    for a in four {
        set_av(&mut c, a, "Health", 300.0);
    }
    c.wait(1.0);
    for av in ["HealRate", "CombatHealthRegenMult"] {
        let v = c.method("Actor", free.clone(), "GetActorValue", [string(av)]);
        put_num(&mut c, &r, &format!("{av} "), v);
        line(&mut c, &r, ", ");
    }
    line(
        &mut c,
        &r,
        "four NPCs at 90 of 300 health: free, held by SetDontMove, restrained, and held by \
         SetDontMove fighting an enemy 1500 units off:\n",
    );
    for a in four {
        damage(&mut c, a, "Health", 210.0);
    }
    c.method("Actor", fighting.clone(), "StartCombat", [e.clone()]);
    let sample = |c: &mut Code, r: &Arg, label: &str| {
        put(c, r, &format!("  {label}:"), string(""));
        for a in four {
            let h = health(c, a);
            put_num(c, r, " ", h);
        }
        let state = c.method("Actor", fighting.clone(), "GetCombatState", []);
        put(c, r, " (combat state ", state);
        line(c, r, ")\n");
    };
    sample(&mut c, &r, "0 s");
    for t in [5, 10, 15, 20] {
        c.wait(5.0);
        sample(&mut c, &r, &format!("{t} s"));
    }
    c.method("Actor", fighting.clone(), "StopCombat", []);
    c.method("Actor", e.clone(), "StopCombat", []);
    remove(&mut c, &e);
    for t in [5, 10] {
        c.wait(5.0);
        sample(&mut c, &r, &format!("StopCombat, {t} s"));
    }
    remove_all(&mut c, &four);
    c.finish(r)
}

fn test_friendly_hits() -> FunctionBuilder {
    let (mut c, r) = report();
    let a = fresh(&mut c, "a", "GroupMember", 250.0, 0.0);
    let f = fresh(&mut c, "f", "GroupMember", 450.0, 0.0);
    for x in [&a, &f] {
        set_av(&mut c, x, "Health", 3000.0);
    }
    c.wait(1.0);
    line(
        &mut c,
        &r,
        "a member casts our 25 point fire bolt at another of its faction, 2 s apart:\n",
    );
    let casts = c.local("casts", "Int");
    c.assign(casts.clone(), 0.into());
    let (top, done) = (c.label(), c.label());
    c.bind(top);
    let enough = c.calc(op::CMP_GE, "Bool", casts.clone(), 8.into());
    c.jmpt(enough, done);
    cast(&mut c, "Bolt", &a, &f);
    c.op(op::IADD, [casts.clone(), casts.clone(), 1.into()]);
    c.wait(2.0);
    put(&mut c, &r, "  cast ", casts.clone());
    let h = health(&mut c, &f);
    put_num(&mut c, &r, ": the struck one's health ", h);
    let state = c.method("Actor", f.clone(), "GetCombatState", []);
    put(&mut c, &r, ", combat state ", state);
    let target = c.method("Actor", f.clone(), "GetCombatTarget", []);
    let target_text = as_text(&mut c, target.clone());
    put(&mut c, &r, ", target ", target_text);
    let d = c.method("ObjectReference", a.clone(), "GetDistance", [f.clone()]);
    put_num(&mut c, &r, ", 2 apart ", d);
    line(&mut c, &r, "\n");
    let caster = c.temp("Actor");
    c.assign(caster.clone(), a.clone());
    let fights = c.calc(op::CMP_EQ, "Bool", target, caster);
    c.jmpt(fights, done);
    c.jmp(top);
    c.bind(done);
    remove_all(&mut c, &[&a, &f]);
    c.finish(r)
}

fn test_scenes() -> FunctionBuilder {
    let (mut c, r) = report();
    let global = auto_var("SceneGlobal");
    let set_global = |c: &mut Code, v: f32| {
        c.method("GlobalVariable", global.clone(), "SetValue", [v.into()]);
    };
    set_global(&mut c, 0.0);
    c.method("Quest", auto_var("SceneQuest"), "Start", []);
    c.wait(0.5);
    let a = fresh(&mut c, "a", "SceneActor", 250.0, 0.0);
    let alias = ref_alias(&mut c, "SceneQuest", 0);
    c.method("ReferenceAlias", alias, "ForceRefTo", [a.clone()]);
    c.wait(0.5);
    let ends = |c: &mut Code, r: &Arg, n: u32, scene: &str, label: &str, global_at: Option<f32>| {
        line(c, r, &format!("{label}:\n"));
        c.method("Scene", auto_var(scene), "Start", []);
        c.wait(0.25);
        let p = c.method("Scene", auto_var(scene), "IsPlaying", []);
        put(c, r, "  playing 0.25 s after Start ", p);
        line(c, r, "\n");
        if let Some(t) = global_at {
            c.wait(t);
            let p = c.method("Scene", auto_var(scene), "IsPlaying", []);
            put(c, r, &format!("  {} s later ", t), p);
            line(c, r, "; the global set to 1\n");
            set_global(c, 1.0);
        }
        let ended = poll(c, &format!("ended{n}"), 0.25, 25.0, |c, hit| {
            let p = c.method("Scene", auto_var(scene), "IsPlaying", []);
            c.jmpf(p, hit);
        });
        put_num(c, r, "  ended after (s; -1 not within 25) ", ended);
        line(c, r, "\n");
        c.method("Scene", auto_var(scene), "Stop", []);
        c.wait(0.5);
    };
    ends(
        &mut c,
        &r,
        0,
        "SceneControl",
        "a 5 s timer, no conditions",
        None,
    );
    ends(
        &mut c,
        &r,
        1,
        "SceneComplete",
        "a 20 s timer, the phase completing on the global >= 1, set 2 s in",
        Some(2.0),
    );
    ends(
        &mut c,
        &r,
        2,
        "SceneComplete",
        "the same, the global already 1",
        None,
    );
    set_global(&mut c, 0.0);
    ends(
        &mut c,
        &r,
        3,
        "SceneSkip",
        "a 20 s timer, the phase starting on the global >= 99 (never)",
        None,
    );
    c.method("Quest", auto_var("SceneQuest"), "Stop", []);
    remove(&mut c, &a);
    c.finish(r)
}

fn test_area_radius() -> FunctionBuilder {
    let (mut c, r) = report();
    line(
        &mut c,
        &r,
        "Fireball (40 fire damage, area 15) cast at a centre NPC from 300 units; each NPC's \
         distance from the centre before, health lost, distance after:\n",
    );
    let (cx, cy) = (600.0f32, 0.0f32);
    let centre = fresh(&mut c, "centre", "Dummy", cx, cy);
    let mut ring = vec![("the centre".to_owned(), centre.clone())];
    for (i, &(angle, d, label)) in RING.iter().enumerate() {
        let (s, co) = angle.to_radians().sin_cos();
        let a = fresh(
            &mut c,
            &format!("ring{i}"),
            "Dummy",
            cx + co * d,
            cy + s * d,
        );
        ring.push((label.to_owned(), a));
    }
    for (_, a) in &ring {
        set_av(&mut c, a, "Health", 5000.0);
        restrain(&mut c, a);
    }
    let caster = fresh(&mut c, "caster", "Caster", cx - 300.0, cy);
    set_av(&mut c, &caster, "Health", 5000.0);
    restrain(&mut c, &caster);
    c.wait(1.5);
    let mut before = Vec::new();
    for (i, (label, a)) in ring.iter().enumerate() {
        let h = health(&mut c, a);
        let hv = c.local(&format!("hp{i}"), "Float");
        c.assign(hv.clone(), h);
        let d = c.method(
            "ObjectReference",
            centre.clone(),
            "GetDistance",
            [a.clone()],
        );
        let dv = c.local(&format!("d{i}"), "Float");
        c.assign(dv.clone(), d);
        before.push((label.clone(), a.clone(), hv, dv));
    }
    cast(&mut c, "Fireball", &caster, &centre);
    c.wait(4.0);
    for (label, a, hv, dv) in before {
        let h = health(&mut c, &a);
        let lost = c.calc(op::FSUB, "Float", hv, h);
        let d = c.method(
            "ObjectReference",
            centre.clone(),
            "GetDistance",
            [a.clone()],
        );
        put_num(&mut c, &r, &format!("  {label}: "), dv);
        put_num(&mut c, &r, ", lost ", lost);
        put_num(&mut c, &r, ", after ", d);
        line(&mut c, &r, "\n");
    }
    for (_, a) in &ring {
        remove(&mut c, a);
    }
    remove(&mut c, &caster);
    c.finish(r)
}

/// `LogTrap(String what, ObjectReference akSelf, ObjectReference akTarget,
/// Bool abInitialHit, Int aeMotionType, Float afXVel, Float afYVel, Float
/// afZVel)`, from the traps' script.
fn log_trap() -> FunctionBuilder {
    let mut c = Code::new("None");
    let what = c.param("what", "String");
    let me = c.param("akSelf", "ObjectReference");
    let target = c.param("akTarget", "ObjectReference");
    let initial = c.param("abInitialHit", "Bool");
    let motion = c.param("aeMotionType", "Int");
    let vel = ["afXVel", "afYVel", "afZVel"].map(|n| c.param(n, "Float"));
    let t = now(&mut c);
    let dt = c.calc(op::FSUB, "Float", t, id("TrapStart"));
    let dt = num_of(&mut c, dt);
    let me = as_text(&mut c, me);
    let target = as_text(&mut c, target);
    c.append(
        id("TrapLog"),
        [
            string("  "),
            dt,
            string(" s: "),
            me,
            string(" "),
            what,
            string(" "),
            target,
            string(", initial "),
            initial,
            string(", motion "),
            motion,
            string(", velocity"),
        ],
    );
    for v in vel {
        let n = num_of(&mut c, v);
        c.append(id("TrapLog"), [string(" "), n]);
    }
    c.append(id("TrapLog"), [string("\n")]);
    c.finish(Arg::None)
}

fn test_traps() -> FunctionBuilder {
    let (mut c, r) = report();
    let start = now(&mut c);
    c.assign(id("TrapStart"), start);
    c.assign(id("TrapLog"), string(""));
    let n = fresh(&mut c, "n", "Dummy", 100.0, 300.0);
    set_av(&mut c, &n, "Health", 500.0);
    c.wait(1.5);
    restrain(&mut c, &n);
    let h0 = health(&mut c, &n);
    put_num(&mut c, &r, "an NPC held in place, health ", h0);
    line(&mut c, &r, "\n");
    let drop = |c: &mut Code, r: &Arg, base: &str, var: &str, height: f32, label: &str| {
        let x = c.method(
            "ObjectReference",
            n.clone(),
            "PlaceAtMe",
            [auto_var(base), 1.into(), false.into(), false.into()],
        );
        let x_v = c.local(var, "ObjectReference");
        c.assign(x_v.clone(), x);
        c.method(
            "ObjectReference",
            x_v.clone(),
            "MoveTo",
            [
                n.clone(),
                0.0f32.into(),
                0.0f32.into(),
                height.into(),
                false.into(),
            ],
        );
        if base == "TrapRock" {
            c.method(
                "ObjectReference",
                x_v.clone(),
                "SetMotionType",
                [MOTION_DYNAMIC.into(), false.into()],
            );
        }
        c.wait(4.0);
        line(c, r, &format!("{label}:\n"));
        put(c, r, "", id("TrapLog"));
        c.assign(id("TrapLog"), string(""));
        let zx = c.method("ObjectReference", x_v.clone(), "GetPositionZ", []);
        let zn = c.method("ObjectReference", n.clone(), "GetPositionZ", []);
        let dz = c.calc(op::FSUB, "Float", zx, zn);
        let d = c.method("ObjectReference", x_v.clone(), "GetDistance", [n.clone()]);
        let h = health(c, &n);
        put_num(c, r, "  4 s later: its height above the NPC's feet ", dz);
        put_num(c, r, ", distance ", d);
        put_num(c, r, ", the NPC's health ", h);
        line(c, r, "\n");
        remove(c, &x_v);
    };
    drop(
        &mut c,
        &r,
        "TrapBowl",
        "bowl",
        250.0,
        "our scripted bowl dropped from 250 units above it",
    );
    drop(
        &mut c,
        &r,
        "TrapRock",
        "rock",
        300.0,
        "a copy of the game's falling rock (TrapFallingRockSm01_Rock) with our script, \
         300 units above it, made dynamic",
    );
    remove(&mut c, &n);
    c.finish(r)
}

fn test_taper() -> FunctionBuilder {
    let (mut c, r) = report();
    for (index, case) in CASES.iter().enumerate() {
        let t = fresh(&mut c, &format!("t{index}"), "Dummy", 450.0, 0.0);
        let caster = fresh(&mut c, &format!("caster{index}"), "Dummy", 150.0, 0.0);
        for a in [&t, &caster] {
            set_av(&mut c, a, "Health", 5000.0);
            for av in ["Magicka", "Stamina"] {
                set_av(&mut c, a, av, 1000.0);
            }
            for rate in RATES {
                set_av(&mut c, a, rate, 0.0);
            }
            restrain(&mut c, a);
        }
        c.wait(1.5);
        if case.caster {
            damage(&mut c, &caster, case.av, 500.0);
        }
        c.wait(0.5);
        put(
            &mut c,
            &r,
            &format!("{} ({}, {} points", case.label, case.effect, case.magnitude),
            string(""),
        );
        if case.caster {
            line(&mut c, &r, &format!(", the caster's {} 500 down", case.av));
        }
        if case.drop_conditions {
            line(&mut c, &r, ", its conditions dropped");
        }
        line(&mut c, &r, "): ");
        let tl = c.temp("String");
        c.call_method(
            id("self"),
            "Timeline",
            tl.clone(),
            [
                auto_var(&format!("Taper{index}")),
                caster.clone(),
                t.clone(),
                string(case.av),
                string(case.second),
                case.caster.into(),
            ],
        );
        c.append(r.clone(), [tl]);
        remove_all(&mut c, &[&t, &caster]);
        c.wait(1.0);
    }
    c.finish(r)
}

fn scripts() -> Result<Vec<(String, Vec<u8>)>> {
    let mut s = ScriptBuilder::new(SCRIPT, "Quest");
    s.auto_property("Player", "Actor");
    for (name, ty, _) in VANILLA {
        s.auto_property(name, ty);
    }
    for (name, ty, _) in OWN {
        s.auto_property(name, ty);
    }
    for i in 0..CASES.len() {
        s.auto_property(&format!("Taper{i}"), "Spell");
    }
    s.variable("Ran", "Bool", false.into());
    s.variable("TrapLog", "String", string(""));
    s.variable("TrapStart", "Float", 0.0f32.into());
    for (name, _) in TESTS {
        s.variable(&format!("Result{name}"), "String", string(""));
    }
    let mut init = Code::new("None");
    init.method(
        "Form",
        id("self"),
        "RegisterForSingleUpdate",
        [5.0f32.into()],
    );
    s.function("OnInit", init.finish(Arg::None))
        .function("OnUpdate", on_update("autotest5", "six minutes", TESTS))
        .function("Num", num())
        .function("Place", place_fn())
        .function("Timeline", timeline())
        .function("LogTrap", log_trap())
        .function("TestAVPercent", test_av_percent())
        .function("TestRecheck", test_recheck())
        .function("TestHasShout", test_has_shout())
        .function("TestAbility", test_ability())
        .function("TestMagnitudePerk", test_magnitude_perk())
        .function("TestPerkPriority", test_perk_priority())
        .function("TestCombatRegen", test_combat_regen())
        .function("TestFriendlyHits", test_friendly_hits())
        .function("TestScenes", test_scenes())
        .function("TestAreaRadius", test_area_radius())
        .function("TestTraps", test_traps())
        .function("TestTaper", test_taper());

    // The traps: their events (and loading) to the test script.
    let mut trap = ScriptBuilder::new(TRAP_SCRIPT, "ObjectReference");
    trap.auto_property("Main", SCRIPT);
    for event in ["OnTrapHitStart", "OnTrapHit"] {
        relay(&mut trap, event, TRAP_PARAMS, "LogTrap", |c, ps| {
            let me = self_ref(c);
            vec![
                string(event),
                me,
                ps[0].clone(),
                ps[8].clone(),
                ps[9].clone(),
                ps[1].clone(),
                ps[2].clone(),
                ps[3].clone(),
            ]
        });
    }
    let quiet = |what: &'static str| {
        move |c: &mut Code, ps: &[Arg]| {
            let me = self_ref(c);
            vec![
                string(what),
                me,
                ps.first().cloned().unwrap_or(Arg::None),
                false.into(),
                0.into(),
                0.0f32.into(),
                0.0f32.into(),
                0.0f32.into(),
            ]
        }
    };
    relay(
        &mut trap,
        "OnTrapHitStop",
        &[("akTarget", "ObjectReference")],
        "LogTrap",
        quiet("OnTrapHitStop"),
    );
    relay(&mut trap, "OnLoad", &[], "LogTrap", quiet("OnLoad"));

    let mut out = Vec::new();
    for (name, b) in [(SCRIPT, &s), (TRAP_SCRIPT, &trap)] {
        out.push((name.to_owned(), papyrus::pex::write(&b.build()?)?));
    }
    Ok(out)
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let v = Vanilla { lo };
    let template = v.id("WEMerchantMiscMaleCommoner")?;
    let race = v.id("ImperialRace")?;
    let class = v.id("Citizen")?;
    let fire_damage = v.id("FireDamageFFAimed")?;
    let frost_damage = v.id("FrostDamageFFAimed")?;
    let either_hand = v.id("EitherHand")?;
    let unrelenting_force = v.id("UnrelentingForceShout")?;

    let object = PropertyValue::Object;
    let mut properties = vec![("Player".to_owned(), object(PLAYER_REF))];
    for (name, _, edid) in VANILLA {
        properties.push((name.to_string(), object(v.id(edid)?)));
    }
    for (name, _, object_id) in OWN {
        properties.push((name.to_string(), object(own(*object_id))));
    }
    for i in 0..CASES.len() {
        properties.push((format!("Taper{i}"), object(own(TAPER_SPELLS + i as u32))));
    }
    let main = AttachedScript {
        name: TRAP_SCRIPT.into(),
        properties: vec![("Main".into(), object(own(QUEST)))],
    };

    let factions = vec![
        records::faction(
            VICTIMS,
            "VrmAutoTest5Victims",
            &[(ENEMIES, reaction::ENEMY)],
        ),
        records::faction(
            ENEMIES,
            "VrmAutoTest5Enemies",
            &[(VICTIMS, reaction::ENEMY)],
        ),
        records::faction(GROUP, "VrmAutoTest5Group", &[(GROUP, reaction::FRIEND)]),
    ];
    let npcs: Vec<RecordBuilder> = [
        (DUMMY, "VrmAutoTest5Dummy", (0, 0), None),
        (CASTER, "VrmAutoTest5Caster", (0, 2), None),
        (VICTIM, "VrmAutoTest5Victim", (0, 2), Some(VICTIMS)),
        (ENEMY, "VrmAutoTest5Enemy", (1, 3), Some(ENEMIES)),
        (GROUP_MEMBER, "VrmAutoTest5GroupMember", (0, 2), Some(GROUP)),
        (SCENE_ACTOR, "VrmAutoTest5SceneActor", (0, 0), None),
    ]
    .into_iter()
    .map(|(object_id, edid, temper, faction)| {
        Npc {
            object_id,
            edid: edid.to_owned(),
            name: Some("vibe_rm Test NPC"),
            level: (10, false, 1, 0),
            template: (template, 0x1),
            temper,
            factions: faction.map(|f| (own(f), 0)).into_iter().collect(),
            script: None,
            assistance: 0,
            combat_style: None,
            unique: false,
        }
        .record(race, class)
    })
    .collect();

    // Magic effects: copies with their casting, delivery (and actor value)
    // changed, conditions dropped where asked.
    let patch = |cast: u32, delivery: u32, av: Option<u32>, drop_conditions: bool| {
        move |tag: &[u8; 4], d: &mut Vec<u8>| {
            if tag == b"DATA" && d.len() >= 0x58 {
                if let Some(av) = av {
                    d[0x44..0x48].copy_from_slice(&av.to_le_bytes());
                }
                d[0x50..0x54].copy_from_slice(&cast.to_le_bytes());
                d[0x54..0x58].copy_from_slice(&delivery.to_le_bytes());
            }
            if tag == b"CTDA" && drop_conditions {
                d.clear();
            }
        }
    };
    let ff = spell::FIRE_AND_FORGET;
    let ta = spell::TARGET_ACTOR;
    let mut effects = vec![
        records::copy(
            lo,
            "FireDamageFFAimed",
            FIRE_TA,
            "VrmAutoTest5FireTA",
            None,
            patch(ff, ta, None, false),
        )?,
        records::copy(
            lo,
            "FrostDamageFFAimed",
            FROST_TA,
            "VrmAutoTest5FrostTA",
            None,
            patch(ff, ta, None, false),
        )?,
        records::copy(
            lo,
            "AlchFortifyOneHanded",
            FORTIFY_AB,
            "VrmAutoTest5FortifyAbility",
            None,
            patch(spell::CONSTANT, spell::SELF, Some(ONE_HANDED), false),
        )?,
        records::copy(
            lo,
            "AlchFortifyOneHanded",
            FORTIFY_FF,
            "VrmAutoTest5FortifySelf",
            None,
            patch(ff, spell::SELF, Some(ONE_HANDED), false),
        )?,
    ];
    for (i, case) in CASES.iter().enumerate() {
        effects.push(records::copy(
            lo,
            case.effect,
            TAPER_EFFECTS + i as u32,
            &format!("VrmAutoTest5Taper{}Effect", case.name),
            None,
            patch(ff, ta, None, case.drop_conditions),
        )?);
    }

    let spell_of = |id, edid: &str, kind, casting, delivery, effects: Vec<Effect>| {
        records::spell(
            id,
            edid,
            kind,
            casting,
            delivery,
            0,
            (kind != spell::ABILITY).then_some(either_hand),
            &effects,
        )
    };
    let aimed =
        |id, edid: &str, effects| spell_of(id, edid, spell::SPELL, ff, spell::AIMED, effects);
    let av_percent = |value| {
        ctda(
            func::GET_ACTOR_VALUE_PERCENT,
            24,
            0,
            records::op::GE,
            value,
            run_on::SUBJECT,
        )
    };
    let mut spells = vec![
        aimed(
            AV_PERCENT,
            "VrmAutoTest5AvPercent",
            vec![
                Effect::new(frost_damage, 7.0, 0).when(av_percent(0.5)),
                Effect::new(frost_damage, 13.0, 0).when(av_percent(50.0)),
            ],
        ),
        aimed(
            RECHECK,
            "VrmAutoTest5Recheck",
            vec![Effect::new(frost_damage, 20.0, 30).when(ctda(
                func::GET_ACTOR_VALUE,
                24,
                0,
                records::op::GE,
                960.0,
                run_on::SUBJECT,
            ))],
        ),
        aimed(
            HAS_SHOUT,
            "VrmAutoTest5HasShout",
            vec![Effect::new(frost_damage, 10.0, 0).when(ctda(
                func::HAS_SHOUT,
                unrelenting_force.0,
                0,
                records::op::EQ,
                1.0,
                run_on::SUBJECT,
            ))],
        ),
        spell_of(
            ABILITY,
            "VrmAutoTest5Ability",
            spell::ABILITY,
            spell::CONSTANT,
            spell::SELF,
            vec![Effect::new(own(FORTIFY_AB), 7.0, 0)],
        ),
        spell_of(
            FORTIFY,
            "VrmAutoTest5Fortify",
            spell::SPELL,
            ff,
            spell::SELF,
            vec![Effect::new(own(FORTIFY_FF), 10.0, 60)],
        ),
        spell_of(
            PLAYER_FIRE,
            "VrmAutoTest5PlayerFire",
            spell::SPELL,
            ff,
            ta,
            vec![Effect::new(own(FIRE_TA), 50.0, 0)],
        ),
        spell_of(
            PLAYER_FROST,
            "VrmAutoTest5PlayerFrost",
            spell::SPELL,
            ff,
            ta,
            vec![Effect::new(own(FROST_TA), 50.0, 0)],
        ),
        aimed(
            BOLT,
            "VrmAutoTest5Bolt",
            vec![Effect::new(fire_damage, 25.0, 0)],
        ),
    ];
    for (i, case) in CASES.iter().enumerate() {
        spells.push(spell_of(
            TAPER_SPELLS + i as u32,
            &format!("VrmAutoTest5Taper{}", case.name),
            spell::SPELL,
            ff,
            ta,
            vec![Effect::new(
                own(TAPER_EFFECTS + i as u32),
                case.magnitude,
                0,
            )],
        ));
    }

    let entry = |function, value, priority| Section::Entry {
        point: MOD_SPELL_MAGNITUDE,
        function,
        tabs: 3,
        value,
        priority,
    };
    let perks = vec![
        records::perk(PERK_ADD, "VrmAutoTest5PerkAdd", &[entry(2, 10.0, 0)]),
        records::perk(PERK_MUL, "VrmAutoTest5PerkMul", &[entry(3, 2.0, 1)]),
        records::perk(
            PERK_ADD_LATE,
            "VrmAutoTest5PerkAddLate",
            &[entry(2, 10.0, 1)],
        ),
        records::perk(
            PERK_MUL_EARLY,
            "VrmAutoTest5PerkMulEarly",
            &[entry(3, 2.0, 0)],
        ),
        records::perk(
            PERK_ABILITY,
            "VrmAutoTest5PerkAbility",
            &[Section::Ability(own(ABILITY))],
        ),
    ];

    let quests = vec![
        Quest {
            object_id: QUEST,
            edid: "VrmAutoTest5Quest",
            flags: 0x0011,
            priority: 0,
            kind: 0,
            script: Some(AttachedScript {
                name: SCRIPT.into(),
                properties,
            }),
            stages: &[],
            aliases: Vec::new(),
        }
        .record(),
        Quest {
            object_id: SCENE_QUEST,
            edid: "VrmAutoTest5SceneQuest",
            flags: 0,
            priority: 0,
            kind: 0,
            script: None,
            stages: &[],
            aliases: vec![Alias {
                id: 0,
                name: "Actor",
                flags: alias_flags::OPTIONAL | alias_flags::ALLOW_REUSE,
                fill: Fill::Empty,
                packages: Vec::new(),
            }],
        }
        .record(),
    ];
    let globals = vec![records::global(
        SCENE_GLOBAL,
        "VrmAutoTest5SceneGlobal",
        0.0,
    )];
    let on_global = |value| {
        ctda(
            func::GET_GLOBAL_VALUE,
            own(SCENE_GLOBAL).0,
            0,
            records::op::GE,
            value,
            run_on::SUBJECT,
        )
    };
    let scene = |id, edid, start: &[[u8; 32]], complete: &[[u8; 32]], seconds| {
        records::scene(
            id,
            edid,
            own(SCENE_QUEST),
            0,
            0,
            0,
            start,
            complete,
            seconds,
        )
    };
    let scenes = vec![
        scene(SCENE_CONTROL, "VrmAutoTest5SceneControl", &[], &[], 5.0),
        scene(
            SCENE_COMPLETE,
            "VrmAutoTest5SceneComplete",
            &[],
            &[on_global(1.0)],
            20.0,
        ),
        scene(
            SCENE_SKIP,
            "VrmAutoTest5SceneSkip",
            &[on_global(99.0)],
            &[],
            20.0,
        ),
    ];

    let misc = vec![records::copy(
        lo,
        "BasicWoodenBowl01",
        TRAP_BOWL,
        "VrmAutoTest5TrapBowl",
        Some(main.clone()),
        |_, _| {},
    )?];
    let activators = vec![records::copy(
        lo,
        "TrapFallingRockSm01_Rock",
        TRAP_ROCK,
        "VrmAutoTest5TrapRock",
        Some(main),
        |_, _| {},
    )?];

    // Top groups in the Creation Kit's order.
    let plugin = write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "autotest4's follow-ups and taper's open ends: tests run by script on loading a save, \
         reports saved.",
        NEXT_OBJECT_ID,
        &[
            Node::top(b"GLOB", globals),
            Node::top(b"FACT", factions),
            Node::top(b"MGEF", effects),
            Node::top(b"SPEL", spells),
            Node::top(b"ACTI", activators),
            Node::top(b"MISC", misc),
            Node::top(b"NPC_", npcs),
            Node::top(b"QUST", quests),
            Node::top(b"PERK", perks),
            Node::top(b"SCEN", scenes),
        ],
    );
    Ok(TestMod {
        plugin,
        scripts: scripts()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_mods::script::dry_run::{form, run};

    #[test]
    fn the_tests_run_and_report() {
        let mut properties = vec![("Player".to_owned(), form(0x14, "Actor"))];
        for (i, (name, ty, _)) in VANILLA.iter().enumerate() {
            properties.push((name.to_string(), form(0x1000 + i as u32, ty)));
        }
        for (name, ty, object_id) in OWN {
            properties.push((name.to_string(), form(own(*object_id).0, ty)));
        }
        for i in 0..CASES.len() {
            properties.push((
                format!("Taper{i}"),
                form(own(TAPER_SPELLS + i as u32).0, "Spell"),
            ));
        }
        let shown = run(
            scripts().unwrap(),
            &[(own(QUEST).0, SCRIPT, properties)],
            TESTS.len(),
        );
        assert_eq!(shown.boxes.len(), TESTS.len(), "every test reports");
        assert_eq!(shown.saved_at_box, Some(0), "saves before the boxes");
        let all = shown.boxes.join("\n");
        for expected in [
            "  at full health: health lost 0\n",
            " (restored)",
            "  after AddShout(Unrelenting Force): health lost 0\n",
            "  removed: the player's OneHanded 1, has the ability False\n",
            "  the player's with Augmented Flames: health lost 0\n",
            "  +10 at priority 1, x2 at 0: health lost 0\n",
            "  StopCombat, 10 s: 1 1 1 1 (combat state 1)\n",
            "  cast 8: the struck one's health 1",
            "the same, the global already 1:\n  playing 0.25 s after Start ",
            "  220 units off, behind the last: 1, lost 0, after 1\n",
            "300 units above it, made dynamic:\n",
            "lightning rune (0.3, 2, 3 s; health and magicka) \
             (TrapRuneLightningFFLocation02, 50 points, its conditions dropped): no change in 6 s",
        ] {
            assert!(all.contains(expected), "{expected:?} not in the reports");
        }
    }
}
