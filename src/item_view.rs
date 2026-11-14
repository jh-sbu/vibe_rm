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
use crate::render::{Camera, Environment, GpuModel, Instance, Projection, Renderer, Scene};
use crate::spell_fx::effect::EffectInstance;

/// The camera's vertical field of view, degrees.
const FOV: f32 = 30.0;
/// How long zooming in or out takes, seconds.
const ZOOM_SECONDS: f32 = 0.25;
/// How far ahead of the camera the item's centre stands.
const DISTANCE: f32 = 300.0;
/// The item's bounding sphere's diameter, as a share of the rect's width
/// (before the marker's zoom).
const RECT_SHARE: f32 = 0.6;
/// The radius a magic menu's effect is shown as having: they are sized by
/// their marker's zoom alone, not their bounds, about their origin. Their
/// authored zooms (0.08 to 1.36) shrink the larger art in step with its
/// size (radius times zoom 23 to 81), so the game doesn't bring them to
/// one size first; this value is picked from captures
/// (`known_ambiguities/item-3d.md`).
const MAGIC_RADIUS: f32 = 15.0;

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

    /// Zoomed in: the middle of the frame.
    pub const ZOOMED: Rect = Rect {
        left: 0.15,
        top: 0.0,
        right: 0.85,
        bottom: 1.0,
    };

    fn lerp(&self, to: &Rect, t: f32) -> Rect {
        let l = |a: f32, b: f32| a + (b - a) * t;
        Rect {
            left: l(self.left, to.left),
            top: l(self.top, to.top),
            right: l(self.right, to.right),
            bottom: l(self.bottom, to.bottom),
        }
    }

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

/// What the view shows: an item menu's item, or a magic menu row's menu
/// display object.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shows {
    Item(FormId),
    Magic(MagicRow),
}

/// The model shown: an item's, still; a magic row's, an effect playing.
enum Body {
    Still(Arc<GpuModel>),
    Effect(Box<EffectInstance>),
}

struct Shown {
    shows: Shows,
    body: Body,
    /// Its bounding sphere at rest, in its own space.
    center: Vec3,
    radius: f32,
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
    /// Zoomed in (`ZoomItemModel`, the Item Zoom key), and how far in it is
    /// (0 to 1: CommonLibSSE's `zoomProgress`).
    pub zoomed: bool,
    zoom: f32,
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

/// A row of the magic menu, as the model beside its card sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MagicRow {
    /// A spell or power.
    Spell(FormId),
    Shout(FormId),
    /// The effects of one item (a spell, potion, enchantment...) on the
    /// player.
    Effects(FormId),
}

/// The model of a menu display object (`MDOB`): its `STAT`'s `MODL`. None
/// for anything else, or an editor marker (`meshes/marker...`, which the
/// game doesn't draw: seven script effects name the map marker's).
pub fn menu_object_model(lo: &LoadOrder, object: FormId) -> Option<String> {
    let rec = lo.get(object).filter(|r| r.tag().0 == *b"STAT")?;
    let model = rec
        .get(b"MODL")
        .map(esp::decode_zstring)
        .filter(|s| !s.is_empty())
        .map(|s| crate::world::records::mesh_path(&s))?;
    (!model.to_ascii_lowercase().starts_with("meshes/marker")).then_some(model)
}

impl Engine {
    /// The menu display object a magic menu row shows: a spell's or
    /// shout's own; an active effect's source spell's, else the first of its
    /// shown effects' that has one (potions and enchantments have none of
    /// their own). Null when none has one. `known_ambiguities/item-3d.md`.
    pub fn magic_menu_object(&self, row: MagicRow) -> FormId {
        match row {
            MagicRow::Spell(spell) => self
                .magic_item(spell)
                .map_or(FormId::NULL, |m| m.menu_object),
            MagicRow::Shout(shout) => self.shout(shout).map_or(FormId::NULL, |s| s.menu_object),
            MagicRow::Effects(item) => {
                let Some(m) = self.magic_item(item) else {
                    return FormId::NULL;
                };
                if !m.menu_object.is_null() {
                    return m.menu_object;
                }
                m.effects
                    .iter()
                    .filter_map(|e| self.magic_effect(e.effect))
                    .filter(|x| !x.has(crate::magic::flags::HIDE_IN_UI))
                    .map(|x| x.menu_object)
                    .find(|o| !o.is_null())
                    .unwrap_or(FormId::NULL)
            }
        }
    }

    /// The model a magic menu row shows, if any.
    pub fn magic_model(&self, row: MagicRow) -> Option<String> {
        menu_object_model(&self.lo, self.magic_menu_object(row))
    }
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
            zoomed: false,
            zoom: 0.0,
        }
    }

    /// The item shown, if one is.
    pub fn form(&self) -> Option<FormId> {
        match self.shows() {
            Some(Shows::Item(form)) => Some(form),
            _ => None,
        }
    }

    /// What is shown, if anything.
    pub fn shows(&self) -> Option<Shows> {
        self.shown.as_ref().map(|s| s.shows)
    }

    /// Where the movie has the item, and the viewport's shape.
    pub fn set_rect(&mut self, rect: Rect, aspect: f32) {
        self.rect = rect;
        if aspect.is_finite() && aspect > 0.0 {
            self.aspect = aspect;
        }
    }

    /// Poses the item for the frame, an effect `dt` seconds on (its parts,
    /// its particles simulated).
    fn pose(&mut self, dt: f32) {
        self.scene.dynamic.clear();
        self.scene.particles.clear();
        let Some(s) = &mut self.shown else {
            return;
        };
        // Eased in and out.
        let t = self.zoom * self.zoom * (3.0 - 2.0 * self.zoom);
        let transform = placement(
            s.center,
            s.radius,
            &s.marker,
            self.turn,
            &self.rect.lerp(&Rect::ZOOMED, t),
            self.aspect,
        );
        match &mut s.body {
            Body::Still(model) => self
                .scene
                .dynamic
                .push(Instance::new(model.clone(), transform)),
            Body::Effect(effect) => {
                effect.transform = transform;
                effect.advance(dt);
                // The camera looks along +Y, +Z up.
                effect.step(dt, Vec3::X, Vec3::Z, &mut self.scene.particles);
                effect.draw(&mut self.scene);
            }
        }
    }

    /// Puts a playing effect's materials at its age: the menu's real time,
    /// while the world's clock (the renderer's) stands still.
    pub fn sync_materials(&self, renderer: &Renderer) {
        let Some(Body::Effect(effect)) = self.shown.as_ref().map(|s| &s.body) else {
            return;
        };
        let age = effect.anim.as_ref().map_or(0.0, |a| a.start) + effect.age;
        for inst in effect
            .root
            .iter()
            .chain(effect.parts.iter().map(|(i, _)| i))
        {
            renderer.set_model_age(&inst.model, age);
        }
    }

    /// The mouse moved `fraction` of the viewport's width across: while it
    /// holds the item, a drag across the whole width turns it once round,
    /// rightward to the right (counterclockwise seen from above).
    pub fn drag(&mut self, fraction: f32) {
        if self.rotating {
            self.turn = (self.turn + fraction * std::f32::consts::TAU) % std::f32::consts::TAU;
        }
    }

    pub fn update(&mut self, dt: f32) {
        let step = dt / ZOOM_SECONDS;
        self.zoom = if self.zoomed {
            (self.zoom + step).min(1.0)
        } else {
            (self.zoom - step).max(0.0)
        };
        self.pose(dt);
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
        if form.is_none() {
            view.zoomed = false;
            view.zoom = 0.0;
        }
        if let Some(form) = form {
            let female = self.npc_is_female(crate::player_body::PLAYER_NPC);
            if let Some(path) = item_model(&self.lo, form, female) {
                self.models
                    .load_all(&mut self.renderer, &self.vfs, std::slice::from_ref(&path));
                let marker = inventory_marker(&self.vfs, &path);
                if let (Some(model), Some(view)) = (self.models.get(&path), &mut self.item_view) {
                    view.shown = Some(Shown {
                        shows: Shows::Item(form),
                        center: model.bound_center,
                        radius: model.bound_radius,
                        body: Body::Still(model),
                        marker,
                    });
                }
            }
        }
        if let Some(view) = &mut self.item_view {
            view.pose(0.0);
        }
    }

    /// Shows a magic menu row's menu display object beside its card (None:
    /// nothing), playing as the effect it is from its start. The same row
    /// again keeps it playing, and the player's turn.
    pub fn show_magic_3d(&mut self, row: Option<MagicRow>) {
        let shows = row.map(Shows::Magic);
        let view = self.item_view.get_or_insert_with(ItemView::new);
        if view.shows() == shows && shows.is_some() {
            return;
        }
        view.turn = 0.0;
        view.shown = None;
        if row.is_none() {
            view.zoomed = false;
            view.zoom = 0.0;
        }
        let shown = row.and_then(|row| {
            let model = self.magic_model(row)?;
            let effect = self.menu_effect(&model)?;
            Some(Shown {
                shows: Shows::Magic(row),
                body: Body::Effect(Box::new(effect)),
                center: Vec3::ZERO,
                radius: MAGIC_RADIUS,
                marker: inventory_marker(&self.vfs, &model),
            })
        });
        if let Some(view) = &mut self.item_view {
            view.shown = shown;
            view.pose(0.0);
        }
    }

    /// A menu display object's model as an effect: its standalone
    /// controllers, materials' controllers and particles running from its
    /// start. Not its behaviour graph: the vanilla ones name
    /// `CastingWithIntro.hkx` or `WardBodyFX.hkx` but have no sequences
    /// for it to play (`plans/magic-3d.md`).
    fn menu_effect(&mut self, model: &str) -> Option<EffectInstance> {
        let path = format!("{model}#effect");
        self.models
            .load_all(&mut self.renderer, &self.vfs, std::slice::from_ref(&path));
        let (root, anim) = (self.models.get(&path), self.models.anim(&path));
        if root.is_none() && anim.is_none() {
            return None;
        }
        let seed = self.rand() as u32;
        let mut effect = EffectInstance::new(&self.renderer, root, anim, Mat4::IDENTITY, seed);
        if let Some(nif) = self.vfs.read(model).and_then(|d| nif::Nif::parse(&d).ok()) {
            effect.configure(&nif);
        }
        Some(effect)
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

    /// A plugin with menu display objects: STATs (one an editor marker), a
    /// spell, a shout, effects (one hidden) and a potion.
    fn magic_plugin(dir: &std::path::Path) {
        use esp::writer::{Node, RecordBuilder, write_plugin};
        let mdob = |v: u32| v.to_le_bytes();
        let mgef = |id: u32, flags: u32, object: Option<u32>| {
            let mut data = vec![0u8; 0x8C];
            data[..4].copy_from_slice(&flags.to_le_bytes());
            let r = RecordBuilder::new(b"MGEF", FormId(id)).field(b"DATA", &data);
            match object {
                Some(o) => r.field(b"MDOB", &mdob(o)),
                None => r,
            }
        };
        let groups = [
            Node::top(
                b"STAT",
                vec![
                    RecordBuilder::new(b"STAT", FormId(0x800))
                        .zstring(b"MODL", "Magic\\SpellArt.nif"),
                    RecordBuilder::new(b"STAT", FormId(0x801))
                        .zstring(b"MODL", "Magic\\EffectArt.nif"),
                    RecordBuilder::new(b"STAT", FormId(0x802)).zstring(b"MODL", "marker_map.nif"),
                    RecordBuilder::new(b"STAT", FormId(0x803))
                        .zstring(b"MODL", "Magic\\HiddenArt.nif"),
                ],
            ),
            Node::top(
                b"MGEF",
                vec![
                    mgef(0x810, 0, Some(0x801)),
                    mgef(0x811, crate::magic::flags::HIDE_IN_UI, Some(0x803)),
                    mgef(0x812, 0, None),
                ],
            ),
            Node::top(
                b"SPEL",
                vec![
                    RecordBuilder::new(b"SPEL", FormId(0x820))
                        .field(b"MDOB", &mdob(0x800))
                        .field(b"EFID", &0x810u32.to_le_bytes()),
                    // No object of its own, an effect with one.
                    RecordBuilder::new(b"SPEL", FormId(0x821))
                        .field(b"EFID", &0x812u32.to_le_bytes())
                        .field(b"EFID", &0x810u32.to_le_bytes()),
                ],
            ),
            Node::top(
                b"SHOU",
                vec![RecordBuilder::new(b"SHOU", FormId(0x830)).field(b"MDOB", &mdob(0x802))],
            ),
            Node::top(
                b"ALCH",
                vec![
                    // A hidden effect's object is passed over.
                    RecordBuilder::new(b"ALCH", FormId(0x840))
                        .field(b"EFID", &0x811u32.to_le_bytes())
                        .field(b"EFID", &0x810u32.to_le_bytes()),
                    RecordBuilder::new(b"ALCH", FormId(0x841))
                        .field(b"EFID", &0x812u32.to_le_bytes()),
                ],
            ),
        ];
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("Magic.esp"),
            write_plugin(0, &[], "test", "", 0x900, &groups),
        )
        .unwrap();
    }

    #[test]
    fn a_menu_display_object_shows_its_statics_model() {
        let dir = std::env::temp_dir().join(format!("vrm-mdob-model-{}", std::process::id()));
        magic_plugin(&dir);
        let lo = LoadOrder::load(&dir, &["Magic.esp".into()]).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let model = |id: u32| menu_object_model(&lo, FormId(id));
        assert_eq!(model(0x800).as_deref(), Some("meshes/magic/spellart.nif"));
        // An editor marker, a non-static, nothing.
        assert_eq!(model(0x802), None);
        assert_eq!(model(0x820), None);
        assert_eq!(model(0x8FF), None);
        assert_eq!(model(0), None);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn each_magic_row_shows_its_menu_display_object() {
        let dir = std::env::temp_dir().join(format!("vrm-mdob-rows-{}", std::process::id()));
        magic_plugin(&dir);
        let e = crate::stats::action_tests::headless_engine(&dir, "Magic.esp");
        let _ = std::fs::remove_dir_all(&dir);
        let object = |row| e.magic_menu_object(row);
        assert_eq!(object(MagicRow::Spell(FormId(0x820))), FormId(0x800));
        assert_eq!(object(MagicRow::Shout(FormId(0x830))), FormId(0x802));
        // The spell's own, else its first shown effect's.
        assert_eq!(object(MagicRow::Effects(FormId(0x820))), FormId(0x800));
        assert_eq!(object(MagicRow::Effects(FormId(0x821))), FormId(0x801));
        assert_eq!(object(MagicRow::Effects(FormId(0x840))), FormId(0x801));
        assert_eq!(object(MagicRow::Effects(FormId(0x841))), FormId::NULL);
        assert_eq!(object(MagicRow::Spell(FormId(0x821))), FormId::NULL);
        assert_eq!(
            e.magic_model(MagicRow::Spell(FormId(0x820))).as_deref(),
            Some("meshes/magic/spellart.nif")
        );
        // The shout's object is an editor marker: nothing shows.
        assert_eq!(e.magic_model(MagicRow::Shout(FormId(0x830))), None);
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

    /// A synthetic effect: a still root of radius 10 at the origin, and one
    /// part of radius 5 standing 20 up.
    fn effect() -> EffectInstance {
        let model = |center: Vec3, radius: f32| {
            Arc::new(GpuModel {
                path: "synthetic".into(),
                parts: Vec::new(),
                skinned: Vec::new(),
                particles: Vec::new(),
                bound_center: center,
                bound_radius: radius,
            })
        };
        let part = model(Vec3::ZERO, 5.0);
        let rest = Mat4::from_translation(Vec3::new(0.0, 0.0, 20.0));
        let anim = Arc::new(crate::world::loader::ModelAnim {
            parts: vec![crate::world::loader::AnimPart {
                node: "part".into(),
                block: 1,
                parent: Mat4::IDENTITY,
                rest,
                model: part.clone(),
                within: None,
            }],
            sequences: Vec::new(),
            controllers: Vec::new(),
            start: 0.0,
            graph: None,
        });
        EffectInstance::from_parts(
            Mat4::IDENTITY,
            Some(Instance::new(model(Vec3::ZERO, 10.0), Mat4::IDENTITY)),
            Some(anim),
            vec![(Instance::new(part, rest), true)],
            f32::INFINITY,
        )
    }

    #[test]
    fn a_magic_rows_effect_plays_on_in_the_rect() {
        let fx = effect();
        let (center, radius) = (Vec3::ZERO, MAGIC_RADIUS);
        let mut view = ItemView::new();
        let row = MagicRow::Spell(FormId(0x800));
        view.shown = Some(Shown {
            shows: Shows::Magic(row),
            body: Body::Effect(Box::new(fx)),
            center,
            radius,
            marker: Marker::default(),
        });
        assert_eq!(view.shows(), Some(Shows::Magic(row)));
        // Not an item: the item menus' zoom and turn checks see none.
        assert_eq!(view.form(), None);
        view.update(0.5);
        view.update(0.25);
        let Some(Shown {
            body: Body::Effect(fx),
            ..
        }) = &view.shown
        else {
            unreachable!()
        };
        assert!((fx.age - 0.75).abs() < 1e-6);
        let at = placement(
            center,
            radius,
            &Marker::default(),
            0.0,
            &Rect::DEFAULT,
            16.0 / 9.0,
        );
        assert_eq!(fx.transform, at);
        // The part stands where its rest pose puts it under the placement,
        // and both it and the still root are drawn.
        let part = fx.parts[0].0.transform;
        let expected = at * Mat4::from_translation(Vec3::new(0.0, 0.0, 20.0));
        assert!(part.abs_diff_eq(expected, 1e-3), "{part} {expected}");
        assert_eq!(
            view.scene.dynamic.len(),
            0,
            "synthetic models have no meshes"
        );
        let (x, y) = project(at, center, 16.0 / 9.0);
        let (cx, cy) = Rect::DEFAULT.centre();
        assert!((x - cx).abs() < 1e-4 && (y - cy).abs() < 1e-4);
    }

    /// Vanilla spells' and a shout's menu objects load as effects, particles
    /// and all, and play on.
    #[test]
    #[ignore = "needs a headless GPU adapter and installed vanilla Skyrim SE"]
    fn vanilla_magic_rows_play_their_effects() {
        let data = vfs::locate_data_dir().expect("Skyrim SE Data directory");
        let names = ["Skyrim.esm".to_string()];
        let lo = LoadOrder::load(&data, &names).unwrap();
        let vfs = vfs::Vfs::new(&data, &names);
        let scratch = std::env::temp_dir().join(format!("vrm-magic-3d-{}", std::process::id()));
        let mut e = crate::stats::action_tests::headless_engine_with(lo, vfs, &scratch);
        for (row, model) in [
            // Flames: the fireball art; Healing; Conjure Familiar; Unrelenting Force.
            (MagicRow::Spell(FormId(0x12FCD)), "fireballinvart"),
            (MagicRow::Spell(FormId(0x12FCC)), "maginvhealspellart"),
            (MagicRow::Spell(FormId(0x640B6)), "maginvsummon"),
            (MagicRow::Shout(FormId(0x13E07)), "maginvunrelentspellart"),
        ] {
            let path = e.magic_model(row).unwrap();
            assert!(path.contains(model), "{row:?}: {path}");
            e.show_magic_3d(Some(row));
            let view = e.item_view.as_mut().unwrap();
            assert_eq!(view.shows(), Some(Shows::Magic(row)));
            for _ in 0..30 {
                view.update(1.0 / 30.0);
            }
            let s = view.shown.as_ref().unwrap();
            let Body::Effect(fx) = &s.body else {
                panic!("{row:?} is still")
            };
            assert_eq!((s.center, s.radius), (Vec3::ZERO, MAGIC_RADIUS));
            let systems = fx
                .root
                .iter()
                .chain(fx.parts.iter().map(|(i, _)| i))
                .map(|i| i.model.particles.len())
                .sum::<usize>();
            if systems > 0 {
                assert!(!view.scene.particles.is_empty(), "{row:?}: no particles");
            }
            assert!(!view.scene.dynamic.is_empty(), "{row:?}: no meshes");
            eprintln!(
                "{row:?} {path}: {} meshes, {} systems, {} batches",
                view.scene.dynamic.len(),
                systems,
                view.scene.particles.len()
            );
            // The same row again keeps it playing.
            let age = fx.age;
            e.show_magic_3d(Some(row));
            let Some(Body::Effect(fx)) = e
                .item_view
                .as_ref()
                .unwrap()
                .shown
                .as_ref()
                .map(|s| &s.body)
            else {
                unreachable!()
            };
            assert_eq!(fx.age, age);
        }
        e.show_magic_3d(None);
        assert_eq!(e.item_view.as_ref().unwrap().shows(), None);
        let _ = std::fs::remove_dir_all(&scratch);
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
    fn zooming_eases_the_item_to_the_middle_and_back() {
        let mut view = ItemView::new();
        view.zoomed = true;
        view.update(ZOOM_SECONDS * 0.5);
        assert_eq!(view.zoom, 0.5);
        view.update(ZOOM_SECONDS);
        assert_eq!(view.zoom, 1.0);
        view.zoomed = false;
        view.update(ZOOM_SECONDS * 2.0);
        assert_eq!(view.zoom, 0.0);
        let half = Rect::DEFAULT.lerp(&Rect::ZOOMED, 0.5);
        assert!((half.left - (Rect::DEFAULT.left + Rect::ZOOMED.left) * 0.5).abs() < 1e-6);
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
