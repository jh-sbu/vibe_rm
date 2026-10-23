//! The skills menu's sky: the game's skydome (`Interface/INTPerkSkydome.nif`)
//! drawn as the scene behind `statsmenu.swf`, the ring of skills and a skill's
//! perks alike. Its eighteen constellation images (`cImage00` to `cImage17`)
//! and the `Point00` to `Point17` nodes stand around its origin 20 degrees
//! apart, clockwise from +Y seen from above: the ring's skills, in the ring's
//! order. The camera stands at the origin and faces the selected skill's. A
//! skill's tree is its perks' stars (`INTPerkStars01.nif`) and the lines
//! between them (`INTPerkLine01.nif`), each posed by its own sequences as its
//! behaviour graph names them, on a plane facing the camera. The menu draws
//! its own scene, as the map does; the loaded cells aren't in it. The
//! skydome plays its own sequences: `CameraIntro` as the menu opens, rising
//! from 3000 units below its origin (its `CameraPosition` node), then `Idle`'s
//! slow drift of the star layers and scroll of the gases, and `CameraOutro`,
//! sinking again, as it closes.
//! Choices: `known_ambiguities/skills-menu.md`.

use esp::FormId;
use glam::{Mat3, Mat4, Vec3, Vec4};

use crate::engine::Engine;
use crate::render::{Camera, Environment, Instance, Scene};
use crate::world::animated::AnimatedObject;

const DOME: &str = "meshes/interface/intperkskydome.nif#world";
const STAR: &str = "meshes/interface/intperkstars01.nif#world";
const LINE: &str = "meshes/interface/intperkline01.nif#world";
/// The camera's vertical field of view on the ring and in a skill's perks
/// (closer in on the skill's constellation), degrees.
const RING_FOV: f32 = 60.0;
const PERKS_FOV: f32 = 40.0;
/// How far a skill's perks look up, degrees: to the middle of the
/// constellation images, which stand about 15 units over the origin at a
/// radius of about 90.
const PERKS_PITCH: f32 = 9.5;
/// The movie's stage, width over height: a tree's points are fractions of
/// it.
const STAGE_ASPECT: f32 = 16.0 / 9.0;
/// How far from the camera a tree's stars stand, just in front of the
/// constellation images.
const TREE_DISTANCE: f32 = 86.0;
/// A star's scale (its art is about 16 units across, its glow 23).
const STAR_SCALE: f32 = 0.16;
/// The line model's length (`BeamEnd`) and the scale across it (its art is
/// 11 units wide).
const LINE_LENGTH: f32 = 56.2;
const LINE_WIDTH: f32 = 0.12;

/// A perk's star at rest, as the star's behaviour graph has them
/// (`StarBehavior.hkx`'s `LockedState`, `UnlockedState`, `OwnedState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StarState {
    Locked,
    Unlocked,
    Owned,
}

impl StarState {
    /// Its sequence while it rests.
    fn resting(self) -> &'static str {
        match self {
            StarState::Locked => "Locked",
            StarState::Unlocked => "Unlocked",
            StarState::Owned => "Owned",
        }
    }
}

/// A skill's tree as the menu shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeShown {
    pub skill: u32,
    /// The ring's heading of the skill, degrees.
    pub angle: f32,
    /// Each perk's place, as fractions of the stage.
    pub points: Vec<(f32, f32)>,
    pub states: Vec<StarState>,
    /// The lines, as pairs of `points` indices (from, to).
    pub lines: Vec<(usize, usize)>,
    pub selected: Option<usize>,
}

/// A star and what it shows: its rest, whether it is picked, and the
/// sequence to play once the one playing ends.
struct Star {
    obj: AnimatedObject,
    state: StarState,
    selected: bool,
    next: Option<&'static str>,
}

struct Line {
    obj: AnimatedObject,
    from: usize,
    to: usize,
    lit: bool,
    next: Option<&'static str>,
}

struct Tree {
    skill: u32,
    points: Vec<(f32, f32)>,
    stars: Vec<Star>,
    lines: Vec<Line>,
}

/// The skydome: its still shapes (`scene.sky[0]`) and the parts its
/// sequences move (the rest of `scene.sky`).
struct Dome {
    obj: Option<AnimatedObject>,
    /// Where its `CameraPosition` node has the camera, relative to its
    /// origin: the dome is drawn the other way about the camera, as sky.
    rise: Vec3,
    closing: bool,
}

pub struct StatsView {
    scene: Scene,
    dome: Dome,
    in_perks: bool,
    /// The ring's turn, degrees: the heading the camera faces.
    angle: f32,
    tree: Option<Tree>,
    clock: f32,
}

/// The camera on a skill's perks, the skill at heading `angle` (degrees).
pub fn perks_camera(angle: f32) -> Camera {
    Camera {
        position: Vec3::ZERO,
        yaw: angle.to_radians(),
        pitch: PERKS_PITCH.to_radians(),
        fov_y: PERKS_FOV.to_radians(),
    }
}

/// Where a point of the stage (fractions of it) is on the plane a tree's
/// stars stand on, the skill at heading `angle`: under the point on a 16:9
/// screen.
pub fn tree_point(angle: f32, (x, y): (f32, f32)) -> Vec3 {
    let cam = perks_camera(angle);
    let t = (cam.fov_y / 2.0).tan();
    let (forward, right) = (cam.forward(), cam.right());
    let up = right.cross(forward);
    cam.position
        + (forward + right * ((x * 2.0 - 1.0) * t * STAGE_ASPECT) + up * ((1.0 - y * 2.0) * t))
            * TREE_DISTANCE
}

/// A star's placement at `at`, its art (in its own xy plane) facing the
/// camera.
fn star_transform(cam: &Camera, at: Vec3) -> Mat4 {
    let (forward, right) = (cam.forward(), cam.right());
    let up = right.cross(forward);
    Mat4::from_translation(at)
        * Mat4::from_mat3(Mat3::from_cols(right, up, -forward))
        * Mat4::from_scale(Vec3::splat(STAR_SCALE))
}

/// A line's placement from `a` to `b`: its length along its y, its art
/// facing the camera.
fn line_transform(cam: &Camera, a: Vec3, b: Vec3) -> Mat4 {
    let along = (b - a) / LINE_LENGTH;
    let toward = -cam.forward();
    let across = along.cross(toward).normalize_or_zero() * LINE_WIDTH;
    Mat4::from_cols(
        across.extend(0.0),
        along.extend(0.0),
        toward.extend(0.0),
        a.extend(1.0),
    )
}

impl StatsView {
    /// The scene to draw.
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// Whether a skill's tree is open rather than the ring.
    pub fn set_perks(&mut self, open: bool) {
        self.in_perks = open;
    }

    pub fn camera(&self) -> Camera {
        if self.in_perks {
            return perks_camera(self.angle);
        }
        Camera {
            position: Vec3::ZERO,
            yaw: self.angle.to_radians(),
            pitch: 0.0,
            fov_y: RING_FOV.to_radians(),
        }
    }

    /// Faces the skill at the ring's angle (20 degrees a skill).
    pub fn set_angle(&mut self, angle: f32) {
        self.angle = angle;
    }

    /// The menu is closing: the camera sinks away (`CameraOutro`).
    pub fn set_closing(&mut self) {
        let dome = &mut self.dome;
        if !dome.closing {
            dome.closing = true;
            if let Some(obj) = &mut dome.obj {
                obj.play("CameraOutro");
            }
        }
    }

    /// Plays the skydome's, each star's and each line's sequences on.
    pub fn update(&mut self, dt: f32) {
        self.clock += dt;
        update_dome(&mut self.dome, &mut self.scene.sky, dt, self.clock);
        let Some(tree) = &mut self.tree else {
            return;
        };
        let instances = &mut self.scene.dynamic;
        for (obj, next) in tree
            .stars
            .iter_mut()
            .map(|s| (&mut s.obj, &mut s.next))
            .chain(tree.lines.iter_mut().map(|l| (&mut l.obj, &mut l.next)))
        {
            obj.update(dt, self.clock, instances);
            if obj.playing.is_none()
                && let Some(name) = next.take()
            {
                obj.play(name);
                obj.update(0.0, self.clock, instances);
            }
        }
    }
}

/// Plays the skydome's sequence on: `CameraIntro` into `Idle`, or
/// `CameraOutro`; while either camera sequence plays, the dome stands where
/// its `CameraPosition` track puts the camera.
fn update_dome(dome: &mut Dome, sky: &mut [Instance], dt: f32, clock: f32) {
    let Some(obj) = &mut dome.obj else {
        return;
    };
    let playing = obj.playing.map(|(si, _)| si);
    obj.update(dt, clock, sky);
    if let Some(si) = playing {
        let seq = &obj.anim.sequences[si];
        let camera = matches!(seq.name.as_str(), "CameraIntro" | "CameraOutro");
        if let Some(ch) = seq
            .channels
            .iter()
            .find(|c| c.node == "CameraPosition")
            .filter(|_| camera)
        {
            let t = obj.playing.map_or(seq.stop, |(_, phase)| phase);
            dome.rise = ch.sample(t, Mat4::IDENTITY).w_axis.truncate();
        }
        if obj.playing.is_none() && seq.name == "CameraIntro" {
            dome.rise = Vec3::ZERO;
            obj.play("Idle");
        }
    }
    let shift = Mat4::from_translation(-dome.rise);
    obj.transform = shift;
    if let Some(still) = sky.first_mut() {
        still.transform = shift;
    }
}

/// The skills menu's sky and camera while it is up.
pub fn frame_parts(
    stats: &Option<StatsView>,
    menu: Option<crate::items::Menu>,
) -> Option<(&Scene, Camera)> {
    match stats {
        Some(v) if menu == Some(crate::items::Menu::Skills) => Some((v.scene(), v.camera())),
        _ => None,
    }
}

/// Plays `first`, then `then` once it ends (none: `first` rests).
fn play(
    obj: &mut AnimatedObject,
    next: &mut Option<&'static str>,
    first: &str,
    then: Option<&'static str>,
) {
    obj.play(first);
    *next = then;
}

impl Engine {
    /// Builds the skills menu's sky. None when the skydome can't be loaded.
    pub fn open_stats_view(&mut self) -> Option<()> {
        let paths = [DOME, STAR, LINE].map(String::from);
        self.models.load_all(&mut self.renderer, &self.vfs, &paths);
        let mut sky = vec![Instance::new(self.models.get(DOME)?, Mat4::IDENTITY)];
        let mut obj = self.animated_object(FormId::NULL, DOME, Mat4::IDENTITY, &mut sky, &[]);
        for inst in &mut sky {
            inst.sky = true;
            inst.tint = Vec4::ONE;
        }
        if let Some(obj) = &mut obj {
            obj.play("CameraIntro");
        }
        let mut dome = Dome {
            obj,
            rise: Vec3::ZERO,
            closing: false,
        };
        update_dome(&mut dome, &mut sky, 0.0, 0.0);
        let scene = Scene {
            env: Environment {
                sun_color: Vec3::ZERO,
                ambient: Vec3::ONE,
                fog_far: 1.0e7,
                clear_color: Vec3::new(0.01, 0.012, 0.02),
                sky: false,
                ..Default::default()
            },
            sky,
            ..Default::default()
        };
        self.stats_view = Some(StatsView {
            scene,
            dome,
            in_perks: false,
            angle: 0.0,
            tree: None,
            clock: 0.0,
        });
        Some(())
    }

    pub fn close_stats_view(&mut self) {
        self.stats_view = None;
    }

    /// Shows a skill's tree (None: the ring, no stars). The same tree again
    /// plays what changed: a star unlocking (`Unlock`), taken (`Own`),
    /// picked (`Select`) or let go (`Deselect`), a line lighting (`Unlock`).
    pub fn show_perk_tree(&mut self, shown: Option<&TreeShown>) {
        let Some(mut view) = self.stats_view.take() else {
            return;
        };
        match shown {
            None => {
                view.tree = None;
                view.scene.dynamic.clear();
            }
            Some(t) => {
                let same = view
                    .tree
                    .as_ref()
                    .is_some_and(|old| old.skill == t.skill && old.points == t.points);
                if !same {
                    view.tree = self.build_tree(&mut view.scene.dynamic, t);
                }
                if let Some(tree) = &mut view.tree {
                    change_tree(tree, t);
                }
            }
        }
        self.stats_view = Some(view);
    }

    /// A tree's stars and lines, each at rest in its state.
    fn build_tree(&self, instances: &mut Vec<Instance>, t: &TreeShown) -> Option<Tree> {
        instances.clear();
        let cam = perks_camera(t.angle);
        let at: Vec<Vec3> = t.points.iter().map(|&p| tree_point(t.angle, p)).collect();
        let mut lines = Vec::new();
        for &(from, to) in &t.lines {
            let transform = line_transform(&cam, at[from], at[to]);
            let mut obj = self.animated_object(FormId::NULL, LINE, transform, instances, &[])?;
            let lit = line_lit(&t.states, from, to);
            obj.play(if lit { "Unlocked" } else { "Locked" });
            lines.push(Line {
                obj,
                from,
                to,
                lit,
                next: None,
            });
        }
        let mut stars = Vec::new();
        for (i, &p) in at.iter().enumerate() {
            let mut obj =
                self.animated_object(FormId::NULL, STAR, star_transform(&cam, p), instances, &[])?;
            let state = t.states[i];
            obj.play(state.resting());
            stars.push(Star {
                obj,
                state,
                selected: false,
                next: None,
            });
        }
        Some(Tree {
            skill: t.skill,
            points: t.points.clone(),
            stars,
            lines,
        })
    }
}

/// A line is lit once both its perks are taken.
fn line_lit(states: &[StarState], from: usize, to: usize) -> bool {
    states[from] == StarState::Owned && states[to] == StarState::Owned
}

/// Plays the stars' and lines' changes towards what the tree shows now.
fn change_tree(tree: &mut Tree, t: &TreeShown) {
    for (i, star) in tree.stars.iter_mut().enumerate() {
        let (state, selected) = (t.states[i], t.selected == Some(i));
        if state == star.state && selected == star.selected {
            continue;
        }
        let rest = if selected {
            "Selected"
        } else {
            state.resting()
        };
        match (star.state, state, star.selected, selected) {
            // Taken or unlocked: its own transition, then its rest.
            (StarState::Unlocked | StarState::Locked, StarState::Owned, ..) => {
                play(&mut star.obj, &mut star.next, "Own", Some(rest))
            }
            (StarState::Locked, StarState::Unlocked, ..) => {
                play(&mut star.obj, &mut star.next, "Unlock", Some(rest))
            }
            // The graph selects and lets go of owned stars only; others
            // jump (its `SelectedWild` and resting wildcards).
            (StarState::Owned, StarState::Owned, false, true) => {
                play(&mut star.obj, &mut star.next, "Select", Some("Selected"))
            }
            (StarState::Owned, StarState::Owned, true, false) => {
                play(&mut star.obj, &mut star.next, "Deselect", Some("Owned"))
            }
            _ => play(&mut star.obj, &mut star.next, rest, None),
        }
        star.state = state;
        star.selected = selected;
    }
    for line in &mut tree.lines {
        let lit = line_lit(&t.states, line.from, line.to);
        if lit != line.lit {
            if lit {
                play(&mut line.obj, &mut line.next, "Unlock", Some("Unlocked"));
            } else {
                play(&mut line.obj, &mut line.next, "Locked", None);
            }
            line.lit = lit;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A world point at window pixels for a camera, as the renderer draws it.
    fn project(cam: &Camera, p: Vec3, (w, h): (f32, f32)) -> (f32, f32) {
        let clip = cam.proj(w / h) * cam.view() * p.extend(1.0);
        let ndc = clip.truncate() / clip.w;
        ((ndc.x + 1.0) / 2.0 * w, (1.0 - ndc.y) / 2.0 * h)
    }

    #[test]
    fn a_tree_point_projects_back_onto_its_stage_point() {
        for angle in [0.0, 20.0, 200.0] {
            for f in [(0.5, 0.5), (0.08, 0.27), (0.92, 0.66)] {
                let (x, y) = project(&perks_camera(angle), tree_point(angle, f), (1280.0, 720.0));
                assert!((x / 1280.0 - f.0).abs() < 1e-4, "{angle} {f:?} {x}");
                assert!((y / 720.0 - f.1).abs() < 1e-4, "{angle} {f:?} {y}");
            }
        }
    }

    #[test]
    fn a_line_runs_from_one_star_to_the_other_facing_the_camera() {
        let cam = perks_camera(40.0);
        let (a, b) = (tree_point(40.0, (0.3, 0.6)), tree_point(40.0, (0.5, 0.3)));
        let m = line_transform(&cam, a, b);
        assert!(m.transform_point3(Vec3::ZERO).distance(a) < 1e-4);
        assert!(m.transform_point3(Vec3::Y * LINE_LENGTH).distance(b) < 1e-3);
        // Its art (the xy plane) faces the camera: its z points back at it.
        assert!(m.z_axis.truncate().dot(-cam.forward()) > 0.999);
        assert!(m.x_axis.truncate().dot(cam.forward()).abs() < 1e-4);
    }

    #[test]
    fn a_line_lights_once_both_perks_are_taken() {
        use StarState::*;
        let states = [Owned, Unlocked, Owned];
        assert!(!line_lit(&states, 0, 1));
        assert!(line_lit(&states, 0, 2));
    }
}
