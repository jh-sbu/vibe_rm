//! A scene drawn over the frame (`Renderer::render_over`): the frame kept
//! where the scene has nothing, and the scene's own depth.

use super::effect_shader_tests::{renderer, solid};
use super::*;
use glam::{Mat4, Vec2, Vec3, Vec4};
use model::{BlendMode, CpuMesh, CpuModel, MaterialDesc, ShaderKind, Vertex};

/// An opaque effect quad facing the camera, `half` units each way, `y`
/// units ahead, its diffuse `tex`.
fn quad(r: &Renderer, tex: &str, half: f32, y: f32) -> Instance {
    let corner = |x: f32, z: f32| Vertex {
        position: [x, 0.0, z],
        normal: [0.0, -1.0, 0.0],
        tangent: [1.0, 0.0, 0.0],
        bitangent: [0.0, 0.0, 1.0],
        uv: [0.5, 0.5],
        color: [1.0; 4],
    };
    let mesh = CpuMesh {
        vertices: vec![
            corner(-half, -half),
            corner(half, -half),
            corner(half, half),
            corner(-half, half),
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
        material: MaterialDesc {
            kind: ShaderKind::Effect,
            diffuse: Some(tex.into()),
            blend: BlendMode::Opaque,
            emissive: Vec4::ONE,
            uv_scale: Vec2::ONE,
            double_sided: true,
            ..Default::default()
        },
        bound_center: Vec3::ZERO,
        bound_radius: half * 1.5,
    };
    let cpu = CpuModel {
        meshes: vec![mesh],
        skinned: Vec::new(),
        bound_center: Vec3::ZERO,
        bound_radius: half * 1.5,
        animated: Vec::new(),
        sequences: Vec::new(),
        particles: Vec::new(),
        addons: Vec::new(),
    };
    Instance::new(
        Arc::new(r.upload_model(&cpu)),
        Mat4::from_translation(Vec3::Y * y),
    )
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic geometry only"]
fn a_scene_over_the_frame_keeps_the_frame_and_has_its_own_depth() {
    let mut r = renderer();
    r.add_texture("green.dds", Some(&solid([0, 255, 0, 255])));
    r.add_texture("red.dds", Some(&solid([255, 0, 0, 255])));
    let camera = Camera {
        position: Vec3::ZERO,
        yaw: 0.0,
        pitch: 0.0,
        fov_y: 1.0,
        projection: Projection::Perspective,
    };
    // The world: a green wall close by, filling the view.
    let mut world = Scene::default();
    world.dynamic.push(quad(&r, "green.dds", 100.0, 10.0));
    // Over it: a small red quad farther off than the wall.
    let mut over = Scene::default();
    over.env.sky = false;
    over.dynamic.push(quad(&r, "red.dds", 5.0, 50.0));
    let size = (r.width, r.height);
    let pixels = r.render_to_image(&world, &camera, size, |r, view| {
        r.render_over(&over, &camera, view)
    });
    let at = |x: u32, y: u32| {
        let i = ((y * size.0 + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2]]
    };
    assert_eq!(
        at(size.0 / 2, size.1 / 2),
        [255, 0, 0],
        "the item, behind the wall"
    );
    assert_eq!(at(1, 1), [0, 255, 0], "the frame kept");
}
