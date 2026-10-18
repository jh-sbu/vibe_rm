//! The game's own menus (Interface `.swf` movies through `ui_swf`), driven from
//! the engine: the HUD (`hudmenu.swf`), message boxes (`messagebox.swf`) and
//! the item menus (the inventory, containers and pockets, bartering: in
//! `item_menu`), the magic menu (`magic_menu`), the favorites menu
//! (`favorites_menu`), the tween menu (`tween_menu`), the sleep / wait menu
//! (`sleep_wait_menu`), the training menu (`training_menu`), the journal
//! (`journal_menu`) and the dialogue menu (`dialogue_menu`), in place of
//! egui's.
//!
//! Each frame the HUD is told what changed: the health, magicka and stamina
//! meters, the compass heading, the location, what the crosshair is on, what's
//! said nearby, notifications, help messages, quest banners, the sneak eye and
//! the health of who the player looks at. Calls and paths are the HUD's own
//! (`_root.HUDMovieBaseInstance...`); how the game words some of them isn't
//! known (`known_ambiguities/swf-hud.md`).

mod dialogue_menu;
mod favorites_menu;
mod item_menu;
mod journal_menu;
mod magic_menu;
mod sleep_wait_menu;
mod training_menu;
mod tween_menu;

use std::time::Duration;

use esp::FormId;
use esp::actor_value as av;
use std::sync::Arc;

use ui_swf::{GameCall, Menu, UiSwf, Value};

use crate::engine::{Engine, PLAYER_REF};

/// The HUD movie's root clip (`HUDMenu`).
const H: &str = "_root.HUDMovieBaseInstance";

pub struct SwfUi {
    pub ui: UiSwf,
    hud: Hud,
    message_box: MessageBoxMenu,
    /// The inventory, while open.
    items: Option<item_menu::ItemMenu>,
    /// The item lists' positions, as the menu saved them on closing.
    item_indices: Vec<Value>,
    /// The favorite key pressed, and an equip button clicked (its slot), for
    /// the item menu's next update.
    favorite: bool,
    /// Ready Weapon in the magic menu: a dragon soul spent on the shout.
    unlock: bool,
    equip: Option<f32>,
    /// The magic menu, while open, and its lists' positions.
    magic: Option<magic_menu::MagicMenu>,
    magic_indices: Vec<Value>,
    /// The favorites menu, while open, the favorite selected when it
    /// closed, and a hotkey pressed (0 to 7) for its next update.
    favorites: Option<favorites_menu::FavoritesMenu>,
    favorites_last: Option<FormId>,
    hotkey: Option<usize>,
    /// The tween menu, while open.
    tween: Option<tween_menu::TweenMenu>,
    /// The sleep / wait menu, while open.
    sleep_wait: Option<sleep_wait_menu::SleepWaitMenu>,
    /// The training menu, while open.
    training: Option<training_menu::TrainingMenu>,
    /// The dialogue menu, while a conversation is on (and fading out after).
    dialogue: Option<dialogue_menu::DialogueMenu>,
    /// Whether the engine has a menu up (over a conversation, hiding it).
    menu_up: bool,
    /// The journal, while open, and where it was left.
    journal: Option<journal_menu::JournalMenu>,
    journal_saved: journal_menu::Saved,
    buttons: Arc<Buttons>,
    size: (u32, u32),
    /// The keyboard's modifiers and the cursor, for the menus' input.
    modifiers: winit::keyboard::ModifiersState,
    cursor: (f64, f64),
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
        let buttons = Arc::new(Buttons::load(&engine.vfs));
        let hud = Hud::open(&ui, size, &buttons)?;
        let message_box = MessageBoxMenu::open(&ui, size)?;
        for menu in [&hud.menu, &message_box.menu] {
            let buttons = buttons.clone();
            menu.set_responder(move |call, _| respond(&buttons, call));
        }
        Ok(SwfUi {
            ui,
            hud,
            message_box,
            items: None,
            item_indices: Vec::new(),
            favorite: false,
            unlock: false,
            equip: None,
            magic: None,
            magic_indices: Vec::new(),
            favorites: None,
            favorites_last: None,
            hotkey: None,
            tween: None,
            sleep_wait: None,
            training: None,
            dialogue: None,
            menu_up: false,
            journal: None,
            journal_saved: Default::default(),
            buttons,
            size,
            modifiers: Default::default(),
            cursor: (0.0, 0.0),
        })
    }

    /// A favorite slot from any keyboard binding in the installed control map.
    pub fn favorite_hotkey(&self, code: winit::keyboard::KeyCode) -> Option<usize> {
        self.buttons.hotkey(code)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.hud.menu.resize(width, height);
        self.message_box.menu.resize(width, height);
        if let Some(items) = &mut self.items {
            items.menu.resize(width, height);
        }
        if let Some(journal) = &mut self.journal {
            journal.menu.resize(width, height);
        }
        if let Some(magic) = &mut self.magic {
            magic.menu.resize(width, height);
        }
        if let Some(favorites) = &mut self.favorites {
            favorites.menu.resize(width, height);
        }
        if let Some(tween) = &mut self.tween {
            tween.menu.resize(width, height);
        }
        if let Some(sleep_wait) = &mut self.sleep_wait {
            sleep_wait.menu.resize(width, height);
        }
        if let Some(training) = &mut self.training {
            training.menu.resize(width, height);
        }
        if let Some(dialogue) = &mut self.dialogue {
            dialogue.menu.resize(width, height);
        }
        self.size = (width, height);
    }

    /// Tells the HUD what changed, lets `dt` pass in it and answers its calls.
    pub fn update(&mut self, engine: &mut Engine, dt: f32) {
        let dt = Duration::from_secs_f32(dt.max(0.0));
        self.hud.update(engine);
        self.hud.menu.advance(dt);
        for call in self.hud.menu.take_calls() {
            answer(&self.hud.menu, engine, &call);
        }
        self.item_menu(engine, dt);
        self.journal_menu(engine, dt);
        self.magic_menu(engine, dt);
        self.favorites_menu(engine, dt);
        self.tween_menu(engine, dt);
        self.sleep_wait_menu(engine, dt);
        self.training_menu(engine, dt);
        self.dialogue_menu(engine, dt);
        self.message_box.update(engine);
        self.message_box.menu.advance(dt);
        for call in self.message_box.menu.take_calls() {
            if !self.message_box.answer(engine, &call) {
                answer(&self.message_box.menu, engine, &call);
            }
        }
    }

    /// Opens the item menu the engine's menu is (the inventory, a container,
    /// bartering), closes it when it goes, and keeps it up to date.
    fn item_menu(&mut self, engine: &mut Engine, dt: Duration) {
        let Some(shown) = engine.menu.filter(|&m| item_menu::ItemMenu::shows(m)) else {
            self.items = None;
            return;
        };
        if self.items.as_ref().is_some_and(|m| m.shows != shown) {
            self.items = None;
        }
        if self.items.is_none() {
            let t = std::time::Instant::now();
            let buttons = self.buttons.clone();
            match item_menu::ItemMenu::open(
                &self.ui,
                self.size,
                engine,
                shown,
                &self.item_indices,
                buttons,
            ) {
                Ok(m) => {
                    log::debug!("{:?} up in {:?}", shown, t.elapsed());
                    self.items = Some(m);
                }
                Err(e) => {
                    log::error!("item menu: {e:#}");
                    engine.menu = None;
                    return;
                }
            }
        }
        let Some(items) = &mut self.items else { return };
        if std::mem::take(&mut self.favorite) {
            items.favorite_key(engine);
        }
        if let Some(slot) = self.equip.take() {
            items.equip_button(slot);
        }
        items.update(engine);
        items.menu.advance(dt);
        for call in items.menu.take_calls() {
            if !items.answer(engine, &call, &mut self.item_indices) {
                answer(&items.menu, engine, &call);
            }
        }
    }

    /// Opens the magic menu when the engine's menu is it, closes it when it
    /// goes, and keeps it up to date.
    fn magic_menu(&mut self, engine: &mut Engine, dt: Duration) {
        if engine.menu != Some(crate::items::Menu::Magic) {
            self.magic = None;
            return;
        }
        if self.magic.is_none() {
            let buttons = self.buttons.clone();
            match magic_menu::MagicMenu::open(
                &self.ui,
                self.size,
                engine,
                &self.magic_indices,
                buttons,
            ) {
                Ok(m) => self.magic = Some(m),
                Err(e) => {
                    log::error!("magic menu: {e:#}");
                    engine.menu = None;
                    return;
                }
            }
        }
        let Some(magic) = &mut self.magic else {
            return;
        };
        if std::mem::take(&mut self.favorite) {
            magic.favorite_key(engine);
        }
        if std::mem::take(&mut self.unlock) {
            magic.unlock_key(engine);
        }
        if let Some(slot) = self.equip.take() {
            magic.equip_button(slot);
        }
        magic.update(engine);
        magic.menu.advance(dt);
        for call in magic.menu.take_calls() {
            if !magic.answer(engine, &call, &mut self.magic_indices) {
                answer(&magic.menu, engine, &call);
            }
        }
    }

    /// Opens the favorites menu when the engine's menu is it, closes it when
    /// it goes, and keeps it up to date.
    fn favorites_menu(&mut self, engine: &mut Engine, dt: Duration) {
        if engine.menu != Some(crate::items::Menu::Favorites) {
            self.favorites = None;
            return;
        }
        if self.favorites.is_none() {
            let buttons = self.buttons.clone();
            match favorites_menu::FavoritesMenu::open(
                &self.ui,
                self.size,
                engine,
                self.favorites_last,
                buttons,
            ) {
                Ok(m) => self.favorites = Some(m),
                Err(e) => {
                    log::error!("favorites menu: {e:#}");
                    engine.menu = None;
                    return;
                }
            }
        }
        let Some(favorites) = &mut self.favorites else {
            return;
        };
        if let Some(key) = self.hotkey.take() {
            favorites.hotkey(engine, key);
        }
        if let Some(slot) = self.equip.take() {
            favorites.equip_button(engine, slot);
        }
        favorites.update(engine);
        favorites.menu.advance(dt);
        for call in favorites.menu.take_calls() {
            if !favorites.answer(engine, &call, &mut self.favorites_last) {
                answer(&favorites.menu, engine, &call);
            }
        }
    }

    /// Opens the tween menu when the engine's menu is it, closes it when it
    /// goes, and answers its calls.
    fn tween_menu(&mut self, engine: &mut Engine, dt: Duration) {
        if engine.menu != Some(crate::items::Menu::Tween) {
            self.tween = None;
            return;
        }
        if self.tween.is_none() {
            let buttons = self.buttons.clone();
            match tween_menu::TweenMenu::open(&self.ui, self.size, engine, buttons) {
                Ok(m) => self.tween = Some(m),
                Err(e) => {
                    log::error!("tween menu: {e:#}");
                    engine.menu = None;
                    return;
                }
            }
        }
        let Some(tween) = &mut self.tween else {
            return;
        };
        tween.menu.advance(dt);
        for call in tween.menu.take_calls() {
            if !tween.answer(engine, &call) {
                answer(&tween.menu, engine, &call);
            }
        }
    }

    /// Opens the sleep / wait menu when the engine's menu is it, closes it
    /// when it goes, and keeps its clock.
    fn sleep_wait_menu(&mut self, engine: &mut Engine, dt: Duration) {
        let Some(crate::items::Menu::SleepWait { sleeping }) = engine.menu else {
            self.sleep_wait = None;
            return;
        };
        if self.sleep_wait.is_none() {
            let buttons = self.buttons.clone();
            match sleep_wait_menu::SleepWaitMenu::open(
                &self.ui, self.size, engine, sleeping, buttons,
            ) {
                Ok(m) => self.sleep_wait = Some(m),
                Err(e) => {
                    log::error!("sleep / wait menu: {e:#}");
                    engine.menu = None;
                    return;
                }
            }
        }
        let Some(sleep_wait) = &mut self.sleep_wait else {
            return;
        };
        sleep_wait.update(engine);
        sleep_wait.menu.advance(dt);
        for call in sleep_wait.menu.take_calls() {
            if !sleep_wait.answer(engine, &call) {
                answer(&sleep_wait.menu, engine, &call);
            }
        }
    }

    /// Opens the training menu when the engine's menu is it, closes it when
    /// it goes, and keeps its card and meter.
    fn training_menu(&mut self, engine: &mut Engine, dt: Duration) {
        let Some(crate::items::Menu::Training(trainer)) = engine.menu else {
            self.training = None;
            return;
        };
        if self.training.as_ref().is_some_and(|t| t.trainer != trainer) {
            self.training = None;
        }
        if self.training.is_none() {
            let buttons = self.buttons.clone();
            match training_menu::TrainingMenu::open(&self.ui, self.size, engine, trainer, buttons) {
                Ok(m) => self.training = Some(m),
                Err(e) => {
                    log::error!("training menu: {e:#}");
                    engine.menu = None;
                    return;
                }
            }
        }
        let Some(training) = &mut self.training else {
            return;
        };
        training.menu.advance(dt);
        for call in training.menu.take_calls() {
            if !training.answer(engine, &call) {
                answer(&training.menu, engine, &call);
            }
        }
        training.update(engine);
    }

    /// Opens the dialogue menu when a conversation starts, fades it out when
    /// it ends, and answers its calls.
    fn dialogue_menu(&mut self, engine: &mut Engine, dt: Duration) {
        let speaker = engine.conversation.as_ref().map(|c| c.npc_ref);
        // Another conversation: a fresh menu.
        if speaker.is_some()
            && self
                .dialogue
                .as_ref()
                .is_some_and(|d| d.closing || Some(d.speaker) != speaker)
        {
            self.dialogue = None;
        }
        if self.dialogue.is_none() && speaker.is_some() {
            let buttons = self.buttons.clone();
            match dialogue_menu::DialogueMenu::open(&self.ui, self.size, engine, buttons) {
                Ok(m) => self.dialogue = Some(m),
                Err(e) => {
                    log::error!("dialogue menu: {e:#}");
                    engine.end_conversation();
                    return;
                }
            }
        }
        self.menu_up = engine.menu.is_some();
        let Some(dialogue) = &mut self.dialogue else {
            return;
        };
        dialogue.update(engine);
        dialogue.menu.advance(dt);
        let mut done = false;
        for call in dialogue.menu.take_calls() {
            if !dialogue.answer(engine, &call, &mut done) {
                answer(&dialogue.menu, engine, &call);
            }
        }
        if done {
            self.dialogue = None;
        }
    }

    /// Opens the journal when the engine's menu is it, closes it when it
    /// goes, and answers its calls.
    fn journal_menu(&mut self, engine: &mut Engine, dt: Duration) {
        if engine.menu != Some(crate::items::Menu::Journal) {
            self.journal = None;
            return;
        }
        if self.journal.is_none() {
            let buttons = self.buttons.clone();
            match journal_menu::JournalMenu::open(
                &self.ui,
                self.size,
                engine,
                &self.journal_saved,
                buttons,
            ) {
                Ok(m) => {
                    engine.play_sound_at("UIJournalOpen", engine.camera.position);
                    self.journal = Some(m);
                }
                Err(e) => {
                    log::error!("journal: {e:#}");
                    engine.menu = None;
                    return;
                }
            }
        }
        let Some(journal) = &mut self.journal else {
            return;
        };
        journal.update(engine);
        journal.menu.advance(dt);
        for call in journal.menu.take_calls() {
            if !journal.answer(engine, &call, &mut self.journal_saved) {
                answer(&journal.menu, engine, &call);
            }
        }
    }

    /// Whether the game's menus handle the engine's menu (egui doesn't draw
    /// it then).
    pub fn shows(menu: crate::items::Menu) -> bool {
        item_menu::ItemMenu::shows(menu)
            || matches!(
                menu,
                crate::items::Menu::Journal
                    | crate::items::Menu::Magic
                    | crate::items::Menu::Favorites
                    | crate::items::Menu::Tween
                    | crate::items::Menu::SleepWait { .. }
                    | crate::items::Menu::Training(_)
            )
    }

    /// Whether a menu over the world takes the keyboard and mouse.
    pub fn wants_input(&self) -> bool {
        self.message_box.shown.is_some()
            || self.items.is_some()
            || self.journal.is_some()
            || self.magic.is_some()
            || self.favorites.is_some()
            || self.tween.is_some()
            || self.sleep_wait.is_some()
            || self.training.is_some()
            || self.dialogue.as_ref().is_some_and(|d| !d.closing)
    }

    /// The window's input, to the menu on top while one takes it (the console
    /// key aside). True when a menu took it.
    pub fn window_event(&mut self, event: &winit::event::WindowEvent) -> bool {
        use ui_swf::{MouseButton, MouseWheelDelta, PlayerEvent};
        use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
        use winit::keyboard::{KeyCode, PhysicalKey};
        match event {
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::CursorMoved { position, .. } => self.cursor = (position.x, position.y),
            _ => {}
        }
        if !self.wants_input() {
            return false;
        }
        // The journal's own keys, unless a message box is over it.
        if let WindowEvent::KeyboardInput { event, .. } = event
            && event.state == ElementState::Pressed
            && !event.repeat
            && let PhysicalKey::Code(code) = event.physical_key
            && self.journal_key(code)
        {
            return true;
        }
        // The item and magic menus' own keys and buttons, unless a message
        // box is over them.
        let item_menu = self.message_box.shown.is_none()
            && (self.items.is_some() || self.magic.is_some() || self.favorites.is_some());
        // The other menus with keys of their own.
        let own_keys = item_menu
            || (self.message_box.shown.is_none()
                && (self.tween.is_some()
                    || self.sleep_wait.is_some()
                    || self.training.is_some()
                    || self.dialogue.is_some()));
        let closing = self.items.as_ref().is_some_and(|i| i.closing)
            || self.magic.as_ref().is_some_and(|m| m.closing)
            || self.favorites.as_ref().is_some_and(|f| f.closing);
        match event {
            WindowEvent::KeyboardInput { event, .. }
                if own_keys
                    && event.state == ElementState::Pressed
                    && !event.repeat
                    && let PhysicalKey::Code(code) = event.physical_key
                    && self.item_key(code) =>
            {
                return true;
            }
            // An attack button equips to its hand (the control map's
            // `LeftEquip` / `RightEquip`): `AttemptEquip`'s slot.
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button,
                ..
            } if item_menu && !closing => {
                let code = match button {
                    winit::event::MouseButton::Left => Some(0),
                    winit::event::MouseButton::Right => Some(1),
                    _ => None,
                };
                // Slot 0 is the right hand, 1 the left (`ContainerMenu.RIGHT_HAND`,
                // `LEFT_HAND`).
                if code.is_some() && code == self.buttons.mouse("Right Attack/Block") {
                    self.equip = Some(0.0);
                } else if code.is_some() && code == self.buttons.mouse("Left Attack/Block") {
                    self.equip = Some(1.0);
                }
            }
            _ => {}
        }
        let menu = self.top();
        let (x, y) = self.cursor;
        match event {
            WindowEvent::CursorMoved { .. } => menu.handle_event(PlayerEvent::MouseMove { x, y }),
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(dialogue) = &self.dialogue
                    && std::ptr::eq(menu, &dialogue.menu)
                    && *state == ElementState::Pressed
                {
                    dialogue.mouse_down();
                }
                let button = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    _ => MouseButton::Unknown,
                };
                menu.handle_event(match state {
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
                    MouseScrollDelta::LineDelta(_, y) => MouseWheelDelta::Lines((*y).into()),
                    MouseScrollDelta::PixelDelta(p) => MouseWheelDelta::Pixels(p.y),
                };
                menu.handle_event(PlayerEvent::MouseWheel { delta });
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return true;
                };
                if code == KeyCode::Backquote {
                    return false;
                }
                // An input field focused: the game's text input mode.
                let typing = menu.typing();
                let keys = if typing {
                    ui_swf::input::text_mode_key(code).into_iter().collect()
                } else {
                    ui_swf::input::keys(code)
                };
                for key in keys {
                    menu.handle_event(match event.state {
                        ElementState::Pressed => PlayerEvent::KeyDown { key },
                        ElementState::Released => PlayerEvent::KeyUp { key },
                    });
                }
                if typing && event.state == ElementState::Pressed {
                    let (shift, ctrl) = (self.modifiers.shift_key(), self.modifiers.control_key());
                    if let Some(code) = ui_swf::input::text_control(code, shift, ctrl) {
                        menu.handle_event(PlayerEvent::TextControl { code });
                    } else if !ctrl && let Some(text) = &event.text {
                        for codepoint in text.chars().filter(|c| !c.is_control()) {
                            menu.handle_event(PlayerEvent::TextInput { codepoint });
                        }
                    }
                }
            }
            _ => return false,
        }
        true
    }

    /// The menu taking input: a message box, else the item, journal, magic
    /// or favorites menu.
    fn top(&self) -> &Menu {
        if self.message_box.shown.is_some() {
            return &self.message_box.menu;
        }
        if let Some(items) = &self.items {
            &items.menu
        } else if let Some(journal) = &self.journal {
            &journal.menu
        } else if let Some(magic) = &self.magic {
            &magic.menu
        } else if let Some(favorites) = &self.favorites {
            &favorites.menu
        } else if let Some(tween) = &self.tween {
            &tween.menu
        } else if let Some(sleep_wait) = &self.sleep_wait {
            &sleep_wait.menu
        } else if let Some(training) = &self.training {
            &training.menu
        } else if let Some(dialogue) = &self.dialogue {
            &dialogue.menu
        } else {
            &self.message_box.menu
        }
    }

    /// The journal's own keys (none while a message box is over it): Esc and
    /// the journal key close it. True when the key was one of them.
    fn journal_key(&mut self, code: winit::keyboard::KeyCode) -> bool {
        use winit::keyboard::KeyCode;
        let (None, Some(journal)) = (&self.message_box.shown, &self.journal) else {
            return false;
        };
        if matches!(code, KeyCode::Escape | KeyCode::KeyJ) {
            journal.close();
            return true;
        }
        false
    }

    /// The item menu's own keys (none while a message box is over it): Esc
    /// closes it, the Ready Weapon key drops or takes all, Toggle POV marks a
    /// favorite (as the bottom bar shows them). True when the key was one of
    /// them.
    fn item_key(&mut self, code: winit::keyboard::KeyCode) -> bool {
        use winit::platform::scancode::PhysicalKeyExtScancode;
        // The dialogue menu, unless a menu is over it: Esc.
        if self.message_box.shown.is_none()
            && !self.menu_up
            && let Some(dialogue) = &self.dialogue
            && self.items.is_none()
            && self.training.is_none()
        {
            return dialogue.key(code);
        }
        // The training menu: its buttons' keys.
        if self.message_box.shown.is_none()
            && let Some(training) = &self.training
        {
            return training.key(code);
        }
        // The sleep / wait menu: Esc cancels.
        if self.message_box.shown.is_none()
            && let Some(sleep_wait) = &self.sleep_wait
        {
            if code == winit::keyboard::KeyCode::Escape {
                sleep_wait.close();
                return true;
            }
            return false;
        }
        // The tween menu: Esc folds it away as Tab does.
        if self.message_box.shown.is_none()
            && let Some(tween) = &mut self.tween
        {
            if tween.closing {
                return code != winit::keyboard::KeyCode::Backquote;
            } else if code == winit::keyboard::KeyCode::Escape {
                tween.close();
                return true;
            }
            return false;
        }
        if self.message_box.shown.is_none()
            && let Some(favorites) = &mut self.favorites
        {
            let scan = code.to_scancode();
            let hotkey = self.buttons.hotkey(code);
            if favorites.closing {
                return code != winit::keyboard::KeyCode::Backquote;
            } else if code == winit::keyboard::KeyCode::Escape
                || (scan.is_some() && scan == self.buttons.key("Favorites"))
            {
                favorites.close();
            } else if let Some(k) = hotkey {
                // Needs the engine: done with the next update.
                self.hotkey = Some(k);
            } else {
                return false;
            }
            return true;
        }
        if self.message_box.shown.is_none()
            && let Some(magic) = &mut self.magic
        {
            let scan = code.to_scancode();
            if magic.closing {
                return code != winit::keyboard::KeyCode::Backquote;
            } else if code == winit::keyboard::KeyCode::Escape {
                magic.close();
            } else if scan.is_some() && scan == self.buttons.key("Ready Weapon") {
                self.unlock = true;
            } else if scan.is_some() && scan == self.buttons.key("Toggle POV") {
                self.favorite = true;
            } else {
                return false;
            }
            return true;
        }
        let (None, Some(items)) = (&self.message_box.shown, &mut self.items) else {
            return false;
        };
        let scan = code.to_scancode();
        if items.closing {
            // Fading out: the keys do nothing more.
            return code != winit::keyboard::KeyCode::Backquote;
        } else if code == winit::keyboard::KeyCode::Escape && items.listing() {
            // The card's list of souls: Esc leaves it as Tab does.
            use ui_swf::PlayerEvent;
            for key in ui_swf::input::keys(winit::keyboard::KeyCode::Tab) {
                items.menu.handle_event(PlayerEvent::KeyDown { key });
                items.menu.handle_event(PlayerEvent::KeyUp { key });
            }
        } else if code == winit::keyboard::KeyCode::Escape {
            items.close();
        } else if items.listing() {
            return false;
        } else if scan.is_some() && scan == self.buttons.key("Wait") {
            items.charge_key();
        } else if scan.is_some() && scan == self.buttons.key("Ready Weapon") {
            items.x_button();
        } else if scan.is_some() && scan == self.buttons.key("Toggle POV") {
            // Needs the engine: done with the next update.
            self.favorite = true;
        } else {
            return false;
        }
        true
    }

    /// A key pressed and let go in the menu on top, as `window_event` would
    /// (`--menu-key`, for testing).
    pub fn press(&mut self, code: winit::keyboard::KeyCode) {
        use ui_swf::PlayerEvent;
        if !self.wants_input() || self.item_key(code) || self.journal_key(code) {
            return;
        }
        let menu = self.top();
        for key in ui_swf::input::keys(code) {
            menu.handle_event(PlayerEvent::KeyDown { key });
            menu.handle_event(PlayerEvent::KeyUp { key });
        }
    }

    /// A left click at (x, y) in the menu on top (`--menu-key click:x,y`, for
    /// testing).
    /// An attack button pressed in an item, magic or favorites menu (testing
    /// aid): `AttemptEquip`'s slot, 0 the right hand, 1 the left.
    pub fn attack_button(&mut self, slot: f32) {
        if self.message_box.shown.is_none()
            && (self.items.is_some() || self.magic.is_some() || self.favorites.is_some())
        {
            self.equip = Some(slot);
        }
    }

    pub fn click(&mut self, x: f64, y: f64) {
        use ui_swf::{MouseButton, PlayerEvent};
        if !self.wants_input() {
            return;
        }
        let menu = self.top();
        if let Some(dialogue) = &self.dialogue
            && std::ptr::eq(menu, &dialogue.menu)
        {
            dialogue.mouse_down();
        }
        let button = MouseButton::Left;
        menu.handle_event(PlayerEvent::MouseMove { x, y });
        menu.handle_event(PlayerEvent::MouseDown {
            x,
            y,
            button,
            index: None,
        });
        menu.handle_event(PlayerEvent::MouseUp { x, y, button });
    }

    /// Whether the HUD shows: not while a menu is up, as in the game.
    pub fn visible(engine: &Engine) -> bool {
        engine.menu.is_none()
    }

    /// Draws the HUD over `target` when `visible`.
    pub fn draw(&mut self, target: &wgpu::TextureView, visible: bool) {
        let mut menus = Vec::new();
        if visible {
            self.hud.menu.render();
            menus.push(&self.hud.menu);
        }
        if let Some(items) = &mut self.items {
            items.menu.render();
            menus.push(&items.menu);
        }
        if let Some(journal) = &mut self.journal {
            journal.menu.render();
            menus.push(&journal.menu);
        }
        if let Some(magic) = &mut self.magic {
            magic.menu.render();
            menus.push(&magic.menu);
        }
        if let Some(favorites) = &mut self.favorites {
            favorites.menu.render();
            menus.push(&favorites.menu);
        }
        if let Some(tween) = &mut self.tween {
            tween.menu.render();
            menus.push(&tween.menu);
        }
        if let Some(sleep_wait) = &mut self.sleep_wait {
            sleep_wait.menu.render();
            menus.push(&sleep_wait.menu);
        }
        // Under the menus a conversation opens (bartering, training), which
        // hide it.
        if let Some(dialogue) = &mut self.dialogue
            && !self.menu_up
        {
            dialogue.menu.render();
            menus.push(&dialogue.menu);
        }
        if let Some(training) = &mut self.training {
            training.menu.render();
            menus.push(&training.menu);
        }
        if self.message_box.shown.is_some() {
            self.message_box.menu.render();
            menus.push(&self.message_box.menu);
        }
        self.ui.composite(target, &menus);
    }
}

/// Answers to the calls every menu makes that want one, while the menu waits
/// (`Menu::set_responder`): a key's art for a user event ("Activate"), to show
/// in a text.
fn respond(buttons: &Buttons, call: &GameCall) -> Option<Vec<Value>> {
    match (call.name.as_str(), call.args.first()) {
        ("GetButtonFromUserEvent", Some(Value::String(event))) => {
            Some(vec![text(buttons.art(event))])
        }
        _ => None,
    }
}

/// A menu's calls to the game (`GameDelegate.call`) that the engine answers.
fn answer(menu: &Menu, engine: &mut Engine, call: &GameCall) {
    let first = |i: usize| match call.args.get(i) {
        Some(Value::String(s)) => Some(s.as_str()),
        _ => None,
    };
    match call.name.as_str() {
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
/// as the button art the menus export (`E.png`, `L-Shift.png`, `Mouse1.png`)
/// and as codes.
struct Buttons {
    /// Event name to art name, keyboard first, else the mouse.
    art: std::collections::HashMap<String, String>,
    /// Event name to its keyboard key (DirectInput scan code) and mouse
    /// button.
    codes: std::collections::HashMap<String, (Option<u32>, Option<u32>)>,
    /// All keyboard alternatives, including the favorites numpad bindings.
    keys: std::collections::HashMap<String, Vec<u32>>,
}

impl Buttons {
    fn load(vfs: &vfs::Vfs) -> Self {
        let text = vfs
            .read("interface/controls/pc/controlmap.txt")
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default();
        Self::from_controlmap(&text)
    }

    fn from_controlmap(text: &str) -> Self {
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
        let resolve = |column: usize, event: &str| -> Vec<u32> {
            let mut event = event.to_string();
            for _ in 0..4 {
                let Some((keyboard, mouse)) = raw.get(&event) else {
                    return Vec::new();
                };
                let value = if column == 0 { keyboard } else { mouse };
                match value.strip_prefix("!0,") {
                    // The first event it refers to.
                    Some(other) => {
                        event = other.split(",!0,").next().unwrap_or_default().to_string()
                    }
                    None => {
                        // Keep alternatives for input; button art uses the first.
                        // Chords retain the existing last-key interpretation.
                        return value
                            .split(',')
                            .filter_map(|alternative| {
                                let code = alternative.rsplit('+').next()?.trim();
                                u32::from_str_radix(code.trim_start_matches("0x"), 16)
                                    .ok()
                                    .filter(|&c| c != 0xff)
                            })
                            .collect();
                    }
                }
            }
            Vec::new()
        };
        let keys = raw.keys().map(|e| (e.clone(), resolve(0, e))).collect();
        let codes: std::collections::HashMap<_, _> = raw
            .keys()
            .map(|e| {
                (
                    e.clone(),
                    (
                        resolve(0, e).first().copied(),
                        resolve(1, e).first().copied(),
                    ),
                )
            })
            .collect();
        let art = codes
            .iter()
            .filter_map(|(e, (key, mouse))| {
                let art = key
                    .and_then(keyboard_art)
                    .or_else(|| mouse.and_then(mouse_art))?;
                Some((e.clone(), art))
            })
            .collect();
        let buttons = Buttons { art, codes, keys };
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

    fn hotkey(&self, code: winit::keyboard::KeyCode) -> Option<usize> {
        use winit::platform::scancode::PhysicalKeyExtScancode;
        let scan = code.to_scancode()?;
        (0..8).find(|k| {
            self.keys
                .get(&format!("Hotkey{}", k + 1))
                .is_some_and(|keys| keys.contains(&scan))
        })
    }

    /// A user event's keyboard key, as a DirectInput scan code.
    fn key(&self, event: &str) -> Option<u32> {
        self.codes.get(event).and_then(|c| c.0)
    }

    /// A user event's mouse button (0 the left).
    fn mouse(&self, event: &str) -> Option<u32> {
        self.codes.get(event).and_then(|c| c.1)
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

/// The message box in front (`Debug.MessageBox`, `Message.Show`), as the game
/// shows it: `messagebox.swf`, its text and its buttons side by side, answered
/// with the mouse or the keys.
struct MessageBoxMenu {
    menu: Menu,
    /// The box shown: its text and buttons (index for `Show`, label).
    shown: Option<(String, Vec<(i32, String)>)>,
}

impl MessageBoxMenu {
    const M: &str = "_root.MessageMenu";

    fn open(ui: &UiSwf, size: (u32, u32)) -> anyhow::Result<Self> {
        Ok(MessageBoxMenu {
            menu: ui.open("messagebox.swf", size)?,
            shown: None,
        })
    }

    /// Shows the box in front when it's a new one.
    fn update(&mut self, engine: &Engine) {
        let front = engine
            .messages
            .boxes
            .front()
            .map(|b| (b.text.replace("\r\n", "\n"), b.buttons.clone()));
        if front == self.shown {
            return;
        }
        if let Some((message, buttons)) = &front {
            let m = Self::M;
            self.menu.invoke(
                &format!("{m}.SetMessage"),
                &[text(message), Value::Bool(false)],
            );
            // The first button focused, for the keyboard; then the labels.
            let mut args = vec![Value::Bool(true)];
            args.extend(buttons.iter().map(|(_, label)| text(label)));
            self.menu.callback("setButtons", &args);
        }
        self.shown = front;
    }

    /// `buttonPress(position)`: the button there answers the box. False for
    /// other calls.
    fn answer(&mut self, engine: &mut Engine, call: &ui_swf::GameCall) -> bool {
        if call.name != "buttonPress" {
            return false;
        }
        let pressed = match call.args.last() {
            Some(Value::Number(n)) => *n as usize,
            _ => return true,
        };
        if let Some((_, buttons)) = &self.shown
            && let Some((index, _)) = buttons.get(pressed)
        {
            engine.choose_message_button(*index);
        }
        true
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
    compass_markers: Vec<crate::map_markers::CompassMarker>,
    /// The crosshair's rollover text (None inside: on nothing); None, not yet
    /// told (the HUD shows no crosshair until told).
    crosshair: Option<Option<String>>,
    subtitle: Option<String>,
    help: Option<String>,
    /// The newest notification shown (its sequence number).
    notified: u64,
    /// The newest banner shown (its time).
    banner: f64,
    sneaking: bool,
    eye_frame: Option<i32>,
    sneak_text: Option<String>,
    enemy: Option<(FormId, i32)>,
    mode: crate::player::HudMode,
    /// Magicka meter blinks shown.
    magicka_blinks: u32,
    shout: Option<f32>,
    shout_flashes: u32,
}

struct Hud {
    menu: Menu,
    sent: Sent,
}

/// The enemy name brackets' frame leaving room for a name `width` pixels
/// wide: frame 1 puts the inner knots at the middle, each frame after moves
/// them 0.7525 px further out, to 74.5 px either side at frame 100 (read from
/// `hudmenu.swf`), about the name field's 151 px (longer names shrink). The
/// game's own choice isn't public: the knots sit 4 px clear of the name.
fn bracket_frame(width: f32) -> f32 {
    const STEP: f32 = 74.4 / 99.0;
    (1.0 + (width / 2.0 + 4.0) / STEP).round().clamp(1.0, 100.0)
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
        let mode = engine.hud_mode();
        if mode != self.sent.mode {
            self.call(
                "ShowElements",
                &[text(self.sent.mode.movie_name()), Value::Bool(false)],
            );
            self.call(
                "ShowElements",
                &[text(mode.movie_name()), Value::Bool(true)],
            );
            self.sent.mode = mode;
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
        // Not enough magicka for a spell.
        if engine.casting.magicka_blinks != self.sent.magicka_blinks {
            self.sent.magicka_blinks = engine.casting.magicka_blinks;
            self.call("StartMagickaBlinking", &[]);
        }
        // The voice resting: the shout meter fills; it flashes when the
        // voice key is pressed too soon.
        let shout = engine.shout_meter_percent();
        if self.sent.shout.is_none_or(|s| (s - shout).abs() > 0.2) {
            self.sent.shout = Some(shout);
            self.call("SetShoutMeterPercent", &[num(shout), Value::Bool(false)]);
        }
        if engine.shouts.meter_flashes != self.sent.shout_flashes {
            self.sent.shout_flashes = engine.shouts.meter_flashes;
            self.call("FlashShoutMeter", &[]);
        }
    }

    /// The heading (degrees clockwise from north, as the camera's yaw), the
    /// markers (`CompassTargetDataA`: heading, alpha, frame and scale of
    /// each) and the location's name.
    fn compass(&mut self, engine: &Engine) {
        let markers = engine.compass_markers();
        let moved = markers != self.sent.compass_markers;
        if moved {
            let data = markers
                .iter()
                .flat_map(|m| [num(m.heading), num(m.alpha), num(m.frame), num(m.scale)])
                .collect();
            self.menu
                .set(&format!("{H}.CompassTargetDataA"), Value::List(data));
            self.call("SetCompassMarkers", &[]);
            self.sent.compass_markers = markers;
        }
        let heading = engine.camera.yaw.to_degrees().rem_euclid(360.0);
        // New markers are placed by the next angle.
        if moved || self.sent.heading.is_none_or(|h| (h - heading).abs() > 0.01) {
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
            let verb = html(&engine.look_verb());
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
        for notification in &engine.scripts.notifications {
            if notification.id > self.sent.notified {
                // The asset's ShowMessage accepts only text. Its animation
                // controls display duration; see known_ambiguities/messages.md.
                if notification.is_active(engine.scripts.real_time) {
                    self.call("ShowMessage", &[text(&notification.text)]);
                }
                newest = newest.max(notification.id);
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
                    let b = format!("{e}.BracketsInstance");
                    self.menu
                        .invoke(&format!("{b}.RolloverNameInstance.SetText"), &[text(name)]);
                    // The brackets' 100 frames part their inner knots from the
                    // middle to the field's edges (`bracket_frame`); the movie
                    // leaves them hidden and playing for the game to set.
                    let width = match self
                        .menu
                        .get(&format!("{b}.RolloverNameInstance.textWidth"))
                    {
                        Value::Number(n) => n as f32,
                        _ => 0.0,
                    };
                    self.menu
                        .invoke(&format!("{b}.gotoAndStop"), &[num(bracket_frame(width))]);
                    self.menu.set(&format!("{b}._alpha"), Value::Number(100.0));
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

/// A key by its winit name (`ArrowRight`, `KeyE`, `Escape`), for `--menu-key`.
pub fn key_code(name: &str) -> Option<winit::keyboard::KeyCode> {
    use winit::keyboard::KeyCode as K;
    const KEYS: &[K] = &[
        K::ArrowUp,
        K::ArrowDown,
        K::ArrowLeft,
        K::ArrowRight,
        K::Enter,
        K::Tab,
        K::Escape,
        K::Space,
        K::Home,
        K::End,
        K::PageUp,
        K::PageDown,
        K::KeyA,
        K::KeyB,
        K::KeyC,
        K::KeyD,
        K::KeyE,
        K::KeyF,
        K::KeyG,
        K::KeyH,
        K::KeyI,
        K::KeyJ,
        K::KeyK,
        K::KeyL,
        K::KeyM,
        K::KeyN,
        K::KeyO,
        K::KeyP,
        K::KeyQ,
        K::KeyR,
        K::KeyS,
        K::KeyT,
        K::KeyU,
        K::KeyV,
        K::KeyW,
        K::KeyX,
        K::KeyY,
        K::KeyZ,
        K::Digit1,
        K::Digit2,
        K::Digit3,
        K::Digit4,
        K::Digit5,
        K::Digit6,
        K::Digit7,
        K::Digit8,
        K::Numpad1,
        K::Numpad2,
        K::Numpad3,
        K::Numpad4,
        K::Numpad5,
        K::Numpad6,
        K::Numpad7,
        K::Numpad8,
    ];
    KEYS.iter().copied().find(|k| format!("{k:?}") == name)
}

#[cfg(test)]
mod input_tests {
    use super::*;
    use winit::keyboard::KeyCode as K;

    #[test]
    fn favorites_accept_alternatives_and_custom_bindings() {
        // Synthetic control map: a remapped slot, an alias, an unbound slot,
        // and a later context that must not replace the gameplay mapping.
        let buttons = Buttons::from_controlmap(
            "Hotkey1\t0x10,0x4f\t0xff\nHotkey2\t!0,Hotkey1\t0xff\n\
             Hotkey3\t0xff\t0xff\nHotkey1\t0x03\t0xff\n",
        );
        assert_eq!(buttons.hotkey(K::KeyQ), Some(0));
        assert_eq!(buttons.hotkey(K::Numpad1), Some(0));
        assert_eq!(buttons.hotkey(K::Digit1), None);
        assert_eq!(buttons.hotkey(K::Digit2), None);
        assert_eq!(buttons.hotkey(K::Numpad3), None);
        assert_eq!(buttons.art("Hotkey1"), "Q");
        assert_eq!(buttons.keys["Hotkey2"], buttons.keys["Hotkey1"]);
        assert!(buttons.keys["Hotkey3"].is_empty());
    }

    #[test]
    fn enemy_brackets_part_for_the_name_within_their_frames() {
        assert_eq!(bracket_frame(0.0), 6.0);
        // "Wolf" (32 px): knots 20 px out.
        assert_eq!(bracket_frame(32.0), 28.0);
        assert!(bracket_frame(64.0) > bracket_frame(32.0));
        assert_eq!(bracket_frame(151.4), 100.0);
        assert_eq!(bracket_frame(1000.0), 100.0);
    }

    #[test]
    fn offscreen_input_names_include_numpad() {
        assert_eq!(key_code("Numpad1"), Some(K::Numpad1));
        assert_eq!(key_code("Numpad8"), Some(K::Numpad8));
    }
}
