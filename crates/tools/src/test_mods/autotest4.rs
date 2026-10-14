//! `test-mod autotest4`: tier 2 of `plans/autotest-survey.md` in one plugin,
//! the ambiguities whose tests need records the earlier test mods didn't
//! write: spells and magic effects, perks, shouts, quests with aliases and
//! stages, Story Manager nodes, scenes, packages, an interior cell with a
//! trigger volume. As the other autotests: loading a save with the plugin
//! for the first time starts `VrmAutoTest4Quest`, whose script runs the
//! tests, keeps each report in a `Result<Name>` variable, asks the game to
//! save and shows the reports; `vrm-tool test-results` reads them.
//!
//! Load the usual save (outdoors near Riverwood, without the test plugins)
//! and leave the game alone for about twenty minutes. The run changes the
//! save for good (spells, perks, skills, shouts and quests of ours, the
//! hour and the timescale moved and put back), and ends with the player
//! taken into the test cell and back.
//!
//! The tests (and the files whose questions they ask):
//!
//! - **Hits** (`combat-state.md`): Firebolt's two `OnHit`s. The target logs
//!   each `OnHit` and `OnMagicEffectApply` with its health then; Firebolt and
//!   a spell of ours with only its fire damage effect are cast at it.
//! - **AVPercent** (`actor-values.md`): a spell whose effects need
//!   `GetActorValuePercent Health` >= 0.5 and >= 50, cast at full health and
//!   at 40%: is the condition's scale 0..1 or 0..100?
//! - **MagicConditions** (`magic.md`): a fire spell needing level 5 or
//!   more, run on the subject or the target, cast by a level 1 caster at a
//!   level 20 NPC: which do they mean? A lasting effect needing health 100
//!   or more: how soon it goes when health drops below, and comes back.
//! - **Resistance** (`magic.md`): a 50 point fire spell at fire resistance
//!   0, 50, 90, 100, 200, with resistance ignored, with magic resistance too,
//!   and at the player with 90 and 100 (the 85 cap).
//! - **TargetValues** (`magic.md`): Calm, Fear, Fury and Courage on level 5
//!   and level 20 NPCs: the values held and the level cap.
//! - **Absorb**: an absorb health spell's take and give.
//! - **BoundWeapon**: Bound Sword on the player; when it ends after another
//!   weapon is equipped.
//! - **AreaRadius** (`casting.md`): Fireball (15 feet) at an NPC with others
//!   at 100 to 500 units and the caster at 300.
//! - **MagnitudePerk**: Augmented Flames on the player's fire spell.
//! - **PerkPriority** (`perks.md`): perks of ours adding 10 and doubling Mod
//!   Spell Magnitude, at either priority: which applies first?
//! - **PerkSections** (`perks.md`): a quest section's stage set on adding
//!   (and kept on removing), an ability section's spell.
//! - **AdvanceSkill** (`skills.md`): `AdvanceSkill(skill, 10)` calls until a
//!   skill set to 20 (and 50) rises, and on to the next level.
//! - **Shouts** (`shouts.md`): word and shout statistics around `TeachWord`,
//!   `UnlockWord`, `AddShout` of ours; `HasShout` (a condition) for a shout
//!   given by script.
//! - **AliasPackages** (`alias-packages.md`): two quests of equal priority
//!   each with an alias forced to one NPC, each alias with its own package,
//!   started in either order; one quest with two such aliases.
//! - **StoryManager** (`story-manager.md`): script event nodes of ours: on
//!   whom node conditions run, the members `OnStoryScript` gets, reset hours,
//!   most running, what `SendStoryEventAndWait` waits for.
//! - **Scenes** (`scenes.md`): `Start` with the quest stopped or the alias
//!   empty, the actor in another scene, `ForceStart`, completion conditions,
//!   start conditions failing, death ending, stopping the quest on end.
//! - **Assistance** (`combat-assistance.md`): helpers allied or friendly to a
//!   victim, helping allies or friends or nobody, 500 to 5000 units off.
//! - **FriendlyHits**: casts at a member of the caster's own faction until
//!   it fights back.
//! - **CombatRegen** (`combat-fleeing.md`): health coming back in combat and
//!   out of it.
//! - **Flee** (`combat-fleeing.md`): a coward facing a strong enemy,
//!   unharmed and harmed.
//! - **DualWield** (`hands.md`): NPCs with a dual wielding combat style or
//!   not, given two swords (and a shield), fighting.
//! - **Torches** (`hands.md`): NPCs given a torch and a one- or two-handed
//!   weapon at night.
//! - In the test cell: **AliasFills** (`alias-fills.md`: forced to a disabled
//!   reference, a dead or unplaced unique actor, created at each level, the
//!   first or closest matching reference), **Triggers** (`triggers.md`: what
//!   enters a box, how far outside counts, disabling, items and bodies),
//!   **Traps** (`traps.md`: a scripted bowl of ours dropped and pushed onto
//!   an NPC).
//!
//! Left out of tier 2 (`plans/autotest-survey.md`): summons, perk ranks,
//! near aliases' reach, item pickup ownership and `XRNK`, scene flag 0x4
//! and looping dialogue, the flee check's timing and its ending.
//!
//! Run in Skyrim SE (2026-10-13): the results are in `known_ambiguities/`
//! (magic, skills, perks, shouts, alias fills and packages, the Story
//! Manager, scenes, assistance, hands, triggers). Not settled: the spells of
//! ours with potions' fortify effects didn't apply from a spell (the
//! condition scale and recheck, `HasShout`, the ability's effect), the
//! player's spells cast by script didn't go at the target (the perk tests),
//! held NPCs neither fought nor regenerated, the bowl sent no trap events,
//! and whether scene completion conditions end a phase wasn't seen:
//! `autotest5` follows these up.

use anyhow::{Context, Result};
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, write_plugin};
use esp::{FormId, LoadOrder, group_type, record_flags};
use papyrus::build::{Arg, FunctionBuilder, ScriptBuilder, auto_var, discard, id, string};
use papyrus::pex::op;

use super::autotest2::{Npc, Vanilla, as_form, line, place, place_fn, put, remove, sheathe};
use super::records::{
    self, Alias, Effect, Fill, Quest, Section, alias_flags, ctda, event_data_is, func, reaction,
    run_on, scene_flags, spell,
};
use super::script::{Code, num, on_update};
use super::venv::{placement, xcll};
use super::{TestMod, own};

// Object ids. The quest first, then factions, NPCs, magic, perks, quests,
// packages, keywords, Story Manager nodes, scenes, shouts and the cell.
const QUEST: u32 = 0x800;
const FACTION_A: u32 = 0x801;
const FACTION_B: u32 = 0x802;
const VICTIMS: u32 = 0x803;
const ENEMIES: u32 = 0x804;
const ALLIES: u32 = 0x805;
const FRIENDS: u32 = 0x806;
const GROUP: u32 = 0x807;

const DUMMY: u32 = 0x810;
const CASTER: u32 = 0x811;
const LEVEL5: u32 = 0x812;
const LEVEL20: u32 = 0x813;
const TARGET: u32 = 0x814;
const HELPER_ALLY_ALLIES: u32 = 0x815;
const HELPER_ALLY_NOBODY: u32 = 0x816;
const HELPER_FRIEND_ALLIES: u32 = 0x817;
const HELPER_FRIEND_FRIENDS: u32 = 0x818;
const VICTIM: u32 = 0x819;
const ENEMY: u32 = 0x81A;
const GROUP_MEMBER: u32 = 0x81B;
const COWARD: u32 = 0x81C;
const BRUTE: u32 = 0x81D;
const DUAL: u32 = 0x81E;
const DUAL_PLAIN: u32 = 0x81F;
const BAG: u32 = 0x820;
const UNIQUE: u32 = 0x822;
const UNPLACED: u32 = 0x823;
const CREATED: u32 = 0x824;
const MATCHABLE: u32 = 0x825;
const PACKAGED: u32 = 0x826;
const SCENE_ACTOR: u32 = 0x827;
const STORY_ACTOR: u32 = 0x828;
const LOW_CASTER: u32 = 0x829;

const BOLT: u32 = 0x830;
const BLAST: u32 = 0x831;
const BLAST_IGNORE: u32 = 0x832;
const FORT_COND: u32 = 0x833;
const SUBJECT: u32 = 0x834;
const SUBJECT_TARGET: u32 = 0x835;
const RECHECK: u32 = 0x836;
const ABSORB: u32 = 0x837;
const HAS_SHOUT: u32 = 0x838;
const ABILITY: u32 = 0x839;
const ABSORB_EFFECT: u32 = 0x83A;
const ABILITY_EFFECT: u32 = 0x83B;

const PERK_ADD: u32 = 0x840;
const PERK_MUL: u32 = 0x841;
const PERK_ADD_LATE: u32 = 0x842;
const PERK_MUL_EARLY: u32 = 0x843;
const PERK_QUEST: u32 = 0x844;
const PERK_ABILITY: u32 = 0x845;

const PERK_STAGE_QUEST: u32 = 0x850;
const FILL_QUEST: u32 = 0x851;
const PACKAGE_QUEST_A: u32 = 0x852;
const PACKAGE_QUEST_B: u32 = 0x853;
const PACKAGE_QUEST_C: u32 = 0x854;
const SCENE_QUEST: u32 = 0x855;
const SCENE_QUEST_STOP: u32 = 0x856;
const STORY_NPC_QUEST: u32 = 0x857;
const STORY_PLAYER_QUEST: u32 = 0x858;
const MEMBERS_QUEST: u32 = 0x859;
const RESET_QUEST: u32 = 0x85A;
const MAX_QUEST_1: u32 = 0x85B;
const MAX_QUEST_2: u32 = 0x85C;
const WAIT_QUEST: u32 = 0x85D;

const PACKAGE_A: u32 = 0x860;
const PACKAGE_B: u32 = 0x861;
const PACKAGE_C1: u32 = 0x862;
const PACKAGE_C2: u32 = 0x863;

const KEY_SUBJECT: u32 = 0x868;
const KEY_MEMBERS: u32 = 0x869;
const KEY_RESET: u32 = 0x86A;
const KEY_MAX: u32 = 0x86B;
const KEY_WAIT: u32 = 0x86C;

const NODE_BRANCH: u32 = 0x870;
const NODE_SUBJECT_NPC: u32 = 0x871;
const NODE_SUBJECT_PLAYER: u32 = 0x872;
const NODE_MEMBERS: u32 = 0x873;
const NODE_RESET: u32 = 0x874;
const NODE_MAX: u32 = 0x875;
const NODE_WAIT: u32 = 0x876;

const SCENE_PLAIN: u32 = 0x878;
const SCENE_LONG: u32 = 0x879;
const SCENE_COMPLETE: u32 = 0x87A;
const SCENE_DEATH: u32 = 0x87B;
const SCENE_STOP_QUEST: u32 = 0x87C;
const SCENE_SKIP: u32 = 0x87D;

const WORDS: u32 = 0x880;
const SHOUT: u32 = 0x883;

const TRIGGER: u32 = 0x888;
const TRAP_BOWL: u32 = 0x889;
const CELL: u32 = 0x88A;
const FLOOR_REF: u32 = 0x88B;
const TRIGGER_REF: u32 = 0x88C;
const ENTRY_REF: u32 = 0x88D;
const FORCED_REF: u32 = 0x88E;
const UNIQUE_REF: u32 = 0x88F;
const NEXT_OBJECT_ID: u32 = 0x890;

const SCRIPT: &str = "VrmAutoTest4Script";
const HIT_SCRIPT: &str = "VrmAutoTest4HitScript";
const TRIGGER_SCRIPT: &str = "VrmAutoTest4TriggerScript";
const TRAP_SCRIPT: &str = "VrmAutoTest4TrapScript";
const MEMBERS_SCRIPT: &str = "VrmAutoTest4MembersScript";
const WAIT_SCRIPT: &str = "VrmAutoTest4WaitScript";
const PLAYER_REF: FormId = FormId(0x14);
const PLAYER_NPC: FormId = FormId(0x7);
/// The trigger box's half extents (units).
const TRIGGER_HALF: f32 = 128.0;
/// The game's script event (`SCPT`) root node.
const SCRIPT_EVENT_ROOT: &str = "SCPT";
/// Mod Spell Magnitude's entry point and its condition tabs (the caster,
/// the spell, the target).
const MOD_SPELL_MAGNITUDE: u8 = 29;

/// Vanilla forms the scripts use: property, type, editor id.
const VANILLA: &[(&str, &str, &str)] = &[
    ("IronSword", "Weapon", "IronSword"),
    ("IronGreatsword", "Weapon", "IronGreatsword"),
    ("IronShield", "Armor", "ArmorIronShield"),
    ("Torch", "Light", "Torch01"),
    ("TimeScale", "GlobalVariable", "TimeScale"),
    ("GameHour", "GlobalVariable", "GameHour"),
    ("Firebolt", "Spell", "Firebolt"),
    ("Fireball", "Spell", "Fireball"),
    ("Calm", "Spell", "Calm"),
    ("Fear", "Spell", "Fear"),
    ("Fury", "Spell", "Fury"),
    ("Courage", "Spell", "Courage"),
    ("BoundSword", "Spell", "BoundSword"),
    ("BoundSwordEffect", "MagicEffect", "BoundSwordFFSelf"),
    ("AugmentedFlames", "Perk", "AugmentedFlames"),
    ("XMarker", "Form", "XMarker"),
];

/// Our own forms the scripts use: property, type, object id.
const OWN: &[(&str, &str, u32)] = &[
    ("Dummy", "ActorBase", DUMMY),
    ("Caster", "ActorBase", CASTER),
    ("LowCaster", "ActorBase", LOW_CASTER),
    ("Level5", "ActorBase", LEVEL5),
    ("Level20", "ActorBase", LEVEL20),
    ("Target", "ActorBase", TARGET),
    ("HelperAllyAllies", "ActorBase", HELPER_ALLY_ALLIES),
    ("HelperAllyNobody", "ActorBase", HELPER_ALLY_NOBODY),
    ("HelperFriendAllies", "ActorBase", HELPER_FRIEND_ALLIES),
    ("HelperFriendFriends", "ActorBase", HELPER_FRIEND_FRIENDS),
    ("Victim", "ActorBase", VICTIM),
    ("Enemy", "ActorBase", ENEMY),
    ("GroupMember", "ActorBase", GROUP_MEMBER),
    ("Coward", "ActorBase", COWARD),
    ("Brute", "ActorBase", BRUTE),
    ("Dual", "ActorBase", DUAL),
    ("DualPlain", "ActorBase", DUAL_PLAIN),
    ("Bag", "ActorBase", BAG),
    ("Created", "ActorBase", CREATED),
    ("Matchable", "ActorBase", MATCHABLE),
    ("Packaged", "ActorBase", PACKAGED),
    ("SceneActor", "ActorBase", SCENE_ACTOR),
    ("StoryActor", "ActorBase", STORY_ACTOR),
    ("Bolt", "Spell", BOLT),
    ("Blast", "Spell", BLAST),
    ("BlastIgnore", "Spell", BLAST_IGNORE),
    ("FortCond", "Spell", FORT_COND),
    ("Subject", "Spell", SUBJECT),
    ("SubjectTarget", "Spell", SUBJECT_TARGET),
    ("Recheck", "Spell", RECHECK),
    ("Absorb", "Spell", ABSORB),
    ("HasShoutSpell", "Spell", HAS_SHOUT),
    ("Ability", "Spell", ABILITY),
    ("PerkAdd", "Perk", PERK_ADD),
    ("PerkMul", "Perk", PERK_MUL),
    ("PerkAddLate", "Perk", PERK_ADD_LATE),
    ("PerkMulEarly", "Perk", PERK_MUL_EARLY),
    ("PerkQuest", "Perk", PERK_QUEST),
    ("PerkAbility", "Perk", PERK_ABILITY),
    ("PerkStageQuest", "Quest", PERK_STAGE_QUEST),
    ("FillQuest", "Quest", FILL_QUEST),
    ("PackageQuestA", "Quest", PACKAGE_QUEST_A),
    ("PackageQuestB", "Quest", PACKAGE_QUEST_B),
    ("PackageQuestC", "Quest", PACKAGE_QUEST_C),
    ("SceneQuest", "Quest", SCENE_QUEST),
    ("SceneQuestStop", "Quest", SCENE_QUEST_STOP),
    ("StoryNpcQuest", "Quest", STORY_NPC_QUEST),
    ("StoryPlayerQuest", "Quest", STORY_PLAYER_QUEST),
    ("MembersQuest", MEMBERS_SCRIPT, MEMBERS_QUEST),
    ("ResetQuest", "Quest", RESET_QUEST),
    ("MaxQuest1", "Quest", MAX_QUEST_1),
    ("MaxQuest2", "Quest", MAX_QUEST_2),
    ("WaitQuest", WAIT_SCRIPT, WAIT_QUEST),
    ("PackageA", "Package", PACKAGE_A),
    ("PackageB", "Package", PACKAGE_B),
    ("PackageC1", "Package", PACKAGE_C1),
    ("PackageC2", "Package", PACKAGE_C2),
    ("KeySubject", "Keyword", KEY_SUBJECT),
    ("KeyMembers", "Keyword", KEY_MEMBERS),
    ("KeyReset", "Keyword", KEY_RESET),
    ("KeyMax", "Keyword", KEY_MAX),
    ("KeyWait", "Keyword", KEY_WAIT),
    ("ScenePlain", "Scene", SCENE_PLAIN),
    ("SceneLong", "Scene", SCENE_LONG),
    ("SceneComplete", "Scene", SCENE_COMPLETE),
    ("SceneDeath", "Scene", SCENE_DEATH),
    ("SceneStopQuest", "Scene", SCENE_STOP_QUEST),
    ("SceneSkip", "Scene", SCENE_SKIP),
    ("Word1", "WordOfPower", WORDS),
    ("Word2", "WordOfPower", WORDS + 1),
    ("Word3", "WordOfPower", WORDS + 2),
    ("Shout", "Shout", SHOUT),
    ("TrapBowl", "MiscObject", TRAP_BOWL),
    ("TriggerRef", "ObjectReference", TRIGGER_REF),
    ("EntryRef", "ObjectReference", ENTRY_REF),
    ("ForcedRef", "ObjectReference", FORCED_REF),
    ("UniqueRef", "Actor", UNIQUE_REF),
];

/// The script's variables holding references the reports name.
const REF_VARS: &[&str] = &[
    "TargetRef",
    "CasterRef",
    "VictimRef",
    "EnemyRef",
    "HelperRef",
    "MatchRef1",
    "MatchRef2",
    "TrigA",
    "TrigB",
    "TrigItem",
    "Bowl1",
    "Bowl2",
];

const TESTS: &[(&str, &str)] = &[
    ("Hits", "Firebolt's hits"),
    ("AVPercent", "GetActorValuePercent's scale"),
    ("MagicConditions", "Magic conditions"),
    ("Resistance", "Resistance"),
    ("TargetValues", "Calm, fear, frenzy, rally"),
    ("Absorb", "Absorb"),
    ("BoundWeapon", "Bound Sword"),
    ("AreaRadius", "Fireball's area"),
    ("MagnitudePerk", "Augmented Flames"),
    ("PerkPriority", "Perk priority"),
    ("PerkSections", "Perk sections"),
    ("AdvanceSkill", "AdvanceSkill"),
    ("Shouts", "Shouts"),
    ("AliasPackages", "Alias packages"),
    ("StoryManager", "Story Manager"),
    ("Scenes", "Scenes"),
    ("Assistance", "Assistance"),
    ("FriendlyHits", "Friendly hits"),
    ("CombatRegen", "Health in combat"),
    ("Flee", "Fleeing"),
    ("DualWield", "Dual wielding"),
    ("Torches", "Torches at night"),
    ("AliasFills", "Alias fills"),
    ("Triggers", "Trigger volume"),
    ("Traps", "Trap hits"),
];

/// `Name(Form f)`: what names `f` among the player, the reference variables
/// and the properties, "none", or `f` cast.
fn name_fn() -> FunctionBuilder {
    let mut c = Code::new("String");
    let f = c.param("f", "Form");
    let none = c.temp("Form");
    c.cast(none.clone(), Arg::None);
    let is_none = c.calc(op::CMP_EQ, "Bool", f.clone(), none);
    let next = c.label();
    c.jmpf(is_none, next).ret(string("none")).bind(next);
    let mut names: Vec<(&str, Arg)> = vec![("Player", auto_var("Player"))];
    names.extend(REF_VARS.iter().map(|v| (*v, id(v))));
    names.extend(VANILLA.iter().map(|v| (v.0, auto_var(v.0))));
    names.extend(OWN.iter().map(|v| (v.0, auto_var(v.0))));
    for (name, var) in names {
        let p = as_form(&mut c, var);
        let eq = c.calc(op::CMP_EQ, "Bool", f.clone(), p);
        let next = c.label();
        c.jmpf(eq, next).ret(string(name)).bind(next);
    }
    let s = c.temp("String");
    c.cast(s.clone(), f);
    c.finish(s)
}

fn name(c: &mut Code, f: Arg) -> Arg {
    let f = as_form(c, f);
    c.own("Name", "String", [f])
}

/// A float as text (`Num`).
fn num_of(c: &mut Code, f: Arg) -> Arg {
    let v = c.temp("Float");
    c.cast(v.clone(), f);
    c.own("Num", "String", [v])
}

fn now(c: &mut Code) -> Arg {
    c.global("Utility", "GetCurrentRealTime", [])
}

/// Seconds since `HitStart`.
fn since_start(c: &mut Code) -> Arg {
    let t = now(c);
    let dt = c.calc(op::FSUB, "Float", t, id("HitStart"));
    num_of(c, dt)
}

fn health(c: &mut Code, a: &Arg) -> Arg {
    c.method("Actor", a.clone(), "GetActorValue", [string("Health")])
}

fn set_av(c: &mut Code, a: &Arg, av: &str, value: f32) {
    c.method(
        "Actor",
        a.clone(),
        "SetActorValue",
        [string(av), value.into()],
    );
}

fn force_av(c: &mut Code, a: &Arg, av: &str, value: f32) {
    c.method(
        "Actor",
        a.clone(),
        "ForceActorValue",
        [string(av), value.into()],
    );
}

fn restrain(c: &mut Code, a: &Arg) {
    c.method("Actor", a.clone(), "SetRestrained", [true.into()]);
}

fn cast(c: &mut Code, spell: &str, source: &Arg, target: &Arg) {
    c.method(
        "Spell",
        auto_var(spell),
        "Cast",
        [source.clone(), target.clone()],
    );
}

/// One placed (`Place`) and kept in a variable of its own.
fn fresh(c: &mut Code, var: &str, base: &str, x: f32, y: f32) -> Arg {
    let a = place(c, base, x, y);
    let v = c.local(var, "Actor");
    c.assign(v.clone(), a);
    v
}

/// `"text" + value` lines: a report's way of saying a number.
fn put_num(c: &mut Code, r: &Arg, text: &str, value: Arg) {
    let n = num_of(c, value);
    put(c, r, text, n);
}

/// A loop: `body` until it jumps to the returned end label or `seconds`
/// pass (checked every `step`). Returns the seconds taken (-1 when the time
/// ran out) in a new local `var`.
fn poll(
    c: &mut Code,
    var: &str,
    step: f32,
    seconds: f32,
    body: impl FnOnce(&mut Code, papyrus::build::Label),
) -> Arg {
    let start = now(c);
    let start_v = c.temp("Float");
    c.assign(start_v.clone(), start);
    let out = c.local(var, "Float");
    c.assign(out.clone(), (-1.0f32).into());
    let (top, hit, done) = (c.label(), c.label(), c.label());
    c.bind(top);
    c.wait(step);
    let t = now(c);
    let dt = c.calc(op::FSUB, "Float", t, start_v.clone());
    let over = c.calc(op::CMP_GT, "Bool", dt, seconds.into());
    c.jmpt(over, done);
    body(c, hit);
    c.jmp(top);
    c.bind(hit);
    let t = now(c);
    let dt = c.calc(op::FSUB, "Float", t, start_v);
    c.assign(out.clone(), dt);
    c.bind(done);
    out
}

/// `LogHit(...)`, from the target's script on each `OnHit`: when, by whom,
/// with what, and the target's health then.
fn log_hit() -> FunctionBuilder {
    let mut c = Code::new("None");
    let aggressor = c.param("akAggressor", "ObjectReference");
    let source = c.param("akSource", "Form");
    let projectile = c.param("akProjectile", "Projectile");
    let flags = [
        "abPowerAttack",
        "abSneakAttack",
        "abBashAttack",
        "abHitBlocked",
    ]
    .map(|n| c.param(n, "Bool"));
    let log = id("HitLog");
    let t = since_start(&mut c);
    let who = name(&mut c, aggressor);
    let source = name(&mut c, source);
    let projectile = name(&mut c, projectile);
    let hp = health(&mut c, &id("TargetRef"));
    let hp = num_of(&mut c, hp);
    c.append(
        log.clone(),
        [
            t,
            string(" s: OnHit by "),
            who,
            string(", source "),
            source,
            string(", projectile "),
            projectile,
            string(", health "),
            hp,
        ],
    );
    for (flag, label) in flags.into_iter().zip(["power", "sneak", "bash", "blocked"]) {
        c.append(log.clone(), [string(&format!(", {label} ")), flag]);
    }
    c.append(log, [string("\n")]);
    c.finish(Arg::None)
}

/// `LogEffect(ObjectReference akCaster, MagicEffect akEffect)`, from the
/// target's script on each `OnMagicEffectApply`.
fn log_effect() -> FunctionBuilder {
    let mut c = Code::new("None");
    let caster = c.param("akCaster", "ObjectReference");
    let effect = c.param("akEffect", "MagicEffect");
    let t = since_start(&mut c);
    let who = name(&mut c, caster);
    let effect = name(&mut c, effect);
    let hp = health(&mut c, &id("TargetRef"));
    let hp = num_of(&mut c, hp);
    c.append(
        id("HitLog"),
        [
            t,
            string(" s: OnMagicEffectApply "),
            effect,
            string(" from "),
            who,
            string(", health "),
            hp,
            string("\n"),
        ],
    );
    c.finish(Arg::None)
}

/// `LogTrigger(ObjectReference akActionRef, String what)`, from the
/// trigger's script.
fn log_trigger() -> FunctionBuilder {
    let mut c = Code::new("None");
    let who = c.param("akActionRef", "ObjectReference");
    let what = c.param("what", "String");
    let who = name(&mut c, who);
    let count = c.method(
        "ObjectReference",
        auto_var("TriggerRef"),
        "GetTriggerObjectCount",
        [],
    );
    c.append(
        id("TrigLog"),
        [
            string("  "),
            what,
            string(" "),
            who,
            string(" (count "),
            count,
            string(")\n"),
        ],
    );
    c.finish(Arg::None)
}

/// `LogTrap(String what, ObjectReference akSelf, ObjectReference akTarget,
/// Bool abInitialHit, Int aeMotionType, Float afXVel, Float afYVel, Float
/// afZVel)`, from the bowl's script.
fn log_trap() -> FunctionBuilder {
    let mut c = Code::new("None");
    let what = c.param("what", "String");
    let me = c.param("akSelf", "ObjectReference");
    let target = c.param("akTarget", "ObjectReference");
    let initial = c.param("abInitialHit", "Bool");
    let motion = c.param("aeMotionType", "Int");
    let vel = ["afXVel", "afYVel", "afZVel"].map(|n| c.param(n, "Float"));
    let t = since_start(&mut c);
    let me = name(&mut c, me);
    let target = name(&mut c, target);
    c.append(
        id("TrapLog"),
        [
            t,
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

/// A test function's start: an empty report.
fn report() -> (Code, Arg) {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    (c, r)
}

/// The usual caster (level 10) and a target held in place with plenty of
/// health, kept in `CasterRef` / `TargetRef`.
fn caster_and_target(c: &mut Code, target_base: &str, hp: f32) -> (Arg, Arg) {
    let t = fresh(c, "t", target_base, 450.0, 0.0);
    set_av(c, &t, "Health", hp);
    restrain(c, &t);
    c.assign(id("TargetRef"), t.clone());
    let caster = fresh(c, "caster", "Caster", 150.0, 0.0);
    c.assign(id("CasterRef"), caster.clone());
    c.wait(1.0);
    (caster, t)
}

fn test_hits() -> FunctionBuilder {
    let (mut c, r) = report();
    let start = now(&mut c);
    c.assign(id("HitStart"), start);
    c.assign(id("HitLog"), string(""));
    let (caster, t) = caster_and_target(&mut c, "Target", 2000.0);
    for (spell, label) in [
        (
            "Firebolt",
            "Firebolt (fire damage, Impact's stagger, Intense Flames' fear)",
        ),
        ("Bolt", "ours: Firebolt's fire damage alone"),
    ] {
        c.append(id("HitLog"), [string(&format!("-- {label}\n"))]);
        cast(&mut c, spell, &caster, &t);
        c.wait(4.0);
    }
    put(&mut c, &r, "", id("HitLog"));
    remove(&mut c, &caster);
    remove(&mut c, &t);
    c.finish(r)
}

/// The values a fortify test reads: one- and two-handed, health percent.
fn fortify_state(c: &mut Code, r: &Arg, a: &Arg, label: &str) {
    let one = c.method("Actor", a.clone(), "GetActorValue", [string("OneHanded")]);
    let two = c.method("Actor", a.clone(), "GetActorValue", [string("TwoHanded")]);
    let pct = c.method(
        "Actor",
        a.clone(),
        "GetActorValuePercentage",
        [string("Health")],
    );
    put_num(c, r, &format!("{label}: OneHanded "), one);
    put_num(c, r, ", TwoHanded ", two);
    put_num(c, r, ", health ", pct);
    line(c, r, "\n");
}

fn test_av_percent() -> FunctionBuilder {
    let (mut c, r) = report();
    line(
        &mut c,
        &r,
        "a self spell: +10 OneHanded if GetActorValuePercent(Health) >= 0.5, \
         +10 TwoHanded if >= 50\n",
    );
    let n = fresh(&mut c, "n", "Dummy", 200.0, 100.0);
    set_av(&mut c, &n, "Health", 200.0);
    c.wait(1.0);
    fortify_state(&mut c, &r, &n, "before");
    cast(&mut c, "FortCond", &n, &n);
    c.wait(1.5);
    fortify_state(&mut c, &r, &n, "cast at full health");
    c.method("Actor", n.clone(), "DispelSpell", [auto_var("FortCond")]);
    c.method(
        "Actor",
        n.clone(),
        "DamageActorValue",
        [string("Health"), 120.0f32.into()],
    );
    c.wait(1.0);
    fortify_state(&mut c, &r, &n, "dispelled, health to 40%");
    cast(&mut c, "FortCond", &n, &n);
    c.wait(1.5);
    fortify_state(&mut c, &r, &n, "cast at 40%");
    remove(&mut c, &n);
    c.finish(r)
}

fn test_magic_conditions() -> FunctionBuilder {
    let (mut c, r) = report();
    // Whose level: a level 1 caster, a level 20 target.
    let t = fresh(&mut c, "t", "Level20", 450.0, 0.0);
    set_av(&mut c, &t, "Health", 2000.0);
    restrain(&mut c, &t);
    let caster = fresh(&mut c, "caster", "LowCaster", 150.0, 0.0);
    c.wait(1.0);
    let level_c = c.method("Actor", caster.clone(), "GetLevel", []);
    let level_t = c.method("Actor", t.clone(), "GetLevel", []);
    put(&mut c, &r, "caster level ", level_c);
    put(&mut c, &r, ", target level ", level_t);
    line(&mut c, &r, "\n");
    for (spell, label) in [
        (
            "Subject",
            "20 fire damage if GetLevel >= 5 (run on the subject)",
        ),
        ("SubjectTarget", "the same run on the target"),
    ] {
        let before = health(&mut c, &t);
        let before_v = c.temp("Float");
        c.assign(before_v.clone(), before);
        cast(&mut c, spell, &caster, &t);
        c.wait(3.0);
        let after = health(&mut c, &t);
        let lost = c.calc(op::FSUB, "Float", before_v, after);
        put_num(&mut c, &r, &format!("{label}: health lost "), lost);
        line(&mut c, &r, "\n");
    }
    remove(&mut c, &caster);
    remove(&mut c, &t);

    // How soon a lasting effect's conditions are looked at again.
    line(
        &mut c,
        &r,
        "a self spell: +10 TwoHanded for 60 s if GetActorValue(Health) >= 100\n",
    );
    let n = fresh(&mut c, "n", "Dummy", 200.0, 100.0);
    set_av(&mut c, &n, "Health", 200.0);
    c.wait(1.0);
    cast(&mut c, "Recheck", &n, &n);
    c.wait(1.5);
    fortify_state(&mut c, &r, &n, "cast at health 200");
    let with = c.method("Actor", n.clone(), "GetActorValue", [string("TwoHanded")]);
    let with_v = c.local("withTwoHanded", "Float");
    c.assign(with_v.clone(), with);
    c.method(
        "Actor",
        n.clone(),
        "DamageActorValue",
        [string("Health"), 150.0f32.into()],
    );
    let gone = poll(&mut c, "gone", 0.1, 6.0, |c, hit| {
        let v = c.method("Actor", n.clone(), "GetActorValue", [string("TwoHanded")]);
        let lower = c.calc(op::CMP_LT, "Bool", v, with_v.clone());
        c.jmpt(lower, hit);
    });
    put_num(
        &mut c,
        &r,
        "health to 50: the bonus gone after (s; -1 not within 6) ",
        gone,
    );
    line(&mut c, &r, "\n");
    c.method(
        "Actor",
        n.clone(),
        "RestoreActorValue",
        [string("Health"), 150.0f32.into()],
    );
    let back = poll(&mut c, "back", 0.1, 6.0, |c, hit| {
        let v = c.method("Actor", n.clone(), "GetActorValue", [string("TwoHanded")]);
        let again = c.calc(op::CMP_GE, "Bool", v, with_v.clone());
        c.jmpt(again, hit);
    });
    put_num(
        &mut c,
        &r,
        "health back to 200: the bonus back after (s; -1 not within 6) ",
        back,
    );
    line(&mut c, &r, "\n");
    remove(&mut c, &n);
    c.finish(r)
}

/// Health lost by `t` to `spell` cast by `caster`, as a line.
fn damage_line(c: &mut Code, r: &Arg, spell: &str, caster: &Arg, t: &Arg, label: &str) {
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

fn test_resistance() -> FunctionBuilder {
    let (mut c, r) = report();
    let (caster, t) = caster_and_target(&mut c, "Dummy", 5000.0);
    line(&mut c, &r, "a 50 point fire spell at an NPC:\n");
    for fire in [0.0, 50.0, 90.0, 100.0, 200.0] {
        force_av(&mut c, &t, "FireResist", fire);
        damage_line(
            &mut c,
            &r,
            "Blast",
            &caster,
            &t,
            &format!("  fire resistance {fire}"),
        );
    }
    force_av(&mut c, &t, "FireResist", 90.0);
    damage_line(
        &mut c,
        &r,
        "BlastIgnore",
        &caster,
        &t,
        "  fire resistance 90, the spell ignoring resistance",
    );
    force_av(&mut c, &t, "FireResist", 50.0);
    force_av(&mut c, &t, "MagicResist", 50.0);
    damage_line(
        &mut c,
        &r,
        "Blast",
        &caster,
        &t,
        "  fire and magic resistance 50",
    );
    // The player's cap.
    let player = auto_var("Player");
    let fire_was = c.method(
        "Actor",
        player.clone(),
        "GetActorValue",
        [string("FireResist")],
    );
    let fire_was_v = c.local("fireWas", "Float");
    c.assign(fire_was_v.clone(), fire_was);
    line(&mut c, &r, "at the player:\n");
    for fire in [90.0, 100.0] {
        force_av(&mut c, &player, "FireResist", fire);
        damage_line(
            &mut c,
            &r,
            "Blast",
            &caster,
            &player,
            &format!("  fire resistance {fire}"),
        );
        c.method(
            "Actor",
            player.clone(),
            "RestoreActorValue",
            [string("Health"), 1000.0f32.into()],
        );
    }
    c.method(
        "Actor",
        player,
        "ForceActorValue",
        [string("FireResist"), fire_was_v],
    );
    c.method("Actor", caster.clone(), "StopCombat", []);
    remove(&mut c, &caster);
    remove(&mut c, &t);
    c.finish(r)
}

fn test_target_values() -> FunctionBuilder {
    let (mut c, r) = report();
    line(
        &mut c,
        &r,
        "aggression, confidence before and 1.5 s after each spell (NPCs \
         aggressive 1, confidence 2)\n",
    );
    let caster = fresh(&mut c, "caster", "Caster", 150.0, 0.0);
    for spell in ["Calm", "Fear", "Fury", "Courage"] {
        for base in ["Level5", "Level20"] {
            let n = fresh(&mut c, "n", base, 450.0, 0.0);
            c.wait(1.0);
            let state = |c: &mut Code, r: &Arg, label: &str| {
                let a = c.method("Actor", n.clone(), "GetActorValue", [string("Aggression")]);
                let k = c.method("Actor", n.clone(), "GetActorValue", [string("Confidence")]);
                put_num(c, r, label, a);
                put_num(c, r, "/", k);
            };
            state(&mut c, &r, &format!("{spell} on {base}: "));
            cast(&mut c, spell, &caster, &n);
            c.wait(1.5);
            state(&mut c, &r, " -> ");
            line(&mut c, &r, "\n");
            c.method("Actor", n.clone(), "StopCombat", []);
            remove(&mut c, &n);
        }
    }
    c.method("Actor", caster.clone(), "StopCombat", []);
    remove(&mut c, &caster);
    c.finish(r)
}

fn test_absorb() -> FunctionBuilder {
    let (mut c, r) = report();
    let (caster, t) = caster_and_target(&mut c, "Dummy", 500.0);
    set_av(&mut c, &caster, "Health", 200.0);
    c.method(
        "Actor",
        caster.clone(),
        "DamageActorValue",
        [string("Health"), 100.0f32.into()],
    );
    c.wait(0.5);
    let state = |c: &mut Code, r: &Arg, label: &str| {
        let hc = health(c, &caster);
        let ht = health(c, &t);
        put_num(c, r, &format!("{label}: caster health "), hc);
        put_num(c, r, ", target ", ht);
        line(c, r, "\n");
    };
    state(&mut c, &r, "before");
    cast(&mut c, "Absorb", &caster, &t);
    c.wait(3.0);
    state(&mut c, &r, "after a 20 point absorb health");
    remove(&mut c, &caster);
    remove(&mut c, &t);
    c.finish(r)
}

fn test_bound_weapon() -> FunctionBuilder {
    let (mut c, r) = report();
    let player = auto_var("Player");
    sheathe(&mut c);
    let state = |c: &mut Code, r: &Arg, label: &str| {
        let w = c.method("Actor", player.clone(), "GetEquippedWeapon", [false.into()]);
        let w = name(c, w);
        let on = c.method(
            "Actor",
            player.clone(),
            "HasMagicEffect",
            [auto_var("BoundSwordEffect")],
        );
        put(c, r, &format!("{label}: right hand "), w);
        put(c, r, ", effect on ", on);
        line(c, r, "\n");
    };
    state(&mut c, &r, "before");
    cast(&mut c, "BoundSword", &player, &player);
    c.wait(2.0);
    state(&mut c, &r, "Bound Sword cast on the player, 2 s");
    c.method(
        "ObjectReference",
        player.clone(),
        "AddItem",
        [auto_var("IronSword"), 1.into(), true.into()],
    );
    c.method(
        "Actor",
        player.clone(),
        "EquipItem",
        [auto_var("IronSword"), false.into(), true.into()],
    );
    let ended = poll(&mut c, "ended", 0.1, 5.0, |c, hit| {
        let on = c.method(
            "Actor",
            player.clone(),
            "HasMagicEffect",
            [auto_var("BoundSwordEffect")],
        );
        c.jmpf(on, hit);
    });
    put_num(
        &mut c,
        &r,
        "an iron sword equipped: the effect ended after (s; -1 not within 5) ",
        ended,
    );
    line(&mut c, &r, "\n");
    state(&mut c, &r, "then");
    c.method(
        "Actor",
        player.clone(),
        "DispelSpell",
        [auto_var("BoundSword")],
    );
    c.method(
        "Actor",
        player.clone(),
        "UnequipItem",
        [auto_var("IronSword"), false.into(), true.into()],
    );
    c.method(
        "ObjectReference",
        player,
        "RemoveItem",
        [auto_var("IronSword"), 1.into(), true.into(), Arg::None],
    );
    c.finish(r)
}

fn test_area_radius() -> FunctionBuilder {
    let (mut c, r) = report();
    line(
        &mut c,
        &r,
        "Fireball (40 fire damage, area 15) cast at a centre NPC; health lost by each:\n",
    );
    let centre = fresh(&mut c, "centre", "Dummy", 600.0, 0.0);
    let mut ring = vec![("centre".to_owned(), centre.clone())];
    for d in [100.0f32, 200.0, 300.0, 350.0, 400.0, 500.0] {
        let a = fresh(&mut c, &format!("ring{}", d as u32), "Dummy", 600.0, d);
        ring.push((format!("{d} units off"), a));
    }
    for (_, a) in &ring {
        set_av(&mut c, a, "Health", 5000.0);
        restrain(&mut c, a);
    }
    let caster = fresh(&mut c, "caster", "Caster", 300.0, 0.0);
    set_av(&mut c, &caster, "Health", 5000.0);
    c.wait(1.5);
    let mut before = Vec::new();
    for (label, a) in ring
        .iter()
        .chain(std::iter::once(&("the caster".to_owned(), caster.clone())))
    {
        let h = health(&mut c, a);
        let v = c.local(&format!("before{}", before.len()), "Float");
        c.assign(v.clone(), h);
        before.push((label.clone(), a.clone(), v));
    }
    cast(&mut c, "Fireball", &caster, &centre);
    c.wait(4.0);
    for (label, a, b) in before {
        let h = health(&mut c, &a);
        let lost = c.calc(op::FSUB, "Float", b, h);
        let d = c.method(
            "ObjectReference",
            centre.clone(),
            "GetDistance",
            [a.clone()],
        );
        put_num(&mut c, &r, &format!("  {label} (distance "), d);
        put_num(&mut c, &r, "): ", lost);
        line(&mut c, &r, "\n");
        c.method("Actor", a.clone(), "StopCombat", []);
        remove(&mut c, &a);
    }
    c.finish(r)
}

/// Health a held NPC loses to the player's `Blast`.
fn player_blast(c: &mut Code, r: &Arg, t: &Arg, label: &str) {
    let player = auto_var("Player");
    damage_line(c, r, "Blast", &player, t, label);
}

fn test_magnitude_perk() -> FunctionBuilder {
    let (mut c, r) = report();
    let t = fresh(&mut c, "t", "Dummy", 450.0, 0.0);
    set_av(&mut c, &t, "Health", 5000.0);
    restrain(&mut c, &t);
    c.wait(1.0);
    let player = auto_var("Player");
    player_blast(&mut c, &r, &t, "the player's 50 point fire spell");
    c.method(
        "Actor",
        player.clone(),
        "AddPerk",
        [auto_var("AugmentedFlames")],
    );
    player_blast(&mut c, &r, &t, "with Augmented Flames");
    c.method("Actor", player, "RemovePerk", [auto_var("AugmentedFlames")]);
    remove(&mut c, &t);
    c.finish(r)
}

fn test_perk_priority() -> FunctionBuilder {
    let (mut c, r) = report();
    let t = fresh(&mut c, "t", "Dummy", 450.0, 0.0);
    set_av(&mut c, &t, "Health", 5000.0);
    restrain(&mut c, &t);
    c.wait(1.0);
    let player = auto_var("Player");
    player_blast(&mut c, &r, &t, "the player's 50 point fire spell");
    for (perks, label) in [
        (
            ["PerkAdd", "PerkMul"],
            "Mod Spell Magnitude +10 at priority 0, x2 at 1 (60 x 2 = 120 or 50 x 2 + 10 = 110)",
        ),
        (
            ["PerkAddLate", "PerkMulEarly"],
            "+10 at priority 1, x2 at 0",
        ),
    ] {
        for p in perks {
            c.method("Actor", player.clone(), "AddPerk", [auto_var(p)]);
        }
        player_blast(&mut c, &r, &t, label);
        for p in perks {
            c.method("Actor", player.clone(), "RemovePerk", [auto_var(p)]);
        }
    }
    remove(&mut c, &t);
    c.finish(r)
}

fn test_perk_sections() -> FunctionBuilder {
    let (mut c, r) = report();
    let player = auto_var("Player");
    let q = auto_var("PerkStageQuest");
    let quest_state = |c: &mut Code, r: &Arg, label: &str| {
        let stage = c.method("Quest", q.clone(), "GetStage", []);
        let done = c.method("Quest", q.clone(), "IsStageDone", [10.into()]);
        let running = c.method("Quest", q.clone(), "IsRunning", []);
        put(c, r, &format!("{label}: stage "), stage);
        put(c, r, ", stage 10 done ", done);
        put(c, r, ", running ", running);
        line(c, r, "\n");
    };
    line(
        &mut c,
        &r,
        "a perk with a quest section (our quest, not running, stage 10):\n",
    );
    quest_state(&mut c, &r, "  before");
    c.method("Actor", player.clone(), "AddPerk", [auto_var("PerkQuest")]);
    c.wait(1.0);
    quest_state(&mut c, &r, "  added");
    c.method(
        "Actor",
        player.clone(),
        "RemovePerk",
        [auto_var("PerkQuest")],
    );
    c.wait(1.0);
    quest_state(&mut c, &r, "  removed");
    c.method("Quest", q, "Stop", []);
    let ability_state = |c: &mut Code, r: &Arg, label: &str| {
        let has = c.method("Actor", player.clone(), "HasSpell", [auto_var("Ability")]);
        let one = c.method(
            "Actor",
            player.clone(),
            "GetActorValue",
            [string("OneHanded")],
        );
        put(c, r, &format!("{label}: has the ability "), has);
        put_num(c, r, ", OneHanded ", one);
        line(c, r, "\n");
    };
    line(
        &mut c,
        &r,
        "a perk with an ability section (our ability: +7 OneHanded):\n",
    );
    ability_state(&mut c, &r, "  before");
    c.method(
        "Actor",
        player.clone(),
        "AddPerk",
        [auto_var("PerkAbility")],
    );
    c.wait(1.0);
    ability_state(&mut c, &r, "  added");
    c.method(
        "Actor",
        player.clone(),
        "RemovePerk",
        [auto_var("PerkAbility")],
    );
    c.wait(1.0);
    ability_state(&mut c, &r, "  removed");
    c.finish(r)
}

/// The skills `AdvanceSkill` is tried on, at a level each.
const ADVANCED: &[(&str, f32)] = &[
    ("OneHanded", 20.0),
    ("Lockpicking", 20.0),
    ("OneHanded", 50.0),
];

fn test_advance_skill() -> FunctionBuilder {
    let (mut c, r) = report();
    let player = auto_var("Player");
    line(
        &mut c,
        &r,
        "AdvanceSkill(skill, 10) calls until the skill rises, twice (at most 800 calls each):\n",
    );
    for (i, &(skill, level)) in ADVANCED.iter().enumerate() {
        let was = c.method(
            "Actor",
            player.clone(),
            "GetBaseActorValue",
            [string(skill)],
        );
        let was_v = c.local(&format!("was{i}"), "Float");
        c.assign(was_v.clone(), was);
        set_av(&mut c, &player, skill, level);
        c.wait(0.5);
        put(
            &mut c,
            &r,
            &format!("  {skill} set to {level}:"),
            string(""),
        );
        for rise in 0..2 {
            let target = level + rise as f32;
            let calls = c.local(&format!("calls{i}_{rise}"), "Int");
            c.assign(calls.clone(), 0.into());
            let (top, done) = (c.label(), c.label());
            c.bind(top);
            let base = c.method(
                "Actor",
                player.clone(),
                "GetBaseActorValue",
                [string(skill)],
            );
            let risen = c.calc(op::CMP_GT, "Bool", base, target.into());
            c.jmpt(risen, done);
            let too_many = c.calc(op::CMP_GE, "Bool", calls.clone(), 800.into());
            c.jmpt(too_many, done);
            c.global("Game", "AdvanceSkill", [string(skill), 10.0f32.into()]);
            c.op(op::IADD, [calls.clone(), calls.clone(), 1.into()]);
            c.jmp(top);
            c.bind(done);
            put(&mut c, &r, &format!(" to {} after ", target + 1.0), calls);
            line(&mut c, &r, " calls;");
        }
        line(&mut c, &r, "\n");
        c.method(
            "Actor",
            player.clone(),
            "SetActorValue",
            [string(skill), was_v],
        );
    }
    c.finish(r)
}

/// Statistics named, before and after `action`.
fn stats_line(c: &mut Code, r: &Arg, label: &str, stats: &[&str], action: impl FnOnce(&mut Code)) {
    let before: Vec<Arg> = stats
        .iter()
        .map(|s| {
            let v = c.global("Game", "QueryStat", [string(s)]);
            let keep = c.temp("Int");
            c.assign(keep.clone(), v);
            keep
        })
        .collect();
    action(c);
    c.wait(1.0);
    put(c, r, label, string(":"));
    for (stat, before) in stats.iter().zip(before) {
        let after = c.global("Game", "QueryStat", [string(stat)]);
        put(c, r, &format!(" {stat} "), before);
        put(c, r, " to ", after);
    }
    line(c, r, "\n");
}

const SHOUT_STATS: &[&str] = &[
    "Words Of Power Learned",
    "Words Of Power Unlocked",
    "Shouts Learned",
    "Shouts Unlocked",
    "Shouts Mastered",
];

fn test_shouts() -> FunctionBuilder {
    let (mut c, r) = report();
    let player = auto_var("Player");
    line(&mut c, &r, "our shout of three words:\n");
    stats_line(&mut c, &r, "  TeachWord(1)", SHOUT_STATS, |c| {
        c.global("Game", "TeachWord", [auto_var("Word1")]);
    });
    stats_line(&mut c, &r, "  UnlockWord(1)", SHOUT_STATS, |c| {
        c.global("Game", "UnlockWord", [auto_var("Word1")]);
    });
    stats_line(&mut c, &r, "  AddShout", SHOUT_STATS, |c| {
        c.method("Actor", player.clone(), "AddShout", [auto_var("Shout")]);
    });
    stats_line(
        &mut c,
        &r,
        "  TeachWord(2), UnlockWord(2)",
        SHOUT_STATS,
        |c| {
            c.global("Game", "TeachWord", [auto_var("Word2")]);
            c.global("Game", "UnlockWord", [auto_var("Word2")]);
        },
    );
    stats_line(
        &mut c,
        &r,
        "  TeachWord(3), UnlockWord(3)",
        SHOUT_STATS,
        |c| {
            c.global("Game", "TeachWord", [auto_var("Word3")]);
            c.global("Game", "UnlockWord", [auto_var("Word3")]);
        },
    );
    // HasShout for a shout given by script.
    line(
        &mut c,
        &r,
        "a self spell: +10 OneHanded if HasShout(ours), on an NPC:\n",
    );
    let n = fresh(&mut c, "n", "Dummy", 200.0, 100.0);
    c.wait(1.0);
    let one = |c: &mut Code, r: &Arg, label: &str| {
        let v = c.method("Actor", n.clone(), "GetActorValue", [string("OneHanded")]);
        put_num(c, r, label, v);
        line(c, r, "\n");
    };
    one(&mut c, &r, "  OneHanded before ");
    cast(&mut c, "HasShoutSpell", &n, &n);
    c.wait(1.5);
    one(&mut c, &r, "  cast, without the shout ");
    c.method(
        "Actor",
        n.clone(),
        "DispelSpell",
        [auto_var("HasShoutSpell")],
    );
    c.method("Actor", n.clone(), "AddShout", [auto_var("Shout")]);
    c.wait(0.5);
    cast(&mut c, "HasShoutSpell", &n, &n);
    c.wait(1.5);
    one(&mut c, &r, "  AddShout(ours), cast again ");
    remove(&mut c, &n);
    c.finish(r)
}

/// A quest's reference alias `alias`, cast as one.
fn ref_alias(c: &mut Code, quest: &str, alias: i32) -> Arg {
    let a = c.method("Quest", auto_var(quest), "GetAlias", [alias.into()]);
    let ra = c.temp("ReferenceAlias");
    c.cast(ra.clone(), a);
    ra
}

fn test_alias_packages() -> FunctionBuilder {
    let (mut c, r) = report();
    let x = fresh(&mut c, "x", "Packaged", 300.0, 0.0);
    c.wait(1.0);
    let current = |c: &mut Code, r: &Arg, label: &str| {
        c.method("Actor", x.clone(), "EvaluatePackage", []);
        c.wait(1.0);
        let p = c.method("Actor", x.clone(), "GetCurrentPackage", []);
        let p = name(c, p);
        put(c, r, &format!("{label}: the NPC runs "), p);
        line(c, r, "\n");
    };
    line(
        &mut c,
        &r,
        "quests A and B (A's form id lower), priority 50, an alias each forced to one NPC, \
         package A / B:\n",
    );
    let order = |c: &mut Code, r: &Arg, first: &str, second: &str, label: &str| {
        for q in [first, second] {
            c.method("Quest", auto_var(q), "Start", []);
        }
        c.wait(0.5);
        for q in [first, second] {
            let a = ref_alias(c, q, 0);
            c.method("ReferenceAlias", a, "ForceRefTo", [x.clone()]);
        }
        current(c, r, label);
        for q in [first, second] {
            c.method("Quest", auto_var(q), "Stop", []);
        }
        c.wait(1.0);
    };
    order(
        &mut c,
        &r,
        "PackageQuestA",
        "PackageQuestB",
        "  A started and forced first",
    );
    order(
        &mut c,
        &r,
        "PackageQuestB",
        "PackageQuestA",
        "  B started and forced first",
    );
    line(
        &mut c,
        &r,
        "quest C, two aliases (0 then 1) forced to the NPC, package C1 / C2:\n",
    );
    c.method("Quest", auto_var("PackageQuestC"), "Start", []);
    c.wait(0.5);
    for alias in [1, 0] {
        let a = ref_alias(&mut c, "PackageQuestC", alias);
        c.method("ReferenceAlias", a, "ForceRefTo", [x.clone()]);
    }
    current(&mut c, &r, "  alias 1 forced, then alias 0");
    c.method("Quest", auto_var("PackageQuestC"), "Stop", []);
    remove(&mut c, &x);
    c.finish(r)
}

/// `quests`' running states as text.
fn running(c: &mut Code, r: &Arg, quests: &[&str]) {
    for q in quests {
        let on = c.method("Quest", auto_var(q), "IsRunning", []);
        put(c, r, &format!(" {q} running "), on);
    }
}

fn send(c: &mut Code, key: &str, r1: Arg, r2: Arg, v1: i32, v2: i32) -> Arg {
    c.method(
        "Keyword",
        auto_var(key),
        "SendStoryEventAndWait",
        [Arg::None, r1, r2, v1.into(), v2.into()],
    )
}

fn test_story_manager() -> FunctionBuilder {
    let (mut c, r) = report();
    let player = auto_var("Player");
    let npc = fresh(&mut c, "npc", "StoryActor", 300.0, 0.0);
    c.wait(1.0);

    // On whom node conditions run.
    line(
        &mut c,
        &r,
        "script event, R1 our NPC: a node needing GetIsID(our NPC), then one needing \
         GetIsID(the player), both run on the subject:\n",
    );
    let ok = send(&mut c, "KeySubject", npc.clone(), Arg::None, 0, 0);
    c.wait(1.0);
    put(&mut c, &r, "  started ", ok);
    running(&mut c, &r, &["StoryNpcQuest", "StoryPlayerQuest"]);
    line(&mut c, &r, "\n");
    for q in ["StoryNpcQuest", "StoryPlayerQuest"] {
        c.method("Quest", auto_var(q), "Stop", []);
    }

    // The members the quest's OnStoryScript gets.
    let ok = send(&mut c, "KeyMembers", npc.clone(), player.clone(), 7, 42);
    c.wait(1.5);
    let members = c.temp("String");
    c.call_method(auto_var("MembersQuest"), "GetMembers", members.clone(), []);
    put(
        &mut c,
        &r,
        "SendStoryEvent(None, our NPC, the player, 7, 42): started ",
        ok,
    );
    put(&mut c, &r, "; OnStoryScript got ", members);
    line(&mut c, &r, "\n");
    c.method("Quest", auto_var("MembersQuest"), "Stop", []);

    // Reset hours: our node's quest may run again an hour after.
    line(
        &mut c,
        &r,
        "a node whose quest may run again after 1 hour:\n",
    );
    let reset = |c: &mut Code, r: &Arg, label: &str| {
        let ok = send(c, "KeyReset", Arg::None, Arg::None, 0, 0);
        c.wait(0.5);
        put(c, r, &format!("  {label}: started "), ok);
        running(c, r, &["ResetQuest"]);
        line(c, r, "\n");
        c.method("Quest", auto_var("ResetQuest"), "Stop", []);
        c.wait(0.5);
    };
    reset(&mut c, &r, "first");
    reset(&mut c, &r, "stopped, at once again");
    let time_scale = auto_var("TimeScale");
    let was = c.method("GlobalVariable", time_scale.clone(), "GetValue", []);
    let was_v = c.local("timeScaleWas", "Float");
    c.assign(was_v.clone(), was);
    c.method(
        "GlobalVariable",
        time_scale.clone(),
        "SetValue",
        [1200.0f32.into()],
    );
    c.wait(4.0);
    c.method("GlobalVariable", time_scale, "SetValue", [was_v]);
    reset(&mut c, &r, "1.3 game hours later");

    // Most running: a node with two quests, at most 1.
    line(
        &mut c,
        &r,
        "a node with two quests and at most 1 running:\n",
    );
    for label in ["first", "again, the first still running"] {
        let ok = send(&mut c, "KeyMax", Arg::None, Arg::None, 0, 0);
        c.wait(0.5);
        put(&mut c, &r, &format!("  {label}: started "), ok);
        running(&mut c, &r, &["MaxQuest1", "MaxQuest2"]);
        line(&mut c, &r, "\n");
    }
    for q in ["MaxQuest1", "MaxQuest2"] {
        c.method("Quest", auto_var(q), "Stop", []);
    }

    // What SendStoryEventAndWait waits for: the quest's OnStoryScript waits
    // 2 s then marks itself done.
    let start = now(&mut c);
    let start_v = c.local("sendStart", "Float");
    c.assign(start_v.clone(), start);
    let ok = send(&mut c, "KeyWait", Arg::None, Arg::None, 0, 0);
    let took = now(&mut c);
    let took = c.calc(op::FSUB, "Float", took, start_v);
    let done = c.temp("Bool");
    c.call_method(auto_var("WaitQuest"), "IsDone", done.clone(), []);
    put(
        &mut c,
        &r,
        "SendStoryEventAndWait, its quest's OnStoryScript waiting 2 s: returned ",
        ok,
    );
    put_num(&mut c, &r, " after ", took);
    put(&mut c, &r, " s, the handler done then ", done);
    c.wait(3.0);
    let done = c.temp("Bool");
    c.call_method(auto_var("WaitQuest"), "IsDone", done.clone(), []);
    put(&mut c, &r, ", 3 s later ", done);
    line(&mut c, &r, "\n");
    c.method("Quest", auto_var("WaitQuest"), "Stop", []);
    remove(&mut c, &npc);
    c.finish(r)
}

fn test_scenes() -> FunctionBuilder {
    let (mut c, r) = report();
    let quest = auto_var("SceneQuest");
    let playing = |c: &mut Code, r: &Arg, scene: &str, label: &str| {
        let p = c.method("Scene", auto_var(scene), "IsPlaying", []);
        put(c, r, &format!("{label}: {scene} playing "), p);
        line(c, r, "\n");
    };
    let start = |c: &mut Code, scene: &str| {
        c.method("Scene", auto_var(scene), "Start", []);
        c.wait(0.5);
    };
    // Start refused?
    start(&mut c, "ScenePlain");
    playing(&mut c, &r, "ScenePlain", "Start, the quest not running");
    c.method("Scene", auto_var("ScenePlain"), "Stop", []);
    c.method("Quest", quest.clone(), "Start", []);
    c.wait(0.5);
    start(&mut c, "ScenePlain");
    playing(
        &mut c,
        &r,
        "ScenePlain",
        "Start, the quest running, its alias empty",
    );
    c.method("Scene", auto_var("ScenePlain"), "Stop", []);
    let a = fresh(&mut c, "a", "SceneActor", 250.0, 0.0);
    let alias = ref_alias(&mut c, "SceneQuest", 0);
    c.method("ReferenceAlias", alias, "ForceRefTo", [a.clone()]);
    c.wait(0.5);
    start(&mut c, "ScenePlain");
    playing(
        &mut c,
        &r,
        "ScenePlain",
        "Start, the alias filled (a 3 s timer)",
    );
    c.wait(4.0);
    playing(&mut c, &r, "ScenePlain", "  4 s later");
    // The actor in another scene.
    start(&mut c, "SceneLong");
    playing(&mut c, &r, "SceneLong", "SceneLong started (30 s)");
    start(&mut c, "ScenePlain");
    playing(
        &mut c,
        &r,
        "ScenePlain",
        "  Start of another scene of the same actor",
    );
    c.method("Scene", auto_var("ScenePlain"), "ForceStart", []);
    c.wait(0.5);
    playing(&mut c, &r, "ScenePlain", "  ForceStart");
    playing(&mut c, &r, "SceneLong", "  then");
    for s in ["ScenePlain", "SceneLong"] {
        c.method("Scene", auto_var(s), "Stop", []);
    }
    c.wait(1.0);
    // When a phase ends: its completion conditions, its start conditions.
    let ends = |c: &mut Code, r: &Arg, scene: &str, label: &str, stage_at: Option<f32>| {
        start(c, scene);
        if let Some(t) = stage_at {
            c.wait(t);
            c.method("Quest", auto_var("SceneQuest"), "SetStage", [10.into()]);
        }
        let ended = poll(c, &format!("ended{scene}"), 0.25, 25.0, |c, hit| {
            let p = c.method("Scene", auto_var(scene), "IsPlaying", []);
            c.jmpf(p, hit);
        });
        put_num(
            c,
            r,
            &format!("{label}: ended after (s; -1 not within 25) "),
            ended,
        );
        line(c, r, "\n");
        c.method("Scene", auto_var(scene), "Stop", []);
    };
    ends(
        &mut c,
        &r,
        "SceneComplete",
        "a 20 s timer, the phase completing on GetStage >= 10, stage 10 set 2 s in",
        Some(2.0),
    );
    ends(
        &mut c,
        &r,
        "SceneSkip",
        "a 20 s timer, the phase starting on GetStage >= 99 (never)",
        None,
    );
    // Death ends it (the actor's behaviour flag).
    start(&mut c, "SceneDeath");
    playing(
        &mut c,
        &r,
        "SceneDeath",
        "SceneDeath started (20 s, death ends it)",
    );
    c.method("Actor", a.clone(), "Kill", [Arg::None]);
    c.wait(1.0);
    playing(&mut c, &r, "SceneDeath", "  the actor killed, 1 s");
    c.method("Scene", auto_var("SceneDeath"), "Stop", []);
    c.method("Quest", quest, "Stop", []);
    remove(&mut c, &a);
    // Stop quest on end.
    let b = fresh(&mut c, "b", "SceneActor", 250.0, 0.0);
    c.method("Quest", auto_var("SceneQuestStop"), "Start", []);
    c.wait(0.5);
    let alias = ref_alias(&mut c, "SceneQuestStop", 0);
    c.method("ReferenceAlias", alias, "ForceRefTo", [b.clone()]);
    c.wait(0.5);
    start(&mut c, "SceneStopQuest");
    c.wait(5.0);
    let on = c.method("Quest", auto_var("SceneQuestStop"), "IsRunning", []);
    put(
        &mut c,
        &r,
        "a 3 s scene flagged to stop its quest on end, 5 s later: the quest running ",
        on,
    );
    line(&mut c, &r, "\n");
    c.method("Quest", auto_var("SceneQuestStop"), "Stop", []);
    remove(&mut c, &b);
    c.finish(r)
}

/// What an actor is doing in a fight: its state and target.
fn fight_line(c: &mut Code, r: &Arg, a: &Arg, label: &str) {
    let state = c.method("Actor", a.clone(), "GetCombatState", []);
    let target = c.method("Actor", a.clone(), "GetCombatTarget", []);
    let target = name(c, target);
    put(c, r, &format!("{label}: combat state "), state);
    put(c, r, ", target ", target);
    line(c, r, "\n");
}

/// The assistance tests: helper base, how far from the fight.
const HELPERS: &[(&str, f32, &str)] = &[
    (
        "HelperAllyAllies",
        500.0,
        "ally of the victim, helps allies",
    ),
    ("HelperAllyAllies", 2500.0, "ally, helps allies"),
    ("HelperAllyAllies", 5000.0, "ally, helps allies"),
    ("HelperAllyNobody", 500.0, "ally, helps nobody"),
    (
        "HelperFriendAllies",
        500.0,
        "friend of the victim, helps allies",
    ),
    (
        "HelperFriendFriends",
        500.0,
        "friend, helps friends and allies",
    ),
];

fn test_assistance() -> FunctionBuilder {
    let (mut c, r) = report();
    line(
        &mut c,
        &r,
        "a victim, an enemy attacking it and a helper placed; the helper 8 s later:\n",
    );
    for (i, &(base, distance, label)) in HELPERS.iter().enumerate() {
        let v = fresh(&mut c, &format!("v{i}"), "Victim", 400.0, 0.0);
        c.assign(id("VictimRef"), v.clone());
        let e = fresh(&mut c, &format!("e{i}"), "Enemy", 300.0, 0.0);
        c.assign(id("EnemyRef"), e.clone());
        let h = fresh(&mut c, &format!("h{i}"), base, 400.0, distance);
        c.assign(id("HelperRef"), h.clone());
        for a in [&v, &e, &h] {
            set_av(&mut c, a, "Health", 3000.0);
        }
        c.wait(1.0);
        c.method("Actor", e.clone(), "StartCombat", [v.clone()]);
        c.wait(8.0);
        let d = c.method("ObjectReference", h.clone(), "GetDistance", [v.clone()]);
        put_num(
            &mut c,
            &r,
            &format!("  {label}, {distance} units off (now "),
            d,
        );
        line(&mut c, &r, ")");
        fight_line(&mut c, &r, &h, "");
        for a in [&v, &e, &h] {
            c.method("Actor", a.clone(), "StopCombat", []);
            remove(&mut c, a);
        }
        c.wait(1.0);
    }
    c.finish(r)
}

fn test_friendly_hits() -> FunctionBuilder {
    let (mut c, r) = report();
    let a = fresh(&mut c, "a", "GroupMember", 250.0, 0.0);
    let f = fresh(&mut c, "f", "GroupMember", 450.0, 0.0);
    c.assign(id("CasterRef"), a.clone());
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
    fight_line(&mut c, &r, &f, ", the struck one");
    let target = c.method("Actor", f.clone(), "GetCombatTarget", []);
    let target = as_form(&mut c, target);
    let caster = as_form(&mut c, a.clone());
    let fights = c.calc(op::CMP_EQ, "Bool", target, caster);
    c.jmpt(fights, done);
    c.jmp(top);
    c.bind(done);
    for x in [&a, &f] {
        c.method("Actor", x.clone(), "StopCombat", []);
        remove(&mut c, x);
    }
    c.finish(r)
}

fn test_combat_regen() -> FunctionBuilder {
    let (mut c, r) = report();
    let g = fresh(&mut c, "g", "Victim", 300.0, 0.0);
    let calm = fresh(&mut c, "calm", "Dummy", 300.0, 200.0);
    let e = fresh(&mut c, "e", "Enemy", 300.0, 1500.0);
    for x in [&g, &calm] {
        set_av(&mut c, x, "Health", 300.0);
        restrain(&mut c, x);
    }
    restrain(&mut c, &e);
    c.wait(1.0);
    for x in [&g, &calm] {
        c.method(
            "Actor",
            x.clone(),
            "DamageActorValue",
            [string("Health"), 210.0f32.into()],
        );
    }
    c.method("Actor", g.clone(), "StartCombat", [e.clone()]);
    line(
        &mut c,
        &r,
        "two NPCs at 90 of 300 health, held in place; one fighting an enemy 1500 units off:\n",
    );
    let sample = |c: &mut Code, r: &Arg, label: &str| {
        let state = c.method("Actor", g.clone(), "GetCombatState", []);
        let hg = health(c, &g);
        let hc = health(c, &calm);
        put(c, r, &format!("  {label}: fighting one (state "), state);
        put_num(c, r, ") ", hg);
        put_num(c, r, ", the other ", hc);
        line(c, r, "\n");
    };
    sample(&mut c, &r, "0 s");
    for t in [5, 10, 15, 20] {
        c.wait(5.0);
        sample(&mut c, &r, &format!("{t} s"));
    }
    c.method("Actor", g.clone(), "StopCombat", []);
    c.method("Actor", e.clone(), "StopCombat", []);
    remove(&mut c, &e);
    for t in [5, 10, 15, 20] {
        c.wait(5.0);
        sample(&mut c, &r, &format!("StopCombat, {t} s"));
    }
    remove(&mut c, &g);
    remove(&mut c, &calm);
    c.finish(r)
}

fn test_flee() -> FunctionBuilder {
    let (mut c, r) = report();
    line(
        &mut c,
        &r,
        "a cowardly NPC set on by a level 50 brute (held in place): its distance from it each second\n",
    );
    for (i, harmed) in [false, true].into_iter().enumerate() {
        let coward = fresh(&mut c, &format!("coward{i}"), "Coward", 450.0, 0.0);
        let brute = fresh(&mut c, &format!("brute{i}"), "Brute", 300.0, 0.0);
        set_av(&mut c, &brute, "Health", 5000.0);
        restrain(&mut c, &brute);
        c.wait(1.0);
        if harmed {
            c.method(
                "Actor",
                coward.clone(),
                "DamageActorValue",
                [string("Health"), 1.0f32.into()],
            );
        }
        c.method("Actor", brute.clone(), "StartCombat", [coward.clone()]);
        let label = if harmed {
            "  harmed by 1 first:"
        } else {
            "  unharmed:"
        };
        put(&mut c, &r, label, string(""));
        for _ in 0..8 {
            c.wait(1.0);
            let d = c.method(
                "ObjectReference",
                coward.clone(),
                "GetDistance",
                [brute.clone()],
            );
            put_num(&mut c, &r, " ", d);
        }
        let state = c.method("Actor", coward.clone(), "GetCombatState", []);
        put(&mut c, &r, "; its combat state ", state);
        line(&mut c, &r, "\n");
        for x in [&coward, &brute] {
            c.method("Actor", x.clone(), "StopCombat", []);
            remove(&mut c, x);
        }
        c.wait(1.0);
    }
    c.finish(r)
}

fn test_dual_wield() -> FunctionBuilder {
    let (mut c, r) = report();
    let bag = fresh(&mut c, "bag", "Bag", 550.0, 0.0);
    set_av(&mut c, &bag, "Health", 5000.0);
    restrain(&mut c, &bag);
    for (i, (base, shield, label)) in [
        ("Dual", false, "dual wielding combat style, two iron swords"),
        (
            "Dual",
            true,
            "dual wielding combat style, two swords and a shield",
        ),
        ("DualPlain", false, "the default combat style, two swords"),
    ]
    .into_iter()
    .enumerate()
    {
        let d = fresh(&mut c, &format!("d{i}"), base, 400.0, 0.0);
        c.method(
            "ObjectReference",
            d.clone(),
            "AddItem",
            [auto_var("IronSword"), 2.into(), true.into()],
        );
        if shield {
            c.method(
                "ObjectReference",
                d.clone(),
                "AddItem",
                [auto_var("IronShield"), 1.into(), true.into()],
            );
        }
        c.wait(1.0);
        c.method("Actor", d.clone(), "StartCombat", [bag.clone()]);
        c.wait(6.0);
        let right = c.method("Actor", d.clone(), "GetEquippedWeapon", [false.into()]);
        let right = name(&mut c, right);
        let left = c.method("Actor", d.clone(), "GetEquippedWeapon", [true.into()]);
        let left = name(&mut c, left);
        let held_shield = c.method("Actor", d.clone(), "GetEquippedShield", []);
        let held_shield = name(&mut c, held_shield);
        let left_type = c.method("Actor", d.clone(), "GetEquippedItemType", [0.into()]);
        put(&mut c, &r, &format!("{label}, 6 s fighting: right "), right);
        put(&mut c, &r, ", left ", left);
        put(&mut c, &r, " (type ", left_type);
        put(&mut c, &r, "), shield ", held_shield);
        line(&mut c, &r, "\n");
        c.method("Actor", d.clone(), "StopCombat", []);
        remove(&mut c, &d);
    }
    remove(&mut c, &bag);
    c.finish(r)
}

fn test_torches() -> FunctionBuilder {
    let (mut c, r) = report();
    let hour = auto_var("GameHour");
    let was = c.method("GlobalVariable", hour.clone(), "GetValue", []);
    let was_v = c.local("hourWas", "Float");
    c.assign(was_v.clone(), was);
    c.method("GlobalVariable", hour.clone(), "SetValue", [23.0f32.into()]);
    c.wait(2.0);
    let level = c.method("Actor", auto_var("Player"), "GetLightLevel", []);
    put_num(&mut c, &r, "at 23:00 (light level at the player ", level);
    line(
        &mut c,
        &r,
        "), NPCs given a torch and a weapon, 10 s later:\n",
    );
    for (i, (weapon, label)) in [
        ("IronSword", "an iron sword"),
        ("IronGreatsword", "an iron greatsword"),
    ]
    .into_iter()
    .enumerate()
    {
        let n = fresh(&mut c, &format!("n{i}"), "Dummy", 300.0, 100.0 * i as f32);
        for item in ["Torch", weapon] {
            c.method(
                "ObjectReference",
                n.clone(),
                "AddItem",
                [auto_var(item), 1.into(), true.into()],
            );
        }
        c.wait(10.0);
        let left = c.method("Actor", n.clone(), "GetEquippedItemType", [0.into()]);
        let right = c.method("Actor", n.clone(), "GetEquippedItemType", [1.into()]);
        put(&mut c, &r, &format!("  {label}: left hand type "), left);
        put(&mut c, &r, ", right ", right);
        line(&mut c, &r, "\n");
        remove(&mut c, &n);
    }
    c.method("GlobalVariable", hour, "SetValue", [was_v]);
    c.finish(r)
}

/// The player into the test cell, a marker left where they stood.
fn enter_cell(c: &mut Code) {
    let player = auto_var("Player");
    let marker = c.method(
        "ObjectReference",
        player.clone(),
        "PlaceAtMe",
        [auto_var("XMarker"), 1.into(), true.into(), false.into()],
    );
    c.assign(id("StartMarker"), marker);
    c.method(
        "ObjectReference",
        player,
        "MoveTo",
        [
            auto_var("EntryRef"),
            0.0f32.into(),
            0.0f32.into(),
            0.0f32.into(),
            true.into(),
        ],
    );
    c.wait(5.0);
}

fn test_alias_fills() -> FunctionBuilder {
    let (mut c, r) = report();
    enter_cell(&mut c);
    // Before the quest starts: the forced reference disabled, the unique
    // actor dead, two matching NPCs placed (the second nearer the player).
    c.method(
        "ObjectReference",
        auto_var("ForcedRef"),
        "Disable",
        [false.into()],
    );
    c.method("Actor", auto_var("UniqueRef"), "Kill", [Arg::None]);
    let m1 = fresh(&mut c, "m1", "Matchable", 500.0, 0.0);
    c.assign(id("MatchRef1"), m1.clone());
    let m2 = fresh(&mut c, "m2", "Matchable", 200.0, 0.0);
    c.assign(id("MatchRef2"), m2.clone());
    c.wait(1.0);
    let ok = c.method("Quest", auto_var("FillQuest"), "Start", []);
    c.wait(1.0);
    put(&mut c, &r, "the quest started ", ok);
    line(&mut c, &r, "\n");
    for (alias, label, level) in FILL_ALIASES {
        let a = ref_alias(&mut c, "FillQuest", *alias as i32);
        let got = c.method("ReferenceAlias", a.clone(), "GetReference", []);
        let got_name = name(&mut c, got.clone());
        put(&mut c, &r, &format!("  {label}: "), got_name);
        if *level {
            let actor = c.method("ReferenceAlias", a, "GetActorReference", []);
            let lvl = c.method("Actor", actor, "GetLevel", []);
            put(&mut c, &r, ", level ", lvl);
        }
        line(&mut c, &r, "\n");
    }
    c.method("Quest", auto_var("FillQuest"), "Stop", []);
    c.method(
        "ObjectReference",
        auto_var("ForcedRef"),
        "Enable",
        [false.into()],
    );
    remove(&mut c, &m1);
    remove(&mut c, &m2);
    c.finish(r)
}

/// The fill quest's aliases reported: id, label, whether the level is read.
const FILL_ALIASES: &[(u32, &str, bool)] = &[
    (1, "forced to a reference disabled first", false),
    (2, "a unique actor killed first", false),
    (3, "a unique actor with no reference", false),
    (
        4,
        "created at the player (PC level x1), level easy (0)",
        true,
    ),
    (5, "created, level medium (1)", true),
    (6, "created, level very hard (3)", true),
    (
        7,
        "the first matching (GetIsID ours; placed 500 then 200 from the player)",
        false,
    ),
    (8, "the closest matching", false),
];

fn test_triggers() -> FunctionBuilder {
    let (mut c, r) = report();
    let trigger = auto_var("TriggerRef");
    c.assign(id("TrigLog"), string(""));
    let at = |c: &mut Code, who: &Arg, x: f32| {
        c.method(
            "ObjectReference",
            who.clone(),
            "MoveTo",
            [
                trigger.clone(),
                x.into(),
                0.0f32.into(),
                0.0f32.into(),
                false.into(),
            ],
        );
        c.wait(1.5);
    };
    let step = |c: &mut Code, r: &Arg, label: &str| {
        let count = c.method(
            "ObjectReference",
            trigger.clone(),
            "GetTriggerObjectCount",
            [],
        );
        put(c, r, &format!("{label} (count "), count);
        put(c, r, "):\n", id("TrigLog"));
        c.assign(id("TrigLog"), string(""));
    };
    put_num(
        &mut c,
        &r,
        "a box trigger of half extent ",
        TRIGGER_HALF.into(),
    );
    line(&mut c, &r, " units, its events (with the count then):\n");
    let a = fresh(&mut c, "a", "Dummy", 0.0, 300.0);
    c.assign(id("TrigA"), a.clone());
    c.wait(1.0);
    at(&mut c, &a, 0.0);
    step(&mut c, &r, "an NPC moved into the middle");
    at(&mut c, &a, 350.0);
    step(&mut c, &r, "moved 350 off");
    for margin in [40.0f32, 20.0, 10.0, 0.0] {
        at(&mut c, &a, TRIGGER_HALF + margin);
        step(
            &mut c,
            &r,
            &format!("moved to {margin} units outside the box's side"),
        );
        at(&mut c, &a, 350.0);
        c.assign(id("TrigLog"), string(""));
    }
    at(&mut c, &a, 0.0);
    c.method(
        "ObjectReference",
        trigger.clone(),
        "Disable",
        [false.into()],
    );
    c.wait(1.0);
    c.method("ObjectReference", trigger.clone(), "Enable", [false.into()]);
    c.wait(1.5);
    step(&mut c, &r, "inside, the trigger disabled and enabled");
    let b = fresh(&mut c, "b", "Dummy", 0.0, -300.0);
    c.assign(id("TrigB"), b.clone());
    at(&mut c, &b, 0.0);
    c.method("Actor", b.clone(), "Kill", [Arg::None]);
    c.wait(1.5);
    step(&mut c, &r, "another NPC moved in and killed");
    let item = c.method(
        "ObjectReference",
        trigger.clone(),
        "PlaceAtMe",
        [auto_var("IronSword"), 1.into(), false.into(), false.into()],
    );
    c.assign(id("TrigItem"), item);
    c.wait(2.0);
    step(&mut c, &r, "an iron sword placed in it");
    let player = auto_var("Player");
    at(&mut c, &player, 0.0);
    step(&mut c, &r, "the player moved in");
    c.method(
        "ObjectReference",
        player,
        "MoveTo",
        [
            auto_var("EntryRef"),
            0.0f32.into(),
            0.0f32.into(),
            0.0f32.into(),
            true.into(),
        ],
    );
    c.wait(1.5);
    step(&mut c, &r, "the player moved out");
    remove(&mut c, &a);
    remove(&mut c, &b);
    remove(&mut c, &id("TrigItem"));
    c.wait(1.0);
    step(&mut c, &r, "the NPCs and the sword deleted");
    c.finish(r)
}

fn test_traps() -> FunctionBuilder {
    let (mut c, r) = report();
    let start = now(&mut c);
    c.assign(id("HitStart"), start);
    c.assign(id("TrapLog"), string(""));
    let n = fresh(&mut c, "n", "Dummy", 100.0, 300.0);
    set_av(&mut c, &n, "Health", 500.0);
    c.wait(1.5);
    restrain(&mut c, &n);
    let hp0 = health(&mut c, &n);
    put_num(&mut c, &r, "an NPC held in place, health ", hp0);
    line(&mut c, &r, "\n");
    // Dropped from above its head.
    let bowl = c.method(
        "ObjectReference",
        n.clone(),
        "PlaceAtMe",
        [auto_var("TrapBowl"), 1.into(), false.into(), false.into()],
    );
    c.assign(id("Bowl1"), bowl.clone());
    c.method(
        "ObjectReference",
        bowl,
        "MoveTo",
        [
            n.clone(),
            0.0f32.into(),
            0.0f32.into(),
            250.0f32.into(),
            false.into(),
        ],
    );
    c.wait(3.0);
    line(
        &mut c,
        &r,
        "our scripted bowl dropped from 250 units above it:\n",
    );
    put(&mut c, &r, "", id("TrapLog"));
    c.assign(id("TrapLog"), string(""));
    // Pushed at it from the side.
    let bowl = c.method(
        "ObjectReference",
        n.clone(),
        "PlaceAtMe",
        [auto_var("TrapBowl"), 1.into(), false.into(), false.into()],
    );
    c.assign(id("Bowl2"), bowl.clone());
    c.method(
        "ObjectReference",
        bowl.clone(),
        "MoveTo",
        [
            n.clone(),
            (-120.0f32).into(),
            0.0f32.into(),
            60.0f32.into(),
            false.into(),
        ],
    );
    c.wait(1.0);
    c.method(
        "ObjectReference",
        bowl,
        "ApplyHavokImpulse",
        [1.0f32.into(), 0.0f32.into(), 0.2f32.into(), 30.0f32.into()],
    );
    c.wait(3.0);
    line(
        &mut c,
        &r,
        "another pushed at it from 120 units (ApplyHavokImpulse 30):\n",
    );
    put(&mut c, &r, "", id("TrapLog"));
    let hp1 = health(&mut c, &n);
    put_num(&mut c, &r, "its health then ", hp1);
    line(&mut c, &r, "\n");
    remove(&mut c, &n);
    remove(&mut c, &id("Bowl1"));
    remove(&mut c, &id("Bowl2"));
    // Back where the player started.
    c.method(
        "ObjectReference",
        auto_var("Player"),
        "MoveTo",
        [
            id("StartMarker"),
            0.0f32.into(),
            0.0f32.into(),
            0.0f32.into(),
            true.into(),
        ],
    );
    c.wait(5.0);
    remove(&mut c, &id("StartMarker"));
    c.finish(r)
}

/// A small script calling `Main.<function>` from an event of its own,
/// passing the event's parameters as they come (`self` cast to
/// ObjectReference where `pass_self`) and `extra` text first.
fn relay(
    script: &mut ScriptBuilder,
    event: &str,
    params: &[(&str, &str)],
    target: &str,
    args: impl FnOnce(&mut Code, &[Arg]) -> Vec<Arg>,
) {
    let mut c = Code::new("None");
    let ps: Vec<Arg> = params.iter().map(|(n, t)| c.param(n, t)).collect();
    let call_args = args(&mut c, &ps);
    c.call_method(auto_var("Main"), target, discard(), call_args);
    script.function(event, c.finish(Arg::None));
}

fn self_ref(c: &mut Code) -> Arg {
    let s = c.temp("ObjectReference");
    c.cast(s.clone(), id("self"));
    s
}

const TRAP_PARAMS: &[(&str, &str)] = &[
    ("akTarget", "ObjectReference"),
    ("afXVel", "Float"),
    ("afYVel", "Float"),
    ("afZVel", "Float"),
    ("afXPos", "Float"),
    ("afYPos", "Float"),
    ("afZPos", "Float"),
    ("aeMaterial", "Int"),
    ("abInitialHit", "Bool"),
    ("aeMotionType", "Int"),
];

const STORY_PARAMS: &[(&str, &str)] = &[
    ("akKeyword", "Keyword"),
    ("akLocation", "Location"),
    ("akRef1", "ObjectReference"),
    ("akRef2", "ObjectReference"),
    ("aiValue1", "Int"),
    ("aiValue2", "Int"),
];

fn scripts() -> Result<Vec<(String, Vec<u8>)>> {
    let mut s = ScriptBuilder::new(SCRIPT, "Quest");
    s.auto_property("Player", "Actor");
    for (name, ty, _) in VANILLA {
        s.auto_property(name, ty);
    }
    for (name, ty, _) in OWN {
        s.auto_property(name, ty);
    }
    s.variable("Ran", "Bool", false.into());
    for name in ["HitLog", "TrigLog", "TrapLog"] {
        s.variable(name, "String", string(""));
    }
    s.variable("HitStart", "Float", 0.0f32.into());
    s.variable("StartMarker", "ObjectReference", Arg::None);
    for var in REF_VARS {
        let ty = if matches!(*var, "TrigItem" | "Bowl1" | "Bowl2") {
            "ObjectReference"
        } else {
            "Actor"
        };
        s.variable(var, ty, Arg::None);
    }
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
        .function("OnUpdate", on_update("autotest4", "twenty minutes", TESTS))
        .function("Num", num())
        .function("Name", name_fn())
        .function("Place", place_fn())
        .function("LogHit", log_hit())
        .function("LogEffect", log_effect())
        .function("LogTrigger", log_trigger())
        .function("LogTrap", log_trap())
        .function("TestHits", test_hits())
        .function("TestAVPercent", test_av_percent())
        .function("TestMagicConditions", test_magic_conditions())
        .function("TestResistance", test_resistance())
        .function("TestTargetValues", test_target_values())
        .function("TestAbsorb", test_absorb())
        .function("TestBoundWeapon", test_bound_weapon())
        .function("TestAreaRadius", test_area_radius())
        .function("TestMagnitudePerk", test_magnitude_perk())
        .function("TestPerkPriority", test_perk_priority())
        .function("TestPerkSections", test_perk_sections())
        .function("TestAdvanceSkill", test_advance_skill())
        .function("TestShouts", test_shouts())
        .function("TestAliasPackages", test_alias_packages())
        .function("TestStoryManager", test_story_manager())
        .function("TestScenes", test_scenes())
        .function("TestAssistance", test_assistance())
        .function("TestFriendlyHits", test_friendly_hits())
        .function("TestCombatRegen", test_combat_regen())
        .function("TestFlee", test_flee())
        .function("TestDualWield", test_dual_wield())
        .function("TestTorches", test_torches())
        .function("TestAliasFills", test_alias_fills())
        .function("TestTriggers", test_triggers())
        .function("TestTraps", test_traps());

    // The magic target: its hits and effects to the test script.
    let mut hit = ScriptBuilder::new(HIT_SCRIPT, "Actor");
    hit.auto_property("Main", SCRIPT);
    relay(
        &mut hit,
        "OnHit",
        &[
            ("akAggressor", "ObjectReference"),
            ("akSource", "Form"),
            ("akProjectile", "Projectile"),
            ("abPowerAttack", "Bool"),
            ("abSneakAttack", "Bool"),
            ("abBashAttack", "Bool"),
            ("abHitBlocked", "Bool"),
        ],
        "LogHit",
        |_, ps| ps.to_vec(),
    );
    relay(
        &mut hit,
        "OnMagicEffectApply",
        &[("akCaster", "ObjectReference"), ("akEffect", "MagicEffect")],
        "LogEffect",
        |_, ps| ps.to_vec(),
    );

    let mut trigger = ScriptBuilder::new(TRIGGER_SCRIPT, "ObjectReference");
    trigger.auto_property("Main", SCRIPT);
    for (event, what) in [("OnTriggerEnter", "enter"), ("OnTriggerLeave", "leave")] {
        relay(
            &mut trigger,
            event,
            &[("akActionRef", "ObjectReference")],
            "LogTrigger",
            |_, ps| vec![ps[0].clone(), string(what)],
        );
    }

    let mut trap = ScriptBuilder::new(TRAP_SCRIPT, "ObjectReference");
    trap.auto_property("Main", SCRIPT);
    for (event, what) in [
        ("OnTrapHitStart", "OnTrapHitStart"),
        ("OnTrapHit", "OnTrapHit"),
    ] {
        relay(&mut trap, event, TRAP_PARAMS, "LogTrap", |c, ps| {
            let me = self_ref(c);
            vec![
                string(what),
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
    relay(
        &mut trap,
        "OnTrapHitStop",
        &[("akTarget", "ObjectReference")],
        "LogTrap",
        |c, ps| {
            let me = self_ref(c);
            vec![
                string("OnTrapHitStop"),
                me,
                ps[0].clone(),
                false.into(),
                0.into(),
                0.0f32.into(),
                0.0f32.into(),
                0.0f32.into(),
            ]
        },
    );

    // The members quest: what OnStoryScript got, as text.
    let mut members = ScriptBuilder::new(MEMBERS_SCRIPT, "Quest");
    members.variable("Members", "String", string(""));
    let mut c = Code::new("None");
    let ps: Vec<Arg> = STORY_PARAMS.iter().map(|(n, t)| c.param(n, t)).collect();
    c.assign(id("Members"), string(""));
    for ((label, _), p) in ["K ", ", L ", ", R1 ", ", R2 ", ", V1 ", ", V2 "]
        .iter()
        .zip(STORY_PARAMS)
        .zip(&ps)
    {
        c.append(id("Members"), [string(label), p.clone()]);
    }
    members.function("OnStoryScript", c.finish(Arg::None));
    members.function("GetMembers", Code::new("String").finish(id("Members")));

    // The waiting quest: its OnStoryScript takes 2 s.
    let mut waiting = ScriptBuilder::new(WAIT_SCRIPT, "Quest");
    waiting.variable("Done", "Bool", false.into());
    let mut c = Code::new("None");
    for (n, t) in STORY_PARAMS {
        c.param(n, t);
    }
    c.assign(id("Done"), false.into());
    c.wait(2.0);
    c.assign(id("Done"), true.into());
    waiting.function("OnStoryScript", c.finish(Arg::None));
    waiting.function("IsDone", Code::new("Bool").finish(id("Done")));

    let mut out = Vec::new();
    for (name, b) in [
        (SCRIPT, &s),
        (HIT_SCRIPT, &hit),
        (TRIGGER_SCRIPT, &trigger),
        (TRAP_SCRIPT, &trap),
        (MEMBERS_SCRIPT, &members),
        (WAIT_SCRIPT, &waiting),
    ] {
        out.push((name.to_owned(), papyrus::pex::write(&b.build()?)?));
    }
    Ok(out)
}

/// The game's script event root node and its last child, after which our
/// branch goes.
fn script_event_root(lo: &LoadOrder) -> Result<(FormId, FormId)> {
    let mut root = None;
    let mut children: Vec<esp::story::Node> = Vec::new();
    for tag in [b"SMEN", b"SMBN", b"SMQN"] {
        for &id in lo.ids_of_type(tag) {
            let Some(rec) = lo.get(id) else { continue };
            let Some(node) = esp::story::parse(&rec, id) else {
                continue;
            };
            if node.event() == Some(*b"SCPT") && id.0 >> 24 == 0 {
                root = Some(id);
            }
            children.push(node);
        }
    }
    let root = root.with_context(|| format!("no {SCRIPT_EVENT_ROOT} event node"))?;
    let kids: Vec<&esp::story::Node> = children.iter().filter(|n| n.parent == root).collect();
    // The last: the one no other names as the sibling before it.
    let last = kids
        .iter()
        .find(|k| !kids.iter().any(|o| o.previous == k.id))
        .map_or(FormId::NULL, |k| k.id);
    Ok((root, last))
}

/// The first three spells of a vanilla shout (its words' spells).
fn shout_spells(lo: &LoadOrder, shout: &str) -> Result<Vec<FormId>> {
    let id = lo
        .find_editor_id(shout)
        .with_context(|| format!("{shout} not found"))?;
    let rec = lo.get(id).context("shout")?;
    Ok(rec
        .subrecords()
        .filter(|s| s.tag.0 == *b"SNAM" && s.data.len() >= 8)
        .map(|s| FormId(u32::from_le_bytes(s.data[4..8].try_into().unwrap())))
        .collect())
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let v = Vanilla { lo };
    let template = v.id("WEMerchantMiscMaleCommoner")?;
    let race = v.id("ImperialRace")?;
    let class = v.id("Citizen")?;
    let fire_damage = v.id("FireDamageFFAimed")?;
    let fortify_one = v.id("AlchFortifyOneHanded")?;
    let fortify_two = v.id("AlchFortifyTwoHanded")?;
    let either_hand = v.id("EitherHand")?;
    let firebolt_projectile = v.id("FireboltProjectile01")?;
    let dual_style = v.id("csThalmorMeleeDual")?;
    let floor = v.id("WRIntFloorSTMid01Large")?;
    let x_marker = v.id("XMarker")?;
    let (root, last) = script_event_root(lo)?;
    let shout_spells = shout_spells(lo, "UnrelentingForceShout")?;
    anyhow::ensure!(shout_spells.len() == 3, "Unrelenting Force's words");

    // The test script's properties.
    let object = PropertyValue::Object;
    let mut properties = vec![("Player".to_owned(), object(PLAYER_REF))];
    for (name, _, edid) in VANILLA {
        properties.push((name.to_string(), object(v.id(edid)?)));
    }
    for (name, _, object_id) in OWN {
        properties.push((name.to_string(), object(own(*object_id))));
    }
    let main = || AttachedScript {
        name: String::new(),
        properties: vec![("Main".into(), object(own(QUEST)))],
    };
    let with = |name: &str| AttachedScript {
        name: name.into(),
        ..main()
    };

    let factions = vec![
        records::faction(
            FACTION_A,
            "VrmAutoTest4FactionA",
            &[(FACTION_B, reaction::ENEMY)],
        ),
        records::faction(
            FACTION_B,
            "VrmAutoTest4FactionB",
            &[(FACTION_A, reaction::ENEMY)],
        ),
        records::faction(
            VICTIMS,
            "VrmAutoTest4Victims",
            &[
                (ENEMIES, reaction::ENEMY),
                (ALLIES, reaction::ALLY),
                (FRIENDS, reaction::FRIEND),
            ],
        ),
        records::faction(
            ENEMIES,
            "VrmAutoTest4Enemies",
            &[(VICTIMS, reaction::ENEMY)],
        ),
        records::faction(ALLIES, "VrmAutoTest4Allies", &[(VICTIMS, reaction::ALLY)]),
        records::faction(
            FRIENDS,
            "VrmAutoTest4Friends",
            &[(VICTIMS, reaction::FRIEND)],
        ),
        records::faction(GROUP, "VrmAutoTest4Group", &[(GROUP, reaction::FRIEND)]),
    ];

    struct Spec {
        id: u32,
        edid: &'static str,
        level: u16,
        temper: (u8, u8),
        faction: Option<u32>,
        assistance: u8,
    }
    let spec = |id, edid, level, temper, faction| Spec {
        id,
        edid,
        level,
        temper,
        faction,
        assistance: 0,
    };
    let specs = [
        spec(DUMMY, "VrmAutoTest4Dummy", 10, (0, 0), None),
        spec(CASTER, "VrmAutoTest4Caster", 10, (0, 2), None),
        spec(LOW_CASTER, "VrmAutoTest4LowCaster", 1, (0, 2), None),
        spec(LEVEL5, "VrmAutoTest4Level5", 5, (1, 2), None),
        spec(LEVEL20, "VrmAutoTest4Level20", 20, (1, 2), None),
        spec(TARGET, "VrmAutoTest4Target", 10, (0, 2), None),
        Spec {
            assistance: 1,
            ..spec(
                HELPER_ALLY_ALLIES,
                "VrmAutoTest4HelperAllyAllies",
                10,
                (0, 3),
                Some(ALLIES),
            )
        },
        spec(
            HELPER_ALLY_NOBODY,
            "VrmAutoTest4HelperAllyNobody",
            10,
            (0, 3),
            Some(ALLIES),
        ),
        Spec {
            assistance: 1,
            ..spec(
                HELPER_FRIEND_ALLIES,
                "VrmAutoTest4HelperFriendAllies",
                10,
                (0, 3),
                Some(FRIENDS),
            )
        },
        Spec {
            assistance: 2,
            ..spec(
                HELPER_FRIEND_FRIENDS,
                "VrmAutoTest4HelperFriendFriends",
                10,
                (0, 3),
                Some(FRIENDS),
            )
        },
        spec(VICTIM, "VrmAutoTest4Victim", 10, (0, 2), Some(VICTIMS)),
        spec(ENEMY, "VrmAutoTest4Enemy", 10, (1, 3), Some(ENEMIES)),
        spec(
            GROUP_MEMBER,
            "VrmAutoTest4GroupMember",
            10,
            (0, 2),
            Some(GROUP),
        ),
        spec(COWARD, "VrmAutoTest4Coward", 10, (0, 0), Some(VICTIMS)),
        spec(BRUTE, "VrmAutoTest4Brute", 50, (1, 4), Some(ENEMIES)),
        spec(DUAL, "VrmAutoTest4Dual", 10, (1, 3), Some(FACTION_A)),
        spec(
            DUAL_PLAIN,
            "VrmAutoTest4DualPlain",
            10,
            (1, 3),
            Some(FACTION_A),
        ),
        spec(BAG, "VrmAutoTest4Bag", 10, (0, 0), Some(FACTION_B)),
        spec(UNIQUE, "VrmAutoTest4Unique", 10, (0, 0), None),
        spec(UNPLACED, "VrmAutoTest4Unplaced", 10, (0, 0), None),
        spec(MATCHABLE, "VrmAutoTest4Matchable", 10, (0, 0), None),
        spec(PACKAGED, "VrmAutoTest4Packaged", 10, (0, 0), None),
        spec(SCENE_ACTOR, "VrmAutoTest4SceneActor", 10, (0, 0), None),
        spec(STORY_ACTOR, "VrmAutoTest4StoryActor", 10, (0, 0), None),
    ];
    let mut npcs: Vec<RecordBuilder> = specs
        .into_iter()
        .map(|s| {
            Npc {
                object_id: s.id,
                edid: s.edid.to_owned(),
                name: Some("vibe_rm Test NPC"),
                level: (s.level, false, 1, 0),
                template: (template, 0x1),
                temper: s.temper,
                factions: s.faction.map(|f| (own(f), 0)).into_iter().collect(),
                script: (s.id == TARGET).then(|| with(HIT_SCRIPT)),
                assistance: s.assistance,
                combat_style: (s.id == DUAL).then_some(dual_style),
                unique: matches!(s.id, UNIQUE | UNPLACED),
            }
            .record(race, class)
        })
        .collect();
    npcs.push(
        Npc {
            object_id: CREATED,
            edid: "VrmAutoTest4Created".into(),
            name: Some("vibe_rm Test NPC"),
            level: (1000, true, 1, 0),
            template: (template, 0x1),
            temper: (0, 0),
            factions: Vec::new(),
            script: None,
            assistance: 0,
            combat_style: None,
            unique: false,
        }
        .record(race, class),
    );

    // Magic effects: an absorb that flies as Firebolt does; an ability's
    // fortify.
    let patch_data = |cast: u32, delivery: u32, projectile: Option<FormId>| {
        move |tag: &[u8; 4], d: &mut Vec<u8>| {
            if tag == b"DATA" && d.len() >= 0x58 {
                if let Some(p) = projectile {
                    d[0x48..0x4C].copy_from_slice(&p.0.to_le_bytes());
                }
                d[0x50..0x54].copy_from_slice(&cast.to_le_bytes());
                d[0x54..0x58].copy_from_slice(&delivery.to_le_bytes());
            }
        }
    };
    let effects = vec![
        records::copy(
            lo,
            "AbsorbHealthConcAimed",
            ABSORB_EFFECT,
            "VrmAutoTest4AbsorbFF",
            None,
            patch_data(
                spell::FIRE_AND_FORGET,
                spell::AIMED,
                Some(firebolt_projectile),
            ),
        )?,
        records::copy(
            lo,
            "AlchFortifyOneHanded",
            ABILITY_EFFECT,
            "VrmAutoTest4AbilityFortify",
            None,
            patch_data(spell::CONSTANT, spell::SELF, None),
        )?,
    ];
    let ff = spell::FIRE_AND_FORGET;
    let aimed = |id, edid, flags, effects: Vec<Effect>| {
        records::spell(
            id,
            edid,
            spell::SPELL,
            ff,
            spell::AIMED,
            flags,
            Some(either_hand),
            &effects,
        )
    };
    let on_self = |id, edid, effects: Vec<Effect>| {
        records::spell(
            id,
            edid,
            spell::SPELL,
            ff,
            spell::SELF,
            0,
            Some(either_hand),
            &effects,
        )
    };
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
    let level_5 = |on| ctda(func::GET_LEVEL, 0, 0, records::op::GE, 5.0, on);
    let spells = vec![
        aimed(
            BOLT,
            "VrmAutoTest4Bolt",
            0,
            vec![Effect::new(fire_damage, 25.0, 0)],
        ),
        aimed(
            BLAST,
            "VrmAutoTest4Blast",
            0,
            vec![Effect::new(fire_damage, 50.0, 0)],
        ),
        aimed(
            BLAST_IGNORE,
            "VrmAutoTest4BlastIgnore",
            spell::IGNORE_RESISTANCE,
            vec![Effect::new(fire_damage, 50.0, 0)],
        ),
        on_self(
            FORT_COND,
            "VrmAutoTest4FortCond",
            vec![
                Effect::new(fortify_one, 10.0, 60).when(av_percent(0.5)),
                Effect::new(fortify_two, 10.0, 60).when(av_percent(50.0)),
            ],
        ),
        aimed(
            SUBJECT,
            "VrmAutoTest4Subject",
            0,
            vec![Effect::new(fire_damage, 20.0, 0).when(level_5(run_on::SUBJECT))],
        ),
        aimed(
            SUBJECT_TARGET,
            "VrmAutoTest4SubjectTarget",
            0,
            vec![Effect::new(fire_damage, 20.0, 0).when(level_5(run_on::TARGET))],
        ),
        on_self(
            RECHECK,
            "VrmAutoTest4Recheck",
            vec![Effect::new(fortify_two, 10.0, 60).when(ctda(
                func::GET_ACTOR_VALUE,
                24,
                0,
                records::op::GE,
                100.0,
                run_on::SUBJECT,
            ))],
        ),
        aimed(
            ABSORB,
            "VrmAutoTest4Absorb",
            0,
            vec![Effect::new(own(ABSORB_EFFECT), 20.0, 0)],
        ),
        on_self(
            HAS_SHOUT,
            "VrmAutoTest4HasShout",
            vec![Effect::new(fortify_one, 10.0, 60).when(ctda(
                func::HAS_SHOUT,
                own(SHOUT).0,
                0,
                records::op::EQ,
                1.0,
                run_on::SUBJECT,
            ))],
        ),
        records::spell(
            ABILITY,
            "VrmAutoTest4Ability",
            spell::ABILITY,
            spell::CONSTANT,
            spell::SELF,
            0,
            None,
            &[Effect::new(own(ABILITY_EFFECT), 7.0, 0)],
        ),
    ];

    let entry = |function, value, priority| Section::Entry {
        point: MOD_SPELL_MAGNITUDE,
        function,
        tabs: 3,
        value,
        priority,
    };
    let perks = vec![
        records::perk(PERK_ADD, "VrmAutoTest4PerkAdd", &[entry(2, 10.0, 0)]),
        records::perk(PERK_MUL, "VrmAutoTest4PerkMul", &[entry(3, 2.0, 1)]),
        records::perk(
            PERK_ADD_LATE,
            "VrmAutoTest4PerkAddLate",
            &[entry(2, 10.0, 1)],
        ),
        records::perk(
            PERK_MUL_EARLY,
            "VrmAutoTest4PerkMulEarly",
            &[entry(3, 2.0, 0)],
        ),
        records::perk(
            PERK_QUEST,
            "VrmAutoTest4PerkQuest",
            &[Section::Quest(own(PERK_STAGE_QUEST), 10)],
        ),
        records::perk(
            PERK_ABILITY,
            "VrmAutoTest4PerkAbility",
            &[Section::Ability(own(ABILITY))],
        ),
    ];

    let optional = alias_flags::OPTIONAL | alias_flags::ALLOW_REUSE;
    let alias = |id, name, flags, fill, packages: Vec<u32>| Alias {
        id,
        name,
        flags,
        fill,
        packages: packages.into_iter().map(own).collect(),
    };
    let matching = || {
        Fill::Matching(vec![ctda(
            func::GET_IS_ID,
            own(MATCHABLE).0,
            0,
            records::op::EQ,
            1.0,
            run_on::SUBJECT,
        )])
    };
    let quest = |id, edid, flags, priority, script, stages, aliases| {
        Quest {
            object_id: id,
            edid,
            flags,
            priority,
            kind: 0,
            script,
            stages,
            aliases,
        }
        .record()
    };
    let scripted = |name: &str| AttachedScript {
        name: name.into(),
        properties: Vec::new(),
    };
    let quests = vec![
        quest(
            QUEST,
            "VrmAutoTest4Quest",
            0x0011,
            0,
            Some(AttachedScript {
                name: SCRIPT.into(),
                properties,
            }),
            &[],
            Vec::new(),
        ),
        quest(
            PERK_STAGE_QUEST,
            "VrmAutoTest4PerkStageQuest",
            0,
            0,
            None,
            &[10],
            Vec::new(),
        ),
        quest(
            FILL_QUEST,
            "VrmAutoTest4FillQuest",
            0,
            0,
            None,
            &[],
            vec![
                alias(0, "Player", optional, Fill::Forced(PLAYER_REF), vec![]),
                alias(
                    1,
                    "ForcedDisabled",
                    optional,
                    Fill::Forced(own(FORCED_REF)),
                    vec![],
                ),
                alias(2, "UniqueDead", optional, Fill::Unique(own(UNIQUE)), vec![]),
                alias(
                    3,
                    "UniqueUnplaced",
                    optional,
                    Fill::Unique(own(UNPLACED)),
                    vec![],
                ),
                alias(
                    4,
                    "CreatedEasy",
                    optional,
                    Fill::Create {
                        object: own(CREATED),
                        at: 0,
                        level: 0,
                    },
                    vec![],
                ),
                alias(
                    5,
                    "CreatedMedium",
                    optional,
                    Fill::Create {
                        object: own(CREATED),
                        at: 0,
                        level: 1,
                    },
                    vec![],
                ),
                alias(
                    6,
                    "CreatedVeryHard",
                    optional,
                    Fill::Create {
                        object: own(CREATED),
                        at: 0,
                        level: 3,
                    },
                    vec![],
                ),
                alias(
                    7,
                    "Matching",
                    optional | alias_flags::IN_LOADED_AREA,
                    matching(),
                    vec![],
                ),
                alias(
                    8,
                    "MatchingClosest",
                    optional | alias_flags::IN_LOADED_AREA | alias_flags::CLOSEST,
                    matching(),
                    vec![],
                ),
            ],
        ),
        quest(
            PACKAGE_QUEST_A,
            "VrmAutoTest4PackageQuestA",
            0,
            50,
            None,
            &[],
            vec![alias(0, "Actor", optional, Fill::Empty, vec![PACKAGE_A])],
        ),
        quest(
            PACKAGE_QUEST_B,
            "VrmAutoTest4PackageQuestB",
            0,
            50,
            None,
            &[],
            vec![alias(0, "Actor", optional, Fill::Empty, vec![PACKAGE_B])],
        ),
        quest(
            PACKAGE_QUEST_C,
            "VrmAutoTest4PackageQuestC",
            0,
            50,
            None,
            &[],
            vec![
                alias(0, "First", optional, Fill::Empty, vec![PACKAGE_C1]),
                alias(1, "Second", optional, Fill::Empty, vec![PACKAGE_C2]),
            ],
        ),
        quest(
            SCENE_QUEST,
            "VrmAutoTest4SceneQuest",
            0,
            0,
            None,
            &[10],
            vec![alias(0, "Actor", optional, Fill::Empty, vec![])],
        ),
        quest(
            SCENE_QUEST_STOP,
            "VrmAutoTest4SceneQuestStop",
            0,
            0,
            None,
            &[],
            vec![alias(0, "Actor", optional, Fill::Empty, vec![])],
        ),
        quest(
            STORY_NPC_QUEST,
            "VrmAutoTest4StoryNpcQuest",
            0,
            0,
            None,
            &[],
            Vec::new(),
        ),
        quest(
            STORY_PLAYER_QUEST,
            "VrmAutoTest4StoryPlayerQuest",
            0,
            0,
            None,
            &[],
            Vec::new(),
        ),
        quest(
            MEMBERS_QUEST,
            "VrmAutoTest4MembersQuest",
            0,
            0,
            Some(scripted(MEMBERS_SCRIPT)),
            &[],
            Vec::new(),
        ),
        quest(
            RESET_QUEST,
            "VrmAutoTest4ResetQuest",
            0,
            0,
            None,
            &[],
            Vec::new(),
        ),
        quest(
            MAX_QUEST_1,
            "VrmAutoTest4MaxQuest1",
            0,
            0,
            None,
            &[],
            Vec::new(),
        ),
        quest(
            MAX_QUEST_2,
            "VrmAutoTest4MaxQuest2",
            0,
            0,
            None,
            &[],
            Vec::new(),
        ),
        quest(
            WAIT_QUEST,
            "VrmAutoTest4WaitQuest",
            0,
            0,
            Some(scripted(WAIT_SCRIPT)),
            &[],
            Vec::new(),
        ),
    ];

    let packages = [
        (PACKAGE_A, "VrmAutoTest4PackageA"),
        (PACKAGE_B, "VrmAutoTest4PackageB"),
        (PACKAGE_C1, "VrmAutoTest4PackageC1"),
        (PACKAGE_C2, "VrmAutoTest4PackageC2"),
    ]
    .into_iter()
    .map(|(id, edid)| records::copy(lo, "DoNothing", id, edid, None, |_, _| {}))
    .collect::<Result<Vec<_>>>()?;

    let keywords = vec![
        records::keyword(KEY_SUBJECT, "VrmAutoTest4KeySubject"),
        records::keyword(KEY_MEMBERS, "VrmAutoTest4KeyMembers"),
        records::keyword(KEY_RESET, "VrmAutoTest4KeyReset"),
        records::keyword(KEY_MAX, "VrmAutoTest4KeyMax"),
        records::keyword(KEY_WAIT, "VrmAutoTest4KeyWait"),
    ];

    // Our branch after the game's last script event node, our quest nodes
    // under it in order.
    let key = |k: u32| event_data_is(b"K1", own(k));
    let is = |f: FormId| {
        ctda(
            func::GET_IS_ID,
            f.0,
            0,
            records::op::EQ,
            1.0,
            run_on::SUBJECT,
        )
    };
    let node = |id, edid, prev: u32, conds: Vec<[u8; 32]>, max, quests: &[(u32, f32)]| {
        let quests: Vec<(FormId, u32, f32)> = quests
            .iter()
            .map(|&(q, reset)| (own(q), 0, reset))
            .collect();
        records::story_node(
            id,
            edid,
            own(NODE_BRANCH),
            if prev == 0 { FormId::NULL } else { own(prev) },
            &conds,
            max,
            Some(&quests),
        )
    };
    let story_branch = vec![records::story_node(
        NODE_BRANCH,
        "VrmAutoTest4Branch",
        root,
        last,
        &[],
        0,
        None,
    )];
    let story_nodes = vec![
        node(
            NODE_SUBJECT_NPC,
            "VrmAutoTest4NodeSubjectNpc",
            0,
            vec![key(KEY_SUBJECT), is(own(STORY_ACTOR))],
            0,
            &[(STORY_NPC_QUEST, 0.0)],
        ),
        node(
            NODE_SUBJECT_PLAYER,
            "VrmAutoTest4NodeSubjectPlayer",
            NODE_SUBJECT_NPC,
            vec![key(KEY_SUBJECT), is(PLAYER_NPC)],
            0,
            &[(STORY_PLAYER_QUEST, 0.0)],
        ),
        node(
            NODE_MEMBERS,
            "VrmAutoTest4NodeMembers",
            NODE_SUBJECT_PLAYER,
            vec![key(KEY_MEMBERS)],
            0,
            &[(MEMBERS_QUEST, 0.0)],
        ),
        node(
            NODE_RESET,
            "VrmAutoTest4NodeReset",
            NODE_MEMBERS,
            vec![key(KEY_RESET)],
            0,
            &[(RESET_QUEST, 1.0)],
        ),
        node(
            NODE_MAX,
            "VrmAutoTest4NodeMax",
            NODE_RESET,
            vec![key(KEY_MAX)],
            1,
            &[(MAX_QUEST_1, 0.0), (MAX_QUEST_2, 0.0)],
        ),
        node(
            NODE_WAIT,
            "VrmAutoTest4NodeWait",
            NODE_MAX,
            vec![key(KEY_WAIT)],
            0,
            &[(WAIT_QUEST, 0.0)],
        ),
    ];

    let stage = |s: f32| {
        ctda(
            func::GET_STAGE,
            own(SCENE_QUEST).0,
            0,
            records::op::GE,
            s,
            run_on::SUBJECT,
        )
    };
    let scene = |id,
                 edid,
                 quest: u32,
                 flags,
                 behaviour,
                 start: &[[u8; 32]],
                 complete: &[[u8; 32]],
                 seconds| {
        records::scene(
            id,
            edid,
            own(quest),
            flags,
            0,
            behaviour,
            start,
            complete,
            seconds,
        )
    };
    let scenes = vec![
        scene(
            SCENE_PLAIN,
            "VrmAutoTest4ScenePlain",
            SCENE_QUEST,
            0,
            0,
            &[],
            &[],
            3.0,
        ),
        scene(
            SCENE_LONG,
            "VrmAutoTest4SceneLong",
            SCENE_QUEST,
            0,
            0,
            &[],
            &[],
            30.0,
        ),
        scene(
            SCENE_COMPLETE,
            "VrmAutoTest4SceneComplete",
            SCENE_QUEST,
            0,
            0,
            &[],
            &[stage(10.0)],
            20.0,
        ),
        scene(
            SCENE_DEATH,
            "VrmAutoTest4SceneDeath",
            SCENE_QUEST,
            0,
            scene_flags::DEATH_END,
            &[],
            &[],
            20.0,
        ),
        scene(
            SCENE_STOP_QUEST,
            "VrmAutoTest4SceneStopQuest",
            SCENE_QUEST_STOP,
            scene_flags::STOP_QUEST_ON_END,
            0,
            &[],
            &[],
            3.0,
        ),
        scene(
            SCENE_SKIP,
            "VrmAutoTest4SceneSkip",
            SCENE_QUEST,
            0,
            0,
            &[stage(99.0)],
            &[],
            20.0,
        ),
    ];

    let words: Vec<RecordBuilder> = (0..3)
        .map(|i| records::word_of_power(WORDS + i, &format!("VrmAutoTest4Word{}", i + 1)))
        .collect();
    let shouts = vec![records::shout(
        SHOUT,
        "VrmAutoTest4Shout",
        &[
            (own(WORDS), shout_spells[0], 15.0),
            (own(WORDS + 1), shout_spells[1], 20.0),
            (own(WORDS + 2), shout_spells[2], 45.0),
        ],
    )];

    let activators = vec![records::copy(
        lo,
        "defaultBlankTrigger",
        TRIGGER,
        "VrmAutoTest4Trigger",
        Some(with(TRIGGER_SCRIPT)),
        |_, _| {},
    )?];
    let misc = vec![records::copy(
        lo,
        "BasicWoodenBowl01",
        TRAP_BOWL,
        "VrmAutoTest4TrapBowl",
        Some(with(TRAP_SCRIPT)),
        |_, _| {},
    )?];

    // The test cell: one 1024 x 1024 floor piece around the origin (all kept
    // within 400 of it), the trigger box in the middle, where the player
    // comes in (west), a marker the forced alias names, the unique NPC.
    let persistent = record_flags::PERSISTENT;
    let cell = RecordBuilder::new(b"CELL", own(CELL))
        .zstring(b"EDID", "VrmAutoTest4Cell")
        .zstring(b"FULL", "vibe_rm Autotest 4")
        .field(b"DATA", &1u16.to_le_bytes())
        .field(b"XCLL", &xcll())
        .u32(b"LTMP", 0);
    let xprm: [u8; 32] = bytes4(&[
        TRIGGER_HALF,
        TRIGGER_HALF,
        TRIGGER_HALF,
        1.0,
        1.0,
        0.5,
        0.15,
    ]);
    let persistent_refs = vec![
        RecordBuilder::new(b"REFR", own(TRIGGER_REF))
            .flags(persistent)
            .form(b"NAME", own(TRIGGER))
            .field(b"XPRM", &xprm)
            .field(b"DATA", &placement([0.0, 0.0, TRIGGER_HALF], 0.0)),
        RecordBuilder::new(b"REFR", own(ENTRY_REF))
            .flags(persistent)
            .form(b"NAME", x_marker)
            .field(
                b"DATA",
                &placement([-400.0, 0.0, 0.0], std::f32::consts::FRAC_PI_2),
            ),
        RecordBuilder::new(b"REFR", own(FORCED_REF))
            .flags(persistent)
            .form(b"NAME", x_marker)
            .field(b"DATA", &placement([350.0, 350.0, 0.0], 0.0)),
        RecordBuilder::new(b"ACHR", own(UNIQUE_REF))
            .flags(persistent)
            .form(b"NAME", own(UNIQUE))
            .field(b"DATA", &placement([-350.0, -350.0, 0.0], 0.0)),
    ];
    let temporary_refs = vec![
        RecordBuilder::new(b"REFR", own(FLOOR_REF))
            .form(b"NAME", floor)
            .field(b"DATA", &placement([0.0, 0.0, 0.0], 0.0)),
    ];
    let cell_label = own(CELL).0.to_le_bytes();
    let children = Node::Group {
        label: cell_label,
        group_type: group_type::CELL_CHILDREN,
        children: vec![
            Node::Group {
                label: cell_label,
                group_type: group_type::CELL_PERSISTENT,
                children: persistent_refs.into_iter().map(Node::Record).collect(),
            },
            Node::Group {
                label: cell_label,
                group_type: group_type::CELL_TEMPORARY,
                children: temporary_refs.into_iter().map(Node::Record).collect(),
            },
        ],
    };
    // Interior cells go by the last two decimal digits of their object id.
    let cells = Node::Group {
        label: *b"CELL",
        group_type: group_type::TOP,
        children: vec![Node::Group {
            label: ((CELL % 10) as i32).to_le_bytes(),
            group_type: group_type::INTERIOR_BLOCK,
            children: vec![Node::Group {
                label: ((CELL / 10 % 10) as i32).to_le_bytes(),
                group_type: group_type::INTERIOR_SUB_BLOCK,
                children: vec![Node::Record(cell), children],
            }],
        }],
    };

    // Top groups in the Creation Kit's order.
    let plugin = write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "Tier 2 of plans/autotest-survey.md: tests run by script on loading a save, reports saved.",
        NEXT_OBJECT_ID,
        &[
            Node::top(b"KYWD", keywords),
            Node::top(b"FACT", factions),
            Node::top(b"MGEF", effects),
            Node::top(b"SPEL", spells),
            Node::top(b"ACTI", activators),
            Node::top(b"MISC", misc),
            Node::top(b"NPC_", npcs),
            Node::top(b"PACK", packages),
            Node::top(b"QUST", quests),
            Node::top(b"PERK", perks),
            Node::top(b"WOOP", words),
            Node::top(b"SHOU", shouts),
            Node::top(b"SCEN", scenes),
            cells,
            Node::top(b"SMBN", story_branch),
            Node::top(b"SMQN", story_nodes),
        ],
    );
    eprintln!("story: our branch under {root} after {last}");
    Ok(TestMod {
        plugin,
        scripts: scripts()?,
    })
}

/// Floats then a u32 type 1 (a box), as `XPRM` holds them.
fn bytes4(floats: &[f32; 7]) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (i, f) in floats.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&f.to_le_bytes());
    }
    out[28..32].copy_from_slice(&1u32.to_le_bytes());
    out
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
        let shown = run(
            scripts().unwrap(),
            &[
                (own(QUEST).0, SCRIPT, properties),
                (own(MEMBERS_QUEST).0, MEMBERS_SCRIPT, Vec::new()),
                (own(WAIT_QUEST).0, WAIT_SCRIPT, Vec::new()),
            ],
            TESTS.len(),
        );
        assert_eq!(shown.boxes.len(), TESTS.len(), "every test reports");
        assert_eq!(shown.saved_at_box, Some(0), "saves before the boxes");
        let all = shown.boxes.join("\n");
        for expected in [
            "-- ours: Firebolt's fire damage alone\n",
            "cast at 40%: OneHanded 1, TwoHanded 1, health 1\n",
            "the same run on the target: health lost 0\n",
            "health to 50: the bonus gone after (s; -1 not within 6) -1\n",
            "  fire resistance 200: health lost 0\n",
            "Courage on Level20: 1/1 -> 1/1\n",
            "after a 20 point absorb health: caster health 1, target 1\n",
            "an iron sword equipped: the effect ended after (s; -1 not within 5) ",
            "  500 units off (distance 1): 0\n",
            "with Augmented Flames: health lost 0\n",
            "+10 at priority 1, x2 at 0: health lost 0\n",
            "  removed: has the ability False, OneHanded 1\n",
            "  OneHanded set to 50: to 51 after 800 calls; to 52 after 800 calls;\n",
            "  TeachWord(3), UnlockWord(3): Words Of Power Learned 1 to 1",
            "  AddShout(ours), cast again 1\n",
            "  alias 1 forced, then alias 0: the NPC runs none\n",
            "OnStoryScript got \n",
            "  1.3 game hours later: started False ResetQuest running False\n",
            "the handler done then False, 3 s later False\n",
            "a 3 s scene flagged to stop its quest on end, 5 s later: the quest running False\n",
            "  friend, helps friends and allies, 500 units off (now 1): combat state 1, target none\n",
            "  cast 8, the struck one: combat state 1",
            "  StopCombat, 20 s: fighting one (state 1) 1, the other 1\n",
            "  harmed by 1 first: 1 1 1 1 1 1 1 1; its combat state 1\n",
            "the default combat style, two swords, 6 s fighting: right none, left none (type 1), shield none\n",
            "  an iron greatsword: left hand type 1, right 1\n",
            "  the closest matching: none\n",
            "the NPCs and the sword deleted (count 1):\n",
            "another pushed at it from 120 units (ApplyHavokImpulse 30):\n",
        ] {
            assert!(all.contains(expected), "{expected:?} not in the reports");
        }
    }
}
