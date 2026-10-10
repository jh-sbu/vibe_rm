//! Spike: Skyrim Interface SWFs through Ruffle.
//!   swfspike dis <swf>
//!   swfspike run <swf> <out.png> [ops...]
//!   swfspike view <swf> [ops...]
//! ops: `call:path|arg|arg` (numbers, true/false, else strings), `cb:name|arg|arg`,
//! `set:path|value`, `move:x|y`, `click:x|y`, `key:KeyD`, `pad:south`, `type:Lydia`,
//! `text:Backspace`,
//! `get:path`, `frames:N`, `png:out.png`.
mod dis;
mod fontconfig;
mod fonts;
mod input;
mod view;

use std::sync::{Arc, Mutex};

use anyhow::{Context, anyhow};
use ruffle_core::backend::log::LogBackend;
use ruffle_core::backend::navigator::{NullExecutor, NullNavigatorBackend};
use ruffle_core::context::UpdateContext;
use ruffle_core::external::{ExternalInterfaceProvider, Value};
use ruffle_core::font::DefaultFont;
use ruffle_core::tag_utils::movie_from_path;
use ruffle_core::{Player, PlayerBuilder};
use ruffle_render_wgpu::backend::{
    WgpuRenderBackend, create_wgpu_instance, request_adapter_and_device,
};
use ruffle_render_wgpu::descriptors::Descriptors;
use ruffle_render_wgpu::target::TextureTarget;
use ruffle_render_wgpu::wgpu;

struct Log;
impl LogBackend for Log {
    fn avm_trace(&self, m: &str) {
        println!("[trace] {m}");
    }
    fn avm_warning(&self, m: &str) {
        println!("[warn] {m}");
    }
}

/// Calls from the movie to the game, for the viewer to answer.
pub static GAME_CALLS: Mutex<Vec<(String, Vec<Value>)>> = Mutex::new(Vec::new());

/// The game's side of `ExternalInterface.call`: prints what the movie asks for
/// and queues it in `GAME_CALLS`.
struct Game;
impl ExternalInterfaceProvider for Game {
    fn call_method(&self, _: &mut UpdateContext<'_>, name: &str, args: &[Value]) -> Value {
        println!("[game] {name}({args:?})");
        GAME_CALLS
            .lock()
            .unwrap()
            .push((name.to_string(), args.to_vec()));
        Value::Undefined
    }
    fn on_callback_available(&self, name: &str) {
        println!("[callback] {name}");
    }
    fn get_id(&self) -> Option<String> {
        None
    }
}

fn arg(s: &str) -> Value {
    match s {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        "undefined" => Value::Undefined,
        "null" => Value::Null,
        s => s
            .parse::<f64>()
            .map(Value::Number)
            .unwrap_or_else(|_| Value::String(s.into())),
    }
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .init();
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("dis") => dis::run(&std::fs::read(&args[2])?),
        Some("fonts") => fonts::run(&std::fs::read(&args[2])?),
        Some("run") => run(&args[2], &args[3], &args[4..]),
        Some("view") => view::run(&args[2], &args[3..]),
        _ => Err(anyhow!(
            "usage: swfspike dis|fonts <swf> | run <swf> <out.png> [ops...] | view <swf> [ops...]"
        )),
    }
}

/// A player for `swf` drawing through `renderer`: the game's fonts and English
/// translations in, its first frames run (imports loaded).
pub fn make_player(
    renderer: Box<dyn ruffle_render::backend::RenderBackend>,
    swf: &str,
    (w, h): (u32, u32),
) -> anyhow::Result<(Arc<Mutex<Player>>, NullExecutor)> {
    let movie = movie_from_path(swf, None).map_err(|e| anyhow!(e.to_string()))?;
    println!(
        "movie {}x{} {} fps v{}",
        movie.width().to_pixels(),
        movie.height().to_pixels(),
        movie.frame_rate(),
        movie.version()
    );
    let base = std::path::Path::new(swf).parent().context("swf dir")?;
    let mut executor = NullExecutor::new();
    let navigator = NullNavigatorBackend::with_base_path(base, &executor)?;
    let player = PlayerBuilder::new()
        .with_boxed_renderer(renderer)
        .with_navigator(navigator)
        .with_log(Log)
        .with_external_interface(Box::new(Game))
        .with_gamepad_button_mapping(input::gamepad_key_codes())
        .with_movie(movie)
        .with_viewport_dimensions(w, h, 1.0)
        .with_autoplay(true)
        .build();
    fontconfig::register(&mut player.lock().unwrap(), base)?;
    player
        .lock()
        .unwrap()
        .set_translations(fontconfig::translations(base, "english")?);
    for f in [
        DefaultFont::Serif,
        DefaultFont::Sans,
        DefaultFont::Typewriter,
    ] {
        player
            .lock()
            .unwrap()
            .set_default_font(f, vec!["$EverywhereFont".into()]);
    }
    step(&player, &mut executor, 5);
    Ok((player, executor))
}

/// Runs `n` frames, each after the imports it needs have loaded (Scaleform loads
/// them with the movie).
pub fn step(player: &Arc<Mutex<Player>>, executor: &mut NullExecutor, n: u32) {
    for _ in 0..n {
        for _ in 0..4 {
            player
                .lock()
                .unwrap()
                .preload(&mut ruffle_core::limits::ExecutionLimit::none());
            executor.run();
        }
        player.lock().unwrap().run_frame();
    }
    executor.run();
}

/// Applies a `call:` / `set:` / `get:` / `quality:` op; false for any other kind.
pub fn apply(player: &Arc<Mutex<Player>>, op: &str) -> anyhow::Result<bool> {
    let (kind, rest) = op.split_once(':').context("op kind")?;
    let parts: Vec<&str> = rest.split('|').collect();
    match kind {
        "call" => {
            let r = player
                .lock()
                .unwrap()
                .invoke_avm1(parts[0], parts[1..].iter().map(|s| arg(s)));
            println!("call {} -> {r:?}", parts[0]);
        }
        // The game calling a `GameDelegate.addCallBack` handler, through the
        // movie's ExternalInterface `call` callback.
        "cb" => {
            let r = player
                .lock()
                .unwrap()
                .call_internal_interface("call", parts.iter().map(|s| arg(s)));
            println!("cb {} -> {r:?}", parts[0]);
        }
        "move" | "click" => {
            use ruffle_core::PlayerEvent as E;
            use ruffle_core::events::MouseButton::Left;
            let (x, y): (f64, f64) = (parts[0].parse()?, parts[1].parse()?);
            let mut p = player.lock().unwrap();
            p.handle_event(E::MouseMove { x, y });
            if kind == "click" {
                p.handle_event(E::MouseDown {
                    x,
                    y,
                    button: Left,
                    index: None,
                });
                p.handle_event(E::MouseUp { x, y, button: Left });
            }
        }
        // A key pressed and released, by its winit name (`KeyD`, `Enter`,
        // `ArrowLeft`), through the same translation as the viewer.
        "key" | "keydown" | "keyup" => {
            let code =
                input::winit_key(parts[0]).with_context(|| format!("unknown key {}", parts[0]))?;
            let mut p = player.lock().unwrap();
            let keys = if input::focus_is_text_input(&mut p) {
                input::text_mode_key(code).into_iter().collect()
            } else {
                input::keys(code)
            };
            for key in keys {
                if kind != "keyup" {
                    p.handle_event(ruffle_core::PlayerEvent::KeyDown { key });
                }
                if kind != "keydown" {
                    p.handle_event(ruffle_core::PlayerEvent::KeyUp { key });
                }
            }
        }
        // Text typed into the focused field, a character at a time.
        "type" => {
            let mut p = player.lock().unwrap();
            for codepoint in rest.chars() {
                p.handle_event(ruffle_core::PlayerEvent::TextInput { codepoint });
            }
        }
        // An editing key in the focused field, by its winit name, with `+shift`
        // / `+ctrl` (`Backspace`, `ArrowLeft+shift`, `KeyA+ctrl`).
        "text" => {
            let mut name = parts[0].split('+');
            let code = input::winit_key(name.next().unwrap_or_default())
                .with_context(|| format!("unknown key {}", parts[0]))?;
            let mods: Vec<&str> = name.collect();
            let control =
                input::text_control(code, mods.contains(&"shift"), mods.contains(&"ctrl"))
                    .with_context(|| format!("{} doesn't edit text", parts[0]))?;
            player
                .lock()
                .unwrap()
                .handle_event(ruffle_core::PlayerEvent::TextControl { code: control });
        }
        // A gamepad button pressed and released (`south`, `east`, `dpad-left`, ...).
        "pad" => {
            let button: ruffle_core::events::GamepadButton = parts[0]
                .parse()
                .map_err(|_| anyhow!("unknown gamepad button {}", parts[0]))?;
            let mut p = player.lock().unwrap();
            p.handle_event(ruffle_core::PlayerEvent::GamepadButtonDown { button });
            p.handle_event(ruffle_core::PlayerEvent::GamepadButtonUp { button });
        }
        // The display tree (built with `ruffle_core/avm_debug`).
        "tree" => player.lock().unwrap().spike_display_tree(),
        // Ruffle's per-action AVM1 trace (built with `ruffle_core/avm_debug`).
        "avmdebug" => player
            .lock()
            .unwrap()
            .mutate_with_update_context(|c| c.avm1.set_show_debug_output(parts[0] == "on")),
        "set" => player
            .lock()
            .unwrap()
            .set_avm1_variable(parts[0], arg(parts[1])),
        "get" => println!(
            "get {} = {:?}",
            parts[0],
            player.lock().unwrap().get_avm1_variable(parts[0])
        ),
        "quality" => player.lock().unwrap().set_quality(quality(parts[0])),
        _ => return Ok(false),
    }
    Ok(true)
}

pub fn quality(name: &str) -> ruffle_render::quality::StageQuality {
    use ruffle_render::quality::StageQuality as Q;
    match name {
        "low" => Q::Low,
        "medium" => Q::Medium,
        "best" => Q::Best,
        _ => Q::High,
    }
}

fn run(swf: &str, out: &str, ops: &[String]) -> anyhow::Result<()> {
    let instance =
        create_wgpu_instance(wgpu::Backends::all(), wgpu::BackendOptions::default(), None);
    let (adapter, device, queue) = futures::executor::block_on(request_adapter_and_device(
        wgpu::Backends::all(),
        &instance,
        None,
        wgpu::PowerPreference::HighPerformance,
    ))
    .map_err(|e| anyhow!(e.to_string()))?;
    let descriptors = Arc::new(Descriptors::new(instance, adapter, device, queue));
    let (w, h) = (1280, 720);
    let target =
        TextureTarget::new(&descriptors.device, (w, h)).map_err(|e| anyhow!(e.to_string()))?;
    let renderer =
        WgpuRenderBackend::new(descriptors, target).map_err(|e| anyhow!(e.to_string()))?;
    let (player, mut executor) = make_player(Box::new(renderer), swf, (w, h))?;
    let step = |n: u32, executor: &mut NullExecutor| step(&player, executor, n);
    player.lock().unwrap().set_window_mode("transparent");
    for op in ops {
        if apply(&player, op)? {
            continue;
        }
        let (kind, rest) = op.split_once(':').context("op kind")?;
        let parts: Vec<&str> = rest.split('|').collect();
        match kind {
            "frames" => step(parts[0].parse()?, &mut executor),
            "png" => capture(&player, parts[0])?,
            "blends" => {
                ruffle_render_wgpu::SPIKE_BLENDS.lock().unwrap().clear();
                step(1, &mut executor);
                player.lock().unwrap().render();
                let b = std::mem::take(&mut *ruffle_render_wgpu::SPIKE_BLENDS.lock().unwrap());
                let mut c = std::collections::BTreeMap::new();
                for m in &b {
                    *c.entry(m.clone()).or_insert(0) += 1;
                }
                println!("blends per frame: {} {c:?}", b.len());
            }
            "split" => {
                // Each part on its own: preload, executor, run_frame, render, GPU wait.
                let n: u32 = parts[0].parse()?;
                let mut t = [0f64; 5];
                for _ in 0..n {
                    let a = std::time::Instant::now();
                    player
                        .lock()
                        .unwrap()
                        .preload(&mut ruffle_core::limits::ExecutionLimit::none());
                    let b = std::time::Instant::now();
                    executor.run();
                    let c = std::time::Instant::now();
                    player.lock().unwrap().run_frame();
                    let d = std::time::Instant::now();
                    player.lock().unwrap().render();
                    let e = std::time::Instant::now();
                    descriptors_poll(&player);
                    let f = std::time::Instant::now();
                    for (i, (x, y)) in [(a, b), (b, c), (c, d), (d, e), (e, f)]
                        .into_iter()
                        .enumerate()
                    {
                        t[i] += (y - x).as_secs_f64();
                    }
                }
                let ms = |x: f64| x * 1000.0 / n as f64;
                println!(
                    "split preload {:.3} executor {:.3} run_frame {:.3} render {:.3} gpu-wait {:.3} ms, cache redraws {}",
                    ms(t[0]),
                    ms(t[1]),
                    ms(t[2]),
                    ms(t[3]),
                    ms(t[4]),
                    ruffle_core::spike_dirty()
                );
            }
            "bench" => {
                let n: u32 = parts[0].parse()?;
                let t = std::time::Instant::now();
                for _ in 0..n {
                    step(1, &mut executor);
                    player.lock().unwrap().render();
                }
                // Wait for the GPU so the timing includes it.
                let _ = capture(&player, "/dev/null.png");
                println!(
                    "bench {:.3} ms/frame (run + render)",
                    t.elapsed().as_secs_f64() * 1000.0 / n as f64
                );
            }
            _ => return Err(anyhow!("unknown op {kind}")),
        }
    }
    step(2, &mut executor);
    capture(&player, out)
}

fn capture(player: &Arc<Mutex<Player>>, out: &str) -> anyhow::Result<()> {
    let mut p = player.lock().unwrap();
    p.render();
    let r = <dyn std::any::Any>::downcast_mut::<WgpuRenderBackend<TextureTarget>>(p.renderer_mut())
        .unwrap();
    let mut img = r.capture_frame().context("capture")?;
    // Over a dark backdrop, as over the world.
    for p in img.pixels_mut() {
        let a = p[3] as f32 / 255.0;
        for (c, bg) in p.0.iter_mut().zip([40.0f32, 48.0, 56.0]) {
            *c = (*c as f32 * a + bg * (1.0 - a)).round() as u8;
        }
        p[3] = 255;
    }
    if out != "/dev/null.png" {
        img.save(out)?;
    }
    println!("wrote {out}");
    Ok(())
}

/// Blocks until the GPU has finished the submitted frame.
fn descriptors_poll(player: &Arc<Mutex<Player>>) {
    let mut p = player.lock().unwrap();
    let r = <dyn std::any::Any>::downcast_mut::<WgpuRenderBackend<TextureTarget>>(p.renderer_mut())
        .unwrap();
    let _ = r
        .descriptors()
        .device
        .poll(wgpu::PollType::wait_indefinitely());
}
