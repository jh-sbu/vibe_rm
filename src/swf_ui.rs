//! The game's own menus (Interface `.swf` movies through `ui_swf`), driven from
//! the engine: for now the HUD (`hudmenu.swf`), in place of egui's.
//!
//! Each frame the HUD is told what changed: the health, magicka and stamina
//! meters, the compass heading, the location, what the crosshair is on, what's
//! said nearby, notifications, help messages, quest banners, the sneak eye and
//! the health of who the player looks at. Calls and paths are the HUD's own
//! (`_root.HUDMovieBaseInstance...`); how the game words some of them isn't
//! known (`known_gaps/swf-hud.md`).

use std::time::Duration;

use esp::FormId;
use esp::actor_value as av;
use ui_swf::{Menu, UiSwf, Value};

use crate::engine::{Engine, PLAYER_REF};

/// The HUD movie's root clip (`HUDMenu`).
const H: &str = "_root.HUDMovieBaseInstance";

pub struct SwfUi {
    pub ui: UiSwf,
    hud: Hud,
    buttons: Buttons,
}

impl SwfUi {
    /// Ruffle on the engine's device and the HUD, the size of the frame.
    pub fn new(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        engine: &Engine,
        format: wgpu::TextureFormat,
        size: (u32, u32),
    ) -> anyhow::Result<Self> {
        let r = &engine.renderer;
        let ui = UiSwf::new(
            instance,
            adapter,
            r.device.clone(),
            r.queue.clone(),
            format,
            &engine.vfs,
            "english",
        )?;
        let buttons = Buttons::load(&engine.vfs);
        let hud = Hud::open(&ui, size, &buttons)?;
        Ok(SwfUi { ui, hud, buttons })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.hud.menu.resize(width, height);
    }

    /// Tells the HUD what changed, lets `dt` pass in it and answers its calls.
    pub fn update(&mut self, engine: &mut Engine, dt: f32) {
        self.hud.update(engine);
        self.hud.menu.advance(Duration::from_secs_f32(dt.max(0.0)));
        for call in self.hud.menu.take_calls() {
            answer(&self.hud.menu, &self.buttons, engine, &call);
        }
    }

    /// Whether the HUD shows: not while a menu is up, as in the game.
    pub fn visible(engine: &Engine) -> bool {
        engine.menu.is_none()
    }

    /// Draws the HUD over `target` when `visible`.
    pub fn draw(&mut self, target: &wgpu::TextureView, visible: bool) {
        if !visible {
            return;
        }
        self.hud.menu.render();
        self.ui.composite(target, &[&self.hud.menu]);
    }
}

/// A menu's calls to the game (`GameDelegate.call`) that the engine answers.
fn answer(menu: &Menu, buttons: &Buttons, engine: &mut Engine, call: &ui_swf::GameCall) {
    let first = |i: usize| match call.args.get(i) {
        Some(Value::String(s)) => Some(s.as_str()),
        _ => None,
    };
    match call.name.as_str() {
        // A key's art for a user event ("Activate"), to show in its text.
        "GetButtonFromUserEvent" => {
            if let (Some(id), Some(event)) = (call.id, first(0)) {
                menu.respond(id, &[text(buttons.art(event))]);
            }
        }
        "PlaySound" => {
            if let Some(sound) = first(0) {
                let at = engine.camera.position;
                engine.play_sound_at(sound, at);
            }
        }
        _ => log::trace!("{} calls {}({:?})", menu.name(), call.name, call.args),
    }
}

/// Which key each user event is on (`interface/controls/pc/controlmap.txt`),
/// as the button art the menus export (`E.png`, `L-Shift.png`, `Mouse1.png`).
struct Buttons {
    /// Event name to art name, keyboard first, else the mouse.
    art: std::collections::HashMap<String, String>,
}

impl Buttons {
    fn load(vfs: &vfs::Vfs) -> Self {
        let text = vfs
            .read("interface/controls/pc/controlmap.txt")
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default();
        // Event: (keyboard, mouse) columns as written; `!0,Other Event` refers
        // to another event's key. The first context naming an event wins.
        let mut raw: std::collections::HashMap<String, (String, String)> = Default::default();
        for line in text.lines() {
            if line.starts_with("//") || line.trim().is_empty() {
                continue;
            }
            let cols: Vec<&str> = line.split('\t').filter(|c| !c.is_empty()).collect();
            if cols.len() >= 3 {
                raw.entry(cols[0].trim().to_string())
                    .or_insert((cols[1].trim().to_string(), cols[2].trim().to_string()));
            }
        }
        let resolve = |column: usize, event: &str| -> Option<String> {
            let mut event = event.to_string();
            for _ in 0..4 {
                let (keyboard, mouse) = raw.get(&event)?;
                let value = if column == 0 { keyboard } else { mouse };
                match value.strip_prefix("!0,") {
                    // The first event it refers to.
                    Some(other) => event = other.split(",!0,").next()?.to_string(),
                    None => {
                        // A combination ("0x2a+0x0f") shows its last key.
                        let code = value.rsplit('+').next()?.trim();
                        let code = u32::from_str_radix(code.trim_start_matches("0x"), 16).ok()?;
                        return if column == 0 {
                            keyboard_art(code)
                        } else {
                            mouse_art(code)
                        };
                    }
                }
            }
            None
        };
        let art = raw
            .keys()
            .filter_map(|e| Some((e.clone(), resolve(0, e).or_else(|| resolve(1, e))?)))
            .collect();
        let buttons = Buttons { art };
        log::debug!(
            "{} user events with key art (Activate {}, Tween Menu {}, Jump {}, Sprint {})",
            buttons.art.len(),
            buttons.art("Activate"),
            buttons.art("Tween Menu"),
            buttons.art("Jump"),
            buttons.art("Sprint"),
        );
        buttons
    }

    /// The art for a user event, `UnknownKey` when it has none.
    fn art(&self, event: &str) -> &str {
        self.art.get(event).map_or("UnknownKey", String::as_str)
    }
}

/// A keyboard key's art by its DirectInput scan code.
fn keyboard_art(code: u32) -> Option<String> {
    const LETTERS: &[(u32, &str)] = &[
        (0x10, "Q"),
        (0x11, "W"),
        (0x12, "E"),
        (0x13, "R"),
        (0x14, "T"),
        (0x15, "Y"),
        (0x16, "U"),
        (0x17, "I"),
        (0x18, "O"),
        (0x19, "P"),
        (0x1E, "A"),
        (0x1F, "S"),
        (0x20, "D"),
        (0x21, "F"),
        (0x22, "G"),
        (0x23, "H"),
        (0x24, "J"),
        (0x25, "K"),
        (0x26, "L"),
        (0x2C, "Z"),
        (0x2D, "X"),
        (0x2E, "C"),
        (0x2F, "V"),
        (0x30, "B"),
        (0x31, "N"),
        (0x32, "M"),
    ];
    const OTHERS: &[(u32, &str)] = &[
        (0x01, "Esc"),
        (0x0C, "Hyphen"),
        (0x0D, "Equal"),
        (0x0E, "Backspace"),
        (0x0F, "Tab"),
        (0x1A, "Bracketleft"),
        (0x1B, "Bracketright"),
        (0x1C, "Enter"),
        (0x1D, "L-Ctrl"),
        (0x27, "Semicolon"),
        (0x28, "Quotesingle"),
        (0x29, "Tilde"),
        (0x2A, "L-Shift"),
        (0x2B, "Backslash"),
        (0x33, "Comma"),
        (0x34, "Period"),
        (0x35, "Slash"),
        (0x36, "R-Shift"),
        (0x37, "NumPadMult"),
        (0x38, "L-Alt"),
        (0x39, "Space"),
        (0x3A, "CapsLock"),
        (0x46, "ScrollLock"),
        (0x49, "NumPad9"),
        (0x4A, "NumPadMinus"),
        (0x4E, "NumPadPlus"),
        (0x52, "NumPad0"),
        (0x53, "NumPadDec"),
        (0x57, "F11"),
        (0x58, "F12"),
        (0x9C, "Enter"),
        (0x9D, "R-Ctrl"),
        (0xB5, "NumPadDivide"),
        (0xB8, "R-Alt"),
        (0xC5, "Pause"),
        (0xC7, "Home"),
        (0xC8, "Up"),
        (0xC9, "PgUp"),
        (0xCB, "Left"),
        (0xCD, "Right"),
        (0xCF, "End"),
        (0xD0, "Down"),
        (0xD1, "PgDn"),
        (0xD2, "Insert"),
        (0xD3, "Delete"),
    ];
    let digit = (0x02..=0x0B)
        .contains(&code)
        .then(|| ((code - 1) % 10).to_string());
    let function = (0x3B..=0x44)
        .contains(&code)
        .then(|| format!("F{}", code - 0x3A));
    digit.or(function).or_else(|| {
        LETTERS
            .iter()
            .chain(OTHERS)
            .find(|(c, _)| *c == code)
            .map(|(_, n)| n.to_string())
    })
}

/// A mouse button's art: buttons 0..7 as Mouse1..8, then the wheel and
/// movement (the controlmap's mouse column; 0xff, none).
fn mouse_art(code: u32) -> Option<String> {
    match code {
        0..=7 => Some(format!("Mouse{}", code + 1)),
        8 | 9 => Some("Wheel".into()),
        0xA => Some("MouseMove".into()),
        _ => None,
    }
}

/// What the HUD was last told.
#[derive(Default)]
struct Sent {
    health: Option<f32>,
    magicka: Option<f32>,
    stamina: Option<f32>,
    heading: Option<f32>,
    location: Option<String>,
    /// The crosshair's rollover text (None inside: on nothing); None, not yet
    /// told (the HUD shows no crosshair until told).
    crosshair: Option<Option<String>>,
    subtitle: Option<String>,
    help: Option<String>,
    /// The newest notification shown (its time).
    notified: f64,
    /// The newest banner shown (its time).
    banner: f64,
    sneaking: bool,
    eye_frame: Option<i32>,
    sneak_text: Option<String>,
    enemy: Option<(FormId, i32)>,
    dialogue: bool,
}

struct Hud {
    menu: Menu,
    sent: Sent,
}

fn num(v: f32) -> Value {
    Value::Number(v as f64)
}

fn text(s: &str) -> Value {
    Value::String(s.to_string())
}

/// Text for an HTML field: `&`, `<` and `>` escaped.
fn html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

impl Hud {
    fn open(ui: &UiSwf, size: (u32, u32), buttons: &Buttons) -> anyhow::Result<Self> {
        let menu = ui.open("hudmenu.swf", size)?;
        // PC (keyboard and mouse art), every element, and no placeholder
        // subtitle ("Dialogue Line 1") until something is said.
        menu.invoke(
            &format!("{H}.SetPlatform"),
            &[Value::Number(0.0), Value::Bool(false)],
        );
        menu.invoke(
            &format!("{H}.ShowElements"),
            &[text("All"), Value::Bool(true)],
        );
        menu.invoke(&format!("{H}.HideSubtitle"), &[]);
        // The activate key's art beside the crosshair's verb.
        menu.invoke(
            &format!("{H}.RefreshActivateButtonArt"),
            &[text(buttons.art("Activate"))],
        );
        Ok(Hud {
            menu,
            sent: Sent::default(),
        })
    }

    fn call(&self, method: &str, args: &[Value]) -> Value {
        self.menu.invoke(&format!("{H}.{method}"), args)
    }

    fn update(&mut self, engine: &Engine) {
        self.meters(engine);
        self.compass(engine);
        self.crosshair(engine);
        self.subtitle(engine);
        self.notifications(engine);
        self.help(engine);
        self.banners(engine);
        self.sneak(engine);
        self.enemy(engine);
        let dialogue = engine.conversation.is_some();
        if dialogue != self.sent.dialogue {
            self.call(
                "ShowElements",
                &[text("DialogueMode"), Value::Bool(dialogue)],
            );
            self.sent.dialogue = dialogue;
        }
    }

    /// Health, magicka and stamina, as percents. A change is told without
    /// `abForce`, so the meter fades in, moves to it and fades out again.
    fn meters(&mut self, engine: &Engine) {
        for (index, method) in [
            (av::HEALTH, "SetHealthMeterPercent"),
            (av::MAGICKA, "SetMagickaMeterPercent"),
            (av::STAMINA, "SetStaminaMeterPercent"),
        ] {
            let percent = engine.actor_value_fraction(PLAYER_REF, index) * 100.0;
            let sent = match index {
                av::HEALTH => &mut self.sent.health,
                av::MAGICKA => &mut self.sent.magicka,
                _ => &mut self.sent.stamina,
            };
            if sent.is_none_or(|s| (s - percent).abs() > 0.05) {
                // The first time, a full meter is set without fading in.
                let force = sent.is_none() && percent >= 100.0;
                *sent = Some(percent);
                self.menu.invoke(
                    &format!("{H}.{method}"),
                    &[num(percent), Value::Bool(force)],
                );
            }
        }
    }

    /// The heading (degrees clockwise from north, as the camera's yaw) and the
    /// location's name.
    fn compass(&mut self, engine: &Engine) {
        let heading = engine.camera.yaw.to_degrees().rem_euclid(360.0);
        if self.sent.heading.is_none_or(|h| (h - heading).abs() > 0.01) {
            self.sent.heading = Some(heading);
            self.call(
                "SetCompassAngle",
                &[num(heading), num(heading), Value::Bool(true)],
            );
        }
        let location = engine.location_name();
        if self.sent.location.as_deref() != Some(location.as_str()) {
            self.call("SetLocationName", &[text(&location)]);
            self.sent.location = Some(location);
        }
    }

    /// What the crosshair is on: the verb over the name (red when it's a
    /// crime), as `SetCrosshairTarget`'s rollover text.
    fn crosshair(&mut self, engine: &Engine) {
        let rollover = engine.look_target.as_ref().map(|(_, name)| {
            let verb = html(engine.look_verb());
            let verb = if engine.look_verb_is_crime() {
                format!("<font color='#E64640'>{verb}</font>")
            } else {
                verb
            };
            format!("{verb}<br>{}", html(name))
        });
        if self.sent.crosshair.as_ref() == Some(&rollover) {
            return;
        }
        let args = match &rollover {
            // activate, text, show the button, text only, favor mode, show the
            // crosshair, then weight, value and a field (none here).
            Some(t) => vec![
                Value::Bool(true),
                text(t),
                Value::Bool(true),
                Value::Bool(false),
                Value::Bool(false),
                Value::Bool(true),
                Value::Undefined,
                Value::Undefined,
                Value::Undefined,
                Value::Undefined,
            ],
            None => vec![
                Value::Bool(false),
                text(""),
                Value::Bool(false),
                Value::Bool(false),
                Value::Bool(false),
                Value::Bool(true),
                Value::Undefined,
                Value::Undefined,
                Value::Undefined,
                Value::Undefined,
            ],
        };
        self.call("SetCrosshairTarget", &args);
        self.sent.crosshair = Some(rollover);
    }

    /// What an NPC nearby says by itself (a scene's line or a bark).
    fn subtitle(&mut self, engine: &Engine) {
        let mut lines: Vec<(&str, &str)> = engine
            .scene_lines()
            .into_iter()
            .map(|l| (l.name.as_str(), l.text.as_str()))
            .collect();
        lines.extend(
            engine
                .barks
                .current
                .as_ref()
                .map(|b| (b.name.as_str(), b.text.as_str())),
        );
        let line = lines
            .into_iter()
            .find(|(_, t)| !t.trim().is_empty())
            .map(|(name, line)| {
                if name.is_empty() {
                    line.to_string()
                } else {
                    format!("{name}: {line}")
                }
            });
        if line == self.sent.subtitle {
            return;
        }
        match &line {
            Some(l) => self.call("ShowSubtitle", &[text(l)]),
            None => self.call("HideSubtitle", &[]),
        };
        self.sent.subtitle = line;
    }

    /// Script notifications (`Debug.Notification`), each once, top left.
    fn notifications(&mut self, engine: &Engine) {
        let mut newest = self.sent.notified;
        for (message, at) in &engine.scripts.notifications {
            if *at > self.sent.notified {
                self.call("ShowMessage", &[text(message)]);
                newest = newest.max(*at);
            }
        }
        self.sent.notified = newest;
    }

    /// The help message up (`Message.ShowAsHelpMessage`), as a tutorial hint.
    fn help(&mut self, engine: &Engine) {
        let help = engine
            .current_help()
            .filter(|_| !engine.menu_up())
            .map(str::to_owned);
        if help == self.sent.help {
            return;
        }
        match &help {
            Some(h) => self.call("ShowTutorialHintText", &[text(h), Value::Bool(true)]),
            None => self.call("ShowTutorialHintText", &[text(""), Value::Bool(false)]),
        };
        self.sent.help = help;
    }

    /// Quest banners ("QUEST ADDED: BEFORE THE STORM"), one at a time as the
    /// HUD has room for them.
    fn banners(&mut self, engine: &Engine) {
        let Some(b) = engine
            .messages
            .banners
            .iter()
            .find(|b| b.at > self.sent.banner)
        else {
            return;
        };
        let q = format!("{H}.QuestUpdateBaseInstance");
        if self.menu.invoke(&format!("{q}.CanShowNotification"), &[]) != Value::Bool(true) {
            return;
        }
        // text, status, sound, objectives, type (QUEST_UPDATE), level, the
        // level meter's start and end, a shout's word.
        self.menu.invoke(
            &format!("{q}.ShowNotification"),
            &[
                text(&b.title),
                text(&b.subtitle),
                text(""),
                num(0.0),
                num(0.0),
                num(0.0),
                num(0.0),
                num(0.0),
                text(""),
            ],
        );
        self.sent.banner = b.at;
    }

    /// The sneak eye: it fades in while sneaking, opens as the player is
    /// noticed, and says [HIDDEN], [CAUTION] or [DETECTED].
    fn sneak(&mut self, engine: &Engine) {
        let meter = format!("{H}.StealthMeterInstance");
        let sneaking = engine.player.sneaking;
        if sneaking != self.sent.sneaking {
            let label = if sneaking { "FadeIn" } else { "FadeOut" };
            self.menu
                .invoke(&format!("{meter}.gotoAndPlay"), &[text(label)]);
            self.sent.sneaking = sneaking;
        }
        if !sneaking {
            return;
        }
        let open = engine.sneak_eye().clamp(0.0, 1.0);
        // The eye's frames run from closed ("invisible", 1) to open ("visible", 100).
        let frame = 1 + (open * 99.0).round() as i32;
        if self.sent.eye_frame != Some(frame) {
            self.menu.invoke(
                &format!("{meter}.SneakAnimInstance.gotoAndStop"),
                &[num(frame as f32)],
            );
            self.sent.eye_frame = Some(frame);
        }
        let (setting, default) = match open {
            o if o <= 0.0 => ("sSneakHidden", "[HIDDEN]"),
            o if o < 1.0 => ("sSneakCaution", "[CAUTION]"),
            _ => ("sSneakDetected", "[DETECTED]"),
        };
        let label = engine
            .gmst_string(setting)
            .unwrap_or_else(|| default.into());
        if self.sent.sneak_text.as_deref() != Some(label.as_str()) {
            self.menu.invoke(
                &format!("{meter}.SneakTextHolder.SneakTextClip.SneakTextInstance.SetText"),
                &[text(&label)],
            );
            self.sent.sneak_text = Some(label);
        }
    }

    /// The health of who the player looks at, while they're hurt and alive,
    /// with their name, under the compass.
    fn enemy(&mut self, engine: &Engine) {
        let target = engine.look_target.as_ref().and_then(|(r, name)| {
            let (h, max) = engine.actor_health(*r)?;
            (h < max && h > 0.0).then(|| (*r, name, (h / max * 100.0).round() as i32))
        });
        let e = format!("{H}.EnemyHealth_mc");
        match target {
            Some((r, name, percent)) => {
                if self.sent.enemy.is_none_or(|(sr, _)| sr != r) {
                    self.menu.set(&format!("{e}._alpha"), Value::Number(100.0));
                    self.menu.invoke(
                        &format!("{e}.BracketsInstance.RolloverNameInstance.SetText"),
                        &[text(name)],
                    );
                    self.call("EnemyHealthMeter.SetPercent", &[num(percent as f32)]);
                } else if self.sent.enemy.is_some_and(|(_, p)| p != percent) {
                    self.call("EnemyHealthMeter.SetTargetPercent", &[num(percent as f32)]);
                }
                self.sent.enemy = Some((r, percent));
            }
            None => {
                if self.sent.enemy.take().is_some() {
                    self.menu.set(&format!("{e}._alpha"), Value::Number(0.0));
                }
            }
        }
    }
}
