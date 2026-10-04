//! Application shell: window, input, and the engine state.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use esp::{FormId, LoadOrder};
use glam::Vec3;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

use crate::Options;
use crate::render::{Camera, Environment, GpuLight, Instance, Renderer, Scene};
use crate::world::cell::{self, PlacedObject, PointLight};
use crate::world::loader::ModelCache;
use crate::world::records::Lighting;

pub struct Engine {
    pub lo: LoadOrder,
    pub vfs: vfs::Vfs,
    pub renderer: Renderer,
    pub models: ModelCache,
    pub scene: Scene,
    pub camera: Camera,
}

impl Engine {
    pub fn resolve_form(&self, s: &str) -> Option<FormId> {
        if s.len() == 8
            && let Ok(v) = u32::from_str_radix(s, 16)
            && self.lo.locate(FormId(v)).is_some()
        {
            return Some(FormId(v));
        }
        self.lo.find_editor_id(s)
    }

    fn build_scene(&mut self, objects: &[PlacedObject], lights: &[PointLight], env: Environment) {
        let paths: Vec<String> = objects.iter().map(|o| o.model.clone()).collect();
        self.models.load_all(&mut self.renderer, &self.vfs, &paths);
        let mut scene = Scene { env, ..Default::default() };
        for o in objects {
            if let Some(m) = self.models.get(&o.model) {
                scene.instances.push(Instance::new(m, o.transform));
            }
        }
        for l in lights {
            scene.lights.push(GpuLight {
                pos_radius: [l.position.x, l.position.y, l.position.z, l.radius],
                color: [l.color.x, l.color.y, l.color.z, 1.0],
            });
        }
        scene.assign_lights();
        log::info!(
            "scene: {} instances, {} lights, {} models cached, {} textures cached",
            scene.instances.len(),
            scene.lights.len(),
            self.models.len(),
            self.renderer.textures.len()
        );
        self.scene = scene;
    }

    pub fn load_interior(&mut self, cell_id: FormId) -> Result<()> {
        let t = Instant::now();
        let contents = cell::load_cell(&self.lo, cell_id).context("cell not found")?;
        log::info!(
            "cell {} '{}' ({}): {} objects, {} lights",
            contents.info.editor_id,
            contents.info.name,
            cell_id,
            contents.objects.len(),
            contents.lights.len()
        );
        let env = interior_environment(&contents.lighting);
        self.build_scene(&contents.objects, &contents.lights, env);

        // Spawn point: a COC marker if present, else the middle of the cell.
        let coc = self.lo.find_editor_id("COCMarkerHeading");
        let index = self.lo.cell(cell_id).cloned().unwrap_or_default();
        let mut spawn = None;
        for &r in index.persistent.iter().chain(index.temporary.iter()) {
            if let Some(rec) = self.lo.get(r) {
                let rf = crate::world::records::reference(&rec);
                if Some(rf.base) == coc {
                    spawn = Some((rf.position, rf.rotation.z));
                    break;
                }
            }
        }
        let (pos, yaw) = spawn.unwrap_or_else(|| {
            let mut c = Vec3::ZERO;
            for o in &contents.objects {
                c += o.transform.w_axis.truncate();
            }
            (c / contents.objects.len().max(1) as f32, 0.0)
        });
        self.camera.position = pos + Vec3::new(0.0, 0.0, 120.0);
        self.camera.yaw = yaw;
        self.camera.pitch = 0.0;
        log::info!("interior loaded in {:?}", t.elapsed());
        Ok(())
    }

    pub fn load_exterior(&mut self, world: FormId, cx: i32, cy: i32, radius: i32) -> Result<()> {
        let t = Instant::now();
        let wi = self.lo.world(world).context("worldspace not indexed")?.clone();
        let mut objects = Vec::new();
        let mut lights = Vec::new();
        let mut doors = Vec::new();
        for y in cy - radius..=cy + radius {
            for x in cx - radius..=cx + radius {
                if let Some(&cell_id) = wi.cells.get(&(x, y)) {
                    let idx = self.lo.cell(cell_id).cloned().unwrap_or_default();
                    for &r in idx.temporary.iter().chain(idx.persistent.iter()) {
                        cell::add_reference(&self.lo, r, &mut objects, &mut lights, &mut doors);
                    }
                }
            }
        }
        // Persistent references of a worldspace live in one special cell; pick those nearby.
        if let Some(pc) = wi.persistent_cell
            && let Some(idx) = self.lo.cell(pc)
        {
            let mut tmp = Vec::new();
            for &r in idx.persistent.iter().chain(idx.temporary.iter()) {
                cell::add_reference(&self.lo, r, &mut tmp, &mut lights, &mut doors);
            }
            let lo_x = ((cx - radius) * 4096) as f32;
            let hi_x = ((cx + radius + 1) * 4096) as f32;
            let lo_y = ((cy - radius) * 4096) as f32;
            let hi_y = ((cy + radius + 1) * 4096) as f32;
            objects.extend(tmp.into_iter().filter(|o| {
                let p = o.transform.w_axis;
                p.x >= lo_x && p.x < hi_x && p.y >= lo_y && p.y < hi_y
            }));
        }
        let env = Environment {
            sun_dir: Vec3::new(0.4, 0.3, 0.6).normalize(),
            sun_color: Vec3::new(0.9, 0.85, 0.75),
            ambient: Vec3::new(0.35, 0.38, 0.45),
            fog_near_color: Vec3::new(0.6, 0.68, 0.78),
            fog_far_color: Vec3::new(0.6, 0.68, 0.78),
            fog_near: 2000.0,
            fog_far: 60000.0,
            fog_power: 1.0,
            fog_max: 0.8,
            clear_color: Vec3::new(0.6, 0.68, 0.78),
        };
        self.build_scene(&objects, &lights, env);
        self.camera.position = Vec3::new(cx as f32 * 4096.0 + 2048.0, cy as f32 * 4096.0 + 2048.0, 2000.0);
        log::info!("exterior loaded in {:?}", t.elapsed());
        Ok(())
    }
}

fn interior_environment(l: &Lighting) -> Environment {
    Environment {
        sun_dir: -l.directional_dir(),
        sun_color: l.directional,
        ambient: l.ambient,
        fog_near_color: l.fog_near_color,
        fog_far_color: l.fog_far_color,
        fog_near: l.fog_near,
        fog_far: if l.fog_far > l.fog_near { l.fog_far } else { l.fog_near + 1.0 },
        fog_power: if l.fog_power > 0.0 { l.fog_power } else { 1.0 },
        fog_max: if l.fog_max > 0.0 { l.fog_max } else { 1.0 },
        clear_color: l.fog_far_color,
    }
}

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
            required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
            ..Default::default()
        })
        .await?;
    Ok((adapter, device, queue))
}

fn setup_engine(opts: &Options, renderer: Renderer, lo: LoadOrder, vfs: vfs::Vfs) -> Result<Engine> {
    let mut engine = Engine {
        lo,
        vfs,
        renderer,
        models: ModelCache::default(),
        scene: Scene::default(),
        camera: Camera { position: Vec3::ZERO, yaw: 0.0, pitch: 0.0, fov_y: 65f32.to_radians() },
    };
    if let Some(c) = &opts.cell {
        let id = engine.resolve_form(c).with_context(|| format!("unknown cell {c}"))?;
        engine.load_interior(id)?;
    } else {
        let wname = opts.world.clone().unwrap_or_else(|| "Tamriel".into());
        let w = engine.resolve_form(&wname).with_context(|| format!("unknown worldspace {wname}"))?;
        let (x, y) = opts.grid.unwrap_or((5, -3));
        engine.load_exterior(w, x, y, opts.radius)?;
    }
    if let Some(p) = opts.position {
        engine.camera.position = p;
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
                update_camera(&mut s.engine.camera, &s.keys, dt);
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
                    s.window.set_title(&format!(
                        "vibe_rm | {} fps | {} draws, {} inst | pos {:.0},{:.0},{:.0}",
                        s.frames, st.draws, st.instances, p.x, p.y, p.z
                    ));
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

fn update_camera(cam: &mut Camera, keys: &HashSet<KeyCode>, dt: f32) {
    let mut speed = 600.0;
    if keys.contains(&KeyCode::ShiftLeft) {
        speed *= 6.0;
    }
    if keys.contains(&KeyCode::AltLeft) {
        speed *= 0.2;
    }
    let mut v = Vec3::ZERO;
    let f = cam.forward();
    let r = cam.right();
    if keys.contains(&KeyCode::KeyW) {
        v += f;
    }
    if keys.contains(&KeyCode::KeyS) {
        v -= f;
    }
    if keys.contains(&KeyCode::KeyD) {
        v += r;
    }
    if keys.contains(&KeyCode::KeyA) {
        v -= r;
    }
    if keys.contains(&KeyCode::Space) {
        v += Vec3::Z;
    }
    if keys.contains(&KeyCode::ControlLeft) {
        v -= Vec3::Z;
    }
    cam.position += v.normalize_or_zero() * speed * dt;
}
