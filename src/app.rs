//! Application shell: window, input, and the engine state.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use esp::LoadOrder;
use glam::Vec3;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, ElementState, MouseButton, WindowEvent};
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
        None => vfs::locate_data_dir().context("could not find a Skyrim Data directory; pass --data")?,
    };
    log::info!("data directory: {}", data_dir.display());
    let names = LoadOrder::default_plugin_list(&data_dir, None);
    let t = Instant::now();
    let mut lo = LoadOrder::load(&data_dir, &names)?;
    log::info!("load order indexed: {} records in {:?}", lo.record_count(), t.elapsed());
    let vfs = vfs::Vfs::new(&data_dir, &names);
    lo.load_strings("english", |p| vfs.read(p));
    Ok((lo, vfs))
}

async fn create_device(instance: &wgpu::Instance, surface: Option<&wgpu::Surface<'_>>) -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue)> {
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: surface,
        })
        .await?;
    log::info!("adapter: {:?}", adapter.get_info());
    let mut features = wgpu::Features::TEXTURE_COMPRESSION_BC;
    if adapter.features().contains(wgpu::Features::TEXTURE_COMPRESSION_BC_SLICED_3D) {
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

fn setup_engine(opts: &Options, renderer: Renderer, lo: LoadOrder, vfs: vfs::Vfs) -> Result<Engine> {
    let mut engine = Engine::new(lo, vfs, renderer, opts.hour, opts.weather.clone(), opts.radius);
    if let Some(c) = &opts.cell {
        let id = engine.resolve_form(c).with_context(|| format!("unknown cell {c}"))?;
        let exterior = engine.lo.cell(id).and_then(|c| c.world);
        match (exterior, opts.position) {
            (Some(w), Some(p)) => engine.enter_exterior(w, p, opts.yaw.unwrap_or(0.0))?,
            (None, Some(p)) => engine.enter_interior(id, Some((p, opts.yaw.unwrap_or(0.0))))?,
            _ => engine.center_on_cell(id)?,
        }
    } else {
        let wname = opts.world.clone().unwrap_or_else(|| "Tamriel".into());
        let w = engine.resolve_form(&wname).with_context(|| format!("unknown worldspace {wname}"))?;
        let (x, y) = opts.grid.unwrap_or((4, -12));
        let p = opts.position.unwrap_or(Vec3::new((x as f32 + 0.5) * 4096.0, (y as f32 + 0.5) * 4096.0, -100_000.0));
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
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let (_adapter, device, queue) = pollster::block_on(create_device(&instance, None))?;
        let renderer = Renderer::new(device, queue, wgpu::TextureFormat::Rgba8Unorm, opts.width, opts.height);
        let mut engine = setup_engine(&opts, renderer, lo, vfs)?;
        if let Some(n) = opts.use_door {
            let doors = engine.load_doors();
            for d in &doors {
                log::info!("door {} at {:?} -> {:?}", d.ref_id, d.position, d.destination);
            }
            let d = doors.get(n).context("no such door")?;
            let (dest, pos, rot) = d.destination.unwrap();
            engine.teleport_through(dest, pos, rot.z)?;
        }
        if let Some(frames) = opts.wait {
            // Advance the world without moving the camera.
            let (pos, yaw, pitch) = (engine.camera.position, engine.camera.yaw, engine.camera.pitch);
            engine.player.noclip = true;
            for _ in 0..frames {
                engine.update(MoveInput::default(), 1.0 / 60.0, 20.0);
            }
            engine.camera.position = pos;
            engine.camera.yaw = yaw;
            engine.camera.pitch = pitch;
        }
        if let Some(frames) = opts.simulate {
            let input = MoveInput { forward: 1.0, run: true, ..Default::default() };
            for i in 0..frames {
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
            for (t, name) in engine.scene.pick(engine.camera.position, dir).iter().take(8) {
                log::info!("pick: {t:.0} {name}");
            }
        }
        if let Some(n) = opts.bench {
            let t = engine.renderer.bench(&engine.scene, &engine.camera, n);
            log::info!("bench: {:?}/frame ({:.1} fps), {:?}", t, 1.0 / t.as_secs_f64(), engine.renderer.stats);
        }
        let pixels = engine.renderer.render_to_image(&engine.scene, &engine.camera);
        log::info!("render stats: {:?}", engine.renderer.stats);
        let file = std::fs::File::create(&path)?;
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), opts.width, opts.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()?.write_image_data(&pixels)?;
        log::info!("wrote {}", path.display());
        return Ok(());
    }
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App { opts, data: Some((lo, vfs)), state: None };
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct WindowState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    engine: Engine,
    keys: HashSet<KeyCode>,
    grabbed: bool,
    last: Instant,
    fps_timer: Instant,
    frames: u32,
}

struct App {
    opts: Options,
    data: Option<(LoadOrder, vfs::Vfs)>,
    state: Option<WindowState>,
}

impl App {
    fn init(&mut self, el: &ActiveEventLoop) -> Result<()> {
        let window = Arc::new(el.create_window(
            Window::default_attributes()
                .with_title("vibe_rm")
                .with_inner_size(winit::dpi::PhysicalSize::new(self.opts.width, self.opts.height)),
        )?);
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(
            el.owned_display_handle(),
        )));
        let surface = instance.create_surface(window.clone())?;
        let (adapter, device, queue) = pollster::block_on(create_device(&instance, Some(&surface)))?;
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
        let engine = setup_engine(&self.opts, renderer, lo, vfs)?;
        self.state = Some(WindowState {
            window,
            surface,
            config,
            engine,
            keys: HashSet::new(),
            grabbed: false,
            last: Instant::now(),
            fps_timer: Instant::now(),
            frames: 0,
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

    fn device_event(&mut self, _el: &ActiveEventLoop, _id: winit::event::DeviceId, event: DeviceEvent) {
        let Some(s) = &mut self.state else { return };
        if let DeviceEvent::MouseMotion { delta } = event
            && s.grabbed
        {
            let sens = 0.0025;
            s.engine.camera.yaw += delta.0 as f32 * sens;
            s.engine.camera.pitch = (s.engine.camera.pitch - delta.1 as f32 * sens).clamp(-1.55, 1.55);
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(s) = &mut self.state else { return };
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(size) => {
                s.config.width = size.width.max(1);
                s.config.height = size.height.max(1);
                s.surface.configure(&s.engine.renderer.device, &s.config);
                s.engine.renderer.resize(s.config.width, s.config.height);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => {
                            if code == KeyCode::Escape {
                                if s.grabbed {
                                    s.grabbed = false;
                                    let _ = s.window.set_cursor_grab(CursorGrabMode::None);
                                    s.window.set_cursor_visible(true);
                                } else {
                                    el.exit();
                                }
                            }
                            if code == KeyCode::KeyE && !event.repeat
                                && let Err(e) = s.engine.activate()
                            {
                                log::error!("activation failed: {e:#}");
                            }
                            if code == KeyCode::KeyN && !event.repeat {
                                s.engine.player.noclip = !s.engine.player.noclip;
                                log::info!("noclip {}", s.engine.player.noclip);
                            }
                            s.keys.insert(code);
                        }
                        ElementState::Released => {
                            s.keys.remove(&code);
                        }
                    }
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                if !s.grabbed {
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
                let scale = if s.keys.contains(&KeyCode::KeyT) { 2000.0 } else { 20.0 };
                s.engine.update(input, dt, scale);
                let frame = match s.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                    wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                        s.surface.configure(&s.engine.renderer.device, &s.config);
                        return;
                    }
                    _ => return,
                };
                let view = frame.texture.create_view(&Default::default());
                s.engine.renderer.render(&s.engine.scene, &s.engine.camera, &view);
                frame.present();
                s.frames += 1;
                if s.fps_timer.elapsed().as_secs_f32() >= 1.0 {
                    let st = s.engine.renderer.stats;
                    let p = s.engine.camera.position;
                    let target = s.engine.look_target.as_ref().map(|t| format!(" | [E] {}", t.1)).unwrap_or_default();
                    s.window.set_title(&format!(
                        "vibe_rm | {} fps | {} draws, {} inst | pos {:.0},{:.0},{:.0} | {:02}:{:02}{}",
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
