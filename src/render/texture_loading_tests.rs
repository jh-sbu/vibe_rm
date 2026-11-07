//! Materials made while their textures load in the background: hidden, then
//! faded in once they're in (`plans/async-textures.md`).

use super::effect_shader_tests::{renderer, solid};
use super::*;
use glam::{Mat4, Vec2, Vec3, Vec4};
use model::{BlendMode, CpuMesh, CpuModel, MaterialDesc, ShaderKind, Vertex};

/// An opaque effect quad filling the view, its diffuse `tex`.
fn quad_scene(r: &Renderer, tex: &str) -> Scene {
    let corner = |x: f32, z: f32| Vertex {
        position: [x, 0.0, z],
        normal: [0.0, -1.0, 0.0],
        tangent: [1.0, 0.0, 0.0],
        bitangent: [0.0, 0.0, 1.0],
        uv: [0.5, 0.5],
        color: [1.0; 4],
    };
    let material = MaterialDesc {
        kind: ShaderKind::Effect,
        diffuse: Some(tex.into()),
        blend: BlendMode::Opaque,
        emissive: Vec4::ONE,
        uv_scale: Vec2::ONE,
        double_sided: true,
        ..Default::default()
    };
    let mesh = CpuMesh {
        vertices: vec![
            corner(-100.0, -100.0),
            corner(100.0, -100.0),
            corner(100.0, 100.0),
            corner(-100.0, 100.0),
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
        material,
        bound_center: Vec3::ZERO,
        bound_radius: 150.0,
    };
    let cpu = CpuModel {
        meshes: vec![mesh],
        skinned: Vec::new(),
        bound_center: Vec3::ZERO,
        bound_radius: 150.0,
        animated: Vec::new(),
        sequences: Vec::new(),
        particles: Vec::new(),
        addons: Vec::new(),
    };
    let mut scene = Scene::default();
    scene.dynamic.push(Instance::new(
        Arc::new(r.upload_model(&cpu)),
        Mat4::from_translation(Vec3::Y * 20.0),
    ));
    scene
}

/// How many pixels are the quad's red.
fn red_pixels(r: &mut Renderer, scene: &Scene) -> usize {
    let camera = Camera {
        position: Vec3::ZERO,
        yaw: 0.0,
        pitch: 0.0,
        fov_y: 1.0,
        projection: crate::render::Projection::Perspective,
    };
    r.render_to_image(scene, &camera, (r.width, r.height), |_, _| {})
        .chunks(4)
        .filter(|p| p[0] > 200 && p[1] < 50 && p[2] < 50)
        .count()
}

/// A renderer loading in the background, with `tex` marked pending as if a
/// worker were loading it (none is: the test delivers it).
fn loading(tex: &str) -> Renderer {
    let mut r = renderer();
    let empty = std::env::temp_dir().join(format!("vrm-no-data-{}", std::process::id()));
    r.load_textures_in_background(Arc::new(vfs::Vfs::new(&empty, &[])));
    assert!(r.textures.mark_pending(tex));
    r
}

fn deliver(r: &Renderer, tex: &str) {
    r.textures
        .sender()
        .send((tex.to_owned(), Some(solid([255, 0, 0, 255]))))
        .unwrap();
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic geometry only"]
fn a_material_waiting_for_its_texture_is_hidden_then_fades_in() {
    let mut r = loading("red.dds");
    let scene = quad_scene(&r, "red.dds");
    let all = (r.width * r.height) as usize;
    assert_eq!(red_pixels(&mut r, &scene), 0, "hidden while loading");
    assert_eq!(red_pixels(&mut r, &scene), 0, "still loading");
    deliver(&r, "red.dds");
    // Taken in this frame: made with it, fully hidden as its fade starts.
    assert_eq!(red_pixels(&mut r, &scene), 0, "fade starts");
    r.time += TEXTURE_FADE_SECONDS * 0.5;
    let half = red_pixels(&mut r, &scene);
    assert!(
        half > all / 4 && half < all * 3 / 4,
        "half faded: {half} of {all}"
    );
    r.time += TEXTURE_FADE_SECONDS;
    assert_eq!(red_pixels(&mut r, &scene), all, "faded in");
    assert!(!r.textures.any_pending());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic geometry only"]
fn finishing_textures_shows_waiting_materials_at_once() {
    let mut r = loading("red.dds");
    let scene = quad_scene(&r, "red.dds");
    let all = (r.width * r.height) as usize;
    deliver(&r, "red.dds");
    r.finish_textures();
    assert_eq!(red_pixels(&mut r, &scene), all);
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic geometry only"]
fn a_material_made_without_waiting_lets_a_pending_texture_finish_first() {
    // Not a model's material (an effect's, a decal's): it waits for the
    // texture instead of keeping the fallback.
    let mut r = loading("red.dds");
    deliver(&r, "red.dds");
    let m = r.create_material(&MaterialDesc {
        diffuse: Some("red.dds".into()),
        ..Default::default()
    });
    assert_eq!(m.hidden(r.time), 0.0);
    assert!(matches!(
        r.textures.peek("red.dds"),
        texture::Peek::Ready(Some(_))
    ));
    r.finish_textures();
}
