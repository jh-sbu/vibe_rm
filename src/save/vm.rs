//! The Papyrus VM in a save: scripts attached with their variables, the
//! threads waiting, and what the engine registered for them (timers,
//! `OnInit` done, sleep and animation event listeners, inventory filters).
//! Arrays are shared by identity in the VM, so they are saved once in a table
//! (`VMAR`) and referred to by index; `VM` holds the rest.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

use esp::FormId;
use papyrus::{Arg, ObjectId, SavedFrame, SavedThread, SavedVm, Value};

use super::codec::{Corrupt, Reader, Result, Writer};
use super::file::SaveFile;
use crate::script::Timer;

/// Everything of the scripting side a save keeps.
#[derive(Default)]
pub struct VmSnapshot {
    pub vm: SavedVm,
    pub initialized: Vec<ObjectId>,
    pub timers: Vec<Timer>,
    pub sleep_listeners: Vec<ObjectId>,
    pub inventory_filters: Vec<(ObjectId, Vec<FormId>)>,
    pub anim_registrations: Vec<(ObjectId, String, FormId, String)>,
    pub message_boxes: VecDeque<crate::messages::MessageBox>,
    pub message_next_key: u64,
    /// (runtime wait key, sender, canonical event). `ANMW` encodes these as
    /// semantic descriptors tied to the filtered VM thread order.
    pub animation_waits: Vec<(u64, FormId, String)>,
}

/// Arrays met while writing, by identity.
#[derive(Default)]
struct ArrayTable {
    ids: HashMap<usize, u32>,
    arrays: Vec<papyrus::Array>,
}

impl ArrayTable {
    fn id(&mut self, a: &papyrus::Array) -> u32 {
        let key = Rc::as_ptr(a) as usize;
        *self.ids.entry(key).or_insert_with(|| {
            self.arrays.push(a.clone());
            self.arrays.len() as u32 - 1
        })
    }
}

fn put_object(w: &mut Writer, id: ObjectId) {
    match id {
        ObjectId::Form(f) => {
            w.u8(0);
            w.u32(f);
        }
        ObjectId::Alias { quest, alias } => {
            w.u8(1);
            w.u32(quest);
            w.u32(alias);
        }
        ObjectId::Effect(n) => {
            w.u8(2);
            w.u64(n);
        }
    }
}

fn get_object(r: &mut Reader) -> Result<ObjectId> {
    match r.u8()? {
        0 => Ok(ObjectId::Form(r.u32()?)),
        1 => Ok(ObjectId::Alias {
            quest: r.u32()?,
            alias: r.u32()?,
        }),
        2 => Ok(ObjectId::Effect(r.u64()?)),
        n => Err(Corrupt(format!("object kind {n}"))),
    }
}

fn put_value(w: &mut Writer, v: &Value, arrays: &mut ArrayTable) {
    match v {
        Value::None => w.u8(0),
        Value::Bool(b) => {
            w.u8(1);
            w.bool(*b);
        }
        Value::Int(i) => {
            w.u8(2);
            w.i32(*i);
        }
        Value::Float(f) => {
            w.u8(3);
            w.f32(*f);
        }
        Value::String(s) => {
            w.u8(4);
            w.str(s);
        }
        Value::Object(id, class) => {
            w.u8(5);
            put_object(w, *id);
            w.str(class);
        }
        Value::Array(a) => {
            w.u8(6);
            w.u32(arrays.id(a));
        }
    }
}

fn get_value(r: &mut Reader, arrays: &[papyrus::Array]) -> Result<Value> {
    Ok(match r.u8()? {
        0 => Value::None,
        1 => Value::Bool(r.bool()?),
        2 => Value::Int(r.i32()?),
        3 => Value::Float(r.f32()?),
        4 => Value::String(Arc::from(r.str()?)),
        5 => {
            let id = get_object(r)?;
            Value::Object(id, Arc::from(r.str()?))
        }
        6 => {
            let i = r.u32()? as usize;
            Value::Array(
                arrays
                    .get(i)
                    .ok_or_else(|| Corrupt(format!("array {i} of {}", arrays.len())))?
                    .clone(),
            )
        }
        n => return Err(Corrupt(format!("value kind {n}"))),
    })
}

fn put_arg(w: &mut Writer, a: &Arg, arrays: &mut ArrayTable) {
    match a {
        Arg::Lit(v) => {
            w.u8(0);
            put_value(w, v, arrays);
        }
        Arg::Local(i) => {
            w.u8(1);
            w.u32(*i as u32);
        }
        Arg::Member(name) => {
            w.u8(2);
            w.str(name);
        }
        Arg::SelfRef => w.u8(3),
        Arg::State => w.u8(4),
        Arg::Discard => w.u8(5),
    }
}

fn get_arg(r: &mut Reader, arrays: &[papyrus::Array]) -> Result<Arg> {
    Ok(match r.u8()? {
        0 => Arg::Lit(get_value(r, arrays)?),
        1 => Arg::Local(r.u32()? as usize),
        2 => Arg::Member(Arc::from(r.str()?)),
        3 => Arg::SelfRef,
        4 => Arg::State,
        5 => Arg::Discard,
        n => return Err(Corrupt(format!("operand kind {n}"))),
    })
}

fn put_opt_arg(w: &mut Writer, a: &Option<Arg>, arrays: &mut ArrayTable) {
    w.bool(a.is_some());
    if let Some(a) = a {
        put_arg(w, a, arrays);
    }
}

fn get_opt_arg(r: &mut Reader, arrays: &[papyrus::Array]) -> Result<Option<Arg>> {
    Ok(if r.bool()? {
        Some(get_arg(r, arrays)?)
    } else {
        None
    })
}

/// An order for objects, so equal states make equal files.
fn order(o: &ObjectId) -> (u8, u64, u64) {
    match *o {
        ObjectId::Form(f) => (0, f as u64, 0),
        ObjectId::Alias { quest, alias } => (1, quest as u64, alias as u64),
        ObjectId::Effect(n) => (2, n, 0),
    }
}

/// Effects aren't saved, so what hangs on them goes.
fn on_effect(id: ObjectId) -> bool {
    matches!(id, ObjectId::Effect(_))
}

impl VmSnapshot {
    pub fn write(&self, file: &mut SaveFile) {
        let mut arrays = ArrayTable::default();
        let mut w = Writer::default();
        let instances: Vec<_> = {
            let mut v: Vec<_> = self
                .vm
                .instances
                .iter()
                .filter(|(obj, ..)| !on_effect(*obj))
                .collect();
            v.sort_by(|a, b| (order(&a.0), &a.1).cmp(&(order(&b.0), &b.1)));
            v
        };
        w.len(instances.len());
        for (obj, script, vars) in instances {
            put_object(&mut w, *obj);
            w.str(script);
            w.len(vars.len());
            for (name, v) in vars {
                w.str(name);
                put_value(&mut w, v, &mut arrays);
            }
        }
        let mut attached: Vec<_> = self
            .vm
            .attached
            .iter()
            .filter(|(obj, _)| !on_effect(*obj))
            .collect();
        attached.sort_by_key(|(obj, _)| order(obj));
        w.len(attached.len());
        for (obj, scripts) in attached {
            put_object(&mut w, *obj);
            w.len(scripts.len());
            for s in scripts {
                w.str(s);
            }
        }
        let threads: Vec<_> = self
            .vm
            .threads
            .iter()
            .filter(|t| {
                !t.frames
                    .iter()
                    .any(|f| f.this.as_ref().is_some_and(|(o, _)| on_effect(*o)))
            })
            .collect();
        w.len(threads.len());
        for t in &threads {
            w.len(t.frames.len());
            for f in &t.frames {
                w.str(&f.class);
                w.str(&f.state);
                w.str(&f.func);
                w.u32(f.pc as u32);
                w.len(f.locals.len());
                for v in &f.locals {
                    put_value(&mut w, v, &mut arrays);
                }
                w.bool(f.this.is_some());
                if let Some((obj, class)) = &f.this {
                    put_object(&mut w, *obj);
                    w.str(class);
                }
                put_opt_arg(&mut w, &f.ret, &mut arrays);
            }
            w.f64(t.wake_in);
            w.bool(t.menu_clock);
            put_opt_arg(&mut w, &t.wait_dest, &mut arrays);
        }
        let mut initialized = self.initialized.clone();
        initialized.sort_by_key(order);
        w.len(initialized.len());
        for o in &initialized {
            put_object(&mut w, *o);
        }
        w.len(self.timers.len());
        for t in &self.timers {
            w.f64(t.at);
            w.bool(t.game_time);
            put_object(&mut w, t.obj);
            w.str(&t.script);
            w.str(t.event);
            w.bool(t.repeat.is_some());
            w.f64(t.repeat.unwrap_or(0.0));
        }
        w.len(self.sleep_listeners.len());
        for o in &self.sleep_listeners {
            put_object(&mut w, *o);
        }
        let mut filters = self.inventory_filters.clone();
        filters.sort_by_key(|(o, _)| order(o));
        w.len(filters.len());
        for (o, items) in &mut filters {
            items.sort();
            put_object(&mut w, *o);
            w.len(items.len());
            for f in items.iter() {
                w.form(*f);
            }
        }
        w.len(self.anim_registrations.len());
        for (receiver, script, sender, event) in &self.anim_registrations {
            put_object(&mut w, *receiver);
            w.str(script);
            w.form(*sender);
            w.str(event);
        }
        file.add(b"VM  ", w.0);

        // The arrays last: writing them may meet more.
        let mut table = Writer::default();
        let mut contents = Writer::default();
        let mut i = 0;
        while i < arrays.arrays.len() {
            let items = arrays.arrays[i].borrow().clone();
            contents.len(items.len());
            for v in &items {
                put_value(&mut contents, v, &mut arrays);
            }
            i += 1;
        }
        table.len(arrays.arrays.len());
        table.0.extend(contents.0);
        file.add(b"VMAR", table.0);

        // Keep message-box host state and its VM signal keys out of VM / VMAR.
        // Indices here are the exact thread order just written above.
        let saved_boxes: Vec<_> = self
            .message_boxes
            .iter()
            .filter(|b| {
                b.key.is_none_or(|key| {
                    self.message_boxes
                        .iter()
                        .filter(|other| other.key == Some(key))
                        .count()
                        == 1
                        && threads
                            .iter()
                            .filter(|t| t.waiting_for == Some(key))
                            .count()
                            == 1
                })
            })
            .collect();
        let mut waits = Vec::new();
        for (thread_index, t) in threads.iter().enumerate() {
            if let Some(key) = t
                .waiting_for
                .filter(|key| saved_boxes.iter().any(|b| b.key == Some(*key)))
            {
                waits.push((thread_index, key));
            }
        }
        if !saved_boxes.is_empty() || self.message_next_key != 0 {
            let mut m = Writer::default();
            m.u64(self.message_next_key);
            m.len(saved_boxes.len());
            for b in saved_boxes {
                m.str(&b.title);
                m.str(&b.text);
                m.len(b.buttons.len());
                for (index, label) in &b.buttons {
                    m.i32(*index);
                    m.str(label);
                }
                m.bool(b.key.is_some());
                if let Some(key) = b.key {
                    m.u64(key);
                }
                match b.action {
                    None => m.u8(0),
                    Some(crate::messages::BoxAction::FastTravel { marker }) => {
                        m.u8(2);
                        m.form(marker);
                    }
                    Some(crate::messages::BoxAction::Poison { weapon, poison }) => {
                        m.u8(1);
                        m.form(weapon);
                        m.form(poison);
                    }
                    Some(crate::messages::BoxAction::Legendary { skill, second }) => {
                        m.u8(3);
                        m.u32(skill);
                        m.bool(second);
                    }
                }
            }
            m.len(waits.len());
            for (thread, key) in &waits {
                m.u32(*thread as u32);
                m.u64(*key);
            }
            file.add(b"MBXW", m.0);
        }

        let message_threads: std::collections::HashSet<usize> =
            waits.iter().map(|(thread, _)| *thread).collect();
        let animation_waits: Vec<_> = threads
            .iter()
            .enumerate()
            .filter(|(i, t)| !message_threads.contains(i) && t.waiting_for.is_some())
            .filter_map(|(thread, t)| {
                let key = t.waiting_for?;
                let (_, sender, event) = self
                    .animation_waits
                    .iter()
                    .find(|(wait_key, _, _)| *wait_key == key)?;
                Some((thread, *sender, event))
            })
            .collect();
        if !animation_waits.is_empty() {
            let mut a = Writer::default();
            a.len(animation_waits.len());
            for (thread, sender, event) in animation_waits {
                a.u32(thread as u32);
                a.form(sender);
                a.str(&event.to_ascii_lowercase());
            }
            file.add(b"ANMW", a.0);
        }
    }

    pub fn read(file: &SaveFile) -> Result<VmSnapshot> {
        let section = |tag: &[u8; 4]| {
            file.section(tag)
                .map(Reader::new)
                .ok_or_else(|| Corrupt(format!("no {} section", String::from_utf8_lossy(tag))))
        };
        let mut r = section(b"VMAR")?;
        let count = r.count(4)?;
        let arrays: Vec<papyrus::Array> = (0..count)
            .map(|_| Rc::new(RefCell::new(Vec::new())))
            .collect();
        for a in &arrays {
            let n = r.count(1)?;
            let items = (0..n)
                .map(|_| get_value(&mut r, &arrays))
                .collect::<Result<Vec<_>>>()?;
            *a.borrow_mut() = items;
        }
        r.finish()?;

        let mut r = section(b"VM  ")?;
        let mut s = VmSnapshot::default();
        let mut message_threads = std::collections::HashSet::new();
        for _ in 0..r.count(10)? {
            let obj = get_object(&mut r)?;
            let script: Arc<str> = Arc::from(r.str()?);
            let mut vars = Vec::new();
            for _ in 0..r.count(5)? {
                let name: Arc<str> = Arc::from(r.str()?);
                vars.push((name, get_value(&mut r, &arrays)?));
            }
            s.vm.instances.push((obj, script, vars));
        }
        for _ in 0..r.count(5)? {
            let obj = get_object(&mut r)?;
            let scripts = (0..r.count(4)?)
                .map(|_| Ok(Arc::from(r.str()?)))
                .collect::<Result<_>>()?;
            s.vm.attached.push((obj, scripts));
        }
        for _ in 0..r.count(4)? {
            let mut frames = Vec::new();
            for _ in 0..r.count(20)? {
                let class: Arc<str> = Arc::from(r.str()?);
                let state: Arc<str> = Arc::from(r.str()?);
                let func: Arc<str> = Arc::from(r.str()?);
                let pc = r.u32()? as usize;
                let locals = (0..r.count(1)?)
                    .map(|_| get_value(&mut r, &arrays))
                    .collect::<Result<_>>()?;
                let this = if r.bool()? {
                    let obj = get_object(&mut r)?;
                    Some((obj, Arc::from(r.str()?)))
                } else {
                    None
                };
                frames.push(SavedFrame {
                    class,
                    state,
                    func,
                    pc,
                    locals,
                    this,
                    ret: get_opt_arg(&mut r, &arrays)?,
                });
            }
            s.vm.threads.push(SavedThread {
                frames,
                wake_in: r.f64()?,
                menu_clock: r.bool()?,
                wait_dest: get_opt_arg(&mut r, &arrays)?,
                waiting_for: None,
            });
        }
        for _ in 0..r.count(5)? {
            s.initialized.push(get_object(&mut r)?);
        }
        for _ in 0..r.count(30)? {
            let at = r.f64()?;
            let game_time = r.bool()?;
            let obj = get_object(&mut r)?;
            let script = r.str()?;
            let event = r.str()?;
            let has_repeat = r.bool()?;
            let repeat = r.f64()?;
            // Only the two update events exist; the rest can't be told.
            let event = match event.as_str() {
                "OnUpdate" => "OnUpdate",
                "OnUpdateGameTime" => "OnUpdateGameTime",
                other => return Err(Corrupt(format!("timer event {other}"))),
            };
            s.timers.push(Timer {
                at,
                game_time,
                obj,
                script,
                event,
                repeat: has_repeat.then_some(repeat),
            });
        }
        for _ in 0..r.count(5)? {
            s.sleep_listeners.push(get_object(&mut r)?);
        }
        for _ in 0..r.count(9)? {
            let o = get_object(&mut r)?;
            let items = (0..r.count(4)?).map(|_| r.form()).collect::<Result<_>>()?;
            s.inventory_filters.push((o, items));
        }
        for _ in 0..r.count(13)? {
            let receiver = get_object(&mut r)?;
            let script = r.str()?;
            let sender = r.form()?;
            s.anim_registrations
                .push((receiver, script, sender, r.str()?));
        }
        r.finish()?;
        if let Some(mut r) = file.section(b"MBXW").map(Reader::new) {
            use crate::messages::{MESSAGE_WAIT_MAX, MessageBox, is_message_wait_key};
            use std::collections::HashSet;

            let next_key = r.u64()?;
            let mut box_keys = HashSet::new();
            for _ in 0..r.count(13)? {
                let title = r.str()?;
                let text = r.str()?;
                let mut buttons = Vec::new();
                let mut button_indices = HashSet::new();
                for _ in 0..r.count(5)? {
                    let index = r.i32()?;
                    if index < 0 || !button_indices.insert(index) {
                        return Err(Corrupt("invalid message-box button index".into()));
                    }
                    buttons.push((index, r.str()?));
                }
                if buttons.is_empty() {
                    return Err(Corrupt("message box without buttons".into()));
                }
                let key = if r.bool()? {
                    let key = r.u64()?;
                    if !is_message_wait_key(key)
                        || (key & 0xffff_ffff) > next_key
                        || !box_keys.insert(key)
                    {
                        return Err(Corrupt("invalid or duplicate message-box wait key".into()));
                    }
                    Some(key)
                } else {
                    None
                };
                let action = match r.u8()? {
                    0 => None,
                    1 => {
                        let weapon = r.form()?;
                        let poison = r.form()?;
                        if weapon.is_null() || poison.is_null() {
                            return Err(Corrupt("invalid native poison action".into()));
                        }
                        Some(crate::messages::BoxAction::Poison { weapon, poison })
                    }
                    2 => {
                        let marker = r.form()?;
                        if marker.is_null() {
                            return Err(Corrupt("invalid fast-travel action".into()));
                        }
                        Some(crate::messages::BoxAction::FastTravel { marker })
                    }
                    3 => {
                        let skill = r.u32()?;
                        if !(esp::actor_value::FIRST_SKILL..=esp::actor_value::LAST_SKILL)
                            .contains(&skill)
                        {
                            return Err(Corrupt("invalid legendary action".into()));
                        }
                        Some(crate::messages::BoxAction::Legendary {
                            skill,
                            second: r.bool()?,
                        })
                    }
                    n => return Err(Corrupt(format!("message-box action {n}"))),
                };
                if key.is_some() && action.is_some() {
                    return Err(Corrupt("script message box has native action".into()));
                }
                s.message_boxes.push_back(MessageBox {
                    title,
                    text,
                    buttons,
                    key,
                    action,
                });
            }
            if next_key > MESSAGE_WAIT_MAX {
                return Err(Corrupt("message-box wait-key counter out of range".into()));
            }
            let mut thread_indices = HashSet::new();
            let mut linked_keys = HashSet::new();
            for _ in 0..r.count(12)? {
                let thread = r.u32()? as usize;
                let key = r.u64()?;
                if thread >= s.vm.threads.len()
                    || !thread_indices.insert(thread)
                    || !linked_keys.insert(key)
                    || !box_keys.contains(&key)
                {
                    return Err(Corrupt("invalid message-box thread link".into()));
                }
                message_threads.insert(thread);
                s.vm.threads[thread].waiting_for = Some(key);
            }
            if linked_keys != box_keys {
                return Err(Corrupt("message-box wait without a saved thread".into()));
            }
            s.message_next_key = next_key;
            r.finish()?;
        }
        if let Some(mut r) = file.section(b"ANMW").map(Reader::new) {
            use std::collections::HashSet;

            let mut linked_threads = HashSet::new();
            let mut descriptors: HashMap<u64, (FormId, String)> = HashMap::new();
            for _ in 0..r.count(12)? {
                let thread = r.u32()? as usize;
                let sender = r.form()?;
                let event = r.str()?.to_ascii_lowercase();
                if thread >= s.vm.threads.len()
                    || sender.is_null()
                    || !linked_threads.insert(thread)
                    || message_threads.contains(&thread)
                {
                    return Err(Corrupt("invalid animation-wait thread link".into()));
                }
                let key = crate::anim_events::wait_key(sender, &event);
                if let Some((prior_sender, prior_event)) = descriptors.get(&key)
                    && (*prior_sender != sender || *prior_event != event)
                {
                    return Err(Corrupt("animation wait-key collision".into()));
                }
                descriptors.insert(key, (sender, event.clone()));
                s.vm.threads[thread].waiting_for = Some(key);
                s.animation_waits.push((key, sender, event));
            }
            r.finish()?;
        }
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn array(items: Vec<Value>) -> papyrus::Array {
        Rc::new(RefCell::new(items))
    }

    fn sample() -> (VmSnapshot, papyrus::Array) {
        let shared = array(vec![Value::Int(1), Value::str("two"), Value::Float(3.0)]);
        let quest = ObjectId::Form(0x20);
        let mut s = VmSnapshot::default();
        s.vm.instances.push((
            quest,
            "myquest".into(),
            vec![
                ("::state".into(), Value::str("busy")),
                ("items".into(), Value::Array(shared.clone())),
                ("again".into(), Value::Array(shared.clone())),
                (
                    "actor".into(),
                    Value::Object(
                        ObjectId::Alias {
                            quest: 0x20,
                            alias: 3,
                        },
                        "ReferenceAlias".into(),
                    ),
                ),
            ],
        ));
        // An active effect's script is not saved.
        s.vm.instances
            .push((ObjectId::Effect(9), "myeffect".into(), vec![]));
        s.vm.attached.push((quest, vec!["myquest".into()]));
        s.vm.threads.push(SavedThread {
            frames: vec![SavedFrame {
                class: "myquest".into(),
                state: "".into(),
                func: "onupdate".into(),
                pc: 4,
                locals: vec![Value::Array(shared.clone()), Value::Bool(true), Value::None],
                this: Some((quest, "myquest".into())),
                ret: Some(Arg::Local(2)),
            }],
            wake_in: 2.5,
            menu_clock: true,
            wait_dest: Some(Arg::Member("::x".into())),
            waiting_for: None,
        });
        s.initialized = vec![quest, ObjectId::Form(0x10)];
        s.timers.push(Timer {
            at: 33.0,
            game_time: true,
            obj: quest,
            script: "myquest".into(),
            event: "OnUpdateGameTime",
            repeat: Some(1.0),
        });
        s.sleep_listeners = vec![quest];
        s.inventory_filters = vec![(quest, vec![FormId(0x30), FormId(0x2F)])];
        s.anim_registrations = vec![(quest, "myquest".into(), FormId(0x14), "bowdrawn".into())];
        (s, shared)
    }

    #[test]
    fn scripts_threads_and_registrations_round_trip_with_shared_arrays() {
        let (s, _) = sample();
        let mut file = SaveFile::default();
        s.write(&mut file);
        let back = VmSnapshot::read(&SaveFile::from_bytes(&file.to_bytes()).unwrap()).unwrap();

        assert_eq!(back.vm.instances.len(), 1, "effect scripts are left out");
        let vars = &back.vm.instances[0].2;
        let get = |name: &str| &vars.iter().find(|(n, _)| &**n == name).unwrap().1;
        let (Value::Array(a), Value::Array(b)) = (get("items"), get("again")) else {
            panic!("arrays");
        };
        assert!(Rc::ptr_eq(a, b), "one array stays one array");
        assert_eq!(a.borrow().len(), 3);
        assert_eq!(*get("::state"), Value::str("busy"));
        let thread = &back.vm.threads[0];
        let Value::Array(c) = &thread.frames[0].locals[0] else {
            panic!("array local");
        };
        assert!(Rc::ptr_eq(a, c), "threads share it too");
        assert_eq!(thread.frames[0].pc, 4);
        assert_eq!(thread.waiting_for, None);
        assert_eq!(thread.wake_in, 2.5);
        assert!(thread.menu_clock);
        assert!(matches!(thread.frames[0].ret, Some(Arg::Local(2))));
        assert!(matches!(&thread.wait_dest, Some(Arg::Member(m)) if &**m == "::x"));
        assert_eq!(back.timers[0].event, "OnUpdateGameTime");
        assert_eq!(back.timers[0].repeat, Some(1.0));
        assert_eq!(back.initialized.len(), 2);
        assert_eq!(
            back.inventory_filters[0].1,
            vec![FormId(0x2F), FormId(0x30)]
        );
        assert_eq!(back.anim_registrations[0].3, "bowdrawn");

        // Writing what was read gives the same bytes.
        let mut again = SaveFile::default();
        back.write(&mut again);
        assert_eq!(again.to_bytes(), file.to_bytes());
    }

    #[test]
    fn a_dangling_array_or_unknown_timer_is_refused() {
        let (s, _) = sample();
        let mut file = SaveFile::default();
        s.write(&mut file);
        // Dropping the array table leaves the references dangling.
        let mut empty = Writer::default();
        empty.len(0);
        for (tag, payload) in &mut file.sections {
            if tag == b"VMAR" {
                *payload = empty.0.clone();
            }
        }
        assert!(VmSnapshot::read(&file).is_err());
    }

    #[test]
    fn message_boxes_link_only_to_the_serialized_filtered_thread_order() {
        use crate::messages::{BoxAction, MESSAGE_WAIT_PREFIX, MessageBox};
        use std::collections::VecDeque;

        let (mut s, _) = sample();
        let mut waiting = s.vm.threads[0].clone();
        waiting.waiting_for = Some(MESSAGE_WAIT_PREFIX | 1);
        let mut effect = waiting.clone();
        effect.frames[0].this = Some((ObjectId::Effect(3), "effect".into()));
        effect.waiting_for = Some(MESSAGE_WAIT_PREFIX | 99);
        s.vm.threads = vec![effect, waiting];
        s.message_next_key = 2;
        s.message_boxes = VecDeque::from([
            MessageBox {
                title: "title".into(),
                text: "question".into(),
                buttons: vec![(0, "No".into()), (2, "Yes".into())],
                key: Some(MESSAGE_WAIT_PREFIX | 1),
                action: None,
            },
            MessageBox {
                title: String::new(),
                text: "poison?".into(),
                buttons: vec![(0, "Apply".into()), (1, "Keep".into())],
                key: None,
                action: Some(BoxAction::Poison {
                    weapon: FormId(0x10),
                    poison: FormId(0x11),
                }),
            },
        ]);
        let mut file = SaveFile::default();
        s.write(&mut file);
        let back = VmSnapshot::read(&SaveFile::from_bytes(&file.to_bytes()).unwrap()).unwrap();
        assert_eq!(back.vm.threads.len(), 1, "effect-owned thread is filtered");
        assert_eq!(
            back.vm.threads[0].waiting_for,
            Some(MESSAGE_WAIT_PREFIX | 1)
        );
        assert_eq!(back.message_next_key, 2);
        assert_eq!(back.message_boxes.len(), 2);
        assert_eq!(
            back.message_boxes[0].buttons,
            vec![(0, "No".into()), (2, "Yes".into())]
        );
        assert!(matches!(
            back.message_boxes[1].action,
            Some(BoxAction::Poison {
                weapon: FormId(0x10),
                poison: FormId(0x11)
            })
        ));
        let mut again = SaveFile::default();
        back.write(&mut again);
        assert_eq!(again.to_bytes(), file.to_bytes());

        let mut legacy = file;
        legacy.sections.retain(|(tag, _)| tag != b"MBXW");
        let back = VmSnapshot::read(&legacy).unwrap();
        assert!(back.message_boxes.is_empty());
        assert_eq!(back.vm.threads[0].waiting_for, None);
    }

    #[test]
    fn malformed_message_box_wait_metadata_is_refused() {
        use crate::messages::{MESSAGE_WAIT_PREFIX, MessageBox};

        let (mut s, _) = sample();
        s.vm.threads[0].waiting_for = Some(MESSAGE_WAIT_PREFIX | 1);
        s.message_next_key = 1;
        s.message_boxes.push_back(MessageBox {
            title: String::new(),
            text: "question".into(),
            buttons: vec![(0, "Okay".into())],
            key: Some(MESSAGE_WAIT_PREFIX | 1),
            action: None,
        });
        let mut valid = SaveFile::default();
        s.write(&mut valid);
        let mut payloads = vec![];
        let mut bad = valid.clone();
        bad.sections
            .iter_mut()
            .find(|(tag, _)| tag == b"MBXW")
            .unwrap()
            .1[0..8]
            .copy_from_slice(&u64::MAX.to_le_bytes());
        payloads.push(bad);
        let mut bad = valid.clone();
        bad.sections
            .iter_mut()
            .find(|(tag, _)| tag == b"MBXW")
            .unwrap()
            .1
            .extend([0]);
        payloads.push(bad);
        let mut bad = valid.clone();
        let section = bad
            .sections
            .iter_mut()
            .find(|(tag, _)| tag == b"MBXW")
            .unwrap();
        section.1.truncate(section.1.len() - 1);
        payloads.push(bad);
        let mut bad = valid.clone();
        bad.sections.retain(|(tag, _)| tag != b"MBXW");
        let mut orphan = SaveFile::default();
        s.write(&mut orphan);
        // Keep the box, but erase its thread link count and link payload.
        let section = orphan
            .sections
            .iter_mut()
            .find(|(tag, _)| tag == b"MBXW")
            .unwrap();
        section.1.truncate(section.1.len() - 16);
        section.1.extend(0u32.to_le_bytes());
        payloads.push(orphan);

        let mut bad_index = valid.clone();
        let payload = &mut bad_index
            .sections
            .iter_mut()
            .find(|(tag, _)| tag == b"MBXW")
            .unwrap()
            .1;
        let link_index = payload.len() - 12;
        payload[link_index..link_index + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        payloads.push(bad_index);

        let mut duplicate_link = valid.clone();
        let payload = &mut duplicate_link
            .sections
            .iter_mut()
            .find(|(tag, _)| tag == b"MBXW")
            .unwrap()
            .1;
        let link_count = payload.len() - 16;
        payload[link_count..link_count + 4].copy_from_slice(&2u32.to_le_bytes());
        payload.extend_from_slice(&0u32.to_le_bytes());
        payload.extend_from_slice(&(MESSAGE_WAIT_PREFIX | 1).to_le_bytes());
        payloads.push(duplicate_link);
        for file in payloads {
            assert!(VmSnapshot::read(&file).is_err());
        }
        assert!(
            VmSnapshot::read(&bad).is_ok(),
            "missing optional section is legacy fallback"
        );
    }

    #[test]
    fn unsupported_signal_waits_keep_the_existing_timed_fallback() {
        let (mut s, _) = sample();
        s.vm.threads[0].wake_in = 1.25;
        s.vm.threads[0].waiting_for = Some(0x1234_5678);
        let mut file = SaveFile::default();
        s.write(&mut file);
        assert!(file.section(b"MBXW").is_none());
        let back = VmSnapshot::read(&file).unwrap();
        assert_eq!(back.vm.threads[0].waiting_for, None);
        assert_eq!(back.vm.threads[0].wake_in, 1.25);
    }

    #[test]
    fn animation_waits_round_trip_on_the_filtered_thread_order() {
        let (mut s, _) = sample();
        let sender = FormId(0x44);
        let key = crate::anim_events::wait_key(sender, "Done");
        s.vm.threads[0].waiting_for = Some(key);
        s.vm.threads.push(s.vm.threads[0].clone());
        s.vm.threads.push({
            let mut effect = s.vm.threads[0].clone();
            effect.frames[0].this = Some((ObjectId::Effect(7), "effect".into()));
            effect
        });
        s.animation_waits = vec![(key, sender, "done".into())];

        let mut file = SaveFile::default();
        s.write(&mut file);
        assert!(file.section(b"ANMW").is_some());
        let back = VmSnapshot::read(&SaveFile::from_bytes(&file.to_bytes()).unwrap()).unwrap();
        assert_eq!(back.vm.threads.len(), 2, "effect-owned thread is filtered");
        assert_eq!(back.vm.threads[0].waiting_for, Some(key));
        assert_eq!(back.vm.threads[1].waiting_for, Some(key));
        assert_eq!(back.animation_waits, vec![(key, sender, "done".into()); 2]);
        let mut again = SaveFile::default();
        back.write(&mut again);
        assert_eq!(again.to_bytes(), file.to_bytes());

        let mut legacy = file;
        legacy.sections.retain(|(tag, _)| tag != b"ANMW");
        let back = VmSnapshot::read(&legacy).unwrap();
        assert!(back.animation_waits.is_empty());
        assert_eq!(back.vm.threads.len(), 2);
        assert!(back.vm.threads.iter().all(|t| t.waiting_for.is_none()));
    }

    #[test]
    fn malformed_animation_wait_metadata_is_refused() {
        let (mut s, _) = sample();
        let sender = FormId(0x44);
        let key = crate::anim_events::wait_key(sender, "done");
        s.vm.threads[0].waiting_for = Some(key);
        s.animation_waits = vec![(key, sender, "done".into())];
        let mut valid = SaveFile::default();
        s.write(&mut valid);

        let mut bad_files = Vec::new();
        let mut truncated = valid.clone();
        truncated
            .sections
            .iter_mut()
            .find(|(tag, _)| tag == b"ANMW")
            .unwrap()
            .1
            .pop();
        bad_files.push(truncated);
        let mut trailing = valid.clone();
        trailing
            .sections
            .iter_mut()
            .find(|(tag, _)| tag == b"ANMW")
            .unwrap()
            .1
            .push(0);
        bad_files.push(trailing);
        for (field_offset, value) in [(4, u32::MAX), (8, 0)] {
            let mut bad = valid.clone();
            let payload = &mut bad
                .sections
                .iter_mut()
                .find(|(tag, _)| tag == b"ANMW")
                .unwrap()
                .1;
            payload[field_offset..field_offset + 4].copy_from_slice(&value.to_le_bytes());
            bad_files.push(bad);
        }
        let mut duplicate = valid.clone();
        let payload = &mut duplicate
            .sections
            .iter_mut()
            .find(|(tag, _)| tag == b"ANMW")
            .unwrap()
            .1;
        let one_record = payload[4..].to_vec();
        payload[0..4].copy_from_slice(&2u32.to_le_bytes());
        payload.extend_from_slice(&one_record);
        bad_files.push(duplicate);

        for file in bad_files {
            assert!(VmSnapshot::read(&file).is_err());
        }
    }
}
