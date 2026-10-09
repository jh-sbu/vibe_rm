//! Engine state: loaded data, the active location, cell streaming, and gameplay glue.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use anyhow::{Context, Result};
use esp::{FormId, LoadOrder};
use glam::{Vec2, Vec3};
use rapier3d::prelude::ColliderHandle;

use crate::physics::Physics;
use crate::player::Player;
use crate::render::{
    Camera, CellKey, Environment, GpuLight, Instance, RenderCell, Renderer, Scene,
};
use crate::world::cell::{self, Door, PlacedObject, PointLight};
use crate::world::loader::{self, ModelCache};
use crate::world::records::{self, Lighting};
use crate::world::terrain::{self, CELL_SIZE, Land};
use crate::world::weather::{self, Climate, Weather};

#[derive(Default)]
pub(crate) struct CellRuntime {
    /// References instantiated in this cell (for script detach / visibility).
    pub(crate) refs: Vec<FormId>,
    /// Looping ambient sounds started for this cell, by the reference playing them.
    sounds: Vec<(FormId, crate::audio::VoiceId)>,
    /// Runtime state for each actor in the matching RenderCell, by index.
    pub(crate) actors: Vec<crate::ai::ActorRuntime>,
    /// Navmeshes loaded for this cell.
    navmeshes: Vec<FormId>,
    pub(crate) colliders: Vec<ColliderHandle>,
    pub(crate) land: Option<Land>,
    pub(crate) lights: Vec<PointLight>,
    pub(crate) doors: Vec<Door>,
    /// Doors and other keyframe-animated objects.
    pub(crate) animated: Vec<crate::world::animated::AnimatedObject>,
    /// Water planes: south-west corner, size and surface height.
    pub(crate) water: Vec<(glam::Vec2, f32, f32)>,
    /// Scripted trigger volumes.
    pub(crate) triggers: Vec<crate::triggers::Trigger>,
    /// Loose objects (simulated bodies).
    pub(crate) loose: Vec<crate::loose::Loose>,
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
    pub(crate) cells: HashMap<CellKey, CellRuntime>,
    /// Persistent references of the current worldspace bucketed by grid cell.
    world_persistent: HashMap<(i32, i32), Vec<FormId>>,
    pending_loads: Vec<(i32, i32)>,
    /// Name of what the crosshair points at, if activatable.
    pub look_target: Option<(FormId, String)>,
    skeletons: HashMap<String, Option<std::sync::Arc<crate::world::skeleton::Skeleton>>>,
    pub(crate) anims: crate::world::animation::AnimationLibrary,
    pub(crate) behaviors: crate::world::animation::BehaviorLibrary,
    pub(crate) graphs: crate::world::behavior::GraphLibrary,
    /// Movement types (`MOVT`) by name, read on first use.
    move_types: Option<HashMap<String, crate::world::movement::MoveSpeeds>>,
    /// Anim objects by lowercase editor id: model path and the bone it hangs from.
    anim_objects: HashMap<String, Option<(String, String)>>,
    /// Model path -> the bone a rigid model hangs from (its root's `Prn` string).
    parent_bones: HashMap<String, Option<String>>,
    /// Rigid models of actors' items (weapons, shields), by actor: (item, model).
    pub(crate) rigid_models: HashMap<FormId, Vec<(FormId, String)>>,
    /// How many of `scene.lights` belong to the cells (carried lights follow).
    pub(crate) static_lights: usize,
    /// Sound descriptors (SNDR / SOUN) as resolved.
    sound_descs: HashMap<FormId, Option<crate::world::sound::SoundDesc>>,
    /// Footstep sets, impacts and ground materials; the player's stride.
    pub(crate) footsteps: crate::footsteps::Footsteps,
    /// Model path -> where a carried light shines from (its `AttachLight` node).
    pub(crate) light_attach: HashMap<String, Vec3>,
    /// What actors and containers carry, by reference (kept across cell loads).
    pub inventories: HashMap<FormId, crate::world::inventory::Inventory>,
    /// IDLE records by parent and keyword (built on first use).
    pub(crate) idles: Option<crate::ai::idles::IdleIndex>,
    pub scripts: crate::script::ScriptState,
    pub vm: papyrus::Vm,
    /// Whole game days elapsed before the current day.
    pub day: u32,
    pub(crate) rng: u64,
    /// Rolls for `GetRandomPercent` (conditions are evaluated through `&Engine`).
    pub(crate) cond_rng: std::cell::Cell<u64>,
    pending_moveto: Option<FormId>,
    pub audio: Option<crate::audio::Audio>,
    music: MusicState,
    pub conversation: Option<crate::dialogue::Conversation>,
    /// The console is open (it pauses the game: `Engine::in_menu_mode`).
    pub console_open: bool,
    /// Image space modifiers, screen fades and camera shakes.
    pub imagespace: crate::imagespace::ImageSpace,
    /// Actors the player has had a conversation with (`GetTalkedToPC`).
    pub(crate) talked_to_pc: std::collections::HashSet<FormId>,
    /// Quest alias packages: who fills which aliases, and their packages.
    pub(crate) alias_packs: crate::ai::alias::AliasPackages,
    /// Factions' relations to others (`XNAM`), as read.
    pub(crate) faction_relations: crate::ai::combat::FactionRelations,
    /// Armor and block game settings, read on first use.
    pub(crate) combat_settings: std::cell::OnceCell<crate::ai::combat::CombatSettings>,
    /// Who detects the player, and the player's stealth points.
    pub(crate) detection: crate::detection::Detection,
    /// The player's health, and when they died (if they have).
    pub player_health: f32,
    /// The player holds their guard up (right mouse button).
    pub player_blocking: bool,
    /// Traps touching their targets.
    pub(crate) traps: crate::traps::Traps,
    /// Scripts registered for animation events, and the events to deliver.
    pub(crate) anim_events: crate::anim_events::AnimEvents,
    /// Activate parents and the child activations waiting on their delay.
    pub(crate) activation: crate::activation::Activation,
    /// Activate held down, and what the player has grabbed.
    pub(crate) grab: crate::grab::Grab,
    /// Player controls scripts have disabled.
    pub disabled_controls: crate::player::DisabledControls,
    /// The player's stamina, seconds before it comes back after being spent, how
    /// long they have held the attack button (a power attack when long enough),
    /// and their stats (race, NPC record), read on first use.
    pub player_stamina: f32,
    pub(crate) player_stamina_wait: f32,
    pub(crate) player_attack_held: Option<f32>,
    pub(crate) player_stats: std::cell::OnceCell<std::sync::Arc<crate::ai::combat::CombatStats>>,
    /// Arrows in flight or stuck where they struck.
    pub(crate) projectiles: Vec<crate::ai::archery::Projectile>,
    /// Where `--wait` runs hold the camera instead (console `tcam`): position,
    /// yaw, pitch.
    pub test_camera: Option<(Vec3, f32, f32)>,
    /// A jump asked for from the console, taken once the player is on the ground.
    pub test_jump: bool,
    /// Frames left of the player walking forward (console `pwalk`).
    pub test_walk: u32,
    pub player_died_at: Option<f64>,
    /// Lines NPCs say by themselves (greetings, idle chatter).
    pub barks: crate::dialogue::barks::Barks,
    /// Scenes playing.
    pub scenes: crate::scene::Scenes,
    /// The Story Manager's tree and what it remembers.
    pub(crate) story: crate::story::StoryManager,
    /// Relationship ranks between NPCs.
    pub(crate) relationships: crate::relationships::Relationships,
    /// The player's bounties and actors' crime factions set by scripts.
    pub(crate) crime: crate::crime::Crimes,
    /// Values scripts keep on locations by keyword (`Location.SetKeywordData`).
    pub(crate) location_keyword_data: HashMap<(FormId, FormId), f32>,
    /// The inventory or container menu, while open.
    pub menu: Option<crate::items::Menu>,
    pub lockpick: Option<crate::locks::Lockpick>,
    /// Message boxes, help messages and banners.
    pub messages: crate::messages::Messages,
    /// References by the enable parent they follow (`XESP`).
    enable_children: std::cell::OnceCell<HashMap<FormId, Vec<FormId>>>,
    /// Each NPC's first placed reference (unique actors' references).
    npc_refs: std::cell::OnceCell<HashMap<FormId, FormId>>,
    /// Locations' references and ref types, read on first use.
    pub(crate) location_index: std::cell::OnceCell<crate::locations::LocationIndex>,
    /// Persistent references, candidates for "find matching reference" aliases.
    pub(crate) persistent_refs: std::cell::OnceCell<Vec<FormId>>,
    /// Reference state kept across cell loads (dead actors...).
    pub(crate) world_state: crate::world_state::WorldState,
    /// References made while playing (`PlaceAtMe`, created alias references).
    pub(crate) created_refs: crate::created::CreatedRefs,
    /// Quests' aliases as parsed.
    pub(crate) alias_spec_cache: HashMap<FormId, std::sync::Arc<Vec<crate::aliases::AliasSpec>>>,
    lod: Option<crate::world::lod::Lod>,
    pub nav: crate::ai::nav::NavWorld,
    pub furniture: crate::ai::furniture::FurnitureWorld,
    /// Patrol routes by start reference.
    pub(crate) patrol_paths: HashMap<FormId, std::sync::Arc<Vec<crate::ai::PatrolPoint>>>,
    /// Current positions of references that have moved from their editor location.
    pub moved_refs: HashMap<FormId, Vec3>,
    /// Actor AI processing (toggled with the `tai` console command).
    pub ai_enabled: bool,
    pub whereabouts: crate::ai::schedule::Whereabouts,
    /// Cell each spawned actor reference belongs to.
    pub actor_cells: HashMap<FormId, CellKey>,
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
/// The player's health (no leveling yet).
pub const PLAYER_HEALTH: f32 = 100.0;
/// Real seconds per game hour at the default timescale of 20.
const TIMESCALE: f64 = 20.0;

pub fn grid_of(p: Vec2) -> (i32, i32) {
    (
        (p.x / CELL_SIZE).floor() as i32,
        (p.y / CELL_SIZE).floor() as i32,
    )
}

impl Engine {
    pub fn new(
        lo: LoadOrder,
        vfs: vfs::Vfs,
        renderer: Renderer,
        hour: f32,
        weather: Option<String>,
        radius: i32,
    ) -> Self {
        Engine {
            lo,
            vfs,
            renderer,
            models: ModelCache::default(),
            scene: Scene::default(),
            camera: Camera {
                position: Vec3::ZERO,
                yaw: 0.0,
                pitch: 0.0,
                fov_y: 65f32.to_radians(),
            },
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
            behaviors: Default::default(),
            graphs: Default::default(),
            move_types: None,
            anim_objects: Default::default(),
            parent_bones: Default::default(),
            inventories: Default::default(),
            rigid_models: Default::default(),
            static_lights: 0,
            light_attach: Default::default(),
            sound_descs: Default::default(),
            footsteps: Default::default(),
            idles: None,
            scripts: Default::default(),
            vm: papyrus::Vm::new(),
            day: 0,
            // VRM_SEED makes runs reproducible (for testing).
            rng: std::env::var("VRM_SEED")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or_else(|| {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos() as u64)
                        .unwrap_or(1)
                })
                | 1,
            cond_rng: std::cell::Cell::new(0x9E37_79B9_7F4A_7C15),
            pending_moveto: None,
            audio: None,
            music: MusicState::default(),
            conversation: None,
            console_open: false,
            imagespace: Default::default(),
            menu: None,
            lockpick: None,
            messages: Default::default(),
            barks: Default::default(),
            scenes: Default::default(),
            story: Default::default(),
            relationships: Default::default(),
            crime: Default::default(),
            location_keyword_data: Default::default(),
            talked_to_pc: Default::default(),
            alias_packs: Default::default(),
            faction_relations: Default::default(),
            combat_settings: Default::default(),
            detection: Default::default(),
            player_blocking: false,
            grab: Default::default(),
            activation: Default::default(),
            traps: Default::default(),
            anim_events: Default::default(),
            disabled_controls: Default::default(),
            // Full: clamped to their most on the first update.
            player_stamina: f32::INFINITY,
            player_stamina_wait: 0.0,
            player_attack_held: None,
            player_stats: Default::default(),
            projectiles: Vec::new(),
            test_camera: None,
            test_jump: false,
            test_walk: 0,
            player_health: PLAYER_HEALTH,
            player_died_at: None,
            npc_refs: Default::default(),
            enable_children: Default::default(),
            location_index: Default::default(),
            persistent_refs: Default::default(),
            alias_spec_cache: HashMap::new(),
            created_refs: Default::default(),
            world_state: Default::default(),
            lod: None,
            nav: Default::default(),
            furniture: Default::default(),
            patrol_paths: Default::default(),
            moved_refs: HashMap::new(),
            ai_enabled: true,
            whereabouts: Default::default(),
            actor_cells: HashMap::new(),
        }
    }

    pub fn resolve_form(&self, s: &str) -> Option<FormId> {
        if s.len() == 8
            && let Ok(v) = u32::from_str_radix(s, 16)
            && (self.lo.locate(FormId(v)).is_some() || self.created(FormId(v)).is_some())
        {
            return Some(FormId(v));
        }
        // Editor ids first, then FormIDs written short (`f` for gold).
        self.lo.find_editor_id(s).or_else(|| {
            u32::from_str_radix(s, 16)
                .ok()
                .map(FormId)
                .filter(|&f| self.lo.locate(f).is_some())
        })
    }

    pub fn camera_copy(&self) -> Camera {
        Camera {
            position: self.camera.position,
            yaw: self.camera.yaw,
            pitch: self.camera.pitch,
            fov_y: self.camera.fov_y,
        }
    }

    /// Play a sound descriptor (by editor id) once at a point: one of its files.
    pub fn play_sound_at(&mut self, edid: &str, at: Vec3) {
        match self.lo.find_editor_id(edid) {
            Some(f) => self.play_sound(f, at),
            None => log::debug!("no sound descriptor {edid}"),
        }
    }

    /// Play a sound descriptor (SNDR, or a SOUN naming one) at a point.
    pub fn play_sound(&mut self, sound: FormId, at: Vec3) {
        let desc = self.sound_descs.entry(sound).or_insert_with(|| {
            let d = crate::world::sound::descriptor(&self.lo, &self.vfs, sound);
            if d.is_none() {
                log::debug!("no sound descriptor {sound}");
            }
            d
        });
        let Some(desc) = desc.clone() else { return };
        let i = (self.rand() % desc.files.len() as u64) as usize;
        if let Some(a) = self.audio.as_mut() {
            a.play(
                &self.vfs,
                &desc.files[i],
                desc.volume,
                false,
                Some(at),
                desc.min_dist,
                desc.max_dist,
            );
        }
    }

    /// Start looping sounds emitted by objects (sound markers, lights, activators, ...).
    fn start_cell_sounds(&mut self, key: CellKey, refs: &[FormId]) {
        let refs: Vec<FormId> = refs
            .iter()
            .copied()
            .filter(|&r| !self.is_disabled(r))
            .collect();
        let Some(audio) = self.audio.as_mut() else {
            return;
        };
        let mut voices = Vec::new();
        for &r in &refs {
            let Some(rec) = self.lo.get(r) else { continue };
            let rf = records::reference(&rec);
            if rf.deleted() {
                continue;
            }
            let Some(base) = self.lo.get(rf.base) else {
                continue;
            };
            let snd = match &base.tag().0 {
                b"SOUN" => Some(rf.base),
                b"LIGH" | b"ACTI" | b"MSTT" | b"FURN" => base
                    .get(b"SNAM")
                    .map(|d| base.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap())))),
                _ => None,
            };
            let Some(desc) =
                snd.and_then(|s| crate::world::sound::descriptor(&self.lo, &self.vfs, s))
            else {
                continue;
            };
            if !desc.looping {
                continue;
            }
            let file = &desc.files[r.0 as usize % desc.files.len()];
            if let Some(v) = audio.play(
                &self.vfs,
                file,
                desc.volume,
                true,
                Some(rf.position),
                desc.min_dist,
                desc.max_dist,
            ) {
                voices.push((r, v));
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
            Location::Exterior { world, center } => self
                .lo
                .world(world)
                .and_then(|w| w.cells.get(&center).copied()),
            Location::Nowhere => None,
        }?;
        let rec = self.lo.get(cell)?;
        if let Some(d) = rec.get(b"XCMO") {
            return Some(rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().ok()?))));
        }
        let regions: Vec<FormId> = rec
            .get(b"XCLR")
            .map(|d| {
                d.chunks_exact(4)
                    .map(|c| rec.fid(FormId(u32::from_le_bytes(c.try_into().unwrap()))))
                    .collect()
            })
            .unwrap_or_default();
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
        let tracks: Vec<FormId> = rec
            .get(b"TNAM")?
            .chunks_exact(4)
            .map(|c| rec.fid(FormId(u32::from_le_bytes(c.try_into().unwrap()))))
            .collect();
        drop(rec);
        let ctx = crate::condition::Context {
            subject: Some(PLAYER_REF),
            ..Default::default()
        };
        let mut candidates: Vec<FormId> = tracks
            .into_iter()
            .filter(|t| {
                self.lo.get(*t).is_some_and(|r| {
                    crate::condition::evaluate(self, &crate::condition::parse_all(&r), ctx)
                })
            })
            .collect();
        log::debug!("music candidates: {}", candidates.len());
        for _ in 0..4 {
            if candidates.is_empty() {
                return None;
            }
            let i = (self.rand() % candidates.len() as u64) as usize;
            let t = self.lo.get(candidates[i])?;
            let kind = t
                .get(b"CNAM")
                .map(|d| u32::from_le_bytes(d[0..4].try_into().unwrap()))
                .unwrap_or(crate::world::sound::MUST_SINGLE);
            if kind == crate::world::sound::MUST_PALETTE {
                candidates = t
                    .get(b"SNAM")
                    .map(|d| {
                        d.chunks_exact(4)
                            .map(|c| t.fid(FormId(u32::from_le_bytes(c.try_into().unwrap()))))
                            .collect()
                    })
                    .unwrap_or_default();
                continue;
            }
            if let Some(f) = t.get(b"ANAM") {
                return Some(crate::world::sound::sound_path(
                    &esp::decode_zstring(f),
                    &self.vfs,
                ));
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
        let playing = self
            .music
            .voice
            .is_some_and(|v| self.audio.as_ref().unwrap().is_playing(v));
        if want != self.music.music_type {
            if let Some(v) = self.music.voice.take() {
                self.audio.as_ref().unwrap().stop(v);
            }
            self.music.music_type = want;
            log::debug!(
                "music type -> {:?}",
                want.and_then(|w| self.lo.get(w))
                    .and_then(|r| r.editor_id())
            );
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
                self.music.voice = self
                    .audio
                    .as_mut()
                    .unwrap()
                    .play(vfs, &track, 0.45, false, None, 0.0, 0.0);
            }
            // Silence between tracks, like the original.
            self.music.next_at = now + 20.0 + (self.rand() % 40) as f64;
        }
    }

    /// Keep the distant LOD quadtree in sync with the camera.
    fn update_lod(&mut self) {
        let Location::Exterior { center, .. } = self.location else {
            self.scene.lod.clear();
            return;
        };
        let r = self.radius as f32;
        self.scene.lod_clip = [
            (center.0 as f32 - r) * CELL_SIZE,
            (center.1 as f32 - r) * CELL_SIZE,
            (center.0 as f32 + r + 1.0) * CELL_SIZE,
            (center.1 as f32 + r + 1.0) * CELL_SIZE,
        ];
        let Some(lod) = self.lod.as_mut() else { return };
        let mut want = lod.desired(self.camera.position.truncate());
        if let Ok(only) = std::env::var("VRM_LOD_ONLY") {
            let v: Vec<i32> = only.split(',').filter_map(|x| x.parse().ok()).collect();
            if v.len() == 3 {
                want = vec![(v[0], v[1], v[2])];
            }
        }
        let want_set: HashSet<(i32, i32, i32)> = want.iter().copied().collect();
        // Load missing blocks, nearest (finest) first, a few per frame.
        let mut missing: Vec<(i32, i32, i32)> = want
            .iter()
            .copied()
            .filter(|k| !lod.is_loaded(*k))
            .collect();
        missing.sort_by_key(|k| k.0);
        let mut loaded_any = false;
        for (level, x, y) in missing.into_iter().take(4) {
            let (btr, bto) = lod.paths(level, x, y);
            let mut models = Vec::new();
            if std::env::var_os("VRM_NO_BTR").is_none()
                && let Some(m) = crate::world::lod::load_block(&self.vfs, &btr, level, x, y, true)
            {
                models.push(m);
            }
            if lod.has(level, x, y, true)
                && std::env::var_os("VRM_NO_BTO").is_none()
                && let Some(m) = crate::world::lod::load_block(&self.vfs, &bto, level, x, y, false)
            {
                models.push(m);
            }
            let mut tex = HashSet::new();
            for m in &models {
                for mesh in &m.meshes {
                    for t in [&mesh.material.diffuse, &mesh.material.normal]
                        .into_iter()
                        .flatten()
                    {
                        if !self.renderer.textures.contains(t) {
                            tex.insert(t.clone());
                        }
                    }
                }
            }
            loader::load_textures(&mut self.renderer, &self.vfs, tex.into_iter().collect());
            for m in &models {
                log::debug!(
                    "lod block {level}.{x}.{y}: {} meshes, bound center {:?} r {}",
                    m.meshes.len(),
                    m.bound_center,
                    m.bound_radius
                );
            }
            let instances = models
                .into_iter()
                .filter(|m| !m.meshes.is_empty())
                .map(|m| {
                    crate::world::lod::instance(std::sync::Arc::new(self.renderer.upload_model(&m)))
                })
                .collect();
            lod.insert((level, x, y), instances);
            loaded_any = true;
        }
        // Drop blocks no longer wanted once everything wanted is present.
        if !loaded_any {
            for k in lod.loaded_keys() {
                if !want_set.contains(&k) {
                    lod.remove(k);
                }
            }
        }
        if lod.dirty {
            lod.dirty = false;
            self.scene.lod = lod
                .instances()
                .map(|i| crate::render::Instance::new(i.model.clone(), i.transform))
                .collect();
        }
    }

    fn unload_all(&mut self) {
        if let Some(a) = self.audio.as_mut() {
            a.stop_all();
        }
        self.music.voice = None;
        let keys: Vec<CellKey> = self.cells.keys().copied().collect();
        for k in keys {
            self.remember_actors(k);
            self.unload_loose(k);
            self.park_object_graphs(k);
        }
        self.scene.cells.clear();
        self.scene.lights.clear();
        self.scene.dynamic.clear();
        self.projectiles.clear();
        self.cells.clear();
        self.physics.clear();
        self.pending_loads.clear();
        self.world_persistent.clear();
        self.nav.clear();
        self.furniture = Default::default();
        self.moved_refs.clear();
        self.actor_cells.clear();
    }

    /// The share of their speed the player keeps sneaking: the NPC sneaking
    /// movement type's forward speeds against the default's.
    fn player_sneak_speed(&mut self) -> f32 {
        let types = self
            .move_types
            .get_or_insert_with(|| crate::world::movement::movement_types(&self.lo));
        match (types.get("npcsneaking"), types.get("npcdefault")) {
            (Some(s), Some(d)) if d.walk > 0.0 && d.run > 0.0 => {
                (s.walk / d.walk + s.run / d.run) / 2.0
            }
            _ => 0.6,
        }
    }

    /// Place the player with their feet at `feet`, facing `yaw` (radians).
    pub fn place_player(&mut self, feet: Vec3, yaw: f32) {
        let sneaking = self.player.sneaking;
        self.player = Player::new(Vec3::ZERO);
        self.player.sneaking = sneaking;
        self.player.crouch = sneaking as u8 as f32;
        // Settling onto the ground isn't a landing.
        self.footsteps.airborne = f32::NEG_INFINITY;
        self.player.position =
            feet + Vec3::Z * (self.physics.player_half_height + self.physics.player_radius + 2.0);
        self.camera.yaw = yaw;
        self.camera.pitch = 0.0;
        self.camera.position = self.player.eye();
    }

    // ------------------------------------------------------------------ cells

    /// Instantiate objects into a render cell plus physics, returning runtime data.
    fn instantiate(
        &mut self,
        key: CellKey,
        objects: &[PlacedObject],
        lights: Vec<PointLight>,
        doors: Vec<Door>,
    ) {
        self.scene.cells.insert(key, RenderCell::default());
        self.cells.insert(
            key,
            CellRuntime {
                lights,
                doors,
                ..Default::default()
            },
        );
        self.add_objects(key, objects, false);
        if let Some(rt) = self.cells.get(&key) {
            let doors = rt.animated.iter().filter(|a| a.door).count();
            if !rt.animated.is_empty() {
                log::debug!(
                    "{key:?}: {} animated objects ({doors} doors)",
                    rt.animated.len()
                );
            }
            if !rt.loose.is_empty() {
                log::debug!("{key:?}: {} loose objects", rt.loose.len());
            }
        }
    }

    /// Draw objects in a loaded cell, with their collision: loose objects as
    /// bodies of their own (falling at once when `fresh`ly made).
    pub(crate) fn add_objects(&mut self, key: CellKey, objects: &[PlacedObject], fresh: bool) {
        let paths: Vec<String> = objects.iter().map(|o| o.model.clone()).collect();
        self.models.load_all(&mut self.renderer, &self.vfs, &paths);
        let (Some(mut rc), Some(mut rt)) = (self.scene.cells.remove(&key), self.cells.remove(&key))
        else {
            return;
        };
        let mut loose = Vec::new();
        for o in objects {
            rt.refs.push(o.ref_id);
            let hidden = self.is_disabled(o.ref_id);
            if let Some(m) = self.models.get(&o.model) {
                let mut inst = Instance::new(m, o.transform);
                inst.ref_id = o.ref_id.0;
                inst.hidden = hidden;
                rc.instances.push(inst);
            }
            if self
                .models
                .collision(&o.model)
                .is_some_and(|c| c.is_loose())
            {
                // The root model's instance, and those of its bodies' nodes.
                let mut drawn: Vec<(Option<String>, usize)> = Vec::new();
                if self.models.get(&o.model).is_some() {
                    drawn.push((None, rc.instances.len() - 1));
                }
                for part in self.models.anim(&o.model).iter().flat_map(|a| &a.parts) {
                    let mut inst =
                        Instance::new(part.model.clone(), o.transform * part.parent * part.rest);
                    inst.ref_id = o.ref_id.0;
                    inst.hidden = hidden;
                    drawn.push((Some(part.node.clone()), rc.instances.len()));
                    rc.instances.push(inst);
                }
                loose.push((o.ref_id, o.model.clone(), o.transform, drawn));
                continue;
            }
            let tagged = match self.models.collision(&o.model) {
                Some(c) => self.physics.add_static_tagged(&c, o.transform, o.ref_id),
                None => Vec::new(),
            };
            rt.colliders.extend(tagged.iter().map(|(h, _)| *h));
            if !tagged.is_empty() && self.is_disabled(o.ref_id) {
                self.physics.set_owner_enabled(o.ref_id, false);
            }
            if let Some(mut obj) =
                self.animated_object(o.ref_id, &o.model, o.transform, &mut rc.instances, &tagged)
            {
                self.start_object_graph(&mut obj, &tagged);
                if obj.open {
                    self.physics.set_enabled(&obj.colliders, false);
                }
                rt.animated.push(obj);
            }
        }
        self.scene.cells.insert(key, rc);
        self.cells.insert(key, rt);
        for (r, model, transform, drawn) in loose {
            self.add_loose(key, r, &model, transform, fresh, drawn);
        }
    }

    /// Attach scripts for references in a newly loaded cell and send load events.
    pub(crate) fn attach_cell_scripts(&mut self, refs: &[FormId]) {
        let mut vm = std::mem::take(&mut self.vm);
        for &r in refs {
            let (mut scripts, base) = match self.lo.get(r) {
                Some(rec) => {
                    let own = crate::script::vmad::parse(&rec)
                        .map(|v| v.scripts)
                        .unwrap_or_default();
                    // Actors take their base's scripts from its script part (templates).
                    let base = if rec.tag().0 == *b"ACHR" {
                        self.templates_of(r)
                            .map_or(FormId::NULL, |t| t.of(crate::world::template::SCRIPT))
                    } else {
                        records::reference(&rec).base
                    };
                    (own, base)
                }
                // Created objects have their base's.
                None => match self.created(r) {
                    Some(c) if !c.actor => (Vec::new(), c.base),
                    _ => continue,
                },
            };
            if let Some(b) = self.lo.get(base)
                && let Some(bv) = crate::script::vmad::parse(&b)
            {
                // A reference's own copy of a base script keeps the base's
                // properties it doesn't set (its VMAD holds only those it
                // overrides: a swinging blade's sets its `TrapLevel`, its base
                // the damage and sounds).
                for s in bv.scripts {
                    match scripts
                        .iter_mut()
                        .find(|x| x.name.eq_ignore_ascii_case(&s.name))
                    {
                        Some(own) => {
                            for (n, v) in s.properties {
                                if !own
                                    .properties
                                    .iter()
                                    .any(|(o, _)| o.eq_ignore_ascii_case(&n))
                                {
                                    own.properties.push((n, v));
                                }
                            }
                        }
                        None => scripts.push(s),
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
                        .map(|(n, pv)| {
                            (
                                n.clone(),
                                crate::script::vmad::to_value(pv, &|f| host.engine.native_class(f)),
                            )
                        })
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

    /// Spawn actors (ACHR references) into a loaded cell, optionally away from their
    /// editor location (NaN: anywhere on the cell's navmesh).
    pub(crate) fn spawn_actors(&mut self, key: CellKey, refs: &[(FormId, Option<Vec3>)]) {
        let mut descs = Vec::new();
        let cell_meshes = self
            .cells
            .get(&key)
            .map(|rt| rt.navmeshes.clone())
            .unwrap_or_default();
        for &(r, at) in refs {
            let is_actor = self.lo.tag_of(r).map_or_else(
                || self.created(r).is_some_and(|c| c.actor),
                |t| t.0 == *b"ACHR",
            );
            if !is_actor || self.actor_cells.contains_key(&r) || self.is_disabled(r) {
                continue;
            }
            let Some(rf) = self.reference_of(r) else {
                continue;
            };
            if let Some(mut d) = crate::world::actor::describe_reference(&self.lo, &rf) {
                let editor_pos = d.transform.w_axis.truncate();
                if let Some(p) = at {
                    let mut rng = self.rand() | 1;
                    let mut rand = move || {
                        rng ^= rng << 13;
                        rng ^= rng >> 7;
                        rng ^= rng << 17;
                        rng
                    };
                    let spot = if self.world_state.dead.contains_key(&r)
                        || self.world_state.moved.get(&r).is_some_and(|m| m.pos == p)
                    {
                        // A body lies where it fell.
                        Some(p)
                    } else if p.is_nan() {
                        self.nav.random_point_in(&cell_meshes, &mut rand)
                    } else {
                        // Spread out actors sent to the same marker.
                        self.nav.random_point(p, 96.0, &mut rand).or(Some(p))
                    };
                    let Some(spot) = spot else { continue };
                    d.transform.w_axis = spot.extend(1.0);
                }
                descs.push((d, editor_pos));
            }
        }
        if descs.is_empty() {
            return;
        }
        let paths: Vec<String> = descs
            .iter()
            .flat_map(|(d, _)| d.models.iter().map(|(m, _)| m.clone()))
            .collect();
        self.models.load_all(&mut self.renderer, &self.vfs, &paths);
        let mut actors = Vec::new();
        let mut runtimes = Vec::new();
        for (ai, (d, editor_pos)) in descs.iter().enumerate() {
            let Some(skel) = self.skeleton(&d.skeleton) else {
                continue;
            };
            let idle = crate::world::animation::idle_clip(&d.skeleton, d.female)
                .iter()
                .find_map(|c| self.anims.clip(&self.vfs, c, &d.skeleton, &skel));
            let project = d
                .behavior
                .as_deref()
                .and_then(|p| self.graphs.project(&self.vfs, p));
            // With root motion from the project's animation data: the AI walks at its speed.
            let motion_project = project
                .as_ref()
                .filter(|p| !p.humanoid())
                .map(|p| p.name.clone());
            let walk = crate::world::animation::locomotion_clip(&d.skeleton, d.female, false)
                .iter()
                .find_map(|c| match &motion_project {
                    Some(p) => {
                        self.anims
                            .clip_in_project(&self.vfs, c, &d.skeleton, &skel, Some(p))
                    }
                    None => self.anims.clip(&self.vfs, c, &d.skeleton, &skel),
                });
            // Desynchronise actors sharing a clip.
            let start = (ai as f32 * 1.618) % 7.0;
            let packages = self.actor_packages(d.ref_id, d.npc);
            let mut rt = crate::ai::ActorRuntime::new(
                d.ref_id,
                d.npc,
                skel.clone(),
                d.transform,
                packages,
                start * 0.3,
            );
            rt.alias_gen = self.scripts.alias_gen;
            rt.editor_pos = *editor_pos;
            rt.skeleton_path = d.skeleton.clone();
            rt.female = d.female;
            rt.child = self.npc_race(d.npc).is_some_and(|r| self.race_is_child(r));
            let mut stats = crate::ai::combat::CombatStats::of(self, &d.templates, d.race);
            self.apply_actor_values(d.ref_id, &mut stats);
            rt.health = self.returning_health(d.ref_id, stats.max_health);
            rt.stamina = stats.max_stamina;
            rt.power_cost = self.power_attack_cost(d.inventory.weapon(&self.lo));
            if let Some(bow) = d
                .inventory
                .weapon(&self.lo)
                .filter(|&w| crate::ai::archery::is_bow(&self.lo, w))
            {
                rt.bow = true;
                rt.bow_speed = crate::ai::archery::bow_speed(&self.lo, bow);
                log::debug!("{} wields bow {bow} (speed {})", d.ref_id, rt.bow_speed);
            }
            rt.stats = std::sync::Arc::new(stats);
            rt.footstep_set = d.footsteps.and_then(|f| self.footstep_set(f));
            log::debug!(target: "footsteps", "{} ({}) walks with footstep set {:?}", d.ref_id, d.name, d.footsteps);
            rt.weapon_reach = d
                .inventory
                .weapon(&self.lo)
                .and_then(|w| self.lo.get(w))
                .and_then(|r| {
                    r.get(b"DNAM")
                        .filter(|x| x.len() >= 12)
                        .map(|x| f32::from_le_bytes(x[8..12].try_into().unwrap()))
                })
                .unwrap_or(0.0);
            rt.anim = idle
                .clone()
                .map(|c| crate::world::animation::ActorAnim::new(c, &skel, start));
            let mut first_pose = None;
            // Actors run their race's behaviour graph, set up as the game does for NPCs.
            if let Some(project) = project {
                // Speeds and turn rates of the graph's default and sneaking movement types.
                let types = self
                    .move_types
                    .get_or_insert_with(|| crate::world::movement::movement_types(&self.lo));
                let character = project
                    .shared
                    .project
                    .character
                    .as_ref()
                    .map_or("", |c| c.name.as_str());
                let (default, sneak) = crate::world::movement::graph_movement_types(
                    project.shared.variables(),
                    character,
                );
                rt.moves = default.and_then(|(n, v)| Some((*types.get(&n)?, v)));
                rt.sneak_moves = sneak.and_then(|(n, v)| Some((*types.get(&n)?, v)));
                let humanoid = project.humanoid();
                let mut g = crate::world::behavior::GraphAnim::new(
                    project,
                    &d.skeleton,
                    d.female,
                    &skel,
                    self.rng ^ d.ref_id.0 as u64,
                );
                g.label = d.ref_id.to_string();
                if humanoid {
                    for (var, value) in
                        [("IsNPC", 1.0), ("i1stPerson", 0.0), ("IsFirstPerson", 0.0)]
                    {
                        g.set_variable(var, value);
                    }
                    // NPC weight (0..100) picks between skinny and muscular body poses.
                    let weight = self.lo.get(d.npc).and_then(|r| {
                        r.get(b"NAM7")
                            .filter(|b| b.len() >= 4)
                            .map(|b| f32::from_le_bytes(b[0..4].try_into().unwrap()))
                    });
                    g.set_variable("weapAdj", weight.unwrap_or(50.0) / 100.0);
                    // What the hands hold (sheathed): the graph picks equip / unequip
                    // and weapon-drawn behaviours by it.
                    g.set_variable(
                        "iLeftHandType",
                        d.inventory.hand(&self.lo, true) as i32 as f32,
                    );
                    g.set_variable(
                        "iRightHandType",
                        d.inventory.hand(&self.lo, false) as i32 as f32,
                    );
                }
                // Desynchronise actors standing about.
                rt.graph_walk_speed = g.walk_speed(&self.vfs, &mut self.anims, &skel);
                first_pose = Some(g.update(start, &self.vfs, &mut self.anims, &skel).pose);
                rt.graph = Some(g);
            }
            rt.idle = idle;
            rt.walk = walk;
            log::debug!(
                "{}: walks at {:.0} units/s; movement {:?}, sneaking {:?}",
                d.ref_id,
                rt.walk_speed(),
                rt.moves,
                rt.sneak_moves
            );
            let (scale, _, feet) = d.transform.to_scale_rotation_translation();
            rt.capsule = Some(self.physics.add_actor_capsule(feet, scale.x, d.ref_id));
            let pose = first_pose.unwrap_or_else(|| skel.model_space(&skel.bind_locals()));
            let mut meshes = Vec::new();
            let mut equipment = Vec::new();
            let mut rigid = Vec::new();
            for (m, item) in &d.models {
                let Some(model) = self.models.get(m) else {
                    log::debug!("{}: missing model {m}", d.name);
                    continue;
                };
                // Rigid models (weapons, shields) hang from the bone they name.
                if model.skinned.is_empty() && !model.parts.is_empty() {
                    match self
                        .parent_bone(m)
                        .and_then(|b| skel.find(&b).map(|i| (b, i)))
                    {
                        Some((_, bone)) => {
                            equipment.push((model.clone(), bone, glam::Mat4::IDENTITY));
                            rigid.push((*item, m.clone()));
                        }
                        None => log::debug!(
                            "{}: rigid model {m} has no parent bone in {}",
                            d.name,
                            d.skeleton
                        ),
                    }
                    continue;
                }
                for (pi, part) in model.skinned.iter().enumerate() {
                    let bone_map = part
                        .bone_names
                        .iter()
                        .map(|n| {
                            skel.find(n).unwrap_or_else(|| {
                                log::debug!(
                                    "{}: bone {n:?} of {m} not in skeleton {}",
                                    d.name,
                                    d.skeleton
                                );
                                usize::MAX
                            })
                        })
                        .collect();
                    meshes.push(crate::render::ActorMesh {
                        model: model.clone(),
                        part: pi,
                        bone_map,
                    });
                }
            }
            log::debug!(
                "actor {} {:?} ({} {}) {} meshes, {} rigid ({:?}) at {:?}, {} packages",
                d.ref_id,
                d.name,
                d.npc,
                self.lo
                    .get(d.npc)
                    .and_then(|r| r.editor_id())
                    .unwrap_or_default(),
                meshes.len(),
                equipment.len(),
                d.inventory.weapon(&self.lo).and_then(|w| self
                    .lo
                    .get(w)?
                    .editor_id()
                    .map(|e| e.to_string())),
                d.transform.w_axis.truncate(),
                rt.packages.len()
            );
            actors.push(crate::render::ActorInstance {
                meshes,
                attachments: Vec::new(),
                equipment,
                held_light: None,
                transform: d.transform,
                pose,
                lights: [0xFFFF; 8],
                radius: 120.0,
            });
            self.rigid_models.insert(d.ref_id, rigid);
            runtimes.push(rt);
        }
        log::info!("spawned {} actors", actors.len());
        if let Some(rc) = self.scene.cells.get_mut(&key) {
            rc.actors.extend(actors);
        }
        for a in &runtimes {
            self.actor_cells.insert(a.ref_id, key);
        }
        for (d, _) in descs {
            self.inventories.entry(d.ref_id).or_insert(d.inventory);
        }
        // Placed as corpses ("Starts Dead"): they lie as ragdolls from the start.
        let corpses: Vec<FormId> = runtimes
            .iter()
            .map(|a| a.ref_id)
            .filter(|&r| {
                self.world_state.dead.contains_key(&r)
                    || self
                        .lo
                        .get(r)
                        .is_some_and(|rec| rec.flags() & esp::record_flags::STARTS_DEAD != 0)
            })
            .collect();
        if let Some(rt) = self.cells.get_mut(&key) {
            rt.colliders
                .extend(runtimes.iter().filter_map(|a| a.capsule));
            rt.actors.extend(runtimes);
        }
        for r in corpses {
            let lying = self.world_state.body_poses.get(&r).cloned();
            self.kill_actor_lying(r, lying);
        }
    }

    /// Pick the idle-tree events for getting on and off each furniture marker in
    /// `key` (not idle markers), from each side, for adults and children.
    fn find_furniture_ways(&mut self, key: CellKey) {
        use crate::ai::furniture::{Entry, Way};
        use crate::condition::{Context, IdleQuery};
        let Some(idles) = self.idles.as_ref() else {
            return;
        };
        let Some(root) = idles.find(&self.lo, "ActivateRootChar") else {
            return;
        };
        let mut found: Vec<(FormId, Way)> = Vec::new();
        for f in self.furniture.items.iter().filter(|f| f.cell == key) {
            for (mi, m) in f.markers.iter().enumerate() {
                if m.kind == crate::ai::furniture::Use::Idle {
                    continue;
                }
                let anim_type = m.anim_type;
                for (entry, bit) in Entry::of(m.entries) {
                    for child in [false, true] {
                        let ctx = |state: f32, holding: bool, instant: bool| Context {
                            target: Some(f.ref_id),
                            idle: Some(IdleQuery {
                                anim_type,
                                entry: u32::from(bit) << 16,
                                state,
                                child: Some(child),
                                holding,
                                instant,
                                ..Default::default()
                            }),
                            ..Default::default()
                        };
                        // The way on for users without what the subtree asks about,
                        // then for those holding it (a wood pile's put-down).
                        let empty = idles.select(self, root, ctx(2.0, false, false));
                        let laden = idles
                            .select(self, root, ctx(2.0, true, false))
                            .filter(|l| empty.as_ref().is_none_or(|e| e.0 != l.0));
                        let enters: Vec<String> =
                            empty.iter().chain(&laden).map(|e| e.1.clone()).collect();
                        let instant_exit = idles
                            .select(self, root, ctx(4.0, false, true))
                            .map(|(_, e)| e);
                        for (held, (idle, enter)) in [(false, empty), (true, laden)]
                            .into_iter()
                            .filter_map(|(h, w)| Some((h, w?)))
                        {
                            let holding = idles.counted_item(idle).map(|item| (item, held));
                            if held && holding.is_none() {
                                continue;
                            }
                            // A subtree without an exit idle offers its enter idle again.
                            let exit = idles
                                .select(self, root, ctx(4.0, held, false))
                                .map(|(_, e)| e)
                                .filter(|e| !enters.contains(e));
                            log::trace!(
                                "{} marker {mi} {entry:?}{}: {enter} / {exit:?} {holding:?}",
                                f.ref_id,
                                if child { " (child)" } else { "" }
                            );
                            found.push((
                                f.ref_id,
                                Way {
                                    marker: mi as u8,
                                    entry,
                                    child,
                                    enter,
                                    exit,
                                    graph: None,
                                    holding,
                                    instant_exit: instant_exit.clone(),
                                },
                            ));
                        }
                    }
                }
            }
        }
        let markers: Vec<(FormId, usize, crate::ai::furniture::Use)> = self
            .furniture
            .items
            .iter()
            .filter(|f| f.cell == key)
            .flat_map(|f| {
                f.markers
                    .iter()
                    .enumerate()
                    .map(move |(i, m)| (f.ref_id, i, m.kind))
            })
            .filter(|m| m.2 != crate::ai::furniture::Use::Idle)
            .collect();
        let missing: Vec<_> = markers
            .iter()
            .filter(|(r, i, _)| {
                !found
                    .iter()
                    .any(|(fr, w)| fr == r && w.marker as usize == *i && !w.child)
            })
            .collect();
        log::debug!(
            "{key:?}: {} of {} furniture markers have no way on: {missing:?}",
            missing.len(),
            markers.len()
        );
        for (r, way) in found {
            if let Some(f) = self.furniture.items.iter_mut().find(|f| f.ref_id == r) {
                f.ways.push(way);
            }
        }
    }

    /// The bone a rigid model (weapon, shield, anim object) hangs from: its root's
    /// `Prn` string.
    pub(crate) fn parent_bone(&mut self, model: &str) -> Option<String> {
        if let Some(r) = self.parent_bones.get(model) {
            return r.clone();
        }
        let r = (|| {
            let nif = nif::Nif::parse(&self.vfs.read(model)?).ok()?;
            let root = nif
                .roots
                .first()
                .and_then(|&r| nif.get(nif::Ref(r as i32)))?
                .av()?;
            root.net.extra_data.iter().find_map(|&e| match nif.get(e) {
                Some(nif::Block::ExtraData(nif::ExtraData::String { name, value }))
                    if name == "Prn" =>
                {
                    Some(value.clone())
                }
                _ => None,
            })
        })();
        self.parent_bones.insert(model.to_owned(), r.clone());
        r
    }

    /// Model path and parent bone of anim object `edid`.
    fn anim_object(&mut self, edid: &str) -> Option<(String, String)> {
        let key = edid.to_ascii_lowercase();
        if let Some(r) = self.anim_objects.get(&key) {
            return r.clone();
        }
        let r = (|| {
            let rec = self.lo.get(self.lo.find_editor_id(edid)?)?;
            if rec.tag().0 != *b"ANIO" {
                return None;
            }
            let model = crate::world::records::mesh_path(&esp::decode_zstring(rec.get(b"MODL")?));
            let bone = self.parent_bone(&model)?;
            Some((model, bone))
        })();
        if r.is_none() {
            log::debug!("anim object {edid}: no model / parent bone");
        }
        self.anim_objects.insert(key, r.clone());
        r
    }

    /// Rebuild the rigid attachments of actor `index` in cell `key` from the anim
    /// objects it holds.
    pub(crate) fn attach_anim_objects(&mut self, key: CellKey, index: usize, objects: &[String]) {
        let found: Vec<(String, String)> =
            objects.iter().filter_map(|o| self.anim_object(o)).collect();
        let paths: Vec<String> = found.iter().map(|(m, _)| m.clone()).collect();
        self.models.load_all(&mut self.renderer, &self.vfs, &paths);
        let Some(skel) = self
            .cells
            .get(&key)
            .and_then(|rt| rt.actors.get(index))
            .map(|a| a.skeleton.clone())
        else {
            return;
        };
        let mut attachments = Vec::new();
        for (model, bone) in &found {
            let (Some(m), Some(b)) = (self.models.get(model), skel.find(bone)) else {
                log::debug!("anim object {model}: not loaded or no bone {bone:?}");
                continue;
            };
            attachments.push((m, b, glam::Mat4::IDENTITY));
        }
        if let Some(inst) = self
            .scene
            .cells
            .get_mut(&key)
            .and_then(|rc| rc.actors.get_mut(index))
        {
            inst.attachments = attachments;
        }
    }

    fn load_navmeshes(&mut self, key: CellKey, cell: FormId) {
        let ids = self.nav.load_cell(&self.lo, cell);
        log::debug!("{cell}: {} navmeshes", ids.len());
        if let Some(rt) = self.cells.get_mut(&key) {
            rt.navmeshes.extend(ids);
        }
    }

    /// The behaviour event an IDLE record plays.
    pub fn idle_event(&mut self, idle: FormId) -> Option<String> {
        self.idles
            .get_or_insert_with(|| crate::ai::idles::IdleIndex::build(&self.lo))
            .humanoid_event(idle)
    }

    fn load_furniture(&mut self, key: CellKey, refs: &[FormId]) {
        let off: Vec<FormId> = refs
            .iter()
            .copied()
            .filter(|&r| self.is_disabled(r))
            .collect();
        let idles = self
            .idles
            .get_or_insert_with(|| crate::ai::idles::IdleIndex::build(&self.lo));
        let (models, vfs) = (&mut self.models, &self.vfs);
        self.furniture.add_cell(&self.lo, idles, key, refs, |path| {
            models.furniture(vfs, path)
        });
        for r in off {
            self.furniture.set_disabled(r, true);
        }
        self.find_furniture_ways(key);
        let (n, m) = self.furniture.count(key);
        log::debug!("{key:?}: {n} furniture with {m} markers");
        for f in self.furniture.items.iter().filter(|f| f.cell == key) {
            log::trace!("furniture {} owner {:?} {:?}", f.ref_id, f.owner, f.markers);
        }
    }

    fn unload_cell(&mut self, key: CellKey) {
        self.remember_actors(key);
        self.scene.cells.remove(&key);
        self.unload_loose(key);
        self.park_object_graphs(key);
        // References' scripts stay attached, keeping their state (and `OnInit`
        // having run) for when the cell loads again.
        if let Some(rt) = self.cells.remove(&key) {
            self.physics.remove_colliders(&rt.colliders);
            if let Some(a) = &self.audio {
                for (_, v) in &rt.sounds {
                    a.stop(*v);
                }
            }
            self.nav.unload(&rt.navmeshes);
            self.furniture.remove_cell(key);
            for a in &rt.actors {
                self.furniture.release(a.ref_id);
                self.moved_refs.remove(&a.ref_id);
                self.actor_cells.remove(&a.ref_id);
            }
        }
    }

    pub(crate) fn rebuild_lights(&mut self) {
        let lights: Vec<PointLight> = self
            .cells
            .values()
            .flat_map(|c| c.lights.iter())
            .filter(|l| !self.is_disabled(l.ref_id))
            .copied()
            .collect();
        self.scene.lights = lights
            .iter()
            .map(|l| GpuLight {
                pos_radius: [l.position.x, l.position.y, l.position.z, l.radius],
                color: [l.color.x, l.color.y, l.color.z, 1.0],
            })
            .collect();
        self.static_lights = self.scene.lights.len();
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
        self.lod = None;
        self.scene.lod.clear();
        self.scene.env = interior_environment(&contents.lighting);
        let key = CellKey::Interior(cell_id);
        let (mut objects, mut lights, mut doors) = (
            contents.objects.clone(),
            contents.lights.clone(),
            contents.doors.clone(),
        );
        self.apply_moves(
            crate::ai::schedule::Place::Interior(cell_id),
            &mut objects,
            &mut lights,
            &mut doors,
        );
        let made = self.created_objects(crate::ai::schedule::Place::Interior(cell_id));
        let made_ids: Vec<FormId> = made.iter().map(|o| o.ref_id).collect();
        objects.extend(made);
        self.instantiate(key, &objects, lights, doors);
        let refs: Vec<FormId> = self
            .lo
            .cell(cell_id)
            .map(|c| {
                c.persistent
                    .iter()
                    .chain(c.temporary.iter())
                    .copied()
                    .collect()
            })
            .unwrap_or_default();
        self.load_navmeshes(key, cell_id);
        self.load_furniture(key, &refs);
        let actors = self.actors_for_cell(key, &refs);
        self.spawn_actors(key, &actors);
        self.load_triggers(key, &refs);
        self.load_activate_parents(&refs);
        self.attach_cell_scripts(&refs);
        self.attach_cell_scripts(&made_ids);
        self.start_cell_sounds(key, &refs);
        if contents.info.has_water
            && let Some(h) = contents.info.water_height
            && h < 1.0e30
        {
            let wt = if contents.info.water_type.is_null() {
                FormId(0x18)
            } else {
                contents.info.water_type
            };
            self.add_water(key, vec![(-50_000.0, -50_000.0, 100_000.0, h, wt)]);
        }
        self.rebuild_lights();
        self.location = Location::Interior(cell_id);
        self.physics.step(1.0 / 60.0);

        let (pos, yaw) =
            spawn.unwrap_or_else(|| self.default_interior_spawn(cell_id, &contents.objects));
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
        let wi = self
            .lo
            .world(world)
            .context("worldspace not indexed")?
            .clone();
        // Bucket the worldspace's persistent references by grid cell.
        if let Some(pc) = wi.persistent_cell
            && let Some(idx) = self.lo.cell(pc)
        {
            for &r in idx.persistent.iter().chain(idx.temporary.iter()) {
                if let Some(rec) = self.lo.get(r) {
                    let rf = records::reference(&rec);
                    self.world_persistent
                        .entry(grid_of(rf.position.truncate()))
                        .or_default()
                        .push(r);
                }
            }
        }
        self.setup_weather(world);
        self.scene.env = self.sky_environment().unwrap_or_default();
        // Distant LOD (child worldspaces may use their parent's).
        let lod_world = self
            .world_field(world, b"EDID", 0x2)
            .map(|(d, _)| esp::decode_zstring(&d))
            .unwrap_or_default();
        // Distant LOD is experimental (block transforms still wrong); opt in with VRM_LOD=1.
        self.lod = if std::env::var_os("VRM_LOD").is_some() {
            Some(crate::world::lod::Lod::new(&self.vfs, &lod_world)).filter(|l| !l.is_empty())
        } else {
            None
        };
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
        self.apply_moves(
            crate::ai::schedule::Place::Exterior(world, (x, y)),
            &mut objects,
            &mut lights,
            &mut doors,
        );
        let made = self.created_objects(crate::ai::schedule::Place::Exterior(world, (x, y)));
        let made_ids: Vec<FormId> = made.iter().map(|o| o.ref_id).collect();
        objects.extend(made);
        self.instantiate(key, &objects, lights, doors);
        let mut refs: Vec<FormId> = cell_id
            .and_then(|c| self.lo.cell(c))
            .map(|c| {
                c.persistent
                    .iter()
                    .chain(c.temporary.iter())
                    .copied()
                    .collect()
            })
            .unwrap_or_default();
        refs.extend(
            self.world_persistent
                .get(&(x, y))
                .cloned()
                .unwrap_or_default(),
        );
        if let Some(cid) = cell_id {
            self.load_navmeshes(key, cid);
        }
        self.load_furniture(key, &refs);
        let actors = self.actors_for_cell(key, &refs);
        self.spawn_actors(key, &actors);
        self.load_triggers(key, &refs);
        self.load_activate_parents(&refs);
        self.attach_cell_scripts(&refs);
        self.attach_cell_scripts(&made_ids);
        self.start_cell_sounds(key, &refs);

        // Landscape
        let dnam = self.world_field(world, b"DNAM", 0x1).map(|d| d.0);
        let default_height = dnam
            .as_ref()
            .map(|d| f32::from_le_bytes(d[0..4].try_into().unwrap()))
            .unwrap_or(-2048.0);
        let default_water = dnam
            .as_ref()
            .map(|d| f32::from_le_bytes(d[4..8].try_into().unwrap()))
            .unwrap_or(0.0);
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
                let wt = if info.water_type.is_null() {
                    world_water.unwrap_or(FormId(0x18))
                } else {
                    info.water_type
                };
                self.add_water(
                    key,
                    vec![(x as f32 * CELL_SIZE, y as f32 * CELL_SIZE, CELL_SIZE, h, wt)],
                );
            }
        }
    }

    fn add_water(&mut self, key: CellKey, planes: Vec<(f32, f32, f32, f32, FormId)>) {
        let mut params = Vec::new();
        if let Some(rt) = self.cells.get_mut(&key) {
            rt.water.extend(
                planes
                    .iter()
                    .map(|&(x, y, size, h, _)| (glam::Vec2::new(x, y), size, h)),
            );
        }
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
        let Location::Exterior { world, center } = self.location else {
            return;
        };
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
            self.pending_loads
                .sort_by_key(|(x, y)| std::cmp::Reverse((x - g.0).abs() + (y - g.1).abs()));
            if !to_unload.is_empty() {
                self.rebuild_lights();
            }
        }
        let mut loaded = false;
        for _ in 0..2 {
            let Some((x, y)) = self.pending_loads.pop() else {
                break;
            };
            self.load_exterior_cell(world, x, y);
            loaded = true;
        }
        if loaded {
            self.rebuild_lights();
        }
    }

    /// Diagnostics (console `probe`): what collision lies along a vertical line at
    /// `p`, against the landscape height there, and each loaded cell's colliders.
    pub fn probe(&self, p: Vec2) -> Vec<String> {
        let top = 2000.0;
        let origin = Vec3::new(p.x, p.y, self.player.position.z + top);
        let mut out = vec![format!(
            "probe at {:.1} {:.1} (cell {:?}); player centre z {:.1}; land height {}; water {}",
            p.x,
            p.y,
            grid_of(p),
            self.player.position.z,
            self.ground_height(p)
                .map_or("none".into(), |h| format!("{h:.1}")),
            self.water_level(p.extend(0.0))
                .map_or("none".into(), |h| format!("{h:.1}"))
        )];
        let cell_of = |h: ColliderHandle| {
            self.cells
                .iter()
                .find(|(_, rt)| rt.colliders.contains(&h))
                .map(|(k, _)| *k)
        };
        for hit in self.physics.probe_ray(origin, -Vec3::Z, 2.0 * top) {
            let what = match hit.owner {
                Some(r) => {
                    let base = self.base_of(r);
                    let edid = base
                        .and_then(|b| self.lo.get(b))
                        .and_then(|b| b.editor_id())
                        .unwrap_or_default();
                    format!("{r} ({edid})")
                }
                None => "no owner (terrain?)".into(),
            };
            let at = origin - Vec3::Z * hit.toi;
            let material = self
                .physics
                .surface_below(at + Vec3::Z, 2.0)
                .map(|(_, s)| self.surface_material(s, at))
                .and_then(|m| self.lo.get(m))
                .and_then(|m| m.editor_id())
                .unwrap_or_default();
            out.push(format!(
                "  z {:.1}: {:?} {what} {material}, cell {:?}, bounds {:.0} .. {:.0}{}{}",
                at.z,
                hit.handle.0.into_raw_parts(),
                cell_of(hit.handle),
                hit.bounds.0,
                hit.bounds.1,
                if hit.in_broad_phase {
                    ""
                } else {
                    ", NOT IN BROAD PHASE"
                },
                if hit.enabled { "" } else { ", disabled" },
            ));
        }
        let mut keys: Vec<_> = self.cells.keys().copied().collect();
        keys.sort_by_key(|k| format!("{k:?}"));
        for k in keys {
            let rt = &self.cells[&k];
            let stale = rt
                .colliders
                .iter()
                .filter(|h| !self.physics.has_collider(**h))
                .count();
            out.push(format!(
                "  {k:?}: {} colliders ({stale} gone), land {}",
                rt.colliders.len(),
                if rt.land.is_some() { "yes" } else { "no" }
            ));
        }
        let listed: HashSet<ColliderHandle> = self
            .cells
            .values()
            .flat_map(|rt| rt.colliders.iter().copied())
            .collect();
        let total = self.physics.world.colliders.len();
        let orphans = self
            .physics
            .world
            .colliders
            .iter()
            .filter(|(h, _)| !listed.contains(h))
            .count();
        out.push(format!(
            "  {total} colliders in the world, {orphans} not listed by any loaded cell"
        ));
        out
    }

    pub fn ground_height(&self, p: Vec2) -> Option<f32> {
        let (x, y) = grid_of(p);
        self.cells
            .get(&CellKey::Exterior(x, y))
            .and_then(|c| c.land.as_ref())
            .map(|l| l.height_at(p))
    }

    /// Read a worldspace subrecord, following the parent worldspace when the
    /// matching "use parent" flag (PNAM) is set.
    pub fn world_field(
        &self,
        world: FormId,
        tag: &[u8; 4],
        parent_flag: u16,
    ) -> Option<(Vec<u8>, FormId)> {
        let mut w = world;
        for _ in 0..4 {
            let rec = self.lo.get(w)?;
            let parent = rec
                .get(b"WNAM")
                .map(|d| rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))));
            let flags = rec
                .get(b"PNAM")
                .map(|d| u16::from_le_bytes([d[0], d[1]]))
                .unwrap_or(0);
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
        let climate = clmt
            .and_then(|c| weather::load_climate(&self.lo, c))
            .unwrap_or_default();
        let forced = self
            .forced_weather
            .as_ref()
            .and_then(|w| self.resolve_form(w));
        let wid = forced.or_else(|| climate.weathers.iter().max_by_key(|w| w.1).map(|w| w.0));
        let Some(w) = wid.and_then(|w| weather::load_weather(&self.lo, w)) else {
            self.sky = None;
            return;
        };
        log::info!("weather {} ({} cloud layers)", w.editor_id, w.clouds.len());
        let mut tex: Vec<String> = vec![climate.sun_texture.clone()];
        tex.extend(w.clouds.iter().take(4).map(|c| c.texture.clone()));
        let missing: Vec<String> = tex
            .iter()
            .filter(|t| !self.renderer.textures.contains(t))
            .cloned()
            .collect();
        loader::load_textures(&mut self.renderer, &self.vfs, missing);
        let get = |p: &String| self.renderer.textures.get(p).flatten();
        let sun = get(&climate.sun_texture).unwrap_or_else(|| self.renderer.white.clone());
        let clouds = w
            .clouds
            .iter()
            .take(4)
            .filter_map(|c| get(&c.texture))
            .collect();
        let (dev, sampler, black) = (
            &self.renderer.device,
            &self.renderer.sampler,
            self.renderer.black.clone(),
        );
        self.renderer
            .sky
            .set_textures(dev, sampler, sun, clouds, black);
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
    pub fn update(&mut self, mut input: crate::player::MoveInput, dt: f32, time_scale: f32) {
        self.scripts.wall_time += dt as f64;
        if self.in_menu_mode() {
            self.update_menu_mode(dt);
            return;
        }
        if self.test_walk > 0 {
            self.test_walk -= 1;
            input.forward = 1.0;
        }
        let off = self.disabled_controls;
        if off.movement {
            input = crate::player::MoveInput::default();
        }
        if off.fighting {
            self.player_blocking = false;
            self.player_attack_held = None;
        }
        if off.sneaking {
            self.player.sneaking = false;
        }
        self.renderer.time += dt;
        let h = self.hour + dt * time_scale / 3600.0;
        if h >= 24.0 {
            self.day += 1;
        }
        self.hour = h.rem_euclid(24.0);
        self.update_scripts(dt);
        self.update_activations();
        self.update_scenes();
        if let Some(env) = self.sky_environment() {
            self.scene.env = env;
        }
        self.update_grab(dt);
        self.physics.step(dt);
        self.update_loose();
        self.update_traps();
        self.update_whereabouts(dt);
        self.update_actors(dt);
        self.update_triggers();
        self.update_story();
        self.update_projectiles(dt);
        self.update_held_lights();
        self.update_animated(dt);
        let cam = self.camera_copy();
        let mut input = input;
        if input.sprint && (input.forward != 0.0 || input.right != 0.0) && !self.player.noclip {
            input.sprint = self.player_sprint(dt);
            // Sprinting stands them up.
            self.player.sneaking &= !input.sprint;
        }
        self.update_player_attack(dt);
        self.update_lockpick(dt);
        let (run, sprint, before) = (input.run, input.sprint, self.player.position);
        if self.test_jump && self.player.grounded {
            input.jump = true;
            self.test_jump = false;
        }
        let sneak_speed = self.player_sneak_speed();
        self.player
            .update(&mut self.physics, &cam, input, sneak_speed, dt);
        let stride = (self.player.position - before).truncate().length();
        self.player.moving = dt > 0.0 && stride / dt > 1.0;
        self.player.running = self.player.moving && (run || sprint);
        self.update_player_footsteps(dt, stride, run, sprint);
        self.camera.position = self.player.eye();
        self.update_streaming();
        self.update_lod();
        self.update_look_target();
        if let Some(a) = self.audio.as_mut() {
            a.set_listener(self.camera.position, self.camera.right());
            a.update();
        }
        self.update_music();
        self.update_conversation();
        self.update_barks();
        self.update_imagespace(dt);
        // The player gets back up a few seconds after dying (until there are saves).
        if let Some(t) = self.player_died_at
            && self.scripts.real_time - t > 5.0
        {
            self.player_died_at = None;
            self.player_health = self.player_max_health();
            self.player_stamina = self.player_stats().max_stamina;
            self.scripts.notify("You come to.");
        }
    }

    /// A frame in menu mode: the world and its clocks stand still; scripts run
    /// (only `WaitMenuMode` counts time), the lock being picked turns, sound plays.
    fn update_menu_mode(&mut self, dt: f32) {
        self.update_scripts(0.0);
        self.update_lockpick(dt);
        if let Some(a) = self.audio.as_mut() {
            a.update();
        }
        self.update_music();
    }

    fn update_look_target(&mut self) {
        let before = self.look_target.take().map(|t| t.0);
        // Holding something, the player has no prompt.
        if self.grab.held.is_none() {
            self.find_look_target();
        }
        if self.look_target.as_ref().map(|t| t.0) != before {
            log::debug!("looking at {:?}", self.look_target);
        }
    }

    fn find_look_target(&mut self) {
        let Some((_, Some(owner))) =
            self.physics
                .raycast(self.camera.position, self.camera.forward(), 220.0)
        else {
            return;
        };
        let Some(rf) = self.reference_of(owner) else {
            return;
        };
        if crate::activation::parent_activate_only(&self.lo, owner) {
            return;
        }
        let Some(base) = self.lo.get(rf.base) else {
            return;
        };
        let mut name = self.actor_name(owner).unwrap_or_else(|| {
            base.get(b"FULL")
                .map(|d| self.lo.lstring(&base, d))
                .unwrap_or_default()
        });
        if base.tag().0 == *b"DOOR"
            && let Some((dest, _, _)) = rf.teleport
        {
            let target = self
                .lo
                .cell_of_ref(dest)
                .and_then(|c| records::cell_info(&self.lo, c));
            let cname = target
                .map(|c| {
                    if c.name.is_empty() {
                        c.editor_id
                    } else {
                        c.name
                    }
                })
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
        let Some((id, _)) = &self.look_target else {
            return "";
        };
        let actor = self.created(*id).map_or_else(
            || self.lo.tag_of(*id).is_some_and(|t| t.0 == *b"ACHR"),
            |c| c.actor,
        );
        if actor {
            return if self.is_dead(*id) {
                "Search"
            } else if self.can_pickpocket(*id) {
                "Pickpocket"
            } else {
                "Talk"
            };
        }
        let Some(base) = self.base_of(*id) else {
            return "Activate";
        };
        let owned =
            crate::ai::furniture::owner_of(&self.lo, *id).is_some_and(|o| self.owned_by_other(o));
        match self.lo.tag_of(base).map(|t| t.0) {
            Some(t) if t == *b"DOOR" => "Open",
            Some(t) if t == *b"CONT" && owned => "Steal from",
            Some(t) if t == *b"CONT" => "Search",
            Some(t) if t == *b"FURN" => {
                let sleep = self.furniture.get(*id).is_some_and(|f| {
                    f.markers
                        .iter()
                        .any(|m| m.kind == crate::ai::furniture::Use::Sleep)
                });
                if sleep { "Sleep" } else { "Sit" }
            }
            Some(t) if t == *b"BOOK" => "Read",
            Some(t) if t == *b"FLOR" => "Harvest",
            Some(t) if t == *b"ACTI" => "Activate",
            _ if self.is_item_ref(*id) && self.stolen_from(base, *id, false).is_some() => "Steal",
            _ => "Take",
        }
    }

    /// Whether the activation prompt's verb is a crime (shown red: the game's
    /// `sSteal` / `sStealFrom` / `sPickpocket`).
    pub fn look_verb_is_crime(&self) -> bool {
        matches!(self.look_verb(), "Steal" | "Steal from" | "Pickpocket")
    }

    pub fn location_name(&self) -> String {
        match self.location {
            Location::Interior(c) | Location::Exterior { world: c, .. } => self
                .lo
                .get(c)
                .map(|r| {
                    let n = r
                        .get(b"FULL")
                        .map(|d| self.lo.lstring(&r, d))
                        .unwrap_or_default();
                    if n.is_empty() {
                        r.editor_id().unwrap_or_default()
                    } else {
                        n
                    }
                })
                .unwrap_or_default(),
            Location::Nowhere => String::new(),
        }
    }

    /// Activate whatever the player is looking at.
    pub fn activate(&mut self) -> Result<()> {
        let Some((owner, name)) = self.look_target.clone() else {
            return Ok(());
        };
        if self.disabled_controls.activate {
            log::debug!("activation disabled: {owner} ({name})");
            return Ok(());
        }
        let rf = self.reference_of(owner).context("reference vanished")?;
        let is_actor = self.created(owner).map_or_else(
            || self.lo.tag_of(owner).is_some_and(|t| t.0 == *b"ACHR"),
            |c| c.actor,
        );
        let base_tag = self.lo.tag_of(rf.base);
        log::info!("activate {owner} ({name})");
        // Actors in a scene that says so can't be talked to.
        if is_actor && !self.is_dead(owner) && self.scene_blocks_activation(owner) {
            let text = self
                .gmst_string("sSceneBlockingActorActivation")
                .unwrap_or_default();
            if !text.is_empty() {
                self.scripts.notify(text.replace("%s", &name));
            }
            return Ok(());
        }
        {
            let mut vm = std::mem::take(&mut self.vm);
            let player = self.object_value(PLAYER_REF);
            let mut host = crate::script::EngineHost { engine: self };
            vm.send_event(
                &mut host,
                papyrus::ObjectId::Form(owner.0),
                "OnActivate",
                vec![player.clone()],
            );
            self.vm = vm;
            // And to the aliases it fills.
            for obj in self.objects_of_ref(owner).into_iter().skip(1) {
                self.scripts
                    .pending_events
                    .push((obj, "OnActivate".into(), vec![player.clone()]));
            }
        }
        if self.scripts.blocked_activation.contains(&owner) {
            return Ok(());
        }
        self.activate_children(owner);
        if is_actor && self.is_dead(owner) {
            // Searching the body.
            self.menu = Some(crate::items::Menu::Container(owner));
            return Ok(());
        }
        if is_actor && self.start_pickpocket(owner, &name) {
            return Ok(());
        }
        if is_actor {
            self.start_conversation(owner);
            return Ok(());
        }
        if base_tag.map(|t| t.0) == Some(*b"CONT") {
            if !self.player_unlock(owner, owner) {
                return Ok(());
            }
            self.menu = Some(crate::items::Menu::Container(owner));
            return Ok(());
        }
        if self.serves_sentence_in(owner) {
            self.menu = Some(crate::items::Menu::ServeSentence);
            return Ok(());
        }
        if base_tag.map(|t| t.0) == Some(*b"BOOK") {
            self.menu = Some(crate::items::Menu::Book {
                book: rf.base,
                reference: Some(owner),
            });
            return Ok(());
        }
        if self.is_item_ref(owner) {
            self.take_item(owner);
            return Ok(());
        }
        if base_tag.map(|t| t.0) == Some(*b"DOOR") {
            if let Some(lock) = self.door_lock_for_player(owner, rf.teleport.map(|t| t.0))
                && !self.player_unlock(lock, owner)
            {
                return Ok(());
            }
            match rf.teleport {
                Some((dest, pos, rot)) => self.teleport_through(dest, pos, rot.z)?,
                None => {
                    self.toggle_door(owner, false);
                }
            }
        }
        Ok(())
    }

    /// Move the player to a door destination (as stored in XTEL).
    pub fn teleport_through(&mut self, dest_door: FormId, pos: Vec3, yaw: f32) -> Result<()> {
        let cell_id = self
            .lo
            .cell_of_ref(dest_door)
            .context("destination door has no cell")?;
        let idx = self.lo.cell(cell_id).cloned().unwrap_or_default();
        match idx.world {
            Some(world) => self.enter_exterior(world, pos, yaw),
            None => self.enter_interior(cell_id, Some((pos, yaw))),
        }
    }

    /// Doors with teleport destinations in the loaded cells.
    pub fn load_doors(&self) -> Vec<Door> {
        self.cells
            .values()
            .flat_map(|c| c.doors.iter())
            .filter(|d| d.destination.is_some() && !self.is_disabled(d.ref_id))
            .cloned()
            .collect()
    }

    /// `coc`-style entry: an interior cell, or an exterior cell by editor id.
    pub fn center_on_cell(&mut self, id: FormId) -> Result<()> {
        let idx = self.lo.cell(id).cloned().unwrap_or_default();
        match (idx.world, idx.grid) {
            (Some(w), Some((x, y))) => {
                let p = Vec3::new(
                    x as f32 * CELL_SIZE + CELL_SIZE * 0.5,
                    y as f32 * CELL_SIZE + CELL_SIZE * 0.5,
                    -100_000.0,
                );
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
        if let Some(c) = self.created(id) {
            return if c.actor { "Actor" } else { "ObjectReference" };
        }
        self.lo
            .tag_of(id)
            .map(|t| crate::script::types::class_for_tag(&t.0))
            .unwrap_or("Form")
    }

    pub fn object_value(&self, id: FormId) -> papyrus::Value {
        if id.is_null()
            || (self.lo.locate(id).is_none() && id != PLAYER_REF && self.created(id).is_none())
        {
            return papyrus::Value::None;
        }
        papyrus::Value::Object(papyrus::ObjectId::Form(id.0), self.native_class(id).into())
    }

    pub fn form_from_file(&self, local: u32, file: &str) -> Option<FormId> {
        let p = self
            .lo
            .plugins()
            .iter()
            .find(|p| p.plugin.name().eq_ignore_ascii_case(file))?;
        let id = match p.slot {
            esp::Slot::Full(i) => FormId(((i as u32) << 24) | (local & 0x00FF_FFFF)),
            esp::Slot::Light(j) => FormId(0xFE00_0000 | ((j as u32) << 12) | (local & 0xFFF)),
        };
        self.lo.locate(id).map(|_| id)
    }

    pub fn form_name(&self, id: FormId) -> String {
        if let Some(name) = self.actor_name(id) {
            return name;
        }
        let Some(rec) = self.lo.get(id) else {
            return String::new();
        };
        let rec = if matches!(&rec.tag().0, b"REFR" | b"ACHR") {
            match self.lo.get(records::reference(&rec).base) {
                Some(b) => b,
                None => return String::new(),
            }
        } else {
            rec
        };
        rec.get(b"FULL")
            .map(|d| self.lo.lstring(&rec, d))
            .unwrap_or_default()
    }

    /// An actor's name: from the template that gives it its base data ("Use
    /// Base Data"), so leveled and templated actors (bandits) are named.
    /// `None` for anything but an actor reference (or the player).
    pub fn actor_name(&self, r: FormId) -> Option<String> {
        let is_actor = r == PLAYER_REF
            || self.created(r).is_some_and(|c| c.actor)
            || self.lo.tag_of(r).is_some_and(|t| t.0 == *b"ACHR");
        if !is_actor {
            return None;
        }
        let rec =
            self.templates_of(r)?
                .record(&self.lo, crate::world::template::BASE_DATA, b"FULL")?;
        rec.get(b"FULL").map(|d| self.lo.lstring(&rec, d))
    }

    /// Where an actor takes each part of its definition from: a reference (picking
    /// its leveled base and templates as it was spawned), the player, or an NPC record.
    pub fn templates_of(&self, actor: FormId) -> Option<crate::world::template::Sources> {
        use crate::world::template::Sources;
        if actor == PLAYER_REF {
            return Some(Sources::of_npc(&self.lo, FormId(0x7), 0));
        }
        if let Some(c) = self.created(actor).filter(|c| c.actor) {
            return Sources::for_ref(&self.lo, c.base, actor.0 as u64);
        }
        let rec = self.lo.get(actor)?;
        match &rec.tag().0 {
            b"ACHR" => Sources::for_ref(&self.lo, records::reference(&rec).base, actor.0 as u64),
            b"NPC_" => Some(Sources::of_npc(&self.lo, actor, 0)),
            _ => None,
        }
    }

    pub fn has_keyword(&self, form: FormId, kw: FormId) -> bool {
        if let Some(c) = self.created(form).filter(|c| !c.actor) {
            return self.has_keyword(c.base, kw);
        }
        if form == PLAYER_REF
            || self.created(form).is_some()
            || self.lo.get(form).is_some_and(|r| r.tag().0 == *b"ACHR")
        {
            // Actors: their keywords part's, and their race's.
            let Some(t) = self.templates_of(form) else {
                return false;
            };
            let race = self
                .lo
                .get(t.of(crate::world::template::TRAITS))
                .and_then(|r| {
                    r.get(b"RNAM")
                        .map(|d| r.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
                });
            return t.keywords(&self.lo).contains(&kw)
                || race.is_some_and(|r| self.has_keyword(r, kw));
        }
        let Some(rec) = self.lo.get(form) else {
            return false;
        };
        let rec = if matches!(&rec.tag().0, b"REFR" | b"ACHR") {
            match self.lo.get(records::reference(&rec).base) {
                Some(b) => b,
                None => return false,
            }
        } else {
            rec
        };
        rec.get(b"KWDA").is_some_and(|d| {
            d.chunks_exact(4)
                .any(|c| rec.fid(FormId(u32::from_le_bytes(c.try_into().unwrap()))) == kw)
        })
    }

    pub fn base_of(&self, r: FormId) -> Option<FormId> {
        if r == PLAYER_REF {
            return Some(FormId(0x7));
        }
        if let Some(c) = self.created(r) {
            return Some(c.base);
        }
        let rec = self.lo.get(r)?;
        Some(records::reference(&rec).base)
    }

    pub fn ref_position(&self, r: FormId) -> Option<Vec3> {
        if r == PLAYER_REF {
            return Some(
                self.player.position
                    - Vec3::Z * (self.physics.player_half_height + self.physics.player_radius),
            );
        }
        if let Some(p) = self.moved_refs.get(&r) {
            return Some(*p);
        }
        // Loaded actors spawned away from their editor place (schedules) before
        // they move.
        if let Some(a) = self.actor_ref(r) {
            return Some(a.pos);
        }
        if let Some(m) = self.world_state.moved.get(&r) {
            return Some(m.pos);
        }
        if let Some(c) = self.created(r) {
            return match c.container {
                Some(holder) => self.ref_position(holder),
                None => Some(c.position),
            };
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
        self.lo
            .get(npc)
            .and_then(|r| r.get(b"ACBS").map(|d| d[0] & 1 != 0))
            .unwrap_or(false)
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

    /// The inventory of a reference; a container's starts out with its contents.
    pub fn inventory_mut(&mut self, r: FormId) -> &mut crate::world::inventory::Inventory {
        if !self.inventories.contains_key(&r) {
            let inv = self
                .base_of(r)
                .map(|b| crate::world::inventory::container_inventory(&self.lo, b, r.0 as u64))
                .unwrap_or_default();
            self.inventories.insert(r, inv);
        }
        self.inventories.get_mut(&r).unwrap()
    }

    /// How many of `item` (or of the forms in a form list) a reference holds.
    pub fn item_count(&self, r: FormId, item: FormId) -> i32 {
        let base = || {
            self.base_of(r)
                .map(|b| crate::world::inventory::container_inventory(&self.lo, b, r.0 as u64))
        };
        let held = self
            .inventories
            .get(&r)
            .cloned()
            .or_else(base)
            .unwrap_or_default();
        if self.lo.tag_of(item).map(|t| t.0) == Some(*b"FLST") {
            self.formlist(item).into_iter().map(|f| held.count(f)).sum()
        } else {
            held.count(item)
        }
    }

    /// Give `count` of `item` to a reference: a form list gives one of each, a
    /// leveled list what it rolls.
    pub fn add_item(&mut self, r: FormId, item: FormId, count: i32) {
        let items: Vec<(FormId, i32)> = match self.lo.tag_of(item).map(|t| t.0) {
            Some(t) if &t == b"FLST" => self
                .formlist(item)
                .into_iter()
                .map(|f| (f, count))
                .collect(),
            Some(t) if &t == b"LVLI" => {
                let seed = self.rand();
                crate::world::actor::resolve_items(&self.lo, item, count, seed, 0)
            }
            _ => vec![(item, count)],
        };
        for (f, n) in items {
            self.inventory_mut(r).add(f, n);
            self.inventory_event(r, true, f, n, None);
        }
    }

    /// Take up to `count` of `item` (or of a form list's forms) from a reference,
    /// handing them to `to` if given. Returns how many were taken.
    pub fn remove_item(&mut self, r: FormId, item: FormId, count: i32, to: Option<FormId>) -> i32 {
        let forms = if self.lo.tag_of(item).map(|t| t.0) == Some(*b"FLST") {
            self.formlist(item)
        } else {
            vec![item]
        };
        let mut total = 0;
        for f in forms {
            total += self
                .remove_stack(r, f, count, None, to, None)
                .iter()
                .map(|(_, n)| n)
                .sum::<i32>();
        }
        total
    }

    /// Take up to `count` of `item` from a reference (only those `only` names
    /// the owner of, if given: see `Inventory::remove_split`), handing them to
    /// `to` if given. Owned (stolen) items stay owned there unless `to` is their
    /// owner; the rest become `mark`'s (stolen from them). Returns what moved,
    /// by owner.
    pub(crate) fn remove_stack(
        &mut self,
        r: FormId,
        item: FormId,
        count: i32,
        only: Option<Option<FormId>>,
        to: Option<FormId>,
        mark: Option<FormId>,
    ) -> Vec<(Option<FormId>, i32)> {
        let parts = self.inventory_mut(r).remove_split(item, count, only);
        let n: i32 = parts.iter().map(|(_, n)| n).sum();
        self.inventory_event(r, false, item, n, to);
        if let Some(to) = to.filter(|_| n > 0) {
            let to_base = self.base_of(to);
            for &(owner, k) in &parts {
                let owner = owner.or(mark).filter(|&o| Some(o) != to_base);
                self.inventory_mut(to).add_owned(item, owner, k);
            }
            self.inventory_event(to, true, item, n, Some(r));
        }
        parts
    }

    pub fn formlist(&self, f: FormId) -> Vec<FormId> {
        let Some(rec) = self.lo.get(f) else {
            return Vec::new();
        };
        rec.subrecords()
            .filter(|s| s.tag.0 == *b"LNAM")
            .map(|s| rec.fid(s.form_id(0)))
            .collect()
    }

    pub fn objective_text(&self, q: FormId, objective: i32) -> String {
        let Some(rec) = self.lo.get(q) else {
            return String::new();
        };
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

    /// Whether a reference is disabled: as scripts left it, else as its enable
    /// parent is (`XESP`; or the opposite, with that flag), else as it starts
    /// (initially disabled).
    pub fn is_disabled(&self, r: FormId) -> bool {
        let mut r = r;
        let mut opposite = false;
        for _ in 0..16 {
            if let Some(d) = self.scripts.disabled.get(&r) {
                return *d != opposite;
            }
            let Some(rec) = self.lo.get(r) else {
                return opposite;
            };
            let parent = rec.get(b"XESP").filter(|d| d.len() >= 5).map(|d| {
                (
                    rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))),
                    d[4] & 1 != 0,
                )
            });
            match parent {
                Some((p, opp)) if !p.is_null() => {
                    opposite ^= opp;
                    r = p;
                }
                _ => return (rec.flags() & esp::record_flags::INITIALLY_DISABLED != 0) != opposite,
            }
        }
        false
    }

    /// References whose enable parent (`XESP`) each reference is, read on first use.
    fn enable_children(&self) -> &HashMap<FormId, Vec<FormId>> {
        self.enable_children.get_or_init(|| {
            let mut out: HashMap<FormId, Vec<FormId>> = HashMap::new();
            for tag in [b"REFR", b"ACHR"] {
                for &r in self.lo.ids_of_type(tag) {
                    let Some(rec) = self.lo.get(r) else { continue };
                    if let Some(d) = rec.get(b"XESP").filter(|d| d.len() >= 4) {
                        out.entry(rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
                            .or_default()
                            .push(r);
                    }
                }
            }
            out
        })
    }

    pub fn set_disabled(&mut self, r: FormId, disabled: bool) {
        self.scripts.disabled.insert(r, disabled);
        // It and the references enabled with it, however deep.
        let mut affected = vec![r];
        let mut i = 0;
        while i < affected.len() && affected.len() < 100_000 {
            let kids = self
                .enable_children()
                .get(&affected[i])
                .cloned()
                .unwrap_or_default();
            affected.extend(
                kids.into_iter()
                    .filter(|k| !self.scripts.disabled.contains_key(k)),
            );
            i += 1;
        }
        let mut lights_changed = false;
        let mut arrivals = Vec::new();
        let mut sounds = Vec::new();
        for r in affected {
            let off = self.is_disabled(r);
            sounds.push((r, off));
            lights_changed |= self
                .cells
                .values()
                .any(|c| c.lights.iter().any(|l| l.ref_id == r));
            self.furniture.set_disabled(r, off);
            // Enabled actors whose place is loaded appear.
            if !off
                && !self.actor_cells.contains_key(&r)
                && self.lo.tag_of(r).is_some_and(|t| t.0 == *b"ACHR")
            {
                let at = self
                    .whereabouts
                    .of
                    .get(&r)
                    .copied()
                    .or_else(|| self.body_of(r))
                    .or_else(|| {
                        let pos = self.reference_of(r)?.position;
                        Some((self.place_of_ref(r, pos)?, pos))
                    });
                if let Some((place, pos)) = at
                    && let Some(key) = self.key_of_place(place)
                {
                    arrivals.push((key, r, pos));
                }
            }
            for rc in self.scene.cells.values_mut() {
                for inst in rc.instances.iter_mut().filter(|inst| inst.ref_id == r.0) {
                    inst.hidden = off;
                }
            }
            self.physics.set_owner_enabled(r, !off);
            if off && self.actor_cells.contains_key(&r) {
                self.despawn_actor(r);
            }
        }
        for (key, r, pos) in arrivals {
            self.spawn_actors(key, &[(r, Some(pos))]);
        }
        // Looping sounds of what is now enabled start, of what is disabled stop.
        let loaded: HashMap<FormId, CellKey> = self
            .cells
            .iter()
            .flat_map(|(k, rt)| rt.refs.iter().map(move |r| (*r, *k)))
            .collect();
        for (r, off) in sounds {
            let Some(&key) = loaded.get(&r) else { continue };
            let playing: Vec<crate::audio::VoiceId> = self.cells[&key]
                .sounds
                .iter()
                .filter(|(s, _)| *s == r)
                .map(|(_, v)| *v)
                .collect();
            if off {
                if let Some(a) = &self.audio {
                    playing.iter().for_each(|v| a.stop(*v));
                }
                if let Some(rt) = self.cells.get_mut(&key) {
                    rt.sounds.retain(|(s, _)| *s != r);
                }
            } else if playing.is_empty() {
                log::debug!("{r} enabled: starting its looping sounds");
                self.start_cell_sounds(key, &[r]);
            }
        }
        if lights_changed {
            self.rebuild_lights();
        }
    }

    pub fn queue_player_moveto(&mut self, target: FormId) {
        self.pending_moveto = Some(target);
    }

    /// Start a quest: fill its aliases and mark it running; its scripts are
    /// attached, sent OnInit and its startup stage run once the VM is free
    /// (`init_quest`), so scripts can start quests. False when it is running
    /// already or a required alias can't be filled.
    pub fn start_quest(&mut self, q: FormId) -> bool {
        self.start_quest_with(q, None)
    }

    /// Start a quest for a Story Manager event: its "from event" aliases and
    /// event data conditions read the event.
    pub fn start_quest_with(&mut self, q: FormId, event: Option<crate::story::StoryEvent>) -> bool {
        if self.scripts.quests.get(&q).is_some_and(|st| st.running) || self.lo.get(q).is_none() {
            return false;
        }
        // Started for an event: the quest's own event conditions, on its data.
        if event.is_some() {
            let conds = self.quest_event_conditions(q);
            if !conds.is_empty() {
                let outer = std::mem::replace(&mut self.story.active, event.clone());
                let ctx = crate::condition::Context {
                    subject: Some(PLAYER_REF),
                    quest: Some(q),
                    ..Default::default()
                };
                let pass = crate::condition::evaluate(self, &conds, ctx);
                if !pass {
                    log::debug!(
                        "{q}: event conditions fail: {}",
                        crate::condition::explain(self, &conds, ctx)
                    );
                }
                self.story.active = outer;
                if !pass {
                    return false;
                }
            }
        }
        self.scripts.quests.entry(q).or_default().event = event;
        if !self.fill_quest_aliases(q) {
            self.scripts.quests.entry(q).or_default().event = None;
            return false;
        }
        self.scripts.quests.entry(q).or_default().running = true;
        self.scripts.alias_gen += 1;
        self.scripts.pending_quest_inits.push(q);
        true
    }

    /// Attach a started quest's scripts (its own and its aliases'), send OnInit,
    /// queue its startup stage and start its scenes.
    fn init_quest(&mut self, q: FormId) {
        if !self.scripts.quests.get(&q).is_some_and(|st| st.running) {
            return;
        }
        let Some(rec) = self.lo.get(q) else { return };
        let vmad = crate::script::vmad::parse(&rec).unwrap_or_default();
        // Startup stage: INDX flags (third byte) 0x2 marks "start up stage".
        let startup = rec
            .subrecords()
            .find(|sr| sr.tag.0 == *b"INDX" && sr.u8(2) & 0x2 != 0)
            .map(|sr| sr.u16(0));
        drop(rec);
        let mut vm = std::mem::take(&mut self.vm);
        {
            let obj = papyrus::ObjectId::Form(q.0);
            // Alias scripts run on the alias objects.
            for (alias, scripts) in &vmad.alias_scripts {
                let aobj = papyrus::ObjectId::Alias {
                    quest: q.0,
                    alias: *alias,
                };
                for s in scripts {
                    let props: Vec<(String, papyrus::Value)> = s
                        .properties
                        .iter()
                        .map(|(n, pv)| {
                            (
                                n.clone(),
                                crate::script::vmad::to_value(pv, &|f| host_class(&self.lo, f)),
                            )
                        })
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
                    .map(|(n, pv)| {
                        (
                            n.clone(),
                            crate::script::vmad::to_value(pv, &|f| host.engine.native_class(f)),
                        )
                    })
                    .collect();
                vm.attach(&mut host, obj, &s.name, &props);
            }
            vm.send_event(&mut host, obj, "OnInit", vec![]);
            // Started by the Story Manager: the event, with its data.
            let event = host
                .engine
                .scripts
                .quests
                .get(&q)
                .and_then(|st| st.event.as_ref())
                .and_then(|e| host.engine.story_papyrus_event(e));
            if let Some((name, args)) = event {
                vm.send_event(&mut host, obj, name, args);
            }
        }
        self.vm = vm;
        if let Some(s) = startup {
            self.scripts.pending_stages.push((q, s));
        }
        self.start_quest_scenes(q);
    }

    /// Initialise the quests started since the last time (outside VM runs).
    pub(crate) fn init_started_quests(&mut self) {
        for q in std::mem::take(&mut self.scripts.pending_quest_inits) {
            self.init_quest(q);
        }
    }

    /// Stop a quest: its aliases empty and its scenes stop.
    pub fn stop_quest(&mut self, q: FormId) {
        let st = self.scripts.quests.entry(q).or_default();
        st.running = false;
        st.aliases.clear();
        st.event = None;
        self.scripts.alias_gen += 1;
        self.stop_quest_scenes(q);
    }

    /// Set a quest stage: record it, add its log entries and run the fragments
    /// of those whose conditions pass (all the stage's when it has no entries).
    fn run_stage(&mut self, q: FormId, stage: u16) {
        {
            let st = self.scripts.quests.entry(q).or_default();
            st.stage = stage;
            st.done.insert(stage);
        }
        let entries = self.add_log_entries(q, stage);
        let Some(rec) = self.lo.get(q) else { return };
        let Some(vmad) = crate::script::vmad::parse(&rec) else {
            return;
        };
        let edid = rec.editor_id().unwrap_or_default();
        drop(rec);
        log::info!("quest {edid} stage {stage}");
        let mut vm = std::mem::take(&mut self.vm);
        {
            let mut host = crate::script::EngineHost { engine: self };
            for f in vmad.fragments.iter().filter(|f| {
                f.stage == stage
                    && entries
                        .as_ref()
                        .is_none_or(|e| e.contains(&(f.log_entry as usize)))
            }) {
                vm.call_method(
                    &mut host,
                    papyrus::ObjectId::Form(q.0),
                    &f.script,
                    &f.function,
                    vec![],
                );
            }
        }
        self.vm = vm;
    }

    /// Each NPC's first placed reference.
    pub fn npc_refs_index(&self) -> &HashMap<FormId, FormId> {
        self.npc_refs.get_or_init(|| {
            let mut out = HashMap::new();
            for &a in self.lo.ids_of_type(b"ACHR") {
                if let Some(r) = self.lo.get(a)
                    && let Some(d) = r.get(b"NAME")
                {
                    let base = r.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap())));
                    out.entry(base).or_insert(a);
                }
            }
            out
        })
    }

    fn update_scripts(&mut self, dt: f32) {
        self.scripts.real_time += dt as f64;
        let now = self.scripts.real_time;
        let game_now = self.game_hours_total();
        // Timers
        let mut fired = Vec::new();
        self.scripts.timers.retain_mut(|t| {
            let due = if t.game_time {
                game_now >= t.at
            } else {
                now >= t.at
            };
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
        vm.menu_time = self.scripts.wall_time;
        for _ in 0..4 {
            {
                let mut host = crate::script::EngineHost { engine: self };
                for (obj, script, ev) in fired.drain(..) {
                    if !vm.send_event_to(&mut host, obj, &script, ev, vec![]) {
                        vm.send_event(&mut host, obj, ev, vec![]);
                    }
                }
                let anim = std::mem::take(&mut host.engine.anim_events.pending);
                for (obj, script, args) in anim {
                    vm.send_event_to(&mut host, obj, &script, "OnAnimationEvent", args);
                }
                let events = std::mem::take(&mut host.engine.scripts.pending_events);
                for (obj, ev, args) in events {
                    vm.send_event(&mut host, obj, &ev, args);
                }
                vm.run(&mut host, now, 20_000);
            }
            let stages = std::mem::take(&mut self.scripts.pending_stages);
            if stages.is_empty()
                && self.scripts.pending_events.is_empty()
                && self.scripts.pending_quest_inits.is_empty()
            {
                break;
            }
            self.vm = vm;
            self.init_started_quests();
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
            let interior = cell
                .and_then(|c| self.lo.cell(c))
                .is_some_and(|c| c.world.is_none());
            let res = match (interior, cell) {
                (true, Some(c)) if self.location != Location::Interior(c) => {
                    self.enter_interior(c, Some((p, 0.0)))
                }
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
        self.update_messages();
    }

    /// Start every quest flagged "start game enabled".
    pub fn start_game_enabled_quests(&mut self) {
        let quests: Vec<FormId> = self
            .lo
            .ids_of_type(b"QUST")
            .iter()
            .copied()
            .filter(|&q| {
                self.lo
                    .get(q)
                    .and_then(|r| {
                        r.get(b"DNAM")
                            .map(|d| u16::from_le_bytes([d[0], d[1]]) & 0x1 != 0)
                    })
                    .unwrap_or(false)
            })
            .collect();
        log::info!("starting {} start-game-enabled quests", quests.len());
        for q in quests {
            self.start_quest(q);
        }
        self.init_started_quests();
    }
}

fn host_class(lo: &LoadOrder, f: FormId) -> &'static str {
    if f == PLAYER_REF {
        return "Actor";
    }
    lo.tag_of(f)
        .map(|t| crate::script::types::class_for_tag(&t.0))
        .unwrap_or("Form")
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
        fog_far: if l.fog_far > l.fog_near {
            l.fog_far
        } else {
            l.fog_near + 1.0
        },
        fog_power: if l.fog_power > 0.0 { l.fog_power } else { 1.0 },
        fog_max: if l.fog_max > 0.0 { l.fog_max } else { 1.0 },
        clear_color: l.fog_far_color,
    }
}
