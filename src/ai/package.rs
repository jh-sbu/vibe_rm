//! AI packages (`PACK`): schedule, conditions, procedure template and target location.

use esp::{FormId, LoadOrder};

use crate::condition::{self, Condition};

/// What an actor does while a package runs. Derived from the package's template,
/// which in the Creation Kit names a procedure tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Behaviour {
    /// Wander between random spots near the location, idling at each.
    Sandbox,
    /// Walk to the location and stay there.
    Travel,
    /// Stay put.
    Hold,
    /// Go to the location and sleep in a bed there.
    Sleep,
    /// Sit in the target chair (or one near the location).
    Sit,
    /// Walk a chain of linked patrol markers from the target reference.
    Patrol,
    /// Stay near the target reference.
    Follow,
    /// Lead the target to the location, waiting while it falls behind.
    Escort,
    /// Wait about (or in a seat) until the player comes near, then walk up to them
    /// and start a conversation (`Package::greet`).
    ForceGreet,
    /// Go to the location, draw and attack (or shoot at) the package's targets in
    /// barrages: training at dummies and archery butts (`Package::use_weapon`).
    UseWeapon,
}

/// Which weapon a UseWeapon package wants (its "Weapon Type" input).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponKind {
    Any,
    /// Object type 19.
    Melee,
    /// Object type 20 (bows, crossbows).
    Ranged,
    Specific(FormId),
}

/// The UseWeapon templates' inputs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UseWeapon {
    pub weapon: WeaponKind,
    /// "Target to Attack" / "Target 01".."03" (one picked per barrage).
    pub targets: [Option<Target>; 3],
    /// Seconds between barrages.
    pub pause: (f32, f32),
    /// Attacks per barrage.
    pub attacks: (u32, u32),
}

/// What a sandboxing actor may do besides wander (the template's "Allow ..." inputs).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Allow {
    pub sitting: bool,
    pub sleeping: bool,
    pub eating: bool,
    pub idle_markers: bool,
    pub special_furniture: bool,
    pub wandering: bool,
    /// Eating is the point (the Eat template): every seated idle is a meal, not
    /// just the occasional one.
    pub meal: bool,
}

impl Allow {
    const NONE: Allow = Allow {
        sitting: false,
        sleeping: false,
        eating: false,
        idle_markers: false,
        special_furniture: false,
        wandering: true,
        meal: false,
    };
}

/// A package's preferred speed (`PKDT`, when its "Preferred Speed" flag is set).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Gait {
    #[default]
    Walk,
    Jog,
    Run,
    FastWalk,
}

/// What a force greet opens the conversation with (the "Topic" input, `PDTO`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GreetTopic {
    Topic(FormId),
    /// The first topic of this subtype with a line for the speaker (`HELO`).
    Subtype([u8; 4]),
}

/// The ForceGreet / ForceGreetFromSitting templates' inputs.
#[derive(Debug, Clone, Copy)]
pub struct ForceGreet {
    pub topic: GreetTopic,
    /// The player being here sets the greeter going ("Trigger Location").
    pub trigger: Option<Location>,
    /// How close to the player the greeter comes before speaking ("Forcegreet
    /// Distance": the location is the player; only its radius is the package's).
    pub distance: f32,
    /// Only once it sees the player ("Player must be detected?").
    pub must_detect: bool,
    /// Sandbox about the wait location rather than stand there ("Sandbox While
    /// Waiting?").
    pub sandbox: bool,
    /// ForceGreetFromSitting: waits in its seat (the package's target) and speaks
    /// from there.
    pub seated: bool,
}

/// `PKDT` general flags.
const PKDT_PREFERRED_SPEED: u32 = 1 << 13;
const PKDT_ALWAYS_SNEAK: u32 = 1 << 17;
const PKDT_UNLOCK_AT_START: u32 = 0x40;
const PKDT_UNLOCK_ON_CHANGE: u32 = 0x80;

/// `Target::ObjectType` of furniture (`FURN`).
pub const OBJECT_TYPE_FURNITURE: u32 = 10;

/// A package "TargetSelector" / "SingleRef" input (`PTDA`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Ref(FormId),
    LinkedRef(Option<FormId>),
    /// A reference alias of the package's quest, by alias id.
    Alias(u32),
    /// The actor running the package.
    SelfRef,
    /// A specific object (base form), and an object type (`19` melee weapons,
    /// `20` ranged...).
    Object(FormId),
    ObjectType(u32),
    /// Aliases, etc. (not resolved yet).
    Other,
}

#[derive(Debug, Clone, Copy)]
pub enum LocationKind {
    NearReference(FormId),
    InCell(FormId),
    NearCurrent,
    NearEditor,
    NearLinkedRef(FormId),
    NearSelf,
    /// Near what fills a reference alias of the package's quest (alias id).
    NearAlias(u32),
    /// In the location a location alias of the package's quest holds (alias id).
    InLocAlias(u32),
    /// Object ids / types, aliases, etc. Approximated by the editor location.
    Other(u32),
}

#[derive(Debug, Clone, Copy)]
pub struct Location {
    pub kind: LocationKind,
    pub radius: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Schedule {
    pub month: i8,
    pub day_of_week: i8,
    pub hour: i8,
    pub minute: i8,
    /// Minutes.
    pub duration: i32,
}

impl Schedule {
    pub fn matches(&self, hour: f32, day: u32) -> bool {
        if self.day_of_week >= 0 && (day % 7) as i8 != self.day_of_week {
            return false;
        }
        if self.hour < 0 {
            return true;
        }
        let start = self.hour as f32 + self.minute.max(0) as f32 / 60.0;
        let len = self.duration as f32 / 60.0;
        if len <= 0.0 {
            return true;
        }
        let since = (hour - start).rem_euclid(24.0);
        since < len
    }
}

#[derive(Debug, Clone)]
pub struct Package {
    pub id: FormId,
    pub editor_id: String,
    /// The quest whose aliases its conditions, locations and targets name (`QNAM`;
    /// for an alias's package without one, the alias's quest).
    pub quest: Option<FormId>,
    pub template: String,
    pub behaviour: Behaviour,
    pub schedule: Schedule,
    pub conditions: Vec<Condition>,
    pub location: Option<Location>,
    pub target: Option<Target>,
    pub allow: Allow,
    /// 0..100: how restless a sandboxing actor is.
    pub energy: f32,
    /// Patrol: how close to each point counts as reached; loop at the end; start
    /// at the nearest point rather than the first.
    pub point_radius: f32,
    pub repeat: bool,
    pub start_nearest: bool,
    /// Follow: keep between these distances from the target.
    pub follow_radius: (f32, f32),
    /// Escort: wait while the escorted actor is farther than this.
    pub escort_wait: f32,
    /// Escort: run while the escorted actor is this far ahead.
    pub escort_run: f32,
    pub gait: Gait,
    pub sneak: bool,
    /// The sleeper locks its home's doors ("Lock Doors?" input).
    pub lock_doors: bool,
    /// `PKDT`: unlock the home's doors as the package starts / once it gives way
    /// to another.
    pub unlock_at_start: bool,
    pub unlock_on_change: bool,
    /// Force greet packages' inputs.
    pub greet: Option<ForceGreet>,
    pub use_weapon: Option<UseWeapon>,
}

fn behaviour_of(template: &str) -> Behaviour {
    let t = template.to_ascii_lowercase();
    if t.starts_with("sleep") {
        Behaviour::Sleep
    } else if t == "sit" || t == "sittarget" {
        Behaviour::Sit
    } else if t.contains("patrol") {
        Behaviour::Patrol
    } else if t.starts_with("useweapon") {
        Behaviour::UseWeapon
    } else if t.starts_with("forcegreet") {
        Behaviour::ForceGreet
    } else if t.starts_with("escort") {
        Behaviour::Escort
    } else if t.starts_with("follow") {
        Behaviour::Follow
    } else if t.starts_with("travel") || t.starts_with("hold") || t == "patrol" {
        Behaviour::Travel
    } else if t.starts_with("sandbox")
        || t.starts_with("eat")
        || t.starts_with("sit")
        || t.starts_with("useidlemarker")
        || t.starts_with("wander")
        || t.starts_with("guard")
        || t.starts_with("find")
    {
        Behaviour::Sandbox
    } else {
        Behaviour::Hold
    }
}

/// A package data input value, in record order.
#[derive(Debug, Clone, Copy)]
enum Input {
    Bool(bool),
    Float(f32),
    Int(i32),
    Location(Location),
    Target(Target),
    Topic(GreetTopic),
    Other,
}

fn location(rec: &esp::LoadedRecord<'_>, d: &[u8]) -> Option<Location> {
    if d.len() < 12 {
        return None;
    }
    let v = u32::from_le_bytes(d[4..8].try_into().unwrap());
    let f = rec.fid(FormId(v));
    let kind = match u32::from_le_bytes(d[0..4].try_into().unwrap()) {
        0 => LocationKind::NearReference(f),
        1 => LocationKind::InCell(f),
        2 => LocationKind::NearCurrent,
        3 => LocationKind::NearEditor,
        6 => LocationKind::NearLinkedRef(if v == 0 { FormId::NULL } else { f }),
        8 => LocationKind::NearAlias(v),
        9 => LocationKind::InLocAlias(v),
        12 => LocationKind::NearSelf,
        k => LocationKind::Other(k),
    };
    Some(Location {
        kind,
        radius: i32::from_le_bytes(d[8..12].try_into().unwrap()).max(0) as f32,
    })
}

fn target(rec: &esp::LoadedRecord<'_>, d: &[u8]) -> Option<Target> {
    if d.len() < 8 {
        return None;
    }
    let v = u32::from_le_bytes(d[4..8].try_into().unwrap());
    Some(match u32::from_le_bytes(d[0..4].try_into().unwrap()) {
        0 => Target::Ref(rec.fid(FormId(v))),
        1 => Target::Object(rec.fid(FormId(v))),
        2 => Target::ObjectType(v),
        3 => Target::LinkedRef((v != 0).then(|| rec.fid(FormId(v)))),
        4 => Target::Alias(v),
        6 => Target::SelfRef,
        _ => Target::Other,
    })
}

/// Data inputs of a package with their indices (`UNAM`), in record order.
fn inputs(rec: &esp::LoadedRecord<'_>) -> Vec<(u8, Input)> {
    let mut values = Vec::new();
    let mut indices = Vec::new();
    let mut kind = String::new();
    for sr in rec.subrecords() {
        match &sr.tag.0 {
            // The procedure tree follows; its UNAMs are names, not input indices.
            b"XNAM" => break,
            b"ANAM" => {
                kind = sr.zstring();
                values.push(Input::Other);
            }
            b"CNAM" if kind == "Bool" && !sr.data.is_empty() => {
                *values.last_mut().unwrap() = Input::Bool(sr.data[0] != 0)
            }
            b"CNAM" if kind == "Float" && sr.data.len() >= 4 => {
                *values.last_mut().unwrap() = Input::Float(sr.f32(0))
            }
            b"CNAM" if kind == "Int" && sr.data.len() >= 4 => {
                *values.last_mut().unwrap() = Input::Int(sr.i32(0))
            }
            b"PLDT" if !values.is_empty() => {
                if let Some(l) = location(rec, sr.data) {
                    *values.last_mut().unwrap() = Input::Location(l);
                }
            }
            b"PTDA" if !values.is_empty() => {
                if let Some(t) = target(rec, sr.data) {
                    *values.last_mut().unwrap() = Input::Target(t);
                }
            }
            b"PDTO" if !values.is_empty() && sr.data.len() >= 8 => {
                let d: [u8; 4] = sr.data[4..8].try_into().unwrap();
                *values.last_mut().unwrap() = Input::Topic(match sr.u32(0) {
                    0 => GreetTopic::Topic(rec.fid(FormId(u32::from_le_bytes(d)))),
                    _ => GreetTopic::Subtype(d),
                });
            }
            b"UNAM" if !sr.data.is_empty() => indices.push(sr.data[0]),
            _ => {}
        }
    }
    indices.into_iter().zip(values).collect()
}

/// Input names of a template package, by index (`UNAM` + `BNAM` after the procedure tree).
fn input_names(lo: &LoadOrder, template: FormId) -> std::collections::HashMap<u8, String> {
    let mut out = std::collections::HashMap::new();
    let Some(rec) = lo.get(template) else {
        return out;
    };
    let mut tree = false;
    let mut index = None;
    for sr in rec.subrecords() {
        match &sr.tag.0 {
            b"XNAM" => tree = true,
            b"UNAM" if tree && !sr.data.is_empty() => index = Some(sr.data[0]),
            b"BNAM" if tree => {
                if let Some(i) = index.take() {
                    // "Allow Sitting*", "AllowSitting", "Energy*" -> "allowsitting", "energy".
                    let name: String = sr
                        .zstring()
                        .chars()
                        .filter(char::is_ascii_alphanumeric)
                        .collect();
                    out.insert(i, name.to_ascii_lowercase());
                }
            }
            _ => {}
        }
    }
    out
}

pub fn parse(lo: &LoadOrder, id: FormId) -> Option<Package> {
    let rec = lo.get(id)?;
    if rec.tag().0 != *b"PACK" {
        return None;
    }
    let editor_id = rec.editor_id().unwrap_or_default();
    let mut schedule = Schedule {
        month: -1,
        day_of_week: -1,
        hour: -1,
        minute: -1,
        duration: 0,
    };
    let mut template = FormId::NULL;
    let (mut gait, mut sneak) = (Gait::Walk, false);
    let mut pkdt_flags = 0;
    let mut quest = None;
    for sr in rec.subrecords() {
        match &sr.tag.0 {
            b"QNAM" if sr.data.len() >= 4 => {
                quest = Some(rec.fid(sr.form_id(0))).filter(|q| !q.is_null())
            }
            b"PSDT" if sr.data.len() >= 12 => {
                schedule = Schedule {
                    month: sr.data[0] as i8,
                    day_of_week: sr.data[1] as i8,
                    hour: sr.data[3] as i8,
                    minute: sr.data[4] as i8,
                    duration: sr.i32(8),
                };
            }
            b"PKCU" if sr.data.len() >= 8 => template = rec.fid(sr.form_id(4)),
            // General flags, type, interrupt override, preferred speed.
            b"PKDT" if sr.data.len() >= 7 => {
                let flags = u32::from_le_bytes(sr.data[0..4].try_into().unwrap());
                if flags & PKDT_PREFERRED_SPEED != 0 {
                    gait = match sr.data[6] {
                        1 => Gait::Jog,
                        2 => Gait::Run,
                        3 => Gait::FastWalk,
                        _ => Gait::Walk,
                    };
                }
                sneak = flags & PKDT_ALWAYS_SNEAK != 0;
                pkdt_flags = flags;
            }
            _ => {}
        }
    }
    let template_name = lo
        .get(template)
        .and_then(|t| t.editor_id())
        .unwrap_or_default();
    // A package without a template has its own procedure tree (`ambushSleepPackage`).
    let tree_src = if template.is_null() { id } else { template };
    let behaviour = match behaviour_of(&template_name) {
        // Templates not known by name: by their tree's main procedure.
        Behaviour::Hold => procedure_tree(lo, tree_src)
            .as_ref()
            .and_then(main_procedure)
            .map_or(Behaviour::Hold, |p| behaviour_of_procedure(&p.procedure)),
        b => b,
    };
    let names = input_names(lo, tree_src);
    let inputs = inputs(&rec);
    let named = |n: &str| {
        inputs
            .iter()
            .find(|(i, _)| names.get(i).is_some_and(|x| x == n))
            .map(|(_, v)| *v)
    };
    let flag = |n: &str| match named(n) {
        Some(Input::Bool(b)) => Some(b),
        _ => None,
    };
    let mut allow = Allow::NONE;
    let fields: [(&mut bool, &[&str]); 6] = [
        (&mut allow.sitting, &["allowsitting"]),
        (&mut allow.sleeping, &["allowsleeping"]),
        (&mut allow.eating, &["alloweating"]),
        (&mut allow.idle_markers, &["allowidlemarkers"]),
        (
            &mut allow.special_furniture,
            &["allowspecialfurniture", "allowfurniture"],
        ),
        (&mut allow.wandering, &["allowwandering"]),
    ];
    for (field, keys) in fields {
        if let Some(b) = keys.iter().find_map(|k| flag(k)) {
            *field = b;
        }
    }
    // Eating means sitting down at a table.
    if template_name.eq_ignore_ascii_case("eat") {
        allow.sitting = true;
        allow.eating = true;
        allow.meal = true;
    }
    let energy = match named("energy") {
        Some(Input::Float(e)) => e.clamp(0.0, 100.0),
        _ => 50.0,
    };
    let float = |keys: &[&str], default: f32| {
        keys.iter()
            .find_map(|k| match named(k) {
                Some(Input::Float(f)) => Some(f),
                _ => None,
            })
            .unwrap_or(default)
    };
    let point_radius = float(&["patrolradius", "pointradius"], 50.0);
    let repeat = ["repeatable"].iter().find_map(|k| flag(k)).unwrap_or(true);
    let start_nearest = ["startatnearest", "startatnearestpoint"]
        .iter()
        .find_map(|k| flag(k))
        .unwrap_or(false);
    let follow_radius = (float(&["minradius"], 128.0), float(&["maxradius"], 384.0));
    let escort_wait = float(&["distancetowaitforfollowers"], 512.0);
    let escort_run = float(&["runifbehinddistance"], 500.0);
    let located = |keys: &[&str]| {
        keys.iter().find_map(|k| match named(k) {
            Some(Input::Location(l)) => Some(l),
            _ => None,
        })
    };
    let greet = (behaviour == Behaviour::ForceGreet).then(|| ForceGreet {
        topic: match named("topic") {
            Some(Input::Topic(t)) => t,
            _ => GreetTopic::Subtype(*b"HELO"),
        },
        trigger: located(&["triggerlocationplayerherecausesforcegreet"]),
        distance: located(&["forcegreetdistancedontchangerefjustradius"])
            .map_or(300.0, |l| l.radius),
        must_detect: ["playermustbedetected", "obsplayermustbedetected"]
            .iter()
            .find_map(|k| flag(k))
            .unwrap_or(false),
        sandbox: flag("sandboxwhilewaiting").unwrap_or(false),
        seated: template_name.eq_ignore_ascii_case("forcegreetfromsitting"),
    });
    let int = |k: &str| match named(k) {
        Some(Input::Int(i)) => Some(i.max(0) as u32),
        _ => None,
    };
    let use_weapon = (behaviour == Behaviour::UseWeapon).then(|| {
        let target = |k: &str| match named(k) {
            Some(Input::Target(t)) => Some(t),
            _ => None,
        };
        let pause = (float(&["minpause"], 2.0), float(&["maxpause"], 6.0));
        let attacks = (
            int("minattacksperbarrage").unwrap_or(1).max(1),
            int("maxattacksperbarrage").unwrap_or(3).max(1),
        );
        UseWeapon {
            weapon: match target("weapontype") {
                Some(Target::ObjectType(19)) => WeaponKind::Melee,
                Some(Target::ObjectType(20)) => WeaponKind::Ranged,
                Some(Target::Object(f)) => WeaponKind::Specific(f),
                _ => WeaponKind::Any,
            },
            targets: [
                target("targettoattack").or_else(|| target("target01")),
                target("target02"),
                target("target03"),
            ],
            pause: (pause.0.min(pause.1), pause.0.max(pause.1)),
            attacks: (attacks.0.min(attacks.1), attacks.0.max(attacks.1)),
        }
    });
    // The first location input is the package's main location (a force greeter's
    // wait location, where to use a weapon); likewise for targets.
    let location =
        located(&["npcwaitlocationnpchangsouthere", "useweaponlocation"]).or_else(|| {
            inputs.iter().find_map(|(_, v)| match v {
                Input::Location(l) => Some(*l),
                _ => None,
            })
        });
    let target = inputs.iter().find_map(|(_, v)| match v {
        Input::Target(t) => Some(*t),
        _ => None,
    });
    Some(Package {
        id,
        editor_id,
        quest,
        behaviour,
        template: template_name,
        schedule,
        conditions: condition::parse_all(&rec),
        location,
        target,
        allow,
        energy,
        point_radius,
        repeat,
        start_nearest,
        follow_radius,
        escort_wait,
        escort_run,
        gait,
        sneak,
        lock_doors: flag("lockdoors").unwrap_or(false),
        unlock_at_start: pkdt_flags & PKDT_UNLOCK_AT_START != 0,
        unlock_on_change: pkdt_flags & PKDT_UNLOCK_ON_CHANGE != 0,
        greet,
        use_weapon,
    })
}

/// A node of a package template's procedure tree (after `XNAM`): a branch
/// (`Stacked`, `Sequence`, `Simultaneous`...) over its children, or a procedure
/// with the inputs it takes (`PKC2`, by input index), each with its conditions.
#[derive(Debug, Clone)]
struct Node {
    kind: String,
    conditions: Vec<Condition>,
    children: Vec<Node>,
    procedure: String,
    args: Vec<u8>,
}

/// The procedure tree of a template package.
fn procedure_tree(lo: &LoadOrder, template: FormId) -> Option<Node> {
    let rec = lo.get(template)?;
    // Flat, in pre-order, with each branch's child count.
    let mut flat: Vec<(Node, usize)> = Vec::new();
    let mut tree = false;
    for sr in rec.subrecords() {
        match &sr.tag.0 {
            b"XNAM" => tree = true,
            _ if !tree => {}
            // The input names follow the tree.
            b"UNAM" => break,
            b"ANAM" => flat.push((
                Node {
                    kind: sr.zstring(),
                    conditions: Vec::new(),
                    children: Vec::new(),
                    procedure: String::new(),
                    args: Vec::new(),
                },
                0,
            )),
            b"PRCB" if sr.data.len() >= 4 => {
                if let Some(n) = flat.last_mut() {
                    n.1 = sr.u32(0) as usize;
                }
            }
            b"CTDA" => {
                if let (Some(n), Some(c)) = (flat.last_mut(), condition::parse(&rec, sr.data)) {
                    n.0.conditions.push(c);
                }
            }
            b"PNAM" => {
                if let Some(n) = flat.last_mut() {
                    n.0.procedure = sr.zstring();
                }
            }
            b"PKC2" if !sr.data.is_empty() => {
                if let Some(n) = flat.last_mut() {
                    n.0.args.push(sr.data[0]);
                }
            }
            _ => {}
        }
    }
    fn build(flat: &mut std::vec::IntoIter<(Node, usize)>) -> Option<Node> {
        let (mut node, count) = flat.next()?;
        for _ in 0..count {
            node.children.push(build(flat)?);
        }
        Some(node)
    }
    build(&mut flat.into_iter())
}

/// What the AI does for a procedure.
fn behaviour_of_procedure(name: &str) -> Behaviour {
    match name {
        "Sandbox" => Behaviour::Sandbox,
        "Travel" | "HoldPosition" => Behaviour::Travel,
        "Patrol" => Behaviour::Patrol,
        "Follow" => Behaviour::Follow,
        "Escort" => Behaviour::Escort,
        "Sit" => Behaviour::Sit,
        "Sleep" => Behaviour::Sleep,
        "ForceGreet" => Behaviour::ForceGreet,
        "UseWeapon" => Behaviour::UseWeapon,
        _ => Behaviour::Hold,
    }
}

/// The procedure a branch is about, for the behaviours the AI has: the first
/// found in this order (a guard post's patrol over its guarding).
fn main_procedure(node: &Node) -> Option<&Node> {
    const ORDER: [&str; 8] = [
        "Patrol",
        "Follow",
        "Sit",
        "Sleep",
        "Sandbox",
        "Escort",
        "Travel",
        "HoldPosition",
    ];
    fn all<'a>(n: &'a Node, out: &mut Vec<&'a Node>) {
        if !n.procedure.is_empty() {
            out.push(n);
        }
        n.children.iter().for_each(|c| all(c, out));
    }
    let mut procs = Vec::new();
    all(node, &mut procs);
    ORDER
        .iter()
        .find_map(|name| procs.iter().find(|p| p.procedure == *name).copied())
        .or(procs.first().copied())
}

/// A package whose template picks one of several branches by their conditions
/// (a `Stacked` root, as the default master packages have: follow the linked
/// actor, patrol from the linked marker, else sandbox) becomes one package per
/// branch, in order, each with the branch's conditions after the package's own;
/// the first whose conditions pass runs, as the first passing branch would.
/// Others stay one package.
pub fn expand(lo: &LoadOrder, id: FormId) -> Vec<Package> {
    let Some(base) = parse(lo, id) else {
        return Vec::new();
    };
    let Some(rec) = lo.get(id) else {
        return Vec::new();
    };
    let template = rec
        .get(b"PKCU")
        .filter(|d| d.len() >= 8)
        .map(|d| rec.fid(FormId(u32::from_le_bytes(d[4..8].try_into().unwrap()))));
    let tree = procedure_tree(lo, template.filter(|t| !t.is_null()).unwrap_or(id));
    let Some(root) = tree.filter(|t| t.kind == "Stacked" && t.children.len() > 1) else {
        return vec![base];
    };
    let values: std::collections::HashMap<u8, Input> = inputs(&rec).into_iter().collect();
    let mut out = Vec::new();
    for (n, branch) in root.children.iter().enumerate() {
        let Some(proc) = main_procedure(branch) else {
            continue;
        };
        let arg = |i: usize| proc.args.get(i).and_then(|a| values.get(a)).copied();
        let flag = |i: usize, default: bool| match arg(i) {
            Some(Input::Bool(b)) => b,
            _ => default,
        };
        let float = |i: usize, default: f32| match arg(i) {
            Some(Input::Float(f)) => f,
            _ => default,
        };
        let mut p = base.clone();
        p.editor_id = format!("{}#{n}", base.editor_id);
        let location = proc.args.iter().find_map(|a| match values.get(a) {
            Some(Input::Location(l)) => Some(*l),
            _ => None,
        });
        let target = proc.args.iter().find_map(|a| match values.get(a) {
            Some(Input::Target(t)) => Some(*t),
            _ => None,
        });
        p.location = location.or(base.location);
        p.target = target;
        // Procedure inputs by position, as the single-procedure templates (Sandbox,
        // Patrol, Follow, Travel) name them.
        match proc.procedure.as_str() {
            "Sandbox" => {
                p.behaviour = Behaviour::Sandbox;
                p.allow = Allow {
                    eating: flag(1, false),
                    sleeping: flag(2, false),
                    idle_markers: flag(4, true),
                    sitting: flag(5, false),
                    wandering: flag(6, true),
                    special_furniture: flag(9, false),
                    meal: false,
                };
                p.energy = float(8, 50.0).clamp(0.0, 100.0);
            }
            "Patrol" => {
                p.behaviour = Behaviour::Patrol;
                p.point_radius = float(1, 50.0);
                p.repeat = flag(2, true);
                p.start_nearest = flag(3, false);
            }
            "Follow" => {
                p.behaviour = Behaviour::Follow;
                p.follow_radius = (float(1, 128.0), float(2, 384.0));
            }
            name => p.behaviour = behaviour_of_procedure(name),
        }
        // The branch's conditions, and the procedure's own when it is nested,
        // with package inputs they run on or name resolved for the package.
        if let Some(last) = p.conditions.last_mut() {
            last.or = false;
        }
        let nested = (!std::ptr::eq(proc, branch)).then_some(&proc.conditions);
        for c in branch.conditions.iter().chain(nested.into_iter().flatten()) {
            let mut c = c.clone();
            let resolve = |input: u8| match values.get(&input) {
                Some(Input::Target(Target::Ref(r))) => Some(Ok(*r)),
                Some(Input::Target(Target::SelfRef)) => Some(Err(condition::RefOf::Subject)),
                Some(Input::Target(Target::LinkedRef(kw))) => {
                    Some(Err(condition::RefOf::LinkedRef(kw.unwrap_or(FormId::NULL))))
                }
                _ => None,
            };
            if let Some(input) = c.pack_input {
                (c.run_on, c.reference) = match resolve(input) {
                    Some(Ok(r)) => (2, r),
                    Some(Err(condition::RefOf::Subject)) => (0, FormId::NULL),
                    Some(Err(condition::RefOf::LinkedRef(kw))) => (4, kw),
                    None => (condition::RUN_ON_NOTHING, FormId::NULL),
                };
            }
            if c.p1_input {
                match resolve(c.p1 as u8) {
                    Some(Ok(r)) => c.p1 = r.0,
                    Some(Err(of)) => c.p1_ref = Some(of),
                    None => c.run_on = condition::RUN_ON_NOTHING,
                }
            }
            p.conditions.push(c);
        }
        out.push(p);
    }
    out
}

/// Packages of an actor in priority order (its AI packages part's: see templates),
/// then its default package list's (`DPLT`, from the part that gives it).
pub fn npc_packages(lo: &LoadOrder, src: &crate::world::template::Sources) -> Vec<Package> {
    use crate::world::template::{AI_PACKAGES, DEF_PACK_LIST};
    let mut out: Vec<Package> = match lo.get(src.of(AI_PACKAGES)) {
        Some(rec) => rec
            .subrecords()
            .filter(|s| s.tag.0 == *b"PKID")
            .flat_map(|s| expand(lo, rec.fid(s.form_id(0))))
            .collect(),
        None => Vec::new(),
    };
    if let Some(list) = src.form(lo, DEF_PACK_LIST, b"DPLT").and_then(|l| lo.get(l)) {
        for s in list.subrecords().filter(|s| s.tag.0 == *b"LNAM") {
            out.extend(expand(lo, list.fid(s.form_id(0))));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_wraps_midnight() {
        let s = Schedule {
            month: -1,
            day_of_week: -1,
            hour: 22,
            minute: -1,
            duration: 8 * 60,
        };
        assert!(s.matches(23.0, 0));
        assert!(s.matches(3.0, 0));
        assert!(!s.matches(7.0, 0));
        assert!(!s.matches(12.0, 0));
    }
}
