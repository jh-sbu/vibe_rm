mod app;
mod console;
mod engine;
mod render;
mod physics;
mod player;
mod script;
mod ui;
mod world;

use anyhow::{Context, Result};

#[derive(Debug, Default, Clone)]
pub struct Options {
    pub data_dir: Option<std::path::PathBuf>,
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
}

fn parse_args() -> Result<Options> {
    let mut o = Options { width: 1280, height: 720, radius: 2, hour: 12.0, ..Default::default() };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = || args.next().with_context(|| format!("{a} needs a value"));
        match a.as_str() {
            "--data" => o.data_dir = Some(val()?.into()),
            "--cell" => o.cell = Some(val()?),
            "--world" => o.world = Some(val()?),
            "--grid" => {
                let v = val()?;
                let (x, y) = v.split_once(',').context("--grid x,y")?;
                o.grid = Some((x.trim().parse()?, y.trim().parse()?));
            }
            "--pos" => {
                let v: Vec<f32> = val()?.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>()?;
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
            "--pick" => {
                let v = val()?;
                let (x, y) = v.split_once(',').context("--pick x,y (0..1 screen coords)")?;
                o.pick = Some((x.parse()?, y.parse()?));
            }
            "-h" | "--help" => {
                println!(
                    "vibe_rm - a Creation Engine (Skyrim SE) compatible engine\n\n\
                     Options:\n  --data <dir>        Skyrim Data directory (default: $SKYRIM_DATA or Steam)\n  \
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
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,wgpu_core=warn,wgpu_hal=warn,naga=warn"))
        .init();
    let opts = parse_args()?;
    app::run(opts)
}
