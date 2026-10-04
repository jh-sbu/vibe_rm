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
    /// References instantiated in this cell (for script detach / visibility).
    refs: Vec<FormId>,
    /// Looping ambient sounds started for this cell.
    sounds: Vec<crate::audio::VoiceId>,
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
    pub scripts: crate::script::ScriptState,
    pub vm: papyrus::Vm,
    /// Whole game days elapsed before the current day.
    pub day: u32,
    rng: u64,
    pending_moveto: Option<FormId>,
    pub audio: Option<crate::audio::Audio>,
    music: MusicState,
    pub conversation: Option<crate::dialogue::Conversation>,
    npc_refs: HashMap<FormId, FormId>,
}

#[derive(Default)]
struct MusicState {
    music_type: Option<FormId>,
    voice: Option<crate::audio::VoiceId>,
    /// Real time at which to start the next track.
    next_at: f64,
}

/// The player character reference ("PlayerRef").
pub const PLAYER_REF: FormId = FormId(0x14);
/// Real seconds per game hour at the default timescale of 20.
const TIMESCALE: f64 = 20.0;

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
            scripts: Default::default(),
            vm: papyrus::Vm::new(),
            day: 0,
            rng: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1) | 1,
            pending_moveto: None,
            audio: None,
            music: MusicState::default(),
            conversation: None,
            npc_refs: HashMap::new(),
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

    /// Start looping sounds emitted by objects (sound markers, lights, activators, ...).
    fn start_cell_sounds(&mut self, key: CellKey, refs: &[FormId]) {
        let Some(audio) = self.audio.as_mut() else { return };
        let mut voices = Vec::new();
        for &r in refs {
            let Some(rec) = self.lo.get(r) else { continue };
            let rf = records::reference(&rec);
            if rf.deleted() || rf.initially_disabled() {
                continue;
            }
            let Some(base) = self.lo.get(rf.base) else { continue };
            let snd = match &base.tag().0 {
                b"SOUN" => Some(rf.base),
                b"LIGH" | b"ACTI" | b"MSTT" | b"FURN" => base.get(b"SNAM").map(|d| base.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap())))),
                _ => None,
            };
            let Some(desc) = snd.and_then(|s| crate::world::sound::descriptor(&self.lo, &self.vfs, s)) else { continue };
            if !desc.looping {
                continue;
            }
            let file = &desc.files[r.0 as usize % desc.files.len()];
            if let Some(v) = audio.play(&self.vfs, file, desc.volume, true, Some(rf.position), desc.min_dist, desc.max_dist) {
                voices.push(v);
            }
        }
        if let Some(rt) = self.cells.get_mut(&key) {
            rt.sounds.extend(voices);
        }
    }

    /// Music type for the current location: cell XCMO, else the cell's regions.
    fn location_music(&self) -> Option<FormId> {
        let cell = match self.location {
            Location::Interior(c) => Some(c),
            Location::Exterior { world, center } => self.lo.world(world).and_then(|w| w.cells.get(&center).copied()),
            Location::Nowhere => None,
        }?;
        let rec = self.lo.get(cell)?;
        if let Some(d) = rec.get(b"XCMO") {
            return Some(rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().ok()?))));
        }
        let regions: Vec<FormId> =
            rec.get(b"XCLR").map(|d| d.chunks_exact(4).map(|c| rec.fid(FormId(u32::from_le_bytes(c.try_into().unwrap())))).collect()).unwrap_or_default();
        for r in regions {
            let Some(reg) = self.lo.get(r) else { continue };
            let mut in_music = false;
            for sr in reg.subrecords() {
                match &sr.tag.0 {
                    b"RDAT" => in_music = sr.u32(0) == 7,
                    b"RDMO" if in_music => return Some(reg.fid(sr.form_id(0))),
                    _ => {}
                }
            }
        }
        None
    }

    /// Choose a track file from a music type, honouring track conditions.
    fn pick_track(&mut self, music: FormId) -> Option<String> {
        let rec = self.lo.get(music)?;
        let tracks: Vec<FormId> = rec.get(b"TNAM")?.chunks_exact(4).map(|c| rec.fid(FormId(u32::from_le_bytes(c.try_into().unwrap())))).collect();
        drop(rec);
        let ctx = crate::condition::Context { subject: Some(PLAYER_REF), ..Default::default() };
        let mut candidates: Vec<FormId> = tracks
            .into_iter()
            .filter(|t| self.lo.get(*t).is_some_and(|r| crate::condition::evaluate(self, &crate::condition::parse_all(&r), ctx)))
            .collect();
        log::debug!("music candidates: {}", candidates.len());
        for _ in 0..4 {
            if candidates.is_empty() {
                return None;
            }
            let i = (self.rand() % candidates.len() as u64) as usize;
            let t = self.lo.get(candidates[i])?;
            let kind = t.get(b"CNAM").map(|d| u32::from_le_bytes(d[0..4].try_into().unwrap())).unwrap_or(crate::world::sound::MUST_SINGLE);
            if kind == crate::world::sound::MUST_PALETTE {
                candidates = t.get(b"SNAM").map(|d| d.chunks_exact(4).map(|c| t.fid(FormId(u32::from_le_bytes(c.try_into().unwrap())))).collect()).unwrap_or_default();
                continue;
            }
            if let Some(f) = t.get(b"ANAM") {
                return Some(crate::world::sound::sound_path(&esp::decode_zstring(f), &self.vfs));
            }
            log::debug!("music: silent track {}", t.editor_id().unwrap_or_default());
            return None;
        }
        None
    }

    fn update_music(&mut self) {
        if self.audio.is_none() {
            return;
        }
        let now = self.scripts.real_time;
        let want = self.location_music();
        let playing = self.music.voice.is_some_and(|v| self.audio.as_ref().unwrap().is_playing(v));
        if want != self.music.music_type {
            if let Some(v) = self.music.voice.take() {
                self.audio.as_ref().unwrap().stop(v);
            }
            self.music.music_type = want;
            log::debug!("music type -> {:?}", want.and_then(|w| self.lo.get(w)).and_then(|r| r.editor_id()));
            self.music.next_at = now + 1.0;
            return;
        }
        if !playing && now >= self.music.next_at {
            self.music.voice = None;
            if let Some(m) = want
                && let Some(track) = self.pick_track(m)
            {
                log::info!("music: {track}");
                let vfs = &self.vfs;
                self.music.voice = self.audio.as_mut().unwrap().play(vfs, &track, 0.45, false, None, 0.0, 0.0);
            }
            // Silence between tracks, like the original.
            self.music.next_at = now + 20.0 + (self.rand() % 40) as f64;
        }
    }

    fn unload_all(&mut self) {
        if let Some(a) = self.audio.as_mut() {
            a.stop_all();
        }
        self.music.voice = None;
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
            rt.refs.push(o.ref_id);
            if let Some(m) = self.models.get(&o.model) {
                let mut inst = Instance::new(m, o.transform);
                inst.ref_id = o.ref_id.0;
                inst.hidden = self.is_disabled(o.ref_id);
                rc.instances.push(inst);
            }
            if let Some(c) = self.models.collision(&o.model) {
                rt.colliders.extend(self.physics.add_static(&c, o.transform, o.ref_id));
            }
        }
        self.scene.cells.insert(key, rc);
        self.cells.insert(key, rt);
    }

    /// Attach scripts for references in a newly loaded cell and send load events.
    fn attach_cell_scripts(&mut self, refs: &[FormId]) {
        let mut vm = std::mem::take(&mut self.vm);
        for &r in refs {
            let Some(rec) = self.lo.get(r) else { continue };
            let base = records::reference(&rec).base;
            let mut scripts = crate::script::vmad::parse(&rec).map(|v| v.scripts).unwrap_or_default();
            if let Some(b) = self.lo.get(base)
                && let Some(bv) = crate::script::vmad::parse(&b)
            {
                for s in bv.scripts {
                    if !scripts.iter().any(|x| x.name.eq_ignore_ascii_case(&s.name)) {
                        scripts.push(s);
                    }
                }
            }
            if scripts.is_empty() {
                continue;
            }
            let obj = papyrus::ObjectId::Form(r.0);
            {
                let mut host = crate::script::EngineHost { engine: self };
                for s in &scripts {
                    let props: Vec<(String, papyrus::Value)> = s
                        .properties
                        .iter()
                        .map(|(n, pv)| (n.clone(), crate::script::vmad::to_value(pv, &|f| host.engine.native_class(f))))
                        .collect();
                    vm.attach(&mut host, obj, &s.name, &props);
                }
                if host.engine.scripts.initialized.insert(obj) {
                    vm.send_event(&mut host, obj, "OnInit", vec![]);
                }
                vm.send_event(&mut host, obj, "OnLoad", vec![]);
                vm.send_event(&mut host, obj, "OnCellAttach", vec![]);
            }
        }
        self.vm = vm;
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
        let mut capsules = Vec::new();
        for d in &descs {
            let (scale, _, feet) = d.transform.to_scale_rotation_translation();
            capsules.push(self.physics.add_actor_capsule(feet, scale.x, d.ref_id));
        }
        if let Some(rc) = self.scene.cells.get_mut(&key) {
            rc.actors.extend(actors);
        }
        if let Some(rt) = self.cells.get_mut(&key) {
            rt.actor_anims.extend(anims);
            rt.colliders.extend(capsules);
        }
    }

    fn unload_cell(&mut self, key: CellKey) {
        self.scene.cells.remove(&key);
        if let Some(rt) = self.cells.remove(&key) {
            self.physics.remove_colliders(&rt.colliders);
            if let Some(a) = &self.audio {
                for v in &rt.sounds {
                    a.stop(*v);
                }
            }
            for r in &rt.refs {
                self.vm.detach_all(papyrus::ObjectId::Form(r.0));
            }
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
        self.attach_cell_scripts(&refs);
        self.start_cell_sounds(key, &refs);
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
        self.attach_cell_scripts(&refs);
        self.start_cell_sounds(key, &refs);

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
        let h = self.hour + dt * time_scale / 3600.0;
        if h >= 24.0 {
            self.day += 1;
        }
        self.hour = h.rem_euclid(24.0);
        self.update_scripts(dt);
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
        if let Some(a) = self.audio.as_mut() {
            a.set_listener(self.camera.position, self.camera.right());
            a.update();
        }
        self.update_music();
        self.update_conversation();
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

    /// The verb shown in the activation prompt.
    pub fn look_verb(&self) -> &'static str {
        let Some((id, _)) = &self.look_target else { return "" };
        let Some(rec) = self.lo.get(*id) else { return "Activate" };
        if rec.tag().0 == *b"ACHR" {
            return "Talk";
        }
        let base = records::reference(&rec).base;
        match self.lo.tag_of(base).map(|t| t.0) {
            Some(t) if t == *b"DOOR" => "Open",
            Some(t) if t == *b"CONT" => "Search",
            Some(t) if t == *b"FURN" => "Sit",
            Some(t) if t == *b"BOOK" => "Read",
            Some(t) if t == *b"FLOR" => "Harvest",
            Some(t) if t == *b"ACTI" => "Activate",
            _ => "Take",
        }
    }

    pub fn location_name(&self) -> String {
        match self.location {
            Location::Interior(c) | Location::Exterior { world: c, .. } => self
                .lo
                .get(c)
                .map(|r| {
                    let n = r.get(b"FULL").map(|d| self.lo.lstring(&r, d)).unwrap_or_default();
                    if n.is_empty() { r.editor_id().unwrap_or_default() } else { n }
                })
                .unwrap_or_default(),
            Location::Nowhere => String::new(),
        }
    }

    /// Activate whatever the player is looking at.
    pub fn activate(&mut self) -> Result<()> {
        let Some((owner, name)) = self.look_target.clone() else { return Ok(()) };
        let rec = self.lo.get(owner).context("reference vanished")?;
        let is_actor = rec.tag().0 == *b"ACHR";
        let rf = records::reference(&rec);
        drop(rec);
        let base_tag = self.lo.tag_of(rf.base);
        log::info!("activate {owner} ({name})");
        {
            let mut vm = std::mem::take(&mut self.vm);
            let player = self.object_value(PLAYER_REF);
            let mut host = crate::script::EngineHost { engine: self };
            vm.send_event(&mut host, papyrus::ObjectId::Form(owner.0), "OnActivate", vec![player]);
            self.vm = vm;
        }
        if self.scripts.blocked_activation.contains(&owner) {
            return Ok(());
        }
        if is_actor {
            self.start_conversation(owner);
            return Ok(());
        }
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

impl Engine {
    // ----------------------------------------------------------- scripting

    pub fn rand(&mut self) -> u64 {
        // xorshift64*
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Game time in days (Papyrus GetCurrentGameTime).
    pub fn game_days(&self) -> f32 {
        self.day as f32 + self.hour / 24.0
    }

    pub fn game_hours_total(&self) -> f64 {
        self.day as f64 * 24.0 + self.hour as f64
    }

    pub fn native_class(&self, id: FormId) -> &'static str {
        if id == PLAYER_REF {
            return "Actor";
        }
        self.lo.tag_of(id).map(|t| crate::script::types::class_for_tag(&t.0)).unwrap_or("Form")
    }

    pub fn object_value(&self, id: FormId) -> papyrus::Value {
        if id.is_null() || (self.lo.locate(id).is_none() && id != PLAYER_REF) {
            return papyrus::Value::None;
        }
        papyrus::Value::Object(papyrus::ObjectId::Form(id.0), self.native_class(id).into())
    }

    pub fn form_from_file(&self, local: u32, file: &str) -> Option<FormId> {
        let p = self.lo.plugins().iter().find(|p| p.plugin.name().eq_ignore_ascii_case(file))?;
        let id = match p.slot {
            esp::Slot::Full(i) => FormId(((i as u32) << 24) | (local & 0x00FF_FFFF)),
            esp::Slot::Light(j) => FormId(0xFE00_0000 | ((j as u32) << 12) | (local & 0xFFF)),
        };
        self.lo.locate(id).map(|_| id)
    }

    pub fn form_name(&self, id: FormId) -> String {
        let Some(rec) = self.lo.get(id) else { return String::new() };
        let rec = if matches!(&rec.tag().0, b"REFR" | b"ACHR") {
            match self.lo.get(records::reference(&rec).base) {
                Some(b) => b,
                None => return String::new(),
            }
        } else {
            rec
        };
        rec.get(b"FULL").map(|d| self.lo.lstring(&rec, d)).unwrap_or_default()
    }

    pub fn has_keyword(&self, form: FormId, kw: FormId) -> bool {
        let Some(rec) = self.lo.get(form) else { return false };
        let rec = if matches!(&rec.tag().0, b"REFR" | b"ACHR") {
            match self.lo.get(records::reference(&rec).base) {
                Some(b) => b,
                None => return false,
            }
        } else {
            rec
        };
        rec.get(b"KWDA").is_some_and(|d| d.chunks_exact(4).any(|c| rec.fid(FormId(u32::from_le_bytes(c.try_into().unwrap()))) == kw))
    }

    pub fn base_of(&self, r: FormId) -> Option<FormId> {
        if r == PLAYER_REF {
            return Some(FormId(0x7));
        }
        let rec = self.lo.get(r)?;
        Some(records::reference(&rec).base)
    }

    pub fn ref_position(&self, r: FormId) -> Option<Vec3> {
        if r == PLAYER_REF {
            return Some(self.player.position - Vec3::Z * (self.physics.player_half_height + self.physics.player_radius));
        }
        let rec = self.lo.get(r)?;
        Some(records::reference(&rec).position)
    }

    pub fn linked_ref(&self, r: FormId, keyword: Option<FormId>) -> Option<FormId> {
        let rec = self.lo.get(r)?;
        for sr in rec.subrecords() {
            if sr.tag.0 == *b"XLKR" {
                let (kw, target) = if sr.data.len() >= 8 {
                    (rec.fid(sr.form_id(0)), rec.fid(sr.form_id(4)))
                } else {
                    (FormId::NULL, rec.fid(sr.form_id(0)))
                };
                if keyword.is_none_or(|k| k == kw) {
                    return Some(target);
                }
            }
        }
        None
    }

    pub fn npc_is_female(&self, npc: FormId) -> bool {
        self.lo.get(npc).and_then(|r| r.get(b"ACBS").map(|d| d[0] & 1 != 0)).unwrap_or(false)
    }

    pub fn global_value(&self, g: FormId) -> f32 {
        if let Some(v) = self.scripts.globals.get(&g) {
            return *v;
        }
        if let Some(rec) = self.lo.get(g) {
            // Well-known time globals.
            match rec.editor_id().as_deref() {
                Some("GameHour") => return self.hour,
                Some("GameDaysPassed") => return self.game_days(),
                _ => {}
            }
            if let Some(d) = rec.get(b"FLTV") {
                return f32::from_le_bytes(d[0..4].try_into().unwrap());
            }
        }
        0.0
    }

    pub fn message_text(&self, m: FormId) -> String {
        let Some(rec) = self.lo.get(m) else { return String::new() };
        rec.get(b"DESC").map(|d| self.lo.lstring(&rec, d)).unwrap_or_default()
    }

    pub fn formlist(&self, f: FormId) -> Vec<FormId> {
        let Some(rec) = self.lo.get(f) else { return Vec::new() };
        rec.subrecords().filter(|s| s.tag.0 == *b"LNAM").map(|s| rec.fid(s.form_id(0))).collect()
    }

    pub fn objective_text(&self, q: FormId, objective: i32) -> String {
        let Some(rec) = self.lo.get(q) else { return String::new() };
        let mut current = None;
        for sr in rec.subrecords() {
            match &sr.tag.0 {
                b"QOBJ" => current = Some(sr.u16(0) as i32),
                b"NNAM" if current == Some(objective) => return self.lo.lstring(&rec, sr.data),
                _ => {}
            }
        }
        String::new()
    }

    pub fn is_disabled(&self, r: FormId) -> bool {
        if let Some(d) = self.scripts.disabled.get(&r) {
            return *d;
        }
        self.lo.get(r).is_some_and(|rec| rec.flags() & esp::record_flags::INITIALLY_DISABLED != 0)
    }

    pub fn set_disabled(&mut self, r: FormId, disabled: bool) {
        self.scripts.disabled.insert(r, disabled);
        for rc in self.scene.cells.values_mut() {
            for i in rc.instances.iter_mut().filter(|i| i.ref_id == r.0) {
                i.hidden = disabled;
            }
        }
        self.physics.set_owner_enabled(r, !disabled);
    }

    pub fn queue_player_moveto(&mut self, target: FormId) {
        self.pending_moveto = Some(target);
    }

    /// Start a quest: mark running, attach its scripts, send OnInit and run its startup stage.
    pub fn start_quest(&mut self, q: FormId) {
        let st = self.scripts.quests.entry(q).or_default();
        if st.running {
            return;
        }
        st.running = true;
        let Some(rec) = self.lo.get(q) else { return };
        let vmad = crate::script::vmad::parse(&rec).unwrap_or_default();
        let alias_specs = alias_specs(&rec);
        // Startup stage: INDX flags (third byte) 0x2 marks "start up stage".
        let startup = rec.subrecords().find(|sr| sr.tag.0 == *b"INDX" && sr.u8(2) & 0x2 != 0).map(|sr| sr.u16(0));
        drop(rec);
        let aliases = self.fill_aliases(&alias_specs);
        self.scripts.quests.entry(q).or_default().aliases = aliases;
        let mut vm = std::mem::take(&mut self.vm);
        {
            let obj = papyrus::ObjectId::Form(q.0);
            // Alias scripts run on the alias objects.
            for (alias, scripts) in &vmad.alias_scripts {
                let aobj = papyrus::ObjectId::Alias { quest: q.0, alias: *alias };
                for s in scripts {
                    let props: Vec<(String, papyrus::Value)> = s
                        .properties
                        .iter()
                        .map(|(n, pv)| (n.clone(), crate::script::vmad::to_value(pv, &|f| host_class(&self.lo, f))))
                        .collect();
                    let mut host = crate::script::EngineHost { engine: self };
                    vm.attach(&mut host, aobj, &s.name, &props);
                }
            }
            let mut host = crate::script::EngineHost { engine: self };
            for s in &vmad.scripts {
                let props: Vec<(String, papyrus::Value)> = s
                    .properties
                    .iter()
                    .map(|(n, pv)| (n.clone(), crate::script::vmad::to_value(pv, &|f| host.engine.native_class(f))))
                    .collect();
                vm.attach(&mut host, obj, &s.name, &props);
            }
            vm.send_event(&mut host, obj, "OnInit", vec![]);
        }
        self.vm = vm;
        if let Some(s) = startup {
            self.scripts.pending_stages.push((q, s));
        }
    }

    /// Set a quest stage: record it and run the stage's fragment.
    fn run_stage(&mut self, q: FormId, stage: u16) {
        {
            let st = self.scripts.quests.entry(q).or_default();
            st.stage = stage;
            st.done.insert(stage);
        }
        let Some(rec) = self.lo.get(q) else { return };
        let Some(vmad) = crate::script::vmad::parse(&rec) else { return };
        let edid = rec.editor_id().unwrap_or_default();
        drop(rec);
        log::info!("quest {edid} stage {stage}");
        let mut vm = std::mem::take(&mut self.vm);
        {
            let mut host = crate::script::EngineHost { engine: self };
            for f in vmad.fragments.iter().filter(|f| f.stage == stage) {
                vm.call_method(&mut host, papyrus::ObjectId::Form(q.0), &f.script, &f.function, vec![]);
            }
        }
        self.vm = vm;
    }

    /// Resolve reference aliases (forced refs and unique actors).
    fn fill_aliases(&mut self, specs: &[(u32, Option<FormId>, Option<FormId>)]) -> HashMap<u32, FormId> {
        let mut out = HashMap::new();
        for &(id, forced, unique) in specs {
            if let Some(r) = forced.or_else(|| unique.and_then(|npc| self.unique_actor_ref(npc))) {
                out.insert(id, r);
            }
        }
        out
    }

    /// The placed reference of a unique NPC.
    pub fn unique_actor_ref(&mut self, npc: FormId) -> Option<FormId> {
        if self.npc_refs.is_empty() {
            for &a in self.lo.ids_of_type(b"ACHR") {
                if let Some(r) = self.lo.get(a)
                    && let Some(d) = r.get(b"NAME")
                {
                    let base = r.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap())));
                    self.npc_refs.entry(base).or_insert(a);
                }
            }
        }
        self.npc_refs.get(&npc).copied()
    }

    fn update_scripts(&mut self, dt: f32) {
        self.scripts.real_time += dt as f64;
        let now = self.scripts.real_time;
        let game_now = self.game_hours_total();
        // Timers
        let mut fired = Vec::new();
        self.scripts.timers.retain_mut(|t| {
            let due = if t.game_time { game_now >= t.at } else { now >= t.at };
            if due {
                fired.push((t.obj, t.script.clone(), t.event));
                if let Some(r) = t.repeat {
                    t.at += r.max(0.01);
                    return true;
                }
                return false;
            }
            true
        });
        let mut vm = std::mem::take(&mut self.vm);
        for _ in 0..4 {
            {
                let mut host = crate::script::EngineHost { engine: self };
                for (obj, script, ev) in fired.drain(..) {
                    if !vm.send_event_to(&mut host, obj, &script, ev, vec![]) {
                        vm.send_event(&mut host, obj, ev, vec![]);
                    }
                }
                let events = std::mem::take(&mut host.engine.scripts.pending_events);
                for (obj, ev, args) in events {
                    vm.send_event(&mut host, obj, &ev, args);
                }
                vm.run(&mut host, now, 20_000);
            }
            let stages = std::mem::take(&mut self.scripts.pending_stages);
            if stages.is_empty() && self.scripts.pending_events.is_empty() {
                break;
            }
            self.vm = vm;
            for (q, s) in stages {
                self.run_stage(q, s);
            }
            vm = std::mem::take(&mut self.vm);
        }
        self.vm = vm;
        if let Some(t) = self.pending_moveto.take()
            && let Some(p) = self.ref_position(t)
        {
            let cell = self.lo.cell_of_ref(t);
            let interior = cell.and_then(|c| self.lo.cell(c)).is_some_and(|c| c.world.is_none());
            let res = match (interior, cell) {
                (true, Some(c)) if self.location != Location::Interior(c) => self.enter_interior(c, Some((p, 0.0))),
                _ => {
                    self.place_player(p, self.camera.yaw);
                    Ok(())
                }
            };
            if let Err(e) = res {
                log::warn!("moveto failed: {e:#}");
            }
        }
        // Drop notifications older than a few seconds.
        self.scripts.notifications.retain(|(_, t)| now - t < 6.0);
    }

    /// Start every quest flagged "start game enabled".
    pub fn start_game_enabled_quests(&mut self) {
        let quests: Vec<FormId> = self
            .lo
            .ids_of_type(b"QUST")
            .iter()
            .copied()
            .filter(|&q| self.lo.get(q).and_then(|r| r.get(b"DNAM").map(|d| u16::from_le_bytes([d[0], d[1]]) & 0x1 != 0)).unwrap_or(false))
            .collect();
        log::info!("starting {} start-game-enabled quests", quests.len());
        for q in quests {
            self.start_quest(q);
        }
    }
}

/// Reference alias definitions of a quest: (alias id, forced reference, unique actor).
fn alias_specs(quest: &esp::LoadedRecord<'_>) -> Vec<(u32, Option<FormId>, Option<FormId>)> {
    let mut out = Vec::new();
    let mut cur: Option<(u32, Option<FormId>, Option<FormId>)> = None;
    for sr in quest.subrecords() {
        match &sr.tag.0 {
            b"ALST" => cur = Some((sr.u32(0), None, None)),
            b"ALLS" => cur = None,
            b"ALFR" => {
                if let Some(c) = cur.as_mut() {
                    c.1 = Some(quest.fid(sr.form_id(0)));
                }
            }
            b"ALUA" => {
                if let Some(c) = cur.as_mut() {
                    c.2 = Some(quest.fid(sr.form_id(0)));
                }
            }
            b"ALED" => {
                if let Some(c) = cur.take() {
                    out.push(c);
                }
            }
            _ => {}
        }
    }
    out
}

fn host_class(lo: &LoadOrder, f: FormId) -> &'static str {
    if f == PLAYER_REF {
        return "Actor";
    }
    lo.tag_of(f).map(|t| crate::script::types::class_for_tag(&t.0)).unwrap_or("Form")
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
