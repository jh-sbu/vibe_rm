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

    /// Opt-in compatibility probe; the game assets remain outside the repo.
    #[test]
    #[ignore = "requires GPU and VRM_CURSOR_DATA pointing to installed/extracted Interface assets"]
    fn installed_cursor_movie_positions_survive_resize() -> anyhow::Result<()> {
        let data = std::env::var_os("VRM_CURSOR_DATA").expect("set VRM_CURSOR_DATA");
        let vfs = vfs::Vfs::new(data, &[]);
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default()))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                required_limits: adapter.limits(),
                ..Default::default()
            }))?;
        let ui = UiSwf::new(
            instance,
            adapter,
            device,
            queue,
            wgpu::TextureFormat::Rgba8Unorm,
            &vfs,
            "english",
        )?;
        let mut cursor = CursorMenu::open(&ui, (1280, 720))?;
        // The default 640x480 movie is centered and scaled by the viewport's
        // height here. Check the tip against the requested physical pixels,
        // including outside the movie's nominal 4:3 stage.
        for (size, pixel, stage) in [
            ((1280, 720), (320.0, 180.0), (106.65, 120.0)),
            ((1280, 720), (10.0, 15.0), (-100.0, 10.0)),
            ((1920, 1080), (960.0, 540.0), (320.0, 240.0)),
            ((720, 1280), (360.0, 640.0), (320.0, 240.0)),
            ((640, 480), (110.0, 75.0), (110.0, 75.0)),
        ] {
            cursor.menu.resize(size.0, size.1);
            cursor.position(pixel);
            let Value::Number(x) = cursor.menu.get(&format!("{CURSOR}._x")) else {
                panic!("cursor x missing");
            };
            let Value::Number(y) = cursor.menu.get(&format!("{CURSOR}._y")) else {
                panic!("cursor y missing");
            };
            assert!((x - stage.0).abs() <= 0.05, "{size:?}: x {x}");
            assert!((y - stage.1).abs() <= 0.05, "{size:?}: y {y}");
            cursor.menu.render();
            let texture = cursor.menu.texture();
            assert_eq!((texture.width(), texture.height()), size);
        }
        Ok(())
    }
}
