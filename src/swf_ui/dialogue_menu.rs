//! The dialogue menu as the game shows it (`dialoguemenu.swf`): the
//! speaker's name, what they say and the topics the player can choose.
//!
//! The game opens it with `InitExtensions` and `SetPlatform`, then talks to
//! it through its callbacks: `SetSpeakerName(name)`, `ShowDialogueText(text)`
//! and `NotifyVoiceReady` as a line starts (the menu takes a click as a skip
//! only 750 ms after), `HideDialogueText`, `PopulateDialogueList(text, new,
//! index, ..., selected)` and `ShowDialogueList(newList, hideExit)`, and
//! `StartHideMenu` when the conversation ends. The menu answers
//! `TopicClicked(index)`, `SkipText`, `CloseMenu` (on Tab or its exit
//! button, as it fades out) and `FadeDone` once faded. Choices:
//! `known_ambiguities/dialogue-menu.md`.

use ui_swf::{GameCall, Menu, UiSwf, Value};

use crate::engine::Engine;
use esp::FormId;

use super::{Buttons, num, text};

const M: &str = "_root.DialogueMenu_mc";

pub(super) struct DialogueMenu {
    pub menu: Menu,
    /// Who the conversation is with.
    pub speaker: FormId,
    /// Closing: it fades out, then calls `FadeDone`.
    pub closing: bool,
    /// The line shown (by when it ends), if one is.
    line: Option<f64>,
    /// The topics last listed, and whether they're up now (until one is
    /// clicked or a line starts).
    topics: Vec<FormId>,
    listed: bool,
    /// The topic last clicked, to keep selected when the list comes back.
    clicked: Option<FormId>,
    /// Whether the menu was told a voice is ready since the last click: it
    /// takes no clicks until it has been.
    ready: bool,
}

impl DialogueMenu {
    pub fn open(
        ui: &UiSwf,
        size: (u32, u32),
        engine: &Engine,
        buttons: std::sync::Arc<Buttons>,
    ) -> anyhow::Result<Self> {
        let menu = ui.open("dialoguemenu.swf", size)?;
        menu.set_responder(move |call, _| super::respond(&buttons, call));
        menu.invoke(&format!("{M}.InitExtensions"), &[]);
        menu.invoke(&format!("{M}.SetPlatform"), &[num(0.0), Value::Bool(false)]);
        let (speaker, name) = engine
            .conversation
            .as_ref()
            .map(|c| (c.npc_ref, c.name.clone()))
            .unwrap_or_default();
        menu.callback("SetSpeakerName", &[text(&name)]);
        Ok(DialogueMenu {
            menu,
            speaker,
            closing: false,
            line: None,
            topics: Vec::new(),
            listed: false,
            clicked: None,
            ready: false,
        })
    }

    /// Fades the menu out; it calls `CloseMenu`, then `FadeDone`.
    pub fn close(&mut self) {
        if !self.closing {
            self.closing = true;
            self.menu.callback("StartHideMenu", &[]);
        }
    }

    /// What's said, and the topics once the speaker is done.
    pub fn update(&mut self, engine: &Engine) {
        if self.closing {
            return;
        }
        let Some(c) = &engine.conversation else {
            self.close();
            return;
        };
        let line = c.current.as_ref().map(|l| l.ends_at);
        if line != self.line {
            match &c.current {
                Some(l) => {
                    self.menu.callback("ShowDialogueText", &[text(&l.text)]);
                    self.menu.callback("NotifyVoiceReady", &[]);
                    self.ready = true;
                    self.listed = false;
                }
                None => {
                    self.menu.callback("HideDialogueText", &[]);
                }
            }
            self.line = line;
        }
        if c.current.is_some() || c.ending || self.listed {
            return;
        }
        let topics: Vec<FormId> = c.options.iter().map(|(t, _, _)| t.id).collect();
        let selected = self
            .clicked
            .and_then(|id| topics.iter().position(|&t| t == id))
            .map_or(-1.0, |i| i as f32);
        let mut list: Vec<Value> = c
            .options
            .iter()
            .enumerate()
            .flat_map(|(i, (_, prompt, new))| [text(prompt), Value::Bool(*new), num(i as f32)])
            .collect();
        list.push(num(selected));
        self.menu.callback("PopulateDialogueList", &list);
        // A new list slides in, the clicked topic copied above it; the same
        // list again fades in.
        let new = topics != self.topics;
        self.menu
            .callback("ShowDialogueList", &[Value::Bool(new), Value::Bool(false)]);
        // No line was said (a greeting or topic without one).
        if !self.ready {
            self.menu.callback("NotifyVoiceReady", &[]);
            self.ready = true;
        }
        self.topics = topics;
        self.listed = true;
    }

    /// The menu's calls this answers; false for those it leaves to the
    /// menus' shared ones. True in `done` once it has faded out.
    pub fn answer(&mut self, engine: &mut Engine, call: &GameCall, done: &mut bool) -> bool {
        match call.name.as_str() {
            "TopicClicked" => {
                if let Some(Value::Number(i)) = call.args.first() {
                    let i = *i as usize;
                    self.clicked = engine
                        .conversation
                        .as_ref()
                        .and_then(|c| c.options.get(i))
                        .map(|(t, _, _)| t.id);
                    self.listed = false;
                    self.ready = false;
                    engine.choose_topic(i);
                }
            }
            "SkipText" => engine.skip_line(),
            "CloseMenu" => {
                self.closing = true;
                engine.end_conversation();
            }
            "FadeDone" => *done = true,
            _ => return false,
        }
        true
    }

    /// Esc is the game's Cancel: it skips the greeting, else closes, as Tab
    /// does through the menu.
    pub fn key(&self, code: winit::keyboard::KeyCode) -> bool {
        if code == winit::keyboard::KeyCode::Escape {
            self.menu.callback("Cancel", &[]);
            return true;
        }
        false
    }

    /// Before a mouse press: the menu skips every second `onMouseDown` it
    /// gets (`iMouseDownExecutionCount`), and it gets one a press here.
    pub fn mouse_down(&self) {
        self.menu
            .set("_global.DialogueMenu.iMouseDownExecutionCount", num(0.0));
    }
}
