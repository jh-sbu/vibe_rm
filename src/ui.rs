//! In-game UI: HUD, activation prompt and the developer console (egui based).

use egui::{Align2, Color32, FontId, Pos2, Stroke};

use esp::FormId;

use crate::engine::Engine;

pub struct Console {
    pub open: bool,
    pub input: String,
    pub lines: Vec<String>,
    history: Vec<String>,
    focus: bool,
}

impl Default for Console {
    fn default() -> Self {
        Console {
            open: false,
            input: String::new(),
            lines: vec!["vibe_rm console. Type 'help' for commands.".into()],
            history: Vec::new(),
            focus: false,
        }
    }
}

pub struct Ui {
    pub ctx: egui::Context,
    renderer: egui_wgpu::Renderer,
    pub console: Console,
    pub show_debug: bool,
    pub fps: u32,
    /// The item menu was shown last frame (the key that opened it doesn't close it).
    menu_shown: bool,
}

impl Ui {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let ctx = egui::Context::default();
        let mut visuals = egui::Visuals::dark();
        visuals.window_fill = Color32::from_rgba_unmultiplied(10, 10, 10, 220);
        ctx.set_visuals(visuals);
        ctx.global_style_mut(|s| s.animation_time = 0.0);
        let renderer = egui_wgpu::Renderer::new(device, format, egui_wgpu::RendererOptions::default());
        Ui { ctx, renderer, console: Console::default(), show_debug: false, fps: 0, menu_shown: false }
    }

    pub fn toggle_console(&mut self) {
        self.console.open = !self.console.open;
        self.console.focus = self.console.open;
    }

    /// Build this frame's UI.
    pub fn build(&mut self, raw: egui::RawInput, engine: &mut Engine) -> egui::FullOutput {
        let ctx = self.ctx.clone();
        let mut commands: Vec<String> = Vec::new();
        let mut choice: Option<usize> = None;
        let mut skip = false;
        let out = ctx.run_ui(raw, |ui| {
            let ctx = ui.ctx().clone();
            if engine.conversation.is_some() {
                self.dialogue(&ctx, engine, &mut choice, &mut skip);
            } else {
                self.hud(&ctx, engine);
            }
            if engine.menu.is_some() {
                self.item_menu(&ctx, engine);
            }
            self.menu_shown = engine.menu.is_some();
            if self.console.open {
                self.console_window(&ctx, &mut commands);
            }
        });
        if let Some(i) = choice {
            engine.choose_topic(i);
        }
        if skip {
            engine.skip_line();
        }
        for c in commands {
            self.console.lines.push(format!("> {c}"));
            let res = crate::console::execute(engine, &c);
            self.console.lines.extend(res);
        }
        out
    }

    fn hud(&self, ctx: &egui::Context, engine: &Engine) {
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("hud")));
        let rect = ctx.content_rect();
        let c = rect.center();
        // Crosshair
        if !self.console.open {
            let s = Stroke::new(1.5, Color32::from_rgba_unmultiplied(235, 235, 235, 200));
            painter.line_segment([Pos2::new(c.x - 6.0, c.y), Pos2::new(c.x - 2.0, c.y)], s);
            painter.line_segment([Pos2::new(c.x + 2.0, c.y), Pos2::new(c.x + 6.0, c.y)], s);
            painter.line_segment([Pos2::new(c.x, c.y - 6.0), Pos2::new(c.x, c.y - 2.0)], s);
            painter.line_segment([Pos2::new(c.x, c.y + 2.0), Pos2::new(c.x, c.y + 6.0)], s);
        }
        // Activation prompt
        if let Some((_, name)) = &engine.look_target {
            let verb = engine.look_verb();
            let pos = Pos2::new(c.x, c.y + rect.height() * 0.14);
            painter.text(pos + egui::vec2(1.5, 1.5), Align2::CENTER_CENTER, name, FontId::proportional(24.0), Color32::BLACK);
            painter.text(pos, Align2::CENTER_CENTER, name, FontId::proportional(24.0), Color32::WHITE);
            let hint = format!("E  {verb}");
            painter.text(pos + egui::vec2(0.0, 28.0), Align2::CENTER_CENTER, hint, FontId::proportional(16.0), Color32::from_gray(200));
        }
        // Script notifications (top left, like Skyrim's HUD messages).
        let now = engine.scripts.real_time;
        for (i, (text, t)) in engine.scripts.notifications.iter().rev().take(6).enumerate() {
            let age = (now - t) as f32;
            let alpha = ((6.0 - age) / 1.5).clamp(0.0, 1.0);
            let col = Color32::from_rgba_unmultiplied(240, 240, 240, (alpha * 255.0) as u8);
            let y = rect.top() + 60.0 + i as f32 * 22.0;
            painter.text(Pos2::new(rect.left() + 30.0, y), Align2::LEFT_TOP, text, FontId::proportional(17.0), col);
        }
        // Compass-ish heading and location at the top.
        let heading = (engine.camera.yaw.to_degrees().rem_euclid(360.0) / 45.0).round() as usize % 8;
        let dirs = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
        painter.text(
            Pos2::new(c.x, rect.top() + 24.0),
            Align2::CENTER_CENTER,
            dirs[heading],
            FontId::proportional(20.0),
            Color32::from_gray(230),
        );
        if self.show_debug {
            let p = engine.camera.position;
            let st = engine.renderer.stats;
            let text = format!(
                "{} fps  {} draws  {} instances\npos {:.0} {:.0} {:.0}  {:02}:{:02}\n{}",
                self.fps,
                st.draws,
                st.instances,
                p.x,
                p.y,
                p.z,
                engine.hour as u32,
                (engine.hour.fract() * 60.0) as u32,
                engine.location_name()
            );
            painter.text(Pos2::new(rect.left() + 8.0, rect.top() + 8.0), Align2::LEFT_TOP, text, FontId::monospace(13.0), Color32::YELLOW);
        }
    }

    fn dialogue(&self, ctx: &egui::Context, engine: &Engine, choice: &mut Option<usize>, skip: &mut bool) {
        let Some(c) = &engine.conversation else { return };
        let rect = ctx.content_rect();
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("dlg")));
        // Subtitle
        if let Some(line) = &c.current {
            let galley = painter.layout(
                line.text.clone(),
                FontId::proportional(20.0),
                Color32::WHITE,
                rect.width() * 0.6,
            );
            let pos = Pos2::new(rect.center().x - galley.size().x / 2.0, rect.bottom() - 120.0);
            painter.galley(pos + egui::vec2(1.5, 1.5), galley.clone(), Color32::BLACK);
            painter.galley(pos, galley, Color32::WHITE);
            if ctx.input(|i| i.pointer.primary_clicked() || i.key_pressed(egui::Key::Space)) {
                *skip = true;
            }
        }
        // Topic menu on the right, Skyrim style (painter-drawn with manual hit testing).
        let x = rect.right() - rect.width() * 0.38;
        let mut y = rect.center().y - 150.0;
        let shadow = |p: Pos2, t: &str, size: f32, col: Color32| {
            painter.text(p + egui::vec2(1.5, 1.5), Align2::LEFT_TOP, t, FontId::proportional(size), Color32::BLACK);
            painter.text(p, Align2::LEFT_TOP, t, FontId::proportional(size), col)
        };
        shadow(Pos2::new(x, y), &c.name, 28.0, Color32::WHITE);
        y += 44.0;
        if c.current.is_some() {
            return;
        }
        let (hover, clicked) = ctx.input(|i| (i.pointer.hover_pos(), i.pointer.primary_clicked()));
        let mut entries: Vec<&str> = c.options.iter().map(|o| o.1.as_str()).collect();
        entries.push("Goodbye.");
        for (i, text) in entries.iter().enumerate() {
            let r = shadow(Pos2::new(x, y), text, 20.0, Color32::from_gray(225));
            if hover.is_some_and(|p| r.expand(3.0).contains(p)) {
                painter.rect_stroke(r.expand(3.0), 2.0, Stroke::new(1.0, Color32::from_gray(170)), egui::StrokeKind::Outside);
                if clicked {
                    *choice = Some(if i < c.options.len() { i } else { usize::MAX });
                }
            }
            y += 30.0;
        }
        // Number keys pick options; Tab leaves.
        let keys = [egui::Key::Num1, egui::Key::Num2, egui::Key::Num3, egui::Key::Num4, egui::Key::Num5, egui::Key::Num6, egui::Key::Num7, egui::Key::Num8, egui::Key::Num9];
        for (k, key) in keys.iter().enumerate() {
            if ctx.input(|inp| inp.key_pressed(*key)) {
                *choice = Some(if k < c.options.len() { k } else { usize::MAX });
            }
        }
        if ctx.input(|inp| inp.key_pressed(egui::Key::Tab)) {
            *choice = Some(usize::MAX);
        }
    }

    /// The inventory, or a container beside it: click an item to move one (shift: all).
    fn item_menu(&mut self, ctx: &egui::Context, engine: &mut Engine) {
        use crate::engine::PLAYER_REF;
        use crate::items::Menu;
        let Some(menu) = engine.menu else { return };
        let mut moves: Vec<(FormId, FormId, FormId, i32)> = Vec::new();
        let mut close = self.menu_shown && ctx.input(|i| i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Tab));
        let all = ctx.input(|i| i.modifiers.shift);
        let rect = ctx.content_rect();
        let list = |ui: &mut egui::Ui, engine: &mut Engine, owner: FormId, to: Option<FormId>, moves: &mut Vec<(FormId, FormId, FormId, i32)>| {
            let items = engine.listed_inventory(owner);
            let weight: f32 = items.iter().map(|(_, n, i)| i.weight * *n as f32).sum();
            ui.set_min_width(420.0);
            egui::ScrollArea::vertical().max_height(rect.height() * 0.6).auto_shrink([false, true]).id_salt(owner.0).show(ui, |ui| {
                egui::Grid::new(("items", owner.0)).striped(true).num_columns(4).show(ui, |ui| {
                    ui.label(egui::RichText::new("Item").strong());
                    ui.label(egui::RichText::new("Count").strong());
                    ui.label(egui::RichText::new("Weight").strong());
                    ui.label(egui::RichText::new("Value").strong());
                    ui.end_row();
                    for (f, n, info) in &items {
                        let r = ui.add(egui::Label::new(&info.name).sense(egui::Sense::click()));
                        if let (Some(to), true) = (to, r.clicked()) {
                            moves.push((owner, to, *f, if all { *n } else { 1 }));
                        }
                        ui.label(n.to_string());
                        ui.label(format!("{:.1}", info.weight));
                        ui.label(info.value.to_string());
                        ui.end_row();
                    }
                });
            });
            ui.separator();
            ui.label(format!("{} items, weight {weight:.1}", items.len()));
        };
        let container = match menu {
            Menu::Container(c) => Some(c),
            Menu::Inventory => None,
        };
        egui::Window::new("Inventory")
            .anchor(Align2::LEFT_CENTER, egui::vec2(40.0, 0.0))
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| list(ui, engine, PLAYER_REF, container, &mut moves));
        if let Some(c) = container {
            let name = engine.base_of(c).and_then(|b| engine.lo.get(b)).and_then(|r| r.get(b"FULL").map(|d| engine.lo.lstring(&r, d))).unwrap_or_default();
            egui::Window::new(if name.is_empty() { "Container".to_owned() } else { name })
                .anchor(Align2::RIGHT_CENTER, egui::vec2(-40.0, 0.0))
                .resizable(false)
                .collapsible(false)
                .show(ctx, |ui| {
                    list(ui, engine, c, Some(PLAYER_REF), &mut moves);
                    ui.horizontal(|ui| {
                        if ui.button("Take all").clicked() {
                            for (f, n, _) in engine.listed_inventory(c) {
                                moves.push((c, PLAYER_REF, f, n));
                            }
                        }
                        close |= ui.button("Close").clicked();
                    });
                });
        }
        for (from, to, item, n) in moves {
            engine.transfer_item(from, to, item, n);
        }
        if close {
            engine.menu = None;
        }
    }

    fn console_window(&mut self, ctx: &egui::Context, commands: &mut Vec<String>) {
        let rect = ctx.content_rect();
        egui::Area::new(egui::Id::new("console")).fixed_pos(Pos2::new(0.0, 0.0)).show(ctx, |ui| {
            let frame = egui::Frame::new().fill(Color32::from_rgba_unmultiplied(0, 0, 0, 200)).inner_margin(8.0);
            frame.show(ui, |ui| {
                ui.set_width(rect.width() - 16.0);
                ui.set_height(rect.height() * 0.4);
                egui::ScrollArea::vertical().stick_to_bottom(true).max_height(rect.height() * 0.4 - 30.0).show(ui, |ui| {
                    for l in &self.console.lines {
                        ui.label(egui::RichText::new(l).monospace().color(Color32::from_gray(220)));
                    }
                });
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.console.input)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY),
                );
                if self.console.focus {
                    resp.request_focus();
                    self.console.focus = false;
                }
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let cmd = std::mem::take(&mut self.console.input);
                    let cmd = cmd.trim().to_owned();
                    if !cmd.is_empty() {
                        self.console.history.push(cmd.clone());
                        commands.push(cmd);
                    }
                    resp.request_focus();
                }
            });
        });
    }

    /// Draw the UI on top of `target`.
    pub fn paint(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        output: egui::FullOutput,
        size: [u32; 2],
    ) {
        let ppp = output.pixels_per_point;
        let jobs = self.ctx.tessellate(output.shapes, ppp);
        for (id, deltas) in &output.textures_delta.set {
            for delta in deltas {
                self.renderer.update_texture(device, queue, *id, delta);
            }
        }
        let screen = egui_wgpu::ScreenDescriptor { size_in_pixels: size, pixels_per_point: ppp };
        let mut enc = device.create_command_encoder(&Default::default());
        let extra = self.renderer.update_buffers(device, queue, &mut enc, &jobs, &screen);
        {
            let pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let mut pass = pass.forget_lifetime();
            self.renderer.render(&mut pass, &jobs, &screen);
        }
        queue.submit(extra.into_iter().chain(std::iter::once(enc.finish())));
        for id in &output.textures_delta.free {
            self.renderer.free_texture(id);
        }
    }
}
