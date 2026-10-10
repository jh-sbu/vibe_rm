//! Application shell: window, input, and the engine state.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use esp::LoadOrder;
use glam::Vec3;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

use crate::Options;
use crate::engine::Engine;
use crate::player::MoveInput;
use crate::render::Renderer;

fn init_data(opts: &Options) -> Result<(LoadOrder, vfs::Vfs)> {
    let data_dir = match &opts.data_dir {
        Some(d) => d.clone(),
        None => {
            vfs::locate_data_dir().context("could not find a Skyrim Data directory; pass --data")?
        }
    };
    log::info!("data directory: {}", data_dir.display());
    if let Some(p) = &opts.plugins_txt {
        anyhow::ensure!(p.is_file(), "plugins.txt not found: {}", p.display());
    }
    let mut names = LoadOrder::default_plugin_list(&data_dir, opts.plugins_txt.as_deref());
    for p in &opts.plugins {
        anyhow::ensure!(
            data_dir.join(p).is_file(),
            "plugin {p} not found in {}",
            data_dir.display()
        );
        if !names.iter().any(|n| n.eq_ignore_ascii_case(p)) {
            names.push(p.clone());
        }
    }
    log::info!("plugins: {}", names.join(", "));
    let t = Instant::now();
    let mut lo = LoadOrder::load(&data_dir, &names)?;
    log::info!(
        "load order indexed: {} records in {:?}",
        lo.record_count(),
        t.elapsed()
    );
    let vfs = vfs::Vfs::new(&data_dir, &names);
    lo.load_strings("english", |p| vfs.read(p));
    Ok((lo, vfs))
}

async fn create_device(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue)> {
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: surface,
            apply_limit_buckets: false,
        })
        .await?;
    log::info!("adapter: {:?}", adapter.get_info());
    let mut features = wgpu::Features::TEXTURE_COMPRESSION_BC;
    if adapter
        .features()
        .contains(wgpu::Features::TEXTURE_COMPRESSION_BC_SLICED_3D)
    {
        features |= wgpu::Features::TEXTURE_COMPRESSION_BC_SLICED_3D;
    }
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("device"),
            required_features: features,
            required_limits: adapter.limits(),
            ..Default::default()
        })
        .await?;
    Ok((adapter, device, queue))
}

fn setup_engine(
    opts: &Options,
    renderer: Renderer,
    lo: LoadOrder,
    vfs: vfs::Vfs,
    audio: Option<crate::audio::Audio>,
) -> Result<Engine> {
    let mut engine = Engine::new(
        lo,
        vfs,
        renderer,
        opts.hour,
        opts.weather.clone(),
        opts.radius,
    );
    engine.audio = audio;
    if !opts.no_scripts {
        let t = Instant::now();
        engine.start_game_enabled_quests();
        log::info!("quests started in {:?}", t.elapsed());
    }
    if let Some(c) = &opts.cell {
        let id = engine
            .resolve_form(c)
            .with_context(|| format!("unknown cell {c}"))?;
        let exterior = engine.lo.cell(id).and_then(|c| c.world);
        match (exterior, opts.position) {
            (Some(w), Some(p)) => engine.enter_exterior(w, p, opts.yaw.unwrap_or(0.0))?,
            (None, Some(p)) => engine.enter_interior(id, Some((p, opts.yaw.unwrap_or(0.0))))?,
            _ => engine.center_on_cell(id)?,
        }
    } else {
        let wname = opts.world.clone().unwrap_or_else(|| "Tamriel".into());
        let w = engine
            .resolve_form(&wname)
            .with_context(|| format!("unknown worldspace {wname}"))?;
        let (x, y) = opts.grid.unwrap_or((4, -12));
        let p = opts.position.unwrap_or(Vec3::new(
            (x as f32 + 0.5) * 4096.0,
            (y as f32 + 0.5) * 4096.0,
            -100_000.0,
        ));
        engine.enter_exterior(w, p, opts.yaw.unwrap_or(0.0))?;
    }
    if let Some(y) = opts.yaw {
        engine.camera.yaw = y;
    }
    if let Some(p) = opts.pitch {
        engine.camera.pitch = p;
    }
    Ok(engine)
}

pub fn run(opts: Options) -> Result<()> {
    let (lo, vfs) = init_data(&opts)?;
    if let Some(path) = opts.screenshot.clone() {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let (adapter, device, queue) = pollster::block_on(create_device(&instance, None))?;
        let renderer = Renderer::new(
            device,
            queue,
            wgpu::TextureFormat::Rgba8Unorm,
            opts.width,
            opts.height,
        );
        // VRM_AUDIO=1 plays (or at least tracks) sounds offscreen too.
        let audio = std::env::var_os("VRM_AUDIO").map(|_| crate::audio::Audio::new());
        let mut engine = setup_engine(&opts, renderer, lo, vfs, audio)?;
        if let Some(n) = opts.use_door {
            let doors = engine.load_doors();
            for d in &doors {
                log::info!(
                    "door {} at {:?} -> {:?}",
                    d.ref_id,
                    d.position,
                    d.destination
                );
            }
            let d = doors.get(n).context("no such door")?;
            let (dest, pos, rot) = d.destination.unwrap();
            engine.teleport_through(dest, pos, rot.z)?;
        }
        if let Some(t) = &opts.talk {
            let id = engine.resolve_form(t).context("unknown reference")?;
            engine.start_conversation(id);
        }
        // "@<frame> <command>" runs at that frame of --wait instead of now.
        let (later, now): (Vec<(u32, &str)>, Vec<(u32, &str)>) = opts
            .console
            .iter()
            .map(
                |l| match l.strip_prefix('@').and_then(|r| r.split_once(' ')) {
                    Some((f, cmd)) if f.parse::<u32>().is_ok() => (f.parse().unwrap(), cmd),
                    _ => (0, l.as_str()),
                },
            )
            .partition(|(f, _)| *f > 0);
        let run_console = |engine: &mut Engine, line: &str| {
            for out in crate::console::execute(engine, line) {
                log::info!("console: {out}");
            }
        };
        for (_, line) in now {
            run_console(&mut engine, line);
        }
        // Frames the message box in front has been up.
        let mut box_up = 0;
        if let Some(frames) = opts.wait {
            // Advance the world without moving the camera.
            let (pos, yaw, pitch) = (
                engine.camera.position,
                engine.camera.yaw,
                engine.camera.pitch,
            );
            let watch = opts
                .watch
                .as_deref()
                .map(|w| engine.resolve_form(w).context("unknown --watch reference"))
                .transpose()?;
            engine.player.noclip = true;
            for i in 0..frames {
                for (_, line) in later.iter().filter(|(f, _)| *f == i) {
                    run_console(&mut engine, line);
                }
                answer_boxes(&mut engine, &opts, &mut box_up);
                engine.update(MoveInput::default(), 1.0 / 60.0, 20.0);
                engine.camera.position = pos;
                engine.camera.yaw = yaw;
                engine.camera.pitch = pitch;
                if let Some((feet, heading)) = watch.and_then(|w| engine.actor_pose(w)) {
                    // Stand in front of the actor at head height and look at its chest.
                    let a = heading + opts.watch_angle;
                    let eye = feet
                        + glam::Vec3::new(a.sin(), a.cos(), 0.0) * 170.0
                        + glam::Vec3::Z * 110.0;
                    let d = feet + glam::Vec3::Z * 80.0 - eye;
                    engine.camera.position = eye;
                    engine.camera.yaw = d.x.atan2(d.y);
                    engine.camera.pitch = (d.z / d.length().max(1.0)).asin();
                }
                if let Some((p, yaw, pitch)) = engine.test_camera {
                    (
                        engine.camera.position,
                        engine.camera.yaw,
                        engine.camera.pitch,
                    ) = (p, yaw, pitch);
                }
                if opts.player_at_camera {
                    engine.player.position =
                        engine.camera.position - (engine.player.eye() - engine.player.position);
                }
                if opts.burst.is_some_and(|n| n > 0 && i % n == 0) {
                    let pixels = engine.renderer.render_to_image(
                        &engine.scene,
                        &engine.view_camera(),
                        |_, _| {},
                    );
                    let stem = path.with_extension("");
                    write_png(
                        &format!("{}_{i:05}.png", stem.display()),
                        opts.width,
                        opts.height,
                        &pixels,
                    )?;
                }
            }
        }
        for &c in &opts.choose {
            engine.choose_topic(c);
            for _ in 0..600 {
                answer_boxes(&mut engine, &opts, &mut box_up);
                engine.update(MoveInput::default(), 1.0 / 60.0, 20.0);
            }
        }
        if let Some(frames) = opts.simulate {
            let input = MoveInput {
                forward: 1.0,
                run: true,
                ..Default::default()
            };
            for i in 0..frames {
                answer_boxes(&mut engine, &opts, &mut box_up);
                engine.update(input, 1.0 / 60.0, 20.0);
                if i % 30 == 0 {
                    log::info!(
                        "sim frame {i}: pos {:?} grounded {} looking at {:?}",
                        engine.player.position,
                        engine.player.grounded,
                        engine.look_target
                    );
                }
            }
        }
        if let Some((px, py)) = opts.pick {
            let aspect = opts.width as f32 / opts.height as f32;
            let vp = engine.camera.proj(aspect) * engine.camera.view();
            let inv = vp.inverse();
            let ndc = glam::Vec4::new(px * 2.0 - 1.0, 1.0 - py * 2.0, 1.0, 1.0);
            let p = inv * ndc;
            let dir = ((p.truncate() / p.w) - engine.camera.position).normalize();
            for (t, name) in engine
                .scene
                .pick(engine.camera.position, dir)
                .iter()
                .take(8)
            {
                log::info!("pick: {t:.0} {name}");
            }
        }
        if let Some(n) = opts.bench {
            let t = engine
                .renderer
                .bench(&engine.scene, &engine.view_camera(), n);
            log::info!(
                "bench: {:?}/frame ({:.1} fps), {:?}",
                t,
                1.0 / t.as_secs_f64(),
                engine.renderer.stats
            );
        }
        let mut ui = crate::ui::Ui::new(&engine.renderer.device, engine.renderer.color_format);
        ui.show_debug = true;
        let mut swf = open_swf_ui(
            &opts,
            instance,
            adapter,
            &engine,
            engine.renderer.color_format,
            (opts.width, opts.height),
        );
        ui.swf_hud = swf.is_some();
        if let Some(swf) = &mut swf {
            // A second of the HUD: its meters and messages fade in.
            for _ in 0..60 {
                swf.update(&engine, 1.0 / 60.0);
            }
        }
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(opts.width as f32, opts.height as f32),
            )),
            ..Default::default()
        };
        log::debug!("conversation active: {}", engine.conversation.is_some());
        // egui hides new areas during their first (sizing) pass.
        let first = ui.build(raw.clone(), &mut engine);
        let mut out = ui.build(raw, &mut engine);
        // Texture uploads (the font atlas) from the first pass must not be lost.
        let mut deltas = first.textures_delta;
        deltas.append(std::mem::take(&mut out.textures_delta));
        out.textures_delta = deltas;
        let size = [opts.width, opts.height];
        let hud_visible = crate::swf_ui::SwfUi::visible(&engine);
        let pixels =
            engine
                .renderer
                .render_to_image(&engine.scene, &engine.view_camera(), |r, view| {
                    if let Some(swf) = &mut swf {
                        swf.draw(view, hud_visible);
                    }
                    ui.paint(&r.device, &r.queue, view, out, size);
                });
        log::info!("render stats: {:?}", engine.renderer.stats);
        write_png(
            &path.display().to_string(),
            opts.width,
            opts.height,
            &pixels,
        )?;
        return Ok(());
    }
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        opts,
        data: Some((lo, vfs)),
        state: None,
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}

/// The game's own menus over the frame, unless `--egui-hud` or they can't be
/// set up (no Interface files): egui draws the HUD then.
fn open_swf_ui(
    opts: &Options,
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    engine: &Engine,
    format: wgpu::TextureFormat,
    size: (u32, u32),
) -> Option<crate::swf_ui::SwfUi> {
    if opts.egui_hud {
        return None;
    }
    let t = Instant::now();
    match crate::swf_ui::SwfUi::new(instance, adapter, engine, format, size) {
        Ok(swf) => {
            log::info!("hudmenu.swf up in {:?}", t.elapsed());
            Some(swf)
        }
        Err(e) => {
            log::error!("the game's HUD couldn't be set up ({e:#}); drawing egui's");
            None
        }
    }
}

/// Offscreen, nobody answers message boxes, and they pause the world (the
/// Survival Mode prompt comes up at the start): press the last button of one
/// left up for two seconds unless `--hold-boxes` (`@<frame> msgbox <button>`
/// presses one).
fn answer_boxes(engine: &mut Engine, opts: &Options, box_up: &mut u32) {
    *box_up = if engine.messages.boxes.is_empty() {
        0
    } else {
        *box_up + 1
    };
    if *box_up > 120 && !opts.hold_boxes {
        let b = engine
            .messages
            .boxes
            .front()
            .and_then(|b| b.buttons.last().cloned());
        log::info!("offscreen: pressing {b:?} on the message box left up");
        engine.choose_message_button(b.map_or(0, |b| b.0));
        *box_up = 0;
    }
}

pub(crate) fn write_png(path: &str, width: u32, height: u32, pixels: &[u8]) -> Result<()> {
    let file = std::fs::File::create(path)?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(pixels)?;
    log::info!("wrote {path}");
    Ok(())
}

struct WindowState {
    ui: crate::ui::Ui,
    /// The game's own menus (the HUD), when they could be set up.
    swf: Option<crate::swf_ui::SwfUi>,
    egui_state: egui_winit::State,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    engine: Engine,
    keys: HashSet<KeyCode>,
    grabbed: bool,
    last: Instant,
    fps_timer: Instant,
    frames: u32,
    #[cfg(feature = "remote-console")]
    remote: Option<crate::remote::RemoteConsole>,
}

struct App {
    opts: Options,
    data: Option<(LoadOrder, vfs::Vfs)>,
    state: Option<WindowState>,
}

impl App {
    fn init(&mut self, el: &ActiveEventLoop) -> Result<()> {
        let window = Arc::new(
            el.create_window(
                Window::default_attributes()
                    .with_title("VibeRM")
                    .with_inner_size(winit::dpi::PhysicalSize::new(
                        self.opts.width,
                        self.opts.height,
                    )),
            )?,
        );
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(
                Box::new(el.owned_display_handle()),
            ));
        let surface = instance.create_surface(window.clone())?;
        let (adapter, device, queue) =
            pollster::block_on(create_device(&instance, Some(&surface)))?;
        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        let renderer = Renderer::new(device, queue, format, config.width, config.height);
        let (lo, vfs) = self.data.take().context("already initialised")?;
        let engine = setup_engine(
            &self.opts,
            renderer,
            lo,
            vfs,
            Some(crate::audio::Audio::new()),
        )?;
        let mut ui = crate::ui::Ui::new(&engine.renderer.device, format);
        let swf = open_swf_ui(
            &self.opts,
            instance,
            adapter,
            &engine,
            format,
            (config.width, config.height),
        );
        ui.swf_hud = swf.is_some();
        let egui_state = egui_winit::State::new(
            ui.ctx.clone(),
            egui::ViewportId::ROOT,
            el,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        self.state = Some(WindowState {
            ui,
            swf,
            egui_state,
            window,
            surface,
            config,
            engine,
            keys: HashSet::new(),
            grabbed: false,
            last: Instant::now(),
            fps_timer: Instant::now(),
            frames: 0,
            #[cfg(feature = "remote-console")]
            remote: crate::remote::RemoteConsole::start()
                .inspect_err(|e| log::error!("remote console: {e}"))
                .ok(),
        });
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.state.is_none()
            && let Err(e) = self.init(el)
        {
            log::error!("initialisation failed: {e:#}");
            el.exit();
        }
    }

    fn device_event(
        &mut self,
        _el: &ActiveEventLoop,
        _id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        let Some(s) = &mut self.state else { return };
        if let DeviceEvent::MouseMotion { delta } = event
            && s.grabbed
            && !s.engine.disabled_controls.looking
        {
            let sens = 0.0025;
            if !s.engine.menu_up() {
                s.engine.input_event("Look");
            }
            s.engine.camera.yaw += delta.0 as f32 * sens;
            s.engine.camera.pitch =
                (s.engine.camera.pitch - delta.1 as f32 * sens).clamp(-1.55, 1.55);
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(s) = &mut self.state else { return };
        // The console and dialogue menu take input while open.
        let console_open = s.ui.console.open || s.engine.conversation.is_some();
        if s.engine.conversation.is_some() && s.grabbed {
            s.grabbed = false;
            let _ = s.window.set_cursor_grab(CursorGrabMode::None);
            s.window.set_cursor_visible(true);
            s.keys.clear();
        }
        let resp = s.egui_state.on_window_event(&s.window, &event);
        if console_open
            && resp.consumed
            && !matches!(&event, WindowEvent::KeyboardInput { event, .. } if event.physical_key == PhysicalKey::Code(KeyCode::Backquote))
        {
            if let WindowEvent::KeyboardInput { .. } = event {
                return;
            }
        }
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(size) => {
                s.config.width = size.width.max(1);
                s.config.height = size.height.max(1);
                s.surface.configure(&s.engine.renderer.device, &s.config);
                s.engine.renderer.resize(s.config.width, s.config.height);
                if let Some(swf) = &mut s.swf {
                    swf.resize(s.config.width, s.config.height);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    if code == KeyCode::Backquote
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        s.ui.toggle_console();
                        s.keys.clear();
                        if s.ui.console.open && s.grabbed {
                            s.grabbed = false;
                            let _ = s.window.set_cursor_grab(CursorGrabMode::None);
                            s.window.set_cursor_visible(true);
                        }
                        return;
                    }
                    if s.ui.console.open || s.engine.conversation.is_some() || s.engine.menu_up() {
                        s.keys.clear();
                        return;
                    }
                    if event.state == ElementState::Pressed && !event.repeat {
                        for ev in crate::messages::key_events(code) {
                            s.engine.input_event(ev);
                        }
                    }
                    match event.state {
                        ElementState::Pressed => {
                            if code == KeyCode::F3 && !event.repeat {
                                s.ui.show_debug = !s.ui.show_debug;
                            }
                            if code == KeyCode::Escape {
                                if s.grabbed {
                                    s.grabbed = false;
                                    let _ = s.window.set_cursor_grab(CursorGrabMode::None);
                                    s.window.set_cursor_visible(true);
                                } else {
                                    el.exit();
                                }
                            }
                            if code == KeyCode::KeyE
                                && !event.repeat
                                && let Err(e) = s.engine.activate_press()
                            {
                                log::error!("activation failed: {e:#}");
                            }
                            if code == KeyCode::Tab
                                && !event.repeat
                                && !s.engine.disabled_controls.menu
                            {
                                s.engine.menu = Some(crate::items::Menu::Inventory);
                            }
                            if code == KeyCode::KeyJ
                                && !event.repeat
                                && !s.engine.disabled_controls.journal
                            {
                                s.engine.menu = Some(crate::items::Menu::Journal);
                            }
                            if code == KeyCode::KeyK
                                && !event.repeat
                                && !s.engine.disabled_controls.menu
                            {
                                s.engine.menu = Some(crate::items::Menu::Skills);
                            }
                            // Menus take the mouse.
                            if s.engine.menu_up() {
                                if s.grabbed {
                                    s.grabbed = false;
                                    let _ = s.window.set_cursor_grab(CursorGrabMode::None);
                                    s.window.set_cursor_visible(true);
                                }
                                s.keys.clear();
                                return;
                            }
                            // Ctrl sneaks (noclip flies down with it instead).
                            if code == KeyCode::ControlLeft
                                && !event.repeat
                                && !s.engine.player.noclip
                                && !s.engine.disabled_controls.sneaking
                            {
                                s.engine.player.sneaking = !s.engine.player.sneaking;
                            }
                            // R draws or sheathes the weapon.
                            if code == KeyCode::KeyR && !event.repeat {
                                let draw = !s.engine.player_weapon_drawn();
                                s.engine.player_draw_weapon(draw);
                            }
                            // F switches between first and third person.
                            if code == KeyCode::KeyF
                                && !event.repeat
                                && !s.engine.disabled_controls.cam_switch
                            {
                                let third = !s.engine.third_person;
                                s.engine.set_third_person(third);
                            }
                            if code == KeyCode::KeyN && !event.repeat {
                                s.engine.player.noclip = !s.engine.player.noclip;
                                log::info!("noclip {}", s.engine.player.noclip);
                            }
                            s.keys.insert(code);
                        }
                        ElementState::Released => {
                            if code == KeyCode::KeyE
                                && let Err(e) = s.engine.activate_release()
                            {
                                log::error!("activation failed: {e:#}");
                            }
                            s.keys.remove(&code);
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // The wheel moves the third-person camera nearer or farther.
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                if s.grabbed && s.engine.third_person && !s.engine.disabled_controls.cam_switch {
                    s.engine.zoom_third_person(-lines * 30.0);
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Right,
                ..
            } => {
                // Captured: hold to block.
                s.engine.player_blocking =
                    state == ElementState::Pressed && s.grabbed && !s.engine.menu_up();
                if s.engine.player_blocking {
                    s.engine.input_event("Left Attack/Block");
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                s.engine.player_attack_release();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                // Captured: swing at what's ahead (held: a power attack).
                if s.grabbed && !s.engine.menu_up() {
                    s.engine.input_event("Right Attack/Block");
                    s.engine.player_attack_press();
                }
                if !s.grabbed
                    && !s.ui.console.open
                    && s.engine.conversation.is_none()
                    && !s.engine.menu_up()
                {
                    let ok = s
                        .window
                        .set_cursor_grab(CursorGrabMode::Locked)
                        .or_else(|_| s.window.set_cursor_grab(CursorGrabMode::Confined));
                    if ok.is_ok() {
                        s.grabbed = true;
                        s.window.set_cursor_visible(false);
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = (now - s.last).as_secs_f32().min(0.1);
                s.last = now;
                let input = move_input(&s.keys);
                let scale = if s.keys.contains(&KeyCode::KeyT) {
                    2000.0
                } else {
                    20.0
                };
                #[cfg(feature = "remote-console")]
                if let Some(r) = &s.remote {
                    r.poll(&mut s.engine);
                }
                s.engine.console_open = s.ui.console.open;
                s.engine.update(input, dt, scale);
                if let Some(swf) = &mut s.swf {
                    swf.update(&s.engine, dt);
                }
                // A menu or message box opened by a script takes the mouse.
                if s.engine.menu_up() && s.grabbed {
                    s.grabbed = false;
                    let _ = s.window.set_cursor_grab(CursorGrabMode::None);
                    s.window.set_cursor_visible(true);
                    s.keys.clear();
                }
                let frame = match s.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(t)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                    wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                        s.surface.configure(&s.engine.renderer.device, &s.config);
                        return;
                    }
                    _ => return,
                };
                let view = frame.texture.create_view(&Default::default());
                s.engine
                    .renderer
                    .render(&s.engine.scene, &s.engine.view_camera(), &view);
                if let Some(swf) = &mut s.swf {
                    swf.draw(&view, crate::swf_ui::SwfUi::visible(&s.engine));
                }
                let raw = s.egui_state.take_egui_input(&s.window);
                let out = s.ui.build(raw, &mut s.engine);
                s.egui_state
                    .handle_platform_output(&s.window, out.platform_output.clone());
                let size = [s.config.width, s.config.height];
                s.ui.paint(
                    &s.engine.renderer.device,
                    &s.engine.renderer.queue,
                    &view,
                    out,
                    size,
                );
                s.engine.renderer.queue.present(frame);
                s.frames += 1;
                if s.fps_timer.elapsed().as_secs_f32() >= 1.0 {
                    s.ui.fps = s.frames;
                    let st = s.engine.renderer.stats;
                    let p = s.engine.camera.position;
                    let target = s
                        .engine
                        .look_target
                        .as_ref()
                        .map(|t| format!(" | [E] {}", t.1))
                        .unwrap_or_default();
                    s.window.set_title(&format!(
                        "VibeRM | {} fps | {} draws, {} inst | pos {:.0},{:.0},{:.0} | {:02}:{:02}{}",
                        s.frames,
                        st.draws,
                        st.instances,
                        p.x,
                        p.y,
                        p.z,
                        s.engine.hour as u32,
                        (s.engine.hour.fract() * 60.0) as u32,
                        target
                    ));
                    log::debug!("{} fps, {:?}", s.frames, st);
                    s.frames = 0;
                    s.fps_timer = Instant::now();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        if let Some(s) = &self.state {
            s.window.request_redraw();
        }
    }
}

fn move_input(keys: &HashSet<KeyCode>) -> MoveInput {
    let k = |c: KeyCode| keys.contains(&c);
    let axis = |p: KeyCode, n: KeyCode| (k(p) as i32 - k(n) as i32) as f32;
    MoveInput {
        forward: axis(KeyCode::KeyW, KeyCode::KeyS),
        right: axis(KeyCode::KeyD, KeyCode::KeyA),
        up: axis(KeyCode::Space, KeyCode::ControlLeft),
        // Skyrim runs by default; Caps Lock would toggle walking, Alt walks while held here.
        run: !k(KeyCode::AltLeft),
        sprint: k(KeyCode::ShiftLeft),
        jump: k(KeyCode::Space),
    }
}
