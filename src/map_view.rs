//! The world map: a top-down camera over the worldspace's distant LOD
//! (`world::lod`), panned and zoomed within the worldspace's map data
//! (`WRLD` `MNAM`: the cells it shows and the camera's limits), and the
//! markers on it projected to the screen for `map.swf` (`swf_ui::map_menu`).
//! The map draws its own scene, only LOD: the loaded cells aren't in it.
//! Choices: `known_ambiguities/map-view.md`.

use esp::FormId;
use esp::worldspace::{WorldMapData, WorldMapOffset};
use glam::{Mat4, Vec2, Vec3};

use crate::engine::{Engine, Location};
use crate::render::{Camera, Environment, Scene};
use crate::world::lod::Lod;

/// `WRLD` `PNAM`'s Use Map Data.
const USE_MAP_DATA: u16 = 0x04;
/// `map.swf`'s icon type of the player's own marker (`IconTypes`' last).
pub const PLAYER_ICON: u8 = 66;
pub const PLAYER_SET_ICON: u8 = 65;
/// The lowest the map's camera tilts, and the highest (nearly straight down:
/// exactly down has no up),
/// degrees above the horizon.
pub(crate) const MIN_PITCH: f32 = 25.0;
pub(crate) const MAX_PITCH: f32 = 89.0;
/// Blocks the map loads a frame.
const BLOCKS_PER_FRAME: usize = 8;

/// A marker on the map, in the order `map.swf` was told them.
#[derive(Debug, Clone)]
pub struct MapRow {
    /// Stable destination identity; overlays and the player have none.
    pub marker: Option<FormId>,
    pub name: String,
    /// `TNAM`, the movie's icon type.
    pub kind: u8,
    /// Not discovered (and not allowed travel to): the hollow icon.
    pub undiscovered: bool,
    /// Where it is in the map's worldspace.
    pub pos: Vec2,
    /// The player's marker turns with them: degrees clockwise from north.
    pub rotation: f32,
}

/// Movie callback indices are untrusted floating-point values. Never truncate a
/// fractional or negative index into another row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowAction {
    Travel(FormId),
    PlayerMarker,
    None,
}

pub(crate) fn row_action(rows: &[MapRow], index: f64) -> RowAction {
    if !index.is_finite() || index < 0.0 || index.fract() != 0.0 {
        return RowAction::None;
    }
    match rows.get(index as usize) {
        Some(row) if row.kind == PLAYER_SET_ICON => RowAction::PlayerMarker,
        Some(row) => row.marker.map_or(RowAction::None, RowAction::Travel),
        None => RowAction::None,
    }
}

/// Stable row identity across rebuilds. Location references survive renaming;
/// the one custom destination survives moving; quest overlays use kind/place/name.
pub(crate) fn same_row(a: &MapRow, b: &MapRow) -> bool {
    if a.marker.is_some() || b.marker.is_some() {
        return a.marker == b.marker;
    }
    a.kind == b.kind
        && (a.kind == PLAYER_SET_ICON
            || a.kind == PLAYER_ICON
            || (a.pos == b.pos && a.name == b.name))
}

pub struct MapView {
    /// The worldspace whose map data this is (a child worldspace shows its
    /// parent's).
    pub world: FormId,
    pub data: WorldMapData,
    /// Where the player is on the map, with their heading, when they're in
    /// the worldspace.
    pub player: Option<(Vec2, f32)>,
    /// What the camera looks at, world units.
    pub center: Vec2,
    /// 0 (the camera at `min_height`) to 1 (at `max_height`).
    pub zoom: f32,
    /// Degrees above the horizon.
    pub pitch: f32,
    pub heading: f32,
    fov_y: f32,
    lod: Lod,
    pub scene: Scene,
    pub rows: Vec<MapRow>,
}

/// Convert a worldspace position to the map it inherits, composing each
/// authored ONAM transform. Cycles and malformed/nonfinite data have no point.
pub(crate) fn world_map_point(
    lo: &esp::LoadOrder,
    mut world: FormId,
    mut pos: Vec2,
) -> Option<(FormId, Vec2)> {
    let mut seen = std::collections::HashSet::new();
    for _ in 0..64 {
        if !pos.is_finite() || !seen.insert(world) {
            return None;
        }
        let rec = lo.get(world).filter(|r| r.tag().0 == *b"WRLD")?;
        let flags = match rec.get(b"PNAM") {
            Some(d) => u16::from_le_bytes(d.get(..2)?.try_into().ok()?),
            None => 0,
        };
        if flags & USE_MAP_DATA == 0 || rec.get(b"WNAM").is_none() {
            return Some((world, pos));
        }
        let parent = rec.fid(FormId(u32::from_le_bytes(
            rec.get(b"WNAM")?.get(..4)?.try_into().ok()?,
        )));
        if let Some(data) = rec.get(b"ONAM") {
            let offset = WorldMapOffset::parse(data)?;
            if !offset.scale.is_finite()
                || offset.scale <= 0.0
                || !offset.offset.iter().all(|v| v.is_finite())
            {
                return None;
            }
            pos = pos * offset.scale + Vec2::new(offset.offset[0], offset.offset[1]);
        }
        world = parent;
    }
    None
}

impl MapView {
    fn camera(&self) -> Camera {
        let cam = &self.data.camera;
        let height = cam.min_height + (cam.max_height - cam.min_height) * self.zoom;
        let p = self.pitch.to_radians();
        // Looking north, tilted down.
        let yaw = self.heading.to_radians();
        let forward = Vec3::new(yaw.sin() * p.cos(), yaw.cos() * p.cos(), -p.sin());
        let target = self.center.extend(0.0);
        Camera {
            position: target - forward * (height / p.sin()),
            yaw,
            pitch: -p,
            fov_y: self.fov_y,
            projection: crate::render::Projection::Perspective,
        }
    }

    fn view_proj(&self, aspect: f32) -> Mat4 {
        let c = self.camera();
        c.proj(aspect) * c.view()
    }

    /// A world position on the screen as fractions of its width and height
    /// from the top left; None behind the camera.
    pub fn screen_of(&self, pos: Vec2, aspect: f32) -> Option<Vec2> {
        let clip = self.view_proj(aspect) * pos.extend(0.0).extend(1.0);
        if clip.w <= 0.0 {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        Some(Vec2::new((ndc.x + 1.0) / 2.0, (1.0 - ndc.y) / 2.0))
    }

    pub(crate) fn ground_height(&self, at: Vec2) -> Option<f32> {
        self.lod.height_at(at)
    }

    /// Inverse marker projection: intersect the pointer ray with marker z=0.
    pub fn ground_at(&self, screen: Vec2, aspect: f32) -> Option<Vec2> {
        if !screen.is_finite() || !aspect.is_finite() || aspect <= 0.0 {
            return None;
        }
        let camera = self.camera();
        let eye = camera.position;
        let forward = camera.forward();
        let right = camera.right();
        let up = right.cross(forward);
        let half = (camera.fov_y / 2.0).tan();
        let ray = forward
            + right * ((screen.x * 2.0 - 1.0) * half * aspect)
            + up * ((1.0 - screen.y * 2.0) * half);
        if !ray.is_finite() || ray.z >= -f32::EPSILON {
            return None;
        }
        let t = -eye.z / ray.z;
        let point = (eye + ray * t).truncate();
        (t >= 0.0 && point.is_finite()).then_some(point)
    }

    /// Where the camera's centre is over the world, as a fraction of the
    /// screen's height: how far one screen fraction is, world units, at the
    /// centre.
    fn world_per_screen_height(&self) -> f32 {
        let cam = &self.data.camera;
        let height = cam.min_height + (cam.max_height - cam.min_height) * self.zoom;
        let distance = height / self.pitch.to_radians().sin();
        2.0 * distance * (self.fov_y / 2.0).tan()
    }

    /// Moves the view by screen fractions (right and up positive), staying
    /// on the map.
    pub fn pan(&mut self, right: f32, up: f32) {
        let scale = self.world_per_screen_height();
        // Up the screen is further north, stretched by the tilt.
        let yaw = self.heading.to_radians();
        self.center += Vec2::new(yaw.cos(), -yaw.sin()) * right * scale
            + Vec2::new(yaw.sin(), yaw.cos()) * up * scale / self.pitch.to_radians().sin();
        let [min_x, min_y, max_x, max_y] = self.data.bounds();
        self.center = self
            .center
            .clamp(Vec2::new(min_x, min_y), Vec2::new(max_x, max_y));
    }

    pub fn zoom_by(&mut self, by: f32) {
        self.zoom = (self.zoom + by).clamp(0.0, 1.0);
    }

    pub fn turn_by(&mut self, degrees: f32) {
        let heading = (self.heading + degrees).rem_euclid(360.0);
        // `rem_euclid` rounds a tiny negative sum up to exactly 360.
        self.heading = if heading < 360.0 { heading } else { 0.0 };
    }

    pub fn tilt_by(&mut self, degrees: f32) {
        self.pitch = (self.pitch + degrees).clamp(MIN_PITCH, MAX_PITCH);
    }

    /// The marker nearest a screen position (fractions) within `reach` of
    /// it, the player's own aside: its row.
    pub fn marker_at(&self, at: Vec2, aspect: f32, reach: f32) -> Option<usize> {
        let mut best: Option<(f32, usize)> = None;
        for (i, row) in self.rows.iter().enumerate() {
            if row.kind == PLAYER_ICON {
                continue;
            }
            let Some(s) = self.screen_of(row.pos, aspect) else {
                continue;
            };
            let d = ((s - at) * Vec2::new(aspect, 1.0)).length();
            if d < reach && best.is_none_or(|(b, _)| d < b) {
                best = Some((d, i));
            }
        }
        best.map(|(_, i)| i)
    }
}

/// The world map's own scene and camera while it is up.
pub fn frame_parts(
    map: &Option<MapView>,
    menu: Option<crate::items::Menu>,
) -> Option<(&Scene, Camera)> {
    match map {
        Some(v)
            if matches!(
                menu,
                Some(crate::items::Menu::Map | crate::items::Menu::Journal)
            ) =>
        {
            Some((&v.scene, v.camera()))
        }
        _ => None,
    }
}

impl Engine {
    /// The worldspace the map opens on, with its map data: where the player
    /// is (a child worldspace's is its parent's), else Tamriel.
    pub(crate) fn map_world(&self) -> Option<(FormId, WorldMapData)> {
        let here = match self.location {
            Location::Exterior { world, .. } => world,
            _ => self.lo.find_editor_id("Tamriel")?,
        };
        let (owner, _) = world_map_point(&self.lo, here, Vec2::ZERO)?;
        let rec = self.lo.get(owner)?;
        Some((owner, WorldMapData::parse(rec.get(b"MNAM")?)?))
    }

    pub(crate) fn map_available(&self) -> bool {
        if self.map_view.is_some() {
            return true;
        }
        self.map_world()
            .and_then(|(world, _)| self.lo.get(world)?.editor_id())
            .is_some_and(|edid| !Lod::new(&self.vfs, &edid).is_empty())
    }

    pub(crate) fn show_objective_on_map(&mut self, quest: FormId, objective: i32) -> bool {
        let Some((world, _)) = self.map_world() else {
            return false;
        };
        if !self.map_available() {
            return false;
        }
        let Some(at) = self.map_objective_target(world, quest, objective) else {
            return false;
        };
        self.map_focus = Some((world, at));
        self.menu = Some(crate::items::Menu::Map);
        true
    }

    pub(crate) fn apply_map_focus(&mut self) {
        if let Some((world, at)) = self.map_focus.take()
            && let Some(view) = &mut self.map_view
            && world == view.world
        {
            view.center = at;
            view.zoom = 1.0;
        }
    }

    /// Builds the map view. None when the worldspace has no map data or
    /// distant LOD.
    pub fn open_map_view(&mut self) -> Option<()> {
        let (world, data) = self.map_world()?;
        let edid = self.lo.get(world)?.editor_id()?;
        let lod = Lod::new(&self.vfs, &edid);
        if lod.is_empty() {
            log::warn!("map: no distant LOD for {edid}");
            return None;
        }
        let player = match self.location {
            Location::Exterior { world: here, .. } => {
                world_map_point(&self.lo, here, self.player.position.truncate())
                    .filter(|(owner, _)| *owner == world)
                    .map(|(_, p)| (p, self.camera.yaw.to_degrees().rem_euclid(360.0)))
            }
            Location::Interior(cell) => self
                .world_exit_for_map(cell, world)
                .map(|p| (p, self.camera.yaw.to_degrees().rem_euclid(360.0))),
            Location::Nowhere => None,
        };
        let [min_x, min_y, max_x, max_y] = data.bounds();
        // The camera's field of view takes in the whole map's height from
        // the highest it goes.
        let half = (max_y - min_y) / 2.0;
        let fov_y = (2.0 * (half / data.camera.max_height).atan())
            .clamp(40f32.to_radians(), 120f32.to_radians());
        let center = player.map_or(
            Vec2::new((min_x + max_x) / 2.0, (min_y + max_y) / 2.0),
            |(p, _)| p,
        );
        let scene = Scene {
            env: Environment {
                sun_dir: Vec3::new(-0.3, -0.4, 0.85).normalize(),
                sun_color: Vec3::splat(0.7),
                ambient: Vec3::splat(0.55),
                fog_far: 1.0e7,
                clear_color: Vec3::new(0.12, 0.2, 0.28),
                ..Default::default()
            },
            ..Default::default()
        };
        self.map_view = Some(MapView {
            world,
            data,
            player,
            center,
            zoom: 1.0,
            pitch: data.camera.initial_pitch.clamp(MIN_PITCH, MAX_PITCH),
            heading: std::mem::take(&mut self.map_heading),
            fov_y,
            lod,
            scene,
            rows: Vec::new(),
        });
        if let Some((at, zoom, pitch)) = self.map_start.take()
            && let Some(view) = &mut self.map_view
        {
            view.center = at;
            view.zoom = zoom.clamp(0.0, 1.0);
            view.pitch = pitch.clamp(MIN_PITCH, MAX_PITCH);
        }
        self.apply_map_focus();
        self.map_view_rows();
        self.update_map_view();
        Some(())
    }

    pub fn close_map_view(&mut self) {
        // Each opening starts on the player again, as in vanilla (user,
        // 2026-11-01); only `map_start` / `map_heading` (console, a save made
        // with the map open) carry a camera into the next opening.
        self.map_focus = None;
        self.map_view = None;
    }

    /// Visible destination markers, including children that inherit this map.
    pub(crate) fn map_rows_for_world(&self, world: FormId) -> Vec<MapRow> {
        self.map_marker_list()
            .iter()
            .filter(|m| (1..=crate::map_markers::frame::LAST_LOCATION_TYPE).contains(&m.kind))
            .filter(|m| self.map_marker_visible(m.r) && !self.is_disabled(m.r))
            .filter_map(|m| {
                let (owner, pos) = world_map_point(&self.lo, m.world, m.pos.truncate())?;
                (owner == world).then(|| MapRow {
                    marker: Some(m.r),
                    name: m.name.clone(),
                    kind: m.kind,
                    undiscovered: !self.can_fast_travel_to(m.r),
                    pos,
                    rotation: 0.0,
                })
            })
            .collect()
    }

    /// Destination markers, then the player, in stable callback order.
    pub(crate) fn map_view_rows(&mut self) {
        let Some(view) = &self.map_view else { return };
        let world = view.world;
        let mut rows = self.map_rows_for_world(world);
        rows.extend(self.map_quest_rows(world));
        if let Some(marker) = self.player_marker.filter(|m| m.world == world) {
            rows.push(MapRow {
                marker: None,
                name: self
                    .gmst_string("sPlayerSetMarkerName")
                    .unwrap_or_else(|| "Custom Destination".into()),
                kind: PLAYER_SET_ICON,
                undiscovered: false,
                pos: marker.pos.truncate(),
                rotation: 0.0,
            });
        }
        if let Some((pos, rotation)) = self.map_view.as_ref().and_then(|v| v.player) {
            rows.push(MapRow {
                marker: None,
                name: String::new(),
                kind: PLAYER_ICON,
                undiscovered: false,
                pos,
                rotation,
            });
        }
        if let Some(view) = &mut self.map_view {
            view.rows = rows;
        }
    }

    /// Keeps the LOD about the camera loaded and in the map's scene.
    pub fn update_map_view(&mut self) {
        let Some(view) = &mut self.map_view else {
            return;
        };
        let cam = view.camera();
        let want = view.lod.desired_from(cam.position);
        crate::world::lod::stream(
            &self.vfs,
            &mut self.renderer,
            &mut view.lod,
            &want,
            BLOCKS_PER_FRAME,
        );
        if view.lod.dirty {
            view.lod.dirty = false;
            view.scene.lod = view
                .lod
                .instances()
                .map(|i| crate::render::Instance::new(i.model.clone(), i.transform))
                .collect();
        }
    }

    /// The marker rows' screen positions, for `RefreshMarkers`: whether
    /// each shows, its position as fractions of the screen from the top
    /// left, and its rotation.
    pub fn map_marker_screen(&self) -> Vec<(bool, Vec2, f32)> {
        let Some(view) = &self.map_view else {
            return Vec::new();
        };
        let aspect = self.renderer.width as f32 / self.renderer.height as f32;
        view.rows
            .iter()
            .map(|row| match view.screen_of(row.pos, aspect) {
                Some(s) => (
                    (-0.05..=1.05).contains(&s.x) && (-0.05..=1.05).contains(&s.y),
                    s,
                    if row.kind == PLAYER_ICON {
                        row.rotation - view.heading
                    } else {
                        row.rotation
                    },
                ),
                None => (false, Vec2::ZERO, 0.0),
            })
            .collect()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use esp::worldspace::CameraLimits;

    pub(crate) fn view(pitch: f32) -> MapView {
        MapView {
            world: FormId(0x3C),
            data: WorldMapData {
                usable: (0, 0),
                nw: (-30, 15),
                se: (40, -40),
                camera: CameraLimits::default(),
            },
            player: None,
            center: Vec2::new(0.0, 0.0),
            zoom: 1.0,
            pitch,
            heading: 0.0,
            fov_y: 1.6,
            lod: Lod::default(),
            scene: Scene::default(),
            rows: Vec::new(),
        }
    }

    pub(crate) fn hierarchy_fixture(label: &str) -> std::path::PathBuf {
        use esp::group_type as g;
        use esp::writer::{Node, RecordBuilder as R, write_plugin};
        let dir =
            std::env::temp_dir().join(format!("vrm-map-parent-{label}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let world = |id, parent: Option<u32>, scale: f32, x: f32, y: f32| {
            let mut r = R::new(b"WRLD", FormId(id));
            if let Some(parent) = parent {
                r = r
                    .u32(b"WNAM", parent)
                    .field(b"PNAM", &4u16.to_le_bytes())
                    .field(
                        b"ONAM",
                        &[scale, x, y, 0.0]
                            .into_iter()
                            .flat_map(f32::to_le_bytes)
                            .collect::<Vec<_>>(),
                    );
            }
            r
        };
        let worlds = vec![
            world(0x800, None, 1.0, 0.0, 0.0)
                .zstring(b"EDID", "SyntheticMap")
                .field(b"MNAM", &[0; 16]),
            world(0x801, Some(0x800), 2.0, 100.0, 200.0),
            world(0x802, Some(0x801), 0.5, -100.0, -200.0),
            world(0x803, None, 1.0, 0.0, 0.0),
            world(0x804, Some(0x805), 1.0, 0.0, 0.0),
            world(0x805, Some(0x804), 1.0, 0.0, 0.0),
            R::new(b"WRLD", FormId(0x806))
                .u32(b"WNAM", 0x800)
                .field(b"PNAM", &4u16.to_le_bytes())
                .field(b"ONAM", &[0]),
            world(0x807, Some(0x800), f32::NAN, 0.0, 0.0),
            R::new(b"WRLD", FormId(0x808))
                .u32(b"WNAM", 0x800)
                .field(b"PNAM", &4u16.to_le_bytes()),
        ];
        let mut children = Vec::new();
        for (i, r) in worlds.into_iter().enumerate() {
            let w = r.form_id;
            let c = FormId(0xA00 + i as u32);
            let marker = R::new(b"REFR", FormId(0x900 + i as u32))
                .u32(b"NAME", 0x880)
                .field(b"XMRK", &[])
                .field(b"FNAM", &[3])
                .field(b"TNAM", &[1, 0])
                .zstring(b"FULL", "Synthetic map marker")
                .field(
                    b"DATA",
                    &[10.0, 20.0, 0.0, 0.0, 0.0, 0.0]
                        .into_iter()
                        .flat_map(f32::to_le_bytes)
                        .collect::<Vec<_>>(),
                );
            let mut refs = vec![Node::Record(marker)];
            if w == FormId(0x800) {
                refs.push(Node::Record(
                    R::new(b"REFR", FormId(0xC00))
                        .u32(b"NAME", 0x880)
                        .field(
                            b"DATA",
                            &[30.0, 40.0, 0.0, 0.0, 0.0, 0.0]
                                .into_iter()
                                .flat_map(f32::to_le_bytes)
                                .collect::<Vec<_>>(),
                        )
                        .field(b"XTEL", &0xC01u32.to_le_bytes()),
                ));
            }
            children.push(Node::Record(r));
            children.push(Node::Group {
                label: w.0.to_le_bytes(),
                group_type: g::WORLD_CHILDREN,
                children: vec![
                    Node::Record(R::new(b"CELL", c).flags(0x400).field(b"DATA", &[0, 0])),
                    Node::Group {
                        label: c.0.to_le_bytes(),
                        group_type: g::CELL_CHILDREN,
                        children: vec![Node::Group {
                            label: c.0.to_le_bytes(),
                            group_type: g::CELL_PERSISTENT,
                            children: refs,
                        }],
                    },
                ],
            });
        }
        let mut false_condition = [0u8; 32];
        false_condition[4..8].copy_from_slice(&1.0f32.to_le_bytes());
        false_condition[8..10].copy_from_slice(&72u16.to_le_bytes());
        false_condition[12..16].copy_from_slice(&0xDEADu32.to_le_bytes());
        let quests = (0..4)
            .map(|i| {
                let r = R::new(b"QUST", FormId(0xB00 + i))
                    .field(b"DNAM", &[0, 0, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0])
                    .zstring(b"FULL", &format!("Synthetic quest {i}"))
                    .field(b"QOBJ", &10u16.to_le_bytes())
                    .field(b"QSTA", &[0; 8]);
                if i == 2 {
                    r.field(b"CTDA", &false_condition)
                } else {
                    r
                }
            })
            .collect();
        let nodes = vec![
            Node::top(b"QUST", quests),
            Node::Group {
                label: *b"CELL",
                group_type: g::TOP,
                children: vec![
                    Node::Record(R::new(b"CELL", FormId(0xD00)).field(b"DATA", &[1, 0])),
                    Node::Group {
                        label: 0xD00u32.to_le_bytes(),
                        group_type: g::CELL_CHILDREN,
                        children: vec![Node::Group {
                            label: 0xD00u32.to_le_bytes(),
                            group_type: g::CELL_TEMPORARY,
                            children: vec![
                                Node::Record(
                                    R::new(b"REFR", FormId(0xC01))
                                        .u32(b"NAME", 0x880)
                                        .field(b"DATA", &[0; 24])
                                        .field(b"XTEL", &0xC00u32.to_le_bytes()),
                                ),
                                Node::Record(
                                    R::new(b"REFR", FormId(0xC02))
                                        .u32(b"NAME", 0x880)
                                        .field(b"DATA", &[0; 24]),
                                ),
                            ],
                        }],
                    },
                ],
            },
            Node::top(b"STAT", vec![R::new(b"STAT", FormId(0x880))]),
            Node::Group {
                label: *b"WRLD",
                group_type: g::TOP,
                children,
            },
        ];
        std::fs::write(
            dir.join("Map.esp"),
            write_plugin(0, &[], "test", "synthetic hierarchy", 0xE00, &nodes),
        )
        .unwrap();
        dir
    }

    #[test]
    fn map_parent_transforms_compose_and_reject_bad_hierarchies() {
        let dir = hierarchy_fixture("projection");
        let lo = esp::LoadOrder::load(&dir, &["Map.esp".into()]).unwrap();
        let p = Vec2::new(10.0, 20.0);
        assert_eq!(
            world_map_point(&lo, FormId(0x801), p),
            Some((FormId(0x800), Vec2::new(120.0, 240.0)))
        );
        assert_eq!(
            world_map_point(&lo, FormId(0x802), p),
            Some((FormId(0x800), Vec2::new(-90.0, -180.0)))
        );
        assert_eq!(
            world_map_point(&lo, FormId(0x803), p),
            Some((FormId(0x803), p))
        );
        assert_eq!(
            world_map_point(&lo, FormId(0x808), p),
            Some((FormId(0x800), p))
        );
        for id in [0x804, 0x805, 0x806, 0x807, 0xDEAD] {
            assert_eq!(world_map_point(&lo, FormId(id), p), None);
        }
        assert_eq!(
            world_map_point(&lo, FormId(0x800), Vec2::splat(f32::NAN)),
            None
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    #[ignore = "requires a headless GPU; synthetic records only"]
    fn map_parent_rows_keep_destination_ids_and_permissions() {
        let dir = hierarchy_fixture("rows");
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Map.esp");
        let rows = e.map_rows_for_world(FormId(0x800));
        assert_eq!(rows.len(), 4);
        let child = rows
            .iter()
            .find(|r| r.marker == Some(FormId(0x901)))
            .unwrap();
        assert_eq!(child.pos, Vec2::new(120.0, 240.0));
        assert!(!child.undiscovered);
        assert!(!rows.iter().any(|r| r.marker == Some(FormId(0x903))));
        e.enable_fast_travel(FormId(0x901), false);
        let rows = e.map_rows_for_world(FormId(0x800));
        assert!(
            rows.iter()
                .find(|r| r.marker == Some(FormId(0x901)))
                .unwrap()
                .undiscovered
        );
        e.scripts.disabled.insert(FormId(0x902), true);
        assert!(
            !e.map_rows_for_world(FormId(0x800))
                .iter()
                .any(|r| r.marker == Some(FormId(0x902)))
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    #[ignore = "requires a headless GPU; synthetic records only"]
    fn map_quest_overlays_share_compass_filters_and_have_no_travel_identity() {
        let dir = hierarchy_fixture("quests");
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Map.esp");
        for (q, target) in [
            (0xB00, 0x901),
            (0xB01, 0xC02),
            (0xB02, 0x902),
            (0xB03, 0x901),
        ] {
            let st = e.scripts.quests.entry(FormId(q)).or_default();
            st.running = true;
            st.active = true;
            st.objectives_displayed.insert(10);
            st.aliases.insert(0, FormId(target));
        }
        e.location = Location::Exterior {
            world: FormId(0x800),
            center: (0, 0),
        };
        assert_eq!(
            e.map_objective_target(FormId(0x800), FormId(0xB00), 10),
            Some(Vec2::new(120.0, 240.0))
        );
        assert_eq!(
            e.map_objective_target(FormId(0x800), FormId(0xB01), 10),
            Some(Vec2::new(30.0, 40.0))
        );
        assert_eq!(
            e.map_objective_target(FormId(0x800), FormId(0xB02), 10),
            None
        ); // target condition fails
        let rows = e.map_quest_rows(FormId(0x800));
        assert_eq!(rows.len(), 2); // duplicate target and false condition are excluded
        let direct = rows.iter().find(|r| r.kind == 62).unwrap();
        assert_eq!(direct.pos, Vec2::new(120.0, 240.0));
        assert_eq!(direct.name, "Synthetic quest 0");
        let door = rows.iter().find(|r| r.kind == 61).unwrap();
        assert_eq!(door.pos, Vec2::new(30.0, 40.0));
        for i in 0..rows.len() {
            assert_eq!(row_action(&rows, i as f64), RowAction::None);
        }
        e.update_quest_targets(1.0);
        assert_eq!(e.quest_targets.spots.len(), 1);
        assert!(e.quest_targets.spots[0].door);
        assert_eq!(e.quest_targets.spots[0].pos.truncate(), door.pos);
        for id in [0xB00, 0xB03] {
            e.scripts
                .quests
                .get_mut(&FormId(id))
                .unwrap()
                .objectives_completed
                .insert(10);
        }
        assert_eq!(e.map_quest_rows(FormId(0x800)).len(), 1);
        e.scripts.disabled.insert(FormId(0xC02), true);
        assert!(e.map_quest_rows(FormId(0x800)).is_empty());
        e.scripts.disabled.clear();
        e.scripts.quests.get_mut(&FormId(0xB01)).unwrap().active = false;
        assert!(e.map_quest_rows(FormId(0x800)).is_empty());
        // Untracking hides map overlays but must not disable Show on Map.
        assert_eq!(
            e.map_objective_target(FormId(0x800), FormId(0xB01), 10),
            Some(Vec2::new(30.0, 40.0))
        );
        assert_eq!(
            e.map_objective_target(FormId(0x800), FormId(0xB00), 10),
            None
        ); // completed
        let st = e.scripts.quests.get_mut(&FormId(0xB01)).unwrap();
        st.active = true;
        st.failed = true;
        assert!(e.map_quest_rows(FormId(0x800)).is_empty());
        assert_eq!(
            e.map_objective_target(FormId(0x800), FormId(0xB01), 10),
            None
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    #[ignore = "requires a headless GPU; synthetic records only"]
    fn custom_destination_places_moves_leaves_removes_and_survives_save() {
        let dir = hierarchy_fixture("custom-destination");
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Map.esp");
        let world = FormId(0x800);
        let first = Vec2::new(10.0, 20.0);
        let next = Vec2::new(300.0, 400.0);
        e.request_player_marker(world, first);
        assert!(e.messages.boxes.is_empty());
        assert_eq!(e.player_marker.unwrap().pos.truncate(), first);
        e.request_player_marker(world, next);
        let question = e.messages.boxes.front().unwrap();
        assert_eq!(
            question
                .buttons
                .iter()
                .map(|(_, s)| s.as_str())
                .collect::<Vec<_>>(),
            ["Move It", "Leave It", "Remove It"]
        );
        assert_eq!(e.player_marker.unwrap().pos.truncate(), first);
        // Preserve the captured click even when the question is saved and loaded.
        let file = crate::save::state::Snapshot::capture(&e).write();
        crate::save::state::Snapshot::read(&file)
            .unwrap()
            .apply(&mut e)
            .unwrap();
        e.choose_message_button(0);
        assert_eq!(e.player_marker.unwrap().pos.truncate(), next);
        assert!(e.messages.boxes.is_empty());
        e.request_player_marker(world, first);
        e.choose_message_button(1);
        assert_eq!(e.player_marker.unwrap().pos.truncate(), next);
        e.request_player_marker(world, first);
        e.choose_message_button(2);
        assert_eq!(e.player_marker, None);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    #[ignore = "requires a headless GPU; synthetic records only"]
    fn undiscovered_location_offers_placement_before_combat_refusal() {
        let dir = hierarchy_fixture("undiscovered-placement");
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Map.esp");
        let marker = FormId(0x901);
        // Discovered, but a script denied travel: no Place Marker question,
        // the travel refusal as before.
        e.map_markers.discovered.insert(marker);
        e.enable_fast_travel(marker, false);
        e.request_map_travel(marker);
        assert!(e.messages.boxes.is_empty());
        assert_eq!(e.player_marker, None);
        e.map_markers.discovered.remove(&marker);
        e.map_markers.denied.remove(&marker);
        e.clear_marker_travel_flag(marker);
        e.fast_travel_disabled = true; // restriction must not suppress the question
        e.request_map_travel(marker);
        assert!(e.messages.boxes[0].text.contains("Place marker?"));
        e.choose_message_button(1);
        assert_eq!(e.player_marker, None);
        e.request_map_travel(marker);
        e.choose_message_button(0);
        let custom = e.player_marker.unwrap();
        assert_eq!(custom.world, FormId(0x800));
        assert_eq!(custom.pos.truncate(), Vec2::new(120.0, 240.0));
        assert!(e.messages.boxes.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    #[ignore = "requires a headless GPU; synthetic records only"]
    fn custom_compass_uses_map_owner_and_stays_when_reached() {
        let dir = hierarchy_fixture("custom-compass");
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Map.esp");
        e.location = Location::Exterior {
            world: FormId(0x801),
            center: (0, 0),
        };
        e.player.position = Vec3::new(10.0, 20.0, 0.0); // map (120,240)
        e.place_player_marker(FormId(0x800), Vec2::new(120.0, 100000.0));
        let custom = || crate::map_markers::frame::PLAYER_SET;
        let compass = e.compass_markers();
        let marker = compass.iter().find(|m| m.frame == custom()).unwrap();
        assert_eq!(
            (marker.heading, marker.alpha, marker.scale),
            (0.0, 100.0, 100.0)
        );
        e.place_player_marker(FormId(0x800), Vec2::new(120.0, 240.0));
        e.update_map_markers(1.0);
        assert!(e.player_marker.is_some());
        assert!(e.compass_markers().iter().any(|m| m.frame == custom()));
        e.location = Location::Interior(FormId(0xD00));
        assert!(!e.compass_markers().iter().any(|m| m.frame == custom()));
        e.location = Location::Exterior {
            world: FormId(0x803),
            center: (0, 0),
        };
        assert!(!e.compass_markers().iter().any(|m| m.frame == custom()));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    #[ignore = "requires a headless GPU; synthetic records only"]
    fn show_objective_focus_is_transient_and_resolves_untracked_targets() {
        let dir = hierarchy_fixture("objective-focus");
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Map.esp");
        e.location = Location::Exterior {
            world: FormId(0x800),
            center: (0, 0),
        };
        let st = e.scripts.quests.entry(FormId(0xB00)).or_default();
        st.running = true;
        st.objectives_displayed.insert(10);
        st.aliases.insert(0, FormId(0x901));
        let mut map = view(50.0);
        map.world = FormId(0x800);
        map.heading = 80.0;
        e.map_view = Some(map);
        let saved = Some((Vec2::new(1000.0, -2000.0), 0.3, 70.0));
        e.map_start = saved;
        assert!(e.show_objective_on_map(FormId(0xB00), 10));
        assert_eq!(e.menu, Some(crate::items::Menu::Map));
        e.apply_map_focus();
        let view = e.map_view.as_ref().unwrap();
        assert_eq!(view.center, Vec2::new(120.0, 240.0));
        assert_eq!((view.zoom, view.heading), (1.0, 80.0));
        assert_eq!(e.map_start, saved);
        assert!(e.map_focus.is_none());
        // Closing keeps no camera: the next opening starts on the player.
        e.map_start = None;
        e.close_map_view();
        assert!(e.map_view.is_none());
        assert_eq!((e.map_start, e.map_heading), (None, 0.0));
        e.scripts
            .quests
            .get_mut(&FormId(0xB00))
            .unwrap()
            .objectives_completed
            .insert(10);
        assert!(!e.show_objective_on_map(FormId(0xB00), 10));
        assert!(!e.show_objective_on_map(FormId(0xDEAD), 10));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn map_callback_indices_keep_stable_destination_identity() {
        let rows = vec![
            MapRow {
                marker: Some(FormId(0x900)),
                name: "A".into(),
                kind: 1,
                undiscovered: false,
                pos: Vec2::ZERO,
                rotation: 0.0,
            },
            MapRow {
                marker: None,
                name: "Player".into(),
                kind: PLAYER_ICON,
                undiscovered: false,
                pos: Vec2::ZERO,
                rotation: 0.0,
            },
        ];
        assert_eq!(row_action(&rows, 0.0), RowAction::Travel(FormId(0x900)));
        let mut custom = rows[1].clone();
        custom.kind = PLAYER_SET_ICON;
        assert_eq!(row_action(&[custom], 0.0), RowAction::PlayerMarker);
        for index in [-1.0, 0.5, 1.0, 2.0, f64::NAN, f64::INFINITY, f64::MAX] {
            assert_eq!(row_action(&rows, index), RowAction::None);
        }
    }

    #[test]
    fn the_centre_is_the_middle_of_the_screen() {
        for pitch in [50.0, 89.0] {
            let v = view(pitch);
            let s = v.screen_of(v.center, 16.0 / 9.0).unwrap();
            assert!((s - Vec2::splat(0.5)).length() < 1e-3, "{pitch}: {s}");
        }
    }

    #[test]
    fn north_is_up_and_east_is_right() {
        let v = view(89.0);
        let n = v.screen_of(Vec2::new(0.0, 20000.0), 1.0).unwrap();
        let e = v.screen_of(Vec2::new(20000.0, 0.0), 1.0).unwrap();
        assert!(n.y < 0.5 && (n.x - 0.5).abs() < 1e-3);
        assert!(e.x > 0.5 && (e.y - 0.5).abs() < 1e-3);
    }

    #[test]
    fn panning_stays_on_the_map() {
        let mut v = view(50.0);
        for _ in 0..100 {
            v.pan(1.0, 1.0);
        }
        let [_, _, max_x, max_y] = v.data.bounds();
        assert_eq!(v.center, Vec2::new(max_x, max_y));
        let before = v.zoom;
        v.zoom_by(5.0);
        assert_eq!(v.zoom, 1.0);
        v.zoom_by(-5.0);
        assert_eq!(v.zoom, 0.0);
        assert_ne!(before, v.zoom);
    }

    #[test]
    fn nearest_marker_within_reach() {
        let mut v = view(89.0);
        v.rows = vec![
            MapRow {
                marker: Some(FormId(0x900)),
                name: "A".into(),
                kind: 1,
                undiscovered: false,
                pos: Vec2::new(1000.0, 0.0),
                rotation: 0.0,
            },
            MapRow {
                marker: None,
                name: String::new(),
                kind: PLAYER_ICON,
                undiscovered: false,
                pos: Vec2::ZERO,
                rotation: 0.0,
            },
        ];
        let a = v.screen_of(Vec2::new(1000.0, 0.0), 1.0).unwrap();
        assert_eq!(v.marker_at(a, 1.0, 0.05), Some(0));
        // The player's marker isn't picked.
        assert_eq!(v.marker_at(Vec2::splat(0.5), 1.0, 0.0001), None);
        assert_eq!(v.marker_at(Vec2::new(0.9, 0.9), 1.0, 0.05), None);
    }
}

#[cfg(test)]
mod inverse_tests {
    use super::*;
    #[test]
    fn turning_wraps_into_a_saveable_heading() {
        let mut v = super::tests::view(50.0);
        v.heading = 0.5;
        // The sum is a tiny negative number, which `rem_euclid` rounds to 360.
        v.turn_by(-0.50000006);
        assert!((0.0..360.0).contains(&v.heading), "{}", v.heading);
        v.turn_by(-90.0);
        assert!((v.heading - 270.0).abs() < 1e-3);
        v.turn_by(450.0);
        assert!(v.heading.abs() < 1e-3);
    }
    #[test]
    fn ground_projection_round_trips_tilt_turn_and_aspect() {
        for pitch in [25.0, 50.0, 89.0] {
            let mut v = super::tests::view(pitch);
            for heading in [0.0, 80.0, 200.0] {
                v.heading = heading;
                for aspect in [1.0, 16.0 / 9.0] {
                    for p in [
                        Vec2::ZERO,
                        Vec2::new(1200.0, -3400.0),
                        Vec2::new(-5000.0, 2000.0),
                    ] {
                        let screen = v.screen_of(p, aspect).unwrap();
                        let actual = v.ground_at(screen, aspect).unwrap();
                        assert!(
                            actual.distance(p) < 0.1,
                            "pitch={pitch}, heading={heading}: {actual} vs {p}"
                        );
                    }
                }
            }
            assert_eq!(v.ground_at(Vec2::splat(f32::NAN), 1.0), None);
            assert_eq!(v.ground_at(Vec2::ZERO, 0.0), None);
        }
    }
}
