//! Developer console commands (a subset of Skyrim's, plus engine-specific ones).

use glam::Vec3;

use crate::engine::{Engine, Location};

pub fn execute(engine: &mut Engine, line: &str) -> Vec<String> {
    let mut parts = line.split_whitespace();
    let Some(cmd) = parts.next() else { return Vec::new() };
    let args: Vec<&str> = parts.collect();
    let lower = cmd.to_ascii_lowercase();
    // `<ref>.command` (`player.additem f 100`) acts on that reference.
    if let Some((target, sub)) = lower.split_once('.').filter(|(_, sub)| ITEM_COMMANDS.contains(sub)) {
        let r = if target == "player" { Some(crate::engine::PLAYER_REF) } else { engine.resolve_form(target) };
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
            "fw / sw <weather>     force / set weather".into(),
            "set gamehour to <h>   set time of day".into(),
            "player.setpos x y z   teleport within the current location".into(),
            "getpos                print position".into(),
            "tdt                   toggle debug text".into(),
            "tai                   toggle actor AI".into(),
            "tsh                   toggle sun shadows".into(),
            "sae <ref> <event>     send a behaviour event to an actor".into(),
            "pi <ref> <idle>       play an IDLE record on an actor".into(),
            "use <ref> <furniture> send an actor to use furniture or an idle marker".into(),
            "gstate <ref>          an actor's active behaviour graph states".into(),
            "sgv <ref> <var> <x>   set a behaviour graph variable".into(),
            "door <ref>            open / close a door".into(),
            "[ref.]additem <item> [n] / removeitem <item> [n] / showinventory".into(),
            "activate <ref>        activate a reference as the player".into(),
        ],
        "bark" => {
            let [r, sub] = args[..] else { return vec!["usage: bark <actor ref> <subtype, e.g. HELO / IDLE>".into()] };
            let (Some(actor), Ok(sub)) = (engine.resolve_form(r), <[u8; 4]>::try_from(sub.to_ascii_uppercase().as_bytes())) else { return vec!["bad reference or subtype".into()] };
            if engine.bark(actor, &sub) { vec![format!("{r} says something")] } else { vec![format!("{r} has nothing to say")] }
        }
        "activate" => {
            let Some(r) = args.first().and_then(|r| engine.resolve_form(r)) else { return vec!["usage: activate <ref>".into()] };
            let name = engine.base_of(r).and_then(|b| engine.lo.get(b)).and_then(|b| b.get(b"FULL").map(|d| engine.lo.lstring(&b, d))).unwrap_or_default();
            engine.look_target = Some((r, name));
            match engine.activate() {
                Ok(()) => vec![format!("activated {r}")],
                Err(e) => vec![format!("error: {e:#}")],
            }
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
        "startcombat" => {
            let [a, t] = args[..] else { return vec!["usage: startcombat <actor ref> <target ref | player>".into()] };
            let target = if t.eq_ignore_ascii_case("player") { Some(crate::engine::PLAYER_REF) } else { engine.resolve_form(t) };
            match (engine.resolve_form(a), target) {
                (Some(a), Some(t)) if engine.start_combat(a, t) => vec![format!("{a} attacks {t}")],
                _ => vec!["can't start that fight".into()],
            }
        }
        "kill" => {
            let Some(actor) = args.first().and_then(|r| engine.resolve_form(r)) else { return vec!["usage: kill <actor ref>".into()] };
            if engine.kill_actor(actor) { vec![format!("{actor} killed")] } else { vec![format!("{actor} isn't a living loaded actor")] }
        }
        "sgv" => {
            let [r, var, value] = args[..] else { return vec!["usage: sgv <actor ref> <variable> <value>".into()] };
            let (Some(actor), Ok(x)) = (engine.resolve_form(r), value.parse::<f32>()) else { return vec!["bad reference or value".into()] };
            if engine.set_graph_variable(actor, var, x) { vec![format!("{r}: {var} = {x}")] } else { vec![format!("{r} has no behaviour graph")] }
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
            let Some(name) = args.first() else { return vec!["usage: fw <weather editor id>".into()] };
            engine.forced_weather = Some(name.to_string());
            if let Location::Exterior { world, .. } = engine.location {
                engine.setup_weather(world);
            }
            vec![format!("weather {name}")]
        }
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
        "qqq" | "quit" => std::process::exit(0),
        _ => vec![format!("unknown command '{cmd}'")],
    }
}

const ITEM_COMMANDS: [&str; 7] = ["additem", "removeitem", "showinventory", "inv", "openactorcontainer", "drawweapon", "sheatheweapon"];

/// Inventory commands on a reference (the player when none is given).
fn item_command(engine: &mut Engine, r: esp::FormId, cmd: &str, args: &[&str]) -> Vec<String> {
    match cmd {
        "additem" | "removeitem" => {
            let Some(item) = args.first().and_then(|a| engine.resolve_form(a)) else { return vec![format!("usage: {cmd} <item> [count]")] };
            let n = args.get(1).and_then(|n| n.parse().ok()).unwrap_or(1);
            if cmd == "additem" {
                engine.add_item(r, item, n);
                vec![format!("{r}: added {n} {item}")]
            } else {
                let taken = engine.remove_item(r, item, n, None);
                vec![format!("{r}: removed {taken} {item}")]
            }
        }
        "drawweapon" | "sheatheweapon" => {
            let draw = cmd == "drawweapon";
            if engine.draw_weapon(r, draw) { vec![format!("{r}: {cmd}")] } else { vec![format!("{r} can't {cmd}")] }
        }
        "openactorcontainer" => {
            engine.menu = Some(crate::items::Menu::Container(r));
            vec![format!("opened {r}")]
        }
        _ => {
            let items = engine.listed_inventory(r);
            let equipped = engine.inventories.get(&r).map(|i| i.equipped.clone()).unwrap_or_default();
            let mut out: Vec<String> =
                items.iter().map(|(f, n, i)| format!("{n:5} {} ({f}){}", i.name, if equipped.contains(f) { " [equipped]" } else { "" })).collect();
            if out.is_empty() {
                out.push(format!("{r} carries nothing"));
            }
            out
        }
    }
}
