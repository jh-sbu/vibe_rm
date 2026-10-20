//! The Papyrus VM in a save: scripts attached with their variables, the
//! threads waiting, and what the engine registered for them (timers,
//! `OnInit` done, sleep and animation event listeners, inventory filters).
//! Arrays are shared by identity in the VM, so they are saved once in a table
//! (`VMAR`) and referred to by index; `VM` holds the rest.

use std::cell::RefCell;
use std::collections::HashMap;
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
        for t in threads {
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
}
