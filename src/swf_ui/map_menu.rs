//! The map as the game shows it (`map.swf`): the bars and the markers over
//! the world map the engine draws under it (`crate::map_view`).
//!
//! The game tells the movie how many markers there are (`SetNumMarkers`),
//! fills `WorldMap.MarkerData` with each one's name, icon type and whether
//! it is undiscovered, has it create them (`CreateMarkers`, a few a frame
//! until `GetCreatingMarkers` says it is done), then each frame fills
//! `MarkerData` anew with each one's visibility, position (as fractions of
//! the screen) and rotation for `RefreshMarkers`. `SetSelectedMarker(i)`
//! names the one under the pointer, `ClickSelectedMarker` clicks it; the
//! movie answers `ClickCallback` (a mouse press), `MarkerClick(i)`,
//! `OpenJournalCallback`, `CurrentLocationCallback` and `ToggleMapCallback`.
//! Choices: `known_ambiguities/map-view.md`.

use std::collections::HashSet;

use glam::Vec2;
use ui_swf::{GameCall, Menu, UiSwf, Value};
use winit::keyboard::KeyCode;

use crate::engine::Engine;
use crate::items::Menu as Shown;
use crate::map_view::PLAYER_ICON;

use super::{Buttons, num, text};

const M: &str = "_root.WorldMap";

/// Screen heights a second the arrow keys pan, and zoom a second the keys
/// zoom. Mouse look is degrees per screen height.
const PAN_SPEED: f32 = 0.5;
const ZOOM_SPEED: f32 = 0.6;
const LOOK_SPEED: f32 = 120.0;
/// Zoom a wheel notch.
const WHEEL_ZOOM: f32 = 0.06;
/// How close a marker must be to the pointer to be under it, screen heights.
const REACH: f32 = 0.03;
/// A press that moves less than this, pixels, is a click, not a drag.
const CLICK_SLOP: f64 = 6.0;

const EVENTS: &[&str] = &[
    "Cancel",
    "Up",
    "Down",
    "Left",
    "Right",
    "Zoom In",
    "Zoom Out",
    "MapLookMode",
    "PlacePlayerMarker",
    "PlayerPosition",
    "LocalMap",
    "LocalMapMoveMode",
    "Journal",
];

pub(super) struct MapMenu {
    pub menu: Menu,
    /// Keys held, for the camera.
    keys: HashSet<KeyCode>,
    mouse: HashSet<u32>,
    bindings: crate::input_map::Shared,
    pending: Vec<&'static str>,
    look: Option<(f64, f64)>,
    /// Wheel notches since the last update.
    wheel: f32,
    /// The left button held: where the pointer was, and how far it has
    /// gone since it went down.
    drag: Option<((f64, f64), f64)>,
    /// A press on the map waiting to be a click when released in place.
    click: bool,
    released: bool,
    marker_snapshot: Option<crate::map_markers::PlayerMarker>,
    /// The pointer as a fraction of the window, and in window pixels.
    pointer: Vec2,
    pointer_px: (f64, f64),
    /// The marker told to the movie as selected.
    selected: Option<usize>,
    restore_selected: Option<usize>,
    release_waiting: bool,
    controls_generation: u64,
    /// Markers are still being created (`CreateMarkers`).
    creating: bool,
    /// Esc, Tab or the map key: close.
    pub close: bool,
    pub journal: bool,
}

impl MapMenu {
    pub fn open(
        ui: &UiSwf,
        size: (u32, u32),
        engine: &Engine,
        buttons: std::sync::Arc<Buttons>,
    ) -> anyhow::Result<Self> {
        let menu = ui.open("map.swf", size)?;
        super::set_button_mapping(&menu, &buttons);
        menu.set_responder(move |call, _| super::respond(&buttons, call));
        menu.invoke(&format!("{M}.SetPlatform"), &[num(0.0), Value::Bool(false)]);
        menu.set(
            &format!("{M}.PlayerLocationMarkerType"),
            num(PLAYER_ICON as f32),
        );
        menu.invoke(&format!("{M}.SetDateString"), &[text(&engine.date_text())]);
        let mut m = MapMenu {
            menu,
            keys: HashSet::new(),
            mouse: HashSet::new(),
            bindings: engine.input_map.clone(),
            pending: Vec::new(),
            look: None,
            wheel: 0.0,
            drag: None,
            click: false,
            released: false,
            marker_snapshot: engine.player_marker,
            pointer: Vec2::splat(0.5),
            pointer_px: (0.0, 0.0),
            selected: None,
            restore_selected: None,
            release_waiting: false,
            controls_generation: u64::MAX,
            creating: false,
            close: false,
            journal: false,
        };
        m.refresh_controls();
        m.give_markers(engine);
        Ok(m)
    }

    pub fn show_journal(&mut self, show: bool) {
        self.journal = show;
        self.menu
            .invoke(&format!("{M}.ShowJournal"), &[Value::Bool(show)]);
        self.clear_input();
    }

    pub fn clear_input(&mut self) {
        self.keys.clear();
        self.mouse.clear();
        self.pending.clear();
        self.drag = None;
        self.look = None;
        self.click = false;
        self.released = false;
        self.release_waiting = false;
        self.wheel = 0.0;
    }

    fn refresh_controls(&mut self) {
        let bindings = self.bindings.read().unwrap();
        if self.controls_generation == bindings.generation {
            return;
        }
        self.controls_generation = bindings.generation;
        for (clip, event) in [
            ("RightButton", "PlacePlayerMarker"),
            ("PlayerLocButton", "PlayerPosition"),
            ("LocalMapButton", "LocalMap"),
            ("JournalButton", "Journal"),
            ("LeftButton", "Zoom In"),
        ] {
            let art = bindings
                .context_bindings("Map Menu", event, 0)
                .first()
                .and_then(|c| c.last())
                .and_then(|n| super::keyboard_art(*n))
                .or_else(|| {
                    bindings
                        .context_bindings("Map Menu", event, 1)
                        .first()
                        .and_then(|c| c.last())
                        .and_then(|n| super::mouse_art(*n))
                })
                .unwrap_or_else(|| "UnknownKey".into());
            self.menu
                .set(&format!("{M}.BottomBar.{clip}.PCArt"), text(&art));
            self.menu
                .invoke(&format!("{M}.BottomBar.{clip}.RefreshArt"), &[]);
        }
    }

    pub fn refresh_rows(&mut self, engine: &mut Engine) {
        let old = engine
            .map_view
            .as_ref()
            .and_then(|v| self.selected.and_then(|i| v.rows.get(i)))
            .cloned();
        engine.map_view_rows();
        let selected = old.and_then(|old| {
            engine
                .map_view
                .as_ref()?
                .rows
                .iter()
                .position(|row| crate::map_view::same_row(&old, row))
        });
        self.give_markers(engine);
        self.restore_selected = selected;
        self.marker_snapshot = engine.player_marker;
    }

    /// A complete synthetic click still waits for the movie's ClickCallback,
    /// so the bottom bar gets the same exclusion as interactive mouse input.
    pub fn test_click(&mut self) {
        self.release_waiting = true;
    }

    /// Tells the movie the markers, to create.
    fn give_markers(&mut self, engine: &Engine) {
        let Some(view) = &engine.map_view else { return };
        let mut data = Vec::new();
        for row in &view.rows {
            data.push(text(&row.name));
            data.push(num(row.kind as f32));
            data.push(Value::Bool(row.undiscovered));
        }
        self.menu.invoke(
            &format!("{M}.SetNumMarkers"),
            &[num(view.rows.len() as f32)],
        );
        self.menu.set(&format!("{M}.MarkerData"), Value::List(data));
        self.creating = true;
        self.selected = None;
    }

    /// A key pressed or released. True when the map takes it.
    pub fn key(&mut self, code: KeyCode, pressed: bool) -> bool {
        use crate::input_map::Button;
        if pressed {
            self.keys.insert(code);
        } else {
            self.keys.remove(&code);
        }
        let bindings = self.bindings.read().unwrap();
        if pressed && bindings.triggered("Quick Map", Button::Key(code), &self.keys, &self.mouse) {
            self.close = true;
        }
        for &event in EVENTS {
            if pressed
                && bindings.triggered_in(
                    "Map Menu",
                    event,
                    Button::Key(code),
                    &self.keys,
                    &self.mouse,
                )
            {
                self.pending.push(event);
            }
        }
        // All map input is modal; releases still update chord state.
        true
    }

    pub fn wheel(&mut self, notches: f32) {
        let bindings = self.bindings.read().unwrap();
        let button = crate::input_map::Button::Mouse(if notches > 0.0 { 8 } else { 9 });
        for (event, sign) in [("Zoom In", 1.0), ("Zoom Out", -1.0)] {
            if bindings.triggered_in("Map Menu", event, button, &self.keys, &self.mouse) {
                self.wheel += notches.abs() * sign;
            }
        }
    }

    pub fn button(&mut self, button: u32, pressed: bool, at: (f64, f64)) {
        if pressed {
            self.mouse.insert(button);
        } else {
            self.mouse.remove(&button);
        }
        let bindings = self.bindings.read().unwrap();
        let input = crate::input_map::Button::Mouse(button);
        if pressed && bindings.triggered("Quick Map", input, &self.keys, &self.mouse) {
            self.close = true;
        }
        for &event in EVENTS {
            if pressed && bindings.triggered_in("Map Menu", event, input, &self.keys, &self.mouse) {
                self.pending.push(event);
            }
        }
        if bindings.triggered_in(
            "Map Menu",
            "MapLookMode",
            crate::input_map::Button::Mouse(button),
            &self.keys,
            &self.mouse,
        ) {
            self.look = pressed.then_some(at);
        }
        if bindings.triggered_in(
            "Map Menu",
            "LocalMapMoveMode",
            crate::input_map::Button::Mouse(button),
            &self.keys,
            &self.mouse,
        ) {
            if pressed {
                self.click = false;
                self.release_waiting = false;
                self.drag = Some((at, 0.0));
            } else if let Some((last, moved)) = self.drag.take() {
                let moved = moved + (at.0 - last.0).hypot(at.1 - last.1);
                if moved < CLICK_SLOP {
                    if std::mem::take(&mut self.click) {
                        self.released = true;
                    } else {
                        self.release_waiting = true;
                    }
                }
            }
        }
    }

    /// The camera's input and the markers' places, once a frame. `pointer`
    /// is in pixels.
    pub fn update(&mut self, engine: &mut Engine, dt: f32, pointer: (f64, f64), size: (u32, u32)) {
        self.refresh_controls();
        let mut place = false;
        for event in std::mem::take(&mut self.pending) {
            match event {
                "Cancel" => self.close = true,
                "PlacePlayerMarker" => place = true,
                "PlayerPosition" => {
                    if let Some(view) = &mut engine.map_view
                        && let Some((p, _)) = view.player
                    {
                        view.center = p;
                    }
                }
                "Journal" => engine.menu = Some(Shown::Journal),
                _ => {}
            }
        }
        let bindings = self.bindings.read().unwrap();
        let held = |event| bindings.held_in("Map Menu", event, &self.keys, &self.mouse);
        let right = held("Right") as i32 as f32 - held("Left") as i32 as f32;
        let up = held("Up") as i32 as f32 - held("Down") as i32 as f32;
        let zoom = held("Zoom In") as i32 as f32 - held("Zoom Out") as i32 as f32;
        let (w, h) = (size.0.max(1) as f64, size.1.max(1) as f64);
        if let Some((last, moved)) = &mut self.drag {
            let (dx, dy) = (pointer.0 - last.0, pointer.1 - last.1);
            *moved += dx.hypot(dy);
            *last = pointer;
            // Dragging the map moves it under the pointer.
            if *moved >= CLICK_SLOP
                && let Some(view) = &mut engine.map_view
            {
                view.pan(-(dx / h) as f32, (dy / h) as f32);
            }
        }
        if let Some(last) = &mut self.look {
            if let Some(view) = &mut engine.map_view {
                view.turn_by(((pointer.0 - last.0) / h) as f32 * LOOK_SPEED);
                view.tilt_by(((pointer.1 - last.1) / h) as f32 * LOOK_SPEED);
            }
            *last = pointer;
        }
        let wheel = std::mem::take(&mut self.wheel);
        if let Some(view) = &mut engine.map_view {
            view.pan(right * PAN_SPEED * dt, up * PAN_SPEED * dt);
            // Zooming in is the wheel forwards, which lowers the camera.
            view.zoom_by(-zoom * ZOOM_SPEED * dt - wheel * WHEEL_ZOOM);
        }
        drop(bindings);
        self.pointer = Vec2::new((pointer.0 / w) as f32, (pointer.1 / h) as f32);
        self.pointer_px = pointer;
        if place {
            self.place_at_pointer(engine);
        }
        if self.marker_snapshot != engine.player_marker {
            self.refresh_rows(engine);
        }
        engine.update_map_view();
        if self.creating {
            self.menu.invoke(&format!("{M}.CreateMarkers"), &[]);
            let state = self.menu.invoke(&format!("{M}.GetCreatingMarkers"), &[]);
            let done = matches!(state, Value::Bool(false));
            if !done {
                return;
            }
            self.creating = false;
        }
        let screen = engine.map_marker_screen();
        let mut data = Vec::with_capacity(screen.len() * 4);
        for (show, at, rotation) in &screen {
            data.push(Value::Bool(*show));
            data.push(num(at.x));
            data.push(num(at.y));
            data.push(num(*rotation));
        }
        self.menu.set(&format!("{M}.MarkerData"), Value::List(data));
        self.menu.invoke(&format!("{M}.RefreshMarkers"), &[]);
        let aspect = w as f32 / h as f32;
        let under = self.restore_selected.take().or_else(|| {
            engine.map_view.as_ref().and_then(|v| {
                let at = Vec2::new((pointer.0 / w) as f32, (pointer.1 / h) as f32);
                v.marker_at(at, aspect, REACH)
                    .filter(|&i| screen.get(i).is_some_and(|s| s.0))
            })
        });
        if under != self.selected {
            self.selected = under;
            // The description's title is the name, with no lines under it.
            if let Some(i) = under
                && let Some(row) = engine.map_view.as_ref().and_then(|v| v.rows.get(i))
            {
                self.menu.invoke(
                    &format!("{M}.MarkerDescriptionObj.SetDescription"),
                    &[text(&row.name), Value::List(Vec::new())],
                );
            }
            self.menu.invoke(
                &format!("{M}.SetSelectedMarker"),
                &[num(under.map_or(-1.0, |i| i as f32))],
            );
        }
        if std::mem::take(&mut self.released) {
            if under.is_some() {
                self.menu.invoke(&format!("{M}.ClickSelectedMarker"), &[]);
            } else {
                self.place_at_pointer(engine);
            }
        }
    }

    fn place_at_pointer(&mut self, engine: &mut Engine) {
        let aspect = engine.renderer.width as f32 / engine.renderer.height.max(1) as f32;
        if !(0.0..=1.0).contains(&self.pointer.x) || !(0.0..=1.0).contains(&self.pointer.y) {
            return;
        }
        // hitTest takes the movie's stage coordinates, not window pixels:
        // they differ whenever the window isn't the stage's size.
        let (x, y) = self.menu.stage_point(self.pointer_px.0, self.pointer_px.1);
        let over_bar = self.menu.invoke(
            &format!("{M}.BottomBar.hitTest"),
            &[Value::Number(x), Value::Number(y), Value::Bool(false)],
        );
        if over_bar == Value::Bool(true) {
            return;
        }
        if let Some((world, at)) = engine
            .map_view
            .as_ref()
            .and_then(|v| v.ground_at(self.pointer, aspect).map(|at| (v.world, at)))
        {
            engine.request_player_marker(world, at);
        }
    }

    /// The menu's calls this answers; false for those it leaves to the
    /// menus' shared ones.
    pub fn answer(&mut self, engine: &mut Engine, call: &GameCall) -> bool {
        match call.name.as_str() {
            "ClickCallback" => {
                if std::mem::take(&mut self.release_waiting) {
                    self.released = true;
                } else {
                    self.click = self.drag.is_some();
                }
            }
            "MarkerClick" => {
                if let Some(Value::Number(i)) = call.args.first() {
                    match engine
                        .map_view
                        .as_ref()
                        .map(|v| crate::map_view::row_action(&v.rows, *i))
                    {
                        Some(crate::map_view::RowAction::Travel(marker)) => {
                            engine.request_map_travel(marker)
                        }
                        Some(crate::map_view::RowAction::PlayerMarker) => {
                            self.place_at_pointer(engine)
                        }
                        _ => {}
                    }
                }
            }
            "OpenJournalCallback" => engine.menu = Some(Shown::Journal),
            "CurrentLocationCallback" => {
                if let Some(view) = &mut engine.map_view
                    && let Some((p, _)) = view.player
                {
                    view.center = p;
                }
            }
            "ToggleMapCallback" | "SetLocalMapExtents" => {}
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::engine::Location;
    use crate::map_view::{
        PLAYER_SET_ICON,
        tests::{hierarchy_fixture, view},
    };
    use esp::FormId;

    fn capture(swf: &mut crate::swf_ui::SwfUi, engine: &mut Engine, name: &str) {
        let Some(out) = std::env::var_os("VRM_MAP_CAPTURES") else {
            return;
        };
        let out = std::path::PathBuf::from(out);
        std::fs::create_dir_all(&out).unwrap();
        let (scene, camera) = crate::map_view::frame_parts(&engine.map_view, engine.menu).unwrap();
        let pixels = engine
            .renderer
            .render_to_image(scene, &camera, |_, target| swf.draw(target, false));
        let file = std::fs::File::create(out.join(format!("{name}.png"))).unwrap();
        let mut encoder = png::Encoder::new(file, 1280, 720);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
    }

    /// Reads only installed Interface assets, never copies them. Run with
    /// SKYRIM_DATA and VRM_SETTINGS pointing outside the repository/game Data.
    #[test]
    #[ignore = "requires installed Skyrim Interface assets, SKYRIM_DATA, VRM_SETTINGS and a GPU"]
    fn map_movie_placement_controls_journal_and_show_on_map() {
        let data = std::env::var_os("SKYRIM_DATA")
            .expect("set SKYRIM_DATA to the installed Data directory");
        assert!(
            std::env::var_os("VRM_SETTINGS").is_some(),
            "set VRM_SETTINGS to a temporary output file"
        );
        let dir = hierarchy_fixture("movie");
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Map.esp");
        e.vfs = vfs::Vfs::new(data, &[]).into();
        let controls = e.vfs.read("interface/controls/pc/controlmap.txt").unwrap();
        e.input_map = std::sync::Arc::new(std::sync::RwLock::new(
            crate::input_map::InputMap::parse(&String::from_utf8_lossy(&controls)),
        ));
        e.renderer.resize(1280, 720);
        e.location = Location::Exterior {
            world: FormId(0x800),
            center: (0, 0),
        };
        let st = e.scripts.quests.entry(FormId(0xB00)).or_default();
        st.running = true;
        st.active = true;
        st.objectives_displayed.insert(10);
        st.objective_texts
            .insert(10, "Find the synthetic target".into());
        st.aliases.insert(0, FormId(0x901));
        let mut map = view(50.0);
        map.world = FormId(0x800);
        map.player = Some((Vec2::new(10.0, 20.0), 0.0));
        e.map_view = Some(map);
        e.map_view_rows();
        e.menu = Some(Shown::Map);
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            apply_limit_buckets: false,
            ..Default::default()
        }))
        .unwrap();
        let mut swf =
            crate::swf_ui::SwfUi::new(instance, adapter, &e, e.renderer.color_format, (1280, 720))
                .unwrap();
        swf.map = Some(MapMenu::open(&swf.ui, (1280, 720), &e, swf.buttons.clone()).unwrap());
        let tick = |swf: &mut crate::swf_ui::SwfUi, e: &mut Engine| {
            for _ in 0..30 {
                swf.update(e, 1.0 / 60.0);
            }
        };
        tick(&mut swf, &mut e);
        swf.click(700.0, 400.0);
        tick(&mut swf, &mut e);
        let first = e.player_marker.expect("empty click places immediately");
        assert!(e.messages.boxes.is_empty());
        let i = e
            .map_view
            .as_ref()
            .unwrap()
            .rows
            .iter()
            .position(|r| r.kind == PLAYER_SET_ICON)
            .unwrap();
        // Check the original delegate table's scope, independently of our
        // explicit world-layer invokes. The callbacks currently reach WorldMap.
        let menu = &swf.map.as_ref().unwrap().menu;
        menu.callback("SetSelectedMarker", &[num(-1.0)]);
        menu.callback("SetSelectedMarker", &[num(i as f32)]);
        assert_eq!(
            menu.get("_root.WorldMap.SelectedMarker.Index"),
            num(i as f32)
        );
        menu.callback("RefreshMarkers", &[]);
        let x = menu.get(&format!("_root.WorldMap.Markers.{i}._x"));
        assert!(matches!(x, Value::Number(x) if (x - 700.0).abs() < 1.0));
        assert_eq!(
            menu.get("_root.WorldMap.BottomBar.RightButton.PCButton"),
            text("P")
        );
        capture(&mut swf, &mut e, "movie-custom-destination");
        swf.map.as_mut().unwrap().key(KeyCode::KeyW, true);
        swf.click(800.0, 450.0);
        tick(&mut swf, &mut e);
        assert_eq!(e.messages.boxes.len(), 1);
        assert_eq!(e.player_marker, Some(first));
        capture(&mut swf, &mut e, "movie-move-leave-remove");
        let center = e.map_view.as_ref().unwrap().center;
        tick(&mut swf, &mut e);
        assert_eq!(e.map_view.as_ref().unwrap().center, center);
        e.choose_message_button(0);
        tick(&mut swf, &mut e);
        assert_ne!(e.player_marker, Some(first));
        assert!(e.messages.boxes.is_empty());
        swf.press(KeyCode::KeyJ);
        tick(&mut swf, &mut e);
        assert!(swf.journal.is_some());
        assert!(swf.map.as_ref().unwrap().journal);
        assert!(e.map_view.is_some());
        let journal = &swf.journal.as_ref().unwrap().menu;
        assert_eq!(
            journal
                .get("_root.QuestJournalFader.Menu_mc.QuestsFader.Page_mc.ShowOnMapButton._alpha"),
            num(100.0)
        );
        capture(&mut swf, &mut e, "movie-journal-show-on-map");
        e.scripts.quests.get_mut(&FormId(0xB00)).unwrap().active = false;
        swf.press(KeyCode::Escape);
        tick(&mut swf, &mut e);
        assert_eq!(e.menu, Some(Shown::Map));
        assert!(swf.journal.is_none());
        assert!(!swf.map.as_ref().unwrap().journal);
        assert!(
            !e.map_view
                .as_ref()
                .unwrap()
                .rows
                .iter()
                .any(|r| r.kind == 62)
        );
        swf.press(KeyCode::KeyJ);
        tick(&mut swf, &mut e);
        swf.press(KeyCode::KeyM);
        tick(&mut swf, &mut e);
        assert_eq!(e.menu, Some(Shown::Map));
        assert!(swf.journal.is_none());
        assert!(
            e.map_view
                .as_ref()
                .unwrap()
                .center
                .distance(Vec2::new(120.0, 240.0))
                < 0.1
        );
        assert_eq!(e.map_view.as_ref().unwrap().zoom, 1.0);
        swf.press(KeyCode::KeyE);
        tick(&mut swf, &mut e);
        assert_eq!(e.map_view.as_ref().unwrap().center, Vec2::new(10.0, 20.0));
        let map = swf.map.as_mut().unwrap();
        map.button(1, true, (640.0, 360.0));
        map.update(&mut e, 1.0 / 60.0, (700.0, 400.0), (1280, 720));
        map.button(1, false, (700.0, 400.0));
        let view = e.map_view.as_ref().unwrap();
        assert!(view.heading > 0.0 && view.pitch > 50.0);
        // Short clicks delivered before a movie tick still wait for its
        // ClickCallback, rather than dropping the click or accepting a bar click.
        e.player_marker = None;
        let map = swf.map.as_mut().unwrap();
        map.button(0, true, (900.0, 450.0));
        map.button(0, false, (900.0, 450.0));
        map.answer(
            &mut e,
            &GameCall {
                name: "ClickCallback".into(),
                id: None,
                args: vec![],
            },
        );
        map.update(&mut e, 1.0 / 60.0, (900.0, 450.0), (1280, 720));
        assert!(e.player_marker.is_some());
        // Gameplay rebinding flows through the map's Up context reference.
        let before = e.map_view.as_ref().unwrap().center;
        assert!(
            e.input_map
                .write()
                .unwrap()
                .remap("Forward", crate::input_map::Button::Key(KeyCode::KeyG))
        );
        let map = swf.map.as_mut().unwrap();
        map.key(KeyCode::KeyW, true);
        map.update(&mut e, 0.1, (900.0, 450.0), (1280, 720));
        map.key(KeyCode::KeyW, false);
        assert_eq!(e.map_view.as_ref().unwrap().center, before);
        map.key(KeyCode::KeyG, true);
        map.update(&mut e, 0.1, (900.0, 450.0), (1280, 720));
        map.key(KeyCode::KeyG, false);
        assert_ne!(e.map_view.as_ref().unwrap().center, before);
        // The bottom bar is hit-tested in the movie's stage coordinates. Find
        // a stage row on it at 1280x720 (stage == window), then press P over
        // the same spot at 1920x1080, where the window is 1.5 times the stage.
        let bar_y = (500..720)
            .rev()
            .find(|&y| {
                swf.map.as_ref().unwrap().menu.invoke(
                    "_root.WorldMap.BottomBar.hitTest",
                    &[num(640.0), num(y as f32), Value::Bool(false)],
                ) == Value::Bool(true)
            })
            .expect("a stage row on the bottom bar");
        swf.resize(1920, 1080);
        e.renderer.resize(1920, 1080);
        for (at, placed) in [((960.0, bar_y as f64 * 1.5), false), ((960.0, 300.0), true)] {
            e.player_marker = None;
            let map = swf.map.as_mut().unwrap();
            map.key(KeyCode::KeyP, true);
            map.update(&mut e, 1.0 / 60.0, at, (1920, 1080));
            map.key(KeyCode::KeyP, false);
            assert_eq!(e.player_marker.is_some(), placed, "P at {at:?}");
        }
        swf.press(KeyCode::KeyM);
        tick(&mut swf, &mut e);
        assert_eq!(e.menu, None);
        assert!(e.map_view.is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
