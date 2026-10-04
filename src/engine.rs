//! Engine state: loaded data, the active location, cell streaming, and gameplay glue.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use anyhow::{Context, Result};
use esp::{FormId, LoadOrder};
use glam::{Vec2, Vec3};
use rapier3d::prelude::ColliderHandle;

use crate::physics::Physics;
use crate::player::Player;
use crate::render::{Camera, CellKey, Environment, GpuLight, Instance, RenderCell, Renderer, Scene};
use crate::world::cell::{self, Door, PlacedObject, PointLight};
use crate::world::loader::{self, ModelCache};
use crate::world::records::{self, Lighting};
use crate::world::terrain::{self, CELL_SIZE, Land};
use crate::world::weather::{self, Climate, Weather};

#[derive(Default)]
struct CellRuntime {
    /// Animation state for each actor in the matching RenderCell, by index.
    actor_anims: Vec<Option<(crate::world::animation::ActorAnim, std::sync::Arc<crate::world::skeleton::Skeleton>)>>,
    colliders: Vec<ColliderHandle>,
    land: Option<Land>,
    lights: Vec<PointLight>,
    doors: Vec<Door>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Location {
    Nowhere,
    Interior(FormId),
    Exterior { world: FormId, center: (i32, i32) },
}

pub struct Engine {
    pub lo: LoadOrder,
    pub vfs: vfs::Vfs,
    pub renderer: Renderer,
    pub models: ModelCache,
    pub scene: Scene,
    pub camera: Camera,
    /// Game time of day in hours.
    pub hour: f32,
    pub sky: Option<(Weather, Climate)>,
    pub forced_weather: Option<String>,
    pub physics: Physics,
    pub player: Player,
    pub location: Location,
    /// Exterior cell load radius.
    pub radius: i32,
    cells: HashMap<CellKey, CellRuntime>,
    /// Persistent references of the current worldspace bucketed by grid cell.
    world_persistent: HashMap<(i32, i32), Vec<FormId>>,
    pending_loads: Vec<(i32, i32)>,
    /// Name of what the crosshair points at, if activatable.
    pub look_target: Option<(FormId, String)>,
    skeletons: HashMap<String, Option<std::sync::Arc<crate::world::skeleton::Skeleton>>>,
    anims: crate::world::animation::AnimationLibrary,
}

pub fn grid_of(p: Vec2) -> (i32, i32) {
    ((p.x / CELL_SIZE).floor() as i32, (p.y / CELL_SIZE).floor() as i32)
}

impl Engine {
    pub fn new(lo: LoadOrder, vfs: vfs::Vfs, renderer: Renderer, hour: f32, weather: Option<String>, radius: i32) -> Self {
        Engine {
            lo,
            vfs,
            renderer,
            models: ModelCache::default(),
            scene: Scene::default(),
            camera: Camera { position: Vec3::ZERO, yaw: 0.0, pitch: 0.0, fov_y: 65f32.to_radians() },
            hour,
            sky: None,
            forced_weather: weather,
            physics: Physics::new(),
            player: Player::new(Vec3::ZERO),
            location: Location::Nowhere,
            radius,
            cells: HashMap::new(),
            world_persistent: HashMap::new(),
            pending_loads: Vec::new(),
            look_target: None,
            skeletons: HashMap::new(),
            anims: Default::default(),
        }
    }

    pub fn resolve_form(&self, s: &str) -> Option<FormId> {
        if s.len() == 8
            && let Ok(v) = u32::from_str_radix(s, 16)
            && self.lo.locate(FormId(v)).is_some()
        {
            return Some(FormId(v));
        }
        self.lo.find_editor_id(s)
    }

    pub fn camera_copy(&self) -> Camera {
        Camera { position: self.camera.position, yaw: self.camera.yaw, pitch: self.camera.pitch, fov_y: self.camera.fov_y }
    }

    fn unload_all(&mut self) {
        self.scene.cells.clear();
        self.scene.lights.clear();
        self.cells.clear();
        self.physics.clear();
        self.pending_loads.clear();
        self.world_persistent.clear();
    }

    /// Place the player with their feet at `feet`, facing `yaw` (radians).
    pub fn place_player(&mut self, feet: Vec3, yaw: f32) {
        self.player = Player::new(Vec3::ZERO);
        self.player.position = feet + Vec3::Z * (self.physics.player_half_height + self.physics.player_radius + 2.0);
        self.camera.yaw = yaw;
        self.camera.pitch = 0.0;
        self.camera.position = self.player.eye();
    }

    // ------------------------------------------------------------------ cells

    /// Instantiate objects into a render cell plus physics, returning runtime data.
    fn instantiate(&mut self, key: CellKey, objects: &[PlacedObject], lights: Vec<PointLight>, doors: Vec<Door>) {
        let paths: Vec<String> = objects.iter().map(|o| o.model.clone()).collect();
        self.models.load_all(&mut self.renderer, &self.vfs, &paths);
        let mut rc = RenderCell::default();
        let mut rt = CellRuntime { lights, doors, ..Default::default() };
        for o in objects {
            if let Some(m) = self.models.get(&o.model) {
                rc.instances.push(Instance::new(m, o.transform));
            }
            if let Some(c) = self.models.collision(&o.model) {
                rt.colliders.extend(self.physics.add_static(&c, o.transform, o.ref_id));
            }
        }
        self.scene.cells.insert(key, rc);
        self.cells.insert(key, rt);
    }

    fn skeleton(&mut self, path: &str) -> Option<std::sync::Arc<crate::world::skeleton::Skeleton>> {
        if let Some(s) = self.skeletons.get(path) {
            return s.clone();
        }
        let s = self
            .vfs
            .read(path)
            .and_then(|d| nif::Nif::parse(&d).ok())
            .map(|n| std::sync::Arc::new(crate::world::skeleton::Skeleton::from_nif(&n)));
        if s.is_none() {
            log::warn!("missing skeleton {path}");
        }
        self.skeletons.insert(path.to_owned(), s.clone());
        s
    }

    /// Spawn actors (ACHR references) into a loaded cell.
    fn spawn_actors(&mut self, key: CellKey, refs: &[FormId]) {
        let mut descs = Vec::new();
        for &r in refs {
            let Some(rec) = self.lo.get(r) else { continue };
            if rec.tag().0 != *b"ACHR" {
                continue;
            }
            if let Some(d) = crate::world::actor::describe_actor(&self.lo, &rec) {
                descs.push(d);
            }
        }
        if descs.is_empty() {
            return;
        }
        let paths: Vec<String> = descs.iter().flat_map(|d| d.models.iter().cloned()).collect();
        self.models.load_all(&mut self.renderer, &self.vfs, &paths);
        let mut actors = Vec::new();
        let mut anims = Vec::new();
        for (ai, d) in descs.iter().enumerate() {
            let Some(skel) = self.skeleton(&d.skeleton) else { continue };
            let clip = crate::world::animation::idle_clip(&d.skeleton, d.female)
                .iter()
                .find_map(|c| self.anims.clip(&self.vfs, c, &d.skeleton, &skel));
            // Desynchronise actors sharing a clip.
            let start = (ai as f32 * 1.618) % 7.0;
            anims.push(clip.map(|c| (crate::world::animation::ActorAnim::new(c, &skel, start), skel.clone())));
            let pose = skel.model_space(&skel.bind_locals());
            let mut meshes = Vec::new();
            for m in &d.models {
                let Some(model) = self.models.get(m) else {
                    log::debug!("{}: missing model {m}", d.name);
                    continue;
                };
                for (pi, part) in model.skinned.iter().enumerate() {
                    let bone_map = part
                        .bone_names
                        .iter()
                        .map(|n| {
                            skel.find(n).unwrap_or_else(|| {
                                log::debug!("{}: bone {n:?} of {m} not in skeleton {}", d.name, d.skeleton);
                                usize::MAX
                            })
                        })
                        .collect();
                    meshes.push(crate::render::ActorMesh { model: model.clone(), part: pi, bone_map });
                }
            }
            log::debug!("actor {} {:?} {} meshes at {:?}", d.ref_id, d.name, meshes.len(), d.transform.w_axis.truncate());
            actors.push(crate::render::ActorInstance {
                meshes,
                attachments: Vec::new(),
                transform: d.transform,
                pose,
                lights: [0xFFFF; 8],
                radius: 120.0,
            });
        }
        log::info!("spawned {} actors", actors.len());
        if let Some(rc) = self.scene.cells.get_mut(&key) {
            rc.actors.extend(actors);
        }
        if let Some(rt) = self.cells.get_mut(&key) {
            rt.actor_anims.extend(anims);
        }
    }

    fn unload_cell(&mut self, key: CellKey) {
        self.scene.cells.remove(&key);
        if let Some(rt) = self.cells.remove(&key) {
            self.physics.remove_colliders(&rt.colliders);
        }
    }

    fn rebuild_lights(&mut self) {
        self.scene.lights = self
            .cells
            .values()
            .flat_map(|c| c.lights.iter())
            .map(|l| GpuLight {
                pos_radius: [l.position.x, l.position.y, l.position.z, l.radius],
                color: [l.color.x, l.color.y, l.color.z, 1.0],
            })
            .collect();
        self.scene.assign_lights();
    }

    pub fn enter_interior(&mut self, cell_id: FormId, spawn: Option<(Vec3, f32)>) -> Result<()> {
        let t = Instant::now();
        self.unload_all();
        let contents = cell::load_cell(&self.lo, cell_id).context("cell not found")?;
        log::info!(
            "entering {} '{}' ({}): {} objects, {} lights",
            contents.info.editor_id,
            contents.info.name,
            cell_id,
            contents.objects.len(),
            contents.lights.len()
        );
        self.sky = None;
        self.scene.env = interior_environment(&contents.lighting);
        let key = CellKey::Interior(cell_id);
        self.instantiate(key, &contents.objects, contents.lights.clone(), contents.doors.clone());
        let refs: Vec<FormId> = self
            .lo
            .cell(cell_id)
            .map(|c| c.persistent.iter().chain(c.temporary.iter()).copied().collect())
            .unwrap_or_default();
        self.spawn_actors(key, &refs);
        if contents.info.has_water
            && let Some(h) = contents.info.water_height
            && h < 1.0e30
        {
            let wt = if contents.info.water_type.is_null() { FormId(0x18) } else { contents.info.water_type };
            self.add_water(key, vec![(-50_000.0, -50_000.0, 100_000.0, h, wt)]);
        }
        self.rebuild_lights();
        self.location = Location::Interior(cell_id);
        self.physics.step(1.0 / 60.0);

        let (pos, yaw) = spawn.unwrap_or_else(|| self.default_interior_spawn(cell_id, &contents.objects));
        self.place_player(pos, yaw);
        log::info!("interior loaded in {:?}", t.elapsed());
        Ok(())
    }

    fn default_interior_spawn(&self, cell_id: FormId, objects: &[PlacedObject]) -> (Vec3, f32) {
        let coc = self.lo.find_editor_id("COCMarkerHeading");
        let index = self.lo.cell(cell_id).cloned().unwrap_or_default();
        for &r in index.persistent.iter().chain(index.temporary.iter()) {
            if let Some(rec) = self.lo.get(r) {
                let rf = records::reference(&rec);
                if Some(rf.base) == coc {
                    return (rf.position, rf.rotation.z);
                }
            }
        }
        let mut c = Vec3::ZERO;
        for o in objects {
            c += o.transform.w_axis.truncate();
        }
        (c / objects.len().max(1) as f32, 0.0)
    }

    pub fn enter_exterior(&mut self, world: FormId, feet: Vec3, yaw: f32) -> Result<()> {
        let t = Instant::now();
        self.unload_all();
        let wi = self.lo.world(world).context("worldspace not indexed")?.clone();
        // Bucket the worldspace's persistent references by grid cell.
        if let Some(pc) = wi.persistent_cell
            && let Some(idx) = self.lo.cell(pc)
        {
            for &r in idx.persistent.iter().chain(idx.temporary.iter()) {
                if let Some(rec) = self.lo.get(r) {
                    let rf = records::reference(&rec);
                    self.world_persistent.entry(grid_of(rf.position.truncate())).or_default().push(r);
                }
            }
        }
        self.setup_weather(world);
        self.scene.env = self.sky_environment().unwrap_or_default();
        let center = grid_of(feet.truncate());
        self.location = Location::Exterior { world, center };
        for y in center.1 - self.radius..=center.1 + self.radius {
            for x in center.0 - self.radius..=center.0 + self.radius {
                self.load_exterior_cell(world, x, y);
            }
        }
        self.rebuild_lights();
        self.physics.step(1.0 / 60.0);
        let mut feet = feet;
        if let Some(h) = self.ground_height(feet.truncate())
            && feet.z < h
        {
            feet.z = h;
        }
        self.place_player(feet, yaw);
        log::info!("exterior loaded in {:?}", t.elapsed());
        Ok(())
    }

    fn load_exterior_cell(&mut self, world: FormId, x: i32, y: i32) {
        let key = CellKey::Exterior(x, y);
        if self.cells.contains_key(&key) {
            return;
        }
        let wi = match self.lo.world(world) {
            Some(w) => w,
            None => return,
        };
        let cell_id = wi.cells.get(&(x, y)).copied();
        let mut objects = Vec::new();
        let mut lights = Vec::new();
        let mut doors = Vec::new();
        if let Some(cid) = cell_id
            && let Some(idx) = self.lo.cell(cid)
        {
            for &r in idx.temporary.iter().chain(idx.persistent.iter()) {
                cell::add_reference(&self.lo, r, &mut objects, &mut lights, &mut doors);
            }
        }
        if let Some(refs) = self.world_persistent.get(&(x, y)) {
            for &r in refs {
                cell::add_reference(&self.lo, r, &mut objects, &mut lights, &mut doors);
            }
        }
        self.instantiate(key, &objects, lights, doors);
        let mut refs: Vec<FormId> = cell_id
            .and_then(|c| self.lo.cell(c))
            .map(|c| c.persistent.iter().chain(c.temporary.iter()).copied().collect())
            .unwrap_or_default();
        refs.extend(self.world_persistent.get(&(x, y)).cloned().unwrap_or_default());
        self.spawn_actors(key, &refs);

        // Landscape
        let dnam = self.world_field(world, b"DNAM", 0x1).map(|d| d.0);
        let default_height = dnam.as_ref().map(|d| f32::from_le_bytes(d[0..4].try_into().unwrap())).unwrap_or(-2048.0);
        let default_water = dnam.as_ref().map(|d| f32::from_le_bytes(d[4..8].try_into().unwrap())).unwrap_or(0.0);
        let world_water = self.world_form(world, b"NAM2", 0x8);
        let land = cell_id
            .and_then(|c| self.lo.cell(c).and_then(|c| c.land))
            .and_then(|l| terrain::load_land(&self.lo, l, x, y, default_height));
        if let Some(l) = land {
            let mut tex = HashSet::new();
            for q in &l.quadrants {
                for layer in &q.layers {
                    if !self.renderer.textures.contains(&layer.diffuse) {
                        tex.insert(layer.diffuse.clone());
                    }
                    if let Some(n) = &layer.normal
                        && !self.renderer.textures.contains(n)
                    {
                        tex.insert(n.clone());
                    }
                }
            }
            loader::load_textures(&mut self.renderer, &self.vfs, tex.into_iter().collect());
            let chunks = self.renderer.build_terrain(&l);
            if let Some(rc) = self.scene.cells.get_mut(&key) {
                rc.terrain = chunks;
            }
            let h = self.physics.add_terrain(&l);
            if let Some(rt) = self.cells.get_mut(&key) {
                rt.colliders.extend(h);
                rt.land = Some(l);
            }
        }

        // Water
        if let Some(cid) = cell_id
            && let Some(info) = records::cell_info(&self.lo, cid)
        {
            let h = info.water_height.unwrap_or(default_water);
            if h < 1.0e30 {
                let wt = if info.water_type.is_null() { world_water.unwrap_or(FormId(0x18)) } else { info.water_type };
                self.add_water(key, vec![(x as f32 * CELL_SIZE, y as f32 * CELL_SIZE, CELL_SIZE, h, wt)]);
            }
        }
    }

    fn add_water(&mut self, key: CellKey, planes: Vec<(f32, f32, f32, f32, FormId)>) {
        let mut params = Vec::new();
        for (x, y, size, h, wt) in planes {
            if let Some(p) = records::water_params(&self.lo, wt) {
                params.push((x, y, size, h, p));
            }
        }
        let tex: Vec<String> = params
            .iter()
            .map(|p| p.4.noise_texture.clone())
            .filter(|t| !self.renderer.textures.contains(t))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        loader::load_textures(&mut self.renderer, &self.vfs, tex);
        for (x, y, size, h, p) in params {
            let plane = self.renderer.build_water(x, y, size, h, &p);
            if let Some(rc) = self.scene.cells.get_mut(&key) {
                rc.water.push(plane);
            }
        }
    }

    /// Exterior streaming: keep cells around the player loaded. Loads at most a
    /// couple of cells per call to bound frame hitches.
    pub fn update_streaming(&mut self) {
        let Location::Exterior { world, center } = self.location else { return };
        let g = grid_of(self.player.position.truncate());
        if g != center {
            self.location = Location::Exterior { world, center: g };
            let r = self.radius;
            let keep = |k: &CellKey| match *k {
                CellKey::Exterior(x, y) => (x - g.0).abs() <= r + 1 && (y - g.1).abs() <= r + 1,
                _ => false,
            };
            let to_unload: Vec<CellKey> = self.cells.keys().filter(|k| !keep(k)).copied().collect();
            for k in &to_unload {
                self.unload_cell(*k);
            }
            self.pending_loads.clear();
            for y in g.1 - r..=g.1 + r {
                for x in g.0 - r..=g.0 + r {
                    if !self.cells.contains_key(&CellKey::Exterior(x, y)) {
                        self.pending_loads.push((x, y));
                    }
                }
            }
            // Nearest first.
            self.pending_loads.sort_by_key(|(x, y)| std::cmp::Reverse((x - g.0).abs() + (y - g.1).abs()));
            if !to_unload.is_empty() {
                self.rebuild_lights();
            }
        }
        let mut loaded = false;
        for _ in 0..2 {
            let Some((x, y)) = self.pending_loads.pop() else { break };
            self.load_exterior_cell(world, x, y);
            loaded = true;
        }
        if loaded {
            self.rebuild_lights();
        }
    }

    pub fn ground_height(&self, p: Vec2) -> Option<f32> {
        let (x, y) = grid_of(p);
        self.cells.get(&CellKey::Exterior(x, y)).and_then(|c| c.land.as_ref()).map(|l| l.height_at(p))
    }

    /// Read a worldspace subrecord, following the parent worldspace when the
    /// matching "use parent" flag (PNAM) is set.
    pub fn world_field(&self, world: FormId, tag: &[u8; 4], parent_flag: u16) -> Option<(Vec<u8>, FormId)> {
        let mut w = world;
        for _ in 0..4 {
            let rec = self.lo.get(w)?;
            let parent = rec.get(b"WNAM").map(|d| rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))));
            let flags = rec.get(b"PNAM").map(|d| u16::from_le_bytes([d[0], d[1]])).unwrap_or(0);
            match parent {
                Some(p) if flags & parent_flag != 0 => w = p,
                _ => {
                    // FormIDs inside the field must be globalised by the caller with this record.
                    return rec.get(tag).map(|d| (d.to_vec(), w));
                }
            }
        }
        None
    }

    fn world_form(&self, world: FormId, tag: &[u8; 4], parent_flag: u16) -> Option<FormId> {
        let (d, w) = self.world_field(world, tag, parent_flag)?;
        let rec = self.lo.get(w)?;
        Some(rec.fid(FormId(u32::from_le_bytes(d.get(0..4)?.try_into().ok()?))))
    }

    // ---------------------------------------------------------------- weather

    /// Pick the climate's weather for a worldspace and load its sky textures.
    pub fn setup_weather(&mut self, world: FormId) {
        let clmt = self.world_form(world, b"CNAM", 0x10);
        let climate = clmt.and_then(|c| weather::load_climate(&self.lo, c)).unwrap_or_default();
        let forced = self.forced_weather.as_ref().and_then(|w| self.resolve_form(w));
        let wid = forced.or_else(|| climate.weathers.iter().max_by_key(|w| w.1).map(|w| w.0));
        let Some(w) = wid.and_then(|w| weather::load_weather(&self.lo, w)) else {
            self.sky = None;
            return;
        };
        log::info!("weather {} ({} cloud layers)", w.editor_id, w.clouds.len());
        let mut tex: Vec<String> = vec![climate.sun_texture.clone()];
        tex.extend(w.clouds.iter().take(4).map(|c| c.texture.clone()));
        let missing: Vec<String> = tex.iter().filter(|t| !self.renderer.textures.contains(t)).cloned().collect();
        loader::load_textures(&mut self.renderer, &self.vfs, missing);
        let get = |p: &String| self.renderer.textures.get(p).flatten();
        let sun = get(&climate.sun_texture).unwrap_or_else(|| self.renderer.white.clone());
        let clouds = w.clouds.iter().take(4).filter_map(|c| get(&c.texture)).collect();
        let (dev, sampler, black) = (&self.renderer.device, &self.renderer.sampler, self.renderer.black.clone());
        self.renderer.sky.set_textures(dev, sampler, sun, clouds, black);
        self.sky = Some((w, climate));
    }

    /// Evaluate the current weather at the current hour into a render environment.
    pub fn sky_environment(&mut self) -> Option<Environment> {
        let (w, c) = self.sky.as_ref()?;
        let st = weather::evaluate(w, c, self.hour);
        let env = Environment {
            sun_dir: st.light_dir,
            sun_color: st.sunlight,
            ambient: st.ambient,
            fog_near_color: st.fog_near_color,
            fog_far_color: st.fog_far_color,
            fog_near: st.fog_near,
            fog_far: st.fog_far.max(st.fog_near + 1.0),
            fog_power: st.fog_power,
            fog_max: st.fog_max,
            clear_color: st.horizon,
            dalc: Some(st.dalc),
            sky: true,
        };
        self.renderer.sky.set_state(st);
        Some(env)
    }

    // --------------------------------------------------------------- gameplay

    /// Advance the simulation by `dt` seconds.
    pub fn update(&mut self, input: crate::player::MoveInput, dt: f32, time_scale: f32) {
        self.renderer.time += dt;
        self.hour = (self.hour + dt * time_scale / 3600.0).rem_euclid(24.0);
        if let Some(env) = self.sky_environment() {
            self.scene.env = env;
        }
        self.physics.step(dt);
        self.animate_actors(dt);
        let cam = self.camera_copy();
        self.player.update(&self.physics, &cam, input, dt);
        self.camera.position = self.player.eye();
        self.update_streaming();
        self.update_look_target();
    }

    fn animate_actors(&mut self, dt: f32) {
        for (key, rt) in self.cells.iter_mut() {
            let Some(rc) = self.scene.cells.get_mut(key) else { continue };
            for (actor, anim) in rc.actors.iter_mut().zip(rt.actor_anims.iter_mut()) {
                if let Some((a, skel)) = anim {
                    actor.pose = a.update(skel, dt);
                }
            }
        }
    }

    fn update_look_target(&mut self) {
        self.look_target = None;
        let Some((_, Some(owner))) = self.physics.raycast(self.camera.position, self.camera.forward(), 220.0) else {
            return;
        };
        let Some(rec) = self.lo.get(owner) else { return };
        let rf = records::reference(&rec);
        let Some(base) = self.lo.get(rf.base) else { return };
        let mut name = base.get(b"FULL").map(|d| self.lo.lstring(&base, d)).unwrap_or_default();
        if base.tag().0 == *b"DOOR"
            && let Some((dest, _, _)) = rf.teleport
        {
            let target = self.lo.cell_of_ref(dest).and_then(|c| records::cell_info(&self.lo, c));
            let cname = target
                .map(|c| if c.name.is_empty() { c.editor_id } else { c.name })
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| "Skyrim".into());
            name = format!("Door to {cname}");
        }
        if !name.is_empty() {
            self.look_target = Some((owner, name));
        }
    }

    /// Activate whatever the player is looking at.
    pub fn activate(&mut self) -> Result<()> {
        let Some((owner, name)) = self.look_target.clone() else { return Ok(()) };
        let rec = self.lo.get(owner).context("reference vanished")?;
        let rf = records::reference(&rec);
        let base_tag = self.lo.tag_of(rf.base);
        log::info!("activate {owner} ({name})");
        if base_tag.map(|t| t.0) == Some(*b"DOOR")
            && let Some((dest, pos, rot)) = rf.teleport
        {
            self.teleport_through(dest, pos, rot.z)?;
        }
        Ok(())
    }

    /// Move the player to a door destination (as stored in XTEL).
    pub fn teleport_through(&mut self, dest_door: FormId, pos: Vec3, yaw: f32) -> Result<()> {
        let cell_id = self.lo.cell_of_ref(dest_door).context("destination door has no cell")?;
        let idx = self.lo.cell(cell_id).cloned().unwrap_or_default();
        match idx.world {
            Some(world) => self.enter_exterior(world, pos, yaw),
            None => self.enter_interior(cell_id, Some((pos, yaw))),
        }
    }

    /// Doors with teleport destinations in the loaded cells.
    pub fn load_doors(&self) -> Vec<Door> {
        self.cells.values().flat_map(|c| c.doors.iter()).filter(|d| d.destination.is_some()).cloned().collect()
    }

    /// `coc`-style entry: an interior cell, or an exterior cell by editor id.
    pub fn center_on_cell(&mut self, id: FormId) -> Result<()> {
        let idx = self.lo.cell(id).cloned().unwrap_or_default();
        match (idx.world, idx.grid) {
            (Some(w), Some((x, y))) => {
                let p = Vec3::new(x as f32 * CELL_SIZE + CELL_SIZE * 0.5, y as f32 * CELL_SIZE + CELL_SIZE * 0.5, -100_000.0);
                self.enter_exterior(w, p, 0.0)
            }
            _ => self.enter_interior(id, None),
        }
    }
}

pub fn interior_environment(l: &Lighting) -> Environment {
    Environment {
        dalc: l.dalc,
        sky: false,
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
