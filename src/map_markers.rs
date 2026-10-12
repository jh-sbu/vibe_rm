//! Map markers: the references the data marks as places on the map (`XMRK`,
//! with their name `FULL`, type `TNAM` and flags `FNAM`). A marker is on the
//! map (visible) when the data says so from the start, when a script adds it
//! (`AddToMap`) or when the player comes near; it is discovered when the
//! player reaches it, which the HUD announces ("Riverwood DISCOVERED"), the
//! discovery music plays for and the Locations Discovered statistic counts.
//! The compass shows the discovered markers near the player and, hollow, the
//! undiscovered nearer still (`compass_markers`).
//! Choices: `known_ambiguities/map-markers.md`.

use std::collections::HashSet;

use esp::FormId;
use glam::Vec3;

use crate::engine::{Engine, Location};

/// `FNAM`: on the map from the start.
const VISIBLE: u8 = 0x01;

/// Seconds between checks of the player's distance to the markers.
const CHECK_INTERVAL: f32 = 0.5;

/// The compass marker clip's frames (`hudmenu.swf`, "Compass Marker"): the
/// quest targets', the enemies' (the player's own marker's, 3, isn't used:
/// there's no map to set it on), then the location markers' (a frame per map
/// marker type, in `TNAM` order from type 1 at `LocationMarkers`: the empty
/// marker has none) and the undiscovered ones' (likewise).
pub mod frame {
    pub const QUEST: f32 = 1.0;
    pub const QUEST_DOOR: f32 = 2.0;
    pub const ENEMY: f32 = 4.0;
    pub const LOCATIONS: f32 = 5.0;
    pub const UNDISCOVERED: f32 = 72.0;
    /// The last map marker type the clip has a frame for (the map's
    /// `IconTypes`, from `CityMarker`, 1, to `DLC02CastleKarstaagMarker`).
    pub const LAST_LOCATION_TYPE: u8 = 60;
}

/// A marker the data places.
#[derive(Debug, Clone)]
pub struct MapMarker {
    pub r: FormId,
    pub world: FormId,
    pub pos: Vec3,
    pub name: String,
    /// `TNAM`: the icon.
    pub kind: u8,
    flags: u8,
}

#[derive(Default)]
pub struct MapMarkers {
    /// Every marker in the load order, read on first use.
    all: std::cell::OnceCell<Vec<MapMarker>>,
    /// Markers put on the map by scripts or by coming near.
    shown: HashSet<FormId>,
    /// Markers the player has reached (or a script allowed travel to).
    pub discovered: HashSet<FormId>,
    next_check: f32,
}

/// A marker on the compass: heading (degrees clockwise from north), alpha
/// (0 to 100), the marker clip's frame and its scale (percent).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompassMarker {
    pub heading: f32,
    pub alpha: f32,
    pub frame: f32,
    pub scale: f32,
}

/// The heading from `from` to `to`, in degrees clockwise from north, to a
/// tenth of a degree (so the compass isn't sent the same markers anew).
pub fn heading(from: Vec3, to: Vec3) -> f32 {
    let d = to - from;
    (d.x.atan2(d.y).to_degrees().rem_euclid(360.0) * 10.0).round() / 10.0
}

fn read_markers(e: &Engine) -> Vec<MapMarker> {
    let mut out = Vec::new();
    for &r in e.lo.ids_of_type(b"REFR") {
        let Some(rec) = e.lo.get(r) else { continue };
        if rec.get(b"XMRK").is_none() {
            continue;
        }
        let Some(world) =
            e.lo.cell_of_ref(r)
                .and_then(|c| e.lo.cell(c))
                .and_then(|c| c.world)
        else {
            continue;
        };
        let name = rec
            .get(b"FULL")
            .map(|d| e.lo.lstring(&rec, d))
            .unwrap_or_default();
        out.push(MapMarker {
            r,
            world,
            pos: crate::world::records::reference(&rec).position,
            name,
            kind: rec
                .get(b"TNAM")
                .and_then(|d| d.first().copied())
                .unwrap_or(0),
            flags: rec
                .get(b"FNAM")
                .and_then(|d| d.first().copied())
                .unwrap_or(0),
        });
    }
    log::info!("{} map markers", out.len());
    out
}

impl Engine {
    fn map_marker_list(&self) -> &[MapMarker] {
        self.map_markers.all.get_or_init(|| read_markers(self))
    }

    fn map_marker(&self, r: FormId) -> Option<&MapMarker> {
        self.map_marker_list().iter().find(|m| m.r == r)
    }

    /// Whether the marker is on the map (`IsMapMarkerVisible`,
    /// `GetMapMarkerVisible`).
    pub fn map_marker_visible(&self, r: FormId) -> bool {
        let Some(m) = self.map_marker(r) else {
            return false;
        };
        m.flags & VISIBLE != 0
            || self.map_markers.shown.contains(&r)
            || self.map_markers.discovered.contains(&r)
    }

    /// `AddToMap(abAllowFastTravel)`: the marker goes on the map ("Map
    /// updated." when it wasn't), travel to it allowed when asked.
    pub fn add_to_map(&mut self, r: FormId, travel: bool) {
        if self.map_marker(r).is_none() {
            return;
        }
        if !self.map_marker_visible(r) {
            let text = self
                .gmst_string("sMapMarkerAdded")
                .unwrap_or_else(|| "Map updated.".into());
            self.scripts.notify(text);
        }
        self.map_markers.shown.insert(r);
        if travel {
            self.map_markers.discovered.insert(r);
        }
    }

    /// `EnableFastTravel(abEnable)`.
    pub fn enable_fast_travel(&mut self, r: FormId, on: bool) {
        if self.map_marker(r).is_none() {
            return;
        }
        if on {
            self.map_markers.discovered.insert(r);
        } else {
            self.map_markers.discovered.remove(&r);
        }
    }

    /// Twice a second: the enabled markers in the player's worldspace come
    /// onto the map within `iMapMarkerVisibleDistance` and are discovered
    /// within `iMapMarkerRevealDistance`.
    pub(crate) fn update_map_markers(&mut self, dt: f32) {
        self.map_markers.next_check -= dt;
        if self.map_markers.next_check > 0.0 {
            return;
        }
        self.map_markers.next_check = CHECK_INTERVAL;
        let Location::Exterior { world, .. } = self.location else {
            return;
        };
        let (visible, reveal) = self.marker_distances();
        let player = self.player.position;
        let near: Vec<MapMarker> = self
            .map_marker_list()
            .iter()
            .filter(|m| m.world == world && m.pos.distance(player) < visible)
            .cloned()
            .collect();
        for m in near {
            if self.is_disabled(m.r) {
                continue;
            }
            self.map_markers.shown.insert(m.r);
            if m.pos.distance(player) < reveal && !self.map_markers.discovered.contains(&m.r) {
                self.discover(&m);
            }
        }
    }

    fn marker_distances(&self) -> (f32, f32) {
        let lo = &self.lo;
        (
            crate::ai::combat::gmst_i32(lo, "iMapMarkerVisibleDistance", 12500) as f32,
            crate::ai::combat::gmst_i32(lo, "iMapMarkerRevealDistance", 1000) as f32,
        )
    }

    fn discover(&mut self, m: &MapMarker) {
        self.map_markers.discovered.insert(m.r);
        log::info!("discovered {} ({})", m.name, m.r);
        if m.name.is_empty() {
            return;
        }
        let status = self
            .gmst_string("sDiscoveredText")
            .unwrap_or_else(|| "discovered".into());
        self.banner(m.name.clone(), status);
        self.increment_stat("Locations Discovered", 1);
        self.play_discovery_music(m.kind);
    }

    /// The music for discovering a place of a marker type.
    fn play_discovery_music(&mut self, kind: u8) {
        let edid = match kind {
            // Cities and their castles.
            1 | 35..=52 => "MUSDiscoveryCity",
            2 | 3 => "MUSDiscoveryTown",
            // Caves, forts, ruins, mines, lairs, strongholds, giant camps,
            // towers.
            4 | 6..=9 | 12 | 15 | 22 | 28 | 31 => "MUSDiscoveryDungeon",
            _ => "MUSDiscoveryGeneric",
        };
        if let Some(music) = self.lo.find_editor_id(edid) {
            self.play_music_once(music);
        }
    }

    /// Everything on the compass: the location markers, the quest targets
    /// and the enemies fighting the player.
    pub fn compass_markers(&self) -> Vec<CompassMarker> {
        let mut out = self.compass_location_markers();
        out.extend(self.compass_quest_markers());
        let player = self.player.position;
        for a in self.cells.values().flat_map(|rt| &rt.actors) {
            let fighting = a
                .combat
                .as_ref()
                .is_some_and(|c| c.target == crate::engine::PLAYER_REF && !c.fleeing);
            if fighting && !a.dead && a.bleeding.is_none() {
                out.push(CompassMarker {
                    heading: heading(player, a.pos),
                    alpha: 100.0,
                    frame: frame::ENEMY,
                    scale: 100.0,
                });
            }
        }
        out
    }

    /// The location markers on the compass: those discovered within
    /// `fHUDCompassLocationMaxDist`, and the undiscovered (hollow) within
    /// `iMapMarkerVisibleDistance`, fading in from there to
    /// `iMapMarkerFadeStartDistance`. None indoors.
    fn compass_location_markers(&self) -> Vec<CompassMarker> {
        let Location::Exterior { world, .. } = self.location else {
            return Vec::new();
        };
        let lo = &self.lo;
        let max = crate::ai::combat::gmst_f32(lo, "fHUDCompassLocationMaxDist", 20000.0);
        let fade = crate::ai::combat::gmst_i32(lo, "iMapMarkerFadeStartDistance", 10000) as f32;
        let (visible, _) = self.marker_distances();
        let player = self.player.position;
        let near: Vec<MapMarker> = self
            .map_marker_list()
            .iter()
            .filter(|m| m.world == world && (1..=frame::LAST_LOCATION_TYPE).contains(&m.kind))
            .filter(|m| m.pos.distance(player) < max)
            .cloned()
            .collect();
        let mut out = Vec::new();
        for m in near {
            if self.is_disabled(m.r) {
                continue;
            }
            let d = m.pos.distance(player);
            let (base, alpha) = if self.map_markers.discovered.contains(&m.r) {
                (frame::LOCATIONS, 100.0)
            } else if d < visible {
                let a = ((visible - d) / (visible - fade).max(1.0)).clamp(0.0, 1.0);
                (frame::UNDISCOVERED, (100.0 * a).round())
            } else {
                continue;
            };
            out.push(CompassMarker {
                heading: heading(player, m.pos),
                alpha,
                frame: base + (m.kind - 1) as f32,
                scale: 100.0,
            });
        }
        out
    }
}
