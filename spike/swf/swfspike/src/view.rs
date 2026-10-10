//! A window playing a menu in real time. The mouse, keyboard and gamepad go to
//! the movie (see `input`); some keys and the answers to the movie's calls play
//! the game's side, per menu (see `Driver`).
//! Timings go to the title bar and stdout each second.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::anyhow;
use ruffle_core::backend::navigator::NullExecutor;
use ruffle_core::events::{MouseButton, MouseWheelDelta};
use ruffle_core::external::Value;
use ruffle_core::{FloatDuration, Player, PlayerEvent, ViewportDimensions};
use ruffle_render_wgpu::backend::WgpuRenderBackend;
use ruffle_render_wgpu::wgpu;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

const QUALITIES: [&str; 4] = ["low", "medium", "high", "best"];

pub fn run(swf: &str, ops: &[String]) -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    let mut app = App {
        swf: swf.to_string(),
        ops: ops.to_vec(),
        state: None,
        error: None,
    };
    event_loop.run_app(&mut app)?;
    match app.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

struct App {
    swf: String,
    ops: Vec<String>,
    state: Option<State>,
    error: Option<anyhow::Error>,
}

struct State {
    window: Arc<Window>,
    player: Arc<Mutex<Player>>,
    executor: NullExecutor,
    last: Instant,
    /// Time not yet given to the player (`SPIKE_TICK_DUE`: ticks only when a
    /// movie frame is due).
    pending: Duration,
    held: HashSet<KeyCode>,
    cursor: (f64, f64),
    modifiers: winit::keyboard::ModifiersState,
    gamepads: Option<gilrs::Gilrs>,
    stick: crate::input::Stick,
    quality: usize,
    driver: Driver,
    stats: Stats,
}

#[derive(Default)]
struct Stats {
    since: Option<Instant>,
    frames: u32,
    /// Ticks that ran a movie frame (the movie's 24 fps), and their time.
    movie_frames: u32,
    movie_tick: f64,
    /// Ticks that didn't, and their time.
    idle_tick: f64,
    render: f64,
}

/// The game's side, per menu.
enum Driver {
    Hud(Hud),
    MessageBox(MessageBox),
    NameEntry(NameEntry),
    /// Any other menu: the mouse and the ops given on the command line.
    Other,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        match self.start(event_loop) {
            Ok(state) => self.state = Some(state),
            Err(e) => {
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(state) = &mut self.state else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                state
                    .player
                    .lock()
                    .unwrap()
                    .set_viewport_dimensions(ViewportDimensions {
                        width: size.width,
                        height: size.height,
                        scale_factor: 1.0,
                    });
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.cursor = (position.x, position.y);
                state.event(PlayerEvent::MouseMove {
                    x: position.x,
                    y: position.y,
                });
            }
            WindowEvent::CursorLeft { .. } => state.event(PlayerEvent::MouseLeave),
            WindowEvent::MouseInput {
                state: pressed,
                button,
                ..
            } => {
                let button = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    _ => MouseButton::Unknown,
                };
                let (x, y) = state.cursor;
                state.event(match pressed {
                    ElementState::Pressed => PlayerEvent::MouseDown {
                        x,
                        y,
                        button,
                        index: None,
                    },
                    ElementState::Released => PlayerEvent::MouseUp { x, y, button },
                });
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    MouseScrollDelta::LineDelta(_, y) => MouseWheelDelta::Lines(y.into()),
                    MouseScrollDelta::PixelDelta(p) => MouseWheelDelta::Pixels(p.y),
                };
                state.event(PlayerEvent::MouseWheel { delta });
            }
            WindowEvent::ModifiersChanged(m) => state.modifiers = m.state(),
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };
                // An input text field focused: the game's text input mode.
                let typing = crate::input::focus_is_text_input(&mut state.player.lock().unwrap());
                // To the movie, repeats included (CLIK reads held keys).
                let keys = if typing {
                    crate::input::text_mode_key(code).into_iter().collect()
                } else {
                    crate::input::keys(code)
                };
                for key in keys {
                    state.event(match event.state {
                        ElementState::Pressed => PlayerEvent::KeyDown { key },
                        ElementState::Released => PlayerEvent::KeyUp { key },
                    });
                }
                if typing && event.state == ElementState::Pressed {
                    let (shift, ctrl) =
                        (state.modifiers.shift_key(), state.modifiers.control_key());
                    if let Some(code) = crate::input::text_control(code, shift, ctrl) {
                        state.event(PlayerEvent::TextControl { code });
                    } else if !ctrl && let Some(text) = &event.text {
                        for codepoint in text.chars().filter(|c| !c.is_control()) {
                            state.event(PlayerEvent::TextInput { codepoint });
                        }
                    }
                }
                // While typing, only the function keys are the viewer's.
                let viewer_key = !typing || matches!(code, KeyCode::F2 | KeyCode::F10);
                if event.state == ElementState::Released {
                    state.held.remove(&code);
                    return;
                }
                state.held.insert(code);
                if !event.repeat && viewer_key {
                    match code {
                        KeyCode::F10 => event_loop.exit(),
                        KeyCode::KeyQ => {
                            state.quality = (state.quality + 1) % QUALITIES.len();
                            println!("quality {}", QUALITIES[state.quality]);
                            state.op(&format!("quality:{}", QUALITIES[state.quality]));
                        }
                        code => state.key(code),
                    }
                }
            }
            WindowEvent::RedrawRequested => state.frame(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}

impl App {
    fn start(&self, event_loop: &ActiveEventLoop) -> anyhow::Result<State> {
        let name = std::path::Path::new(&self.swf)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title(format!("swfspike {name}"))
                    .with_inner_size(winit::dpi::PhysicalSize::new(1280, 720)),
            )?,
        );
        let size = window.inner_size();
        // The window outlives the renderer: both live in `State`, the player dropped first.
        let renderer = unsafe {
            WgpuRenderBackend::for_window_unsafe(
                wgpu::SurfaceTargetUnsafe::from_window(&*window)?,
                (size.width, size.height),
                wgpu::Backends::all(),
                wgpu::PowerPreference::HighPerformance,
                Some(Box::new(window.clone())),
            )
        }
        .map_err(|e| anyhow!(e.to_string()))?;
        let (player, executor) =
            crate::make_player(Box::new(renderer), &self.swf, (size.width, size.height))?;
        player
            .lock()
            .unwrap()
            .set_background_color(Some(ruffle_core::Color {
                r: 40,
                g: 48,
                b: 56,
                a: 255,
            }));
        let quality = std::env::var("QUALITY").unwrap_or_else(|_| "high".into());
        let mut state = State {
            window,
            player,
            executor,
            last: Instant::now(),
            pending: Duration::ZERO,
            held: HashSet::new(),
            cursor: (0.0, 0.0),
            modifiers: Default::default(),
            gamepads: match gilrs::Gilrs::new() {
                Ok(g) => {
                    for (_, pad) in g.gamepads() {
                        println!("gamepad: {}", pad.name());
                    }
                    Some(g)
                }
                Err(e) => {
                    println!("no gamepad input: {e}");
                    None
                }
            },
            stick: Default::default(),
            quality: QUALITIES.iter().position(|q| *q == quality).unwrap_or(2),
            driver: match name.as_str() {
                "hudmenu" => Driver::Hud(Hud::default()),
                "messagebox" => Driver::MessageBox(MessageBox::default()),
                "racesex_menu" => Driver::NameEntry(NameEntry::default()),
                _ => Driver::Other,
            },
            stats: Stats::default(),
        };
        state.op(&format!("quality:{}", QUALITIES[state.quality]));
        println!(
            "input: mouse, keyboard and gamepad go to the movie. Menu keys: W/S/A/D or arrows move,\n       \
             E/Enter accept, Tab/Esc cancel; gamepad A/B, d-pad, left stick. Q cycles quality, F10 quits"
        );
        state.start_driver();
        for op in &self.ops {
            crate::apply(&state.player, op)?;
        }
        Ok(state)
    }
}

impl State {
    fn event(&self, event: PlayerEvent) {
        self.player.lock().unwrap().handle_event(event);
    }

    /// An op as `run` takes it (`call:path|arg|...`, `cb:name|arg|...`).
    fn op(&self, op: &str) {
        if let Err(e) = crate::apply(&self.player, op) {
            println!("{op}: {e}");
        }
    }

    fn ops(&self, ops: &[String]) {
        for op in ops {
            self.op(op);
        }
    }

    fn start_driver(&mut self) {
        let ops = match &mut self.driver {
            Driver::Hud(_) => {
                println!("{}", Hud::KEYS);
                Hud::start()
            }
            Driver::MessageBox(m) => {
                println!("{}", MessageBox::KEYS);
                m.open()
            }
            Driver::NameEntry(n) => {
                println!("{}", NameEntry::KEYS);
                n.start()
            }
            Driver::Other => vec![],
        };
        self.ops(&ops);
    }

    fn key(&mut self, code: KeyCode) {
        let ops = match &mut self.driver {
            Driver::Hud(h) => h.key(code),
            Driver::MessageBox(m) => m.key(code),
            Driver::NameEntry(n) => n.key(code),
            Driver::Other => vec![],
        };
        self.ops(&ops);
    }

    fn frame(&mut self) {
        let now = Instant::now();
        let dt = now - self.last;
        self.last = now;

        self.gamepads();

        let ops = match &mut self.driver {
            Driver::Hud(h) => h.frame(&self.held, dt),
            Driver::MessageBox(m) => m.frame(dt),
            Driver::NameEntry(n) => n.frame(dt),
            Driver::Other => vec![],
        };
        self.ops(&ops);

        let t0 = Instant::now();
        self.executor.run();
        self.pending += dt;
        let ran = {
            let mut p = self.player.lock().unwrap();
            let before = p.time_til_next_frame();
            let due = std::env::var_os("SPIKE_TICK_DUE").is_none() || self.pending >= before;
            let dt = std::mem::take(&mut self.pending);
            if due {
                p.tick(FloatDuration::from_secs(dt.as_secs_f64()));
            } else {
                self.pending = dt;
            }
            // The time to the next frame grew back: a frame ran.
            due && p.time_til_next_frame() > before.saturating_sub(dt)
        };
        self.executor.run();
        let t1 = Instant::now();
        self.player.lock().unwrap().render();
        let t2 = Instant::now();

        // What the movie asked of the game this frame.
        let calls = std::mem::take(&mut *crate::GAME_CALLS.lock().unwrap());
        for (name, args) in calls {
            let ops = match &mut self.driver {
                Driver::MessageBox(m) => m.game_call(&name, &args),
                Driver::NameEntry(n) => n.game_call(&name, &args),
                _ => vec![],
            };
            self.ops(&ops);
        }

        self.stats(now, ran, t0, t1, t2);
    }

    /// Gamepad buttons and the left stick to the movie.
    fn gamepads(&mut self) {
        let Some(gilrs) = &mut self.gamepads else {
            return;
        };
        let mut events = vec![];
        while let Some(gilrs::Event { event, .. }) = gilrs.next_event() {
            match event {
                gilrs::EventType::ButtonPressed(b, _) => {
                    events.extend(crate::input::gamepad_button(b).map(|b| (b, true)))
                }
                gilrs::EventType::ButtonReleased(b, _) => {
                    events.extend(crate::input::gamepad_button(b).map(|b| (b, false)))
                }
                gilrs::EventType::AxisChanged(axis, value, _) => {
                    events.extend(self.stick.axis(axis, value))
                }
                gilrs::EventType::Connected => println!("gamepad connected"),
                gilrs::EventType::Disconnected => println!("gamepad disconnected"),
                _ => {}
            }
        }
        for (button, down) in events {
            self.event(if down {
                PlayerEvent::GamepadButtonDown { button }
            } else {
                PlayerEvent::GamepadButtonUp { button }
            });
        }
    }

    fn stats(&mut self, now: Instant, ran: bool, t0: Instant, t1: Instant, t2: Instant) {
        let s = &mut self.stats;
        let since = *s.since.get_or_insert(now);
        s.frames += 1;
        if ran {
            s.movie_frames += 1;
            s.movie_tick += (t1 - t0).as_secs_f64();
        } else {
            s.idle_tick += (t1 - t0).as_secs_f64();
        }
        s.render += (t2 - t1).as_secs_f64();
        let elapsed = (now - since).as_secs_f64();
        if elapsed >= 1.0 {
            let n = s.frames as f64;
            let m = s.movie_frames.max(1) as f64;
            let idle = (s.frames - s.movie_frames).max(1) as f64;
            let line = format!(
                "{:.0} fps | movie frames {} at {:.2} ms, other ticks {:.3} ms | render+present {:.2} ms | quality {}{}",
                n / elapsed,
                s.movie_frames,
                s.movie_tick * 1000.0 / m,
                s.idle_tick * 1000.0 / idle,
                s.render * 1000.0 / n,
                QUALITIES[self.quality],
                if std::env::var_os("SPIKE_NOVSYNC").is_some() {
                    " (no vsync)"
                } else {
                    ""
                },
            );
            println!("{line}");
            let mut t = ruffle_core::SPIKE_TICK.lock().unwrap();
            println!(
                "  per tick: sockets {:.3} timers {:.3} update {:.3} (mouse {:.3} gc {:.3}) ms",
                t[0] * 1000.0 / n,
                t[1] * 1000.0 / n,
                t[2] * 1000.0 / n,
                t[3] * 1000.0 / n,
                t[4] * 1000.0 / n
            );
            *t = [0.0; 5];
            self.window.set_title(&format!("swfspike | {line}"));
            *s = Stats {
                since: Some(now),
                ..Stats::default()
            };
        }
    }
}

/// `call:` / `cb:` op text from a head and its arguments.
fn op(head: &str, args: &[&str]) -> String {
    std::iter::once(head)
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join("|")
}

/// The HUD: what the "game" last told it.
#[derive(Default)]
struct Hud {
    health: f64,
    magicka: f64,
    stamina: f64,
    angle: f64,
    message: usize,
    location: usize,
    subtitle: bool,
    crosshair: bool,
}

impl Hud {
    const KEYS: &str = "\
HUD keys: 1/2/3 drain health/magicka/stamina, R restore, Left/Right turn the compass,
          M message, S subtitle on/off, C crosshair target on/off, L location";
    const H: &str = "_root.HUDMovieBaseInstance";
    const MESSAGES: [&str; 4] = [
        "You have entered Riverwood",
        "Quest started: Before the Storm",
        "Skill increase: One-Handed",
        "You need a key to open this door",
    ];
    const LOCATIONS: [&str; 3] = ["Riverwood", "Whiterun", "Bleak Falls Barrow"];

    fn start() -> Vec<String> {
        vec![
            op(&format!("call:{}.ShowElements", Self::H), &["All", "true"]),
            op(
                &format!("call:{}.SetCompassAngle", Self::H),
                &["0", "0", "true"],
            ),
        ]
    }

    fn meter(name: &str, value: f64) -> String {
        op(
            &format!("call:{}.Set{name}MeterPercent", Self::H),
            &[&value.to_string(), "false"],
        )
    }

    fn key(&mut self, code: KeyCode) -> Vec<String> {
        let h = Self::H;
        match code {
            KeyCode::Digit1 => {
                self.health = (self.health - 15.0).max(0.0);
                vec![Self::meter("Health", self.health)]
            }
            KeyCode::Digit2 => {
                self.magicka = (self.magicka - 15.0).max(0.0);
                vec![Self::meter("Magicka", self.magicka)]
            }
            KeyCode::Digit3 => {
                self.stamina = (self.stamina - 15.0).max(0.0);
                vec![Self::meter("Stamina", self.stamina)]
            }
            KeyCode::KeyR => {
                (self.health, self.magicka, self.stamina) = (100.0, 100.0, 100.0);
                ["Health", "Magicka", "Stamina"]
                    .iter()
                    .map(|m| Self::meter(m, 100.0))
                    .collect()
            }
            KeyCode::KeyM => {
                let m = Self::MESSAGES[self.message];
                self.message = (self.message + 1) % Self::MESSAGES.len();
                vec![op(&format!("call:{h}.ShowMessage"), &[m])]
            }
            KeyCode::KeyL => {
                self.location = (self.location + 1) % Self::LOCATIONS.len();
                vec![op(
                    &format!("call:{h}.SetLocationName"),
                    &[Self::LOCATIONS[self.location]],
                )]
            }
            KeyCode::KeyS => {
                self.subtitle = !self.subtitle;
                if self.subtitle {
                    vec![op(
                        &format!("call:{h}.ShowSubtitle"),
                        &["Hey, you. You're finally awake."],
                    )]
                } else {
                    vec![op(&format!("call:{h}.HideSubtitle"), &[])]
                }
            }
            KeyCode::KeyC => {
                self.crosshair = !self.crosshair;
                let args: &[&str] = if self.crosshair {
                    &[
                        "true",
                        "Iron Sword",
                        "true",
                        "false",
                        "false",
                        "true",
                        "9",
                        "25",
                        "7",
                        "",
                    ]
                } else {
                    &[
                        "false", "", "false", "false", "false", "true", "0", "0", "0", "",
                    ]
                };
                vec![op(&format!("call:{h}.SetCrosshairTarget"), args)]
            }
            _ => vec![],
        }
    }

    /// Held arrows turn the player (and the compass) 90 degrees a second.
    fn frame(&mut self, held: &HashSet<KeyCode>, dt: Duration) -> Vec<String> {
        let turn = match (
            held.contains(&KeyCode::ArrowLeft),
            held.contains(&KeyCode::ArrowRight),
        ) {
            (true, false) => -1.0,
            (false, true) => 1.0,
            _ => return vec![],
        };
        self.angle = (self.angle + turn * 90.0 * dt.as_secs_f64()).rem_euclid(360.0);
        let a = format!("{:.2}", self.angle);
        vec![op(
            &format!("call:{}.SetCompassAngle", Self::H),
            &[&a, &a, "true"],
        )]
    }
}

/// The message box: questions as the game (or a script's `Message.Show`) asks
/// them; a click answers through `GameDelegate.call("buttonPress", [index])`.
#[derive(Default)]
struct MessageBox {
    question: usize,
    /// After an answer, the next question opens this much later.
    reopen: Option<Duration>,
}

struct Question {
    text: &'static str,
    buttons: &'static [&'static str],
}

impl MessageBox {
    const KEYS: &str = "\
message box: click a button, or move with A/D (arrows, d-pad, stick) and accept
             with E/Enter (gamepad A), cancel with Tab/Esc (gamepad B); Y/N answer
             Yes/No. The answer prints. F2 skips to the next question";
    const M: &str = "_root.MessageMenu";
    const QUESTIONS: [Question; 4] = [
        Question {
            text: "Do you want to wait here for 3 hours?",
            buttons: &["Yes", "No"],
        },
        Question {
            text: "This item is stolen. Sell it to the fence anyway?",
            buttons: &["Sell", "Keep it"],
        },
        Question {
            text: "The Greybeards have summoned you to High Hrothgar.<br>Which way will you go?",
            buttons: &["The Way of the Voice", "The Path of Knowledge", "Not yet"],
        },
        Question {
            text: "Fast travel to Whiterun?",
            buttons: &["Yes", "No"],
        },
    ];

    /// The game opening the box: the message, then the buttons, side by side
    /// as the game lays them out.
    fn open(&mut self) -> Vec<String> {
        let q = &Self::QUESTIONS[self.question];
        // The first button focused, for the keyboard and gamepad.
        let mut buttons = vec!["true"];
        buttons.extend_from_slice(q.buttons);
        vec![
            op(&format!("call:{}.SetMessage", Self::M), &[q.text, "true"]),
            op("cb:setButtons", &buttons),
        ]
    }

    fn next(&mut self) -> Vec<String> {
        self.question = (self.question + 1) % Self::QUESTIONS.len();
        self.open()
    }

    fn key(&mut self, code: KeyCode) -> Vec<String> {
        match code {
            KeyCode::F2 => self.next(),
            _ => vec![],
        }
    }

    fn game_call(&mut self, name: &str, args: &[Value]) -> Vec<String> {
        if name != "buttonPress" {
            return vec![];
        }
        // `GameDelegate.call` puts its response id first; the index is last.
        let index = match args.last() {
            Some(Value::Number(n)) => *n as usize,
            _ => return vec![],
        };
        let q = &Self::QUESTIONS[self.question];
        println!(
            "answered {:?}: {:?}",
            q.text,
            q.buttons.get(index).copied().unwrap_or("?")
        );
        self.reopen = Some(Duration::from_millis(800));
        vec![]
    }

    fn frame(&mut self, dt: Duration) -> Vec<String> {
        match self.reopen {
            Some(left) if left > dt => {
                self.reopen = Some(left - dt);
                vec![]
            }
            Some(_) => {
                self.reopen = None;
                self.next()
            }
            None => vec![],
        }
    }
}

/// Character creation's name popup (racesex_menu.swf loading TextEntry.swf):
/// the game opens it with `ShowTextEntry(true)` and `ShowTextEntryField`; the
/// menu asks for `SetAllowTextInput`, takes typed text, and answers
/// `ChangeName(name)` on Accept (then `ChangeName()` once it has faded out) or
/// `ChangeName()` on Cancel.
#[derive(Default)]
struct NameEntry {
    /// After a name, the popup opens again this much later.
    reopen: Option<Duration>,
}

impl NameEntry {
    const KEYS: &str = "\
name entry: type a name (Backspace, arrows, Home/End, Ctrl+A/C/V/X edit), Enter or
            the Accept button to accept, Tab or Cancel to cancel; F2 opens it again";
    const R: &str = "_root.RaceSexMenuBaseInstance.RaceSexPanelsInstance";

    /// The game opening the menu, then the popup.
    fn start(&mut self) -> Vec<String> {
        let r = Self::R;
        let mut ops = vec![
            op("call:_root.InitExtensions", &[]),
            // PC: keyboard and mouse button art.
            op("call:_root.SetPlatform", &["0", "false"]),
            op(&format!("call:{r}.SetNameText"), &["Prisoner"]),
            op(&format!("call:{r}.SetRaceText"), &["Nord"]),
        ];
        ops.extend(Self::open());
        ops
    }

    fn open() -> Vec<String> {
        vec![
            op("cb:ShowTextEntry", &["true"]),
            op("cb:ShowTextEntryField", &[]),
        ]
    }

    fn key(&mut self, code: KeyCode) -> Vec<String> {
        match code {
            KeyCode::F2 => Self::open(),
            _ => vec![],
        }
    }

    fn game_call(&mut self, name: &str, args: &[Value]) -> Vec<String> {
        match (name, args.get(1)) {
            ("SetAllowTextInput", _) => {
                println!("text input on");
                vec![]
            }
            ("ChangeName", Some(Value::String(new))) => {
                println!("named: {new:?}");
                self.reopen = Some(Duration::from_millis(1500));
                vec![op(&format!("call:{}.SetNameText", Self::R), &[new])]
            }
            ("ChangeName", _) => {
                println!("name entry closed");
                vec![]
            }
            _ => vec![],
        }
    }

    fn frame(&mut self, dt: Duration) -> Vec<String> {
        match self.reopen {
            Some(left) if left > dt => {
                self.reopen = Some(left - dt);
                vec![]
            }
            Some(_) => {
                self.reopen = None;
                Self::open()
            }
            None => vec![],
        }
    }
}
