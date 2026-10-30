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
        let answer = (|| {
            let mut responder = self.responder.lock().unwrap();
            responder.as_mut()?(&call, &mut Movie { context })
        })();
        if call.id.is_none() && answer.is_some() {
            return Value::Undefined;
        }
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

    fn call_host(&self, context: &mut UpdateContext<'_>, name: &str, args: &[Value]) -> Value {
        log::trace!("menu calls host {name}({args:?})");
        let call = GameCall {
            name: name.to_string(),
            id: None,
            args: args.to_vec(),
        };
        let answer = (|| {
            let mut responder = self.responder.lock().unwrap();
            responder.as_mut()?(&call, &mut Movie { context })
        })();
        match answer {
            Some(values) => values.into_iter().next().unwrap_or(Value::Undefined),
            None => {
                self.calls.lock().unwrap().push(call);
                Value::Undefined
            }
        }
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
    images: crate::HostImages,
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
            .with_navigator(Navigator::new(
                ui.files.clone(),
                executor.spawner(),
                ui.images.clone(),
            ))
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
            images: ui.images.clone(),
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

    pub fn set_host_image(&self, name: &str, width: u32, height: u32, rgba: &[u8]) {
        if width == 0 || height == 0 || rgba.len() != width as usize * height as usize * 4 {
            return;
        }
        let mut bytes = Vec::new();
        let encoded = (|| -> Result<(), png::EncodingError> {
            let mut encoder = png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.write_header()?.write_image_data(rgba)?;
            Ok(())
        })();
        if encoded.is_ok() {
            self.images.lock().unwrap().insert(name.into(), bytes);
        }
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

    /// Installs a code object at `path` (`_root.CodeObj`): the movie's calls
    /// to its methods reach the responder, or the queue, as calls named
    /// `<path>.<method>`; the responder's first value is the method's result.
    pub fn install_host_object(&self, path: &str, methods: &[&str]) -> bool {
        self.player
            .lock()
            .unwrap()
            .mutate_with_update_context(|context| {
                ruffle_core::external::install_avm1_host_object(context, path, methods)
            })
    }

    /// Rebind a live movie object/function after changing a timeline frame.
    pub fn alias(&self, target: &str, source: &str) -> bool {
        self.player
            .lock()
            .unwrap()
            .mutate_with_update_context(|context| {
                ruffle_core::external::alias_avm1_variable(context, target, source)
            })
    }

    /// Places a root-level clip's origin at viewport pixels, using the movie's
    /// actual stage transform (including scaling and letterbox offsets).
    pub fn position_clip(&self, path: &str, x: f64, y: f64) {
        self.player
            .lock()
            .unwrap()
            .mutate_with_update_context(|context| {
                let point = context.stage.inverse_view_matrix() * swf::Point::from_pixels(x, y);
                ruffle_core::external::set_avm1_variable(
                    context,
                    &format!("{path}._x"),
                    Value::Number(point.x.to_pixels()),
                );
                ruffle_core::external::set_avm1_variable(
                    context,
                    &format!("{path}._y"),
                    Value::Number(point.y.to_pixels()),
                );
            });
    }

    /// Viewport pixels as stage pixels, through the movie's actual stage
    /// transform (including scaling and letterbox offsets).
    pub fn stage_point(&self, x: f64, y: f64) -> (f64, f64) {
        self.player
            .lock()
            .unwrap()
            .mutate_with_update_context(|context| {
                let point = context.stage.inverse_view_matrix() * swf::Point::from_pixels(x, y);
                (point.x.to_pixels(), point.y.to_pixels())
            })
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

#[cfg(test)]
mod host_image_tests {
    use super::*;
    use swf::avm1::types::{Action, DefineFunction, Push, Value as AValue};
    fn string(s: &'static str) -> AValue<'static> {
        AValue::Str(swf::SwfStr::from_utf8_str(s))
    }
    fn push(values: Vec<AValue<'static>>) -> Action<'static> {
        Action::Push(Push { values })
    }
    fn bytecode(actions: &[Action<'_>]) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut w = swf::avm1::write::Writer::new(&mut bytes, 8);
            for a in actions {
                w.write_action(a).unwrap();
            }
        }
        bytes.push(0);
        bytes
    }
    #[test]
    fn host_image_loads_initializes_and_resizes_without_network() {
        // Authored synthetic AS2: onLoadInit remembers intrinsic dimensions
        // and sizes the target, as the journal's listener does.
        let body = bytecode(&[
            push(vec![string("_root.receivedWidth"), string("target")]),
            Action::GetVariable,
            push(vec![string("_width")]),
            Action::GetMember,
            Action::SetVariable,
            push(vec![string("_root.receivedHeight"), string("target")]),
            Action::GetVariable,
            push(vec![string("_height")]),
            Action::GetMember,
            Action::SetVariable,
            push(vec![string("target")]),
            Action::GetVariable,
            push(vec![string("_width"), AValue::Int(96)]),
            Action::SetMember,
            push(vec![string("target")]),
            Action::GetVariable,
            push(vec![string("_height"), AValue::Int(54)]),
            Action::SetMember,
        ]);
        let script = bytecode(&[
            Action::DefineFunction(DefineFunction {
                name: swf::SwfStr::from_utf8_str("onLoadInit"),
                params: vec![swf::SwfStr::from_utf8_str("target")],
                actions: &body,
            }),
            push(vec![
                string("loader"),
                AValue::Int(0),
                string("MovieClipLoader"),
            ]),
            Action::NewObject,
            Action::SetVariable,
            push(vec![string("_root")]),
            Action::GetVariable,
            push(vec![AValue::Int(1), string("loader")]),
            Action::GetVariable,
            push(vec![string("addListener")]),
            Action::CallMethod,
            Action::Pop,
            Action::Stop,
        ]);
        let mut data = Vec::new();
        swf::write_swf(
            &swf::Header {
                version: 8,
                ..swf::Header::default_with_swf_version(8)
            },
            &[swf::Tag::DoAction(&script), swf::Tag::ShowFrame],
            &mut data,
        )
        .unwrap();
        let movie = SwfMovie::from_data(&data, "file:///synthetic.swf".into(), None, None).unwrap();
        let executor = NullExecutor::new();
        let files = Arc::new(crate::InterfaceFiles::load(&vfs::Vfs::new(
            std::path::Path::new("/tmp/vrm-host-image-no-assets"),
            &[],
        )));
        let mut png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png, 2, 3);
            encoder.set_color(png::ColorType::Rgba);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&[255; 24])
                .unwrap();
        }
        let images: crate::HostImages = Default::default();
        images.lock().unwrap().insert("Probe".into(), png);
        let player = PlayerBuilder::new()
            .with_navigator(Navigator::new(files, executor.spawner(), images))
            .with_movie(movie)
            .with_autoplay(true)
            .build();
        let mut p = player.lock().unwrap();
        p.run_frame();
        p.invoke_avm1(
            "_root.createEmptyMovieClip",
            [Value::String("target".into()), Value::Number(1.)],
        );
        assert_eq!(
            p.invoke_avm1(
                "_root.loader.loadClip",
                [
                    Value::String("img://Probe".into()),
                    Value::String("_root.target".into())
                ]
            ),
            Value::Bool(true)
        );
        for _ in 0..8 {
            p.run_frame();
        }
        assert_eq!(
            p.get_avm1_variable("_root.receivedWidth"),
            Value::Number(2.)
        );
        assert_eq!(
            p.get_avm1_variable("_root.receivedHeight"),
            Value::Number(3.)
        );
        assert_eq!(
            p.get_avm1_variable("_root.target._width"),
            Value::Number(96.)
        );
        assert_eq!(
            p.get_avm1_variable("_root.target._height"),
            Value::Number(54.)
        );
    }

    #[test]
    fn host_object_methods_reach_the_responder() {
        let mut data = Vec::new();
        swf::write_swf(
            &swf::Header::default_with_swf_version(8),
            &[swf::Tag::ShowFrame],
            &mut data,
        )
        .unwrap();
        let movie = SwfMovie::from_data(&data, "file:///synthetic.swf".into(), None, None).unwrap();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let responder: Arc<Mutex<Option<Responder>>> = Arc::new(Mutex::new(Some(Box::new(
            |call: &GameCall, _: &mut Movie<'_, '_>| {
                (call.name == "_root.CodeObj.getScrollSpeed").then(|| vec![Value::Number(2.)])
            },
        ))));
        let player = PlayerBuilder::new()
            .with_external_interface(Box::new(Game {
                calls: calls.clone(),
                responder,
            }))
            .with_movie(movie)
            .with_autoplay(true)
            .build();
        let mut p = player.lock().unwrap();
        p.run_frame();
        assert!(p.mutate_with_update_context(|context| {
            ruffle_core::external::install_avm1_host_object(
                context,
                "_root.CodeObj",
                &["getScrollSpeed", "closeMenu"],
            )
        }));
        assert_eq!(
            p.invoke_avm1("_root.CodeObj.getScrollSpeed", []),
            Value::Number(2.)
        );
        p.invoke_avm1("_root.CodeObj.closeMenu", [Value::String("now".into())]);
        let queued = calls.lock().unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].name, "_root.CodeObj.closeMenu");
        assert_eq!(queued[0].args, vec![Value::String("now".into())]);
    }
}
