//! Particle systems (SSE layouts, after niftools' nif.xml): the system and its
//! data, the modifiers that emit, age, move, scale, colour and animate the
//! particles, and the controllers that drive them over time (birth rate,
//! modifiers switched on and off). Bombs, strip data and mesh particles
//! are left unparsed.

use glam::{Vec3, Vec4};

use crate::Result;
use crate::anim::Timing;
use crate::blocks::{AvObject, Ref};
use crate::reader::Reader;

/// `NiParticleSystem` / `BSStripParticleSystem`.
#[derive(Debug, Clone)]
pub struct ParticleSystem {
    pub av: AvObject,
    pub bound_center: Vec3,
    pub bound_radius: f32,
    pub shader: Ref,
    pub alpha: Ref,
    pub data: Ref,
    /// Particles live in world space (else in the system's space).
    pub world_space: bool,
    pub modifiers: Vec<Ref>,
    /// A strip system (trails), drawn differently.
    pub strip: bool,
}

/// `NiPSysData`.
#[derive(Debug, Clone)]
pub struct ParticleData {
    pub max_particles: u16,
    /// Texture atlas cells (u offset, v offset, u size, v size).
    pub subtexture_offsets: Vec<Vec4>,
    pub aspect_ratio: f32,
    /// Aspect flags: bit 0 velocity orientation, bit 1 initial rotation from
    /// velocity, bit 8 speed to aspect enabled (niftools' AspectFlags).
    pub aspect_flags: u16,
    /// Second aspect ratio, first speed, second speed (niftools' field order).
    pub speed_to_aspect: (f32, f32, f32),
}

/// A particle modifier: its name (what controllers address), order and kind.
#[derive(Debug, Clone)]
pub struct Modifier {
    pub name: String,
    pub order: u32,
    pub active: bool,
    pub kind: ModifierKind,
}

#[derive(Debug, Clone)]
pub enum ModifierKind {
    AgeDeath {
        spawn_on_death: bool,
        spawn: Ref,
    },
    Spawn {
        generations: u16,
        percentage: f32,
        min: u16,
        max: u16,
        speed_variation: f32,
        direction_variation: f32,
        life_span: f32,
        life_span_variation: f32,
    },
    BoundUpdate,
    Position,
    ColliderManager {
        collider: Ref,
    },
    Gravity {
        object: Ref,
        axis: Vec3,
        decay: f32,
        strength: f32,
        /// 0 planar, 1 spherical.
        force: u32,
        turbulence: f32,
        turbulence_scale: f32,
        world_aligned: bool,
    },
    Drag {
        object: Ref,
        axis: Vec3,
        percentage: f32,
        range: f32,
        falloff: f32,
    },
    Rotation {
        speed: f32,
        speed_variation: f32,
        angle: f32,
        angle_variation: f32,
        random_sign: bool,
        random_axis: bool,
        axis: Vec3,
    },
    /// `BSPSysScaleModifier`: size over the particle's life.
    Scale(Vec<f32>),
    /// `BSPSysSimpleColorModifier`: alpha fading in and out, three colours.
    SimpleColor {
        fade_in: f32,
        fade_out: f32,
        color1_end: f32,
        color1_start: f32,
        color2_end: f32,
        color2_start: f32,
        colors: [Vec4; 3],
    },
    /// `BSPSysSubTexModifier`: flipping through the texture atlas.
    SubTex {
        start: f32,
        start_fudge: f32,
        end: f32,
        loop_start: f32,
        loop_start_fudge: f32,
        frame_count: f32,
        frame_count_fudge: f32,
    },
    Lod {
        begin: f32,
        end: f32,
        end_emit_scale: f32,
        end_size: f32,
    },
    InheritVelocity {
        object: Ref,
        chance: f32,
        multiplier: f32,
        variation: f32,
    },
    RecycleBound {
        offset: Vec3,
        extent: Vec3,
        object: Ref,
    },
    Emitter(Emitter),
}

/// A particle-only collider; its object supplies the position and orientation.
#[derive(Debug, Clone)]
pub struct Collider {
    pub bounce: f32,
    pub spawn_on_collide: bool,
    pub die_on_collide: bool,
    pub spawn: Ref,
    pub parent: Ref,
    pub next: Ref,
    pub object: Ref,
    pub shape: ColliderShape,
}

#[derive(Debug, Clone)]
pub enum ColliderShape {
    Plane {
        width: f32,
        height: f32,
        x_axis: Vec3,
        y_axis: Vec3,
    },
    Sphere {
        radius: f32,
    },
}

#[derive(Debug, Clone)]
pub struct Emitter {
    pub speed: f32,
    pub speed_variation: f32,
    pub declination: f32,
    pub declination_variation: f32,
    pub planar_angle: f32,
    pub planar_angle_variation: f32,
    pub color: Vec4,
    pub radius: f32,
    pub radius_variation: f32,
    pub life_span: f32,
    pub life_span_variation: f32,
    pub shape: EmitterShape,
}

#[derive(Debug, Clone)]
pub enum EmitterShape {
    Box {
        object: Ref,
        width: f32,
        height: f32,
        depth: f32,
    },
    Cylinder {
        object: Ref,
        radius: f32,
        height: f32,
    },
    Sphere {
        object: Ref,
        radius: f32,
    },
    Mesh {
        meshes: Vec<Ref>,
        /// 0 normals, 1 random, 2 direction (the emission axis).
        velocity_type: u32,
        /// 0 vertices, 1 face centres, 2 edge centres, 3 faces, 4 edges.
        emit_from: u32,
        axis: Vec3,
    },
}

/// A particle system controller.
#[derive(Debug, Clone)]
pub struct ParticleController {
    pub next: Ref,
    pub timing: Timing,
    pub interpolator: Ref,
    /// The modifier it drives (by name).
    pub modifier: String,
    pub kind: ControllerKind,
}

#[derive(Debug, Clone)]
pub enum ControllerKind {
    /// `NiPSysEmitterCtlr` (and `BSPSysMultiTargetEmitterCtlr`): the
    /// interpolator is the birth rate, `visibility` switches emitting on.
    /// `multi_target`: a master particle system's, emitting at the addon
    /// nodes using it.
    Emitter { visibility: Ref, multi_target: bool },
    /// `NiPSysUpdateCtlr`: runs the system (no interpolator or modifier).
    Update,
    /// `NiPSysModifierActiveCtlr`: switches a modifier on and off.
    ModifierActive,
    /// A float on a modifier: which one (`NiPSysGravityStrengthCtlr`...).
    Float(FloatTarget),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatTarget {
    GravityStrength,
    EmitterSpeed,
    EmitterInitialRadius,
    EmitterLifeSpan,
    EmitterDeclination,
    EmitterPlanarAngle,
    InitialRotationSpeed,
    InitialRotationSpeedVariation,
    InitialRotationAngle,
    InitialRotationAngleVariation,
}

/// `NiBoolInterpolator`'s value and keys (`NiBoolData`).
#[derive(Debug, Clone)]
pub struct BoolInterpolator {
    pub value: bool,
    pub data: Ref,
}

fn modifier_base(r: &mut Reader) -> Result<(String, u32, bool)> {
    let name = r.string_value()?;
    let order = r.u32()?;
    r.i32()?; // target
    let active = r.bool()?;
    Ok((name, order, active))
}

fn emitter(r: &mut Reader, ty: &str) -> Result<Emitter> {
    let speed = r.f32()?;
    let speed_variation = r.f32()?;
    let declination = r.f32()?;
    let declination_variation = r.f32()?;
    let planar_angle = r.f32()?;
    let planar_angle_variation = r.f32()?;
    let color = r.vec4()?;
    let radius = r.f32()?;
    let radius_variation = r.f32()?;
    let life_span = r.f32()?;
    let life_span_variation = r.f32()?;
    let shape = if ty == "NiPSysMeshEmitter" {
        let n = r.u32()?;
        let mut meshes = Vec::with_capacity(n.min(256) as usize);
        for _ in 0..n {
            meshes.push(r.block_ref()?);
        }
        EmitterShape::Mesh {
            meshes,
            velocity_type: r.u32()?,
            emit_from: r.u32()?,
            axis: r.vec3()?,
        }
    } else {
        let object = r.block_ref()?;
        match ty {
            "NiPSysBoxEmitter" => EmitterShape::Box {
                object,
                width: r.f32()?,
                height: r.f32()?,
                depth: r.f32()?,
            },
            "NiPSysCylinderEmitter" => EmitterShape::Cylinder {
                object,
                radius: r.f32()?,
                height: r.f32()?,
            },
            _ => EmitterShape::Sphere {
                object,
                radius: r.f32()?,
            },
        }
    };
    Ok(Emitter {
        speed,
        speed_variation,
        declination,
        declination_variation,
        planar_angle,
        planar_angle_variation,
        color,
        radius,
        radius_variation,
        life_span,
        life_span_variation,
        shape,
    })
}

/// A particle block of type `ty`, `None` when it isn't one handled here (or
/// the file predates SSE).
pub(crate) fn parse(ty: &str, r: &mut Reader) -> Result<Option<crate::Block>> {
    use crate::Block;
    if r.bs_version < 100 {
        return Ok(None);
    }
    Ok(Some(match ty {
        "NiParticleSystem" | "BSStripParticleSystem" => {
            let av = crate::blocks::av_object(r)?;
            let bound_center = r.vec3()?;
            let bound_radius = r.f32()?;
            r.block_ref()?; // skin
            let shader = r.block_ref()?;
            let alpha = r.block_ref()?;
            r.u64()?; // vertex desc
            for _ in 0..4 {
                r.u16()?; // far / near begin / end
            }
            let data = r.block_ref()?;
            let world_space = r.bool()?;
            let n = r.u32()?;
            let mut modifiers = Vec::with_capacity(n.min(64) as usize);
            for _ in 0..n {
                modifiers.push(r.block_ref()?);
            }
            Block::ParticleSystem(Box::new(ParticleSystem {
                av,
                bound_center,
                bound_radius,
                shader,
                alpha,
                data,
                world_space,
                modifiers,
                strip: ty == "BSStripParticleSystem",
            }))
        }
        "NiPSysData" => {
            r.i32()?; // group id
            let max_particles = r.u16()?;
            r.u8()?; // keep flags
            r.u8()?; // compress flags
            // The vertex, normal, colour and UV arrays are sized by a vertex
            // count SSE doesn't store: always empty.
            r.bool()?; // has vertices
            r.u16()?; // data flags
            r.u32()?; // material CRC
            r.bool()?; // has normals
            r.skip(16)?; // bounding sphere
            r.bool()?; // has vertex colours
            r.u16()?; // consistency
            r.block_ref()?; // additional data
            r.bool()?; // has radii
            r.u16()?; // num active
            r.bool()?; // has sizes
            r.bool()?; // has rotations
            r.bool()?; // has rotation angles
            r.bool()?; // has rotation axes
            r.bool()?; // has texture indices
            let k = r.u32()?;
            let mut subtexture_offsets = Vec::with_capacity(k.min(1024) as usize);
            for _ in 0..k {
                subtexture_offsets.push(r.vec4()?);
            }
            let aspect_ratio = r.f32()?;
            let aspect_flags = r.u16()?;
            let speed_to_aspect = (r.f32()?, r.f32()?, r.f32()?);
            r.bool()?; // has rotation speeds
            Block::ParticleData(Box::new(ParticleData {
                max_particles,
                subtexture_offsets,
                aspect_ratio,
                aspect_flags,
                speed_to_aspect,
            }))
        }
        "NiPSysPlanarCollider" | "NiPSysSphericalCollider" => {
            let bounce = r.f32()?;
            let spawn_on_collide = r.bool()?;
            let die_on_collide = r.bool()?;
            let spawn = r.block_ref()?;
            let parent = r.block_ref()?;
            let next = r.block_ref()?;
            let object = r.block_ref()?;
            let shape = if ty == "NiPSysPlanarCollider" {
                ColliderShape::Plane {
                    width: r.f32()?,
                    height: r.f32()?,
                    x_axis: r.vec3()?,
                    y_axis: r.vec3()?,
                }
            } else {
                ColliderShape::Sphere { radius: r.f32()? }
            };
            Block::ParticleCollider(Box::new(Collider {
                bounce,
                spawn_on_collide,
                die_on_collide,
                spawn,
                parent,
                next,
                object,
                shape,
            }))
        }
        "NiPSysAgeDeathModifier"
        | "NiPSysColliderManager"
        | "NiPSysSpawnModifier"
        | "NiPSysBoundUpdateModifier"
        | "NiPSysPositionModifier"
        | "NiPSysGravityModifier"
        | "NiPSysDragModifier"
        | "NiPSysRotationModifier"
        | "BSPSysScaleModifier"
        | "BSPSysSimpleColorModifier"
        | "BSPSysSubTexModifier"
        | "BSPSysLODModifier"
        | "BSPSysInheritVelocityModifier"
        | "BSPSysRecycleBoundModifier"
        | "NiPSysBoxEmitter"
        | "NiPSysCylinderEmitter"
        | "NiPSysSphereEmitter"
        | "NiPSysMeshEmitter" => {
            let (name, order, active) = modifier_base(r)?;
            let kind = match ty {
                "NiPSysAgeDeathModifier" => ModifierKind::AgeDeath {
                    spawn_on_death: r.bool()?,
                    spawn: r.block_ref()?,
                },
                "NiPSysSpawnModifier" => ModifierKind::Spawn {
                    generations: r.u16()?,
                    percentage: r.f32()?,
                    min: r.u16()?,
                    max: r.u16()?,
                    speed_variation: r.f32()?,
                    direction_variation: r.f32()?,
                    life_span: r.f32()?,
                    life_span_variation: r.f32()?,
                },
                "NiPSysBoundUpdateModifier" => {
                    r.u16()?; // update skip
                    ModifierKind::BoundUpdate
                }
                "NiPSysPositionModifier" => ModifierKind::Position,
                "NiPSysColliderManager" => ModifierKind::ColliderManager {
                    collider: r.block_ref()?,
                },
                "NiPSysGravityModifier" => ModifierKind::Gravity {
                    object: r.block_ref()?,
                    axis: r.vec3()?,
                    decay: r.f32()?,
                    strength: r.f32()?,
                    force: r.u32()?,
                    turbulence: r.f32()?,
                    turbulence_scale: r.f32()?,
                    world_aligned: r.bool()?,
                },
                "NiPSysDragModifier" => ModifierKind::Drag {
                    object: r.block_ref()?,
                    axis: r.vec3()?,
                    percentage: r.f32()?,
                    range: r.f32()?,
                    falloff: r.f32()?,
                },
                "NiPSysRotationModifier" => ModifierKind::Rotation {
                    speed: r.f32()?,
                    speed_variation: r.f32()?,
                    angle: r.f32()?,
                    angle_variation: r.f32()?,
                    random_sign: r.bool()?,
                    random_axis: r.bool()?,
                    axis: r.vec3()?,
                },
                "BSPSysScaleModifier" => {
                    let n = r.u32()?;
                    let mut v = Vec::with_capacity(n.min(256) as usize);
                    for _ in 0..n {
                        v.push(r.f32()?);
                    }
                    ModifierKind::Scale(v)
                }
                "BSPSysSimpleColorModifier" => ModifierKind::SimpleColor {
                    fade_in: r.f32()?,
                    fade_out: r.f32()?,
                    color1_end: r.f32()?,
                    color1_start: r.f32()?,
                    color2_end: r.f32()?,
                    color2_start: r.f32()?,
                    colors: [r.vec4()?, r.vec4()?, r.vec4()?],
                },
                "BSPSysSubTexModifier" => ModifierKind::SubTex {
                    start: r.f32()?,
                    start_fudge: r.f32()?,
                    end: r.f32()?,
                    loop_start: r.f32()?,
                    loop_start_fudge: r.f32()?,
                    frame_count: r.f32()?,
                    frame_count_fudge: r.f32()?,
                },
                "BSPSysLODModifier" => ModifierKind::Lod {
                    begin: r.f32()?,
                    end: r.f32()?,
                    end_emit_scale: r.f32()?,
                    end_size: r.f32()?,
                },
                "BSPSysInheritVelocityModifier" => ModifierKind::InheritVelocity {
                    object: r.block_ref()?,
                    chance: r.f32()?,
                    multiplier: r.f32()?,
                    variation: r.f32()?,
                },
                "BSPSysRecycleBoundModifier" => ModifierKind::RecycleBound {
                    offset: r.vec3()?,
                    extent: r.vec3()?,
                    object: r.block_ref()?,
                },
                _ => ModifierKind::Emitter(emitter(r, ty)?),
            };
            Block::ParticleModifier(Box::new(Modifier {
                name,
                order,
                active,
                kind,
            }))
        }
        "NiPSysEmitterCtlr"
        | "BSPSysMultiTargetEmitterCtlr"
        | "NiPSysUpdateCtlr"
        | "NiPSysModifierActiveCtlr"
        | "NiPSysGravityStrengthCtlr"
        | "NiPSysEmitterSpeedCtlr"
        | "NiPSysEmitterInitialRadiusCtlr"
        | "NiPSysEmitterLifeSpanCtlr"
        | "NiPSysEmitterDeclinationCtlr"
        | "NiPSysEmitterPlanarAngleCtlr"
        | "NiPSysInitialRotSpeedCtlr"
        | "NiPSysInitialRotSpeedVarCtlr"
        | "NiPSysInitialRotAngleCtlr"
        | "NiPSysInitialRotAngleVarCtlr" => {
            let (next, timing) = crate::anim::timing(r)?;
            if ty == "NiPSysUpdateCtlr" {
                Block::ParticleController(ParticleController {
                    next,
                    timing,
                    interpolator: Ref(-1),
                    modifier: String::new(),
                    kind: ControllerKind::Update,
                })
            } else {
                let interpolator = r.block_ref()?;
                let modifier = r.string_value()?;
                let kind = match ty {
                    "NiPSysEmitterCtlr" | "BSPSysMultiTargetEmitterCtlr" => {
                        let visibility = r.block_ref()?;
                        let multi_target = ty == "BSPSysMultiTargetEmitterCtlr";
                        if multi_target {
                            r.u16()?; // max emitters
                            r.i32()?; // master particle system
                        }
                        ControllerKind::Emitter {
                            visibility,
                            multi_target,
                        }
                    }
                    "NiPSysModifierActiveCtlr" => ControllerKind::ModifierActive,
                    "NiPSysGravityStrengthCtlr" => {
                        ControllerKind::Float(FloatTarget::GravityStrength)
                    }
                    "NiPSysEmitterSpeedCtlr" => ControllerKind::Float(FloatTarget::EmitterSpeed),
                    "NiPSysEmitterInitialRadiusCtlr" => {
                        ControllerKind::Float(FloatTarget::EmitterInitialRadius)
                    }
                    "NiPSysEmitterLifeSpanCtlr" => {
                        ControllerKind::Float(FloatTarget::EmitterLifeSpan)
                    }
                    "NiPSysEmitterDeclinationCtlr" => {
                        ControllerKind::Float(FloatTarget::EmitterDeclination)
                    }
                    "NiPSysEmitterPlanarAngleCtlr" => {
                        ControllerKind::Float(FloatTarget::EmitterPlanarAngle)
                    }
                    "NiPSysInitialRotSpeedVarCtlr" => {
                        ControllerKind::Float(FloatTarget::InitialRotationSpeedVariation)
                    }
                    "NiPSysInitialRotAngleCtlr" => {
                        ControllerKind::Float(FloatTarget::InitialRotationAngle)
                    }
                    "NiPSysInitialRotAngleVarCtlr" => {
                        ControllerKind::Float(FloatTarget::InitialRotationAngleVariation)
                    }
                    _ => ControllerKind::Float(FloatTarget::InitialRotationSpeed),
                };
                Block::ParticleController(ParticleController {
                    next,
                    timing,
                    interpolator,
                    modifier,
                    kind,
                })
            }
        }
        "NiBoolInterpolator" => Block::BoolInterpolator(BoolInterpolator {
            value: r.bool()?,
            data: r.block_ref()?,
        }),
        "NiBoolData" => Block::ValueKeys(crate::anim::bool_keys(r)?),
        _ => return Ok(None),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Block;

    #[test]
    fn initial_rotation_controllers_read_exact_sse_layouts() {
        for (ty, target) in [
            (
                "NiPSysInitialRotSpeedCtlr",
                FloatTarget::InitialRotationSpeed,
            ),
            (
                "NiPSysInitialRotSpeedVarCtlr",
                FloatTarget::InitialRotationSpeedVariation,
            ),
            (
                "NiPSysInitialRotAngleCtlr",
                FloatTarget::InitialRotationAngle,
            ),
            (
                "NiPSysInitialRotAngleVarCtlr",
                FloatTarget::InitialRotationAngleVariation,
            ),
        ] {
            let mut bytes = 7i32.to_le_bytes().to_vec(); // next controller
            bytes.extend(10u16.to_le_bytes()); // active, reverse cycle
            for value in [2.0f32, 1.0, 3.0, 5.0] {
                bytes.extend(value.to_le_bytes());
            }
            bytes.extend(8i32.to_le_bytes()); // target system
            bytes.extend(9i32.to_le_bytes()); // interpolator
            bytes.extend(0u32.to_le_bytes()); // modifier name
            let strings = vec!["rotation".to_owned()];
            let mut reader = Reader::new(&bytes);
            reader.bs_version = 100;
            reader.strings = &strings;
            let Some(Block::ParticleController(c)) = parse(ty, &mut reader).unwrap() else {
                panic!("{ty} was not decoded");
            };
            assert_eq!(reader.pos(), bytes.len());
            assert_eq!((c.next, c.interpolator), (Ref(7), Ref(9)));
            assert_eq!(c.modifier, "rotation");
            assert_eq!(c.timing.flags, 10);
            assert_eq!(c.timing.key_time(2.5), 4.0);
            assert!(matches!(c.kind, ControllerKind::Float(t) if t == target));
            for length in 0..bytes.len() {
                let mut reader = Reader::new(&bytes[..length]);
                reader.bs_version = 100;
                reader.strings = &strings;
                assert!(parse(ty, &mut reader).is_err(), "{ty}: {length} bytes");
            }
        }
    }

    #[test]
    fn collider_shapes_read_common_links_and_exact_sse_layouts() {
        for (ty, shape_words) in [
            (
                "NiPSysPlanarCollider",
                vec![12.0f32, 8.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            ),
            ("NiPSysSphericalCollider", vec![6.0f32]),
        ] {
            let mut bytes = 0.25f32.to_le_bytes().to_vec();
            bytes.extend([1, 1]);
            for reference in [7i32, 8, 9, 10] {
                bytes.extend(reference.to_le_bytes());
            }
            for value in shape_words {
                bytes.extend(value.to_le_bytes());
            }
            let mut r = Reader::new(&bytes);
            r.bs_version = 100;
            let Some(Block::ParticleCollider(c)) = parse(ty, &mut r).unwrap() else {
                panic!()
            };
            assert_eq!(r.pos(), bytes.len());
            assert_eq!(c.bounce, 0.25);
            assert!(c.spawn_on_collide && c.die_on_collide);
            assert_eq!(
                (c.spawn, c.parent, c.next, c.object),
                (Ref(7), Ref(8), Ref(9), Ref(10))
            );
            match c.shape {
                ColliderShape::Plane {
                    width,
                    height,
                    x_axis,
                    y_axis,
                } => {
                    assert_eq!((width, height), (12.0, 8.0));
                    assert_eq!((x_axis, y_axis), (Vec3::X, Vec3::Y));
                }
                ColliderShape::Sphere { radius } => assert_eq!(radius, 6.0),
            }
            for length in 0..bytes.len() {
                let mut truncated = Reader::new(&bytes[..length]);
                truncated.bs_version = 100;
                assert!(
                    parse(ty, &mut truncated).is_err(),
                    "{ty} accepted {length} bytes"
                );
            }
        }
    }

    #[test]
    fn collider_manager_uses_modifier_header_then_collider_reference() {
        let mut bytes = 0u32.to_le_bytes().to_vec(); // name string reference
        bytes.extend(5000u32.to_le_bytes());
        bytes.extend(2i32.to_le_bytes()); // target
        bytes.push(0); // inactive
        bytes.extend(19i32.to_le_bytes());
        let strings = vec!["collisions".to_owned()];
        let mut r = Reader::new(&bytes);
        r.bs_version = 100;
        r.strings = &strings;
        let Some(Block::ParticleModifier(m)) = parse("NiPSysColliderManager", &mut r).unwrap()
        else {
            panic!()
        };
        assert_eq!(r.pos(), bytes.len());
        assert_eq!(m.name, "collisions");
        assert_eq!(m.order, 5000);
        assert!(!m.active);
        assert!(matches!(
            m.kind,
            ModifierKind::ColliderManager { collider: Ref(19) }
        ));
        r.bs_version = 83;
        r.set_pos(0);
        assert!(parse("NiPSysColliderManager", &mut r).unwrap().is_none());
        assert_eq!(r.pos(), 0);
    }
}
