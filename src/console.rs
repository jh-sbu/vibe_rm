//! Developer console commands (a subset of Skyrim's, plus engine-specific ones).

use glam::Vec3;

use crate::engine::Engine;

pub fn execute(engine: &mut Engine, line: &str) -> Vec<String> {
    let mut parts = line.split_whitespace();
    let Some(cmd) = parts.next() else {
        return Vec::new();
    };
    let args: Vec<&str> = parts.collect();
    let lower = cmd.to_ascii_lowercase();
    // `<ref>.command` (`player.additem f 100`) acts on that reference.
    if let Some((target, sub)) = lower
        .split_once('.')
        .filter(|(_, sub)| ITEM_COMMANDS.contains(sub))
    {
        let r = if target == "player" {
            Some(crate::engine::PLAYER_REF)
        } else {
            engine.resolve_form(target)
        };
        return match r {
            Some(r) => item_command(engine, r, sub, &args),
            None => vec![format!("unknown reference '{target}'")],
        };
    }
    if ITEM_COMMANDS.contains(&lower.as_str()) {
        return item_command(engine, crate::engine::PLAYER_REF, &lower, &args);
    }
    match lower.as_str() {
        "help" => vec![
            "coc <cell>            center on cell (editor id)".into(),
            "tcl                   toggle collision (noclip)".into(),
            "fw / sw <weather>     force / set weather (overriding; rwo releases)".into(),
            "weather               the weathers, transition and region".into(),
            "thunder               roll thunder and lightning now (while there is thunder)".into(),
            "set gamehour to <h>   set time of day".into(),
            "player.setpos x y z   teleport within the current location".into(),
            "getpos                print position".into(),
            "tdt                   toggle debug text".into(),
            "tai                   toggle actor AI".into(),
            "tsh                   toggle sun shadows".into(),
            "sae <ref> <event>     send a behaviour event to an actor".into(),
            "pi <ref> <idle>       play an IDLE record on an actor".into(),
            "lock <ref> [level]    lock a door or container; unlock <ref> unlocks it".into(),
            "picklock <deg> [secs] while picking: pick at an angle, turn the lock a while".into(),
            "templates <ref>       which NPC record each part of an actor comes from".into(),
            "use <ref> <furniture> send an actor to use furniture or an idle marker".into(),
            "escort <ref> <target> <dest> [wait] [run]  lead target to dest, waiting while it lags".into(),
            "gstate <ref>          an actor's active behaviour graph states".into(),
            "bones <ref> [filter]  an actor's posed and bind bone positions (model space)".into(),
            "sgv <ref> <var> <x>   set a behaviour graph variable".into(),
            "door <ref>            open / close a door".into(),
            "[ref.]additem <item> [n] / removeitem <item> [n] / showinventory".into(),
            "[ref.]moveto <target>  move a reference (or the player) to another".into(),
            "<ref>.enable / disable  enable or disable a reference (and those enabled with it)".into(),
            "[ref.]drop <item> [n]  drop items in front of the player (or an actor)".into(),
            "[ref.]placeatme <form> [n]  make new references there (actors join the world)".into(),
            "player.equipitem / unequipitem <item>   wear armor, wield a weapon, ready ammo".into(),
            "activate <ref>        activate a reference as the player".into(),
            "sqs <quest>           a quest's stages, log entries and fragments".into(),
            "sqo / journal [open]  displayed objectives / the journal's quests (or open it)".into(),
            "msgbox [button]       the message box up / press one of its buttons".into(),
            "menumode / closemenu  whether the game is paused by a menu / close the menu".into(),
            "imod <imad> [strength] / rimod <imad> / imods  apply / remove an image space modifier, list them".into(),
            "fadeout <0|1> [secs] / shake [strength] [secs]  fade the game out (1) or in (0) / shake the camera".into(),
            "inputevent <event>    as if the player did an input event (Activate, Jump...)".into(),
            "[ref.]getav <av> / setav, modav, forceav, damageav, restoreav <av> <n>   actor values".into(),
            "pblock                toggle the player's guard (right mouse button)".into(),
            "pattack [power]       the player swings (power: as holding the button)".into(),
            "pbash                 the player bashes (attacking with the guard up)".into(),
            "pshoot [secs]         the player looses an arrow drawn so long (default full)".into(),
            "psneak / pjump        the player sneaks (toggle) / jumps when next on the ground".into(),
            "pdraw [off]           the player draws (or sheathes) their weapon".into(),
            "ppress / prelease     the player's attack button goes down / up".into(),
            "pwalk [frames] [forward|back|left|right...] [run|sprint]  the player walks (default forward, 60 frames; tcl first under --wait)".into(),
            "tcam x y z yaw pitch  hold the --wait camera there (degrees; yaw 0 = north)".into(),
            "stamina <ref|player> [n]  show or set stamina".into(),
            "probe [x y]           collision under the player (or x y) and per-cell colliders".into(),
            "screenshot <png>      save the 3D view (no HUD) at the window size".into(),
            "startquest / stopquest <quest>, setstage <quest> <stage>   quests".into(),
            "startscene <scene> [force] / stopscene <scene> / scenes   start, stop, list scenes".into(),
            "storyevent <TYPE> [r1] [r2] [l1] [l2] [v1=n f1=form...]   send a Story Manager event (ADIA, CLOC, AIPL...)".into(),
            "[ref.]getrelationshiprank <actor> / setrelationshiprank <actor> <rank>   relationship ranks (-4..4)".into(),
            "crime                 the player's bounties; player.setcrimegold <n> [faction] [violent]".into(),
            "player.paycrimegold <remove stolen 0/1> <jail 0/1> [faction]   pay off a bounty (the hold here by default)".into(),
            "crimefaction <ref> [faction]   show or set an actor's crime faction".into(),
            "alarm [faction] / jail [faction] / servetime   send the faction's guards to arrest the player, jail them, serve the sentence".into(),
            "pickpocket <ref> <item> [n]   try to take an item from a sneaking player's victim (psneak first)".into(),
            "cgf <Class.Func> [@self] [args]  call a Papyrus native (cgf Actor.GetCombatState @<ref>)".into(),
            "loose [n]             the loose objects nearest the player (position, mass, motion)".into(),
            "sv <ref> [var]        a reference's script variables".into(),
            "grab [ref | off]      grab what the crosshair (or a ref) is on, say what is held, let go".into(),
            "epc                   enable all player controls (EnablePlayerControls)".into(),
            "detect                who detects the player, by how much; the player's light level and stealth points".into(),
            "skills [open [skill]]  the player's level, XP and each skill's progress (open: the menu, with a perk tree)".into(),
            "player.advskill <skill> <xp> / player.incpcs <skill>   skill XP / a skill level (OneHanded, Marksman, Sneak...)".into(),
            "player.advlevel / levelup <health|magicka|stamina>   earn a level up / take one".into(),
            "[ref.]getlevel        an actor's level".into(),
            "[ref.]perks [all]     an actor's perks (all: with their entry points)".into(),
            "[ref.]effects / dispelallspells   magic effects on an actor / end those cast on it".into(),
            "[ref.]addspell / removespell <spell>   give or take a spell (abilities take effect)".into(),
            "[ref.]cast <spell> [target | player]   the ref's spell lands on the target (else itself)".into(),
            "player.equipitem <potion>   drink a potion, eat food or an ingredient".into(),
            "[ref.]addperk / removeperk / hasperk <perk>   give, take or check a perk".into(),
            "[ref.]perkep <entry point> <value> [subject...]   a value through the ref's perks (tabs 1, 2...)".into(),
            "perkpoints [n]        the player's perk points (or set them)".into(),
        ],
        "detect" => engine.describe_detection(),
        "skills" if args.first() == Some(&"open") => {
            // skills open [skill]: the skills menu, with that skill's perk tree.
            engine.perks.tree = args.get(1).and_then(|s| crate::skills::skill_index(s));
            engine.menu = Some(crate::items::Menu::Skills);
            vec![]
        }
        "skills" => engine.describe_skills(),
        "perkpoints" => {
            if let Some(n) = args.first().and_then(|a| a.parse::<u32>().ok()) {
                engine.skills.perk_points = n;
            }
            vec![format!("perk points: {}", engine.skills.perk_points)]
        }
        "levelup" => {
            let attr = match args.first().map(|a| a.to_ascii_lowercase()) {
                Some(a) if a == "health" => esp::actor_value::HEALTH,
                Some(a) if a == "magicka" => esp::actor_value::MAGICKA,
                Some(a) if a == "stamina" => esp::actor_value::STAMINA,
                _ => return vec!["usage: levelup <health|magicka|stamina>".into()],
            };
            if !engine.take_level_up(attr) {
                return vec!["no level up to take".into()];
            }
            engine.describe_skills()
        }
        "loose" => engine.describe_loose(args.first().and_then(|a| a.parse().ok()).unwrap_or(8)),
        "epc" | "enableplayercontrols" => {
            engine.disabled_controls = Default::default();
            vec![engine.disabled_controls.describe()]
        }
        "cgf" => {
            let Some((class, func)) = args.first().and_then(|f| f.split_once('.')) else { return vec!["usage: cgf <Class.Func> [@self] [args]".into()] };
            let form = |s: &str| if s.eq_ignore_ascii_case("player") { Some(crate::engine::PLAYER_REF) } else { engine.resolve_form(s) };
            let mut this = None;
            let mut values = Vec::new();
            for a in &args[1..] {
                if let Some(r) = a.strip_prefix('@') {
                    let Some(f) = form(r) else { return vec![format!("unknown reference '{r}'")] };
                    this = Some(engine.object_value(f));
                } else if let Some(f) = form(a).filter(|_| a.len() == 8 || a.eq_ignore_ascii_case("player")) {
                    values.push(engine.object_value(f));
                } else if let Ok(i) = a.parse::<i32>() {
                    values.push(papyrus::Value::Int(i));
                } else if let Ok(x) = a.parse::<f32>() {
                    values.push(papyrus::Value::Float(x));
                } else if let Some(f) = form(a) {
                    values.push(engine.object_value(f));
                } else {
                    values.push(papyrus::Value::str(a));
                }
            }
            let (class, func) = (class.to_ascii_lowercase(), func.to_ascii_lowercase());
            match crate::script::natives::call(engine, &class, &func, this.as_ref(), &values) {
                papyrus::NativeResult::Value(v) => vec![format!("{} >> {v}", args[0])],
                _ => vec![format!("{} waits; nothing to return", args[0])],
            }
        }
        "crime" => engine.describe_bounties(),
        "alarm" | "jail" => {
            let Some(f) = args.first().and_then(|a| engine.resolve_form(a)).or_else(|| engine.location_crime_faction()) else { return vec!["no crime faction here; name one".into()] };
            if lower == "alarm" {
                engine.raise_alarm(f);
            } else {
                engine.send_player_to_jail(f, true);
            }
            engine.describe_bounties()
        }
        "servetime" => {
            engine.serve_sentence();
            engine.describe_bounties()
        }
        "pickpocket" => {
            let (Some(r), Some(item)) = (args.first().and_then(|a| engine.resolve_form(a)), args.get(1).and_then(|a| engine.resolve_form(a))) else {
                return vec!["usage: pickpocket <ref> <item> [n]".into()];
            };
            let n = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(1);
            if !engine.can_pickpocket(r) {
                return vec![format!("can't pick {r}'s pocket (sneaking {})", engine.player.sneaking)];
            }
            let chance = engine.pickpocket_chance(r, item, n);
            let took = engine.try_pickpocket(r, item, n, None);
            vec![format!("{r}: {n} x {item} at {chance:.0}%: {}", if took { "taken" } else { "caught" })]
        }
        "crimefaction" => {
            let Some(r) = args.first().and_then(|a| engine.resolve_form(a)) else { return vec!["usage: crimefaction <ref> [faction]".into()] };
            if let Some(f) = args.get(1) {
                engine.set_crime_faction(r, engine.resolve_form(f));
            }
            vec![format!("{r}: crime faction {:?}", engine.crime_faction(r).map(|f| format!("{} {f}", engine.form_name(f))))]
        }
        "storyevent" => {
            let Some(code) = args.first().and_then(|c| <[u8; 4]>::try_from(c.to_ascii_uppercase().as_bytes()).ok()) else {
                return vec!["usage: storyevent <TYPE> [r1] [r2] [l1] [l2] [v1=n v2=n f1=form k1=keyword]".into()];
            };
            let mut ev = crate::story::StoryEvent::new(&code);
            // Positional r1 r2 l1 l2; then member=value (v1=2, f1=<form>, k1=<keyword>...).
            let (named, positional): (Vec<&str>, Vec<&str>) = args[1..].iter().copied().partition(|a| a.contains('='));
            let f = |i: usize| positional.get(i).and_then(|a| engine.resolve_form(a)).unwrap_or_default();
            ev.refs = [f(0), f(1)];
            ev.locs = [f(2), f(3)];
            for a in named {
                let (m, x) = a.split_once('=').unwrap();
                let form = || engine.resolve_form(x).unwrap_or_default();
                match m.to_ascii_lowercase().as_str() {
                    "r1" => ev.refs[0] = form(),
                    "r2" => ev.refs[1] = form(),
                    "l1" => ev.locs[0] = form(),
                    "l2" => ev.locs[1] = form(),
                    "k1" => ev.keyword = form(),
                    "f1" => ev.form = form(),
                    "v1" => ev.values[0] = x.parse().unwrap_or(0.0),
                    "v2" => ev.values[1] = x.parse().unwrap_or(0.0),
                    _ => return vec![format!("unknown member {m}")],
                }
            }
            if engine.send_story_event(ev) { vec!["a quest started".into()] } else { vec!["nothing started".into()] }
        }
        "startquest" | "stopquest" => {
            let Some(q) = args.first().and_then(|q| engine.resolve_form(q)) else { return vec![format!("usage: {cmd} <quest>")] };
            if cmd == "stopquest" {
                engine.stop_quest(q);
                vec![format!("{q} stopped")]
            } else if engine.start_quest(q) {
                vec![format!("{q} started")]
            } else {
                vec![format!("{q} didn't start (running, or an alias can't be filled)")]
            }
        }
        "sqs" => {
            let Some(q) = args.first().and_then(|q| engine.resolve_form(q)) else {
                return vec!["usage: sqs <quest>".into()];
            };
            sqs(engine, q)
        }
        "sqo" => {
            let mut out = Vec::new();
            let mut quests: Vec<_> = engine.scripts.quests.iter().filter(|(_, st)| st.running).collect();
            quests.sort_by_key(|(q, _)| q.0);
            for (q, st) in quests {
                let mut objs: Vec<_> = st.objectives_displayed.iter().copied().collect();
                objs.sort();
                for o in objs {
                    let state = if st.objectives_completed.contains(&o) { " (completed)" } else { "" };
                    let text = st.objective_texts.get(&o).cloned().unwrap_or_default();
                    out.push(format!("{q} {o}: {text}{state}"));
                }
            }
            if out.is_empty() { vec!["no objectives displayed".into()] } else { out }
        }
        "journal" => {
            if args.first() == Some(&"open") {
                engine.menu = Some(crate::items::Menu::Journal);
                return vec![];
            }
            let mut out = Vec::new();
            for j in engine.journal() {
                let state = if j.failed { " (failed)" } else if j.completed { " (completed)" } else { "" };
                out.push(format!("{} {:?} type {}{state}", j.id, j.name, j.kind));
                if let Some(l) = j.log.last() {
                    out.push(format!("  {l}"));
                }
                for (i, text, st) in &j.objectives {
                    out.push(format!("  [{i}] {text} {st:?}"));
                }
            }
            if out.is_empty() { vec!["the journal is empty".into()] } else { out }
        }
        "imod" | "rimod" => {
            let Some(m) = args.first().and_then(|a| engine.resolve_form(a)) else {
                return vec![format!("usage: {cmd} <image space modifier>")];
            };
            if lower == "imod" {
                let s = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1.0);
                engine.apply_imod(m, s);
            } else {
                engine.remove_imod(m);
            }
            engine.describe_imagespace()
        }
        "imods" => engine.describe_imagespace(),
        "fadeout" => {
            let out = args.first().is_none_or(|a| *a != "0");
            let secs = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1.0);
            engine.fade_out_game(out, true, 0.0, secs);
            vec![]
        }
        "shake" => {
            let strength = args.first().and_then(|s| s.parse().ok()).unwrap_or(0.5);
            let secs = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            engine.shake_camera(None, strength, secs);
            vec![]
        }
        "menumode" => vec![format!(
            "menu mode {} (menu {:?}, {} message boxes), hour {:.3}, real time {:.2}s, menus included {:.2}s",
            engine.in_menu_mode(),
            engine.menu,
            engine.messages.boxes.len(),
            engine.hour,
            engine.scripts.real_time,
            engine.scripts.wall_time
        )],
        "closemenu" => {
            let menu = engine.menu.take();
            vec![format!("closed {menu:?}")]
        }
        "msgbox" => {
            let Some(b) = engine.messages.boxes.front().cloned() else {
                return vec!["no message box up".into()];
            };
            match args.first().and_then(|a| a.parse::<i32>().ok()) {
                Some(i) if b.buttons.iter().any(|(n, _)| *n == i) => {
                    engine.choose_message_button(i);
                    vec![format!("pressed {i}")]
                }
                Some(i) => vec![format!("no button {i}")],
                None => {
                    let mut out = vec![format!("{:?}", b.text)];
                    out.extend(b.buttons.iter().map(|(i, t)| format!("  {i}: {t}")));
                    out
                }
            }
        }
        "inputevent" => {
            engine.input_event(&args.join(" "));
            vec![]
        }
        "setstage" => {
            let (Some(q), Some(st)) = (args.first().and_then(|q| engine.resolve_form(q)), args.get(1).and_then(|s| s.parse::<u16>().ok())) else {
                return vec!["usage: setstage <quest> <stage>".into()];
            };
            engine.scripts.pending_stages.push((q, st));
            vec![format!("{q} stage {st}")]
        }
        "startscene" => {
            let Some(s) = args.first().and_then(|s| engine.resolve_form(s)) else { return vec!["usage: startscene <scene> [force]".into()] };
            if engine.start_scene(s, args.get(1) == Some(&"force")) { vec![format!("{s} playing")] } else { vec![format!("{s} didn't start (see the log)")] }
        }
        "stopscene" => {
            let Some(s) = args.first().and_then(|s| engine.resolve_form(s)) else { return vec!["usage: stopscene <scene>".into()] };
            engine.stop_scene(s);
            vec![format!("{s} stops")]
        }
        "scenes" => {
            let v = engine.describe_scenes();
            if v.is_empty() { vec!["no scenes playing".into()] } else { v }
        }
        "bark" => {
            let [r, sub] = args[..] else { return vec!["usage: bark <actor ref> <subtype, e.g. HELO / IDLE>".into()] };
            let (Some(actor), Ok(sub)) = (engine.resolve_form(r), <[u8; 4]>::try_from(sub.to_ascii_uppercase().as_bytes())) else { return vec!["bad reference or subtype".into()] };
            if engine.bark(actor, &sub) { vec![format!("{r} says something")] } else { vec![format!("{r} has nothing to say")] }
        }
        "activate" => {
            let Some(r) = args.first().and_then(|r| engine.resolve_form(r)) else { return vec!["usage: activate <ref>".into()] };
            let name = engine.form_name(r);
            engine.look_target = Some((r, name));
            match engine.activate() {
                Ok(()) => vec![format!("activated {r}")],
                Err(e) => vec![format!("error: {e:#}")],
            }
        }
        "lock" | "unlock" => {
            let Some(r) = args.first().and_then(|r| engine.resolve_form(r)) else { return vec![format!("usage: {lower} <ref> [level]")] };
            if let Some(level) = args.get(1).and_then(|l| l.parse::<u8>().ok()) {
                engine.scripts.lock_levels.insert(r, level);
            }
            engine.set_locked(r, lower == "lock");
            let lock = engine.lock_of(r);
            vec![format!("{r}: locked {}, level {:?}, key {:?}", engine.is_locked(r), lock.map(|l| l.level), lock.and_then(|l| l.key))]
        }
        "picklock" => {
            let Some(lp) = engine.lockpick.as_mut() else { return vec!["not picking a lock".into()] };
            let Some(angle) = args.first().and_then(|a| a.parse::<f32>().ok()) else {
                return vec![format!("usage: picklock <deg> [secs] (sweet spot {:.1} wide at {:.1})", lp.sweet, lp.centre)];
            };
            lp.pick = angle.clamp(-90.0, 90.0);
            lp.hold = args.get(1).and_then(|a| a.parse::<f32>().ok()).unwrap_or(1.0);
            vec![format!("pick at {:.1}: the lock turns {:.0}%", lp.pick, lp.most_turn() * 100.0)]
        }
        "door" => {
            let Some(d) = args.first().and_then(|r| engine.resolve_form(r)) else { return vec!["usage: door <ref>".into()] };
            if engine.toggle_door(d, false) { vec![format!("toggled {d}")] } else { vec![format!("{d} is not an animated door")] }
        }
        "use" => {
            let [r, f] = args[..] else { return vec!["usage: use <actor ref> <furniture ref>".into()] };
            match (engine.resolve_form(r), engine.resolve_form(f)) {
                (Some(a), Some(f)) if engine.use_furniture(a, f) => vec![format!("{r} -> {f}")],
                (Some(_), Some(_)) => vec![format!("{r} can't use {f}")],
                _ => vec!["unknown reference".into()],
            }
        }
        "sae" | "sendanimevent" | "pi" | "playidle" => {
            let [r, what] = args[..] else { return vec![format!("usage: {lower} <ref> <{}>", if lower.starts_with('s') { "event" } else { "idle" })] };
            let Some(actor) = engine.resolve_form(r) else { return vec![format!("unknown reference '{r}'")] };
            let event = if lower.starts_with('s') {
                Some(what.to_owned())
            } else {
                engine.resolve_form(what).and_then(|i| engine.idle_event(i))
            };
            match event {
                Some(ev) if engine.play_animation_event(actor, &ev) => vec![format!("{r}: {ev}")],
                Some(ev) => vec![format!("{r}: nothing plays {ev}")],
                None => vec![format!("unknown idle '{what}'")],
            }
        }
        "travel" => {
            // Test hook: travel <actor> <ref> [walk|jog|run|fastwalk] [sneak]
            let usage = || vec!["usage: travel <actor ref> <target ref> [walk|jog|run|fastwalk] [sneak]".into()];
            let [r, to, rest @ ..] = &args[..] else { return usage() };
            use crate::ai::package::Gait;
            let mut gait = Gait::Walk;
            let mut sneak = false;
            for w in rest {
                match w.to_ascii_lowercase().as_str() {
                    "walk" => gait = Gait::Walk,
                    "jog" => gait = Gait::Jog,
                    "run" => gait = Gait::Run,
                    "fastwalk" => gait = Gait::FastWalk,
                    "sneak" => sneak = true,
                    _ => return usage(),
                }
            }
            let (Some(actor), Some(target)) = (engine.resolve_form(r), engine.resolve_form(to)) else { return vec!["unknown reference".into()] };
            let Some(pos) = engine.ref_position(target) else { return vec![format!("{to} isn't loaded")] };
            if engine.travel_to(actor, pos, gait, sneak) {
                vec![format!("{r} -> {to} ({gait:?}{})", if sneak { ", sneaking" } else { "" })]
            } else {
                vec![format!("{r} isn't a loaded actor")]
            }
        }
        "escort" => {
            // Test hook: escort <actor> <target> <destination ref> [wait distance] [run if behind distance]
            let [r, t, to, rest @ ..] = &args[..] else { return vec!["usage: escort <actor ref> <target ref> <destination ref> [wait distance] [run distance]".into()] };
            let wait = rest.first().and_then(|w| w.parse().ok()).unwrap_or(512.0);
            let run = rest.get(1).and_then(|w| w.parse().ok()).unwrap_or(500.0);
            let (Some(actor), Some(target), Some(dest)) = (engine.resolve_form(r), engine.resolve_form(t), engine.resolve_form(to)) else { return vec!["unknown reference".into()] };
            let Some(pos) = engine.ref_position(dest) else { return vec![format!("{to} isn't loaded")] };
            if engine.escort(actor, target, pos, wait, run) {
                vec![format!("{r} escorting {t} to {to} (waits beyond {wait:.0})")]
            } else {
                vec![format!("{r} isn't a loaded actor")]
            }
        }
        "startcombat" => {
            let [a, t] = args[..] else { return vec!["usage: startcombat <actor ref> <target ref | player>".into()] };
            let target = if t.eq_ignore_ascii_case("player") { Some(crate::engine::PLAYER_REF) } else { engine.resolve_form(t) };
            match (engine.resolve_form(a), target) {
                (Some(a), Some(t)) if engine.start_combat(a, t) => vec![format!("{a} attacks {t}")],
                _ => vec!["can't start that fight".into()],
            }
        }
        "guard" => {
            let [r, secs] = args[..] else { return vec!["usage: guard <actor ref> <seconds>".into()] };
            let (Some(actor), Ok(x)) = (engine.resolve_form(r), secs.parse::<f32>()) else { return vec!["bad reference or seconds".into()] };
            if engine.force_guard(actor, x) { vec![format!("{r} holds its guard up for {x}s")] } else { vec![format!("{r} isn't fighting")] }
        }
        "sv" | "showvars" => {
            // sv <ref> [var]: a reference's script variables (those named like the var).
            let Some(r) = args.first().and_then(|r| engine.resolve_form(r)) else { return vec!["usage: sv <ref> [var]".into()] };
            let pat = args.get(1).map(|p| p.to_ascii_lowercase());
            let rows: Vec<String> = engine
                .vm
                .vars(papyrus::ObjectId::Form(r.0))
                .into_iter()
                .filter(|(_, n, _)| pat.as_ref().is_none_or(|p| n.to_ascii_lowercase().contains(p.as_str())))
                .map(|(s, n, v)| format!("{s}.{n} = {v:?}"))
                .collect();
            if rows.is_empty() { vec![format!("{r}: no script variables{}", if pat.is_some() { " like that" } else { "" })] } else { rows }
        }
        "grab" => {
            // grab: grab what the crosshair is on (or say what is held); grab <ref>:
            // look at it and grab it; grab off: let go.
            if args.first().is_some_and(|a| a.eq_ignore_ascii_case("off")) {
                match engine.release_grab() {
                    Some(r) => vec![format!("let go of {r}")],
                    None => vec!["holding nothing".into()],
                }
            } else if let Some(r) = args.first().and_then(|r| engine.resolve_form(r)) {
                // grab <ref>: look at it first.
                if !engine.look_at_bodies(r) {
                    return vec![format!("{r} has no bodies loaded")];
                }
                match engine.start_grab() {
                    Some(g) => vec![format!("grabbed {g}"), engine.describe_grab()],
                    None => vec![format!("can't grab {r} from here: {}", engine.crosshair_on())],
                }
            } else if engine.grabbed_ref().is_some() {
                vec![engine.describe_grab()]
            } else {
                match engine.start_grab() {
                    Some(r) => vec![format!("grabbed {r}"), engine.describe_grab()],
                    None => vec![format!("nothing to grab there: {}", engine.crosshair_on())],
                }
            }
        }
        "pblock" => {
            engine.player_blocking = !engine.player_blocking;
            vec![format!("player {}", if engine.player_blocking { "blocks" } else { "lowers their guard" })]
        }
        "ppress" => {
            engine.player_attack_press();
            vec!["attack button down".into()]
        }
        "prelease" => {
            engine.player_attack_release();
            vec!["attack button up".into()]
        }
        "pdraw" => {
            let draw = !args.first().is_some_and(|a| a.eq_ignore_ascii_case("off"));
            let took = engine.player_draw_weapon(draw);
            vec![format!("player {}{}", if draw { "draws" } else { "sheathes" }, if took { "" } else { " (the graph won't)" })]
        }
        "pattack" => {
            let power = args.first().is_some_and(|a| a.eq_ignore_ascii_case("power"));
            engine.player_attack(power);
            vec![format!("player stamina {:.0}", engine.player_stamina)]
        }
        "tcam" => {
            let v: Vec<f32> = args.iter().filter_map(|a| a.parse().ok()).collect();
            let [x, y, z, yaw, pitch] = v[..] else { return vec!["usage: tcam x y z yaw pitch".into()] };
            engine.test_camera = Some((Vec3::new(x, y, z), yaw.to_radians(), pitch.to_radians()));
            vec!["ok".into()]
        }
        "impact" => {
            // impact <IPCT> [distance]: an impact on whatever lies ahead of the
            // camera (within the distance, default 512), as an arrow's would be.
            let Some(ipct) = args.first().and_then(|a| engine.resolve_form(a)) else {
                return vec!["usage: impact <impact> [distance]".into()];
            };
            let reach = args.get(1).and_then(|d| d.parse().ok()).unwrap_or(512.0);
            vec![engine.test_impact(ipct, reach)]
        }
        "pshoot" => {
            let held = args.first().and_then(|s| s.parse::<f32>().ok()).unwrap_or(5.0);
            engine.player_loose(held);
            vec![format!("player looses after {held}s")]
        }
        "psneak" => {
            engine.player.sneaking = !engine.player.sneaking;
            vec![format!("player {}", if engine.player.sneaking { "sneaks" } else { "stands up" })]
        }
        "pwalk" => {
            engine.test_walk = args.first().and_then(|a| a.parse().ok()).unwrap_or(60);
            let mut input = crate::player::MoveInput::default();
            for a in args.iter().skip(1) {
                match a.to_ascii_lowercase().as_str() {
                    "forward" => input.forward += 1.0,
                    "back" => input.forward -= 1.0,
                    "right" => input.right += 1.0,
                    "left" => input.right -= 1.0,
                    "run" => input.run = true,
                    "sprint" => input.sprint = true,
                    _ => {}
                }
            }
            if input.forward == 0.0 && input.right == 0.0 {
                input.forward = 1.0;
            }
            engine.test_walk_input = input;
            vec![format!("player walks {} frames", engine.test_walk)]
        }
        "pjump" => {
            engine.test_jump = true;
            vec!["player jumps".into()]
        }
        "pbash" => {
            engine.player_bash();
            vec![format!("player stamina {:.0}", engine.player_stamina)]
        }
        "stamina" => {
            let who = match args.first() {
                Some(r) if r.eq_ignore_ascii_case("player") => Some(crate::engine::PLAYER_REF),
                Some(r) => engine.resolve_form(r),
                None => None,
            };
            let Some(who) = who else { return vec!["usage: stamina <actor ref | player> [value]".into()] };
            if let Some(x) = args.get(1).and_then(|v| v.parse::<f32>().ok())
                && !engine.set_stamina(who, x)
            {
                return vec![format!("{who} isn't a loaded actor")];
            }
            match engine.stamina(who) {
                Some((st, max)) => vec![format!("{who}: stamina {st:.0} / {max:.0}")],
                None => vec![format!("{who} isn't a loaded actor")],
            }
        }
        "templates" => {
            // Where an actor takes each part of its definition from (TPLT chains).
            let Some(actor) = args.first().and_then(|r| engine.resolve_form(r)) else { return vec!["usage: templates <actor ref>".into()] };
            let Some(t) = engine.templates_of(actor) else { return vec![format!("{actor}: no NPC (a leveled list picked nothing?)")] };
            let name = |f: esp::FormId| format!("{f} {}", engine.lo.get(f).and_then(|r| r.editor_id()).unwrap_or_default());
            let mut out = vec![format!("chain: {}", t.chain().iter().map(|&f| name(f)).collect::<Vec<_>>().join(" -> "))];
            out.extend(t.parts().map(|(part, f)| format!("  {part:18} {}", name(f))));
            out
        }
        "cstats" => {
            let Some(actor) = args.first().and_then(|r| engine.resolve_form(r)) else { return vec!["usage: cstats <actor ref>".into()] };
            engine.combat_summary(actor)
        }
        "kill" => {
            let Some(actor) = args.first().and_then(|r| engine.resolve_form(r)) else { return vec!["usage: kill <actor ref>".into()] };
            if engine.kill(actor, false) { vec![format!("{actor} killed")] } else { vec![format!("{actor} isn't a living loaded actor")] }
        }
        "damage" => {
            let (r, amount, by) = match args[..] {
                [r, amount] => (r, amount, None),
                [r, amount, by] => (r, amount, Some(by)),
                _ => return vec!["usage: damage <actor ref | player> <health> [attacker ref | player]".into()],
            };
            let actor = if r.eq_ignore_ascii_case("player") { Some(crate::engine::PLAYER_REF) } else { engine.resolve_form(r) };
            let (Some(actor), Ok(x)) = (actor, amount.parse::<f32>()) else { return vec!["bad reference or amount".into()] };
            let attacker = match by {
                Some(b) if b.eq_ignore_ascii_case("player") => Some(crate::engine::PLAYER_REF),
                Some(b) => match engine.resolve_form(b) {
                    Some(b) => Some(b),
                    None => return vec![format!("unknown reference '{b}'")],
                },
                None => None,
            };
            engine.damage(actor, x, attacker, 0.0);
            engine.combat_summary(actor).into_iter().take(1).collect()
        }
        "sgv" => {
            let [r, var, value] = args[..] else { return vec!["usage: sgv <actor ref> <variable> <value>".into()] };
            let (Some(actor), Ok(x)) = (engine.resolve_form(r), value.parse::<f32>()) else { return vec!["bad reference or value".into()] };
            if engine.set_graph_variable(actor, var, x) { vec![format!("{r}: {var} = {x}")] } else { vec![format!("{r} has no behaviour graph")] }
        }
        "bones" => {
            let Some(actor) = args.first().and_then(|r| engine.resolve_form(r)) else { return vec!["usage: bones <actor ref> [name filter]".into()] };
            engine.describe_bones(actor, args.get(1).copied().unwrap_or(""))
        }
        "gstate" => {
            let [r] = args[..] else { return vec!["usage: gstate <actor ref>".into()] };
            let Some(actor) = engine.resolve_form(r) else { return vec![format!("unknown reference '{r}'")] };
            match engine.graph_states(actor) {
                Some(states) => vec![format!("{r}: {}", states.join(" > "))],
                None => vec![format!("{r} has no behaviour graph")],
            }
        }
        "coc" | "centeroncell" => {
            let Some(name) = args.first() else { return vec!["usage: coc <cell editor id>".into()] };
            match engine.resolve_form(name) {
                Some(id) if engine.lo.tag_of(id).map(|t| t.0) == Some(*b"CELL") => match engine.center_on_cell(id) {
                    Ok(()) => vec![format!("moved to {name}")],
                    Err(e) => vec![format!("error: {e:#}")],
                },
                _ => vec![format!("unknown cell '{name}'")],
            }
        }
        "tcl" | "togglecollision" => {
            engine.player.noclip = !engine.player.noclip;
            vec![format!("collision {}", if engine.player.noclip { "off" } else { "on" })]
        }
        "fw" | "forceweather" | "sw" | "setweather" => {
            let Some(name) = args.first() else { return vec!["usage: fw <weather>".into()] };
            let Some(w) = engine.resolve_form(name) else { return vec![format!("no weather {name}")] };
            if matches!(lower.as_str(), "fw" | "forceweather") {
                engine.force_weather(w, true);
            } else {
                engine.set_weather(w, true, false);
            }
            engine.describe_weather()
        }
        "rwo" | "releaseweatheroverride" => {
            engine.release_weather_override();
            engine.describe_weather()
        }
        "weather" => engine.describe_weather(),
        "thunder" => vec![if engine.thunder_now() { "thunder".into() } else { "no thunder in this weather now".into() }],
        "set" if args.len() >= 3 && args[0].eq_ignore_ascii_case("gamehour") => match args[2].parse::<f32>() {
            Ok(h) => {
                engine.hour = h.rem_euclid(24.0);
                vec![format!("gamehour {h}")]
            }
            Err(_) => vec!["bad number".into()],
        },
        "tsh" | "toggleshadows" => {
            engine.renderer.shadows_enabled = !engine.renderer.shadows_enabled;
            vec![format!("shadows {}", if engine.renderer.shadows_enabled { "on" } else { "off" })]
        }
        "tai" | "toggleai" => {
            engine.ai_enabled = !engine.ai_enabled;
            vec![format!("AI {}", if engine.ai_enabled { "on" } else { "off" })]
        }
        "getpos" | "player.getpos" => {
            if let Some(r) = args.first().and_then(|r| engine.resolve_form(r)) {
                let p = engine.actor_pose(r).map(|p| p.0).or_else(|| engine.ref_position(r));
                return vec![match p {
                    Some(p) => format!("{r}: {:.1} {:.1} {:.1}", p.x, p.y, p.z),
                    None => format!("{r} isn't loaded"),
                }];
            }
            let p = engine.player.position;
            vec![format!("{:.1} {:.1} {:.1} in {}", p.x, p.y, p.z, engine.location_name())]
        }
        "player.setpos" | "setpos" => {
            let v: Vec<f32> = args.iter().filter_map(|a| a.parse().ok()).collect();
            if v.len() != 3 {
                return vec!["usage: player.setpos x y z".into()];
            }
            engine.place_player(Vec3::new(v[0], v[1], v[2]), engine.camera.yaw);
            vec!["ok".into()]
        }
        "screenshot" => {
            if args.is_empty() {
                return vec!["usage: screenshot <png>".into()];
            }
            let path = args.join(" ");
            let r = &mut engine.renderer;
            let (w, h) = (r.width, r.height);
            let pixels = r.render_to_image(&engine.scene, &engine.camera, |_, _| {});
            match crate::app::write_png(&path, w, h, &pixels) {
                Ok(()) => vec![format!("wrote {path} ({w}x{h}, {:?})", engine.renderer.color_format)],
                Err(e) => vec![format!("error: {e:#}")],
            }
        }
        "probe" => {
            let v: Vec<f32> = args.iter().filter_map(|a| a.parse().ok()).collect();
            match v[..] {
                [] => engine.probe(engine.player.position.truncate()),
                [x, y] => engine.probe(glam::Vec2::new(x, y)),
                _ => vec!["usage: probe [x y]".into()],
            }
        }
        "qqq" | "quit" => std::process::exit(0),
        _ => vec![format!("unknown command '{cmd}'")],
    }
}

const ITEM_COMMANDS: [&str; 38] = [
    "drop",
    "addspell",
    "removespell",
    "cast",
    "effects",
    "dispelallspells",
    "perkep",
    "addperk",
    "removeperk",
    "hasperk",
    "perks",
    "advskill",
    "incpcs",
    "advlevel",
    "getlevel",
    "setcrimegold",
    "paycrimegold",
    "getrelationshiprank",
    "setrelationshiprank",
    "moveto",
    "enable",
    "disable",
    "placeatme",
    "additem",
    "removeitem",
    "showinventory",
    "inv",
    "openactorcontainer",
    "drawweapon",
    "sheatheweapon",
    "equipitem",
    "unequipitem",
    "getav",
    "setav",
    "modav",
    "forceav",
    "damageav",
    "restoreav",
];

/// Inventory and actor value commands on a reference (the player when none is given).
fn item_command(engine: &mut Engine, r: esp::FormId, cmd: &str, args: &[&str]) -> Vec<String> {
    match cmd {
        "setcrimegold" => {
            let Some(n) = args.first().and_then(|x| x.parse::<i32>().ok()) else {
                return vec!["usage: player.setcrimegold <amount> [faction] [violent 0/1]".into()];
            };
            let Some(f) = args
                .get(1)
                .and_then(|a| engine.resolve_form(a))
                .or_else(|| engine.location_crime_faction())
            else {
                return vec!["no crime faction here; name one".into()];
            };
            engine.set_crime_gold(f, n, args.get(2).is_some_and(|v| *v == "1"));
            engine.describe_bounties()
        }
        "paycrimegold" => {
            let flag = |i: usize| args.get(i).is_none_or(|v| *v != "0");
            let Some(f) = args
                .get(2)
                .and_then(|a| engine.resolve_form(a))
                .or_else(|| engine.location_crime_faction())
            else {
                return vec!["no crime faction here; name one".into()];
            };
            let paid = engine.pay_crime_gold(f, flag(0), flag(1));
            vec![format!("paid {paid} gold to {}", engine.form_name(f))]
        }
        "getrelationshiprank" | "setrelationshiprank" => {
            let Some(other) = args.first().and_then(|a| engine.resolve_form(a)) else {
                return vec![format!("usage: [ref.]{cmd} <actor> [rank]")];
            };
            if cmd == "setrelationshiprank" {
                let Some(rank) = args.get(1).and_then(|x| x.parse::<i32>().ok()) else {
                    return vec!["usage: [ref.]setrelationshiprank <actor> <rank -4..4>".into()];
                };
                engine.set_relationship_rank(r, other, rank);
            }
            vec![format!(
                "{r} / {other}: rank {}",
                engine.relationship_rank(r, other)
            )]
        }
        "getlevel" => vec![format!("{r}: level {}", engine.actor_level(r))],
        "addperk" | "removeperk" | "hasperk" => {
            let Some(p) = args.first().and_then(|a| engine.resolve_form(a)) else {
                return vec![format!("usage: [ref.]{cmd} <perk>")];
            };
            match cmd {
                "addperk" if !engine.add_perk(r, p) => return vec![format!("{p} is not a perk")],
                "removeperk" => engine.remove_perk(r, p),
                _ => {}
            }
            vec![format!("{r} has {p}: {}", engine.has_perk(r, p))]
        }
        "perks" => engine.describe_perks(r, args.first() == Some(&"all")),
        "addspell" | "removespell" => {
            let Some(s) = args.first().and_then(|a| engine.resolve_form(a)) else {
                return vec![format!("usage: [ref.]{cmd} <spell>")];
            };
            let done = if cmd == "addspell" {
                engine.add_spell(r, s)
            } else {
                engine.remove_spell(r, s)
            };
            let mut out = vec![format!("{r} {cmd} {s}: {done}")];
            out.extend(engine.describe_effects(r));
            out
        }
        "cast" => {
            // [ref.]cast <spell> [target]: the ref casts it on the target (itself).
            let Some(s) = args.first().and_then(|a| engine.resolve_form(a)) else {
                return vec!["usage: [ref.]cast <spell> [target | player]".into()];
            };
            let target = match args.get(1) {
                Some(t) if t.eq_ignore_ascii_case("player") => crate::engine::PLAYER_REF,
                Some(t) => match engine.resolve_form(t) {
                    Some(t) => t,
                    None => return vec![format!("no {t}")],
                },
                None => r,
            };
            if !engine.apply_item(s, Some(r), target) {
                return vec![format!("{s} didn't land on {target}")];
            }
            engine.describe_effects(target)
        }
        "effects" => engine.describe_effects(r),
        "dispelallspells" => {
            let lasting: Vec<esp::FormId> = engine
                .magic
                .effects
                .iter()
                .filter(|x| x.target == r && x.duration.is_finite())
                .map(|x| x.item)
                .collect();
            engine.dispel(r, |x| lasting.contains(&x.item));
            engine.describe_effects(r)
        }
        "perkep" => {
            let (Some(point), Some(value)) = (
                args.first().and_then(|a| a.parse::<u8>().ok()),
                args.get(1).and_then(|a| a.parse::<f32>().ok()),
            ) else {
                return vec!["usage: [ref.]perkep <entry point> <value> [subject...]".into()];
            };
            let subjects: Vec<Option<esp::FormId>> =
                args[2..].iter().map(|a| engine.resolve_form(a)).collect();
            let out = engine.perk_entry_point(point, r, &subjects, value);
            vec![format!("{r}: entry point {point}: {value} -> {out}")]
        }
        "advskill" | "incpcs" | "advlevel" if r != crate::engine::PLAYER_REF => {
            vec!["only the player advances".into()]
        }
        "advskill" | "incpcs" => {
            let Some(skill) = args.first().and_then(|n| crate::skills::skill_index(n)) else {
                return vec![format!(
                    "usage: player.{cmd} <skill> {}",
                    if cmd == "advskill" { "<xp>" } else { "" }
                )];
            };
            if cmd == "incpcs" {
                engine.increment_skill(skill);
            } else {
                let Some(xp) = args.get(1).and_then(|x| x.parse::<f32>().ok()) else {
                    return vec!["usage: player.advskill <skill> <xp>".into()];
                };
                engine.add_skill_xp(skill, xp);
            }
            engine.describe_skills()
        }
        "advlevel" => {
            let s = &engine.skills;
            let need = crate::skills::level_up_xp(
                crate::ai::combat::gmst_f32(&engine.lo, "fXPLevelUpBase", 75.0),
                crate::ai::combat::gmst_f32(&engine.lo, "fXPLevelUpMult", 25.0),
                s.level + s.pending as u16,
            ) - s.level_xp;
            engine.add_level_xp(need);
            engine.describe_skills()
        }
        "getav" => {
            let Some(i) = args.first().and_then(|n| esp::actor_value::index(n)) else {
                return vec!["usage: [ref.]getav <actor value>".into()];
            };
            vec![format!("{r}: {}", engine.describe_actor_value(r, i))]
        }
        "setav" | "modav" | "forceav" | "damageav" | "restoreav" => {
            let (Some(i), Some(x)) = (
                args.first().and_then(|n| esp::actor_value::index(n)),
                args.get(1).and_then(|x| x.parse::<f32>().ok()),
            ) else {
                return vec![format!("usage: [ref.]{cmd} <actor value> <amount>")];
            };
            match cmd {
                "setav" => engine.set_actor_value(r, i, x),
                "modav" => engine.mod_actor_value(r, i, x),
                "forceav" => engine.force_actor_value(r, i, x),
                "damageav" => engine.damage_actor_value(r, i, x),
                _ => engine.damage_actor_value(r, i, -x),
            }
            vec![format!("{r}: {}", engine.describe_actor_value(r, i))]
        }
        "moveto" => {
            let Some(t) = args.first().and_then(|a| engine.resolve_form(a)) else {
                return vec!["usage: [ref.]moveto <target ref>".into()];
            };
            let Some(p) = engine.ref_position(t) else {
                return vec![format!("{t} has no position")];
            };
            let rot = engine.ref_rotation(t);
            if engine.move_ref(r, t, p, rot) {
                vec![format!("{r} moved to {t}")]
            } else {
                vec![format!("can't move {r}")]
            }
        }
        "enable" | "disable" => {
            engine.set_disabled(r, cmd == "disable");
            vec![format!("{r} {cmd}d")]
        }
        "drop" => {
            let Some(item) = args.first().and_then(|a| engine.resolve_form(a)) else {
                return vec!["usage: [ref.]drop <item> [count]".into()];
            };
            let n = args.get(1).and_then(|n| n.parse().ok()).unwrap_or(1);
            match engine.drop_item(r, item, n) {
                Some(m) => vec![format!("{r} dropped {item} as {m}")],
                None => vec![format!("{r} has no {item} to drop")],
            }
        }
        "placeatme" => {
            let Some(base) = args.first().and_then(|a| engine.resolve_form(a)) else {
                return vec!["usage: [ref.]placeatme <form> [count]".into()];
            };
            let n = args.get(1).and_then(|n| n.parse().ok()).unwrap_or(1);
            (0..n)
                .map(|_| {
                    engine.create_ref(base, r, false).map_or_else(
                        || format!("can't place {base} at {r}"),
                        |m| format!("placed {m} ({base}) at {r}"),
                    )
                })
                .collect()
        }
        "additem" | "removeitem" => {
            let Some(item) = args.first().and_then(|a| engine.resolve_form(a)) else {
                return vec![format!("usage: {cmd} <item> [count]")];
            };
            let n = args.get(1).and_then(|n| n.parse().ok()).unwrap_or(1);
            if cmd == "additem" {
                engine.add_item(r, item, n);
                vec![format!("{r}: added {n} {item}")]
            } else {
                let taken = engine.remove_item(r, item, n, None);
                vec![format!("{r}: removed {taken} {item}")]
            }
        }
        "equipitem" | "unequipitem" => {
            let Some(item) = args.first().and_then(|a| engine.resolve_form(a)) else {
                return vec![format!("usage: {cmd} <item>")];
            };
            if r != crate::engine::PLAYER_REF {
                return vec!["only the player's equipment can be changed for now".into()];
            }
            let on = cmd == "equipitem";
            if on && engine.magic_item(item).is_some() {
                return match engine.consume(r, item) {
                    Ok(()) => engine.describe_effects(r),
                    Err(e) => vec![e],
                };
            }
            match engine.equip_item(r, item, on) {
                Ok(()) => {
                    let p = engine.protection(r);
                    vec![format!(
                        "{r}: {cmd} {item}; armor {:.0} ({} pieces), blows {:.0}% weaker",
                        p.rating,
                        p.pieces,
                        p.reduction * 100.0
                    )]
                }
                Err(e) => vec![e],
            }
        }
        "drawweapon" | "sheatheweapon" => {
            let draw = cmd == "drawweapon";
            if engine.draw_weapon(r, draw) {
                vec![format!("{r}: {cmd}")]
            } else {
                vec![format!("{r} can't {cmd}")]
            }
        }
        "openactorcontainer" => {
            engine.menu = Some(crate::items::Menu::Container(r));
            vec![format!("opened {r}")]
        }
        _ => {
            let items = engine.listed_inventory(r);
            let equipped = engine
                .inventories
                .get(&r)
                .map(|i| i.equipped.clone())
                .unwrap_or_default();
            let mut out: Vec<String> = items
                .iter()
                .map(|x| {
                    format!(
                        "{:5} {} ({}){}{}",
                        x.count,
                        x.info.name,
                        x.item,
                        if equipped.contains(&x.item) {
                            " [equipped]"
                        } else {
                            ""
                        },
                        x.owner
                            .map_or(String::new(), |o| format!(" [stolen from {o}]"))
                    )
                })
                .collect();
            if out.is_empty() {
                out.push(format!("{r} carries nothing"));
            }
            out
        }
    }
}

/// `sqs`: a quest's stages (done ones marked), each one's log entries and the
/// fragments on them.
fn sqs(engine: &mut Engine, q: esp::FormId) -> Vec<String> {
    let Some(rec) = engine.lo.get(q) else {
        return vec![format!("{q}: no record")];
    };
    let vmad = crate::script::vmad::parse(&rec).unwrap_or_default();
    let st = engine.scripts.quests.get(&q);
    let mut out = vec![format!(
        "{} {q} running {} stage {}",
        rec.editor_id().unwrap_or_default(),
        st.is_some_and(|s| s.running),
        st.map_or(0, |s| s.stage)
    )];
    let mut stage = None;
    let mut entry = 0;
    for sr in rec.subrecords() {
        match &sr.tag.0 {
            b"INDX" => {
                let s = sr.u16(0);
                stage = Some(s);
                entry = 0;
                let done = st.is_some_and(|st| st.done.contains(&s));
                out.push(format!("  stage {s}{}", if done { " (done)" } else { "" }));
            }
            b"QSDT" => {
                let s = stage.unwrap_or(0);
                let frags: Vec<&str> = vmad
                    .fragments
                    .iter()
                    .filter(|f| f.stage == s && f.log_entry == entry)
                    .map(|f| f.function.as_str())
                    .collect();
                out.push(format!("    entry {entry} flags {:#x} {frags:?}", sr.u8(0)));
                entry += 1;
            }
            b"QOBJ" => break,
            _ => {}
        }
    }
    // Fragments on no entry the record has.
    for f in &vmad.fragments {
        if !rec
            .subrecords()
            .any(|sr| sr.tag.0 == *b"INDX" && sr.u16(0) == f.stage)
        {
            out.push(format!(
                "  fragment {} on missing stage {}",
                f.function, f.stage
            ));
        }
    }
    out
}
