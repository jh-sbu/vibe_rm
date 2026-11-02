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
/// zoom, and degrees a second the keys tilt.
const PAN_SPEED: f32 = 0.5;
const ZOOM_SPEED: f32 = 0.6;
const LOOK_SPEED: f32 = 120.0;
/// Zoom a wheel notch.
const WHEEL_ZOOM: f32 = 0.06;
/// How close a marker must be to the pointer to be under it, screen heights.
const REACH: f32 = 0.03;
/// A press that moves less than this, pixels, is a click, not a drag.
const CLICK_SLOP: f64 = 6.0;

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
    /// The marker told to the movie as selected.
    selected: Option<usize>,
    /// Markers are still being created (`CreateMarkers`).
    creating: bool,
    /// Esc, Tab or the map key: close.
    pub close: bool,
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
        menu.callback("SetDateString", &[text(&engine.date_text())]);
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
            selected: None,
            creating: false,
            close: false,
        };
        m.give_markers(engine);
        Ok(m)
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
        for event in [
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
        ] {
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
                self.drag = Some((at, 0.0));
            } else if let Some((_, moved)) = self.drag.take()
                && std::mem::take(&mut self.click)
                && moved < CLICK_SLOP
            {
                self.menu.callback("ClickSelectedMarker", &[]);
            }
        }
    }

    /// The camera's input and the markers' places, once a frame. `pointer`
    /// is in pixels.
    pub fn update(&mut self, engine: &mut Engine, dt: f32, pointer: (f64, f64), size: (u32, u32)) {
        for event in std::mem::take(&mut self.pending) {
            match event {
                "Cancel" => self.close = true,
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
        self.menu.callback("RefreshMarkers", &[]);
        let aspect = w as f32 / h as f32;
        let under = engine.map_view.as_ref().and_then(|v| {
            let at = Vec2::new((pointer.0 / w) as f32, (pointer.1 / h) as f32);
            v.marker_at(at, aspect, REACH)
                .filter(|&i| screen.get(i).is_some_and(|s| s.0))
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
            self.menu.callback(
                "SetSelectedMarker",
                &[num(under.map_or(-1.0, |i| i as f32))],
            );
        }
    }

    /// The menu's calls this answers; false for those it leaves to the
    /// menus' shared ones.
    pub fn answer(&mut self, engine: &mut Engine, call: &GameCall) -> bool {
        match call.name.as_str() {
            "ClickCallback" => self.click = true,
            "MarkerClick" => {
                if let Some(Value::Number(i)) = call.args.first()
                    && let Some(crate::map_view::RowAction::Travel(marker)) = engine
                        .map_view
                        .as_ref()
                        .map(|v| crate::map_view::row_action(&v.rows, *i))
                {
                    engine.request_map_travel(marker);
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
