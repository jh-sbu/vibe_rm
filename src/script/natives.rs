//! Native function implementations.

use esp::FormId;
use papyrus::{NativeResult, ObjectId, Value};

use crate::engine::Engine;
use crate::world::records;

fn v(x: Value) -> NativeResult {
    NativeResult::Value(x)
}
fn none() -> NativeResult {
    NativeResult::Value(Value::None)
}

fn form_arg(args: &[Value], i: usize) -> Option<FormId> {
    args.get(i).and_then(|a| a.as_form()).map(FormId)
}

pub fn call(e: &mut Engine, class: &str, func: &str, this: Option<&Value>, args: &[Value]) -> NativeResult {
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
        ("game", "isactivatecontrolsenabled") | ("game", "ismovementcontrolsenabled") => v(Value::Bool(true)),
        ("game", "disableplayercontrols") | ("game", "enableplayercontrols") | ("game", "setinchargen") => none(),
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
        ("form", "registerforsingleupdate") | ("form", "registerforupdate") | ("alias", "registerforsingleupdate") | ("alias", "registerforupdate") => {
            if let Some(t) = this.and_then(|t| t.as_object()) {
                let secs = arg(0).as_float() as f64;
                let script = this.map(|t| if let Value::Object(_, c) = t { c.to_string() } else { String::new() }).unwrap_or_default();
                e.scripts.timers.retain(|x| !(x.obj == t && x.event == "OnUpdate"));
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
                let script = this.map(|t| if let Value::Object(_, c) = t { c.to_string() } else { String::new() }).unwrap_or_default();
                e.scripts.timers.retain(|x| !(x.obj == t && x.event == "OnUpdateGameTime"));
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
        ("form", "unregisterforupdate") | ("form", "unregisterforupdategametime") | ("alias", "unregisterforupdate") => {
            if let Some(t) = this.and_then(|t| t.as_object()) {
                let ev = if func.ends_with("gametime") { "OnUpdateGameTime" } else { "OnUpdate" };
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
        ("objectreference", "isdisabled") => v(Value::Bool(me.map(|f| e.is_disabled(f)).unwrap_or(true))),
        ("objectreference", "isenabled") => v(Value::Bool(me.map(|f| !e.is_disabled(f)).unwrap_or(false))),
        ("objectreference", "getbaseobject") | ("actor", "getactorbase") | ("actor", "getleveledactorbase") => match me.and_then(|f| e.base_of(f)) {
            Some(b) => v(e.object_value(b)),
            None => none(),
        },
        ("objectreference", "getpositionx") => v(Value::Float(me.and_then(|f| e.ref_position(f)).map(|p| p.x).unwrap_or(0.0))),
        ("objectreference", "getpositiony") => v(Value::Float(me.and_then(|f| e.ref_position(f)).map(|p| p.y).unwrap_or(0.0))),
        ("objectreference", "getpositionz") => v(Value::Float(me.and_then(|f| e.ref_position(f)).map(|p| p.z).unwrap_or(0.0))),
        ("objectreference", "getdistance") => {
            let d = match (me.and_then(|f| e.ref_position(f)), form_arg(args, 0).and_then(|f| e.ref_position(f))) {
                (Some(a), Some(b)) => a.distance(b),
                _ => 0.0,
            };
            v(Value::Float(d))
        }
        ("objectreference", "isininterior") => v(Value::Bool(matches!(e.location, crate::engine::Location::Interior(_)))),
        ("objectreference", "is3dloaded") => v(Value::Bool(true)),
        ("objectreference", "isactivationblocked") => v(Value::Bool(me.is_some_and(|f| e.scripts.blocked_activation.contains(&f)))),
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
        ("objectreference", "activate") => {
            if let Some(t) = this.and_then(|t| t.as_object()) {
                e.scripts.pending_events.push((t, "OnActivate".into(), vec![arg(0)]));
            }
            v(Value::Bool(true))
        }
        ("objectreference", "islocked") => v(Value::Bool(me.and_then(|f| e.scripts.locked.get(&f).copied()).unwrap_or(false))),
        ("objectreference", "lock") => {
            if let Some(f) = me {
                e.scripts.locked.insert(f, args.first().map(|a| a.as_bool()).unwrap_or(true));
            }
            none()
        }
        ("objectreference", "getlinkedref") => match me.and_then(|f| e.linked_ref(f, form_arg(args, 0))) {
            Some(r) => v(e.object_value(r)),
            None => none(),
        },
        ("objectreference", "getparentcell") => match me.and_then(|f| e.lo.cell_of_ref(f)) {
            Some(c) => v(e.object_value(c)),
            None => none(),
        },
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
        ("objectreference", "getitemcount") => v(Value::Int(0)),
        ("objectreference", "additem") | ("objectreference", "removeitem") | ("objectreference", "removeallitems") => none(),
        ("objectreference", "moveto") => {
            if me == Some(crate::engine::PLAYER_REF)
                && let Some(t) = form_arg(args, 0)
            {
                e.queue_player_moveto(t);
            }
            none()
        }
        ("objectreference", "getcurrentlocation") | ("objectreference", "getediblelocation") => none(),
        ("objectreference", "getreftype") | ("objectreference", "getowningfaction") | ("objectreference", "getactorowner") => none(),
        ("objectreference", "isnearplayer") => v(Value::Bool(true)),
        ("objectreference", "getheadingangle") => v(Value::Float(0.0)),
        // ------------------------------------------------------------- Actor
        ("actor", "isdead") => v(Value::Bool(false)),
        ("actor", "isincombat") | ("actor", "isinfaction") | ("actor", "isguard") | ("actor", "isarrested") | ("actor", "isbleedingout") => {
            v(Value::Bool(false))
        }
        ("actor", "getactorvalue") | ("actor", "getav") | ("actor", "getbaseactorvalue") | ("actor", "getbaseav") => {
            let key = (me.unwrap_or_default(), arg(0).to_string().to_ascii_lowercase());
            v(Value::Float(e.scripts.actor_values.get(&key).copied().unwrap_or(100.0)))
        }
        ("actor", "setactorvalue") | ("actor", "setav") | ("actor", "forceactorvalue") | ("actor", "forceav") => {
            let key = (me.unwrap_or_default(), arg(0).to_string().to_ascii_lowercase());
            e.scripts.actor_values.insert(key, arg(1).as_float());
            none()
        }
        ("actor", "modactorvalue") | ("actor", "modav") | ("actor", "damageactorvalue") | ("actor", "damageav") | ("actor", "restoreactorvalue") | ("actor", "restoreav") => {
            let key = (me.unwrap_or_default(), arg(0).to_string().to_ascii_lowercase());
            let sign = if func.starts_with("damage") { -1.0 } else { 1.0 };
            let cur = e.scripts.actor_values.get(&key).copied().unwrap_or(100.0);
            e.scripts.actor_values.insert(key, cur + sign * arg(1).as_float());
            none()
        }
        ("actor", "getlevel") => v(Value::Int(1)),
        ("actor", "getfactionrank") => v(Value::Int(-1)),
        ("actor", "isplayerteammate") | ("actor", "isweapondrawn") | ("actor", "issneaking") | ("actor", "isonmount") => v(Value::Bool(false)),
        ("actor", "evaluatepackage") | ("actor", "setrestrained") | ("actor", "setdontmove") | ("actor", "setalert") | ("actor", "stopcombat") => none(),
        ("actor", "getsitstate") | ("actor", "getsleepstate") => v(Value::Int(0)),
        ("actorbase", "getsex") => v(Value::Int(me.map(|f| e.npc_is_female(f) as i32).unwrap_or(0))),
        ("actorbase", "isunique") => v(Value::Bool(true)),
        // ------------------------------------------------------------- Quest
        ("quest", "getstage") | ("quest", "getcurrentstageid") => {
            v(Value::Int(me.map(|q| e.scripts.quests.get(&q).map(|s| s.stage as i32).unwrap_or(0)).unwrap_or(0)))
        }
        ("quest", "getstagedone") | ("quest", "isstagedone") => {
            let st = arg(0).as_int() as u16;
            v(Value::Bool(me.and_then(|q| e.scripts.quests.get(&q)).is_some_and(|s| s.done.contains(&st))))
        }
        ("quest", "setcurrentstageid") | ("quest", "setstage") => {
            if let Some(q) = me {
                let st = arg(0).as_int() as u16;
                e.scripts.pending_stages.push((q, st));
            }
            v(Value::Bool(true))
        }
        ("quest", "isrunning") | ("quest", "isactive") => v(Value::Bool(me.and_then(|q| e.scripts.quests.get(&q)).is_some_and(|s| s.running))),
        ("quest", "iscompleted") => v(Value::Bool(me.and_then(|q| e.scripts.quests.get(&q)).is_some_and(|s| s.completed))),
        ("quest", "start") => {
            if let Some(q) = me {
                e.start_quest(q);
            }
            v(Value::Bool(true))
        }
        ("quest", "stop") => {
            if let Some(q) = me {
                e.scripts.quests.entry(q).or_default().running = false;
            }
            none()
        }
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
                e.scripts.quests.entry(q).or_default().objectives_completed.insert(arg(0).as_int());
            }
            none()
        }
        ("quest", "isobjectivedisplayed") => {
            v(Value::Bool(me.and_then(|q| e.scripts.quests.get(&q)).is_some_and(|s| s.objectives_displayed.contains(&arg(0).as_int()))))
        }
        ("quest", "isobjectivecompleted") => {
            v(Value::Bool(me.and_then(|q| e.scripts.quests.get(&q)).is_some_and(|s| s.objectives_completed.contains(&arg(0).as_int()))))
        }
        ("quest", "getalias") => match this.and_then(|t| t.as_form()) {
            Some(q) => v(Value::Object(ObjectId::Alias { quest: q, alias: arg(0).as_int() as u32 }, "ReferenceAlias".into())),
            None => none(),
        },
        // ---------------------------------------------------- GlobalVariable
        ("globalvariable", "getvalue") => v(Value::Float(me.map(|g| e.global_value(g)).unwrap_or(0.0))),
        ("globalvariable", "getvalueint") => v(Value::Int(me.map(|g| e.global_value(g) as i32).unwrap_or(0))),
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
        ("referencealias", "getreference") | ("referencealias", "getactorreference") | ("referencealias", "getactorref") => none(),
        ("referencealias", "forcerefto") | ("referencealias", "clear") | ("referencealias", "forcereftoifempty") => none(),
        ("alias", "getowningquest") => match this {
            Some(Value::Object(ObjectId::Alias { quest, .. }, _)) => v(e.object_value(FormId(*quest))),
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
        ("formlist", "getsize") => v(Value::Int(me.map(|f| e.formlist(f).len() as i32).unwrap_or(0))),
        ("formlist", "getat") => match me.and_then(|f| e.formlist(f).get(arg(0).as_int().max(0) as usize).copied()) {
            Some(x) => v(e.object_value(x)),
            None => none(),
        },
        ("formlist", "hasform") => v(Value::Bool(match (me, form_arg(args, 0)) {
            (Some(f), Some(x)) => e.formlist(f).contains(&x),
            _ => false,
        })),
        ("keyword", "getkeyword") => none(),
        ("cell", "isattached") | ("cell", "isinterior") => v(Value::Bool(func == "isattached" || matches!(e.location, crate::engine::Location::Interior(_)))),
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
