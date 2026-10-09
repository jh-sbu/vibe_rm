mod activation;
mod actor_values;
mod ai;
mod aliases;
mod anim_events;
mod app;
mod arrest;
mod audio;
mod condition;
mod console;
mod created;
mod crime;
mod detection;
mod dialogue;
mod engine;
mod footsteps;
mod grab;
mod imagespace;
mod impacts;
mod items;
mod journal;
mod locations;
mod locks;
mod loose;
mod messages;
mod physics;
mod pickpocket;
mod player;
mod relationships;
#[cfg(feature = "remote-console")]
mod remote;
mod render;
mod scene;
mod script;
mod story;
mod traps;
mod triggers;
mod ui;
mod weather;
mod world;
mod world_state;

use anyhow::{Context, Result};

#[derive(Debug, Default, Clone)]
pub struct Options {
    pub data_dir: Option<std::path::PathBuf>,
    /// `plugins.txt` whose `*`-enabled entries load after the base game.
    pub plugins_txt: Option<std::path::PathBuf>,
    /// Extra plugins to load after those, in the order given.
    pub plugins: Vec<String>,
    pub cell: Option<String>,
    pub world: Option<String>,
    pub grid: Option<(i32, i32)>,
    pub position: Option<glam::Vec3>,
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
    pub screenshot: Option<std::path::PathBuf>,
    pub width: u32,
    pub height: u32,
    pub radius: i32,
    pub hour: f32,
    pub weather: Option<String>,
    pub simulate: Option<u32>,
    pub use_door: Option<usize>,
    pub bench: Option<u32>,
    pub pick: Option<(f32, f32)>,
    pub wait: Option<u32>,
    pub no_scripts: bool,
    pub talk: Option<String>,
    pub choose: Vec<usize>,
    /// While waiting, also save a screenshot every this many frames.
    pub burst: Option<u32>,
    /// Keep the camera in front of this actor while waiting.
    pub watch: Option<String>,
    /// Degrees to orbit the --watch camera around the actor (0: in front).
    pub watch_angle: f32,
    /// Keep the player's eye at the camera while waiting (NPCs look at it).
    pub player_at_camera: bool,
    /// Console commands to run once the world is set up.
    pub console: Vec<String>,
    /// While waiting, leave message boxes up (they pause the world) instead of
    /// pressing their last button after two seconds.
    pub hold_boxes: bool,
}

fn parse_args() -> Result<Options> {
    let mut o = Options {
        width: 1280,
        height: 720,
        radius: 2,
        hour: 12.0,
        ..Default::default()
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = || args.next().with_context(|| format!("{a} needs a value"));
        match a.as_str() {
            "--data" => o.data_dir = Some(val()?.into()),
            "--plugins" => o.plugins_txt = Some(val()?.into()),
            "--plugin" => o.plugins.push(val()?),
            "--cell" => o.cell = Some(val()?),
            "--world" => o.world = Some(val()?),
            "--grid" => {
                let v = val()?;
                let (x, y) = v.split_once(',').context("--grid x,y")?;
                o.grid = Some((x.trim().parse()?, y.trim().parse()?));
            }
            "--pos" => {
                let v: Vec<f32> = val()?
                    .split(',')
                    .map(|s| s.trim().parse())
                    .collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "--pos x,y,z");
                o.position = Some(glam::Vec3::new(v[0], v[1], v[2]));
            }
            "--yaw" => o.yaw = Some(val()?.parse::<f32>()?.to_radians()),
            "--pitch" => o.pitch = Some(val()?.parse::<f32>()?.to_radians()),
            "--screenshot" => o.screenshot = Some(val()?.into()),
            "--size" => {
                let v = val()?;
                let (w, h) = v.split_once('x').context("--size WxH")?;
                o.width = w.parse()?;
                o.height = h.parse()?;
            }
            "--radius" => o.radius = val()?.parse()?,
            "--hour" => o.hour = val()?.parse()?,
            "--weather" => o.weather = Some(val()?),
            "--simulate" => o.simulate = Some(val()?.parse()?),
            "--use-door" => o.use_door = Some(val()?.parse()?),
            "--bench" => o.bench = Some(val()?.parse()?),
            "--wait" => o.wait = Some(val()?.parse()?),
            "--no-scripts" => o.no_scripts = true,
            "--talk" => o.talk = Some(val()?),
            "--choose" => o.choose.push(val()?.parse()?),
            "--burst" => o.burst = Some(val()?.parse()?),
            "--watch" => o.watch = Some(val()?),
            "--console" => o.console.push(val()?),
            "--watch-angle" => o.watch_angle = val()?.parse::<f32>()?.to_radians(),
            "--player-at-camera" => o.player_at_camera = true,
            "--hold-boxes" => o.hold_boxes = true,
            "--pick" => {
                let v = val()?;
                let (x, y) = v
                    .split_once(',')
                    .context("--pick x,y (0..1 screen coords)")?;
                o.pick = Some((x.parse()?, y.parse()?));
            }
            "-h" | "--help" => {
                println!(
                    "VibeRM - a Creation Engine (Skyrim SE) compatible engine\n\n\
                     Options:\n  --data <dir>        Skyrim Data directory (default: $SKYRIM_DATA or Steam)\n  \
                     --plugins <txt>     Load the enabled (*) plugins of a plugins.txt\n  \
                     --plugin <file>     Load a plugin from Data (repeatable)\n  \
                     --cell <edid|formid> Interior cell to load\n  --world <edid>      Worldspace (default Tamriel)\n  \
                     --grid x,y          Exterior cell coordinates\n  --pos x,y,z         Camera position\n  \
                     --yaw/--pitch deg   Camera orientation\n  --radius n          Exterior cell load radius\n  --hour h            Time of day (0-24)\n  \
                     --weather <edid>    Force a weather\n  \
                     --screenshot <png>  Render one frame offscreen and exit\n  --size WxH          Window/screenshot size"
                );
                std::process::exit(0);
            }
            other => anyhow::bail!("unknown argument {other}"),
        }
    }
    Ok(o)
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,wgpu_core=warn,wgpu_hal=warn,naga=warn"),
    )
    .init();
    let opts = parse_args()?;
    app::run(opts)
}
