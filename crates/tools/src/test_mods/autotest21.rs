//! `test-mod autotest21`: the strictly automatic items of
//! `plans/autotest-audit.md`, as many as one plugin runs: Papyrus natives,
//! conditions (each read through a probe ability), records' runtime
//! behaviour, magic, combat and detection. The tests are Papyrus source
//! (`autotest21/*.psc`) compiled by [`papyrus::compile`]; each
//! `String Function TestName()` is a test, run in the order the files give
//! them, its report kept in `ResultName` (with the seconds it took).
//!
//! Where: a test cell of ours (`VrmAt21Cell`: lit, floored, nothing living
//! in it) for everything that needs no navmesh, and the game's empty
//! developer cell `WarehouseFurniture` (navmeshed, furniture, no actors) for
//! fights, fleeing and packages: a cell of ours has no navmesh, and writing
//! one risks the whole run. The player is moved between them by script.
//!
//! The groups, in running order: `natives` (query, math and the remaining
//! natives, references, outfits, form lists, factions, protection),
//! `conditions` (status, comparison, geometry, death and variable
//! conditions through probes), `magic`, `items` (actor values, statistics,
//! loose objects, disarm), `detect`, `story` (Story Manager events, quest
//! aliases), `extra`, then in the warehouse `combat` and `combat2`, `leave`
//! (back to the cell), and `outdoors` (weather, fast travel, map markers,
//! from where the run began; skipped if that was indoors). Which audit
//! items each answers: `plans/autotest-audit.md`, "Covered by test-mod
//! autotest21".
//!
//! Run: load a save outdoors (the usual one), wait for the "running"
//! notification and leave the game alone until the "done" box (the reports
//! say how long each took; expect 40 to 60 minutes, mostly the probes'
//! waits). The player is moved to the cells and back, fast travels to
//! Whiterun and Solitude and visits a far map marker (with 100000 extra
//! health: run 1 fell to its death there and looped on the reloaded save).
//! Early on (the first `SetPlayerTeammate`) the game shows its follower
//! commands help, which waits to be dismissed by hand. The game saves every
//! 20 tests, so a crash keeps what ran; `vrm-tool test-results` reads the
//! newest save. The run changes that save for good (factions, crime gold,
//! a marker or two discovered, the player's skills and statistics), so use
//! a throwaway.
//!
//! A run in our own engine (`vibe_rm --plugin tests/assets/VrmAutoTest21.esp
//! --loose tests/assets --cell Riverwood --screenshot <png> --wait 220000`,
//! traces at `RUST_LOG=vibe_rm::script::natives=debug`) runs every test to
//! the end; its readings are ours, not the game's.
//!
//! Run in Skyrim SE on 2026-11-01: what each test showed, and which were
//! test faults, is in `research/autotest21.md`. Every probe was void: the
//! probe effect's actor value is still the potion's damage modifier (135;
//! autotest5 set it to `OneHanded`), and `HasMagicEffect` counts an effect
//! whose condition fails. Created aliases created nothing, and our caster
//! NPCs never cast.

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, vmad, write_plugin};
use esp::{FormId, LoadOrder, group_type, record_flags};

use super::places::{self, Interior};
use super::records::{self, spell};
use super::script::{self, bytes};
use super::venv::{placement, xcll};
use super::{TestMod, own};

#[path = "../../../../src/condition/functions.rs"]
#[allow(dead_code)]
mod ctda_functions;

const SCRIPT: &str = "VrmAt21Script";
const PLAYER_REF: FormId = FormId(0x14);

/// The test sources, in running order (base first: the helpers).
const SOURCES: &[(&str, &str)] = &[
    ("base", include_str!("autotest21/base.psc")),
    ("natives", include_str!("autotest21/natives.psc")),
    ("conditions", include_str!("autotest21/conditions.psc")),
    ("magic", include_str!("autotest21/magic.psc")),
    ("items", include_str!("autotest21/items.psc")),
    ("detect", include_str!("autotest21/detect.psc")),
    ("story", include_str!("autotest21/story.psc")),
    ("extra", include_str!("autotest21/extra.psc")),
    ("combat", include_str!("autotest21/combat.psc")),
    ("combat2", include_str!("autotest21/combat2.psc")),
    ("leave", include_str!("autotest21/leave.psc")),
    ("outdoors", include_str!("autotest21/outdoors.psc")),
];

/// The other scripts: attached to our records.
const LOGGERS: &[&str] = &[
    include_str!("autotest21/reflog.psc"),
    include_str!("autotest21/actorlog.psc"),
    include_str!("autotest21/storylog.psc"),
];

/// Story Manager event types a quest node of ours listens to.
const STORY_EVENTS: &[&str] = &[
    "REMP", "AIPL", "KILL", "ASSU", "CAST", "ADCR", "SKIL", "CHRR",
];

// Object ids, by kind.
const QUEST: u32 = 0x800;
const SIDE_QUEST: u32 = 0x801;
const ALIAS_QUEST: u32 = 0x802;
const PROBE_VALUE: u32 = 0x803;

const RED: u32 = 0x810;
const GREEN: u32 = 0x811;
const TEAL: u32 = 0x812;
const GROUPED: u32 = 0x813;
const CRIME: u32 = 0x814;

const TWO_SWORDS: u32 = 0x820;
const CRIME_GROUP: u32 = 0x821;
const KILLER_BASES: u32 = 0x822;
const KILLER_REFS: u32 = 0x823;
const LINK_KEYWORD: u32 = 0x824;
const OUTFIT_A: u32 = 0x825;
const OUTFIT_B: u32 = 0x826;
const REMOTE_MARKERS: u32 = 0x827;
const MATCH_QUESTS: u32 = 0x828;
const LEVEL_QUESTS: u32 = 0x829;
const CREATE_QUEST: u32 = 0x82A;
const MATCH_QUEST: u32 = 0x82B; // three
const LEVEL_QUEST: u32 = 0x82E; // two
const MATCH_NPC: u32 = 0x8A1;
const MATCH_FAR: u32 = 0x87D;
const MATCH_WH: u32 = 0x882;
/// Story quests and nodes: a quest (`+ 2i`) and a node (`+ 2i + 1`) for
/// each of `STORY_EVENTS`.
const STORY: u32 = 0x8B0;

const NAKED: u32 = 0x830;
const LOGGED: u32 = 0x831;
const RED_NPC: u32 = 0x832;
const GREEN_NPC: u32 = 0x833;
const OUTFIT_NPC: u32 = 0x834;
const PROTECT_NPC: u32 = 0x835;
const INVULNERABLE_NPC: u32 = 0x836;
const DEAD_COUNT_NPC: u32 = 0x837;
const GUARD_CLASS_NPC: u32 = 0x838;
const GUARD_FACTION_NPC: u32 = 0x839;
const UNIQUE_NPC: u32 = 0x83A;
const TPL_UNIQUE_CHILD: u32 = 0x83B;
const TPL_PLAIN_UNIQUE_CHILD: u32 = 0x83C;
const LVL_STATS_CHILD: u32 = 0x83D;
const LVL_TRAITS_CHILD: u32 = 0x83E;
const ALPHA_NPC: u32 = 0x83F;
const BETA_NPC: u32 = 0x840;
const GAMMA_NPC: u32 = 0x841;
const VICTIM_NPC: u32 = 0x842;
const LOW_NPC: u32 = 0x843;
const LEVEL8_NPC: u32 = 0x844;
const TEAL_HELPER: u32 = 0x845;
const RED_FIGHTER: u32 = 0x846;
const HELPER: u32 = 0x847;
const AVERAGE: u32 = 0x848;
const COWARD: u32 = 0x849;
const BRUTE: u32 = 0x84A;
const GREEN_FIGHTER: u32 = 0x84B;
const MAGE: u32 = 0x84C;
const HEALER: u32 = 0x84D;
const STAFF_NPC: u32 = 0x84E;
const PC_MULT_DREMORA: u32 = 0x84F;
const CRIME_GUARD: u32 = 0x8A0;

const DOOR: u32 = 0x850;
const WEB: u32 = 0x851;
const RESET_CHEST: u32 = 0x852;
const FIRE_EFFECT: u32 = 0x853;
const FIRE_SPELL: u32 = 0x854;
const ABSORB_EFFECT: u32 = 0x855;
const ABSORB_SPELL: u32 = 0x856;
const HEALTH_RING: u32 = 0x857;

const CELL: u32 = 0x860;
// References in our cell.
const CELL_CENTER: u32 = 0x861;
const CELL_ENTRY: u32 = 0x862;
const ALPHA: u32 = 0x863;
const BETA: u32 = 0x864;
const GAMMA: u32 = 0x865;
const VICTIM: u32 = 0x866;
const DOOR_REF: u32 = 0x867;
const WEB_REF: u32 = 0x868;
const WEB_REF2: u32 = 0x869;
const RESET_CHEST_REF: u32 = 0x86A;
const DELETE_ME: u32 = 0x86B;
const LINK_A: u32 = 0x86C;
const LINK_B: u32 = 0x86D;
const LINK_C: u32 = 0x86E;
const LINK_K: u32 = 0x86F;
const FLOATING_BOWL: u32 = 0x87B;
const REMEMBER_BOWL: u32 = 0x87C;
const FLOORS: u32 = 0x870; // 9 floor pieces
// In WarehouseFurniture.
const WH_CENTER: u32 = 0x880;
const WH_ENTRY: u32 = 0x881;

/// Probes: an effect (`PROBES + 2i`) and an ability (`+ 2i + 1`) each.
const PROBES: u32 = 0x900;

/// A condition parameter.
#[derive(Clone, Copy)]
enum P {
    None,
    Own(u32),
    Vanilla(&'static str),
    Float(f32),
    Int(u32),
    /// An axis, as the game stores one (`'X'`).
    Axis(char),
    /// A variable name (`CIS2`).
    Var(&'static str),
}

/// Condition comparisons.
const EQ: u8 = 0;
const GE: u8 = 3;

/// What a probe's condition compares with.
#[derive(Clone, Copy)]
enum V {
    Value(f32),
    /// The global `ProbeValue`, which the scripts set.
    Probe,
}

/// A probe: its property suffix, the condition, and a reference it runs on
/// (else the probe's holder).
struct Probe {
    name: &'static str,
    func: &'static str,
    p1: P,
    p2: P,
    op: u8,
    value: V,
    on: Option<u32>,
}

const fn probe(name: &'static str, func: &'static str, p1: P, p2: P, op: u8, value: V) -> Probe {
    Probe {
        name,
        func,
        p1,
        p2,
        op,
        value,
        on: None,
    }
}

const ONE: V = V::Value(1.0);

const PROBE_LIST: &[Probe] = &[
    probe("Win32", "IsWin32", P::None, P::None, EQ, ONE),
    probe("Gore", "IsGoreDisabled", P::None, P::None, EQ, ONE),
    probe("FirstPerson", "IsPC1stPerson", P::None, P::None, EQ, ONE),
    probe(
        "XPNext",
        "GetXPForNextLevel",
        P::None,
        P::None,
        EQ,
        V::Probe,
    ),
    probe(
        "XPNextGE",
        "GetXPForNextLevel",
        P::None,
        P::None,
        GE,
        V::Probe,
    ),
    probe(
        "Controls00",
        "GetPlayerControlsDisabled",
        P::Int(0),
        P::Int(0),
        EQ,
        ONE,
    ),
    probe(
        "Controls10",
        "GetPlayerControlsDisabled",
        P::Int(1),
        P::Int(0),
        EQ,
        ONE,
    ),
    probe(
        "Controls01",
        "GetPlayerControlsDisabled",
        P::Int(0),
        P::Int(1),
        EQ,
        ONE,
    ),
    probe("CrimeGold", "GetCrimeGold", P::None, P::None, EQ, V::Probe),
    probe(
        "KwCount",
        "GetKeywordItemCount",
        P::Vanilla("WeapTypeSword"),
        P::None,
        EQ,
        V::Probe,
    ),
    probe(
        "AnimType",
        "GetWeaponAnimType",
        P::None,
        P::None,
        EQ,
        V::Probe,
    ),
    probe(
        "TwoHanded",
        "HasTwoHandedWeaponEquipped",
        P::None,
        P::None,
        EQ,
        ONE,
    ),
    probe("Torch", "IsActorUsingATorch", P::None, P::None, EQ, ONE),
    probe("TorchOut", "IsTorchOut", P::None, P::None, EQ, ONE),
    probe("ShieldOut", "IsShieldOut", P::None, P::None, EQ, ONE),
    probe(
        "ClothingGE",
        "GetClothingValue",
        P::None,
        P::None,
        GE,
        V::Probe,
    ),
    probe("Creature", "GetIsCreature", P::None, P::None, EQ, ONE),
    probe("ScaleGE", "GetScale", P::None, P::None, GE, V::Probe),
    probe("Disease", "GetDisease", P::None, P::None, EQ, ONE),
    probe("Unconscious", "GetUnconscious", P::None, P::None, EQ, ONE),
    probe(
        "Stamina50",
        "GetStaminaPercentage",
        P::None,
        P::None,
        GE,
        V::Value(50.0),
    ),
    probe(
        "Stamina03",
        "GetStaminaPercentage",
        P::None,
        P::None,
        GE,
        V::Value(0.3),
    ),
    probe(
        "Stamina05",
        "GetStaminaPercentage",
        P::None,
        P::None,
        GE,
        V::Value(0.5),
    ),
    probe("OverEnc", "IsOverEncumbered", P::None, P::None, EQ, ONE),
    probe("Protected", "IsProtected", P::None, P::None, EQ, ONE),
    probe("SameFaction", "SameFaction", P::Own(BETA), P::None, EQ, ONE),
    probe("SameRace", "SameRace", P::Own(GAMMA), P::None, EQ, ONE),
    probe(
        "ReactionRG",
        "GetFactionCombatReaction",
        P::Own(RED),
        P::Own(GREEN),
        EQ,
        V::Probe,
    ),
    probe(
        "ReactionGR",
        "GetFactionCombatReaction",
        P::Own(GREEN),
        P::Own(RED),
        EQ,
        V::Probe,
    ),
    probe(
        "ReactionTT",
        "GetFactionCombatReaction",
        P::Own(TEAL),
        P::Own(TEAL),
        EQ,
        V::Probe,
    ),
    probe(
        "FactionRelation",
        "GetFactionRelation",
        P::Own(BETA),
        P::None,
        EQ,
        V::Probe,
    ),
    probe(
        "RankDiff",
        "GetFactionRankDifference",
        P::Own(TEAL),
        P::Own(BETA),
        EQ,
        V::Probe,
    ),
    probe(
        "ShouldHelp",
        "GetShouldHelp",
        P::Own(BETA),
        P::None,
        EQ,
        ONE,
    ),
    probe(
        "InCellParam",
        "GetInCellParam",
        P::Own(CELL),
        P::Own(BETA),
        EQ,
        ONE,
    ),
    probe(
        "ScriptVar",
        "GetScriptVariable",
        P::Own(ALPHA),
        P::Var("Marker"),
        EQ,
        V::Value(7.0),
    ),
    probe(
        "ScriptVarMissing",
        "GetScriptVariable",
        P::Own(ALPHA),
        P::Var("VrmNoSuchVariable"),
        EQ,
        V::Value(0.0),
    ),
    probe(
        "Heading",
        "GetHeadingAngle",
        P::Own(BETA),
        P::None,
        GE,
        V::Probe,
    ),
    probe(
        "RelAngle",
        "GetRelativeAngle",
        P::Own(BETA),
        P::None,
        GE,
        V::Probe,
    ),
    probe(
        "Within300",
        "GetWithinDistance",
        P::Own(BETA),
        P::Float(300.0),
        EQ,
        ONE,
    ),
    probe(
        "Closer",
        "IsCloserToAThanB",
        P::Own(BETA),
        P::Own(GAMMA),
        EQ,
        ONE,
    ),
    probe("LOS", "GetLineOfSight", P::Own(BETA), P::None, EQ, ONE),
    probe(
        "StartX",
        "GetStartingPos",
        P::Axis('X'),
        P::None,
        EQ,
        V::Probe,
    ),
    probe("KillerOf", "IsKiller", P::Own(VICTIM), P::None, EQ, ONE),
    Probe {
        on: Some(VICTIM),
        ..probe("KillerRev", "IsKiller", P::Own(ALPHA), P::None, EQ, ONE)
    },
    probe(
        "KillerBase",
        "IsKillerObject",
        P::Own(KILLER_BASES),
        P::None,
        EQ,
        ONE,
    ),
    probe(
        "KillerRef",
        "IsKillerObject",
        P::Own(KILLER_REFS),
        P::None,
        EQ,
        ONE,
    ),
    Probe {
        on: Some(VICTIM),
        ..probe("Cause", "GetCauseofDeath", P::None, P::None, EQ, V::Probe)
    },
    Probe {
        on: Some(VICTIM),
        ..probe("TimeDead", "GetTimeDead", P::None, P::None, GE, V::Probe)
    },
    // Health is actor value 24.
    probe(
        "HealthHigh",
        "GetActorValuePercent",
        P::Int(24),
        P::None,
        GE,
        V::Value(0.9),
    ),
    probe("InCombat0", "IsInCombat", P::Int(0), P::None, EQ, ONE),
    probe("InCombat1", "IsInCombat", P::Int(1), P::None, EQ, ONE),
    probe("InCombat2", "IsInCombat", P::Int(2), P::None, EQ, ONE),
];

/// Vanilla forms the scripts name: property, type, editor id.
const VANILLA: &[(&str, &str, &str)] = &[
    ("XMarker", "Static", "XMarker"),
    ("IronSword", "Weapon", "IronSword"),
    ("SteelSword", "Weapon", "SteelSword"),
    ("IronGreatsword", "Weapon", "IronGreatsword"),
    ("IronCuirass", "Armor", "ArmorIronCuirass"),
    ("LeatherCuirass", "Armor", "ArmorLeatherCuirass"),
    ("IronShield", "Armor", "ArmorIronShield"),
    ("Torch", "Light", "Torch01"),
    ("Gold", "MiscObject", "Gold001"),
    ("IngotIron", "MiscObject", "IngotIron"),
    ("SweetRoll", "Potion", "FoodSweetroll"),
    ("PotionHealth", "Potion", "RestoreHealth01"),
    ("WeapTypeSword", "Keyword", "WeapTypeSword"),
    ("ArmorCuirassKw", "Keyword", "ArmorCuirass"),
    ("CitizenClass", "Class", "Citizen"),
    ("NordRace", "Race", "NordRace"),
    ("BossType", "LocationRefType", "Boss"),
    ("BleakFalls", "Location", "BleakFallsBarrowLocation"),
    ("WhiterunHold", "Location", "WhiterunHoldLocation"),
    ("WhiterunCity", "Location", "WhiterunLocation"),
    ("FlamesSpell", "Spell", "Flames"),
    ("Ataxia", "Spell", "DiseaseAtaxia"),
    ("UISound", "Sound", "UIMenuOK"),
    ("GameHour", "GlobalVariable", "GameHour"),
    ("WolfBase", "ActorBase", "EncWolf"),
    ("EmptyChest", "Container", "BarrelEmpty01"),
    ("Barrel", "Container", "BarrelEmpty01"),
    ("CalmSpell", "Spell", "Calm"),
    ("FurySpell", "Spell", "Fury"),
    ("FearSpell", "Spell", "Fear"),
    ("FlameCloakSpell", "Spell", "flameCloak"),
    ("EtherealSpell", "Spell", "VoiceBecomeEthereal1"),
    ("EtherealEffect", "MagicEffect", "VoiceMakeEthereal"),
    ("SlowTimeSpell", "Spell", "VoiceSlowTime1"),
    ("SlowTimeEffect", "MagicEffect", "VoiceSlowTimeEffect"),
    ("Bowl", "MiscObject", "BasicWoodenBowl01"),
    ("SoulGemPetty", "SoulGem", "SoulGemPetty"),
    ("SoulGemPettyFilled", "SoulGem", "SoulGemPettyFilled"),
    ("SoulGemGrand", "SoulGem", "SoulGemGrand"),
    ("SoulGemGrandFilled", "SoulGem", "SoulGemGrandFilled"),
    ("SoulGemBlack", "SoulGem", "SoulGemBlack"),
    ("SoulGemBlackFilled", "SoulGem", "SoulGemBlackFilled"),
    ("KhaylaBase", "ActorBase", "Khayla"),
    ("DerveninBase", "ActorBase", "Dervenin"),
    ("DeerBase", "ActorBase", "EncDeer"),
    ("ArnielSummonBase", "ActorBase", "MGArnielSummon"),
    ("HareBase", "ActorBase", "EncHare"),
    ("MudcrabBase", "ActorBase", "EncMudcrabMedium"),
    ("SkeletonBase", "ActorBase", "EncSkeleton01Melee1H"),
    ("AtronachBase", "ActorBase", "EncAtronachFlame"),
    ("SpiderBase", "ActorBase", "EncDwarvenSpider01"),
    ("VampireBase", "ActorBase", "EncVampire00BretonF"),
    ("Wheat", "Ingredient", "Wheat"),
    ("BoundSwordSpell", "Spell", "BoundSword"),
    ("BoundSword", "Weapon", "BoundWeaponSword"),
    ("DisarmSpell", "Spell", "VoiceDisarm1"),
    ("FireboltSpell", "Spell", "Firebolt"),
    ("VigilantBase", "ActorBase", "EncVigilantOfStendarr01NordF"),
    ("WarlockBase", "ActorBase", "EncWarlockFire03BretonM"),
    ("SparksStaff", "Weapon", "StaffSparks"),
    ("GerdurBase", "ActorBase", "Gerdur"),
    ("FrodnarBase", "ActorBase", "Frodnar"),
    ("HodBase", "ActorBase", "Hod"),
    ("AlvorBase", "ActorBase", "Alvor"),
    ("DortheBase", "ActorBase", "Dorthe"),
    ("ParentChildType", "AssociationType", "ParentChild"),
    ("SpouseType", "AssociationType", "Spouse"),
    ("ArmorHeavyKw", "Keyword", "ArmorHeavy"),
    ("ActorTypeNPC", "Keyword", "ActorTypeNPC"),
    ("MagicCloakKw", "Keyword", "MagicCloak"),
    ("InvisibilitySpell", "Spell", "Invisibility"),
    ("FireSword", "Weapon", "EnchIronSwordFire01"),
    ("Dagger", "Weapon", "IronDagger"),
    ("TimeScale", "GlobalVariable", "TimeScale"),
    ("WhiterunMarker", "ObjectReference", "WhiterunMapMarkerREF"),
    ("SolitudeMarker", "ObjectReference", "SolitudeMapmarkerRef"),
];

/// The weathers whose transitions are timed, with their trans deltas: the
/// game's are 125 but for a few (DA02's 82, the Blue Palace fogs' 61).
fn weathers(lo: &LoadOrder) -> Result<Vec<(FormId, u8)>> {
    [
        "SkyrimClear",
        "SkyrimDA02Weather",
        "SolitudeBluePalaceFogNMARE",
    ]
    .iter()
    .map(|e| {
        let w = lo
            .find_editor_id(e)
            .with_context(|| format!("{e} not found"))?;
        let d = lo
            .get(w)
            .and_then(|r| r.get(b"DATA").and_then(|d| d.get(3).copied()))
            .context("a weather's delta")?;
        Ok((w, d))
    })
    .collect()
}

/// Map markers far from Riverwood, the first not yet on the map tried.
const REMOTE: &[&str] = &[
    "AlftandMapMarker",
    "LoreiusMapMarker",
    "SaarthalMapMarker",
    "EboncrestMapMarker",
    "MerryFairMapMarker",
    "AngasMillMapMarker",
];

/// Our forms the scripts name: property, type, object id.
const OWN: &[(&str, &str, u32)] = &[
    ("SideQuest", "Quest", SIDE_QUEST),
    ("AliasQuest", "Quest", ALIAS_QUEST),
    ("ProbeValue", "GlobalVariable", PROBE_VALUE),
    ("RedFaction", "Faction", RED),
    ("GreenFaction", "Faction", GREEN),
    ("TealFaction", "Faction", TEAL),
    ("GroupedFaction", "Faction", GROUPED),
    ("CrimeFaction", "Faction", CRIME),
    ("TwoSwords", "FormList", TWO_SWORDS),
    ("LinkKeyword", "Keyword", LINK_KEYWORD),
    ("OutfitA", "Outfit", OUTFIT_A),
    ("OutfitB", "Outfit", OUTFIT_B),
    ("NakedBase", "ActorBase", NAKED),
    ("LoggedBase", "ActorBase", LOGGED),
    ("RedBase", "ActorBase", RED_NPC),
    ("GreenBase", "ActorBase", GREEN_NPC),
    ("OutfitBase", "ActorBase", OUTFIT_NPC),
    ("ProtectBase", "ActorBase", PROTECT_NPC),
    ("InvulnerableBase", "ActorBase", INVULNERABLE_NPC),
    ("DeadCountBase", "ActorBase", DEAD_COUNT_NPC),
    ("GuardClassBase", "ActorBase", GUARD_CLASS_NPC),
    ("GuardFactionBase", "ActorBase", GUARD_FACTION_NPC),
    ("UniqueBase", "ActorBase", UNIQUE_NPC),
    ("TplUniqueChild", "ActorBase", TPL_UNIQUE_CHILD),
    ("TplPlainUniqueChild", "ActorBase", TPL_PLAIN_UNIQUE_CHILD),
    ("LvlStatsChild", "ActorBase", LVL_STATS_CHILD),
    ("LvlTraitsChild", "ActorBase", LVL_TRAITS_CHILD),
    ("WhFireSpell", "Spell", FIRE_SPELL),
    ("CellCenter", "ObjectReference", CELL_CENTER),
    ("CellEntry", "ObjectReference", CELL_ENTRY),
    ("WhCenter", "ObjectReference", WH_CENTER),
    ("WhEntry", "ObjectReference", WH_ENTRY),
    ("Alpha", "Actor", ALPHA),
    ("Beta", "Actor", BETA),
    ("Gamma", "Actor", GAMMA),
    ("Victim", "Actor", VICTIM),
    ("DoorRef", "ObjectReference", DOOR_REF),
    ("WebRef", "ObjectReference", WEB_REF),
    ("WebRef2", "ObjectReference", WEB_REF2),
    ("ResetChest", "ObjectReference", RESET_CHEST_REF),
    ("DeleteMe", "ObjectReference", DELETE_ME),
    ("LinkA", "ObjectReference", LINK_A),
    ("LinkB", "ObjectReference", LINK_B),
    ("LinkC", "ObjectReference", LINK_C),
    ("LinkK", "ObjectReference", LINK_K),
    ("AbsorbMagickaSpell", "Spell", ABSORB_SPELL),
    ("HealthRing", "Armor", HEALTH_RING),
    ("LowBase", "ActorBase", LOW_NPC),
    ("Level8Base", "ActorBase", LEVEL8_NPC),
    ("PCMultDremora", "ActorBase", PC_MULT_DREMORA),
    ("FloatingBowl", "ObjectReference", FLOATING_BOWL),
    ("TealHelperBase", "ActorBase", TEAL_HELPER),
    ("RedFighterBase", "ActorBase", RED_FIGHTER),
    ("HelperBase", "ActorBase", HELPER),
    ("AverageBase", "ActorBase", AVERAGE),
    ("CowardBase", "ActorBase", COWARD),
    ("BruteBase", "ActorBase", BRUTE),
    ("GreenFighterBase", "ActorBase", GREEN_FIGHTER),
    ("MageBase", "ActorBase", MAGE),
    ("HealerBase", "ActorBase", HEALER),
    ("StaffBase", "ActorBase", STAFF_NPC),
    ("RememberBowl", "ObjectReference", REMEMBER_BOWL),
    ("CrimeGuardBase", "ActorBase", CRIME_GUARD),
    ("RemoteMarkers", "FormList", REMOTE_MARKERS),
    ("CreateQuest", "Quest", CREATE_QUEST),
    ("MatchQuests", "FormList", MATCH_QUESTS),
    ("LevelQuests", "FormList", LEVEL_QUESTS),
    ("MatchBase", "ActorBase", MATCH_NPC),
    ("MatchFar", "Actor", MATCH_FAR),
    ("MatchWh", "Actor", MATCH_WH),
];

/// The tests, in running order: every `String Function TestName()` of the
/// sources.
fn tests() -> Vec<String> {
    let mut out = Vec::new();
    for (_, src) in SOURCES {
        for line in src.lines() {
            let l = line.trim();
            if let Some(rest) = l.strip_prefix("String Function Test")
                && let Some(name) = rest.strip_suffix("()")
            {
                out.push(name.to_owned());
            }
        }
    }
    out
}

/// The main script's header: properties, variables and the runner.
fn header() -> String {
    let mut s = format!("Scriptname {SCRIPT} extends Quest\n");
    s += "Actor Property Player Auto\n";
    for (name, ty, _) in VANILLA {
        s += &format!("{ty} Property {name} Auto\n");
    }
    for (name, ty, _) in OWN {
        s += &format!("{ty} Property {name} Auto\n");
    }
    for p in PROBE_LIST {
        s += &format!("Spell Property PS_{} Auto\n", p.name);
        s += &format!("MagicEffect Property PE_{} Auto\n", p.name);
    }
    for name in ["DeltaA", "DeltaB", "DeltaC"] {
        s += &format!("Float Property {name} Auto\n");
    }
    for name in ["WeatherA", "WeatherB", "WeatherC"] {
        s += &format!("Weather Property {name} Auto\n");
    }
    s += "Bool Ran = false\n";
    s += "ObjectReference StartMarker\nWeather CellWeather\nInt CellSkyMode\n";
    let tests = tests();
    for t in &tests {
        s += &format!("String Result{t}\n");
    }
    s += "\nEvent OnInit()\n    RegisterForSingleUpdate(10.0)\nEndEvent\n\n";
    s += "Event OnUpdate()\n    If Ran\n        return\n    EndIf\n    Ran = true\n";
    s += &format!(
        "    Debug.Notification(\"vibe_rm autotest21 running: {} tests, about an hour. Leave the game be.\")\n",
        tests.len()
    );
    s += "    StartMarker = Player.PlaceAtMe(XMarker, 1, true, false)\n";
    s += "    GoCell()\n    Float t = 0.0\n";
    s += "    CellWeather = Weather.GetCurrentWeather()\n    CellSkyMode = Weather.GetSkyMode()\n";
    for (i, t) in tests.iter().enumerate() {
        s += &format!("    t = Now()\n    Result{t} = Test{t}()\n");
        s += &format!("    Result{t} += \"(took \" + Num(Now() - t) + \" s)\"\n");
        s += &format!("    Debug.Trace(\"vibe_rm autotest21 {t}: \" + Result{t})\n");
        if i % 20 == 19 {
            s += "    Game.RequestSave()\n    Utility.Wait(3.0)\n";
        }
    }
    s += "    Game.EnablePlayerControls(true, true, true, true, true, true, true, true, 0)\n";
    s += "    Game.RequestSave()\n    Utility.Wait(3.0)\n";
    s += "    Debug.MessageBox(\"vibe_rm autotest21 done: read the reports with vrm-tool test-results.\")\n";
    s += "EndEvent\n";
    s
}

/// The main script's source: header, then the test files.
fn main_source() -> String {
    let mut s = header();
    for (_, src) in SOURCES {
        s += "\n";
        s += src;
    }
    s
}

fn scripts() -> Result<Vec<(String, Vec<u8>)>> {
    let main = main_source();
    let mut sources: Vec<&str> = vec![&main];
    sources.extend(LOGGERS.iter().copied());
    script::compile(&sources)
}

fn func_index(name: &str) -> u16 {
    ctda_functions::FUNCTIONS
        .iter()
        .find(|f| f.1.eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("no condition function {name}"))
        .0
}

/// A plain condition on the subject: `func(p1, p2) <op> value`.
fn ctda(func: u16, p1: u32, p2: u32, op: u8, value: f32) -> [u8; 32] {
    bytes(&[
        &[op << 5, 0, 0, 0],
        &value.to_le_bytes(),
        &func.to_le_bytes(),
        &[0, 0],
        &p1.to_le_bytes(),
        &p2.to_le_bytes(),
        &0u32.to_le_bytes(),
        &0u32.to_le_bytes(),
        &u32::MAX.to_le_bytes(),
    ])
}

/// The game's event node for Story Manager event `code`, and its last
/// child, after which a node of ours goes.
fn event_root(lo: &LoadOrder, code: &str) -> Option<(FormId, FormId)> {
    let code: [u8; 4] = code.as_bytes().try_into().ok()?;
    let mut root = None;
    let mut nodes: Vec<esp::story::Node> = Vec::new();
    for tag in [b"SMEN", b"SMBN", b"SMQN"] {
        for &id in lo.ids_of_type(tag) {
            let Some(rec) = lo.get(id) else { continue };
            let Some(node) = esp::story::parse(&rec, id) else {
                continue;
            };
            if node.event() == Some(code) && id.0 >> 24 == 0 && tag == b"SMEN" {
                root = Some(id);
            }
            nodes.push(node);
        }
    }
    let root = root?;
    let kids: Vec<&esp::story::Node> = nodes.iter().filter(|n| n.parent == root).collect();
    let last = kids
        .iter()
        .find(|k| !kids.iter().any(|o| o.previous == k.id))
        .map_or(FormId::NULL, |k| k.id);
    Some((root, last))
}

struct Vanilla<'a> {
    lo: &'a LoadOrder,
}

impl Vanilla<'_> {
    fn id(&self, edid: &str) -> Result<FormId> {
        // References aren't indexed by editor id: those are looked for.
        let id = self
            .lo
            .find_editor_id(edid)
            .or_else(|| {
                self.lo.ids_of_type(b"REFR").iter().copied().find(|&r| {
                    self.lo
                        .get(r)
                        .and_then(|rec| rec.editor_id().map(|e| e.eq_ignore_ascii_case(edid)))
                        .unwrap_or(false)
                })
            })
            .with_context(|| format!("{edid} not found"))?;
        anyhow::ensure!(id.0 >> 24 == 0, "{edid} ({id}) isn't from Skyrim.esm");
        Ok(id)
    }
}

/// A probe's condition: `CTDA`, and `CIS2` for a variable name.
fn probe_condition(p: &Probe, v: &Vanilla) -> Result<Vec<([u8; 4], Vec<u8>)>> {
    let param = |p: P| -> Result<u32> {
        Ok(match p {
            P::None | P::Var(_) => 0,
            P::Own(o) => own(o).0,
            P::Vanilla(e) => v.id(e)?.0,
            P::Float(f) => f.to_bits(),
            P::Int(i) => i,
            P::Axis(c) => c as u32,
        })
    };
    let (flags, value) = match p.value {
        V::Value(f) => (0u8, f.to_bits()),
        // Flag 0x4: the value is a global's.
        V::Probe => (0x4, own(PROBE_VALUE).0),
    };
    let (run_on, reference) = match p.on {
        None => (0u32, 0u32),
        Some(r) => (2, own(r).0),
    };
    let ctda: [u8; 32] = bytes(&[
        &[p.op << 5 | flags, 0, 0, 0],
        &value.to_le_bytes(),
        &func_index(p.func).to_le_bytes(),
        &[0, 0],
        &param(p.p1)?.to_le_bytes(),
        &param(p.p2)?.to_le_bytes(),
        &run_on.to_le_bytes(),
        &reference.to_le_bytes(),
        &u32::MAX.to_le_bytes(),
    ]);
    let mut out = vec![(*b"CTDA", ctda.to_vec())];
    if let P::Var(name) = p.p2 {
        let mut s = name.as_bytes().to_vec();
        s.push(0);
        out.push((*b"CIS2", s));
    }
    Ok(out)
}

/// An NPC record of ours.
struct Npc {
    object_id: u32,
    edid: &'static str,
    /// `ACBS` flags (0x20 unique, 0x80 PC level mult, 0x2 essential, 0x800
    /// protected, 0x80000000 invulnerable).
    flags: u32,
    level: u16,
    /// Template and its flags (0x1 traits, 0x2 stats, 0x4 factions).
    template: Option<(FormId, u16)>,
    race: FormId,
    class: FormId,
    factions: Vec<(FormId, i8)>,
    /// Aggression, confidence, assistance.
    temper: (u8, u8, u8),
    script: Option<&'static str>,
    outfit: Option<FormId>,
    /// Inventory (`CNTO`): item, count.
    items: Vec<(FormId, u32)>,
    spells: Vec<FormId>,
    combat_style: Option<FormId>,
    crime_faction: Option<FormId>,
}

impl Npc {
    fn record(self) -> RecordBuilder {
        let (tpl, tpl_flags) = self.template.unwrap_or((FormId(0), 0));
        let acbs: [u8; 24] = bytes(&[
            &self.flags.to_le_bytes(),
            &0i16.to_le_bytes(),
            &0i16.to_le_bytes(),
            &self.level.to_le_bytes(),
            &1u16.to_le_bytes(),
            &0u16.to_le_bytes(),
            &100u16.to_le_bytes(),
            &35i16.to_le_bytes(),
            &tpl_flags.to_le_bytes(),
            &0i16.to_le_bytes(),
            &0u16.to_le_bytes(),
        ]);
        let mut aidt = [0u8; 20];
        aidt[..6].copy_from_slice(&[self.temper.0, self.temper.1, 50, 3, 0, self.temper.2]);
        let dnam: [u8; 52] = bytes(&[
            &[15; 18],
            &[0; 18],
            &50u16.to_le_bytes(),
            &50u16.to_le_bytes(),
            &50u16.to_le_bytes(),
            &[0; 10],
        ]);
        let bounds: Vec<u8> = [-41i16, -25, 0, 41, 72, 86]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let mut rec = RecordBuilder::new(b"NPC_", own(self.object_id)).zstring(b"EDID", self.edid);
        if let Some(name) = self.script {
            rec = rec.field(
                b"VMAD",
                &vmad(&[AttachedScript {
                    name: name.into(),
                    properties: Vec::new(),
                }]),
            );
        }
        rec = rec.field(b"OBND", &bounds).field(b"ACBS", &acbs);
        for &(faction, rank) in &self.factions {
            let snam: [u8; 8] = bytes(&[&faction.0.to_le_bytes(), &[rank as u8, 0, 0, 0]]);
            rec = rec.field(b"SNAM", &snam);
        }
        if self.template.is_some() {
            rec = rec.form(b"TPLT", tpl);
        }
        rec = rec.form(b"RNAM", self.race);
        // The spell list is counted, like inventory, and follows the race.
        if !self.spells.is_empty() {
            rec = rec.u32(b"SPCT", self.spells.len() as u32);
            for sp in &self.spells {
                rec = rec.form(b"SPLO", *sp);
            }
        }
        if !self.items.is_empty() {
            rec = rec.u32(b"COCT", self.items.len() as u32);
            for (item, count) in &self.items {
                let cnto: [u8; 8] = bytes(&[&item.0.to_le_bytes(), &count.to_le_bytes()]);
                rec = rec.field(b"CNTO", &cnto);
            }
        }
        rec = rec
            .field(b"AIDT", &aidt)
            .form(b"CNAM", self.class)
            .zstring(b"FULL", "vibe_rm Test NPC")
            .field(b"DATA", &[])
            .field(b"DNAM", &dnam);
        if let Some(style) = self.combat_style {
            rec = rec.form(b"ZNAM", style);
        }
        rec = rec
            .field(b"NAM5", &[0xFF, 0])
            .f32(b"NAM6", 1.0)
            .f32(b"NAM7", 50.0)
            .u32(b"NAM8", 1);
        if let Some(o) = self.outfit {
            rec = rec.form(b"DOFT", o);
        }
        if let Some(cf) = self.crime_faction {
            rec = rec.form(b"CRIF", cf);
        }
        rec
    }
}

fn form_list(object_id: u32, edid: &str, forms: &[FormId]) -> RecordBuilder {
    let mut rec = RecordBuilder::new(b"FLST", own(object_id)).zstring(b"EDID", edid);
    for f in forms {
        rec = rec.form(b"LNAM", *f);
    }
    rec
}

fn outfit(object_id: u32, edid: &str, items: &[FormId]) -> RecordBuilder {
    let inam: Vec<u8> = items.iter().flat_map(|f| f.0.to_le_bytes()).collect();
    RecordBuilder::new(b"OTFT", own(object_id))
        .zstring(b"EDID", edid)
        .field(b"INAM", &inam)
}

/// A faction: `DATA` flags (0x40 tracks crime), reactions (faction,
/// reaction: 1 enemy, 2 ally, 3 friend) and a crime group list.
fn faction(
    object_id: u32,
    edid: &str,
    flags: u32,
    reactions: &[(u32, u32)],
    group: Option<u32>,
) -> RecordBuilder {
    let mut rec = RecordBuilder::new(b"FACT", own(object_id))
        .zstring(b"EDID", edid)
        .zstring(b"FULL", edid);
    for &(other, reaction) in reactions {
        let xnam: [u8; 12] = bytes(&[
            &own(other).0.to_le_bytes(),
            &0i32.to_le_bytes(),
            &reaction.to_le_bytes(),
        ]);
        rec = rec.field(b"XNAM", &xnam);
    }
    rec = rec.u32(b"DATA", flags);
    if let Some(g) = group {
        rec = rec.form(b"CRGR", own(g));
    }
    let crva: [u8; 20] = bytes(&[
        &[1, 0],
        &1000u16.to_le_bytes(),
        &40u16.to_le_bytes(),
        &5u16.to_le_bytes(),
        &25u16.to_le_bytes(),
        &0u16.to_le_bytes(),
        &0.5f32.to_le_bytes(),
        &100u16.to_le_bytes(),
        &1000u16.to_le_bytes(),
    ]);
    rec.field(b"CRVA", &crva)
}

fn attached(name: &str) -> AttachedScript {
    AttachedScript {
        name: name.into(),
        properties: Vec::new(),
    }
}

/// A persistent reference of `base` in our cell.
fn reference(object_id: u32, base: FormId, pos: [f32; 3], rot_z: f32) -> RecordBuilder {
    let tag = if base.0 >> 24 == 1
        && (matches!(base.0 & 0xFFF, NAKED..=PC_MULT_DREMORA)
            || matches!(base.0 & 0xFFF, CRIME_GUARD | MATCH_NPC))
    {
        b"ACHR"
    } else {
        b"REFR"
    };
    RecordBuilder::new(tag, own(object_id))
        .flags(record_flags::PERSISTENT)
        .form(b"NAME", base)
        .field(b"DATA", &placement(pos, rot_z))
}

/// `XLKR`: a linked reference, under `keyword` (none: 0).
fn linked(rec: RecordBuilder, keyword: FormId, to: FormId) -> RecordBuilder {
    let xlkr: [u8; 8] = bytes(&[&keyword.0.to_le_bytes(), &to.0.to_le_bytes()]);
    rec.field(b"XLKR", &xlkr)
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let v = Vanilla { lo };
    let object = PropertyValue::Object;

    // The main script's properties.
    let mut properties = vec![("Player".to_owned(), object(PLAYER_REF))];
    for (name, _, edid) in VANILLA {
        properties.push((name.to_string(), object(v.id(edid)?)));
    }
    for (name, _, id) in OWN {
        properties.push((name.to_string(), object(own(*id))));
    }
    for (k, (w, delta)) in weathers(lo)?.into_iter().enumerate() {
        let letter = ["A", "B", "C"][k];
        properties.push((format!("Weather{letter}"), object(w)));
        properties.push((format!("Delta{letter}"), PropertyValue::Float(delta as f32)));
    }
    for (i, p) in PROBE_LIST.iter().enumerate() {
        let i = i as u32;
        properties.push((format!("PS_{}", p.name), object(own(PROBES + 2 * i + 1))));
        properties.push((format!("PE_{}", p.name), object(own(PROBES + 2 * i))));
    }

    let quests = vec![
        records::Quest {
            object_id: QUEST,
            edid: "VrmAt21Quest",
            flags: 0x11,
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
        records::Quest {
            object_id: SIDE_QUEST,
            edid: "VrmAt21SideQuest",
            flags: 0,
            priority: 0,
            kind: 0,
            script: None,
            stages: &[10],
            aliases: Vec::new(),
        }
        .record(),
        records::Quest {
            object_id: ALIAS_QUEST,
            edid: "VrmAt21AliasQuest",
            flags: 0,
            priority: 0,
            kind: 0,
            script: None,
            stages: &[],
            aliases: vec![records::Alias {
                id: 0,
                name: "Guarded",
                // Optional, essential (0x40).
                flags: records::alias_flags::OPTIONAL | 0x40,
                fill: records::Fill::Empty,
                packages: Vec::new(),
            }],
        }
        .record(),
    ];

    let mut quests = quests;
    let anchor = || records::Alias {
        id: 0,
        name: "Anchor",
        flags: 0,
        fill: records::Fill::Forced(own(CELL_CENTER)),
        packages: Vec::new(),
    };
    let create = |object: FormId, level: u32, id: u32, name: &'static str| records::Alias {
        id,
        name,
        flags: 0,
        fill: records::Fill::Create {
            object,
            at: 0,
            level,
        },
        packages: Vec::new(),
    };
    let quest = |object_id, edid, aliases| {
        records::Quest {
            object_id,
            edid,
            flags: 0,
            priority: 0,
            kind: 0,
            script: None,
            stages: &[],
            aliases,
        }
        .record()
    };
    quests.push(quest(
        CREATE_QUEST,
        "VrmAt21CreateQuest",
        vec![
            anchor(),
            create(own(NAKED), 4, 1, "Made"),
            create(v.id("IronSword")?, 4, 2, "Item"),
        ],
    ));
    let is_match = ctda(func_index("GetIsID"), own(MATCH_NPC).0, 0, EQ, 1.0);
    for (i, (edid, flags)) in [
        ("VrmAt21MatchQuest", 0),
        (
            "VrmAt21MatchLoadedQuest",
            records::alias_flags::IN_LOADED_AREA,
        ),
        (
            "VrmAt21MatchClosestQuest",
            records::alias_flags::IN_LOADED_AREA | records::alias_flags::CLOSEST,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        quests.push(quest(
            MATCH_QUEST + i as u32,
            edid,
            vec![records::Alias {
                id: 0,
                name: "Match",
                flags: flags | records::alias_flags::OPTIONAL,
                fill: records::Fill::Matching(vec![is_match]),
                packages: Vec::new(),
            }],
        ));
    }
    let bandit = v.id("LCharBanditMelee1H")?;
    for (i, (edid, level)) in [("VrmAt21LevelEasyQuest", 0), ("VrmAt21LevelHardQuest", 3)]
        .into_iter()
        .enumerate()
    {
        quests.push(quest(
            LEVEL_QUEST + i as u32,
            edid,
            vec![anchor(), create(bandit, level, 1, "Made")],
        ));
    }
    // Story quests: each the handler script, its main quest the runner's.
    let mut story_nodes = Vec::new();
    for (i, code) in STORY_EVENTS.iter().enumerate() {
        let i = i as u32;
        let edid = format!("VrmAt21Story{code}");
        quests.push(
            records::Quest {
                object_id: STORY + 2 * i,
                edid: &edid,
                flags: 0,
                priority: 0,
                kind: 0,
                script: Some(AttachedScript {
                    name: "VrmAt21StoryLog".into(),
                    properties: vec![("Main".to_owned(), PropertyValue::Object(own(QUEST)))],
                }),
                stages: &[],
                aliases: Vec::new(),
            }
            .record(),
        );
        match event_root(lo, code) {
            Some((root, last)) => story_nodes.push(records::story_node(
                STORY + 2 * i + 1,
                &format!("VrmAt21StoryNode{code}"),
                root,
                last,
                &[],
                0,
                Some(&[(own(STORY + 2 * i), 0, 0.0)]),
            )),
            None => eprintln!("autotest21: no {code} event node; its events go unheard"),
        }
    }

    let factions = vec![
        faction(RED, "VrmAt21Red", 0, &[(GREEN, 1)], None),
        faction(GREEN, "VrmAt21Green", 0, &[], None),
        faction(TEAL, "VrmAt21Teal", 0, &[], None),
        faction(GROUPED, "VrmAt21Grouped", 0x40, &[], Some(CRIME_GROUP)),
        faction(CRIME, "VrmAt21Crime", 0x40, &[], None),
    ];

    let iron = v.id("ArmorIronCuirass")?;
    let leather = v.id("ArmorLeatherCuirass")?;
    let lists = vec![
        form_list(
            TWO_SWORDS,
            "VrmAt21TwoSwords",
            &[v.id("IronSword")?, v.id("SteelSword")?],
        ),
        form_list(CRIME_GROUP, "VrmAt21CrimeGroup", &[own(RED)]),
        form_list(KILLER_BASES, "VrmAt21KillerBases", &[own(VICTIM_NPC)]),
        form_list(KILLER_REFS, "VrmAt21KillerRefs", &[own(VICTIM)]),
        form_list(
            MATCH_QUESTS,
            "VrmAt21MatchQuests",
            &[own(MATCH_QUEST), own(MATCH_QUEST + 1), own(MATCH_QUEST + 2)],
        ),
        form_list(
            LEVEL_QUESTS,
            "VrmAt21LevelQuests",
            &[own(LEVEL_QUEST), own(LEVEL_QUEST + 1)],
        ),
        form_list(
            REMOTE_MARKERS,
            "VrmAt21RemoteMarkers",
            &REMOTE.iter().map(|e| v.id(e)).collect::<Result<Vec<_>>>()?,
        ),
    ];
    let keywords = vec![records::keyword(LINK_KEYWORD, "VrmAt21LinkKeyword")];
    let outfits = vec![
        outfit(OUTFIT_A, "VrmAt21OutfitA", &[iron]),
        outfit(OUTFIT_B, "VrmAt21OutfitB", &[leather]),
    ];

    // NPCs: a commoner's looks (traits template), naked, unaggressive.
    let commoner = v.id("WEMerchantMiscMaleCommoner")?;
    let commoner_race = lo
        .get(commoner)
        .and_then(|r| {
            r.get(b"RNAM")
                .map(|d| FormId(u32::from_le_bytes(d[..4].try_into().unwrap())))
        })
        .context("the commoner's race")?;
    let citizen = v.id("Citizen")?;
    let npc = |object_id, edid| Npc {
        object_id,
        edid,
        flags: 0,
        level: 10,
        template: Some((commoner, 0x1)),
        race: commoner_race,
        class: citizen,
        factions: Vec::new(),
        temper: (0, 3, 0),
        script: None,
        outfit: None,
        items: Vec::new(),
        spells: Vec::new(),
        combat_style: None,
        crime_faction: None,
    };
    let teal = own(TEAL);
    let sword = v.id("IronSword")?;
    let npcs: Vec<RecordBuilder> = vec![
        npc(NAKED, "VrmAt21Naked").record(),
        Npc {
            script: Some("VrmAt21ActorLog"),
            ..npc(LOGGED, "VrmAt21Logged")
        }
        .record(),
        Npc {
            factions: vec![(own(RED), 0)],
            ..npc(RED_NPC, "VrmAt21RedNpc")
        }
        .record(),
        Npc {
            factions: vec![(own(GREEN), 0)],
            ..npc(GREEN_NPC, "VrmAt21GreenNpc")
        }
        .record(),
        Npc {
            outfit: Some(own(OUTFIT_A)),
            ..npc(OUTFIT_NPC, "VrmAt21OutfitNpc")
        }
        .record(),
        npc(PROTECT_NPC, "VrmAt21ProtectNpc").record(),
        Npc {
            flags: 0x8000_0000,
            ..npc(INVULNERABLE_NPC, "VrmAt21InvulnerableNpc")
        }
        .record(),
        npc(DEAD_COUNT_NPC, "VrmAt21DeadCountNpc").record(),
        Npc {
            class: v.id("GuardImperial")?,
            ..npc(GUARD_CLASS_NPC, "VrmAt21GuardClassNpc")
        }
        .record(),
        Npc {
            factions: vec![(v.id("IsGuardFaction")?, 0)],
            ..npc(GUARD_FACTION_NPC, "VrmAt21GuardFactionNpc")
        }
        .record(),
        Npc {
            flags: 0x20,
            ..npc(UNIQUE_NPC, "VrmAt21UniqueNpc")
        }
        .record(),
        Npc {
            template: Some((v.id("Gerdur")?, 0x1)),
            ..npc(TPL_UNIQUE_CHILD, "VrmAt21TplUniqueChild")
        }
        .record(),
        Npc {
            flags: 0x20,
            template: Some((own(NAKED), 0x1)),
            ..npc(TPL_PLAIN_UNIQUE_CHILD, "VrmAt21TplPlainUniqueChild")
        }
        .record(),
        Npc {
            template: Some((v.id("LCharBanditMelee1H")?, 0x2)),
            ..npc(LVL_STATS_CHILD, "VrmAt21LvlStatsChild")
        }
        .record(),
        Npc {
            template: Some((v.id("LCharBanditMelee1H")?, 0x1)),
            race: v.id("NordRace")?,
            ..npc(LVL_TRAITS_CHILD, "VrmAt21LvlTraitsChild")
        }
        .record(),
        Npc {
            script: Some("VrmAt21ActorLog"),
            factions: vec![(teal, 0)],
            ..npc(ALPHA_NPC, "VrmAt21AlphaNpc")
        }
        .record(),
        Npc {
            factions: vec![(teal, -1)],
            ..npc(BETA_NPC, "VrmAt21BetaNpc")
        }
        .record(),
        Npc {
            template: Some((v.id("EncBandit01Melee1HOrcM")?, 0x1)),
            ..npc(GAMMA_NPC, "VrmAt21GammaNpc")
        }
        .record(),
        npc(VICTIM_NPC, "VrmAt21VictimNpc").record(),
        Npc {
            level: 1,
            ..npc(LOW_NPC, "VrmAt21LowNpc")
        }
        .record(),
        Npc {
            level: 8,
            ..npc(LEVEL8_NPC, "VrmAt21Level8Npc")
        }
        .record(),
        Npc {
            factions: vec![(teal, 0)],
            temper: (1, 3, 1),
            items: vec![(sword, 1)],
            ..npc(TEAL_HELPER, "VrmAt21TealHelper")
        }
        .record(),
        Npc {
            factions: vec![(own(RED), 0)],
            temper: (1, 3, 0),
            items: vec![(sword, 1)],
            ..npc(RED_FIGHTER, "VrmAt21RedFighter")
        }
        .record(),
        Npc {
            temper: (1, 3, 2),
            items: vec![(sword, 1)],
            ..npc(HELPER, "VrmAt21Helper")
        }
        .record(),
        Npc {
            temper: (1, 2, 0),
            items: vec![(sword, 1)],
            ..npc(AVERAGE, "VrmAt21Average")
        }
        .record(),
        Npc {
            temper: (1, 0, 0),
            items: vec![(sword, 1)],
            ..npc(COWARD, "VrmAt21Coward")
        }
        .record(),
        Npc {
            level: 50,
            temper: (2, 4, 0),
            items: vec![(v.id("IronGreatsword")?, 1)],
            ..npc(BRUTE, "VrmAt21Brute")
        }
        .record(),
        Npc {
            factions: vec![(own(GREEN), 0)],
            temper: (1, 3, 0),
            items: vec![(sword, 1)],
            ..npc(GREEN_FIGHTER, "VrmAt21GreenFighter")
        }
        .record(),
        Npc {
            temper: (1, 3, 0),
            script: Some("VrmAt21ActorLog"),
            spells: vec![v.id("Firebolt")?],
            combat_style: Some(v.id("csHumanMagic")?),
            ..npc(MAGE, "VrmAt21Mage")
        }
        .record(),
        Npc {
            temper: (1, 3, 0),
            script: Some("VrmAt21ActorLog"),
            spells: vec![v.id("Healing")?],
            items: vec![(sword, 1)],
            ..npc(HEALER, "VrmAt21Healer")
        }
        .record(),
        Npc {
            temper: (1, 3, 0),
            combat_style: Some(v.id("csDragonPriest")?),
            ..npc(STAFF_NPC, "VrmAt21StaffNpc")
        }
        .record(),
        npc(MATCH_NPC, "VrmAt21MatchNpc").record(),
        Npc {
            factions: vec![(own(CRIME), 0)],
            crime_faction: Some(own(CRIME)),
            class: v.id("GuardImperial")?,
            temper: (1, 3, 0),
            items: vec![(sword, 1)],
            ..npc(CRIME_GUARD, "VrmAt21CrimeGuard")
        }
        .record(),
        records::copy(
            lo,
            "EncDremoraMelee06",
            PC_MULT_DREMORA,
            "VrmAt21PCMultDremora",
            None,
            |tag, d| {
                // ACBS: the PC level mult flag (0x80), level 1000 (x1.0), calc
                // min 46, max 0.
                if tag == b"ACBS" && d.len() >= 14 {
                    let flags = u32::from_le_bytes(d[0..4].try_into().unwrap()) | 0x80;
                    d[0..4].copy_from_slice(&flags.to_le_bytes());
                    d[8..10].copy_from_slice(&1000u16.to_le_bytes());
                    d[10..12].copy_from_slice(&46u16.to_le_bytes());
                    d[12..14].copy_from_slice(&0u16.to_le_bytes());
                }
            },
        )?,
    ];

    // Objects with a logger.
    let doors = vec![records::copy(
        lo,
        "WRDoor02",
        DOOR,
        "VrmAt21Door",
        Some(attached("VrmAt21RefLog")),
        |_, _| {},
    )?];
    let activators = vec![records::copy(
        lo,
        "FXspiderWebKitDestructible",
        WEB,
        "VrmAt21Web",
        Some(attached("VrmAt21RefLog")),
        |_, _| {},
    )?];
    // CNTO: three iron ingots.
    let ingot = v.id("IngotIron")?;
    let containers = vec![records::copy_inserting(
        lo,
        "BarrelEmpty01",
        RESET_CHEST,
        "VrmAt21ResetChest",
        None,
        |_, _| {},
        |tag| {
            if tag == b"OBND" {
                let cnto: Vec<u8> =
                    bytes::<8>(&[&ingot.0.to_le_bytes(), &3u32.to_le_bytes()]).to_vec();
                vec![(*b"COCT", 1u32.to_le_bytes().to_vec()), (*b"CNTO", cnto)]
            } else {
                Vec::new()
            }
        },
    )?];

    // A fire damage effect and spell for script casts (target actor).
    let mut effects = vec![records::copy(
        lo,
        "FireDamageFFAimed",
        FIRE_EFFECT,
        "VrmAt21FireEffect",
        None,
        |tag, d| {
            if tag == b"DATA" && d.len() >= 0x58 {
                d[0x50..0x54].copy_from_slice(&spell::FIRE_AND_FORGET.to_le_bytes());
                d[0x54..0x58].copy_from_slice(&spell::TARGET_ACTOR.to_le_bytes());
            }
        },
    )?];
    effects.push(records::copy(
        lo,
        "AbsorbMagickaConcAimed",
        ABSORB_EFFECT,
        "VrmAt21AbsorbMagickaEffect",
        None,
        |tag, d| {
            if tag == b"DATA" && d.len() >= 0x58 {
                d[0x50..0x54].copy_from_slice(&spell::FIRE_AND_FORGET.to_le_bytes());
                d[0x54..0x58].copy_from_slice(&spell::TARGET_ACTOR.to_le_bytes());
            }
        },
    )?);
    let mut spells = vec![
        records::spell(
            ABSORB_SPELL,
            "VrmAt21AbsorbMagicka",
            spell::SPELL,
            spell::FIRE_AND_FORGET,
            spell::TARGET_ACTOR,
            0,
            None,
            &[records::Effect::new(own(ABSORB_EFFECT), 20.0, 0)],
        ),
        records::spell(
            FIRE_SPELL,
            "VrmAt21FireSpell",
            spell::SPELL,
            spell::FIRE_AND_FORGET,
            spell::TARGET_ACTOR,
            0,
            None,
            &[records::Effect::new(own(FIRE_EFFECT), 10.0, 0)],
        ),
    ];

    // Probes: a harmless constant +3 OneHanded on self, its condition the
    // probe's (autotest5 found this copy works as an ability's effect).
    for (i, p) in PROBE_LIST.iter().enumerate() {
        let i = i as u32;
        let effect_id = PROBES + 2 * i;
        effects.push(records::copy(
            lo,
            "AlchFortifyOneHanded",
            effect_id,
            &format!("VrmAt21Probe{}Effect", p.name),
            None,
            |tag, d| {
                if tag == b"DATA" && d.len() >= 0x58 {
                    d[0x50..0x54].copy_from_slice(&spell::CONSTANT.to_le_bytes());
                    d[0x54..0x58].copy_from_slice(&spell::SELF.to_le_bytes());
                }
                if tag == b"CTDA" {
                    d.clear();
                }
            },
        )?);
        let condition = probe_condition(p, &v)?;
        let spit: [u8; 36] = bytes(&[
            &0u32.to_le_bytes(),
            &spell::MANUAL_COST.to_le_bytes(),
            &spell::ABILITY.to_le_bytes(),
            &0f32.to_le_bytes(),
            &spell::CONSTANT.to_le_bytes(),
            &spell::SELF.to_le_bytes(),
            &0f32.to_le_bytes(),
            &0f32.to_le_bytes(),
            &0u32.to_le_bytes(),
        ]);
        let efit: [u8; 12] = bytes(&[
            &3f32.to_le_bytes(),
            &0u32.to_le_bytes(),
            &0u32.to_le_bytes(),
        ]);
        let mut rec = RecordBuilder::new(b"SPEL", own(effect_id + 1))
            .zstring(b"EDID", &format!("VrmAt21Probe{}", p.name))
            .field(b"OBND", &[0; 12])
            .zstring(b"FULL", &format!("VrmAt21Probe{}", p.name))
            .zstring(b"DESC", "")
            .field(b"SPIT", &spit)
            .form(b"EFID", own(effect_id))
            .field(b"EFIT", &efit);
        for (tag, d) in condition {
            rec = rec.field(&tag, &d);
        }
        spells.push(rec);
    }

    let enchantment = v.id("EnchArmorFortifyHealth01")?;
    let armor = vec![records::copy_inserting(
        lo,
        "JewelryRingGold",
        HEALTH_RING,
        "VrmAt21HealthRing",
        None,
        |_, _| {},
        |tag| {
            if tag == b"OBND" {
                vec![(*b"EITM", enchantment.0.to_le_bytes().to_vec())]
            } else {
                Vec::new()
            }
        },
    )?];
    let globals = vec![records::global(PROBE_VALUE, "VrmAt21ProbeValue", 0.0)];

    // Our cell: nine floor pieces round the origin, lit.
    let floor = v.id("WRIntFloorSTMid01Large")?;
    let x_marker = v.id("XMarker")?;
    let heading = v.id("XMarkerHeading")?;
    let cell = RecordBuilder::new(b"CELL", own(CELL))
        .zstring(b"EDID", "VrmAt21Cell")
        .zstring(b"FULL", "vibe_rm Autotest 21")
        .field(b"DATA", &1u16.to_le_bytes())
        .field(b"XCLL", &xcll())
        .u32(b"LTMP", 0);
    let mut temporary = Vec::new();
    let mut k = 0;
    for gx in [-1.0f32, 0.0, 1.0] {
        for gy in [-1.0f32, 0.0, 1.0] {
            temporary.push(
                RecordBuilder::new(b"REFR", own(FLOORS + k))
                    .form(b"NAME", floor)
                    .field(b"DATA", &placement([gx * 1024.0, gy * 1024.0, 0.0], 0.0)),
            );
            k += 1;
        }
    }
    let link_keyword = own(LINK_KEYWORD);
    let persistent = vec![
        reference(CELL_CENTER, heading, [0.0, 0.0, 0.0], 0.0),
        reference(
            CELL_ENTRY,
            heading,
            [-900.0, 0.0, 0.0],
            std::f32::consts::FRAC_PI_2,
        ),
        reference(ALPHA, own(ALPHA_NPC), [-600.0, -600.0, 0.0], 0.0),
        reference(BETA, own(BETA_NPC), [-300.0, -600.0, 0.0], 0.0),
        reference(GAMMA, own(GAMMA_NPC), [0.0, -600.0, 0.0], 0.0),
        reference(VICTIM, own(VICTIM_NPC), [300.0, -600.0, 0.0], 0.0),
        reference(DOOR_REF, own(DOOR), [800.0, 800.0, 0.0], 0.0),
        reference(WEB_REF, own(WEB), [-800.0, 800.0, 100.0], 0.0),
        reference(WEB_REF2, own(WEB), [-500.0, 800.0, 100.0], 0.0),
        reference(RESET_CHEST_REF, own(RESET_CHEST), [600.0, -900.0, 0.0], 0.0).field(b"XLOC", &{
            // XLOC: level 25, no key, flags 0, unused.
            let mut x = vec![25u8, 0, 0, 0];
            x.extend_from_slice(&[0; 4]);
            x.extend_from_slice(&[0; 4]);
            x.extend_from_slice(&[0; 8]);
            x
        }),
        reference(DELETE_ME, v.id("BarrelEmpty01")?, [900.0, -900.0, 0.0], 0.0),
        linked(
            reference(LINK_A, x_marker, [-900.0, -900.0, 0.0], 0.0),
            FormId(0),
            own(LINK_B),
        ),
        linked(
            reference(LINK_B, x_marker, [-800.0, -900.0, 0.0], 0.0),
            FormId(0),
            own(LINK_C),
        ),
        reference(LINK_C, x_marker, [-700.0, -900.0, 0.0], 0.0),
        reference(MATCH_FAR, own(MATCH_NPC), [900.0, 900.0, 0.0], 0.0),
        reference(
            REMEMBER_BOWL,
            v.id("BasicWoodenBowl01")?,
            [400.0, 400.0, 0.0],
            0.0,
        ),
        reference(
            FLOATING_BOWL,
            v.id("BasicWoodenBowl01")?,
            [-200.0, -300.0, 150.0],
            0.0,
        ),
        linked(
            reference(LINK_K, x_marker, [-600.0, -900.0, 0.0], 0.0),
            link_keyword,
            own(LINK_B),
        ),
    ];
    let cell_label = own(CELL).0.to_le_bytes();
    let children = Node::Group {
        label: cell_label,
        group_type: group_type::CELL_CHILDREN,
        children: vec![
            Node::Group {
                label: cell_label,
                group_type: group_type::CELL_PERSISTENT,
                children: persistent.into_iter().map(Node::Record).collect(),
            },
            Node::Group {
                label: cell_label,
                group_type: group_type::CELL_TEMPORARY,
                children: temporary.into_iter().map(Node::Record).collect(),
            },
        ],
    };
    let mut cells: BTreeMap<i32, BTreeMap<i32, Vec<Node>>> = BTreeMap::new();
    cells
        .entry((CELL % 10) as i32)
        .or_default()
        .entry((CELL / 10 % 10) as i32)
        .or_default()
        .extend([Node::Record(cell), children]);

    // The warehouse: markers on its floor's middle.
    let warehouse = v.id("WarehouseFurniture")?;
    let floors = places::floors(lo, warehouse)?;
    let mid = {
        let n = floors.len() as f32;
        let c = floors.iter().fold([0.0f32; 3], |a, p| {
            [a[0] + p[0] / n, a[1] + p[1] / n, a[2] + p[2] / n]
        });
        *floors
            .iter()
            .min_by(|a, b| {
                let d = |p: &[f32; 3]| (p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2);
                d(a).total_cmp(&d(b))
            })
            .context("the warehouse floor")?
    };
    let entry = *floors
        .iter()
        .min_by(|a, b| {
            let d = |p: &[f32; 3]| (p[0] - mid[0] + 500.0).powi(2) + (p[1] - mid[1]).powi(2);
            d(a).total_cmp(&d(b))
        })
        .context("the warehouse entry")?;
    let warehouse_cell = places::interiors(
        lo,
        vec![Interior {
            cell: warehouse,
            name: "Warehouse Furniture",
            persistent: vec![
                reference(WH_CENTER, heading, mid, 0.0),
                reference(WH_ENTRY, heading, entry, std::f32::consts::FRAC_PI_2),
                reference(MATCH_WH, own(MATCH_NPC), mid, 0.0),
            ],
            temporary: Vec::new(),
        }],
    )?;
    // Ours and the warehouse in one CELL group.
    let Node::Group {
        children: wh_blocks,
        ..
    } = warehouse_cell
    else {
        unreachable!()
    };
    let mut blocks: Vec<Node> = Vec::new();
    for (block, subs) in cells {
        blocks.push(Node::Group {
            label: block.to_le_bytes(),
            group_type: group_type::INTERIOR_BLOCK,
            children: subs
                .into_iter()
                .map(|(sub, children)| Node::Group {
                    label: sub.to_le_bytes(),
                    group_type: group_type::INTERIOR_SUB_BLOCK,
                    children,
                })
                .collect(),
        });
    }
    blocks.extend(wh_blocks);
    let cells = Node::Group {
        label: *b"CELL",
        group_type: group_type::TOP,
        children: blocks,
    };

    let plugin = write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "plans/autotest-audit.md's automatic items: tests run by script, reports saved.",
        PROBES + 2 * PROBE_LIST.len() as u32,
        &[
            Node::top(b"KYWD", keywords),
            Node::top(b"FACT", factions),
            Node::top(b"MGEF", effects),
            Node::top(b"SPEL", spells),
            Node::top(b"ACTI", activators),
            Node::top(b"ARMO", armor),
            Node::top(b"CONT", containers),
            Node::top(b"DOOR", doors),
            Node::top(b"NPC_", npcs),
            Node::top(b"FLST", lists),
            Node::top(b"OTFT", outfits),
            Node::top(b"GLOB", globals),
            Node::top(b"QUST", quests),
            cells,
            Node::top(b"SMQN", story_nodes),
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
    fn the_scripts_compile() {
        let built = scripts().unwrap();
        assert_eq!(built.len(), 1 + LOGGERS.len());
        assert!(tests().len() > 20);
    }

    #[test]
    fn the_tests_run_and_report() {
        let mut properties = vec![("Player".to_owned(), form(0x14, "Actor"))];
        for (i, (name, ty, _)) in VANILLA.iter().enumerate() {
            properties.push((name.to_string(), form(0x1000 + i as u32, ty)));
        }
        for (name, ty, object_id) in OWN {
            properties.push((name.to_string(), form(own(*object_id).0, ty)));
        }
        for name in ["DeltaA", "DeltaB", "DeltaC"] {
            properties.push((name.to_owned(), papyrus::Value::Float(125.0)));
        }
        for (i, name) in ["WeatherA", "WeatherB", "WeatherC"].iter().enumerate() {
            properties.push((name.to_string(), form(0x81A + i as u32, "Weather")));
        }
        for (i, p) in PROBE_LIST.iter().enumerate() {
            let i = i as u32;
            properties.push((
                format!("PS_{}", p.name),
                form(own(PROBES + 2 * i + 1).0, "Spell"),
            ));
            properties.push((
                format!("PE_{}", p.name),
                form(own(PROBES + 2 * i).0, "MagicEffect"),
            ));
        }
        let shown = run(scripts().unwrap(), &[(own(QUEST).0, SCRIPT, properties)], 1);
        assert_eq!(shown.boxes.len(), 1, "the closing box");
        assert!(shown.saved_at_box == Some(0), "saved before it");
    }
}
