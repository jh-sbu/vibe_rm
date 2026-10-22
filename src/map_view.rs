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
use crate::map_markers::MapMarker;
use crate::render::{Camera, Environment, Scene};
use crate::world::lod::Lod;

/// `WRLD` `PNAM`'s Use Map Data.
const USE_MAP_DATA: u16 = 0x04;
/// `map.swf`'s icon type of the player's own marker (`IconTypes`' last).
pub const PLAYER_ICON: u8 = 66;
/// The lowest the map's camera tilts, and the highest (nearly straight down:
/// exactly down has no up),
/// degrees above the horizon.
const MIN_PITCH: f32 = 25.0;
const MAX_PITCH: f32 = 89.0;
/// Blocks the map loads a frame.
const BLOCKS_PER_FRAME: usize = 8;

/// A marker on the map, in the order `map.swf` was told them.
#[derive(Debug, Clone)]
pub struct MapRow {
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
    fov_y: f32,
    lod: Lod,
    pub scene: Scene,
    pub rows: Vec<MapRow>,
}

impl MapView {
    fn camera(&self) -> Camera {
        let cam = &self.data.camera;
        let height = cam.min_height + (cam.max_height - cam.min_height) * self.zoom;
        let p = self.pitch.to_radians();
        // Looking north, tilted down.
        let forward = Vec3::new(0.0, p.cos(), -p.sin());
        let target = self.center.extend(0.0);
        Camera {
            position: target - forward * (height / p.sin()),
            yaw: 0.0,
            pitch: -p,
            fov_y: self.fov_y,
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
        self.center.x += right * scale;
        self.center.y += up * scale / self.pitch.to_radians().sin();
        let [min_x, min_y, max_x, max_y] = self.data.bounds();
        self.center = self
            .center
            .clamp(Vec2::new(min_x, min_y), Vec2::new(max_x, max_y));
    }

    pub fn zoom_by(&mut self, by: f32) {
        self.zoom = (self.zoom + by).clamp(0.0, 1.0);
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

/// What a frame draws: the world and the player's view, or the world map's
/// own scene and camera while it is up (by parts: the renderer is borrowed
/// too).
pub fn frame_parts<'a>(
    map: &'a Option<MapView>,
    menu: Option<crate::items::Menu>,
    scene: &'a Scene,
    camera: Camera,
) -> (&'a Scene, Camera) {
    match map {
        Some(v) if menu == Some(crate::items::Menu::Map) => (&v.scene, v.camera()),
        _ => (scene, camera),
    }
}

impl Engine {
    /// The worldspace the map opens on, with its map data: where the player
    /// is (a child worldspace's is its parent's), else Tamriel.
    fn map_world(&self) -> Option<(FormId, WorldMapData, Option<WorldMapOffset>)> {
        let here = match self.location {
            Location::Exterior { world, .. } => world,
            _ => self.lo.find_editor_id("Tamriel")?,
        };
        let (mnam, owner) = self.world_field(here, b"MNAM", USE_MAP_DATA)?;
        let data = WorldMapData::parse(&mnam)?;
        let offset = (owner != here)
            .then(|| {
                self.lo
                    .get(here)?
                    .get(b"ONAM")
                    .and_then(WorldMapOffset::parse)
            })
            .flatten();
        Some((owner, data, offset))
    }

    /// Builds the map view. None when the worldspace has no map data or
    /// distant LOD.
    pub fn open_map_view(&mut self) -> Option<()> {
        let (world, data, offset) = self.map_world()?;
        let edid = self.lo.get(world)?.editor_id()?;
        let lod = Lod::new(&self.vfs, &edid);
        if lod.is_empty() {
            log::warn!("map: no distant LOD for {edid}");
            return None;
        }
        let in_world = match self.location {
            Location::Exterior { world: here, .. } => here == world || offset.is_some(),
            _ => false,
        };
        let player = in_world.then(|| {
            let p = self.player.position.truncate();
            // A child worldspace sits on its parent's map at its offset.
            let p = match offset {
                Some(o) => p * o.scale + Vec2::new(o.offset[0], o.offset[1]),
                None => p,
            };
            (p, self.camera.yaw.to_degrees().rem_euclid(360.0))
        });
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
        self.map_view_rows();
        self.update_map_view();
        Some(())
    }

    pub fn close_map_view(&mut self) {
        self.map_view = None;
    }

    /// The markers on the map: every one in the worldspace that is on the
    /// map and enabled, then the player's.
    fn map_view_rows(&mut self) {
        let Some(view) = &self.map_view else { return };
        let world = view.world;
        let markers: Vec<MapMarker> = self
            .map_marker_list()
            .iter()
            .filter(|m| {
                m.world == world
                    && (1..=crate::map_markers::frame::LAST_LOCATION_TYPE).contains(&m.kind)
            })
            .filter(|m| self.map_marker_visible(m.r) && !self.is_disabled(m.r))
            .cloned()
            .collect();
        let mut rows: Vec<MapRow> = markers
            .iter()
            .map(|m| MapRow {
                name: m.name.clone(),
                kind: m.kind,
                undiscovered: !(self.map_markers.discovered.contains(&m.r)
                    || self.can_fast_travel_to(m.r)),
                pos: m.pos.truncate(),
                rotation: 0.0,
            })
            .collect();
        if let Some((pos, rotation)) = self.map_view.as_ref().and_then(|v| v.player) {
            rows.push(MapRow {
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
                    row.rotation,
                ),
                None => (false, Vec2::ZERO, 0.0),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp::worldspace::CameraLimits;

    fn view(pitch: f32) -> MapView {
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
            fov_y: 1.6,
            lod: Lod::default(),
            scene: Scene::default(),
            rows: Vec::new(),
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
                name: "A".into(),
                kind: 1,
                undiscovered: false,
                pos: Vec2::new(1000.0, 0.0),
                rotation: 0.0,
            },
            MapRow {
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
