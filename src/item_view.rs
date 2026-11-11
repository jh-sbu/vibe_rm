//! The item menus' and crafting's 3D item: the highlighted item's model
//! beside its card, drawn over the frame (`Renderer::render_over`) with a
//! camera and light of its own. The movies ask for it (`UpdateItem3D`,
//! `ShowItem3D`) and say where it goes: their `MouseRotationRect`, the card's
//! column down to 0.55 of the stage's height. The model is brought to one
//! size, then turned and sized by its `BSInvMarker` (`INV`), when it has one.
//! Plan: `plans/item-3d.md`; choices: `known_ambiguities/item-3d.md`.

use std::sync::Arc;

use esp::{FormId, LoadOrder};
use glam::{Mat4, Quat, Vec3};

use crate::engine::Engine;
use crate::render::{Camera, Environment, GpuModel, Instance, Projection, Scene};

/// The camera's vertical field of view, degrees.
const FOV: f32 = 30.0;
/// How far ahead of the camera the item's centre stands.
const DISTANCE: f32 = 300.0;
/// The item's bounding sphere's diameter, as a share of the rect's width
/// (before the marker's zoom).
const RECT_SHARE: f32 = 0.6;

/// The rect the item stands in, as fractions of the viewport (left, top,
/// right, bottom).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Rect {
    /// The card's column on a 16:9 stage, until the movie says (the item
    /// menus' card holder stands about 0.62 to 0.94 of the stage across).
    pub const DEFAULT: Rect = Rect {
        left: 0.62,
        top: 0.0,
        right: 0.94,
        bottom: 0.55,
    };

    fn centre(&self) -> (f32, f32) {
        (
            (self.left + self.right) * 0.5,
            (self.top + self.bottom) * 0.5,
        )
    }
}

/// How the inventory shows a model: its marker's turn, Rx·Ry·Rz (about its
/// own Z first, then Y, then X: the draugr sword's 135° about Z and zoom 1.74
/// lay it across the rect's diagonal; the other order stands it up and off
/// the screen), and zoom; none turns it nowhere and leaves its size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Marker {
    pub rotation: Quat,
    pub zoom: f32,
}

impl Default for Marker {
    fn default() -> Self {
        Marker {
            rotation: Quat::IDENTITY,
            zoom: 1.0,
        }
    }
}

impl From<nif::InvMarker> for Marker {
    fn from(m: nif::InvMarker) -> Self {
        let r = m.radians();
        Marker {
            rotation: Quat::from_euler(glam::EulerRot::XYZ, r.x, r.y, r.z),
            zoom: if m.zoom > 0.0 { m.zoom } else { 1.0 },
        }
    }
}

struct Shown {
    form: FormId,
    model: Arc<GpuModel>,
    marker: Marker,
}

pub struct ItemView {
    scene: Scene,
    shown: Option<Shown>,
    rect: Rect,
    /// The viewport's width over its height.
    aspect: f32,
    /// The player's turn of it about the vertical, radians.
    pub turn: f32,
    /// The mouse holds it to turn (`StartMouseRotation` to
    /// `StopMouseRotation`).
    pub rotating: bool,
}

/// The camera the item is seen with: at the origin, looking along +Y.
pub fn camera() -> Camera {
    Camera {
        position: Vec3::ZERO,
        yaw: 0.0,
        pitch: 0.0,
        fov_y: FOV.to_radians(),
        projection: Projection::Perspective,
    }
}

/// Where a model with bounding sphere (`center`, `radius`) stands to fill
/// its share of `rect`: its centre on the rect's middle, `DISTANCE` ahead,
/// turned by the marker and then `turn` about the vertical.
pub fn placement(
    center: Vec3,
    radius: f32,
    marker: &Marker,
    turn: f32,
    rect: &Rect,
    aspect: f32,
) -> Mat4 {
    let tan = (FOV.to_radians() * 0.5).tan();
    let (cx, cy) = rect.centre();
    let (ndc_x, ndc_y) = (cx * 2.0 - 1.0, 1.0 - cy * 2.0);
    let at = Vec3::new(
        ndc_x * tan * aspect * DISTANCE,
        DISTANCE,
        ndc_y * tan * DISTANCE,
    );
    // The rect's width in view units at that distance.
    let width = (rect.right - rect.left) * 2.0 * tan * aspect * DISTANCE;
    let scale = width * RECT_SHARE * 0.5 * marker.zoom / radius.max(0.01);
    Mat4::from_translation(at)
        * Mat4::from_rotation_z(turn)
        * Mat4::from_quat(marker.rotation)
        * Mat4::from_scale(Vec3::splat(scale))
        * Mat4::from_translation(-center)
}

/// The model the inventory shows for an item: its world model (`MODL`), an
/// armor's ground model for the player's sex (`MOD2` male, `MOD4` female,
/// either if the other is missing), a book's inventory art (`INAM`, a
/// static) before its own. Skinned shapes are posed at rest (bows).
pub fn item_model(lo: &LoadOrder, form: FormId, female: bool) -> Option<String> {
    let rec = lo.get(form)?;
    let path = |rec: &esp::LoadedRecord<'_>, tag: &[u8; 4]| {
        rec.get(tag)
            .map(esp::decode_zstring)
            .filter(|s| !s.is_empty())
            .map(|s| crate::world::records::mesh_path(&s))
    };
    let model = match &rec.tag().0 {
        b"ARMO" => {
            let (first, then) = if female {
                (b"MOD4", b"MOD2")
            } else {
                (b"MOD2", b"MOD4")
            };
            path(&rec, first).or_else(|| path(&rec, then))
        }
        b"BOOK" => rec
            .get(b"INAM")
            .and_then(|d| d.get(..4))
            .map(|d| rec.fid(FormId(u32::from_le_bytes(d.try_into().unwrap()))))
            .filter(|f| !f.is_null())
            .and_then(|art| lo.get(art))
            .and_then(|art| path(&art, b"MODL"))
            .or_else(|| path(&rec, b"MODL")),
        _ => path(&rec, b"MODL"),
    }?;
    Some(format!("{model}#rigid"))
}

impl ItemView {
    fn new() -> Self {
        let scene = Scene {
            env: Environment {
                // From the camera's upper left.
                sun_dir: Vec3::new(-0.4, -1.0, 0.6).normalize(),
                sun_color: Vec3::splat(0.9),
                ambient: Vec3::splat(0.45),
                fog_near: 0.0,
                fog_far: 1.0e7,
                fog_max: 0.0,
                sky: false,
                ..Default::default()
            },
            ..Default::default()
        };
        ItemView {
            scene,
            shown: None,
            rect: Rect::DEFAULT,
            aspect: 16.0 / 9.0,
            turn: 0.0,
            rotating: false,
        }
    }

    /// The item shown, if one is.
    pub fn form(&self) -> Option<FormId> {
        self.shown.as_ref().map(|s| s.form)
    }

    /// Where the movie has the item, and the viewport's shape.
    pub fn set_rect(&mut self, rect: Rect, aspect: f32) {
        self.rect = rect;
        if aspect.is_finite() && aspect > 0.0 {
            self.aspect = aspect;
        }
    }

    /// Poses the item for the frame.
    fn pose(&mut self) {
        self.scene.dynamic.clear();
        let Some(s) = &self.shown else {
            return;
        };
        let transform = placement(
            s.model.bound_center,
            s.model.bound_radius,
            &s.marker,
            self.turn,
            &self.rect,
            self.aspect,
        );
        self.scene
            .dynamic
            .push(Instance::new(s.model.clone(), transform));
    }

    /// The mouse moved `fraction` of the viewport's width across: while it
    /// holds the item, a drag across the whole width turns it once round,
    /// rightward to the right (counterclockwise seen from above).
    pub fn drag(&mut self, fraction: f32) {
        if self.rotating {
            self.turn = (self.turn + fraction * std::f32::consts::TAU) % std::f32::consts::TAU;
        }
    }

    pub fn update(&mut self, _dt: f32) {
        self.pose();
    }
}

/// The item's scene and camera, while one is shown.
pub fn frame_parts(view: &Option<ItemView>) -> Option<(&Scene, Camera)> {
    view.as_ref()
        .filter(|v| v.shown.is_some())
        .map(|v| (&v.scene, camera()))
}

impl Engine {
    /// Shows `form`'s model beside the card (None: nothing). The same item
    /// again keeps the player's turn.
    pub fn show_item_3d(&mut self, form: Option<FormId>) {
        let view = self.item_view.get_or_insert_with(ItemView::new);
        if view.form() == form {
            return;
        }
        view.turn = 0.0;
        view.shown = None;
        if let Some(form) = form {
            let female = self.npc_is_female(crate::player_body::PLAYER_NPC);
            if let Some(path) = item_model(&self.lo, form, female) {
                self.models
                    .load_all(&mut self.renderer, &self.vfs, std::slice::from_ref(&path));
                let marker = inventory_marker(&self.vfs, &path);
                if let (Some(model), Some(view)) = (self.models.get(&path), &mut self.item_view) {
                    view.shown = Some(Shown {
                        form,
                        model,
                        marker,
                    });
                }
            }
        }
        if let Some(view) = &mut self.item_view {
            view.pose();
        }
    }

    /// The menu closed: no item, and its view gone.
    pub fn close_item_3d(&mut self) {
        self.item_view = None;
    }
}

/// The model's `BSInvMarker`, or none's defaults.
fn inventory_marker(vfs: &vfs::Vfs, path: &str) -> Marker {
    let file = path.split_once('#').map_or(path, |(f, _)| f);
    vfs.read(file)
        .and_then(|d| nif::Nif::parse(&d).ok())
        .and_then(|n| n.inv_marker())
        .map(Marker::from)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(m: Mat4, p: Vec3, aspect: f32) -> (f32, f32) {
        let cam = camera();
        let clip = cam.proj(aspect) * cam.view() * m.transform_point3(p).extend(1.0);
        let ndc = clip.truncate() / clip.w;
        ((ndc.x + 1.0) * 0.5, (1.0 - ndc.y) * 0.5)
    }

    #[test]
    fn each_kind_of_item_shows_its_inventory_model() {
        use esp::writer::{Node, RecordBuilder, write_plugin};
        let dir = std::env::temp_dir().join(format!("vrm-item-view-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let groups = [
            Node::top(
                b"WEAP",
                vec![
                    RecordBuilder::new(b"WEAP", FormId(0x800))
                        .zstring(b"MODL", "Weapons\\Sword.nif"),
                ],
            ),
            Node::top(
                b"ARMO",
                vec![
                    RecordBuilder::new(b"ARMO", FormId(0x801))
                        .zstring(b"MOD2", "Armor\\M.nif")
                        .zstring(b"MOD4", "Armor\\F.nif"),
                    RecordBuilder::new(b"ARMO", FormId(0x802)).zstring(b"MOD2", "Armor\\Only.nif"),
                ],
            ),
            Node::top(
                b"STAT",
                vec![
                    RecordBuilder::new(b"STAT", FormId(0x803))
                        .zstring(b"MODL", "Clutter\\Open.nif"),
                ],
            ),
            Node::top(
                b"BOOK",
                vec![
                    RecordBuilder::new(b"BOOK", FormId(0x804))
                        .zstring(b"MODL", "Clutter\\Shut.nif")
                        .field(b"INAM", &0x803u32.to_le_bytes()),
                    RecordBuilder::new(b"BOOK", FormId(0x805))
                        .zstring(b"MODL", "Clutter\\Note.nif"),
                ],
            ),
        ];
        std::fs::write(
            dir.join("Items.esp"),
            write_plugin(0, &[], "test", "inventory models", 0x900, &groups),
        )
        .unwrap();
        let lo = LoadOrder::load(&dir, &["Items.esp".into()]).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let model = |id: u32, female: bool| item_model(&lo, FormId(id), female);
        assert_eq!(
            model(0x800, false).as_deref(),
            Some("meshes/weapons/sword.nif#rigid")
        );
        assert_eq!(
            model(0x801, false).as_deref(),
            Some("meshes/armor/m.nif#rigid")
        );
        assert_eq!(
            model(0x801, true).as_deref(),
            Some("meshes/armor/f.nif#rigid")
        );
        assert_eq!(
            model(0x802, true).as_deref(),
            Some("meshes/armor/only.nif#rigid")
        );
        assert_eq!(
            model(0x804, false).as_deref(),
            Some("meshes/clutter/open.nif#rigid")
        );
        assert_eq!(
            model(0x805, false).as_deref(),
            Some("meshes/clutter/note.nif#rigid")
        );
        assert_eq!(model(0x806, false), None);
    }

    #[test]
    fn the_items_centre_lands_on_the_rects_middle() {
        let rect = Rect {
            left: 0.6,
            top: 0.1,
            right: 0.9,
            bottom: 0.5,
        };
        let center = Vec3::new(3.0, -2.0, 40.0);
        let m = placement(center, 25.0, &Marker::default(), 1.0, &rect, 16.0 / 9.0);
        let (x, y) = project(m, center, 16.0 / 9.0);
        assert!((x - 0.75).abs() < 1e-4 && (y - 0.3).abs() < 1e-4, "{x} {y}");
    }

    #[test]
    fn every_model_is_brought_to_one_size_then_zoomed() {
        let rect = Rect::DEFAULT;
        let size = |radius: f32, zoom: f32| {
            let marker = Marker {
                zoom,
                ..Default::default()
            };
            let m = placement(Vec3::ZERO, radius, &marker, 0.0, &rect, 1.5);
            (m.transform_point3(Vec3::X * radius) - m.transform_point3(Vec3::ZERO)).length()
        };
        assert!((size(10.0, 1.0) - size(80.0, 1.0)).abs() < 1e-3);
        assert!((size(10.0, 1.25) / size(10.0, 1.0) - 1.25).abs() < 1e-4);
    }

    #[test]
    fn a_ground_armors_marker_stands_it_up_facing_the_camera() {
        // Armor ground models lie on their backs; 1570 about X turns their
        // front (+Z) to -Y, toward the camera.
        let marker = Marker::from(nif::InvMarker {
            rotation: [1570, 0, 0],
            zoom: 1.0,
        });
        let front = marker.rotation * Vec3::Z;
        assert!((front - Vec3::NEG_Y).length() < 1e-3, "{front}");
    }

    #[test]
    fn a_markers_z_turn_lays_a_blade_across_the_screen() {
        // A blade lying along +Y, stood up by X and turned 45° by Z: Z
        // first, in the model's frame, so it lies diagonally in the screen's
        // plane (X and Z), not turned about the vertical.
        let marker = Marker::from(nif::InvMarker {
            rotation: [1570, 0, 785],
            zoom: 1.0,
        });
        let blade = marker.rotation * Vec3::Y;
        assert!(blade.y.abs() < 2e-3, "{blade}");
        assert!((blade.x.abs() - blade.z.abs()).abs() < 2e-3, "{blade}");
    }

    #[test]
    fn the_mouse_turns_the_item_only_while_it_holds_it() {
        let mut view = ItemView::new();
        view.drag(0.25);
        assert_eq!(view.turn, 0.0);
        view.rotating = true;
        view.drag(0.25);
        assert!((view.turn - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        view.rotating = false;
        view.drag(0.25);
        assert!((view.turn - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    }

    #[test]
    fn turning_is_about_the_vertical_after_the_marker() {
        let marker = Marker::from(nif::InvMarker {
            rotation: [1570, 0, 0],
            zoom: 1.0,
        });
        let rect = Rect::DEFAULT;
        let m0 = placement(Vec3::ZERO, 1.0, &marker, 0.0, &rect, 1.5);
        let m1 = placement(Vec3::ZERO, 1.0, &marker, 0.5, &rect, 1.5);
        // The model's +Y, up after the marker, stays up.
        let up = |m: Mat4| (m.transform_vector3(Vec3::Y)).normalize();
        assert!((up(m0) - Vec3::Z).length() < 1e-3);
        assert!((up(m0) - up(m1)).length() < 2e-3, "{} {}", up(m0), up(m1));
    }
}
