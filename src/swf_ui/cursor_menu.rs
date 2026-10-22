//! The game's cursor art, placed independently of the menu receiving input.

use ui_swf::{Menu, UiSwf, Value};
use winit::event::WindowEvent;

const CURSOR: &str = "_root.mc_Cursor";

pub(super) struct CursorMenu {
    pub menu: Menu,
}

impl CursorMenu {
    pub fn open(ui: &UiSwf, size: (u32, u32)) -> anyhow::Result<Self> {
        let menu = ui.open("cursormenu.swf", size)?;
        anyhow::ensure!(
            matches!(menu.get(&format!("{CURSOR}._x")), Value::Number(_))
                && matches!(menu.get(&format!("{CURSOR}._y")), Value::Number(_)),
            "cursor movie has no mc_Cursor clip"
        );
        Ok(Self { menu })
    }

    pub fn position(&self, (x, y): (f64, f64)) {
        // The SWF's onMouseMove copies _xmouse / _ymouse into this clip.
        // Ruffle broadcasts that listener before updating its mouse position;
        // set the same fields directly to avoid a one-event visual delay.
        self.menu.position_clip(CURSOR, x, y);
    }
}

/// A position is usable only after a move inside the focused window. Forget
/// it when leaving, so returning never draws a cursor at a stale position.
#[derive(Default)]
pub(super) struct Pointer {
    position: Option<(f64, f64)>,
}

impl Pointer {
    pub fn observe(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.moved(position.x, position.y);
            }
            WindowEvent::CursorLeft { .. }
            | WindowEvent::CursorEntered { .. }
            | WindowEvent::Focused(false) => self.position = None,
            _ => {}
        }
    }

    pub fn moved(&mut self, x: f64, y: f64) {
        self.position = (x.is_finite() && y.is_finite()).then_some((x, y));
    }

    pub fn position(&self) -> (f64, f64) {
        self.position.unwrap_or_default()
    }

    pub fn visible_position(&self, allowed: bool, menu_takes_input: bool) -> Option<(f64, f64)> {
        self.position.filter(|_| allowed && menu_takes_input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::dpi::PhysicalPosition;
    use winit::event::DeviceId;

    fn moved(x: f64, y: f64) -> WindowEvent {
        WindowEvent::CursorMoved {
            device_id: DeviceId::dummy(),
            position: PhysicalPosition::new(x, y),
        }
    }

    #[test]
    fn cursor_requires_menu_permission_and_current_pointer() {
        let mut pointer = Pointer::default();
        assert_eq!(pointer.visible_position(true, true), None);
        pointer.observe(&moved(0.0, 0.0));
        assert_eq!(pointer.visible_position(true, true), Some((0.0, 0.0)));
        assert_eq!(pointer.visible_position(false, true), None);
        assert_eq!(pointer.visible_position(true, false), None);
        pointer.observe(&moved(731.5, 412.25));
        assert_eq!(pointer.visible_position(true, true), Some((731.5, 412.25)));
    }

    #[test]
    fn leaving_or_focus_loss_forgets_position_until_next_move() {
        let mut pointer = Pointer::default();
        for event in [
            WindowEvent::CursorLeft {
                device_id: DeviceId::dummy(),
            },
            WindowEvent::CursorEntered {
                device_id: DeviceId::dummy(),
            },
            WindowEvent::Focused(false),
        ] {
            pointer.observe(&moved(320.0, 180.0));
            pointer.observe(&event);
            assert_eq!(pointer.visible_position(true, true), None);
            pointer.observe(&WindowEvent::Focused(true));
            assert_eq!(pointer.visible_position(true, true), None);
        }
        pointer.observe(&moved(900.0, 600.0));
        assert_eq!(pointer.visible_position(true, true), Some((900.0, 600.0)));
    }

    #[test]
    fn invalid_pointer_positions_do_not_hide_the_system_cursor() {
        let mut pointer = Pointer::default();
        for (x, y) in [(f64::NAN, 10.0), (10.0, f64::INFINITY)] {
            pointer.observe(&moved(x, y));
            assert_eq!(pointer.visible_position(true, true), None);
        }
    }
}
