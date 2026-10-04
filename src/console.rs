//! Developer console commands (a subset of Skyrim's, plus engine-specific ones).

use glam::Vec3;

use crate::engine::{Engine, Location};

pub fn execute(engine: &mut Engine, line: &str) -> Vec<String> {
    let mut parts = line.split_whitespace();
    let Some(cmd) = parts.next() else { return Vec::new() };
    let args: Vec<&str> = parts.collect();
    let lower = cmd.to_ascii_lowercase();
    match lower.as_str() {
        "help" => vec![
            "coc <cell>            center on cell (editor id)".into(),
            "tcl                   toggle collision (noclip)".into(),
            "fw / sw <weather>     force / set weather".into(),
            "set gamehour to <h>   set time of day".into(),
            "player.setpos x y z   teleport within the current location".into(),
            "getpos                print position".into(),
            "tdt                   toggle debug text".into(),
        ],
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
