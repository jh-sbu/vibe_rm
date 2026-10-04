//! In-game UI: HUD, activation prompt and the developer console (egui based).

use egui::{Align2, Color32, FontId, Pos2, Stroke};

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
}

impl Ui {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let ctx = egui::Context::default();
        let mut visuals = egui::Visuals::dark();
        visuals.window_fill = Color32::from_rgba_unmultiplied(10, 10, 10, 220);
        ctx.set_visuals(visuals);
        let renderer = egui_wgpu::Renderer::new(device, format, egui_wgpu::RendererOptions::default());
        Ui { ctx, renderer, console: Console::default(), show_debug: false, fps: 0 }
    }

    pub fn toggle_console(&mut self) {
        self.console.open = !self.console.open;
        self.console.focus = self.console.open;
    }

    /// Build this frame's UI.
    pub fn build(&mut self, raw: egui::RawInput, engine: &mut Engine) -> egui::FullOutput {
        let ctx = self.ctx.clone();
        let mut commands: Vec<String> = Vec::new();
        let out = ctx.run_ui(raw, |ui| {
            let ctx = ui.ctx().clone();
            self.hud(&ctx, engine);
            if self.console.open {
                self.console_window(&ctx, &mut commands);
            }
        });
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
