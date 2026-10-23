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
pub(crate) fn row_destination(rows: &[MapRow], index: f64) -> Option<FormId> {
    if !index.is_finite() || index < 0.0 || index.fract() != 0.0 {
        return None;
    }
    rows.get(index as usize)?.marker
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

/// The world map's own scene and camera while it is up.
pub fn frame_parts(
    map: &Option<MapView>,
    menu: Option<crate::items::Menu>,
) -> Option<(&Scene, Camera)> {
    match map {
        Some(v) if menu == Some(crate::items::Menu::Map) => Some((&v.scene, v.camera())),
        _ => None,
    }
}

impl Engine {
    /// The worldspace the map opens on, with its map data: where the player
    /// is (a child worldspace's is its parent's), else Tamriel.
    fn map_world(&self) -> Option<(FormId, WorldMapData)> {
        let here = match self.location {
            Location::Exterior { world, .. } => world,
            _ => self.lo.find_editor_id("Tamriel")?,
        };
        let (owner, _) = world_map_point(&self.lo, here, Vec2::ZERO)?;
        let rec = self.lo.get(owner)?;
        Some((owner, WorldMapData::parse(rec.get(b"MNAM")?)?))
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
            _ => None,
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
    fn map_view_rows(&mut self) {
        let Some(view) = &self.map_view else { return };
        let world = view.world;
        let mut rows = self.map_rows_for_world(world);
        rows.extend(self.map_quest_rows(world));
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

    fn hierarchy_fixture(label: &str) -> std::path::PathBuf {
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
            world(0x800, None, 1.0, 0.0, 0.0),
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
        let rows = e.map_quest_rows(FormId(0x800));
        assert_eq!(rows.len(), 2); // duplicate target and false condition are excluded
        let direct = rows.iter().find(|r| r.kind == 62).unwrap();
        assert_eq!(direct.pos, Vec2::new(120.0, 240.0));
        assert_eq!(direct.name, "Synthetic quest 0");
        let door = rows.iter().find(|r| r.kind == 61).unwrap();
        assert_eq!(door.pos, Vec2::new(30.0, 40.0));
        for i in 0..rows.len() {
            assert_eq!(row_destination(&rows, i as f64), None);
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
        let st = e.scripts.quests.get_mut(&FormId(0xB01)).unwrap();
        st.active = true;
        st.failed = true;
        assert!(e.map_quest_rows(FormId(0x800)).is_empty());
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
        assert_eq!(row_destination(&rows, 0.0), Some(FormId(0x900)));
        for index in [-1.0, 0.5, 1.0, 2.0, f64::NAN, f64::INFINITY, f64::MAX] {
            assert_eq!(row_destination(&rows, index), None);
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
