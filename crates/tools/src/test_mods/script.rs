//! What the test mods' scripts are built with: the vanilla natives they
//! call ([`natives`], checked when built and declared to the VM for dry
//! runs), [`Code`] to write functions calling them, and the functions
//! several test scripts share (numbers as text, timing the weather, running
//! the tests and saving).

use std::ops::{Deref, DerefMut};
use std::sync::OnceLock;

use papyrus::build::{Arg, FunctionBuilder, NativeDecl, auto_var, discard, id, string};
use papyrus::pex::op;

/// The vanilla natives the scripts call, as the game's scripts declare them
/// (`vrm-tool pex-dump`): a class (`Class : Parent`), then its functions,
/// `global` for those called on the class. Calls are checked against these
/// when built, and the dry runs in the tests declare them to the VM.
const NATIVE_TABLE: &str = "\
Form
    None RegisterForSingleUpdate(Float afInterval)
    Bool RegisterForAnimationEvent(ObjectReference akSender, String asEventName)
    None UnregisterForAnimationEvent(ObjectReference akSender, String asEventName)
Quest : Form
    Bool Start()
    None Stop()
    Bool IsRunning()
    None CompleteQuest()
    None SetObjectiveDisplayed(Int aiObjective, Bool abDisplayed, Bool abForce)
    None SetObjectiveCompleted(Int aiObjective, Bool abCompleted)
ActorBase : Form
EncounterZone : Form
Location : Form
    None SetCleared(Bool abCleared)
    Bool IsCleared()
GlobalVariable : Form
    Float GetValue()
    None SetValue(Float afNewValue)
Faction : Form
    Int GetCrimeGold()
    Int GetCrimeGoldViolent()
    None SetCrimeGold(Int aiGold)
    None SetCrimeGoldViolent(Int aiGold)
    None PlayerPayCrimeGold(Bool abRemoveStolenItems, Bool abGoToJail)
Spell : Form
    None Cast(ObjectReference akSource, ObjectReference akTarget)
Weapon : Form
Armor : Form
Ammo : Form
Light : Form
Potion : Form
MiscObject : Form
Projectile : Form
ObjectReference : Form
    actor PlaceActorAtMe(ActorBase akActorToPlace, Int aiLevelMod, EncounterZone akZone)
    ObjectReference PlaceAtMe(Form akFormToPlace, Int aiCount, Bool abForcePersist, Bool abInitiallyDisabled)
    None Disable(Bool abFadeOut)
    None Delete()
    Bool IsLocked()
    Int GetLockLevel()
    None Lock(Bool abLock, Bool abAsOwner)
    Bool IsInInterior()
    None MoveTo(ObjectReference akTarget, Float afXOffset, Float afYOffset, Float afZOffset, Bool abMatchRotation)
    None AddItem(Form akItemToAdd, Int aiCount, Bool abSilent)
    None RemoveItem(Form akItemToRemove, Int aiCount, Bool abSilent, ObjectReference akOtherContainer)
    Int GetItemCount(Form akItem)
    Bool IsMapMarkerVisible()
    Bool CanFastTravelToMarker()
    None AddToMap(Bool abAllowFastTravel)
    Bool GetAnimationVariableBool(String arVariableName)
    Int GetAnimationVariableInt(String arVariableName)
    Float GetAnimationVariableFloat(String arVariableName)
Actor : ObjectReference
    Int GetLevel()
    Float GetBaseActorValue(String asValueName)
    Float GetActorValue(String asValueName)
    Float GetActorValuePercentage(String asValueName)
    None SetActorValue(String asValueName, Float afValue)
    None ModActorValue(String asValueName, Float afAmount)
    None ForceActorValue(String asValueName, Float afNewValue)
    None DamageActorValue(String asValueName, Float afDamage)
    None RestoreActorValue(String asValueName, Float afAmount)
    Bool IsWeaponDrawn()
    None DrawWeapon()
    Float GetLightLevel()
    Bool IsEquipped(Form akItem)
    None EquipItem(Form akItem, Bool abPreventRemoval, Bool abSilent)
    None UnequipItem(Form akItem, Bool abPreventEquip, Bool abSilent)
    None EquipSpell(Spell akSpell, Int aiSource)
    None UnequipSpell(Spell akSpell, Int aiSource)
    Weapon GetEquippedWeapon(Bool abLeftHand)
    Armor GetEquippedShield()
    Spell GetEquippedSpell(Int aiSource)
    Int GetEquippedItemType(Int aiHand)
    Bool AddSpell(Spell akSpell, Bool abVerbose)
    Bool RemoveSpell(Spell akSpell)
    Bool HasSpell(Form akForm)
    None SetPlayerTeammate(Bool abTeammate, Bool abCanDoFavor)
    Bool IsInFaction(Faction akFaction)
    Int GetFactionRank(Faction akFaction)
    None SetRelationshipRank(Actor akOther, Int aiRank)
    Int GetRelationshipRank(Actor akOther)
    None SetCrimeFaction(Faction akFaction)
    None SendAssaultAlarm()
    None Kill(Actor akKiller)
    None StartCombat(Actor akTarget)
    None StopCombat()
    Int GetCombatState()
Weather : Form
    global Weather GetCurrentWeather()
    global Float GetCurrentWeatherTransition()
    global Int GetSkyMode()
    global None ReleaseOverride()
    None SetActive(Bool abOverride, Bool abAccelerate)
    None ForceActive(Bool abOverride)
    global Weather GetOutgoingWeather()
Game
    global Actor GetPlayer()
    global None DisablePlayerControls(Bool abMovement, Bool abFighting, Bool abCamSwitch, Bool abLooking, Bool abSneaking, Bool abMenu, Bool abActivate, Bool abJournalTabs, Int aiDisablePOVType)
    global None EnablePlayerControls(Bool abMovement, Bool abFighting, Bool abCamSwitch, Bool abLooking, Bool abSneaking, Bool abMenu, Bool abActivate, Bool abJournalTabs, Int aiDisablePOVType)
    global Bool IsMovementControlsEnabled()
    global Bool IsFightingControlsEnabled()
    global None RequestSave()
    global Float GetGameSettingFloat(String asGameSetting)
    global Int GetGameSettingInt(String asGameSetting)
    global Int QueryStat(String asStat)
    global None IncrementSkill(String asSkillName)
    global None IncrementSkillBy(String asSkillName, Int aiCount)
    global None ForceFirstPerson()
    global None ForceThirdPerson()
Debug
    global None MessageBox(String asMessageBoxText)
    global None Notification(String asNotificationText)
    global None Trace(String asTextToPrint, Int aiSeverity)
Utility
    global None Wait(Float afSeconds)
    global Float GetCurrentRealTime()
    global Float GetCurrentGameTime()
Math
    global Int Floor(Float afValue)
";

/// A class of [`NATIVE_TABLE`].
pub(super) struct NativeClass {
    pub name: &'static str,
    pub parent: &'static str,
    pub functions: Vec<NativeDecl<'static>>,
}

/// [`NATIVE_TABLE`], read.
pub(super) fn natives() -> &'static [NativeClass] {
    static CLASSES: OnceLock<Vec<NativeClass>> = OnceLock::new();
    CLASSES.get_or_init(|| {
        let mut classes: Vec<NativeClass> = Vec::new();
        for line in NATIVE_TABLE.lines().filter(|l| !l.trim().is_empty()) {
            if !line.starts_with(' ') {
                let (name, parent) = line.split_once(" : ").unwrap_or((line, ""));
                classes.push(NativeClass {
                    name,
                    parent,
                    functions: Vec::new(),
                });
                continue;
            }
            let line = line.trim();
            let (global, line) = match line.strip_prefix("global ") {
                Some(rest) => (true, rest),
                None => (false, line),
            };
            let (ret, rest) = line.split_once(' ').expect("return type");
            let (name, params) = rest
                .trim_end_matches(')')
                .split_once('(')
                .expect("parameters");
            let params: Vec<(&'static str, &'static str)> = params
                .split(", ")
                .filter(|p| !p.is_empty())
                .map(|p| {
                    let (ty, name) = p.split_once(' ').expect("parameter");
                    (name, ty)
                })
                .collect();
            let params: &'static [(&str, &str)] = Box::leak(params.into_boxed_slice());
            classes
                .last_mut()
                .expect("a class first")
                .functions
                .push((name, ret, params, global));
        }
        classes
    })
}

/// `class.name`'s declaration, looked up through `class`'s parents: the
/// declaring class and the function.
pub(super) fn native(class: &str, name: &str) -> (&'static str, &'static NativeDecl<'static>) {
    let mut c = class;
    loop {
        let decl = natives()
            .iter()
            .find(|n| n.name.eq_ignore_ascii_case(c))
            .unwrap_or_else(|| panic!("native class {c} not declared"));
        if let Some(f) = decl
            .functions
            .iter()
            .find(|f| f.0.eq_ignore_ascii_case(name))
        {
            return (decl.name, f);
        }
        assert!(!decl.parent.is_empty(), "{class}.{name} not declared");
        c = decl.parent;
    }
}

/// A function being built, with natives' calls checked against
/// [`natives`] and their results in new temporaries.
pub(super) struct Code(pub(super) FunctionBuilder);

impl Deref for Code {
    type Target = FunctionBuilder;
    fn deref(&self) -> &FunctionBuilder {
        &self.0
    }
}

impl DerefMut for Code {
    fn deref_mut(&mut self) -> &mut FunctionBuilder {
        &mut self.0
    }
}

impl Code {
    pub(super) fn new(return_type: &str) -> Self {
        Code(FunctionBuilder::new(return_type))
    }

    fn result(&mut self, type_name: &str) -> Arg {
        if type_name == "None" {
            discard()
        } else {
            self.temp(type_name)
        }
    }

    /// `obj.name(args)`, `obj` of native class `class`.
    pub(super) fn method<const N: usize>(
        &mut self,
        class: &str,
        obj: Arg,
        name: &str,
        args: [Arg; N],
    ) -> Arg {
        let (_, (_, ret, params, global)) = native(class, name);
        assert!(!global, "{class}.{name} is global");
        assert_eq!(N, params.len(), "{class}.{name}: argument count");
        let dest = self.result(ret);
        self.call_method(obj, name, dest.clone(), args);
        dest
    }

    /// `class.name(args)`, a global native.
    pub(super) fn global<const N: usize>(
        &mut self,
        class: &str,
        name: &str,
        args: [Arg; N],
    ) -> Arg {
        let (decl_class, (_, ret, params, global)) = native(class, name);
        assert!(global, "{class}.{name} isn't global");
        assert_eq!(N, params.len(), "{class}.{name}: argument count");
        let dest = self.result(ret);
        self.call_static(decl_class, name, dest.clone(), args);
        dest
    }

    /// `self.name(args)`, one of the script's own functions.
    pub(super) fn own<const N: usize>(&mut self, name: &str, ret: &str, args: [Arg; N]) -> Arg {
        let dest = self.result(ret);
        self.call_method(id("self"), name, dest.clone(), args);
        dest
    }

    /// `a <op> b` into a new temporary of `type_name`.
    pub(super) fn calc(&mut self, op: u8, type_name: &str, a: Arg, b: Arg) -> Arg {
        let dest = self.temp(type_name);
        self.op(op, [dest.clone(), a, b]);
        dest
    }

    /// `"part" + Num(value)` for each pair, appended to `dest`.
    pub(super) fn numbers<const N: usize>(&mut self, dest: &Arg, parts: [(&str, Arg); N]) {
        for (text, value) in parts {
            let n = self.own("Num", "String", [value]);
            self.append(dest.clone(), [string(text), n]);
        }
    }

    pub(super) fn controls(&mut self, enable: bool, movement: bool, fighting: bool) {
        let name = if enable {
            "EnablePlayerControls"
        } else {
            "DisablePlayerControls"
        };
        let mut args: [Arg; 9] = std::array::from_fn(|_| false.into());
        args[0] = movement.into();
        args[1] = fighting.into();
        args[8] = 0.into();
        self.global("Game", name, args);
    }

    pub(super) fn enable_all(&mut self) {
        let mut args: [Arg; 9] = std::array::from_fn(|_| true.into());
        args[8] = 0.into();
        self.global("Game", "EnablePlayerControls", args);
    }

    pub(super) fn wait(&mut self, seconds: f32) {
        self.global("Utility", "Wait", [seconds.into()]);
    }

    pub(super) fn finish(mut self, value: Arg) -> FunctionBuilder {
        self.ret(value);
        self.0
    }
}

/// `Num(f)`: a float as text to three decimals, without them when whole
/// (casting gives six).
pub(super) fn num() -> FunctionBuilder {
    let mut c = Code::new("String");
    let f = c.param("f", "Float");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let negative = c.calc(op::CMP_LT, "Bool", f.clone(), 0.0f32.into());
    let positive = c.label();
    c.jmpf(negative, positive)
        .assign(r.clone(), string("-"))
        .op(op::FNEG, [f.clone(), f.clone()])
        .bind(positive);
    let scaled = c.calc(op::FMUL, "Float", f, 1000.0f32.into());
    let scaled = c.calc(op::FADD, "Float", scaled, 0.5f32.into());
    let thousandths = c.global("Math", "Floor", [scaled]);
    let whole = c.calc(op::IDIV, "Int", thousandths.clone(), 1000.into());
    let fraction = c.calc(op::IMOD, "Int", thousandths, 1000.into());
    c.append(r.clone(), [whole]);
    let end = c.label();
    let is_whole = c.calc(op::CMP_EQ, "Bool", fraction.clone(), 0.into());
    c.jmpt(is_whole, end).append(r.clone(), [string(".")]);
    for below in [100, 10] {
        let pad = c.label();
        let short = c.calc(op::CMP_LT, "Bool", fraction.clone(), below.into());
        c.jmpf(short, pad)
            .append(r.clone(), [string("0")])
            .bind(pad);
    }
    c.append(r.clone(), [fraction]).bind(end);
    c.finish(r)
}

pub(super) fn real_time(c: &mut Code) -> Arg {
    c.global("Utility", "GetCurrentRealTime", [])
}

pub(super) fn transition(c: &mut Code) -> Arg {
    c.global("Weather", "GetCurrentWeatherTransition", [])
}

/// `Settle()`: waits up to 30 s for the weather transition to finish.
pub(super) fn settle() -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    let start = real_time(&mut c);
    let (top, out) = (c.label(), c.label());
    c.bind(top);
    let pct = transition(&mut c);
    let done = c.calc(op::CMP_GE, "Bool", pct, 1.0f32.into());
    c.jmpt(done, out);
    let now = real_time(&mut c);
    let waited = c.calc(op::FSUB, "Float", now, start.clone());
    let over = c.calc(op::CMP_GT, "Bool", waited, 30.0f32.into());
    c.jmpt(over, out);
    c.wait(0.5);
    c.jmp(top);
    c.bind(out);
    let pct = transition(&mut c);
    let now = real_time(&mut c);
    let waited = c.calc(op::FSUB, "Float", now, start);
    c.assign(r.clone(), string(""));
    c.numbers(&r, [("(settled at ", pct), (" after ", waited)]);
    c.append(r.clone(), [string(" s)\n")]);
    c.finish(r)
}

/// `Measure(Weather w, Bool override, Bool accelerate, String label)`: sets
/// `w` active and times its transition.
pub(super) fn measure() -> FunctionBuilder {
    let mut c = Code::new("String");
    let w = c.param("w", "Weather");
    let overriding = c.param("override", "Bool");
    let accelerate = c.param("accelerate", "Bool");
    let label = c.param("label", "String");
    let r = c.local("r", "String");
    let vars = ["at1", "half", "halfGame", "full", "fullGame"].map(|n| {
        let v = c.local(n, "Float");
        c.assign(v.clone(), (-1.0f32).into());
        v
    });
    let [at1, half, half_game, full, full_game] = vars;
    let real0 = real_time(&mut c);
    let game0 = c.global("Utility", "GetCurrentGameTime", []);
    c.method("Weather", w.clone(), "SetActive", [overriding, accelerate]);
    let pct0 = transition(&mut c);
    let current0 = c.global("Weather", "GetCurrentWeather", []);
    let is_current0 = c.calc(op::CMP_EQ, "Bool", current0, w.clone());

    let (top, done) = (c.label(), c.label());
    c.bind(top);
    c.wait(0.1);
    let now = real_time(&mut c);
    let dt = c.calc(op::FSUB, "Float", now, real0.clone());
    let over = c.calc(op::CMP_GT, "Bool", dt.clone(), 120.0f32.into());
    c.jmpt(over, done);
    let current = c.global("Weather", "GetCurrentWeather", []);
    let is_current = c.calc(op::CMP_EQ, "Bool", current, w);
    c.jmpf(is_current, top);
    let pct = transition(&mut c);
    // The transition at the first sample a second in.
    let skip = c.label();
    let unset = c.calc(op::CMP_LT, "Bool", at1.clone(), 0.0f32.into());
    c.jmpf(unset, skip);
    let second = c.calc(op::CMP_GE, "Bool", dt.clone(), 1.0f32.into());
    c.jmpf(second, skip);
    c.assign(at1.clone(), pct.clone());
    c.bind(skip);
    // Times to half and full, in real seconds and game minutes.
    let game_minutes = |c: &mut Code| {
        let game = c.global("Utility", "GetCurrentGameTime", []);
        let days = c.calc(op::FSUB, "Float", game, game0.clone());
        c.calc(op::FMUL, "Float", days, 1440.0f32.into())
    };
    let skip = c.label();
    let unset = c.calc(op::CMP_LT, "Bool", half.clone(), 0.0f32.into());
    c.jmpf(unset, skip);
    let past = c.calc(op::CMP_GE, "Bool", pct.clone(), 0.5f32.into());
    c.jmpf(past, skip);
    c.assign(half.clone(), dt.clone());
    let m = game_minutes(&mut c);
    c.assign(half_game.clone(), m);
    c.bind(skip);
    let finished = c.calc(op::CMP_GE, "Bool", pct, 1.0f32.into());
    c.jmpf(finished, top);
    c.assign(full.clone(), dt);
    let m = game_minutes(&mut c);
    c.assign(full_game.clone(), m);
    c.bind(done);

    c.assign(r.clone(), label)
        .append(r.clone(), [string(": current at once "), is_current0]);
    c.numbers(
        &r,
        [
            (", transition at once ", pct0),
            (", after 1 s ", at1),
            ("; half at ", half),
            (" s / ", half_game),
            (" game min; full at ", full),
            (" s / ", full_game),
        ],
    );
    c.append(r.clone(), [string(" game min\n")]);
    c.finish(r)
}

/// `OnUpdate()`: runs each of `tests` (name, title) once, keeping its
/// report in `Result<name>`, asks the game to save, then shows the reports.
/// `mod_name` names the test mod in what's shown.
pub(super) fn on_update(mod_name: &str, minutes: &str, tests: &[(&str, &str)]) -> FunctionBuilder {
    let mut c = Code::new("None");
    let ran = id("Ran");
    let go = c.label();
    c.jmpf(ran.clone(), go).ret(Arg::None).bind(go);
    c.assign(ran, true.into());
    c.global(
        "Debug",
        "Notification",
        [string(&format!(
            "vibe_rm {mod_name} running: about {minutes}. Leave the game be."
        ))],
    );
    for (name, _) in tests {
        let report = c.own(&format!("Test{name}"), "String", []);
        let var = id(&format!("Result{name}"));
        c.assign(var.clone(), report);
        let line = c.temp("String");
        c.assign(
            line.clone(),
            string(&format!("vibe_rm {mod_name} {name}:\n")),
        )
        .append(line.clone(), [var]);
        c.global("Debug", "Trace", [line, 0.into()]);
    }
    // Whatever a test left disabled.
    c.enable_all();
    // The save to read the reports from, made before the boxes (a menu)
    // come up.
    c.global("Game", "RequestSave", []);
    c.wait(3.0);
    for (name, title) in tests {
        let text = c.temp("String");
        c.assign(
            text.clone(),
            string(&format!("vibe_rm {mod_name}: {title}\n")),
        )
        .append(text.clone(), [id(&format!("Result{name}"))]);
        c.global("Debug", "MessageBox", [text]);
    }
    c.global(
        "Debug",
        "Notification",
        [string(&format!("vibe_rm {mod_name} done, reports saved"))],
    );
    c.finish(Arg::None)
}

pub(super) fn bytes<const N: usize>(parts: &[&[u8]]) -> [u8; N] {
    let v: Vec<u8> = parts.concat();
    v.try_into().expect("field size")
}

/// `SpawnLevel(ActorBase base)`: the level of one placed at the player,
/// deleted again.
pub(super) fn spawn_level() -> FunctionBuilder {
    let mut c = Code::new("Int");
    let base = c.param("base", "ActorBase");
    let actor = c.method(
        "Actor",
        auto_var("Player"),
        "PlaceActorAtMe",
        [base, 4.into(), Arg::None],
    );
    let level = c.method("Actor", actor.clone(), "GetLevel", []);
    c.method("Actor", actor.clone(), "Disable", [false.into()]);
    c.method("Actor", actor, "Delete", []);
    c.finish(level)
}
