//! Effect shader colour, drawn on a headless adapter and read back.

use super::*;
use glam::{Mat4, Vec2, Vec3, Vec4};
use model::{BlendMode, CpuMesh, CpuModel, MaterialDesc, ShaderKind, Vertex};

const SIZE: u32 = 16;

fn renderer() -> Renderer {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        compatible_surface: None,
        apply_limit_buckets: false,
        ..Default::default()
    }))
    .expect("a headless GPU adapter is required for this ignored test");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::TEXTURE_COMPRESSION_BC,
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .unwrap();
    Renderer::new(device, queue, wgpu::TextureFormat::Rgba8Unorm, SIZE, SIZE)
}

/// A 4x4 texture of one colour.
fn solid(rgba: [u8; 4]) -> dds::Dds {
    dds::Dds {
        width: 4,
        height: 4,
        mips: 1,
        layers: 1,
        cube: false,
        format: dds::Format::Rgba8,
        data: rgba.repeat(16),
    }
}

/// The centre pixel of an opaque effect quad filling the view, its vertices
/// coloured `grey`, with or without the palette giving its colour.
fn centre(r: &mut Renderer, grey: f32, palette: bool) -> [u8; 4] {
    let corner = |x: f32, z: f32| Vertex {
        position: [x, 0.0, z],
        normal: [0.0, -1.0, 0.0],
        tangent: [1.0, 0.0, 0.0],
        bitangent: [0.0, 0.0, 1.0],
        uv: [0.5, 0.5],
        color: [grey, grey, grey, 1.0],
    };
    let material = MaterialDesc {
        kind: ShaderKind::Effect,
        diffuse: Some("base.dds".into()),
        glow: Some("palette.dds".into()),
        flags1: if palette {
            nif::sf1::GREYSCALE_TO_PALETTE_COLOR
        } else {
            0
        },
        flags2: nif::sf2::VERTEX_COLORS,
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
    let model = Arc::new(r.upload_model(&cpu));
    let mut scene = Scene::default();
    scene
        .dynamic
        .push(Instance::new(model, Mat4::from_translation(Vec3::Y * 20.0)));
    let camera = Camera {
        position: Vec3::ZERO,
        yaw: 0.0,
        pitch: 0.0,
        fov_y: 1.0,
    };
    let pixels = r.render_to_image(&scene, &camera, |_, _| {});
    let i = ((SIZE / 2 * SIZE + SIZE / 2) * 4) as usize;
    pixels[i..i + 4].try_into().unwrap()
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic geometry only"]
fn palette_colour_is_picked_by_the_vertex_colour_not_also_tinted_by_it() {
    let mut r = renderer();
    r.add_texture("base.dds", Some(&solid([255, 255, 255, 255])));
    r.add_texture("palette.dds", Some(&solid([255, 128, 0, 255])));

    // Every palette row is the same orange: a dark vertex grey picks a row
    // and leaves the colour as bright as a white one does.
    let dark = centre(&mut r, 0.1, true);
    let white = centre(&mut r, 1.0, true);
    assert!(dark[0] > 150, "palette colour drawn at {dark:?}");
    assert!(
        dark[..3]
            .iter()
            .zip(&white[..3])
            .all(|(a, b)| a.abs_diff(*b) <= 2),
        "{dark:?} vs {white:?}"
    );

    // Without the palette the vertex colour still tints the texture.
    let tinted = centre(&mut r, 0.1, false);
    let plain = centre(&mut r, 1.0, false);
    assert!(
        u32::from(tinted[0]) * 2 < u32::from(plain[0]),
        "{tinted:?} vs {plain:?}"
    );
}
