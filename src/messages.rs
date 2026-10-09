//! Messages (MESG): message boxes whose buttons scripts wait on, notifications,
//! help messages in the HUD, and the text replacement that messages and quest
//! text share (`<Alias=...>`, `<Global=...>`, `[Control]`, `%.0f`).

use std::collections::{HashSet, VecDeque};

use esp::FormId;
use papyrus::{NativeResult, Value};

use crate::engine::{Engine, PLAYER_REF};

/// A message box waiting for the player.
#[derive(Debug, Clone)]
pub struct MessageBox {
    pub title: String,
    pub text: String,
    /// The buttons whose conditions pass: (index among all the message's
    /// buttons, which `Show` returns; text).
    pub buttons: Vec<(i32, String)>,
    /// The script waiting in `Message.Show` (`Vm::signal_with`); none for
    /// `Debug.MessageBox`.
    pub key: Option<u64>,
}

/// A help message (`ShowAsHelpMessage`): shown for `duration` seconds every
/// `interval` seconds, `times_left` more times, until the player does `event`.
#[derive(Debug, Clone)]
struct HelpMessage {
    event: String,
    text: String,
    duration: f64,
    interval: f64,
    times_left: i32,
    next_at: f64,
    until: f64,
}

/// A line across the top of the HUD: a quest's name and what happened to it.
#[derive(Debug, Clone)]
pub struct Banner {
    pub title: String,
    pub subtitle: String,
    pub at: f64,
}

#[derive(Default)]
pub struct Messages {
    pub boxes: VecDeque<MessageBox>,
    help: Vec<HelpMessage>,
    /// Input events the player did while their help message was up; their help
    /// messages don't show again until `ResetHelpMessage`.
    done_events: HashSet<String>,
    pub banners: Vec<Banner>,
    next_key: u64,
}

/// How long a banner stays up.
pub const BANNER_SECONDS: f64 = 4.0;

/// The key or button a control (as message text names it in brackets) is
/// bound to here.
pub fn control_key(control: &str) -> Option<&'static str> {
    Some(match control.to_ascii_lowercase().as_str() {
        "forward" => "W",
        "back" => "S",
        "strafe left" => "A",
        "strafe right" => "D",
        "move" => "W, A, S, D",
        "look" => "Mouse",
        "activate" => "E",
        "jump" => "Space",
        "sprint" => "Left Shift",
        "sneak" => "Left Ctrl",
        "run" => "Left Alt",
        "tween menu" => "Tab",
        "journal" => "J",
        "right attack/block" => "Left Mouse Button",
        "left attack/block" => "Right Mouse Button",
        "accept" => "Enter",
        "cancel" => "Escape",
        _ => return None,
    })
}

/// The input event a key press is (`ShowAsHelpMessage`'s event names).
pub fn key_events(key: winit::keyboard::KeyCode) -> &'static [&'static str] {
    use winit::keyboard::KeyCode as K;
    match key {
        K::KeyW => &["Forward", "Move"],
        K::KeyS => &["Back", "Move"],
        K::KeyA => &["Strafe Left", "Move"],
        K::KeyD => &["Strafe Right", "Move"],
        K::KeyE => &["Activate"],
        K::Space => &["Jump"],
        K::ShiftLeft => &["Sprint"],
        K::ControlLeft => &["Sneak"],
        K::AltLeft => &["Run"],
        K::Tab => &["Tween Menu"],
        K::KeyJ => &["Journal"],
        _ => &[],
    }
}

/// Papyrus-style `printf` of `Show`'s float arguments: `%.0f`, `%g`, `%d`...;
/// `%%` is a percent sign.
pub fn format_floats(text: &str, args: &[f32]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut next = args.iter();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let mut spec = String::new();
        while let Some(&d) = chars.peek() {
            if d.is_ascii_digit() || matches!(d, '.' | '-' | '+' | ' ' | '#') {
                spec.push(d);
                chars.next();
            } else {
                break;
            }
        }
        let Some(conv) = chars.next() else {
            out.push('%');
            out.push_str(&spec);
            break;
        };
        let precision = spec
            .split_once('.')
            .and_then(|(_, p)| p.parse::<usize>().ok());
        match conv {
            '%' => out.push('%'),
            'f' | 'F' | 'e' | 'g' | 'G' | 'd' | 'i' | 'u' => {
                let v = next.next().copied().unwrap_or(0.0);
                match conv {
                    'd' | 'i' | 'u' => out.push_str(&(v as i64).to_string()),
                    'g' | 'G' => out.push_str(&v.to_string()),
                    _ => out.push_str(&format!("{v:.*}", precision.unwrap_or(6))),
                }
            }
            other => {
                out.push('%');
                out.push_str(&spec);
                out.push(other);
            }
        }
    }
    out
}

impl Engine {
    /// Replace the tags of message or quest text: `<Alias=Name>` (and its
    /// `.ShortName` / pronoun forms) by what quest `quest`'s alias holds,
    /// `<Global=EditorId>` by the global's value, `[Control]` by the key it is
    /// bound to.
    pub fn replace_text_tags(&mut self, text: &str, quest: Option<FormId>) -> String {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(i) = rest.find(['<', '[']) {
            out.push_str(&rest[..i]);
            rest = &rest[i..];
            let close = if rest.starts_with('<') { '>' } else { ']' };
            let Some(end) = rest.find(close) else { break };
            let inner = &rest[1..end];
            let replaced = if close == ']' {
                control_key(inner).map(str::to_owned)
            } else {
                self.text_tag(inner, quest)
            };
            match replaced {
                Some(s) => out.push_str(&s),
                None => out.push_str(&rest[..=end]),
            }
            rest = &rest[end + 1..];
        }
        out.push_str(rest);
        out
    }

    fn text_tag(&mut self, tag: &str, quest: Option<FormId>) -> Option<String> {
        let (kind, name) = tag.split_once('=')?;
        let (kind, form) = kind.split_once('.').unwrap_or((kind, ""));
        if kind.eq_ignore_ascii_case("global") {
            let g = self.lo.find_editor_id(name.trim())?;
            return Some(format!("{}", self.global_value(g).round() as i64));
        }
        if !kind.eq_ignore_ascii_case("alias") {
            return None;
        }
        let q = quest?;
        let alias = self
            .alias_specs(q)
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(name.trim()))?
            .id;
        let Some(r) = self.alias_ref(q, alias) else {
            return Some(String::new());
        };
        let female = || {
            self.templates_of(r)
                .and_then(|t| t.record(&self.lo, crate::world::template::TRAITS, b"ACBS"))
                .and_then(|rec| rec.get(b"ACBS").map(|d| d[0] & 1 != 0))
        };
        let pronoun = |he: &str, she: &str, it: &str| -> String {
            match female() {
                Some(true) => she,
                Some(false) => he,
                None => it,
            }
            .to_owned()
        };
        let lower = form.to_ascii_lowercase();
        let (base, cap) = match lower.strip_suffix("cap") {
            Some(b) => (b, true),
            None => (lower.as_str(), false),
        };
        let s = match base {
            "" => self.form_name(r),
            "shortname" => self.short_name(r),
            "pronoun" => pronoun("he", "she", "it"),
            "pronounobj" => pronoun("him", "her", "it"),
            "pronounpos" => pronoun("his", "her", "its"),
            "pronounposobj" => pronoun("his", "hers", "its"),
            "pronounref" => pronoun("himself", "herself", "itself"),
            _ => return None,
        };
        Some(if cap { capitalise(&s) } else { s })
    }

    /// An actor's short name (`SHRT`), else its name.
    fn short_name(&self, r: FormId) -> String {
        self.templates_of(r)
            .and_then(|t| t.record(&self.lo, crate::world::template::BASE_DATA, b"SHRT"))
            .and_then(|rec| rec.get(b"SHRT").map(|d| self.lo.lstring(&rec, d)))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.form_name(r))
    }

    /// `Message.Show`: a message box waits for the player's button (the calling
    /// script waits too); other messages show as notifications.
    pub fn show_message(&mut self, m: FormId, args: &[f32]) -> NativeResult {
        let Some(rec) = self.lo.get(m) else {
            return NativeResult::Value(Value::Int(0));
        };
        let message_box = rec.get(b"DNAM").is_some_and(|d| d[0] & 1 != 0);
        let quest = rec
            .get(b"QNAM")
            .filter(|d| d.len() >= 4)
            .map(|d| rec.fid(FormId(u32::from_le_bytes(d[..4].try_into().unwrap()))));
        let title = rec
            .get(b"FULL")
            .map(|d| self.lo.lstring(&rec, d))
            .unwrap_or_default();
        let text = rec
            .get(b"DESC")
            .map(|d| self.lo.lstring(&rec, d))
            .unwrap_or_default();
        // Buttons, each with the conditions that follow it.
        let mut buttons: Vec<(String, Vec<crate::condition::Condition>)> = Vec::new();
        for sr in rec.subrecords() {
            match &sr.tag.0 {
                b"ITXT" => buttons.push((self.lo.lstring(&rec, sr.data), Vec::new())),
                b"CTDA" => {
                    if let (Some(b), Some(c)) =
                        (buttons.last_mut(), crate::condition::parse(&rec, sr.data))
                    {
                        b.1.push(c);
                    }
                }
                b"CIS1" | b"CIS2" => {
                    if let Some(c) = buttons.last_mut().and_then(|b| b.1.last_mut()) {
                        let s = Some(sr.zstring().into());
                        if sr.tag.0 == *b"CIS1" {
                            c.string_p1 = s;
                        } else {
                            c.string_p2 = s;
                        }
                    }
                }
                _ => {}
            }
        }
        drop(rec);
        let text = self.replace_text_tags(&format_floats(&text, args), quest);
        if !message_box {
            if !text.is_empty() {
                self.scripts.notify(text);
            }
            return NativeResult::Value(Value::Int(0));
        }
        let title = self.replace_text_tags(&title, quest);
        let ctx = crate::condition::Context {
            subject: Some(PLAYER_REF),
            quest,
            ..Default::default()
        };
        let mut shown = Vec::new();
        for (i, (label, conds)) in buttons.into_iter().enumerate() {
            if crate::condition::evaluate(self, &conds, ctx) {
                shown.push((i as i32, self.replace_text_tags(&label, quest)));
            }
        }
        if shown.is_empty() {
            shown.push((0, self.gmst_string("sOk").unwrap_or_else(|| "Ok".into())));
        }
        self.messages.next_key += 1;
        let key = 0x4d45_5347_0000_0000 | self.messages.next_key;
        log::info!(
            "message box {m}: {text:?} [{}]",
            shown
                .iter()
                .map(|(i, s)| format!("{i}: {s}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        self.messages.boxes.push_back(MessageBox {
            title,
            text,
            buttons: shown,
            key: Some(key),
        });
        NativeResult::WaitFor {
            key,
            timeout: f32::MAX,
            value: Value::Int(0),
        }
    }

    /// `Debug.MessageBox`: a box with an Ok button no script waits on.
    pub fn debug_message_box(&mut self, text: String) {
        log::info!("message box: {text:?}");
        let ok = self.gmst_string("sOk").unwrap_or_else(|| "Ok".into());
        self.messages.boxes.push_back(MessageBox {
            title: String::new(),
            text,
            buttons: vec![(0, ok)],
            key: None,
        });
    }

    /// The player pressed button `index` (as `Show` returns it) of the message
    /// box in front: it closes and its script carries on with the index.
    pub fn choose_message_button(&mut self, index: i32) {
        let Some(b) = self.messages.boxes.pop_front() else {
            return;
        };
        log::info!("message box button {index}");
        if let Some(key) = b.key {
            self.vm.signal_with(key, Value::Int(index));
        }
    }

    /// `Message.ShowAsHelpMessage`. Nothing when the player already did the
    /// event while its help was up (until `ResetHelpMessage`).
    pub fn show_help_message(
        &mut self,
        m: FormId,
        event: &str,
        duration: f32,
        interval: f32,
        max_times: i32,
    ) {
        let event = event.to_ascii_lowercase();
        if self.messages.done_events.contains(&event) {
            return;
        }
        let Some(rec) = self.lo.get(m) else { return };
        let text = rec
            .get(b"DESC")
            .map(|d| self.lo.lstring(&rec, d))
            .unwrap_or_default();
        drop(rec);
        let text = self.replace_text_tags(&text, None);
        let now = self.scripts.real_time;
        log::info!("help message for {event:?}: {text:?}");
        // One help message per event.
        self.messages.help.retain(|h| h.event != event);
        self.messages.help.push(HelpMessage {
            event,
            text,
            duration: duration.max(0.0) as f64,
            interval: interval.max(0.0) as f64,
            times_left: max_times.max(1) - 1,
            next_at: now,
            until: now + duration.max(0.0) as f64,
        });
    }

    /// `Message.ResetHelpMessage`: the event's help can show again.
    pub fn reset_help_message(&mut self, event: &str) {
        self.messages
            .done_events
            .remove(&event.to_ascii_lowercase());
    }

    /// The player did an input event (`Activate`, `Jump`...): a help message
    /// waiting on it goes away for good.
    pub fn input_event(&mut self, event: &str) {
        let event = event.to_ascii_lowercase();
        let before = self.messages.help.len();
        self.messages.help.retain(|h| h.event != event);
        if self.messages.help.len() != before {
            log::info!("help message for {event:?} done");
            self.messages.done_events.insert(event);
        }
    }

    /// A menu or message box is up (the game's controls wait).
    pub fn menu_up(&self) -> bool {
        self.menu.is_some() || !self.messages.boxes.is_empty()
    }

    /// Menu mode: a menu that pauses the game is open (every one but dialogue's:
    /// CommonLibSSE's menus' `kPausesGame`). The world stands still; scripts
    /// run on, `Wait` waiting for it to end (`Utility.IsInMenuMode`).
    pub fn in_menu_mode(&self) -> bool {
        self.menu_up() || self.console_open
    }

    /// The help message showing now, if any (the latest).
    pub fn current_help(&self) -> Option<&str> {
        let now = self.scripts.real_time;
        self.messages
            .help
            .iter()
            .rev()
            .find(|h| now >= h.next_at && now < h.until)
            .map(|h| h.text.as_str())
    }

    /// Show a banner across the top of the HUD.
    pub fn banner(&mut self, title: String, subtitle: String) {
        log::info!("banner: {title} ({subtitle})");
        self.messages.banners.push(Banner {
            title,
            subtitle,
            at: self.scripts.real_time,
        });
    }

    /// Help messages come back after their interval; old banners go.
    pub(crate) fn update_messages(&mut self) {
        let now = self.scripts.real_time;
        self.messages.help.retain_mut(|h| {
            if now < h.until {
                return true;
            }
            if h.times_left <= 0 {
                return false;
            }
            if now >= h.until + h.interval {
                h.times_left -= 1;
                h.next_at = now;
                h.until = now + h.duration;
            }
            true
        });
        // Banners show one after another.
        if let Some(b) = self.messages.banners.first()
            && now - b.at >= BANNER_SECONDS
        {
            self.messages.banners.remove(0);
            if let Some(n) = self.messages.banners.first_mut() {
                n.at = now;
            }
        }
    }
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_format_like_printf() {
        assert_eq!(
            format_floats("%.0f gold, 20%% faster", &[150.4]),
            "150 gold, 20% faster"
        );
        assert_eq!(format_floats("%.2f and %.0f", &[1.0, 2.6]), "1.00 and 3");
        assert_eq!(format_floats("missing %.0f", &[]), "missing 0");
        assert_eq!(format_floats("100%", &[]), "100%");
    }

    #[test]
    fn controls_have_keys() {
        assert_eq!(control_key("Activate"), Some("E"));
        assert_eq!(control_key("Right Attack/Block"), Some("Left Mouse Button"));
        assert_eq!(control_key("XButton"), None);
    }
}
