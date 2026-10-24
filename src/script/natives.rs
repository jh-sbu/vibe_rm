//! Native function implementations.

use esp::FormId;
use papyrus::{NativeResult, ObjectId, Value};

use crate::engine::Engine;

mod math;

fn v(x: Value) -> NativeResult {
    NativeResult::Value(x)
}
fn none() -> NativeResult {
    NativeResult::Value(Value::None)
}

fn str_arg(args: &[Value], i: usize) -> String {
    match args.get(i) {
        Some(Value::String(s)) => s.to_string(),
        _ => String::new(),
    }
}

fn form_arg(args: &[Value], i: usize) -> Option<FormId> {
    args.get(i).and_then(|a| a.as_form()).map(FormId)
}

/// The active magic effect a method runs on.
fn effect_of(this: Option<&Value>) -> Option<u64> {
    match this {
        Some(Value::Object(ObjectId::Effect(id), _)) => Some(*id),
        _ => None,
    }
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
            e.debug_message_box(arg(0).to_string());
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
        // `Wait` stops in menu mode (the game clock does), `WaitMenuMode` goes on.
        ("utility", "wait") => NativeResult::Wait(arg(0).as_float()),
        ("utility", "waitmenumode") => NativeResult::WaitMenuMode(arg(0).as_float()),
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
        ("utility", "getcurrentrealtime") => v(Value::Float(e.scripts.wall_time as f32)),
        ("utility", "isinmenumode") => v(Value::Bool(e.in_menu_mode())),
        ("utility", "isingamemode") => v(Value::Bool(true)),
        // ------------------------------------------------------------- Game
        ("game", "getplayergrabbedref") => {
            v(e.grabbed_ref().map_or(Value::None, |r| e.object_value(r)))
        }
        ("game", "incrementstat") => {
            let by = args.get(1).map_or(1, |a| a.as_int());
            e.increment_stat(&arg(0).to_string(), by);
            none()
        }
        // The trainers' dialogue: the training menu, for the speaker.
        ("game", "showtrainingmenu") => {
            if let Some(t) = arg(0).as_form() {
                e.show_training_menu(FormId(t));
            }
            none()
        }
        ("game", "querystat") => v(Value::Int(e.query_stat(&arg(0).to_string()))),
        ("game", "requestsave") | ("game", "requestautosave") => {
            e.request_save(func == "requestautosave");
            v(Value::None)
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
        ("game", "getrealhoursspent") => v(Value::Float(e.scripts.wall_time as f32 / 3600.0)),
        // (abFadingOut, abBlackFade, afSecsBeforeFade, afFadeDuration)
        ("game", "fadeoutgame") => {
            e.fade_out_game(
                arg(0).as_bool(),
                arg(1).as_bool(),
                arg(2).as_float(),
                arg(3).as_float(),
            );
            none()
        }
        // (akSource = None, afStrength = 0.5, afDuration = 0.0)
        ("game", "shakecamera") => {
            let strength = args.get(1).map_or(0.5, |a| a.as_float());
            e.shake_camera(form_arg(args, 0), strength, arg(2).as_float());
            none()
        }
        ("imagespacemodifier", "apply") => {
            if let Some(m) = me {
                e.apply_imod(m, args.first().map_or(1.0, |a| a.as_float()));
            }
            none()
        }
        ("imagespacemodifier", "applycrossfade") => {
            if let Some(m) = me {
                e.apply_imod_cross_fade(m, args.first().map_or(1.0, |a| a.as_float()));
            }
            none()
        }
        ("imagespacemodifier", "remove") => {
            if let Some(m) = me {
                e.remove_imod(m);
            }
            none()
        }
        // This modifier off, another on (akNewModifier, afStrength = 1.0).
        ("imagespacemodifier", "popto") => {
            if let Some(m) = me {
                e.remove_imod(m);
            }
            if let Some(next) = form_arg(args, 0) {
                e.apply_imod(next, args.get(1).map_or(1.0, |a| a.as_float()));
            }
            none()
        }
        ("imagespacemodifier", "removecrossfade") => {
            e.remove_imod_cross_fade(args.first().map_or(1.0, |a| a.as_float()));
            none()
        }
        // ---------------------------------------------------------- Weather
        // (abOverride = false)
        ("weather", "forceactive") => {
            if let Some(w) = me {
                e.force_weather(w, arg(0).as_bool());
            }
            none()
        }
        // (abOverride = false, abAccelerate = false)
        ("weather", "setactive") => {
            if let Some(w) = me {
                e.set_weather(w, arg(0).as_bool(), arg(1).as_bool());
            }
            none()
        }
        ("weather", "releaseoverride") => {
            e.release_weather_override();
            none()
        }
        ("weather", "getcurrentweather") => v(e
            .current_weather()
            .map_or(Value::None, |w| e.object_value(w))),
        ("weather", "getoutgoingweather") => v(e
            .outgoing_weather()
            .map_or(Value::None, |w| e.object_value(w))),
        ("weather", "getcurrentweathertransition") => v(Value::Float(e.weather.pct)),
        ("weather", "getskymode") => v(Value::Int(e.sky_mode())),
        ("weather", "findweather") => {
            let w = e.find_weather(arg(0).as_int());
            v(w.map_or(Value::None, |w| e.object_value(w)))
        }
        ("weather", "getclassification") => {
            let class = me
                .and_then(|w| e.weather_record(w))
                .map_or(-1, |w| w.classification());
            v(Value::Int(class))
        }
        ("game", "isplayersleeping") => v(Value::Bool(false)),
        ("game", "isfasttravelenabled") => v(Value::Bool(!e.fast_travel_disabled)),
        ("game", "enablefasttravel") => {
            e.fast_travel_disabled = !args.first().is_none_or(Value::as_bool);
            none()
        }
        // Player controls: each flag given true disables (enables) that control,
        // in Papyrus argument order. Missing arguments take
        // Papyrus's defaults.
        ("game", "disableplayercontrols") | ("game", "enableplayercontrols") => {
            let disable = func == "disableplayercontrols";
            const DISABLE: [bool; 8] = [true, true, false, false, false, true, true, false];
            let selected = std::array::from_fn(|i| {
                args.get(i).map_or(!disable || DISABLE[i], |a| a.as_bool())
            });
            let pov_type = args.get(8).map_or(0, Value::as_int);
            e.disabled_controls.apply(disable, selected, pov_type);
            log::info!("{func}: {}", e.disabled_controls.describe());
            // Disabling fighting puts a drawn weapon away (`test-mod
            // autotest`).
            if disable && e.disabled_controls.fighting && e.player_weapon_drawn() {
                e.player_draw_weapon(false);
            }
            none()
        }
        ("game", "ismovementcontrolsenabled") => v(Value::Bool(!e.disabled_controls.movement)),
        ("game", "isfightingcontrolsenabled") => v(Value::Bool(!e.disabled_controls.fighting)),
        ("game", "iscamswitchcontrolsenabled") => {
            v(Value::Bool(!e.disabled_controls.camera_switch_disabled()))
        }
        ("game", "islookingcontrolsenabled") => v(Value::Bool(!e.disabled_controls.looking)),
        ("game", "issneakingcontrolsenabled") => v(Value::Bool(!e.disabled_controls.sneaking)),
        ("game", "ismenucontrolsenabled") => v(Value::Bool(!e.disabled_controls.menu)),
        ("game", "isactivatecontrolsenabled") => v(Value::Bool(!e.disabled_controls.activate)),
        ("game", "isjournalcontrolsenabled") => v(Value::Bool(!e.disabled_controls.journal)),
        ("game", "sethudcartmode") => {
            e.hud_cart_mode = args.first().is_none_or(Value::as_bool);
            none()
        }
        ("game", "forcefirstperson") => {
            e.set_third_person(false);
            none()
        }
        ("game", "forcethirdperson") => {
            e.set_third_person(true);
            none()
        }
        ("game", "setinchargen") => none(),
        ("game", "getgamesettingfloat") => v(Value::Float(crate::ai::combat::gmst_f32(
            &e.lo,
            &str_arg(args, 0),
            0.0,
        ))),
        ("game", "getgamesettingint") => v(Value::Int(crate::ai::combat::gmst_i32(
            &e.lo,
            &str_arg(args, 0),
            0,
        ))),
        // Skills (`crate::skills`): the player's.
        ("game", "advanceskill") => {
            if let Some(skill) = crate::skills::skill_index(&str_arg(args, 0)) {
                e.use_skill(skill, arg(1).as_float());
            }
            none()
        }
        ("game", "incrementskill") | ("game", "incrementskillby") => {
            let times = if func == "incrementskill" {
                1
            } else {
                arg(1).as_int()
            };
            if let Some(skill) = crate::skills::skill_index(&str_arg(args, 0)) {
                for _ in 0..times {
                    e.increment_skill(skill);
                }
            }
            none()
        }
        ("game", "addperkpoints") => {
            let n = e.skills.perk_points as i64 + arg(0).as_int() as i64;
            e.skills.perk_points = n.clamp(0, 255) as u32;
            none()
        }
        // ------------------------------------------------------------- Form
        ("form", "getformid") => v(Value::Int(me.map(|f| f.0 as i32).unwrap_or(0))),
        ("form", "haskeyword") => v(Value::Bool(match (me, form_arg(args, 0)) {
            (Some(f), Some(k)) => e.has_keyword(f, k),
            _ => false,
        })),
        ("form", "registerforsingleupdate")
        | ("form", "registerforupdate")
        | ("alias", "registerforsingleupdate")
        | ("alias", "registerforupdate")
        | ("activemagiceffect", "registerforsingleupdate")
        | ("activemagiceffect", "registerforupdate") => {
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
        ("form", "registerforsingleupdategametime")
        | ("form", "registerforupdategametime")
        | ("activemagiceffect", "registerforsingleupdategametime")
        | ("activemagiceffect", "registerforupdategametime") => {
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
        ("form", "registerforsleep")
        | ("alias", "registerforsleep")
        | ("activemagiceffect", "registerforsleep")
        | ("form", "unregisterforsleep")
        | ("alias", "unregisterforsleep")
        | ("activemagiceffect", "unregisterforsleep") => {
            if let Some(t) = this.and_then(|t| t.as_object()) {
                e.register_for_sleep(t, func == "registerforsleep");
            }
            none()
        }
        ("form", "unregisterforupdate")
        | ("form", "unregisterforupdategametime")
        | ("alias", "unregisterforupdate")
        | ("activemagiceffect", "unregisterforupdate")
        | ("activemagiceffect", "unregisterforupdategametime") => {
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
        ("objectreference", "getcurrentdestructionstage") => v(Value::Int(
            me.map_or(-1, |f| e.current_destruction_stage(f)),
        )),
        ("objectreference", "damageobject") => {
            if let Some(reference) = me {
                e.damage_object(reference, arg(0).as_float());
            }
            none()
        }
        ("objectreference", "cleardestruction") => {
            if let Some(reference) = me {
                e.clear_destruction(reference);
            }
            none()
        }
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
                e.lock_reference(
                    f,
                    args.first().map(|a| a.as_bool()).unwrap_or(true),
                    args.get(1).is_some_and(|a| a.as_bool()),
                );
            }
            none()
        }
        ("objectreference", "getlocklevel") => v(Value::Int(
            me.and_then(|f| e.lock_of(f)).map_or(0, |l| l.level as i32),
        )),
        ("objectreference", "setlocklevel") => {
            if let Some(f) = me {
                e.set_lock_level(f, arg(0).as_int().clamp(0, 255) as u8);
            }
            none()
        }
        ("objectreference", "getlinkedref") => {
            match me.and_then(|f| e.linked_ref(f, form_arg(args, 0))) {
                Some(r) => v(e.object_value(r)),
                None => none(),
            }
        }
        // (aiLinkedRef): follow the unkeyworded link that many times; 0 is
        // the reference itself.
        ("objectreference", "getnthlinkedref") => {
            let mut at = me;
            for _ in 0..arg(0).as_int().max(0) {
                at = at.and_then(|r| e.linked_ref(r, None));
            }
            match at {
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
            ) && !e.play_animation_event(r, &ev)
            {
                // Not an actor: an object's graph.
                e.play_object_animation(r, &ev);
            }
            none()
        }
        // An object's graph, else an actor's.
        ("objectreference", "playanimation") | ("objectreference", "playanimationandwait") => {
            let event = str_arg(args, 0);
            let Some(r) = me else {
                return v(Value::Bool(false));
            };
            if !(e.play_object_animation(r, &event) || e.play_animation_event(r, &event)) {
                return v(Value::Bool(false));
            }
            if func == "playanimation" {
                return v(Value::Bool(true));
            }
            // Wait for the graph to raise asEventName.
            let wait = str_arg(args, 1);
            log::debug!("{r}: {event}, waiting for {wait}");
            NativeResult::WaitFor {
                key: e.anim_events.wait_for(r, &wait),
                timeout: crate::anim_events::WAIT_LIMIT,
                value: Value::Bool(true),
            }
        }
        ("form", "registerforanimationevent")
        | ("alias", "registerforanimationevent")
        | ("activemagiceffect", "registerforanimationevent")
        | ("form", "unregisterforanimationevent")
        | ("alias", "unregisterforanimationevent")
        | ("activemagiceffect", "unregisterforanimationevent") => {
            let (Some(Value::Object(receiver, script)), Some(sender)) = (this, form_arg(args, 0))
            else {
                return v(Value::Bool(false));
            };
            let event = str_arg(args, 1);
            if func.starts_with("register") {
                v(Value::Bool(
                    e.register_anim_event(*receiver, script, sender, &event),
                ))
            } else {
                e.unregister_anim_event(*receiver, script, sender, &event);
                none()
            }
        }
        ("objectreference", "playgamebryoanimation") => {
            let name = str_arg(args, 0);
            v(Value::Bool(
                me.is_some_and(|r| e.play_gamebryo_animation(r, &name)),
            ))
        }
        ("objectreference", "setanimationvariablebool")
        | ("objectreference", "setanimationvariablefloat")
        | ("objectreference", "setanimationvariableint") => {
            if let Some(r) = me {
                let value = match arg(1) {
                    Value::Bool(b) => b as i32 as f32,
                    Value::Int(i) => i as f32,
                    v => v.as_float(),
                };
                e.set_anim_variable(r, &str_arg(args, 0), value);
            }
            none()
        }
        ("objectreference", "getanimationvariablebool")
        | ("objectreference", "getanimationvariablefloat")
        | ("objectreference", "getanimationvariableint") => {
            let x = me.map_or(0.0, |r| e.anim_variable(r, &str_arg(args, 0)));
            v(match func {
                "getanimationvariablebool" => Value::Bool(x != 0.0),
                "getanimationvariableint" => Value::Int(x as i32),
                _ => Value::Float(x),
            })
        }
        ("objectreference", "setopen")
        | ("objectreference", "setdestroyed")
        | ("objectreference", "setnoFavorallowed") => v(Value::Bool(true)),
        // Answered at the end of the frame, once the scripts it set going have
        // run: the pillar puzzles' lever activates its gate and asks at once,
        // and in the game sees it opening (it closes the gate again when a
        // pillar is then turned wrong).
        ("objectreference", "getopenstate") => {
            let Some(r) = me else {
                return v(Value::Int(0));
            };
            if !e.scripts.open_state_queries.contains(&r) {
                e.scripts.open_state_queries.push(r);
            }
            NativeResult::WaitFor {
                key: crate::world::animated::open_state_key(r),
                timeout: 1.0,
                value: Value::Int(e.open_state(r) as i32),
            }
        }
        ("objectreference", "addtomap") => {
            if let Some(r) = me {
                e.add_to_map(r, args.first().is_some_and(|a| a.as_bool()));
            }
            none()
        }
        ("objectreference", "enablefasttravel") => {
            if let Some(r) = me {
                e.enable_fast_travel(r, args.first().is_none_or(|a| a.as_bool()));
            }
            none()
        }
        ("objectreference", "canfasttraveltomarker") => {
            v(Value::Bool(me.is_some_and(|r| e.can_fast_travel_to(r))))
        }
        ("objectreference", "ismapmarkervisible") => {
            v(Value::Bool(me.is_some_and(|r| e.map_marker_visible(r))))
        }
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
                // An NPC given a weapon it would pick wields it at once;
                // armor and shields it leaves be, but a teammate wears armor
                // (`test-mod autotest2`).
                if e.lo.tag_of(item).is_some_and(|t| t.0 == *b"WEAP") || e.is_player_teammate(r) {
                    e.take_up_if_better(r, item);
                }
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
        // The weapon in a hand (a two-handed one is the right hand's).
        ("actor", "getequippedweapon") => {
            let inv = me.and_then(|r| e.inventories.get(&r));
            let weapon = inv.and_then(|i| match arg(0).as_bool() {
                true if i.both_hands() => None,
                true => i.weapon_in(&e.lo, crate::spells::LEFT),
                false => i.weapon(&e.lo),
            });
            v(weapon.map_or(Value::None, |w| e.object_value(w)))
        }
        ("actor", "getequippedshield") => {
            let shield = me.and_then(|r| e.inventories.get(&r)?.shield(&e.lo));
            v(shield.map_or(Value::None, |s| e.object_value(s)))
        }
        ("objectreference", "delete") => {
            if let Some(r) = me {
                e.delete_reference(r);
            }
            none()
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
                if r == crate::engine::PLAYER_REF {
                    e.fast_travel_disabled = false;
                }
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
                if r == crate::engine::PLAYER_REF {
                    e.fast_travel_disabled = false;
                }
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
        ("objectreference", "geteditorlocation") => v(me
            .and_then(|r| e.editor_location(r))
            .map_or(Value::None, |l| e.object_value(l))),
        ("objectreference", "getcurrentlocation") => v(me
            .and_then(|r| e.ref_current_location(r))
            .map_or(Value::None, |l| e.object_value(l))),
        ("objectreference", "getediblelocation") => none(),
        ("objectreference", "getreftype") | ("objectreference", "getowningfaction") => none(),
        // Owners: an actor base (the player's is 7) or a faction.
        ("objectreference", "setactorowner")
        | ("objectreference", "setfactionowner")
        | ("cell", "setactorowner")
        | ("cell", "setfactionowner") => {
            if let Some(r) = me {
                e.scripts.owners.insert(r, form_arg(args, 0));
            }
            none()
        }
        ("objectreference", "getactorowner") | ("cell", "getactorowner") => {
            let owner = me.and_then(|r| e.ref_owner(r));
            let npc = owner.filter(|&o| e.lo.tag_of(o).map(|t| t.0) == Some(*b"NPC_"));
            v(npc.map_or(Value::None, |o| e.object_value(o)))
        }
        ("objectreference", "getfactionowner") | ("cell", "getfactionowner") => {
            let owner = me.and_then(|r| e.ref_owner(r));
            let faction = owner.filter(|&o| e.lo.tag_of(o).map(|t| t.0) == Some(*b"FACT"));
            v(faction.map_or(Value::None, |o| e.object_value(o)))
        }
        ("objectreference", "isnearplayer") => v(Value::Bool(true)),
        ("objectreference", "getheadingangle") => v(Value::Float(0.0)),
        // ------------------------------------------------------------- Actor
        ("actor", "isdead") => v(Value::Bool(me.is_some_and(|r| e.is_dead(r)))),
        ("actor", "getkiller") => {
            let killer = me
                .filter(|&actor| e.is_dead(actor))
                .and_then(|actor| e.death(actor))
                .and_then(|death| death.killer);
            v(killer.map_or(Value::None, |actor| e.object_value(actor)))
        }
        // `Caster.TrapSoul(Victim)` (the soul trap effect's script, as it
        // finishes): the dead victim's soul into the caster's gems.
        ("actor", "trapsoul") => {
            let trapped = match (me, form_arg(args, 0)) {
                (Some(caster), Some(victim)) => e.trap_soul(caster, victim),
                _ => false,
            };
            v(Value::Bool(trapped))
        }
        ("actor", "kill") | ("actor", "killessential") | ("actor", "killsilent") => {
            if let Some(r) = me {
                let killer = form_arg(args, 0);
                e.kill(r, func == "killessential", killer);
            }
            none()
        }
        // The merchant's barter menu; a teammate's inventory to trade with.
        ("actor", "showbartermenu") => {
            if let Some(r) = me {
                e.show_barter_menu(r);
            }
            none()
        }
        // (abGivingGift, apFilterList, abShowStolenItems, abUseFavorPoints)
        ("actor", "showgiftmenu") => match me {
            Some(r) => {
                let filter = args.get(1).and_then(|a| a.as_form()).map(FormId);
                let stolen = args.get(2).is_some_and(|a| a.as_bool());
                let favor = args.get(3).is_none_or(|a| a.as_bool());
                e.show_gift_menu(r, arg(0).as_bool(), filter, stolen, favor)
            }
            None => v(Value::Int(0)),
        },
        ("actor", "openinventory") => {
            if let Some(r) = me {
                e.menu = Some(crate::items::Menu::Container(r));
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
                .map_or(-1, |(r, f)| e.faction_rank(r, f)),
        )),
        // In a faction, an actor keeps the rank it has; out of it, it joins at 0.
        ("actor", "addtofaction") => {
            if let Some((r, f)) = me.zip(form_arg(args, 0))
                && e.faction_rank(r, f) < 0
            {
                e.set_faction_rank(r, f, 0);
            }
            none()
        }
        ("actor", "removefromfaction") => {
            if let Some((r, f)) = me.zip(form_arg(args, 0)) {
                e.set_faction_rank(r, f, -1);
            }
            none()
        }
        ("actor", "removefromallfactions") => {
            if let Some(r) = me {
                for (f, _) in e.npc_factions(r) {
                    e.set_faction_rank(r, f, -1);
                }
            }
            none()
        }
        // Setting a rank puts the actor in the faction; changing one it isn't
        // in is lost.
        ("actor", "setfactionrank") => {
            if let Some((r, f)) = me.zip(form_arg(args, 0)) {
                e.set_faction_rank(r, f, arg(1).as_int());
            }
            none()
        }
        ("actor", "modfactionrank") => {
            if let Some((r, f)) = me.zip(form_arg(args, 0)) {
                let rank = e.faction_rank(r, f);
                if rank >= 0 {
                    e.set_faction_rank(r, f, (rank + arg(1).as_int()).max(0));
                }
            }
            none()
        }
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
        ("actor", "enableai") => {
            if let Some(a) = me.filter(|a| !a.is_null()) {
                if args.first().is_none_or(|b| b.as_bool()) {
                    e.ai_off.remove(&a);
                } else {
                    e.ai_off.insert(a);
                }
            }
            none()
        }
        ("actor", "setoutfit") => {
            if let (Some(a), Some(o)) = (me, form_arg(args, 0)) {
                e.set_outfit(a, o, arg(1).as_bool());
            }
            none()
        }
        ("actor", "stopcombatalarm") => {
            if let Some(a) = me {
                e.stop_combat_alarm(a);
            }
            none()
        }
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
        ("actor", "getlevel") => v(Value::Int(me.map_or(1, |a| e.actor_level(a)))),
        // Magic (`crate::magic`).
        ("actor", "addspell") => v(Value::Bool(match (me, form_arg(args, 0)) {
            (Some(a), Some(s)) => {
                let added = e.add_spell(a, s);
                // abVerbose (default true): "<spell> added" for the player.
                if added
                    && a == crate::engine::PLAYER_REF
                    && args.get(1).is_none_or(|x| x.as_bool())
                {
                    let name = e.magic_item(s).map(|m| m.name.clone()).unwrap_or_default();
                    let fmt = e
                        .gmst_string("sSpellAdded")
                        .unwrap_or_else(|| "Spell added".into());
                    e.scripts.notify(format!("{fmt}: {name}"));
                }
                added
            }
            _ => false,
        })),
        ("actor", "removespell") => v(Value::Bool(match (me, form_arg(args, 0)) {
            (Some(a), Some(s)) => e.remove_spell(a, s),
            _ => false,
        })),
        ("actor", "hasspell") => v(Value::Bool(match (me, form_arg(args, 0)) {
            (Some(a), Some(s)) => e.has_spell(a, s),
            _ => false,
        })),
        ("actor", "dispelspell") => v(Value::Bool(match (me, form_arg(args, 0)) {
            (Some(a), Some(s)) => {
                let had = e.is_spell_target(a, s);
                e.dispel(a, |x| x.item == s);
                had
            }
            _ => false,
        })),
        ("actor", "dispelallspells") => {
            // Not abilities or diseases: what was cast on the actor.
            if let Some(a) = me {
                let lasting: Vec<FormId> = e
                    .magic
                    .effects
                    .iter()
                    .filter(|x| x.target == a && x.duration.is_finite())
                    .map(|x| x.item)
                    .collect();
                e.dispel(a, |x| lasting.contains(&x.item));
            }
            none()
        }
        ("actor", "hasmagiceffect") => v(Value::Bool(match (me, form_arg(args, 0)) {
            (Some(a), Some(m)) => e.has_magic_effect(a, m),
            _ => false,
        })),
        ("actor", "hasmagiceffectwithkeyword") => v(Value::Bool(match (me, form_arg(args, 0)) {
            (Some(a), Some(k)) => e.has_magic_effect_keyword(a, k),
            _ => false,
        })),
        ("actor", "docombatspellapply") => {
            if let (Some(a), Some(s), Some(t)) = (me, form_arg(args, 0), form_arg(args, 1)) {
                e.apply_item(s, Some(a), t);
            }
            none()
        }
        // Shouts (`crate::shouts`).
        ("actor", "addshout") => v(Value::Bool(match (me, form_arg(args, 0)) {
            (Some(a), Some(s)) => e.add_shout(a, s),
            _ => false,
        })),
        ("actor", "removeshout") => v(Value::Bool(match (me, form_arg(args, 0)) {
            (Some(a), Some(s)) => e.remove_shout(a, s),
            _ => false,
        })),
        ("actor", "equipshout") => {
            if let (Some(a), Some(s)) = (me, form_arg(args, 0)) {
                e.equip_shout(a, s);
            }
            none()
        }
        ("actor", "unequipshout") => {
            if let (Some(a), Some(s)) = (me, form_arg(args, 0)) {
                e.unequip_spell(a, s, Some(crate::spells::VOICE));
            }
            none()
        }
        ("actor", "getequippedshout") => {
            let shout = me.and_then(|a| e.equipped_shout(a));
            v(shout.map_or(Value::None, |s| e.object_value(s)))
        }
        ("game", "teachword") => {
            if let Some(w) = form_arg(args, 0) {
                e.teach_word(w);
            }
            none()
        }
        ("game", "unlockword") => {
            if let Some(w) = form_arg(args, 0) {
                e.unlock_word(w);
            }
            none()
        }
        ("game", "iswordunlocked") => v(Value::Bool(
            form_arg(args, 0).is_some_and(|w| e.is_word_unlocked(w)),
        )),
        // aiSource: 0 the left hand, 1 the right, 2 a power or shout.
        ("actor", "equipspell") | ("actor", "unequipspell") | ("actor", "getequippedspell") => {
            let source = |i: usize| match arg(i).as_int() {
                0 => crate::spells::LEFT,
                1 => crate::spells::RIGHT,
                _ => crate::spells::VOICE,
            };
            let Some(actor) = me else { return none() };
            match func {
                "getequippedspell" => {
                    // A scroll in the hand is an item, not a spell.
                    let spell = e
                        .equipped_spell(actor, source(0))
                        .filter(|&s| e.lo.tag_of(s).is_some_and(|t| t.0 != *b"SCRL"));
                    return v(spell.map_or(Value::None, |s| e.object_value(s)));
                }
                "equipspell" => {
                    if let Some(spell) = form_arg(args, 0) {
                        e.equip_spell(actor, spell, source(1));
                    }
                }
                _ => {
                    if let Some(spell) = form_arg(args, 0) {
                        e.unequip_spell(actor, spell, Some(source(1)));
                    }
                }
            }
            none()
        }
        ("actor", "equipitem") | ("actor", "unequipitem") => {
            if let (Some(a), Some(item)) = (me, form_arg(args, 0)) {
                let on = func == "equipitem";
                let r = if on && e.magic_item(item).is_some() {
                    e.consume(a, item)
                } else {
                    e.equip_item(a, item, on)
                };
                if let Err(err) = r {
                    log::debug!("{func} {item} on {a}: {err}");
                }
            }
            none()
        }
        // Spell.Cast(akSource, akTarget): lands on the target, else the caster.
        // RemoteCast(akSource, akBlameActor, akTarget).
        ("spell", "cast")
        | ("spell", "remotecast")
        | ("scroll", "cast")
        | ("enchantment", "cast") => {
            let source = form_arg(args, 0);
            let (blame, target) = if func == "remotecast" {
                (form_arg(args, 1).or(source), form_arg(args, 2))
            } else {
                (source, form_arg(args, 1))
            };
            if let (Some(s), Some(t)) = (me, target.or(source)) {
                e.cast_item(s, blame, t);
            }
            none()
        }
        ("spell", "ishostile") | ("potion", "ishostile") | ("enchantment", "ishostile") => v(
            Value::Bool(me.and_then(|s| e.magic_item(s)).is_some_and(|m| {
                m.effects.iter().any(|x| {
                    e.magic_effect(x.effect)
                        .is_some_and(|f| f.has(crate::magic::flags::HOSTILE))
                })
            })),
        ),
        ("magiceffect", "getassociatedskill") => v(Value::str(
            me.and_then(|m| e.magic_effect(m))
                .and_then(|m| m.skill)
                .and_then(esp::actor_value::name)
                .unwrap_or(""),
        )),
        ("activemagiceffect", "gettargetactor") | ("activemagiceffect", "getcasteractor") => {
            let x = effect_of(this).and_then(|id| e.active_effect(id));
            let who = x.and_then(|x| {
                if func == "gettargetactor" {
                    Some(x.target)
                } else {
                    x.caster
                }
            });
            v(who.map_or(Value::None, |w| e.object_value(w)))
        }
        ("activemagiceffect", "getbaseobject") => v(effect_of(this)
            .and_then(|id| e.active_effect(id))
            .map_or(Value::None, |x| e.object_value(x.effect))),
        ("activemagiceffect", "dispel") => {
            if let Some(id) = effect_of(this) {
                e.dispel_effect(id);
            }
            none()
        }
        // Perks (`crate::perks`).
        ("actor", "addperk") => {
            if let (Some(a), Some(p)) = (me, form_arg(args, 0)) {
                e.add_perk(a, p);
            }
            none()
        }
        ("actor", "removeperk") => {
            if let (Some(a), Some(p)) = (me, form_arg(args, 0)) {
                e.remove_perk(a, p);
            }
            none()
        }
        ("actor", "hasperk") => v(Value::Bool(match (me, form_arg(args, 0)) {
            (Some(a), Some(p)) => e.has_perk(a, p),
            _ => false,
        })),
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
        // Factions' reactions (`crate::factions`).
        ("faction", "setenemy") | ("faction", "setally") => {
            if let (Some(a), Some(b)) = (me, form_arg(args, 0)) {
                let (x, y) = (arg(1).as_bool(), arg(2).as_bool());
                if func == "setenemy" {
                    e.set_factions_enemies(a, b, x, y);
                } else {
                    e.set_factions_allies(a, b, x, y);
                }
            }
            none()
        }
        ("faction", "setreaction") | ("faction", "modreaction") => {
            if let (Some(a), Some(b)) = (me, form_arg(args, 0)) {
                e.set_reaction_modifier(a, b, arg(1).as_int(), func == "modreaction");
            }
            none()
        }
        ("faction", "getreaction") => v(Value::Int(match (me, form_arg(args, 0)) {
            (Some(a), Some(b)) => e.reaction_towards(a, b).modifier,
            _ => 0,
        })),
        ("faction", "setplayerexpelled") | ("faction", "setplayerenemy") => {
            if let Some(f) = me {
                let flag = if func == "setplayerenemy" {
                    crate::factions::PLAYER_ENEMY
                } else {
                    crate::factions::PLAYER_EXPELLED
                };
                e.set_faction_player_flag(f, flag, args.first().is_none_or(|b| b.as_bool()));
            }
            none()
        }
        ("actor", "setattackactoronsight") => {
            if let Some(a) = me {
                e.set_attacked_on_sight(a, args.first().is_none_or(|b| b.as_bool()));
            }
            none()
        }
        ("faction", "getstolenitemvaluecrime") | ("faction", "getstolenitemvaluenocrime") => {
            v(Value::Int(me.map_or(0, |f| {
                e.stolen_item_value(f, func == "getstolenitemvaluecrime")
            })))
        }
        ("faction", "isfactionincrimegroup") => v(Value::Bool(match (me, form_arg(args, 0)) {
            (Some(a), Some(b)) => e.faction_in_crime_group(a, b),
            _ => false,
        })),
        ("faction", "isplayerexpelled") => {
            v(Value::Bool(me.is_some_and(|f| e.player_expelled_from(f))))
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
        ("location", "getreftypealivecount") => v(Value::Int(
            me.zip(form_arg(args, 0))
                .map_or(0, |(l, ty)| e.location_population(l, ty, true) as i32),
        )),
        ("location", "iscleared") => v(Value::Bool(me.is_some_and(|l| e.is_cleared(l)))),
        ("location", "setcleared") => {
            if let Some(l) = me {
                e.set_cleared(l, args.first().is_none_or(|a| a.as_bool()));
            }
            none()
        }
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
        ("actor", "drawweapon") => {
            if let Some(r) = me {
                e.draw_weapon(r, true);
            }
            none()
        }
        ("actor", "isplayerteammate") => {
            v(Value::Bool(me.is_some_and(|r| e.is_player_teammate(r))))
        }
        ("actor", "setplayerteammate") => {
            if let Some(r) = me {
                let on = args.first().is_none_or(Value::as_bool);
                let favor = args.get(1).is_none_or(Value::as_bool);
                e.set_player_teammate(r, on, favor);
            }
            none()
        }
        ("actor", "isonmount") => v(Value::Bool(false)),
        ("actor", "issneaking") => v(Value::Bool(me.is_some_and(|r| e.is_sneaking(r)))),
        ("actor", "isdetectedby") => v(Value::Bool(
            me.zip(form_arg(args, 0))
                .is_some_and(|(r, by)| e.detects(by, r)),
        )),
        ("actor", "getlightlevel") => v(Value::Float(me.map_or(0.0, |r| e.light_level(r)))),
        ("actor", "isalerted") => v(Value::Bool(me.is_some_and(|r| e.is_alerted(r)))),
        ("actor", "setalert") => {
            if let Some(r) = me {
                e.set_alerted(r, args.first().is_none_or(|a| a.as_bool()));
            }
            none()
        }
        ("actor", "evaluatepackage") | ("actor", "setrestrained") | ("actor", "setdontmove") => {
            none()
        }
        ("actor", "getsitstate") | ("actor", "getsleepstate") => v(Value::Int(0)),
        ("actorbase", "getclass") => {
            let class = me.and_then(|b| e.templates_of(b)).and_then(|src| {
                let r = e.lo.get(src.of(crate::world::template::STATS))?;
                let d = r.get(b"CNAM")?.get(..4)?;
                Some(r.fid(FormId(u32::from_le_bytes(d.try_into().unwrap()))))
            });
            v(class.map_or(Value::None, |c| e.object_value(c)))
        }
        ("actorbase", "getsex") => v(Value::Int(
            me.map(|f| e.npc_is_female(f) as i32).unwrap_or(0),
        )),
        ("actorbase", "isunique") => v(Value::Bool(me.is_some_and(|b| e.base_has_flag(b, 0x20)))),
        ("actorbase", "setessential")
        | ("actorbase", "setprotected")
        | ("actorbase", "setinvulnerable") => {
            if let Some(base) = me {
                let flag = match func {
                    "setessential" => crate::protection::acbs::ESSENTIAL,
                    "setprotected" => crate::protection::acbs::PROTECTED,
                    _ => crate::protection::acbs::INVULNERABLE,
                };
                e.set_base_flag(base, flag, args.first().is_none_or(|b| b.as_bool()));
            }
            none()
        }
        ("actorbase", "isessential")
        | ("actorbase", "isprotected")
        | ("actorbase", "isinvulnerable") => {
            let flag = match func {
                "isessential" => crate::protection::acbs::ESSENTIAL,
                "isprotected" => crate::protection::acbs::PROTECTED,
                _ => crate::protection::acbs::INVULNERABLE,
            };
            v(Value::Bool(me.is_some_and(|b| e.base_has_flag(b, flag))))
        }
        // The race of the actor, or of the base, through its templates.
        ("actor", "getrace") | ("actorbase", "getrace") => {
            match me.and_then(|a| e.race_of(a).or_else(|| e.npc_race(a))) {
                Some(r) => v(e.object_value(r)),
                None => none(),
            }
        }
        ("actor", "setghost") => {
            if let Some(a) = me {
                e.set_ghost(a, args.first().is_none_or(|b| b.as_bool()));
            }
            none()
        }
        ("actor", "isghost") => v(Value::Bool(me.is_some_and(|a| e.is_ghost(a)))),
        ("actor", "isessential") => v(Value::Bool(me.is_some_and(|a| e.is_essential(a)))),
        // The NPC's gift filter (`GNAM`, a form list of items).
        ("actorbase", "getgiftfilter") => {
            let list = me.and_then(|f| e.lo.get(f)).and_then(|r| {
                r.get(b"GNAM")
                    .filter(|d| d.len() >= 4)
                    .map(|d| r.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
                    .filter(|f| !f.is_null())
            });
            v(list.map_or(Value::None, |f| e.object_value(f)))
        }
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
        ("quest", "isrunning") => v(Value::Bool(
            me.and_then(|q| e.scripts.quests.get(&q))
                .is_some_and(|s| s.running),
        )),
        // Tracked in the journal.
        ("quest", "isactive") => v(Value::Bool(
            me.and_then(|q| e.scripts.quests.get(&q))
                .is_some_and(|s| s.active),
        )),
        ("quest", "setactive") => {
            if let Some(q) = me {
                e.set_quest_active(q, args.first().is_none_or(|a| a.as_bool()));
            }
            none()
        }
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
                e.complete_quest(q);
            }
            none()
        }
        // Missing arguments take Papyrus's defaults (abDisplayed / abCompleted /
        // abFailed true, abForce false).
        ("quest", "setobjectivedisplayed") => {
            if let Some(q) = me {
                let shown = args.get(1).is_none_or(|a| a.as_bool());
                let force = args.get(2).is_some_and(|a| a.as_bool());
                e.set_objective_displayed(q, arg(0).as_int(), shown, force);
            }
            none()
        }
        ("quest", "setobjectivecompleted") => {
            if let Some(q) = me {
                let done = args.get(1).is_none_or(|a| a.as_bool());
                e.set_objective_completed(q, arg(0).as_int(), done);
            }
            none()
        }
        ("quest", "setobjectivefailed") => {
            if let Some(q) = me {
                let failed = args.get(1).is_none_or(|a| a.as_bool());
                e.set_objective_failed(q, arg(0).as_int(), failed);
            }
            none()
        }
        ("quest", "completeallobjectives") | ("quest", "failallobjectives") => {
            if let Some(q) = me {
                let shown: Vec<i32> = e
                    .scripts
                    .quests
                    .get(&q)
                    .map(|s| s.objectives_displayed.iter().copied().collect())
                    .unwrap_or_default();
                for o in shown {
                    if func == "completeallobjectives" {
                        e.set_objective_completed(q, o, true);
                    } else {
                        e.set_objective_failed(q, o, true);
                    }
                }
            }
            none()
        }
        ("quest", "isobjectivefailed") => v(Value::Bool(
            me.and_then(|q| e.scripts.quests.get(&q))
                .is_some_and(|s| s.objectives_failed.contains(&arg(0).as_int())),
        )),
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
                let quest = FormId(*quest);
                if func == "forcerefto" || e.alias_ref(quest, *alias).is_none() {
                    e.force_alias_ref(quest, *alias, r);
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
        // A message box waits for the player's button.
        ("message", "show") => match me {
            Some(m) => {
                let floats: Vec<f32> = args.iter().map(|a| a.as_float()).collect();
                e.show_message(m, &floats)
            }
            None => v(Value::Int(0)),
        },
        // (asEvent, afDuration, afInterval, aiMaxTimes)
        ("message", "showashelpmessage") => {
            if let Some(m) = me {
                e.show_help_message(
                    m,
                    &str_arg(args, 0),
                    arg(1).as_float(),
                    arg(2).as_float(),
                    arg(3).as_int(),
                );
            }
            none()
        }
        ("message", "resethelpmessage") => {
            e.reset_help_message(&str_arg(args, 0));
            none()
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
        ("formlist", "addform") => {
            if let (Some(list), Some(f)) = (me, form_arg(args, 0)) {
                e.add_to_formlist(list, f);
            }
            none()
        }
        ("formlist", "removeaddedform") => {
            if let (Some(list), Some(f)) = (me, form_arg(args, 0)) {
                e.remove_added_form(list, f);
            }
            none()
        }
        ("formlist", "revert") => {
            if let Some(list) = me {
                e.revert_formlist(list);
            }
            none()
        }
        ("formlist", "find") => v(Value::Int(match (me, form_arg(args, 0)) {
            (Some(list), Some(f)) => e
                .formlist(list)
                .iter()
                .position(|&x| x == f)
                .map_or(-1, |i| i as i32),
            _ => -1,
        })),
        ("cell", "isattached") => v(Value::Bool(true)),
        ("cell", "isinterior") => v(Value::Bool(me.and_then(|c| e.lo.get(c)).is_some_and(|r| {
            r.tag().0 == *b"CELL"
                && r.get(b"DATA")
                    .and_then(|d| d.first())
                    .is_some_and(|f| f & 1 != 0)
        }))),
        ("math", _) => math::call(func, args).map_or_else(none, v),
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
