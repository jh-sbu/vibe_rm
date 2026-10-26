//! Spell visuals, derived from records and live casting/effect state.
pub mod records;

pub mod art;
pub mod effect;
mod enchant;
pub mod particles;
pub mod shaders;
use crate::engine::{Engine, PLAYER_REF};
use art::{ActorFrame, ArtEffect};
use effect::EffectInstance;
use esp::FormId;
use glam::{Mat4, Vec3};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ArtKey {
    Hand(usize, FormId, FormId),
    Hit(FormId),
    Script(FormId),
    Enchant(usize, FormId, FormId),
}
#[derive(Default)]
pub struct SpellFx {
    art: HashMap<(FormId, ArtKey), ArtEffect>,
    shaders: HashMap<(FormId, FormId, u64), shaders::ShaderEffect>,
    pub events: Vec<(FormId, String)>,
    pub hits: Vec<crate::magic::ActiveEffect>,
    observed: std::collections::HashSet<u64>,
    persistent_hits: HashMap<u64, FormId>,
    available_targets: std::collections::HashSet<FormId>,
    missing: std::collections::HashSet<(String, String)>,
    pub lights: Vec<crate::render::GpuLight>,
    pub tails: Vec<EffectInstance>,
    streams: HashMap<(FormId, FormId, crate::casting::CastSource), HeldStream>,
    waves: Vec<ConeWave>,
    model_nifs: HashMap<String, Option<nif::Nif>>,
    model_nodes: std::cell::RefCell<HashMap<String, std::sync::Arc<HashMap<String, Mat4>>>>,
    actor_poses: std::cell::RefCell<HashMap<FormId, CachedActorPose>>,
}

const MODEL_CACHE_CAPACITY: usize = 128;

fn cached_model<'a, T>(
    cache: &'a mut HashMap<String, T>,
    model: &str,
    load: impl FnOnce() -> T,
) -> &'a T {
    if !cache.contains_key(model) {
        if cache.len() >= MODEL_CACHE_CAPACITY {
            let victim = cache.keys().next().cloned().unwrap();
            cache.remove(&victim);
        }
        cache.insert(model.to_owned(), load());
    }
    &cache[model]
}

impl SpellFx {
    fn queue_active_hits(
        &mut self,
        effects: &[crate::magic::ActiveEffect],
        available: std::collections::HashSet<FormId>,
    ) -> std::collections::HashSet<u64> {
        let active: std::collections::HashSet<_> = effects
            .iter()
            .filter(|x| x.active && !x.done)
            .map(|x| x.id)
            .collect();
        self.observed.retain(|id| active.contains(id));
        self.persistent_hits.retain(|id, _| active.contains(id));
        let should_queue = |hit: &crate::magic::ActiveEffect| {
            !self.observed.contains(&hit.id)
                || (self.persistent_hits.contains_key(&hit.id)
                    && available.contains(&hit.target)
                    && !self.available_targets.contains(&hit.target))
        };
        self.hits.retain(&should_queue);
        self.hits.extend(
            effects
                .iter()
                .filter(|x| active.contains(&x.id) && should_queue(x))
                .cloned(),
        );
        self.hits.sort_by_key(|x| x.id);
        self.hits.dedup_by_key(|x| x.id);
        self.available_targets = available;
        active
    }

    fn prune_actor_poses(&self, loaded: &std::collections::HashSet<FormId>) {
        self.actor_poses
            .borrow_mut()
            .retain(|id, _| loaded.contains(id));
    }
}

#[derive(Default)]
struct CachedActorPose {
    skeleton: Option<std::sync::Arc<crate::world::skeleton::Skeleton>>,
    pose: Vec<Mat4>,
    bones: std::sync::Arc<HashMap<String, Mat4>>,
}
impl CachedActorPose {
    fn bones(
        &mut self,
        skeleton: &std::sync::Arc<crate::world::skeleton::Skeleton>,
        pose: &[Mat4],
    ) -> std::sync::Arc<HashMap<String, Mat4>> {
        let same_skeleton = self
            .skeleton
            .as_ref()
            .is_some_and(|s| std::sync::Arc::ptr_eq(s, skeleton));
        if !same_skeleton || self.pose != pose {
            let bones = std::sync::Arc::make_mut(&mut self.bones);
            if !same_skeleton
                || self.pose.len().min(skeleton.bones.len()) != pose.len().min(skeleton.bones.len())
            {
                bones.clear();
                bones.extend(
                    skeleton
                        .bones
                        .iter()
                        .zip(pose)
                        .map(|(b, p)| (b.name.clone(), *p)),
                );
            } else {
                for (bone, transform) in skeleton.bones.iter().zip(pose) {
                    *bones.get_mut(&bone.name).unwrap() = *transform;
                }
            }
            self.skeleton = Some(skeleton.clone());
            self.pose.clear();
            self.pose.extend_from_slice(pose);
        }
        self.bones.clone()
    }
}
fn art_one_shot_done(effect: &EffectInstance) -> bool {
    effect.finished || effect.age >= effect.one_shot_duration()
}

fn enabled() -> bool {
    std::env::var_os("VRM_NO_SPELL_FX").is_none()
}

impl Engine {
    pub(crate) fn load_spell_model(
        &mut self,
        model: &str,
        transform: Mat4,
    ) -> Option<EffectInstance> {
        if !enabled() {
            return None;
        }
        let path = format!("{model}#effect");
        self.models
            .load_all(&mut self.renderer, &self.vfs, std::slice::from_ref(&path));
        let (root, anim) = (self.models.get(&path), self.models.anim(&path));
        if root.is_none() && anim.is_none() {
            return None;
        }
        let project = anim
            .as_ref()
            .and_then(|a| a.graph.as_ref())
            .map(|g| g.0.clone());
        let seed = self.rand();
        let mut effect = EffectInstance::new(&self.renderer, root, anim, transform, seed as u32);
        let nif = cached_model(&mut self.spell_fx.model_nifs, model, || {
            self.vfs.read(model).and_then(|d| nif::Nif::parse(&d).ok())
        });
        if let Some(nif) = nif {
            effect.configure(nif);
        }
        if let Some(project) = project.and_then(|p| self.graphs.project(&self.vfs, &p)) {
            effect.graph = Some(havok::behavior::runtime::Instance::new(
                project.shared.clone(),
                seed,
            ));
        } else {
            effect.play("Idle");
        }
        Some(effect)
    }
    fn load_art(&mut self, id: FormId, default_node: &str) -> Option<ArtEffect> {
        let art = records::ArtObject::load(&self.lo, id)?;
        let model = art.model?;
        let effect = self.load_spell_model(&model, Mat4::IDENTITY)?;
        Some(ArtEffect {
            effect,
            default_node: default_node.into(),
            owners: Vec::new(),
            persistent: false,
            flags: 4,
            facing: None,
            scale: 1.0,
            stopping_age: None,
        })
    }
    fn spell_actor_frame(&self, actor: FormId) -> Option<ActorFrame> {
        if self.is_disabled(actor) {
            return None;
        }
        let (inst, skeleton) = if actor == PLAYER_REF {
            (
                self.scene.player.as_ref()?,
                &self.player_body.as_ref()?.skeleton,
            )
        } else {
            if !self.actor_cells.contains_key(&actor) {
                let inst = self
                    .scene
                    .cells
                    .values()
                    .flat_map(|c| &c.instances)
                    .find(|i| i.ref_id == actor.0 && !i.hidden)?;
                let path = inst
                    .model
                    .path
                    .split('#')
                    .next()
                    .unwrap_or(&inst.model.path);
                let bones = cached_model(&mut self.spell_fx.model_nodes.borrow_mut(), path, || {
                    std::sync::Arc::new(
                        self.vfs
                            .read(path)
                            .and_then(|d| nif::Nif::parse(&d).ok())
                            .map(|n| crate::render::model::node_transforms(&n))
                            .unwrap_or_default(),
                    )
                })
                .clone();
                return Some(ActorFrame {
                    transform: inst.transform,
                    bones,
                });
            }
            let key = self.actor_cells.get(&actor)?;
            let rt = self.cells.get(key)?;
            let index = rt.actors.iter().position(|a| a.ref_id == actor)?;
            (
                self.scene.cells.get(key)?.actors.get(index)?,
                &rt.actors[index].skeleton,
            )
        };
        Some(ActorFrame {
            transform: inst.transform,
            bones: self
                .spell_fx
                .actor_poses
                .borrow_mut()
                .entry(actor)
                .or_default()
                .bones(skeleton, &inst.pose),
        })
    }
    pub(crate) fn update_spell_fx(&mut self, dt: f32) {
        self.scene.membranes.clear();
        if !enabled() {
            self.spell_fx = SpellFx::default();
            return;
        }
        let mut actors = vec![PLAYER_REF];
        for rt in self.cells.values_mut() {
            for a in &mut rt.actors {
                actors.push(a.ref_id);
                self.spell_fx.events.extend(
                    std::mem::take(&mut a.spell_fx_events)
                        .into_iter()
                        .map(|e| (a.ref_id, e)),
                );
            }
        }
        self.spell_fx
            .prune_actor_poses(&actors.iter().copied().collect());
        let mut desired = std::collections::HashSet::new();
        self.spell_fx.lights.clear();
        for actor in actors {
            let drawn = if actor == PLAYER_REF {
                self.player_weapon_drawn()
            } else {
                self.actor_ref(actor).is_some_and(|a| a.drawn)
            };
            if !drawn {
                continue;
            }
            let Some(frame) = self.spell_actor_frame(actor) else {
                continue;
            };
            for hand in [crate::spells::LEFT, crate::spells::RIGHT] {
                let Some(spell) = self
                    .equipped_spell(actor, hand)
                    .or_else(|| self.staff_in(actor, hand).map(|(_, ench)| ench))
                    .or_else(|| self.hand_casting(actor, hand))
                else {
                    continue;
                };
                let Some(item) = self.magic_item(spell) else {
                    continue;
                };
                let Some(mgef) = item
                    .effects
                    .iter()
                    .find_map(|e| self.magic_effect(e.effect))
                else {
                    continue;
                };
                let node = if self.staff_in(actor, hand).is_some() {
                    if hand == crate::spells::RIGHT {
                        "Weapon"
                    } else {
                        "SHIELD"
                    }
                } else if hand == crate::spells::LEFT {
                    "NPC L MagicNode [LMag]"
                } else {
                    "NPC R MagicNode [RMag]"
                };
                if !mgef.casting_art.is_null() {
                    let key = (actor, ArtKey::Hand(hand, spell, mgef.casting_art));
                    desired.insert(key);
                    if !self.spell_fx.art.contains_key(&key) {
                        if let Some(mut art) = self.load_art(mgef.casting_art, node) {
                            art.effect
                                .set_variable("bLeftHand", f32::from(hand == crate::spells::LEFT));
                            art.effect.set_variable(
                                "bRightHand",
                                f32::from(hand == crate::spells::RIGHT),
                            );
                            art.effect.set_variable(
                                "iHandType",
                                f32::from(self.staff_in(actor, hand).is_some()),
                            );
                            art.persistent = true;
                            self.spell_fx.art.insert(key, art);
                        }
                    }
                }
                if !mgef.casting_light.is_null() {
                    if let Some(light) =
                        self.spell_light(mgef.casting_light, frame.fallback(node).w_axis.truncate())
                    {
                        self.spell_fx.lights.push(light);
                    }
                }
            }
        }
        desired.extend(self.update_enchant_fx());
        let targets: std::collections::HashSet<_> = self
            .spell_fx
            .persistent_hits
            .values()
            .copied()
            .chain(
                self.magic
                    .effects
                    .iter()
                    .filter(|x| x.active && !x.done && !self.spell_fx.observed.contains(&x.id))
                    .map(|x| x.target),
            )
            .chain(self.spell_fx.hits.iter().map(|x| x.target))
            .collect();
        let available = targets
            .into_iter()
            .filter(|target| self.spell_actor_frame(*target).is_some())
            .collect();
        let active = self
            .spell_fx
            .queue_active_hits(&self.magic.effects, available);

        for hit in std::mem::take(&mut self.spell_fx.hits) {
            let first = self.spell_fx.observed.insert(hit.id);
            let Some(mgef) = self.magic_effect(hit.effect) else {
                continue;
            };
            let persistent = mgef.has(crate::magic::flags::FX_PERSIST) && hit.duration > 0.0;
            if persistent {
                self.spell_fx.persistent_hits.insert(hit.id, hit.target);
            }
            if (!first && !persistent) || !self.spell_fx.available_targets.contains(&hit.target) {
                continue;
            }
            let mut hit_art = mgef.hit_art;
            let mut hit_shader = mgef.hit_shader;
            let mut art_scale = 1.0;
            if hit.dual
                && let Some(dual) = records::DualCastData::load(&self.lo, mgef.dual_cast.0)
            {
                if !dual.hit_art.is_null() {
                    hit_art = dual.hit_art;
                }
                if !dual.shader.is_null() {
                    hit_shader = dual.shader;
                }
                if dual.inherit_scale & 1 != 0 {
                    art_scale = mgef.dual_cast.1.max(0.01);
                }
            }
            if hit.quiet || mgef.has(crate::magic::flags::NO_HIT_EFFECT) {
                continue;
            }
            if !hit_shader.is_null()
                && !self
                    .spell_fx
                    .shaders
                    .contains_key(&(hit.target, hit_shader, hit.id))
            {
                if let Some(shader) = records::EffectShader::load(&self.lo, hit_shader) {
                    crate::world::loader::load_textures(
                        &mut self.renderer,
                        &self.vfs,
                        shader.textures.iter().flatten().cloned().collect(),
                    );
                    let duration = if persistent {
                        f32::INFINITY
                    } else {
                        shader.float(0x14) + shader.float(0x18)
                    };
                    self.spell_fx.shaders.insert(
                        (hit.target, hit_shader, hit.id),
                        shaders::ShaderEffect::new(shader, duration, persistent),
                    );
                }
            }
            if hit_art.is_null() {
                continue;
            }
            let key = (hit.target, ArtKey::Hit(hit_art));
            let refresh =
                first
                    && self.spell_fx.art.get(&key).is_some_and(|a| {
                        a.stopping_age.is_some() || (!persistent && !a.persistent)
                    });
            if refresh && let Some(mut old) = self.spell_fx.art.remove(&key) {
                old.effect.stop();
                old.effect.fade_duration = 0.0;
                old.effect.finished = true;
                self.spell_fx.tails.push(old.effect);
            }
            if let Some(art) = self.spell_fx.art.get_mut(&key) {
                if !art.owners.contains(&hit.id) {
                    art.owners.push(hit.id);
                }
                art.scale = art_scale;
                art.persistent |= persistent;
                art.stopping_age = None;
            } else if let Some(mut art) = self.load_art(hit_art, "MagicEffectsNode") {
                art.scale = art_scale;
                art.owners.push(hit.id);
                art.persistent = persistent;
                self.spell_fx.art.insert(key, art);
            }
        }
        for (actor, event) in std::mem::take(&mut self.spell_fx.events) {
            for ((target, key), art) in &mut self.spell_fx.art {
                if *target != actor {
                    continue;
                }
                if let ArtKey::Hand(hand, _, _) = key {
                    if event.starts_with("DualMagic_")
                        || (event.starts_with("MLh_") && *hand == crate::spells::LEFT)
                        || (event.starts_with("MRh_") && *hand == crate::spells::RIGHT)
                    {
                        art.effect.send_event(&event);
                    }
                }
            }
        }
        let frames: HashMap<_, _> = self
            .spell_fx
            .art
            .keys()
            .filter_map(|(actor, _)| self.spell_actor_frame(*actor).map(|f| (*actor, f)))
            .collect();
        for ((actor, _), art) in &self.spell_fx.art {
            if let Some(frame) = frames.get(actor) {
                for tags in art.effect.tags.values() {
                    for name in &tags.named_nodes {
                        if !frame.bones.contains_key(name) {
                            let model = art
                                .effect
                                .parts
                                .first()
                                .map(|(i, _)| i.model.path.clone())
                                .unwrap_or_default();
                            if self.spell_fx.missing.insert((model.clone(), name.clone())) {
                                log::warn!(
                                    "spell art {model}: missing attach node {name}; using {} or root",
                                    art.default_node
                                );
                            }
                        }
                    }
                }
            }
        }
        let camera = self.camera_copy();
        let facing: HashMap<_, _> = self
            .spell_fx
            .art
            .values()
            .filter_map(|a| a.facing)
            .filter_map(|r| self.ref_position(r).map(|p| (r, p)))
            .collect();
        for (key, mut art) in std::mem::take(&mut self.spell_fx.art) {
            let Some(frame) = frames.get(&key.0) else {
                continue;
            };
            art.owners.retain(|id| active.contains(id));
            let keep = match key.1 {
                ArtKey::Hand(..) => desired.contains(&key),
                ArtKey::Hit(_) => art.owners.iter().any(|id| active.contains(id)),
                ArtKey::Script(_) => true,
                ArtKey::Enchant(..) => desired.contains(&key),
            };
            if art.persistent && !keep && art.stopping_age.is_none() {
                art.effect.stop();
                art.stopping_age = Some(art.effect.age);
            }
            art.attach(frame);
            if matches!(key.1, ArtKey::Script(_)) {
                if art.flags & 2 != 0 {
                    art.effect.transform = Mat4::from_translation(camera.position);
                    art.effect.anchors.clear();
                }
                if art.flags & 4 == 0 {
                    art.effect.transform =
                        Mat4::from_translation(art.effect.transform.w_axis.truncate());
                }
                if art.flags & 1 != 0 {
                    if let Some(target) = art.facing.and_then(|r| facing.get(&r)) {
                        let from = art.effect.transform.w_axis.truncate();
                        art.effect.transform = Mat4::from_rotation_translation(
                            glam::Quat::from_rotation_arc(
                                Vec3::Y,
                                (*target - from).normalize_or(Vec3::Y),
                            ),
                            from,
                        );
                    }
                }
                if art.effect.age >= art.effect.duration && art.stopping_age.is_none() {
                    art.effect.stop();
                    art.stopping_age = Some(art.effect.age);
                }
            }
            art.effect.advance(dt);
            let one_shot_done = !art.persistent && art_one_shot_done(&art.effect);
            let over = one_shot_done
                || art
                    .stopping_age
                    .is_some_and(|age| art.effect.age - age > 3.0 || art.effect.finished);
            if over {
                art.effect.stop();
                if one_shot_done {
                    art.effect.finished = true;
                    art.effect.fade_duration = 0.0;
                }
                self.spell_fx.tails.push(art.effect);
            } else {
                art.effect.draw(&mut self.scene);
                self.spell_fx.art.insert(key, art);
            }
        }
        for projectile in &mut self.projectiles {
            if let Some(effect) = &mut projectile.fx {
                effect.transform = Mat4::from_scale_rotation_translation(
                    Vec3::splat(effect.scale),
                    glam::Quat::from_rotation_arc(Vec3::Y, projectile.dir.normalize_or(Vec3::Y)),
                    projectile.pos,
                );
                effect.advance(dt);
                effect.draw(&mut self.scene);
            }
        }
        let projectile_lights: Vec<_> = self
            .projectiles
            .iter()
            .filter_map(|p| {
                let fx = p.fx.as_ref()?;
                self.spell_light(fx.light, fx.transform.transform_point3(fx.light_attach))
            })
            .collect();
        self.spell_fx.lights.extend(projectile_lights);
        self.update_stream_fx(dt);
        self.update_shader_fx(dt, &active);
        for effect in &mut self.spell_fx.tails {
            effect.advance(dt);
            if !effect.finished && effect.age - effect.duration < effect.fade_duration {
                effect.draw(&mut self.scene);
            }
        }
        self.spell_fx
            .tails
            .retain(|e| e.age - e.duration < e.fade_duration || e.live_particles());
    }
    pub(crate) fn spell_light(&self, form: FormId, at: Vec3) -> Option<crate::render::GpuLight> {
        let rec = self.lo.get(form).filter(|r| r.tag().0 == *b"LIGH")?;
        let light = crate::world::records::light_data(&rec)?;
        let color = light.color * light.fade * if light.negative() { -1.0 } else { 1.0 };
        Some(crate::render::GpuLight {
            pos_radius: [at.x, at.y, at.z, light.radius],
            color: [color.x, color.y, color.z, 1.0],
        })
    }
    pub(crate) fn step_spell_fx(
        &mut self,
        dt: f32,
        right: Vec3,
        up: Vec3,
        batches: &mut Vec<crate::render::particles::ParticleBatch>,
    ) {
        if !enabled() {
            return;
        }
        for effect in self
            .spell_fx
            .streams
            .values_mut()
            .filter_map(|s| s.effect.as_mut())
        {
            effect.step(dt, right, up, batches);
        }
        for wave in &mut self.spell_fx.waves {
            wave.effect.step(dt, right, up, batches);
        }
        for (&(target, _, _), fx) in &mut self.spell_fx.shaders {
            if fx.particle_material.is_none() {
                fx.particle_material = Some(std::sync::Arc::new(
                    self.renderer
                        .create_material(&particles::material(&fx.shader)),
                ));
            }
            let actor = if target == PLAYER_REF {
                self.scene.player.as_ref()
            } else {
                self.actor_cells.get(&target).and_then(|key| {
                    self.cells
                        .get(key)
                        .and_then(|rt| rt.actors.iter().position(|a| a.ref_id == target))
                        .and_then(|i| self.scene.cells.get(key)?.actors.get(i))
                })
            };
            let surface = actor.map(|actor| {
                crate::render::membrane::shader_target(actor, fx.shader.flags(), fx.weapon.as_ref())
            });
            let mut vertices = Vec::new();
            fx.particles.step(
                &fx.shader,
                fx.age,
                fx.stopped,
                surface.as_ref(),
                dt,
                right,
                up,
                &mut vertices,
            );
            if !vertices.is_empty() {
                batches.push(crate::render::particles::ParticleBatch {
                    material: fx.particle_material.as_ref().unwrap().clone(),
                    vertices,
                    center: actor.map_or(Vec3::ZERO, |a| a.transform.w_axis.truncate()),
                });
            }
        }
        for p in &mut self.projectiles {
            if let Some(fx) = &mut p.fx {
                fx.step(dt, right, up, batches);
            }
        }
        for art in self.spell_fx.art.values_mut() {
            art.effect.step(dt, right, up, batches);
        }
        for effect in &mut self.spell_fx.tails {
            effect.step(dt, right, up, batches);
        }
    }
}

impl Engine {
    fn update_shader_fx(&mut self, dt: f32, active: &std::collections::HashSet<u64>) {
        let targets: std::collections::HashSet<_> = self
            .spell_fx
            .shaders
            .keys()
            .filter_map(|&(r, _, _)| self.spell_actor_frame(r).map(|_| r))
            .collect();
        self.spell_fx.shaders.retain(|&(r, _, id), fx| {
            targets.contains(&r) && fx.advance(dt, id == 0 || active.contains(&id))
        });
        let mut keys: Vec<_> = self.spell_fx.shaders.keys().copied().collect();
        keys.sort_by_key(|k| (k.0.0, k.1.0, k.2));
        for key in keys {
            let target = key.0;
            let fx = self.spell_fx.shaders.get_mut(&key).unwrap();
            if fx.shader.flags() & 1 != 0 {
                continue;
            }
            let desc = crate::render::membrane::material(&fx.shader, fx.age, fx.stopped);
            let (rigid, skin) = fx.membrane_materials.get_or_insert_with(|| {
                let rigid = std::sync::Arc::new(self.renderer.create_material(&desc));
                let mut skin = self.renderer.create_material(&desc);
                skin.key.skinned = true;
                (rigid, std::sync::Arc::new(skin))
            });
            self.renderer.update_membrane_material(rigid, &desc);
            self.renderer.update_membrane_material(skin, &desc);
            let actor = if target == PLAYER_REF {
                self.scene.player.as_ref()
            } else {
                self.actor_cells.get(&target).and_then(|key| {
                    self.cells
                        .get(key)
                        .and_then(|rt| rt.actors.iter().position(|a| a.ref_id == target))
                        .and_then(|i| self.scene.cells.get(key)?.actors.get(i))
                })
            };
            if let Some(actor) = actor {
                self.scene
                    .membranes
                    .push(crate::render::membrane::overlay_actor(
                        actor,
                        rigid,
                        skin,
                        fx.shader.flags(),
                        fx.weapon.as_ref(),
                    ));
            } else {
                for cell in self.scene.cells.values() {
                    for inst in &cell.instances {
                        if inst.ref_id == target.0 && !inst.hidden {
                            let model = crate::render::membrane::overlay_model(&inst.model, rigid);
                            self.scene
                                .dynamic
                                .push(crate::render::Instance::new(model, inst.transform));
                        }
                    }
                }
            }
        }
    }
    pub(crate) fn play_shader_fx(&mut self, target: FormId, form: FormId, duration: f32) {
        self.play_shader_source(target, form, form, duration);
    }
    fn play_shader_source(&mut self, target: FormId, form: FormId, source: FormId, duration: f32) {
        if !enabled() {
            return;
        }
        if let Some(fx) = self.spell_fx.shaders.get_mut(&(target, source, 0)) {
            if records::EffectShader::load(&self.lo, form).is_some_and(|shader| {
                shader.data == fx.shader.data && shader.textures == fx.shader.textures
            }) {
                fx.age = 0.0;
                fx.stopped = None;
                fx.duration = if duration < 0.0 {
                    f32::INFINITY
                } else {
                    duration.max(0.0)
                };
                fx.particles = Default::default();
                return;
            }
        }
        let Some(shader) = records::EffectShader::load(&self.lo, form) else {
            return;
        };
        crate::world::loader::load_textures(
            &mut self.renderer,
            &self.vfs,
            shader.textures.iter().flatten().cloned().collect(),
        );
        let duration = if duration < 0.0 {
            f32::INFINITY
        } else {
            duration.max(0.0)
        };
        self.spell_fx.shaders.insert(
            (target, source, 0),
            shaders::ShaderEffect::new(shader, duration, false),
        );
    }
    pub(crate) fn stop_shader_fx(&mut self, target: FormId, form: FormId) {
        if let Some(fx) = self.spell_fx.shaders.get_mut(&(target, form, 0)) {
            fx.stop();
        }
    }
}

impl Engine {
    pub(crate) fn play_visual_fx(
        &mut self,
        target: FormId,
        form: FormId,
        duration: f32,
        facing: Option<FormId>,
    ) {
        if !enabled() {
            return;
        }
        let Some(visual) = records::VisualEffect::load(&self.lo, form) else {
            return;
        };
        if !visual.shader.is_null() {
            self.play_shader_source(target, visual.shader, form, duration);
        }
        if !visual.art.is_null() {
            if let Some(mut art) = self.load_art(visual.art, "MagicEffectsNode") {
                art.persistent = true;
                art.flags = visual.flags;
                art.facing = facing;
                art.effect.duration = if duration < 0.0 {
                    f32::INFINITY
                } else {
                    duration.max(0.0)
                };
                self.spell_fx
                    .art
                    .insert((target, ArtKey::Script(form)), art);
            }
        }
    }
    pub(crate) fn stop_visual_fx(&mut self, target: FormId, form: FormId) {
        self.stop_shader_fx(target, form);
        if let Some(art) = self.spell_fx.art.get_mut(&(target, ArtKey::Script(form))) {
            art.effect.stop();
            art.stopping_age = Some(art.effect.age);
        }
    }
}

fn stream_start(
    frame: Option<&ActorFrame>,
    source: crate::casting::CastSource,
    from: Vec3,
) -> Vec3 {
    use crate::casting::CastSource;
    let node = match source {
        CastSource::Left => "NPC L MagicNode [LMag]",
        CastSource::Right => "NPC R MagicNode [RMag]",
        CastSource::Dual | CastSource::Other => return from,
    };
    frame
        .and_then(|f| f.node(node))
        .map_or(from, |m| m.w_axis.truncate())
}

/// The model of a projectile pointing along `dir` from `at`.
fn stream_transform(at: Vec3, dir: Vec3) -> Mat4 {
    Mat4::from_rotation_translation(
        glam::Quat::from_rotation_arc(Vec3::Y, dir.normalize_or(Vec3::Y)),
        at,
    )
}

/// Seconds a held stream outlasts the cast that last fed it (concentration
/// casts feed it every `CONCENTRATION_TICK`).
const STREAM_HOLD: f32 = 0.3;
const PROJ_BEAM: u16 = 0x4;

/// A flame or beam projectile's model, held at its source while casts feed
/// it: one per caster, projectile and hand. Its particles carry the reach.
struct HeldStream {
    effect: Option<EffectInstance>,
    /// The cast's origin, for sources without a magic node.
    from: Vec3,
    dir: Vec3,
    /// How far the cast reached: to what it struck, else its range.
    reach: f32,
    /// That reach as a fraction of the range (`fFlameProjectileLength`).
    length: f32,
    beam: bool,
    remaining: f32,
}

/// A cone projectile's model, sent out once as far as its range.
struct ConeWave {
    effect: EffectInstance,
    velocity: Vec3,
    /// Distance still to go.
    left: f32,
}

impl Engine {
    /// A projectile's model (`PROJ` `MODL`) placed at `transform`, with its
    /// light (`DATA` +16), fading for its fade duration (+48) once stopped,
    /// or without one for its `mEnd` sequence.
    pub(crate) fn load_projectile_effect(
        &mut self,
        proj: FormId,
        transform: Mat4,
    ) -> Option<EffectInstance> {
        let (model, data) = {
            let rec = self.lo.get(proj)?;
            let data = rec.get(b"DATA").map(|d| {
                (
                    records::optional_form(&rec, d, 16),
                    records::optional_float(d, 48),
                )
            });
            (crate::world::records::model_path(&rec)?, data)
        };
        let mut effect = self.load_spell_model(&model, transform)?;
        if let Some((light, fade)) = data {
            effect.light = light;
            if fade > 0.0 {
                effect.fade_duration = fade;
            } else if let Some(anim) = &effect.anim {
                effect.fade_duration = anim
                    .sequences
                    .iter()
                    .filter(|s| s.name.eq_ignore_ascii_case("mEnd"))
                    .map(|s| (s.stop - s.start) / s.frequency.abs().max(1e-6))
                    .fold(0.0, f32::max)
                    .max(0.05);
            }
        }
        Some(effect)
    }

    /// Feed a flame or beam projectile's held model: cast from `from` along
    /// `dir`, reaching `reach` of its `range`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn request_spell_stream(
        &mut self,
        caster: FormId,
        proj: FormId,
        from: Vec3,
        dir: Vec3,
        reach: f32,
        range: f32,
        input: crate::casting::CastSource,
    ) {
        if !enabled() {
            return;
        }
        let length = if range > 0.0 {
            (reach / range).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let key = (caster, proj, input);
        if let Some(stream) = self.spell_fx.streams.get_mut(&key) {
            stream.from = from;
            stream.dir = dir;
            stream.reach = reach;
            stream.length = length;
            stream.remaining = STREAM_HOLD;
            return;
        }
        let beam = self
            .lo
            .get(proj)
            .and_then(|r| r.get(b"DATA").map(|d| records::optional_u32(d, 0) >> 16))
            .is_some_and(|kind| kind as u16 & PROJ_BEAM != 0);
        let at = stream_start(self.spell_actor_frame(caster).as_ref(), input, from);
        let mut effect = self.load_projectile_effect(proj, stream_transform(at, dir));
        if let Some(effect) = &mut effect {
            let left = input == crate::casting::CastSource::Left;
            effect.set_variable("bLeftHand", left as u8 as f32);
            effect.set_variable("bRightHand", !left as u8 as f32);
        }
        self.spell_fx.streams.insert(
            key,
            HeldStream {
                effect,
                from,
                dir,
                reach,
                length,
                beam,
                remaining: STREAM_HOLD,
            },
        );
    }

    /// Send a cone projectile's model out from `from` along `dir`, at `speed`
    /// as far as `range`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn launch_spell_cone(
        &mut self,
        caster: FormId,
        proj: FormId,
        from: Vec3,
        dir: Vec3,
        speed: f32,
        range: f32,
        input: crate::casting::CastSource,
    ) {
        if !enabled() {
            return;
        }
        let at = stream_start(self.spell_actor_frame(caster).as_ref(), input, from);
        if let Some(effect) = self.load_projectile_effect(proj, stream_transform(at, dir)) {
            self.spell_fx.waves.push(ConeWave {
                effect,
                velocity: dir.normalize_or(Vec3::Y) * speed.max(0.0),
                left: range.max(0.0),
            });
        }
    }

    fn update_stream_fx(&mut self, dt: f32) {
        // The player's streams follow the aim between casts.
        let aim = self.scene.player.is_some().then(|| self.player_aim());
        let frames: Vec<_> = self
            .spell_fx
            .streams
            .keys()
            .map(|&key| (key, self.spell_actor_frame(key.0)))
            .collect();
        let mut lights = Vec::new();
        for ((caster, proj, source), frame) in frames {
            let stream = self
                .spell_fx
                .streams
                .get_mut(&(caster, proj, source))
                .unwrap();
            stream.remaining -= dt;
            if let Some((eye, dir)) = aim.filter(|_| caster == PLAYER_REF && !stream.beam) {
                stream.from = eye;
                stream.dir = dir;
            }
            let end = stream.from + stream.dir * stream.reach;
            let at = stream_start(frame.as_ref(), source, stream.from);
            let Some(effect) = &mut stream.effect else {
                continue;
            };
            effect.transform = stream_transform(at, (end - at).normalize_or(stream.dir));
            if stream.beam
                && let Some(anim) = &effect.anim
            {
                for part in &anim.parts {
                    if part.node.eq_ignore_ascii_case("BeamEnd") {
                        effect.anchors.insert(
                            part.block,
                            Mat4::from_translation(end) * part.rest.inverse(),
                        );
                    }
                }
            }
            effect.set_variable("fFlameProjectileLength", stream.length);
            if stream.remaining > 0.0 {
                effect.advance(dt);
                effect.draw(&mut self.scene);
                lights.push((
                    effect.light,
                    effect.transform.transform_point3(effect.light_attach),
                ));
            }
        }
        let tails = &mut self.spell_fx.tails;
        self.spell_fx.streams.retain(|_, stream| {
            if stream.remaining > 0.0 {
                return true;
            }
            if let Some(mut effect) = stream.effect.take() {
                effect.stop();
                tails.push(effect);
            }
            false
        });
        for wave in &mut self.spell_fx.waves {
            let step = (wave.velocity.length() * dt).min(wave.left);
            wave.left -= step;
            wave.effect.transform.w_axis += (wave.velocity.normalize_or_zero() * step).extend(0.0);
            wave.effect.advance(dt);
            wave.effect.draw(&mut self.scene);
            let fx = &wave.effect;
            lights.push((fx.light, fx.transform.transform_point3(fx.light_attach)));
        }
        let tails = &mut self.spell_fx.tails;
        self.spell_fx.waves.retain_mut(|wave| {
            if wave.left > 0.0 {
                return true;
            }
            let mut effect = std::mem::replace(
                &mut wave.effect,
                EffectInstance::from_parts(Mat4::IDENTITY, None, None, Vec::new(), 0.0),
            );
            effect.stop();
            tails.push(effect);
            false
        });
        let lights: Vec<_> = lights
            .into_iter()
            .filter_map(|(light, at)| self.spell_light(light, at))
            .collect();
        self.spell_fx.lights.extend(lights);
    }
}

#[cfg(test)]
mod tests;
