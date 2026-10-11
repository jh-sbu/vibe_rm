//! One running menu movie.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, anyhow};
use ruffle_core::backend::log::LogBackend;
use ruffle_core::backend::navigator::NullExecutor;
use ruffle_core::context::UpdateContext;
use ruffle_core::events::PlayerEvent;
use ruffle_core::external::{ExternalInterfaceProvider, Value};
use ruffle_core::limits::ExecutionLimit;
use ruffle_core::tag_utils::SwfMovie;
use ruffle_core::{FloatDuration, Player, PlayerBuilder, ViewportDimensions};
use ruffle_render_wgpu::backend::WgpuRenderBackend;
use ruffle_render_wgpu::target::TextureTarget;

use crate::UiSwf;
use crate::navigator::{BASE_URL, Navigator};

/// A call from the menu to the game: `GameDelegate.call(name, [args])`, which
/// reaches the game as `ExternalInterface.call(name, id, ...args)`.
#[derive(Clone, Debug)]
pub struct GameCall {
    pub name: String,
    /// The call's id: it wants an answer (`Menu::set_responder`).
    pub id: Option<f64>,
    pub args: Vec<Value>,
}

/// The movie as a responder sees it, in the middle of a call.
pub struct Movie<'a, 'gc> {
    context: &'a mut UpdateContext<'gc>,
}

impl Movie<'_, '_> {
    /// A variable's value (`_root.Menu_mc.List.selectedIndex`); objects come
    /// back as null.
    pub fn get(&mut self, path: &str) -> Value {
        ruffle_core::external::get_avm1_variable(self.context, path)
    }

    /// Sets a variable (`_root.List.entryList`).
    pub fn set(&mut self, path: &str, value: Value) {
        ruffle_core::external::set_avm1_variable(self.context, path, value)
    }

    /// Calls a function (`_root.List.entryList.push`), as `Menu::invoke`.
    pub fn invoke(&mut self, path: &str, args: &[Value]) -> Value {
        ruffle_core::external::invoke_avm1(self.context, path, args.iter().cloned())
    }
}

/// Answers calls that want one, while the menu waits: the arguments of the
/// answer, or None to leave the call unanswered (queued with the others).
pub type Responder = Box<dyn FnMut(&GameCall, &mut Movie<'_, '_>) -> Option<Vec<Value>> + Send>;

/// The game's side of `ExternalInterface.call`: answers the calls that want
/// an answer right away, as the game does (`GameDelegate.call` forgets a call
/// once it returns), and queues the rest.
struct Game {
    calls: Arc<Mutex<Vec<GameCall>>>,
    responder: Arc<Mutex<Option<Responder>>>,
}

impl ExternalInterfaceProvider for Game {
    fn call_method(&self, context: &mut UpdateContext<'_>, name: &str, args: &[Value]) -> Value {
        let (id, args) = match args.split_first() {
            Some((Value::Number(id), rest)) => (Some(*id), rest.to_vec()),
            _ => (None, args.to_vec()),
        };
        log::trace!("menu calls {name}({args:?})");
        let call = GameCall {
            name: name.to_string(),
            id,
            args,
        };
        // The responder's lock is let go before answering: the answer runs
        // the movie, which may call again.
        let answer = id.and_then(|_| {
            let mut responder = self.responder.lock().unwrap();
            responder.as_mut()?(&call, &mut Movie { context })
        });
        if let (Some(id), Some(answer)) = (id, answer)
            && let Some(respond) = context.external_interface.get_callback("respond")
        {
            let args = std::iter::once(Value::Number(id)).chain(answer);
            respond.call(context, "respond", args);
            return Value::Undefined;
        }
        self.calls.lock().unwrap().push(call);
        Value::Undefined
    }

    fn on_callback_available(&self, _name: &str) {}

    fn get_id(&self) -> Option<String> {
        None
    }
}

/// The movie's `trace` and ActionScript warnings, to the log.
struct Log(String);

impl LogBackend for Log {
    fn avm_trace(&self, message: &str) {
        log::debug!("{}: trace: {message}", self.0);
    }

    fn avm_warning(&self, message: &str) {
        log::debug!("{}: {message}", self.0);
    }
}

/// A menu movie (`hudmenu.swf`...) playing in Ruffle, drawn into a texture of
/// its own on the engine's device, for `Compositor` to draw over the frame.
pub struct Menu {
    name: String,
    player: Arc<Mutex<Player>>,
    executor: NullExecutor,
    /// Time not yet given to the movie: it's only ticked when a frame is due,
    /// as each tick costs a mouse hit-test and a GC step frame or not.
    pending: Duration,
    calls: Arc<Mutex<Vec<GameCall>>>,
    responder: Arc<Mutex<Option<Responder>>>,
}

impl Menu {
    pub(crate) fn open(
        ui: &UiSwf,
        name: &str,
        (width, height): (u32, u32),
    ) -> anyhow::Result<Self> {
        let data = ui
            .files
            .get(name)
            .with_context(|| format!("no interface file {name}"))?;
        let url = format!("{BASE_URL}{name}");
        let movie =
            SwfMovie::from_data(&data, url, None, None).map_err(|e| anyhow!("{name}: {e}"))?;
        let executor = NullExecutor::new();
        let target = TextureTarget::new_without_readback(&ui.descriptors.device, (width, height))
            .map_err(|e| anyhow!("{name}: {e}"))?;
        let renderer = WgpuRenderBackend::new(ui.descriptors.clone(), target)
            .map_err(|e| anyhow!("{name}: {e}"))?;
        let calls = Arc::new(Mutex::new(Vec::new()));
        let responder = Arc::new(Mutex::new(None));
        let player = PlayerBuilder::new()
            .with_renderer(renderer)
            .with_navigator(Navigator::new(ui.files.clone(), executor.spawner()))
            .with_log(Log(name.to_string()))
            .with_external_interface(Box::new(Game {
                calls: calls.clone(),
                responder: responder.clone(),
            }))
            .with_gamepad_button_mapping(crate::input::gamepad_key_codes())
            .with_movie(movie)
            .with_viewport_dimensions(width, height, 1.0)
            .with_autoplay(true)
            .build();
        {
            let mut p = player.lock().unwrap();
            ui.fonts.register(&mut p)?;
            p.set_translations(ui.translations.iter().cloned());
            // Drawn over the world: no stage background.
            p.set_window_mode("transparent");
        }
        let mut menu = Menu {
            name: name.to_string(),
            player,
            executor,
            pending: Duration::ZERO,
            calls,
            responder,
        };
        // The first frames, each once the imports it needs have loaded
        // (Scaleform loads them with the movie): classes are set up, menus
        // register their callbacks.
        for _ in 0..5 {
            menu.run_frame();
        }
        Ok(menu)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    fn run_frame(&mut self) {
        for _ in 0..4 {
            self.player
                .lock()
                .unwrap()
                .preload(&mut ExecutionLimit::none());
            self.executor.run();
        }
        self.player.lock().unwrap().run_frame();
        self.executor.run();
    }

    /// Lets `dt` pass: the movie runs a frame when one is due (at its own frame
    /// rate), and its loads complete.
    pub fn advance(&mut self, dt: Duration) {
        self.pending += dt;
        let mut player = self.player.lock().unwrap();
        if self.pending >= player.time_til_next_frame() {
            player.tick(FloatDuration::from_secs(self.pending.as_secs_f64()));
            self.pending = Duration::ZERO;
        }
        drop(player);
        self.executor.run();
    }

    /// Draws the movie into its texture if anything changed since last time.
    pub fn render(&mut self) {
        let mut player = self.player.lock().unwrap();
        if player.needs_render() {
            player.render();
        }
    }

    /// The texture the movie is drawn into (premultiplied alpha, `Rgba8Unorm`).
    pub fn texture(&self) -> wgpu::Texture {
        let mut player = self.player.lock().unwrap();
        let renderer = <dyn std::any::Any>::downcast_mut::<WgpuRenderBackend<TextureTarget>>(
            player.renderer_mut(),
        )
        .expect("a menu draws into a texture");
        renderer.target().get_texture()
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.player
            .lock()
            .unwrap()
            .set_viewport_dimensions(ViewportDimensions {
                width: width.max(1),
                height: height.max(1),
                scale_factor: 1.0,
            });
    }

    /// Calls the function at a path in the movie (`_root.HUDMovieBaseInstance.
    /// SetCompassAngle`), as the game does (Scaleform's Invoke).
    pub fn invoke(&self, path: &str, args: &[Value]) -> Value {
        self.player
            .lock()
            .unwrap()
            .invoke_avm1(path, args.iter().cloned())
    }

    /// Calls a handler the menu registered with `GameDelegate.addCallBack`.
    pub fn callback(&self, name: &str, args: &[Value]) -> Value {
        let args = std::iter::once(Value::String(name.to_string())).chain(args.iter().cloned());
        self.player
            .lock()
            .unwrap()
            .call_internal_interface("call", args)
    }

    /// Sets what answers the menu's calls that want an answer. The menu's
    /// first frames run on opening: calls it makes then aren't answered.
    pub fn set_responder(
        &self,
        responder: impl FnMut(&GameCall, &mut Movie<'_, '_>) -> Option<Vec<Value>> + Send + 'static,
    ) {
        *self.responder.lock().unwrap() = Some(Box::new(responder));
    }

    pub fn set(&self, path: &str, value: Value) {
        self.player.lock().unwrap().set_avm1_variable(path, value);
    }

    /// A variable's value; objects come back as null.
    pub fn get(&self, path: &str) -> Value {
        self.player.lock().unwrap().get_avm1_variable(path)
    }

    /// The menu's calls to the game since last asked.
    pub fn take_calls(&self) -> Vec<GameCall> {
        std::mem::take(&mut *self.calls.lock().unwrap())
    }

    /// Mouse, keys, text and gamepad, in the movie's viewport pixels.
    pub fn handle_event(&self, event: PlayerEvent) {
        self.player.lock().unwrap().handle_event(event);
    }

    /// Whether an input text field has the focus: keys then go as typed
    /// (`input::text_mode_key`), not as menu commands.
    pub fn typing(&self) -> bool {
        let Value::String(path) = self.invoke("Selection.getFocus", &[]) else {
            return false;
        };
        matches!(self.get(&format!("{path}.type")), Value::String(t) if t == "input")
    }
}
