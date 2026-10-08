//! Native function implementations.

use esp::FormId;
use papyrus::{NativeResult, ObjectId, Value};

use crate::engine::Engine;

fn v(x: Value) -> NativeResult {
    NativeResult::Value(x)
}
fn none() -> NativeResult {
    NativeResult::Value(Value::None)
}

fn form_arg(args: &[Value], i: usize) -> Option<FormId> {
    args.get(i).and_then(|a| a.as_form()).map(FormId)
}

pub fn call(
    e: &mut Engine,
    class: &str,
    func: &str,
    this: Option<&Value>,
    args: &[Value],
) -> NativeResult {
    let me = this.and_then(|t| t.as_form()).map(FormId);
    let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
    match (class, func) {
        // ------------------------------------------------------------ Debug
        ("debug", "notification") => {
            e.scripts.notify(arg(0).to_string());
            none()
        }
        ("debug", "messagebox") => {
            let s = arg(0).to_string();
            e.scripts.notify(s.clone());
            e.scripts.message_boxes.push(s);
            none()
        }
        ("debug", "trace") | ("debug", "traceuser") | ("debug", "tracestack") => {
            log::debug!("papyrus trace: {}", arg(0));
            v(Value::Bool(true))
        }
        ("debug", "traceandbox") => {
            e.scripts.notify(arg(0).to_string());
            none()
        }
        ("debug", "getplatformname") => v(Value::str("PC")),
        ("debug", "getversionnumber") => v(Value::str("1.6.1170.0")),
        // ---------------------------------------------------------- Utility
        ("utility", "wait") | ("utility", "waitmenumode") => NativeResult::Wait(arg(0).as_float()),
        ("utility", "waitgametime") => NativeResult::Wait(arg(0).as_float() * 3600.0 / 20.0),
        ("utility", "randomint") => {
            let (a, b) = (arg(0).as_int(), arg(1).as_int());
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            v(Value::Int(lo + (e.rand() % ((hi - lo) as u64 + 1)) as i32))
        }
        ("utility", "randomfloat") => {
            let (a, b) = (arg(0).as_float(), arg(1).as_float());
            let t = (e.rand() % 1_000_000) as f32 / 1_000_000.0;
            v(Value::Float(a + (b - a) * t))
        }
        ("utility", "getcurrentgametime") => v(Value::Float(e.game_days())),
        ("utility", "getcurrentrealtime") => v(Value::Float(e.scripts.real_time as f32)),
        ("utility", "ismenumode") => v(Value::Bool(false)),
        ("utility", "isingamemode") => v(Value::Bool(true)),
        // ------------------------------------------------------------- Game
        ("game", "getplayergrabbedref") => {
            v(e.grabbed_ref().map_or(Value::None, |r| e.object_value(r)))
        }
        ("game", "getplayer") => v(e.object_value(crate::engine::PLAYER_REF)),
        ("game", "getform") => v(e.object_value(FormId(arg(0).as_int() as u32))),
        ("game", "getformfromfile") => {
            let local = arg(0).as_int() as u32;
            let file = arg(1).to_string();
            match e.form_from_file(local, &file) {
                Some(id) => v(e.object_value(id)),
                None => none(),
            }
        }
        ("game", "getrealhoursspent") => v(Value::Float(e.scripts.real_time as f32 / 3600.0)),
        ("game", "isplayersleeping") | ("game", "isfasttravelenabled") => v(Value::Bool(false)),
        // Player controls: each flag given true disables (enables) that control,
        // in the order of `DisabledControls::flags_mut`. Missing arguments take
        // Papyrus's defaults.
        ("game", "disableplayercontrols") | ("game", "enableplayercontrols") => {
            let disable = func == "disableplayercontrols";
            const DISABLE: [bool; 8] = [true, true, false, false, false, true, true, false];
            for (i, flag) in e.disabled_controls.flags_mut().into_iter().enumerate() {
                if args.get(i).map_or(!disable || DISABLE[i], |a| a.as_bool()) {
                    *flag = disable;
                }
            }
            log::info!("{func}: {}", e.disabled_controls.describe());
            none()
        }
        ("game", "ismovementcontrolsenabled") => v(Value::Bool(!e.disabled_controls.movement)),
        ("game", "isfightingcontrolsenabled") => v(Value::Bool(!e.disabled_controls.fighting)),
        ("game", "iscamswitchcontrolsenabled") => v(Value::Bool(!e.disabled_controls.cam_switch)),
        ("game", "islookingcontrolsenabled") => v(Value::Bool(!e.disabled_controls.looking)),
        ("game", "issneakingcontrolsenabled") => v(Value::Bool(!e.disabled_controls.sneaking)),
        ("game", "ismenucontrolsenabled") => v(Value::Bool(!e.disabled_controls.menu)),
        ("game", "isactivatecontrolsenabled") => v(Value::Bool(!e.disabled_controls.activate)),
        ("game", "isjournalcontrolsenabled") => v(Value::Bool(!e.disabled_controls.journal)),
        ("game", "setinchargen") => none(),
        ("game", "getgamesettingfloat") => v(Value::Float(0.0)),
        ("game", "getgamesettingint") => v(Value::Int(0)),
        // ------------------------------------------------------------- Form
        ("form", "getformid") => v(Value::Int(me.map(|f| f.0 as i32).unwrap_or(0))),
        ("form", "getname") => v(Value::str(&me.map(|f| e.form_name(f)).unwrap_or_default())),
        ("form", "haskeyword") => v(Value::Bool(match (me, form_arg(args, 0)) {
            (Some(f), Some(k)) => e.has_keyword(f, k),
            _ => false,
        })),
        ("form", "gettype") => v(Value::Int(0)),
        ("form", "registerforsingleupdate")
        | ("form", "registerforupdate")
        | ("alias", "registerforsingleupdate")
        | ("alias", "registerforupdate") => {
            if let Some(t) = this.and_then(|t| t.as_object()) {
                let secs = arg(0).as_float() as f64;
                let script = this
                    .map(|t| {
                        if let Value::Object(_, c) = t {
                            c.to_string()
                        } else {
                            String::new()
                        }
                    })
                    .unwrap_or_default();
                e.scripts
                    .timers
                    .retain(|x| !(x.obj == t && x.event == "OnUpdate"));
                e.scripts.timers.push(crate::script::Timer {
                    at: e.scripts.real_time + secs,
                    game_time: false,
                    obj: t,
                    script,
                    event: "OnUpdate",
                    repeat: (func == "registerforupdate").then_some(secs),
                });
            }
            none()
        }
        ("form", "registerforsingleupdategametime") | ("form", "registerforupdategametime") => {
            if let Some(t) = this.and_then(|t| t.as_object()) {
                let hours = arg(0).as_float() as f64;
                let script = this
                    .map(|t| {
                        if let Value::Object(_, c) = t {
                            c.to_string()
                        } else {
                            String::new()
                        }
                    })
                    .unwrap_or_default();
                e.scripts
                    .timers
                    .retain(|x| !(x.obj == t && x.event == "OnUpdateGameTime"));
                e.scripts.timers.push(crate::script::Timer {
                    at: e.game_hours_total() + hours,
                    game_time: true,
                    obj: t,
                    script,
                    event: "OnUpdateGameTime",
                    repeat: (func == "registerforupdategametime").then_some(hours),
                });
            }
            none()
        }
        ("form", "unregisterforupdate")
        | ("form", "unregisterforupdategametime")
        | ("alias", "unregisterforupdate") => {
            if let Some(t) = this.and_then(|t| t.as_object()) {
                let ev = if func.ends_with("gametime") {
                    "OnUpdateGameTime"
                } else {
                    "OnUpdate"
                };
                e.scripts.timers.retain(|x| !(x.obj == t && x.event == ev));
            }
            none()
        }
        // ---------------------------------------------------- ObjectReference
        ("objectreference", "enable") | ("objectreference", "enablenowait") => {
            if let Some(f) = me {
                e.set_disabled(f, false);
            }
            none()
        }
        ("objectreference", "disable") | ("objectreference", "disablenowait") => {
            if let Some(f) = me {
                e.set_disabled(f, true);
            }
            none()
        }
        ("objectreference", "gettriggerobjectcount") => v(Value::Int(
            me.map_or(0, |f| e.trigger_object_count(f) as i32),
        )),
        ("objectreference", "isdisabled") => {
            v(Value::Bool(me.map(|f| e.is_disabled(f)).unwrap_or(true)))
        }
        ("objectreference", "isenabled") => {
            v(Value::Bool(me.map(|f| !e.is_disabled(f)).unwrap_or(false)))
        }
        ("objectreference", "getbaseobject")
        | ("actor", "getactorbase")
        | ("actor", "getleveledactorbase") => match me.and_then(|f| e.base_of(f)) {
            Some(b) => v(e.object_value(b)),
            None => none(),
        },
        ("objectreference", "getpositionx") => v(Value::Float(
            me.and_then(|f| e.ref_position(f))
                .map(|p| p.x)
                .unwrap_or(0.0),
        )),
        ("objectreference", "getpositiony") => v(Value::Float(
            me.and_then(|f| e.ref_position(f))
                .map(|p| p.y)
                .unwrap_or(0.0),
        )),
        ("objectreference", "getpositionz") => v(Value::Float(
            me.and_then(|f| e.ref_position(f))
                .map(|p| p.z)
                .unwrap_or(0.0),
        )),
        ("objectreference", "getdistance") => {
            let d = match (
                me.and_then(|f| e.ref_position(f)),
                form_arg(args, 0).and_then(|f| e.ref_position(f)),
            ) {
                (Some(a), Some(b)) => a.distance(b),
                _ => 0.0,
            };
            v(Value::Float(d))
        }
        ("objectreference", "isininterior") => v(Value::Bool(matches!(
            e.location,
            crate::engine::Location::Interior(_)
        ))),
        ("objectreference", "is3dloaded") => v(Value::Bool(true)),
        ("objectreference", "isactivationblocked") => {
            v(Value::Bool(me.is_some_and(|f| {
                e.scripts.blocked_activation.contains(&f)
            })))
        }
        ("objectreference", "blockactivation") => {
            if let Some(f) = me {
                if arg(0).as_bool() || args.is_empty() {
                    e.scripts.blocked_activation.insert(f);
                } else {
                    e.scripts.blocked_activation.remove(&f);
                }
            }
            none()
        }
        ("objectreference", "processtraphit") => {
            // ProcessTrapHit(akTrap, afDamage, afPushback, afXVel, afYVel, afZVel,
            // afXPos, afYPos, afZPos, aeMaterial, afStagger)
            if let Some(t) = me {
                e.process_trap_hit(t, form_arg(args, 0), arg(1).as_float(), arg(10).as_float());
            }
            none()
        }
        ("objectreference", "activate") => {
            // Activate(akActivator, abDefaultProcessingOnly)
            match me {
                Some(t) => e.activate_ref(t, form_arg(args, 0), arg(1).as_bool()),
                None => {
                    if let Some(t) = this.and_then(|t| t.as_object()) {
                        e.scripts
                            .pending_events
                            .push((t, "OnActivate".into(), vec![arg(0)]));
                    }
                }
            }
            v(Value::Bool(true))
        }
        ("objectreference", "islocked") => v(Value::Bool(me.is_some_and(|f| e.is_locked(f)))),
        ("objectreference", "lock") => {
            if let Some(f) = me {
                e.set_locked(f, args.first().map(|a| a.as_bool()).unwrap_or(true));
            }
            none()
        }
        ("objectreference", "getlocklevel") => v(Value::Int(
            me.and_then(|f| e.lock_of(f)).map_or(0, |l| l.level as i32),
        )),
        ("objectreference", "setlocklevel") => {
            if let Some(f) = me {
                e.scripts
                    .lock_levels
                    .insert(f, arg(0).as_int().clamp(0, 255) as u8);
            }
            none()
        }
        ("objectreference", "getlinkedref") => {
            match me.and_then(|f| e.linked_ref(f, form_arg(args, 0))) {
                Some(r) => v(e.object_value(r)),
                None => none(),
            }
        }
        ("objectreference", "getparentcell") => match me.and_then(|f| e.lo.cell_of_ref(f)) {
            Some(c) => v(e.object_value(c)),
            None => none(),
        },
        ("actor", "playidle") | ("actor", "playidlewithtarget") => {
            let event = form_arg(args, 0).and_then(|i| e.idle_event(i));
            v(Value::Bool(match (me, event) {
                (Some(a), Some(ev)) => e.play_animation_event(a, &ev),
                _ => false,
            }))
        }
        ("debug", "sendanimationevent") => {
            if let (Some(r), Some(ev)) = (
                form_arg(args, 0),
                args.get(1).and_then(|a| match a {
                    Value::String(s) => Some(s.to_string()),
                    _ => None,
                }),
            ) {
                e.play_animation_event(r, &ev);
            }
            none()
        }
        ("objectreference", "playanimation")
        | ("objectreference", "playanimationandwait")
        | ("objectreference", "playgamebryoanimation")
        | ("objectreference", "setanimationvariablebool")
        | ("objectreference", "setanimationvariablefloat")
        | ("objectreference", "setanimationvariableint")
        | ("objectreference", "setopen")
        | ("objectreference", "setdestroyed")
        | ("objectreference", "setactorowner")
        | ("objectreference", "setfactionowner")
        | ("objectreference", "setnoFavorallowed")
        | ("objectreference", "addtomap")
        | ("objectreference", "setplayerknows") => v(Value::Bool(true)),
        ("objectreference", "getopenstate") => v(Value::Int(3)),
        // ------------------------------------------------------------ Havok
        ("objectreference", "applyhavokimpulse") => {
            if let Some(r) = me {
                let dir = glam::Vec3::new(arg(0).as_float(), arg(1).as_float(), arg(2).as_float());
                e.apply_havok_impulse(r, dir, arg(3).as_float());
            }
            none()
        }
        ("objectreference", "dropobject") => {
            let made = match (me, form_arg(args, 0)) {
                (Some(r), Some(item)) => {
                    e.drop_item(r, item, if args.len() > 1 { arg(1).as_int() } else { 1 })
                }
                _ => None,
            };
            v(made.map_or(Value::None, |r| e.object_value(r)))
        }
        ("objectreference", "setmotiontype") => {
            if let Some(r) = me {
                e.set_motion_type(r, arg(0).as_int());
            }
            none()
        }
        (_, "addinventoryeventfilter")
        | (_, "removeinventoryeventfilter")
        | (_, "removeallinventoryeventfilters") => {
            if let Some(obj) = this.and_then(|t| match t {
                Value::Object(o, _) => Some(*o),
                _ => None,
            }) {
                let set = e.scripts.inventory_filters.entry(obj).or_default();
                match (func, form_arg(args, 0)) {
                    ("addinventoryeventfilter", Some(f)) => {
                        set.insert(f);
                    }
                    ("removeinventoryeventfilter", Some(f)) => {
                        set.remove(&f);
                    }
                    ("removeallinventoryeventfilters", _) => set.clear(),
                    _ => {}
                }
            }
            none()
        }
        ("objectreference", "getitemcount") => v(Value::Int(match (me, form_arg(args, 0)) {
            (Some(r), Some(item)) => e.item_count(r, FormId(item.0)),
            _ => 0,
        })),
        ("objectreference", "additem") => {
            if let (Some(r), Some(item)) = (me, form_arg(args, 0)) {
                let n = if args.len() > 1 { arg(1).as_int() } else { 1 };
                e.add_item(r, item, n);
            }
            none()
        }
        ("objectreference", "removeitem") => {
            if let (Some(r), Some(item)) = (me, form_arg(args, 0)) {
                let n = if args.len() > 1 { arg(1).as_int() } else { 1 };
                e.remove_item(r, item, n, form_arg(args, 3));
            }
            none()
        }
        ("objectreference", "removeallitems") => {
            if let Some(r) = me {
                let items = e.inventory_mut(r).items.clone();
                let to = form_arg(args, 0);
                for (f, n) in items {
                    e.remove_stack(r, f, n, None, to, None);
                }
            }
            none()
        }
        ("actor", "isequipped") => v(Value::Bool(match (me, form_arg(args, 0)) {
            (Some(r), Some(item)) => e.inventories.get(&r).is_some_and(|i| i.is_equipped(item)),
            _ => false,
        })),
        ("actor", "getequippeditemtype") => {
            let left = arg(0).as_int() == 0;
            v(Value::Int(
                me.and_then(|r| e.inventories.get(&r))
                    .map_or(0, |i| i.hand(&e.lo, left) as i32),
            ))
        }
        ("objectreference", "placeatme") | ("objectreference", "placeactoratme") => {
            let count = if func == "placeatme" {
                args.get(1).map_or(1, |c| c.as_int().max(1))
            } else {
                1
            };
            let made = match (me, form_arg(args, 0)) {
                (Some(at), Some(base)) => (0..count)
                    .filter_map(|_| e.create_ref(base, at, false))
                    .last(),
                _ => None,
            };
            v(made.map(|r| e.object_value(r)).unwrap_or(Value::None))
        }
        // MoveTo(target, x offset, y offset, z offset, match rotation = true).
        ("objectreference", "moveto") => {
            if let (Some(r), Some(t)) = (me, form_arg(args, 0))
                && let Some(p) = e.ref_position(t)
            {
                let off = glam::Vec3::new(arg(1).as_float(), arg(2).as_float(), arg(3).as_float());
                let rot = if args.get(4).is_none_or(|m| m.as_bool()) {
                    e.ref_rotation(t)
                } else {
                    e.ref_rotation(r)
                };
                e.move_ref(r, t, p + off, rot);
            }
            none()
        }
        ("objectreference", "movetomyeditorlocation") => {
            if let Some(r) = me {
                e.move_to_editor_location(r);
            }
            none()
        }
        ("objectreference", "setposition") => {
            if let Some(r) = me {
                let rot = e.ref_rotation(r);
                e.move_ref(
                    r,
                    r,
                    glam::Vec3::new(arg(0).as_float(), arg(1).as_float(), arg(2).as_float()),
                    rot,
                );
            }
            none()
        }
        // Degrees, as scripts give them.
        ("objectreference", "setangle") => {
            if let Some(r) = me
                && let Some(p) = e.ref_position(r)
            {
                let rot = glam::Vec3::new(arg(0).as_float(), arg(1).as_float(), arg(2).as_float())
                    * std::f32::consts::PI
                    / 180.0;
                e.move_ref(r, r, p, rot);
            }
            none()
        }
        ("objectreference", "getanglex")
        | ("objectreference", "getangley")
        | ("objectreference", "getanglez") => {
            let rot = me
                .map(|r| e.ref_rotation(r))
                .unwrap_or_default()
                .to_array()
                .map(f32::to_degrees);
            v(Value::Float(rot[usize::from(func.as_bytes()[8] - b'x')]))
        }
        ("objectreference", "getcurrentlocation") | ("objectreference", "getediblelocation") => {
            none()
        }
        ("objectreference", "getreftype")
        | ("objectreference", "getowningfaction")
        | ("objectreference", "getactorowner") => none(),
        ("objectreference", "isnearplayer") => v(Value::Bool(true)),
        ("objectreference", "getheadingangle") => v(Value::Float(0.0)),
        // ------------------------------------------------------------- Actor
        ("actor", "isdead") => v(Value::Bool(me.is_some_and(|r| e.is_dead(r)))),
        ("actor", "kill") | ("actor", "killessential") | ("actor", "killsilent") => {
            if let Some(r) = me {
                e.kill(r, func == "killessential");
            }
            none()
        }
        ("actor", "isbleedingout") => v(Value::Bool(me.is_some_and(|r| e.is_bleeding_out(r)))),
        ("actor", "isguard") => v(Value::Bool(me.is_some_and(|r| e.is_guard(r)))),
        // Rank -1 (potential followers' `CurrentFollowerFaction`) isn't membership.
        ("actor", "isinfaction") => v(Value::Bool(me.zip(form_arg(args, 0)).is_some_and(
            |(r, f)| {
                e.npc_factions(r)
                    .iter()
                    .any(|&(x, rank)| x == f && rank >= 0)
            },
        ))),
        ("actor", "getfactionrank") => v(Value::Int(
            me.zip(form_arg(args, 0))
                .and_then(|(r, f)| e.npc_factions(r).into_iter().find(|x| x.0 == f))
                .map_or(-1, |x| x.1 as i32),
        )),
        ("actor", "isarrested") => v(Value::Bool(false)),
        // ------------------------------------------------------------ Combat
        ("actor", "isincombat") => {
            v(Value::Bool(me.is_some_and(|r| {
                e.combat_state(r) != crate::ai::combat::CombatState::None
            })))
        }
        ("actor", "getcombatstate") => v(Value::Int(me.map_or(0, |r| e.combat_state(r) as i32))),
        ("actor", "getcombattarget") => match me.and_then(|r| e.combat_target(r)) {
            Some(t) => v(e.object_value(t)),
            None => none(),
        },
        ("actor", "startcombat") => {
            if let (Some(a), Some(t)) = (me, form_arg(args, 0)) {
                e.start_combat(a, t);
            }
            none()
        }
        // Fighting or searching, it calms down (it may find a reason to fight again).
        ("actor", "stopcombat") => {
            if let Some(a) = me {
                e.end_combat(a);
                e.stop_searching(a);
            }
            none()
        }
        // Actor values: the shared store (`actor_values`); names outside the known
        // values are plain numbers.
        ("actor", "getactorvalue")
        | ("actor", "getav")
        | ("actor", "getbaseactorvalue")
        | ("actor", "getbaseav")
        | ("actor", "getactorvaluepercentage")
        | ("actor", "getavpercentage")
        | ("actor", "getactorvaluemax")
        | ("actor", "getavmax") => {
            let (actor, name) = (me.unwrap_or_default(), arg(0).to_string());
            let Some(i) = esp::actor_value::index(&name) else {
                return v(Value::Float(e.actor_value_named(actor, &name)));
            };
            v(Value::Float(match func {
                f if f.contains("base") => e.av_base(actor, i),
                f if f.contains("percentage") => e.actor_value_fraction(actor, i),
                f if f.ends_with("max") => e.av_max(actor, i),
                _ => e.actor_value(actor, i),
            }))
        }
        ("actor", "setactorvalue")
        | ("actor", "setav")
        | ("actor", "forceactorvalue")
        | ("actor", "forceav")
        | ("actor", "modactorvalue")
        | ("actor", "modav")
        | ("actor", "damageactorvalue")
        | ("actor", "damageav")
        | ("actor", "restoreactorvalue")
        | ("actor", "restoreav") => {
            let (actor, name, x) = (
                me.unwrap_or_default(),
                arg(0).to_string(),
                arg(1).as_float(),
            );
            let Some(i) = esp::actor_value::index(&name) else {
                let key = (actor, name.to_ascii_lowercase());
                let cur = e
                    .scripts
                    .other_actor_values
                    .get(&key)
                    .copied()
                    .unwrap_or(0.0);
                let new = match func {
                    f if f.starts_with("set") || f.starts_with("force") => x,
                    f if f.starts_with("damage") => cur - x,
                    _ => cur + x,
                };
                e.scripts.other_actor_values.insert(key, new);
                return none();
            };
            match func {
                f if f.starts_with("set") => e.set_actor_value(actor, i, x),
                f if f.starts_with("force") => e.force_actor_value(actor, i, x),
                f if f.starts_with("mod") => e.mod_actor_value(actor, i, x),
                f if f.starts_with("damage") => e.damage_actor_value(actor, i, x),
                _ => e.damage_actor_value(actor, i, -x),
            }
            none()
        }
        ("actor", "getlevel") => v(Value::Int(1)),
        ("actor", "getrelationshiprank") => v(Value::Int(match (me, form_arg(args, 0)) {
            (Some(a), Some(b)) => e.relationship_rank(a, b),
            _ => 0,
        })),
        ("actor", "setrelationshiprank") => {
            if let (Some(a), Some(b)) = (me, form_arg(args, 0)) {
                e.set_relationship_rank(a, b, arg(1).as_int());
            }
            none()
        }
        ("actor", "gethighestrelationshiprank") | ("actor", "getlowestrelationshiprank") => {
            v(Value::Int(me.map_or(0, |a| {
                e.relationship_extreme(a, func == "gethighestrelationshiprank")
            })))
        }
        // ------------------------------------------------------------- Crime
        ("faction", "getcrimegold")
        | ("faction", "getcrimegoldviolent")
        | ("faction", "getcrimegoldnonviolent") => {
            let b = me.map(|f| e.bounty(f)).unwrap_or_default();
            v(Value::Int(match func {
                "getcrimegoldviolent" => b.violent,
                "getcrimegoldnonviolent" => b.nonviolent,
                _ => b.total(),
            }))
        }
        ("faction", "getinfamy")
        | ("faction", "getinfamyviolent")
        | ("faction", "getinfamynonviolent") => {
            let b = me.map(|f| e.bounty(f)).unwrap_or_default();
            v(Value::Int(match func {
                "getinfamyviolent" => b.infamy_violent,
                "getinfamynonviolent" => b.infamy_nonviolent,
                _ => b.infamy_violent + b.infamy_nonviolent,
            }))
        }
        ("faction", "modcrimegold") => {
            if let Some(f) = me {
                e.mod_crime_gold(f, arg(0).as_int(), args.get(1).is_some_and(|b| b.as_bool()));
            }
            none()
        }
        ("faction", "setcrimegold") | ("faction", "setcrimegoldviolent") => {
            if let Some(f) = me {
                e.set_crime_gold(f, arg(0).as_int(), func == "setcrimegoldviolent");
            }
            none()
        }
        ("faction", "playerpaycrimegold") => {
            if let Some(f) = me {
                e.pay_crime_gold(
                    f,
                    args.first().is_none_or(|b| b.as_bool()),
                    args.get(1).is_none_or(|b| b.as_bool()),
                );
            }
            none()
        }
        ("faction", "sendplayertojail") => {
            if let Some(f) = me {
                e.send_player_to_jail(f, args.first().is_none_or(|b| b.as_bool()));
            }
            none()
        }
        ("faction", "canpaycrimegold") => {
            v(Value::Bool(me.is_some_and(|f| e.can_pay_crime_gold(f))))
        }
        ("actor", "setplayerresistingarrest") => {
            if let Some(a) = me {
                e.set_player_resisting_arrest(a);
            }
            none()
        }
        ("actor", "getcrimefaction") => match me.and_then(|a| e.crime_faction(a)) {
            Some(f) => v(e.object_value(f)),
            None => none(),
        },
        ("actor", "setcrimefaction") => {
            if let Some(a) = me {
                e.set_crime_faction(a, form_arg(args, 0));
            }
            none()
        }
        // ---------------------------------------------------------- Location
        ("location", "getkeyworddata") => v(Value::Float(
            me.zip(form_arg(args, 0))
                .and_then(|k| e.location_keyword_data.get(&k).copied())
                .unwrap_or(0.0),
        )),
        ("location", "setkeyworddata") => {
            if let Some(k) = me.zip(form_arg(args, 0)) {
                e.location_keyword_data.insert(k, arg(1).as_float());
            }
            none()
        }
        ("actor", "isweapondrawn") => v(Value::Bool(me.is_some_and(|r| e.weapon_drawn(r)))),
        ("actor", "drawweapon") | ("actor", "sheatheweapon") => {
            if let Some(r) = me {
                e.draw_weapon(r, func == "drawweapon");
            }
            none()
        }
        ("actor", "isplayerteammate") | ("actor", "isonmount") => v(Value::Bool(false)),
        ("actor", "issneaking") => v(Value::Bool(me.is_some_and(|r| e.is_sneaking(r)))),
        ("actor", "isdetectedby") => v(Value::Bool(
            me.zip(form_arg(args, 0))
                .is_some_and(|(r, by)| e.detects(by, r)),
        )),
        ("actor", "getlightlevel") => v(Value::Float(me.map_or(0.0, |r| e.light_level(r)))),
        ("actor", "evaluatepackage")
        | ("actor", "setrestrained")
        | ("actor", "setdontmove")
        | ("actor", "setalert") => none(),
        ("actor", "getsitstate") | ("actor", "getsleepstate") => v(Value::Int(0)),
        ("actorbase", "getsex") => v(Value::Int(
            me.map(|f| e.npc_is_female(f) as i32).unwrap_or(0),
        )),
        ("actorbase", "isunique") => v(Value::Bool(true)),
        // ------------------------------------------------------------- Quest
        ("quest", "getstage") | ("quest", "getcurrentstageid") => v(Value::Int(
            me.map(|q| {
                e.scripts
                    .quests
                    .get(&q)
                    .map(|s| s.stage as i32)
                    .unwrap_or(0)
            })
            .unwrap_or(0),
        )),
        ("quest", "getstagedone") | ("quest", "isstagedone") => {
            let st = arg(0).as_int() as u16;
            v(Value::Bool(
                me.and_then(|q| e.scripts.quests.get(&q))
                    .is_some_and(|s| s.done.contains(&st)),
            ))
        }
        ("quest", "setcurrentstageid") | ("quest", "setstage") => {
            if let Some(q) = me {
                let st = arg(0).as_int() as u16;
                e.scripts.pending_stages.push((q, st));
            }
            v(Value::Bool(true))
        }
        ("quest", "isrunning") | ("quest", "isactive") => v(Value::Bool(
            me.and_then(|q| e.scripts.quests.get(&q))
                .is_some_and(|s| s.running),
        )),
        ("quest", "iscompleted") => v(Value::Bool(
            me.and_then(|q| e.scripts.quests.get(&q))
                .is_some_and(|s| s.completed),
        )),
        ("quest", "start") => v(Value::Bool(me.is_some_and(|q| e.start_quest(q)))),
        ("quest", "stop") => {
            if let Some(q) = me {
                e.stop_quest(q);
            }
            none()
        }
        // ----------------------------------------------------- Story Manager
        ("keyword", "sendstoryevent") | ("keyword", "sendstoryeventandwait") => {
            // (akLoc, akRef1, akRef2, aiValue1, aiValue2): L1, R1, R2, V1, V2; the keyword is K1.
            let mut ev = crate::story::StoryEvent::new(b"SCPT");
            ev.keyword = me.unwrap_or_default();
            ev.locs[0] = form_arg(args, 0).unwrap_or_default();
            ev.refs = [
                form_arg(args, 1).unwrap_or_default(),
                form_arg(args, 2).unwrap_or_default(),
            ];
            ev.values = [arg(3).as_int() as f32, arg(4).as_int() as f32];
            let started = e.send_story_event(ev);
            if func == "sendstoryeventandwait" {
                v(Value::Bool(started))
            } else {
                none()
            }
        }
        // ------------------------------------------------------------ Scene
        ("scene", "start") | ("scene", "forcestart") => {
            if let Some(s) = me {
                e.start_scene(s, func == "forcestart");
            }
            none()
        }
        ("scene", "stop") => {
            if let Some(s) = me {
                e.stop_scene(s);
            }
            none()
        }
        ("scene", "isplaying") => v(Value::Bool(me.is_some_and(|s| e.is_scene_playing(s)))),
        ("scene", "isactioncomplete") => {
            v(Value::Bool(me.is_some_and(|s| {
                e.is_scene_action_complete(s, arg(0).as_int() as u32)
            })))
        }
        ("scene", "getowningquest") => match me
            .and_then(|s| e.scene_def(s))
            .map(|d| d.scene.quest)
            .filter(|q| !q.is_null())
        {
            Some(q) => v(e.object_value(q)),
            None => none(),
        },
        ("quest", "completequest") => {
            if let Some(q) = me {
                let s = e.scripts.quests.entry(q).or_default();
                s.completed = true;
            }
            none()
        }
        ("quest", "setobjectivedisplayed") => {
            if let Some(q) = me {
                let obj = arg(0).as_int();
                let text = e.objective_text(q, obj);
                let s = e.scripts.quests.entry(q).or_default();
                if s.objectives_displayed.insert(obj) && !text.is_empty() {
                    e.scripts.notify(text);
                }
            }
            none()
        }
        ("quest", "setobjectivecompleted") => {
            if let Some(q) = me {
                e.scripts
                    .quests
                    .entry(q)
                    .or_default()
                    .objectives_completed
                    .insert(arg(0).as_int());
            }
            none()
        }
        ("quest", "isobjectivedisplayed") => v(Value::Bool(
            me.and_then(|q| e.scripts.quests.get(&q))
                .is_some_and(|s| s.objectives_displayed.contains(&arg(0).as_int())),
        )),
        ("quest", "isobjectivecompleted") => v(Value::Bool(
            me.and_then(|q| e.scripts.quests.get(&q))
                .is_some_and(|s| s.objectives_completed.contains(&arg(0).as_int())),
        )),
        ("quest", "getalias") => match this.and_then(|t| t.as_form()) {
            Some(q) => {
                let alias = arg(0).as_int() as u32;
                let class = if e.is_location_alias(FormId(q), alias) {
                    "LocationAlias"
                } else {
                    "ReferenceAlias"
                };
                v(Value::Object(
                    ObjectId::Alias { quest: q, alias },
                    class.into(),
                ))
            }
            None => none(),
        },
        // ---------------------------------------------------- GlobalVariable
        ("globalvariable", "getvalue") => {
            v(Value::Float(me.map(|g| e.global_value(g)).unwrap_or(0.0)))
        }
        ("globalvariable", "getvalueint") => v(Value::Int(
            me.map(|g| e.global_value(g) as i32).unwrap_or(0),
        )),
        ("globalvariable", "setvalue") | ("globalvariable", "setvalueint") => {
            if let Some(g) = me {
                e.scripts.globals.insert(g, arg(0).as_float());
            }
            none()
        }
        ("globalvariable", "mod") => {
            let cur = me.map(|g| e.global_value(g)).unwrap_or(0.0);
            if let Some(g) = me {
                e.scripts.globals.insert(g, cur + arg(0).as_float());
            }
            v(Value::Float(cur + arg(0).as_float()))
        }
        // ------------------------------------------------------------- Alias
        ("referencealias", "getreference")
        | ("referencealias", "getactorreference")
        | ("referencealias", "getactorref") => match this {
            Some(Value::Object(ObjectId::Alias { quest, alias }, _)) => {
                match e.alias_ref(FormId(*quest), *alias) {
                    Some(r) => v(e.object_value(r)),
                    None => none(),
                }
            }
            _ => none(),
        },
        ("referencealias", "forcerefto") | ("referencealias", "forcereftoifempty") => {
            if let (Some(Value::Object(ObjectId::Alias { quest, alias }, _)), Some(r)) =
                (this, form_arg(args, 0))
            {
                let q = e.scripts.quests.entry(FormId(*quest)).or_default();
                if func == "forcerefto" || !q.aliases.contains_key(alias) {
                    q.aliases.insert(*alias, r);
                    e.scripts.alias_gen += 1;
                }
            }
            none()
        }
        ("locationalias", "getlocation") => match this {
            Some(Value::Object(ObjectId::Alias { quest, alias }, _)) => {
                match e.alias_ref(FormId(*quest), *alias) {
                    Some(l) => v(e.object_value(l)),
                    None => none(),
                }
            }
            _ => none(),
        },
        ("locationalias", "forcelocationto") => {
            if let (Some(Value::Object(ObjectId::Alias { quest, alias }, _)), Some(l)) =
                (this, form_arg(args, 0))
            {
                e.scripts
                    .quests
                    .entry(FormId(*quest))
                    .or_default()
                    .aliases
                    .insert(*alias, l);
                e.scripts.alias_gen += 1;
            }
            none()
        }
        ("referencealias", "clear") | ("locationalias", "clear") => {
            if let Some(Value::Object(ObjectId::Alias { quest, alias }, _)) = this {
                e.scripts
                    .quests
                    .entry(FormId(*quest))
                    .or_default()
                    .aliases
                    .remove(alias);
                e.scripts.alias_gen += 1;
            }
            none()
        }
        ("alias", "getowningquest") => match this {
            Some(Value::Object(ObjectId::Alias { quest, .. }, _)) => {
                v(e.object_value(FormId(*quest)))
            }
            _ => none(),
        },
        // ------------------------------------------------------------- Misc
        ("sound", "play") => v(Value::Int(0)),
        ("sound", "playandwait") => none(),
        ("message", "show") => {
            if let Some(m) = me {
                let text = e.message_text(m);
                if !text.is_empty() {
                    e.scripts.notify(text);
                }
            }
            v(Value::Int(0))
        }
        ("formlist", "getsize") => v(Value::Int(
            me.map(|f| e.formlist(f).len() as i32).unwrap_or(0),
        )),
        ("formlist", "getat") => {
            match me.and_then(|f| e.formlist(f).get(arg(0).as_int().max(0) as usize).copied()) {
                Some(x) => v(e.object_value(x)),
                None => none(),
            }
        }
        ("formlist", "hasform") => v(Value::Bool(match (me, form_arg(args, 0)) {
            (Some(f), Some(x)) => e.formlist(f).contains(&x),
            _ => false,
        })),
        ("keyword", "getkeyword") => none(),
        ("cell", "isattached") | ("cell", "isinterior") => v(Value::Bool(
            func == "isattached" || matches!(e.location, crate::engine::Location::Interior(_)),
        )),
        ("math", "abs") => v(Value::Float(arg(0).as_float().abs())),
        ("math", "sqrt") => v(Value::Float(arg(0).as_float().sqrt())),
        ("math", "pow") => v(Value::Float(arg(0).as_float().powf(arg(1).as_float()))),
        ("math", "sin") => v(Value::Float(arg(0).as_float().to_radians().sin())),
        ("math", "cos") => v(Value::Float(arg(0).as_float().to_radians().cos())),
        ("math", "floor") => v(Value::Int(arg(0).as_float().floor() as i32)),
        ("math", "ceiling") => v(Value::Int(arg(0).as_float().ceil() as i32)),
        _ => {
            let key = format!("{class}.{func}");
            if e.scripts.warned.insert(key.clone()) {
                log::debug!("unimplemented native {key}");
            }
            // Reasonable defaults: functions returning objects/ints get None/0.
            none()
        }
    }
}
