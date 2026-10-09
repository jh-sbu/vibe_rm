//! The Papyrus virtual machine.
//!
//! Scripts are compiled from PEX into resolved operations. Each running call
//! chain is a [`Thread`] with explicit frames so that latent natives (e.g.
//! `Utility.Wait`) can suspend it without blocking the host.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use crate::pex::{self, Data, Pex};
use crate::value::{ObjectId, Value};

fn lc(s: &str) -> Arc<str> {
    Arc::from(s.to_ascii_lowercase())
}

#[derive(Debug, Clone)]
enum Arg {
    Lit(Value),
    Local(usize),
    Member(Arc<str>),
    SelfRef,
    State,
    Discard,
}

#[derive(Debug, Clone)]
struct Op {
    code: u8,
    /// Name operands (method / property / class names).
    name: Option<Arc<str>>,
    name2: Option<Arc<str>>,
    args: Vec<Arg>,
}

pub struct Func {
    pub name: Arc<str>,
    /// Lower-cased name of the class defining this function.
    pub class: Arc<str>,
    pub return_type: Arc<str>,
    pub global: bool,
    pub native: bool,
    pub params: Vec<(Arc<str>, Arc<str>)>,
    local_types: Vec<Arc<str>>,
    code: Vec<Op>,
}

struct PropertyDef {
    type_name: Arc<str>,
    auto_var: Option<Arc<str>>,
    read: Option<Arc<Func>>,
    write: Option<Arc<Func>>,
}

pub struct Class {
    pub name: Arc<str>,
    pub parent: Option<Arc<str>>,
    variables: Vec<(Arc<str>, Arc<str>, Value)>,
    properties: HashMap<Arc<str>, PropertyDef>,
    states: HashMap<Arc<str>, HashMap<Arc<str>, Arc<Func>>>,
    auto_state: Arc<str>,
}

/// The value a variable of a type starts with: 0, 0.0, false, "", else None
/// (objects and arrays).
fn type_default(ty: &str) -> Value {
    match ty {
        "int" => Value::Int(0),
        "float" => Value::Float(0.0),
        "bool" => Value::Bool(false),
        "string" => Value::str(""),
        _ => Value::None,
    }
}

impl Class {
    fn compile(p: &Pex) -> Option<Class> {
        let obj = p.objects.first()?;
        let class_lc = lc(p.str(obj.name));
        let s = |i: u16| p.str(i);
        let data_value = |d: &Data| -> Value {
            match d {
                Data::Null => Value::None,
                Data::Int(i) => Value::Int(*i),
                Data::Float(f) => Value::Float(*f),
                Data::Bool(b) => Value::Bool(*b),
                Data::String(i) => Value::str(s(*i)),
                Data::Identifier(i) => Value::str(s(*i)),
            }
        };
        let compile_fn = |name: &str, f: &pex::Function| -> Arc<Func> {
            let mut local_names: Vec<Arc<str>> = Vec::new();
            let mut local_types: Vec<Arc<str>> = Vec::new();
            for (n, t) in f.params.iter().chain(f.locals.iter()) {
                local_names.push(lc(s(*n)));
                local_types.push(lc(s(*t)));
            }
            let resolve = |d: &Data| -> Arg {
                match d {
                    Data::Identifier(i) => {
                        let n = s(*i).to_ascii_lowercase();
                        if n == "self" {
                            Arg::SelfRef
                        } else if n == "::nonevar" {
                            Arg::Discard
                        } else if n == "::state" {
                            Arg::State
                        } else if let Some(k) = local_names.iter().position(|x| **x == *n) {
                            Arg::Local(k)
                        } else {
                            Arg::Member(Arc::from(n))
                        }
                    }
                    other => Arg::Lit(data_value(other)),
                }
            };
            let name_of = |d: &Data| -> Arc<str> {
                match d {
                    Data::Identifier(i) | Data::String(i) => lc(s(*i)),
                    _ => Arc::from(""),
                }
            };
            let code = f
                .code
                .iter()
                .map(|ins| {
                    let a = &ins.args;
                    match ins.op {
                        23 => Op {
                            code: 23,
                            name: Some(name_of(&a[0])),
                            name2: None,
                            args: a[1..].iter().map(resolve).collect(),
                        },
                        24 => Op {
                            code: 24,
                            name: Some(name_of(&a[0])),
                            name2: None,
                            args: a[1..].iter().map(resolve).collect(),
                        },
                        25 => Op {
                            code: 25,
                            name: Some(name_of(&a[0])),
                            name2: Some(name_of(&a[1])),
                            args: a[2..].iter().map(resolve).collect(),
                        },
                        28 | 29 => Op {
                            code: ins.op,
                            name: Some(name_of(&a[0])),
                            name2: None,
                            args: a[1..].iter().map(resolve).collect(),
                        },
                        op => Op {
                            code: op,
                            name: None,
                            name2: None,
                            args: a.iter().map(resolve).collect(),
                        },
                    }
                })
                .collect();
            Arc::new(Func {
                name: Arc::from(name),
                class: class_lc.clone(),
                return_type: lc(s(f.return_type)),
                global: f.is_global(),
                native: f.is_native(),
                params: f
                    .params
                    .iter()
                    .map(|(n, t)| (lc(s(*n)), lc(s(*t))))
                    .collect(),
                local_types,
                code,
            })
        };
        let mut states: HashMap<Arc<str>, HashMap<Arc<str>, Arc<Func>>> = HashMap::new();
        for st in &obj.states {
            let fns = states.entry(lc(s(st.name))).or_default();
            for (fname, f) in &st.functions {
                fns.insert(lc(s(*fname)), compile_fn(s(*fname), f));
            }
        }
        let mut properties = HashMap::new();
        for pr in &obj.properties {
            let pname = s(pr.name);
            properties.insert(
                lc(pname),
                PropertyDef {
                    type_name: lc(s(pr.type_name)),
                    auto_var: pr.auto_var.map(|v| lc(s(v))),
                    read: pr.read.as_ref().map(|f| compile_fn(pname, f)),
                    write: pr.write.as_ref().map(|f| compile_fn(pname, f)),
                },
            );
        }
        let parent = s(obj.parent);
        Some(Class {
            name: Arc::from(s(obj.name)),
            parent: if parent.is_empty() {
                None
            } else {
                Some(lc(parent))
            },
            variables: obj
                .variables
                .iter()
                .map(|v| {
                    let ty = lc(s(v.type_name));
                    let init = match v.init {
                        Data::Null => type_default(&ty),
                        ref d => data_value(d),
                    };
                    (lc(s(v.name)), ty, init)
                })
                .collect(),
            properties,
            states,
            auto_state: lc(s(obj.auto_state)),
        })
    }
}

/// Result of a native function call.
pub enum NativeResult {
    Value(Value),
    /// Suspend the calling thread for this many real seconds, then return None.
    Wait(f32),
    /// As `Wait`, counted on [`Vm::menu_time`] (which runs on in menu mode).
    WaitMenuMode(f32),
    /// Return `value`, suspending the calling thread until the host calls
    /// [`Vm::signal`] with `key`, or for `timeout` real seconds at most.
    WaitFor {
        key: u64,
        timeout: f32,
        value: Value,
    },
}

/// The embedding application.
pub trait Host {
    fn load_script(&mut self, name: &str) -> Option<Vec<u8>>;
    /// Call a native function. `class` is the (lower-case) class that declares it.
    fn call_native(
        &mut self,
        class: &str,
        func: &str,
        this: Option<&Value>,
        args: &[Value],
    ) -> NativeResult;
    /// Whether an object satisfies a native base type (`form`, `objectreference`, `actor`, ...).
    fn is_native_type(&self, obj: ObjectId, class: &str) -> bool;
}

struct Frame {
    func: Arc<Func>,
    pc: usize,
    locals: Vec<Value>,
    /// (object, instance class) when running on an object.
    this: Option<(ObjectId, Arc<str>)>,
    /// Where to store the return value in the caller frame.
    ret: Option<Arg>,
}

struct Thread {
    frames: Vec<Frame>,
    wake_at: f64,
    /// `wake_at` is on [`Vm::menu_time`], not [`Vm::time`].
    menu_clock: bool,
    /// The signal it waits for (`NativeResult::WaitFor`).
    waiting_for: Option<u64>,
    /// Where the waiting call's result goes (`Vm::signal_with`).
    wait_dest: Option<Arg>,
}

#[derive(Default)]
struct Instance {
    vars: HashMap<Arc<str>, Value>,
}

#[derive(Default)]
pub struct Vm {
    classes: HashMap<Arc<str>, Option<Arc<Class>>>,
    instances: HashMap<(ObjectId, Arc<str>), Instance>,
    attached: HashMap<ObjectId, Vec<Arc<str>>>,
    threads: Vec<Thread>,
    pub time: f64,
    /// Real seconds menus included, which `WaitMenuMode` counts on; the host
    /// keeps it, stopping `time` in menu mode.
    pub menu_time: f64,
    warned: HashSet<String>,
    /// Total instructions executed (for diagnostics).
    pub executed: u64,
}

const MAX_DEPTH: usize = 128;

impl Vm {
    pub fn new() -> Self {
        Vm::default()
    }

    pub fn class(&mut self, host: &mut dyn Host, name: &str) -> Option<Arc<Class>> {
        let key = lc(name);
        if let Some(c) = self.classes.get(&key) {
            return c.clone();
        }
        let c = host.load_script(&key).and_then(|b| match pex::parse(&b) {
            Ok(p) => Class::compile(&p).map(Arc::new),
            Err(e) => {
                log::warn!("script {name}: {e}");
                None
            }
        });
        self.classes.insert(key, c.clone());
        c
    }

    /// Whether `class` is `ancestor` or derives from it.
    pub fn derives(&mut self, host: &mut dyn Host, class: &str, ancestor: &str) -> bool {
        let mut cur: Option<Arc<str>> = Some(lc(class));
        let anc = ancestor.to_ascii_lowercase();
        for _ in 0..32 {
            let Some(c) = cur else { return false };
            if *c == *anc {
                return true;
            }
            cur = self.class(host, &c).and_then(|k| k.parent.clone());
        }
        false
    }

    /// Attach a script to an object, initialising variables and the given property values.
    pub fn attach(
        &mut self,
        host: &mut dyn Host,
        obj: ObjectId,
        script: &str,
        props: &[(String, Value)],
    ) -> bool {
        let key = lc(script);
        if self.instances.contains_key(&(obj, key.clone())) {
            return true;
        }
        let Some(class) = self.class(host, script) else {
            self.warn_once(format!("missing script {script}"));
            return false;
        };
        let mut inst = Instance::default();
        let mut cur = Some(class.clone());
        let mut auto_state = None;
        while let Some(c) = cur {
            for (n, _, init) in &c.variables {
                inst.vars.entry(n.clone()).or_insert_with(|| init.clone());
            }
            if auto_state.is_none() && !c.auto_state.is_empty() {
                auto_state = Some(c.auto_state.clone());
            }
            cur = c.parent.as_ref().and_then(|p| self.class(host, p));
        }
        inst.vars.insert(
            Arc::from("::state"),
            Value::String(auto_state.unwrap_or_else(|| Arc::from(""))),
        );
        for (pname, v) in props {
            let pkey = lc(pname);
            match self.find_property(host, &key, &pkey) {
                Some((ty, Some(var), _)) => {
                    // An alias is a ReferenceAlias or a LocationAlias as the property says.
                    let v = match v {
                        Value::Object(id @ ObjectId::Alias { .. }, _)
                            if &*ty == "locationalias" =>
                        {
                            Value::Object(*id, "LocationAlias".into())
                        }
                        _ => v.clone(),
                    };
                    inst.vars.insert(var, v);
                }
                _ => {
                    // Unknown or handler-backed property: store under the conventional name.
                    inst.vars
                        .insert(Arc::from(format!("::{pkey}_var")), v.clone());
                }
            }
        }
        self.instances.insert((obj, key.clone()), inst);
        self.attached.entry(obj).or_default().push(key);
        true
    }

    pub fn detach_all(&mut self, obj: ObjectId) {
        if let Some(list) = self.attached.remove(&obj) {
            for c in list {
                self.instances.remove(&(obj, c));
            }
        }
    }

    pub fn attached_scripts(&self, obj: ObjectId) -> &[Arc<str>] {
        self.attached.get(&obj).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Read a script variable (by its compiled name, e.g. `::Foo_var`) from any script on `obj`.
    pub fn get_var(&self, obj: ObjectId, name: &str) -> Option<Value> {
        let key = name.to_ascii_lowercase();
        let alt = format!("::{key}_var");
        for c in self.attached.get(&obj)? {
            if let Some(i) = self.instances.get(&(obj, c.clone()))
                && let Some(v) = i
                    .vars
                    .get(key.as_str())
                    .or_else(|| i.vars.get(alt.as_str()))
            {
                return Some(v.clone());
            }
        }
        None
    }

    /// Every variable of every script on `obj` (script, compiled name, value),
    /// sorted.
    pub fn vars(&self, obj: ObjectId) -> Vec<(Arc<str>, Arc<str>, Value)> {
        let mut out = Vec::new();
        for c in self.attached.get(&obj).into_iter().flatten() {
            if let Some(i) = self.instances.get(&(obj, c.clone())) {
                for (n, v) in &i.vars {
                    out.push((c.clone(), n.clone(), v.clone()));
                }
            }
        }
        out.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
        out
    }

    /// Whether a script on `obj`, or one it extends, has a function `name` in
    /// any state (an event handler, say).
    pub fn handles(&self, obj: ObjectId, name: &str) -> bool {
        let name = lc(name);
        self.attached_scripts(obj).iter().any(|script| {
            let mut cur = Some(script.clone());
            for _ in 0..32 {
                let Some(c) = cur.and_then(|k| self.classes.get(&k).cloned().flatten()) else {
                    return false;
                };
                if c.states.values().any(|fs| fs.contains_key(&name)) {
                    return true;
                }
                cur = c.parent.as_deref().map(lc);
            }
            false
        })
    }

    pub fn has_instance(&self, obj: ObjectId, script: &str) -> bool {
        self.instances.contains_key(&(obj, lc(script)))
    }

    fn warn_once(&mut self, msg: String) {
        if self.warned.insert(msg.clone()) {
            log::warn!("papyrus: {msg}");
        }
    }

    fn state_of(&self, obj: ObjectId, class: &Arc<str>) -> Arc<str> {
        match self
            .instances
            .get(&(obj, class.clone()))
            .and_then(|i| i.vars.get("::state"))
        {
            Some(Value::String(s)) => lc(s),
            _ => Arc::from(""),
        }
    }

    /// Find a function by name, walking up from `class`, preferring `state`.
    fn resolve(
        &mut self,
        host: &mut dyn Host,
        class: &str,
        state: &str,
        name: &str,
    ) -> Option<Arc<Func>> {
        let mut cur: Option<Arc<str>> = Some(lc(class));
        for _ in 0..32 {
            let c = self.class(host, &cur?)?;
            if !state.is_empty()
                && let Some(f) = c.states.get(state).and_then(|s| s.get(name))
            {
                return Some(f.clone());
            }
            if let Some(f) = c.states.get("").and_then(|s| s.get(name)) {
                return Some(f.clone());
            }
            cur = c.parent.clone();
        }
        None
    }

    /// (property type, auto var, (read, write))
    #[allow(clippy::type_complexity)]
    fn find_property(
        &mut self,
        host: &mut dyn Host,
        class: &str,
        prop: &str,
    ) -> Option<(
        Arc<str>,
        Option<Arc<str>>,
        (Option<Arc<Func>>, Option<Arc<Func>>),
    )> {
        let mut cur: Option<Arc<str>> = Some(lc(class));
        for _ in 0..32 {
            let c = self.class(host, &cur?)?;
            if let Some(p) = c.properties.get(prop) {
                return Some((
                    p.type_name.clone(),
                    p.auto_var.clone(),
                    (p.read.clone(), p.write.clone()),
                ));
            }
            cur = c.parent.clone();
        }
        None
    }

    fn member_type(&mut self, host: &mut dyn Host, class: &str, var: &str) -> Option<Arc<str>> {
        let mut cur: Option<Arc<str>> = Some(lc(class));
        for _ in 0..32 {
            let c = self.class(host, &cur?)?;
            if let Some(v) = c.variables.iter().find(|v| *v.0 == *var) {
                return Some(v.1.clone());
            }
            cur = c.parent.clone();
        }
        None
    }

    /// The attached instance class for an object viewed as `class` (most derived match).
    fn instance_class(&mut self, host: &mut dyn Host, obj: ObjectId, class: &str) -> Arc<str> {
        let list: Vec<Arc<str>> = self.attached.get(&obj).cloned().unwrap_or_default();
        for c in &list {
            if self.derives(host, c, class) {
                return c.clone();
            }
        }
        lc(class)
    }

    /// Queue an event on every script attached to `obj` that handles it.
    pub fn send_event(
        &mut self,
        host: &mut dyn Host,
        obj: ObjectId,
        event: &str,
        args: Vec<Value>,
    ) -> usize {
        let name = lc(event);
        let list: Vec<Arc<str>> = self.attached.get(&obj).cloned().unwrap_or_default();
        let mut n = 0;
        for class in list {
            let state = self.state_of(obj, &class);
            if let Some(f) = self.resolve(host, &class, &state, &name)
                && !f.native
            {
                log::trace!("event {event} -> {class} ({state:?}) on {obj:?}");
                let frame = self.make_frame(f, Some((obj, class)), &args, None);
                self.threads.push(Thread {
                    frames: vec![frame],
                    wake_at: self.time,
                    menu_clock: false,
                    waiting_for: None,
                    wait_dest: None,
                });
                n += 1;
            }
        }
        n
    }

    /// Queue an event on one specific script instance.
    pub fn send_event_to(
        &mut self,
        host: &mut dyn Host,
        obj: ObjectId,
        script: &str,
        event: &str,
        args: Vec<Value>,
    ) -> bool {
        let class = lc(script);
        if !self.instances.contains_key(&(obj, class.clone())) {
            return false;
        }
        let state = self.state_of(obj, &class);
        if let Some(f) = self.resolve(host, &class, &state, &lc(event))
            && !f.native
        {
            let frame = self.make_frame(f, Some((obj, class)), &args, None);
            self.threads.push(Thread {
                frames: vec![frame],
                wake_at: self.time,
                menu_clock: false,
                waiting_for: None,
                wait_dest: None,
            });
            return true;
        }
        false
    }

    /// Start a call to a method on a specific script instance (e.g. quest fragments).
    pub fn call_method(
        &mut self,
        host: &mut dyn Host,
        obj: ObjectId,
        script: &str,
        func: &str,
        args: Vec<Value>,
    ) -> bool {
        self.send_event_to(host, obj, script, func, args)
    }

    fn make_frame(
        &self,
        f: Arc<Func>,
        this: Option<(ObjectId, Arc<str>)>,
        args: &[Value],
        ret: Option<Arg>,
    ) -> Frame {
        let mut locals: Vec<Value> = f.local_types.iter().map(|t| type_default(t)).collect();
        for (i, a) in args.iter().enumerate().take(f.params.len()) {
            locals[i] = a.clone();
        }
        Frame {
            func: f,
            pc: 0,
            locals,
            this,
            ret,
        }
    }

    /// Wake the threads waiting for `key` (`NativeResult::WaitFor`); they run
    /// on the next [`Vm::run`]. Returns how many there were.
    pub fn signal(&mut self, key: u64) -> usize {
        let mut n = 0;
        for t in &mut self.threads {
            if t.waiting_for == Some(key) {
                t.waiting_for = None;
                t.wake_at = self.time;
                t.menu_clock = false;
                n += 1;
            }
        }
        n
    }

    /// Wake the threads waiting for `key` as [`Vm::signal`] does, with `value`
    /// as the result of the call they wait in (a message box's button).
    pub fn signal_with(&mut self, key: u64, value: Value) -> usize {
        let mut n = 0;
        for t in &mut self.threads {
            if t.waiting_for == Some(key) {
                t.waiting_for = None;
                t.wake_at = self.time;
                t.menu_clock = false;
                if let (Some(dest), Some(frame)) = (t.wait_dest.take(), t.frames.last_mut()) {
                    Self::set_in(&mut self.instances, frame, &dest, value.clone());
                }
                n += 1;
            }
        }
        n
    }

    /// Whether a thread waits for `key`.
    pub fn is_waiting_for(&self, key: u64) -> bool {
        self.threads.iter().any(|t| t.waiting_for == Some(key))
    }

    pub fn thread_count(&self) -> usize {
        self.threads.len()
    }

    /// Run all threads that are due. `budget` bounds instructions per thread.
    pub fn run(&mut self, host: &mut dyn Host, now: f64, budget: usize) {
        self.time = now;
        let pending = std::mem::take(&mut self.threads);
        let mut keep = Vec::with_capacity(pending.len());
        for mut t in pending {
            if t.wake_at > if t.menu_clock { self.menu_time } else { now } {
                keep.push(t);
                continue;
            }
            // Woken by its signal or timed out.
            t.waiting_for = None;
            t.menu_clock = false;
            if !self.step_thread(host, &mut t, budget) {
                keep.push(t);
            }
        }
        keep.append(&mut self.threads);
        self.threads = keep;
    }

    fn get(&self, frame: &Frame, a: &Arg) -> Value {
        match a {
            Arg::Lit(v) => v.clone(),
            Arg::Local(i) => frame.locals[*i].clone(),
            Arg::SelfRef => match &frame.this {
                Some((o, c)) => Value::Object(*o, c.clone()),
                None => Value::None,
            },
            Arg::State => frame
                .this
                .as_ref()
                .and_then(|(o, c)| self.instances.get(&(*o, c.clone())))
                .and_then(|i| i.vars.get("::state").cloned())
                .unwrap_or_else(|| Value::str("")),
            Arg::Member(n) => frame
                .this
                .as_ref()
                .and_then(|(o, c)| self.instances.get(&(*o, c.clone())))
                .and_then(|i| i.vars.get(n).cloned())
                .unwrap_or_default(),
            Arg::Discard => Value::None,
        }
    }

    fn set_in(
        instances: &mut HashMap<(ObjectId, Arc<str>), Instance>,
        frame: &mut Frame,
        a: &Arg,
        v: Value,
    ) {
        match a {
            Arg::Local(i) => frame.locals[*i] = v,
            Arg::Member(n) => {
                if let Some((o, c)) = &frame.this
                    && let Some(inst) = instances.get_mut(&(*o, c.clone()))
                {
                    inst.vars.insert(n.clone(), v);
                }
            }
            Arg::State => {
                if let Some((o, c)) = &frame.this
                    && let Some(inst) = instances.get_mut(&(*o, c.clone()))
                {
                    inst.vars.insert(Arc::from("::state"), v);
                }
            }
            _ => {}
        }
    }

    fn arg_type(&mut self, host: &mut dyn Host, frame: &Frame, a: &Arg) -> Arc<str> {
        match a {
            Arg::Local(i) => frame.func.local_types[*i].clone(),
            Arg::Member(n) => {
                let class = frame
                    .this
                    .as_ref()
                    .map(|t| t.1.clone())
                    .unwrap_or_else(|| frame.func.class.clone());
                self.member_type(host, &class, n)
                    .unwrap_or_else(|| Arc::from(""))
            }
            _ => Arc::from(""),
        }
    }

    fn cast(&mut self, host: &mut dyn Host, v: Value, ty: &str) -> Value {
        match ty {
            "int" => Value::Int(v.as_int()),
            "float" => Value::Float(v.as_float()),
            "bool" => Value::Bool(v.as_bool()),
            "string" => match v {
                Value::None => Value::str("None"),
                Value::String(_) => v,
                other => Value::str(&other.to_string()),
            },
            "" => v,
            t if t.ends_with("[]") => v,
            t => match v {
                Value::Object(o, _) => {
                    let ok = host.is_native_type(o, t) || {
                        let list: Vec<Arc<str>> =
                            self.attached.get(&o).cloned().unwrap_or_default();
                        list.iter().any(|c| self.derives(host, c, t))
                    };
                    if ok {
                        Value::Object(o, Arc::from(t))
                    } else {
                        Value::None
                    }
                }
                _ => Value::None,
            },
        }
    }

    /// Execute until the thread finishes (true) or suspends / exhausts its budget (false).
    fn step_thread(&mut self, host: &mut dyn Host, t: &mut Thread, budget: usize) -> bool {
        let mut steps = 0;
        loop {
            let Some(frame) = t.frames.last_mut() else {
                return true;
            };
            if frame.pc >= frame.func.code.len() {
                // Implicit return None.
                let f = t.frames.pop().unwrap();
                if let (Some(caller), Some(ret)) = (t.frames.last_mut(), &f.ret) {
                    Self::set_in(&mut self.instances, caller, ret, Value::None);
                }
                continue;
            }
            steps += 1;
            self.executed += 1;
            if steps > budget {
                return false;
            }
            let op = frame.func.code[frame.pc].clone();
            frame.pc += 1;
            let frame_idx = t.frames.len() - 1;
            macro_rules! frame {
                () => {
                    t.frames[frame_idx]
                };
            }
            let a = |vm: &Vm, f: &Frame, i: usize| vm.get(f, &op.args[i]);
            match op.code {
                0 => {}
                1..=9 => {
                    let (x, y) = (a(self, &frame!(), 1), a(self, &frame!(), 2));
                    let r = match op.code {
                        1 => Value::Int(x.as_int().wrapping_add(y.as_int())),
                        2 => Value::Float(x.as_float() + y.as_float()),
                        3 => Value::Int(x.as_int().wrapping_sub(y.as_int())),
                        4 => Value::Float(x.as_float() - y.as_float()),
                        5 => Value::Int(x.as_int().wrapping_mul(y.as_int())),
                        6 => Value::Float(x.as_float() * y.as_float()),
                        7 => Value::Int(if y.as_int() == 0 {
                            0
                        } else {
                            x.as_int().wrapping_div(y.as_int())
                        }),
                        8 => Value::Float(x.as_float() / y.as_float()),
                        _ => Value::Int(if y.as_int() == 0 {
                            0
                        } else {
                            x.as_int().wrapping_rem(y.as_int())
                        }),
                    };
                    Self::set_in(&mut self.instances, &mut frame!(), &op.args[0], r);
                }
                10 => {
                    let r = Value::Bool(!a(self, &frame!(), 1).as_bool());
                    Self::set_in(&mut self.instances, &mut frame!(), &op.args[0], r);
                }
                11 => {
                    let r = Value::Int(-a(self, &frame!(), 1).as_int());
                    Self::set_in(&mut self.instances, &mut frame!(), &op.args[0], r);
                }
                12 => {
                    let r = Value::Float(-a(self, &frame!(), 1).as_float());
                    Self::set_in(&mut self.instances, &mut frame!(), &op.args[0], r);
                }
                13 => {
                    let r = a(self, &frame!(), 1);
                    Self::set_in(&mut self.instances, &mut frame!(), &op.args[0], r);
                }
                14 => {
                    let v = a(self, &frame!(), 1);
                    let ty = self.arg_type(host, &frame!(), &op.args[0]);
                    let r = self.cast(host, v, &ty);
                    Self::set_in(&mut self.instances, &mut frame!(), &op.args[0], r);
                }
                15..=19 => {
                    let (x, y) = (a(self, &frame!(), 1), a(self, &frame!(), 2));
                    let r = if op.code == 15 {
                        x == y
                    } else {
                        let ord = match (&x, &y) {
                            (Value::Int(p), Value::Int(q)) => p.partial_cmp(q),
                            (Value::String(p), Value::String(q)) => {
                                Some(p.to_ascii_lowercase().cmp(&q.to_ascii_lowercase()))
                            }
                            _ => x.as_float().partial_cmp(&y.as_float()),
                        };
                        match (op.code, ord) {
                            (16, Some(o)) => o.is_lt(),
                            (17, Some(o)) => o.is_le(),
                            (18, Some(o)) => o.is_gt(),
                            (19, Some(o)) => o.is_ge(),
                            _ => false,
                        }
                    };
                    Self::set_in(
                        &mut self.instances,
                        &mut frame!(),
                        &op.args[0],
                        Value::Bool(r),
                    );
                }
                20 => {
                    let off = a(self, &frame!(), 0).as_int();
                    frame!().pc = (frame!().pc as i64 - 1 + off as i64).max(0) as usize;
                }
                21 | 22 => {
                    let c = a(self, &frame!(), 0).as_bool();
                    if c == (op.code == 21) {
                        let off = a(self, &frame!(), 1).as_int();
                        frame!().pc = (frame!().pc as i64 - 1 + off as i64).max(0) as usize;
                    }
                }
                23 | 24 | 25 => {
                    // callmethod: [self, dest, n, args...]; callparent: [dest, n, args...]; callstatic: [dest, n, args...]
                    let (target, dest_i) = match op.code {
                        23 => (Some(a(self, &frame!(), 0)), 1),
                        _ => (None, 0),
                    };
                    let argv: Vec<Value> = op.args[dest_i + 2..]
                        .iter()
                        .map(|x| self.get(&frame!(), x))
                        .collect();
                    let dest = op.args[dest_i].clone();
                    let name = op.name.clone().unwrap_or_else(|| Arc::from(""));
                    let mut call: Option<(Arc<Func>, Option<(ObjectId, Arc<str>)>, Option<Value>)> =
                        None;
                    match op.code {
                        23 => match target.unwrap() {
                            Value::Object(o, cls) => {
                                let ic = self.instance_class(host, o, &cls);
                                let state = self.state_of(o, &ic);
                                match self.resolve(host, &ic, &state, &name) {
                                    Some(f) => {
                                        call = Some((f, Some((o, ic)), Some(Value::Object(o, cls))))
                                    }
                                    None => {
                                        if !matches!(&*name, "onbeginstate" | "onendstate") {
                                            self.warn_once(format!("unknown method {cls}.{name}"));
                                        }
                                    }
                                }
                            }
                            Value::None => {
                                let caller = frame!().func.name.clone();
                                log::debug!(
                                    "papyrus: cannot call {name}() on a None object (in {caller})"
                                );
                            }
                            other => self.warn_once(format!("cannot call {name}() on {other:?}")),
                        },
                        24 => {
                            let parent = self
                                .class(host, &frame!().func.class)
                                .and_then(|c| c.parent.clone());
                            if let Some(p) = parent {
                                let state = frame!()
                                    .this
                                    .as_ref()
                                    .map(|(o, c)| self.state_of(*o, c))
                                    .unwrap_or_else(|| Arc::from(""));
                                if let Some(f) = self.resolve(host, &p, &state, &name) {
                                    let this = frame!().this.clone();
                                    let tv =
                                        this.as_ref().map(|(o, c)| Value::Object(*o, c.clone()));
                                    call = Some((f, this, tv));
                                }
                            }
                        }
                        _ => {
                            let class = name.clone();
                            let fname = op.name2.clone().unwrap_or_else(|| Arc::from(""));
                            match self.resolve(host, &class, "", &fname) {
                                Some(f) => call = Some((f, None, None)),
                                None => self.warn_once(format!("unknown global {class}.{fname}")),
                            }
                        }
                    }
                    match call {
                        Some((f, this, this_val)) if f.native => {
                            match host.call_native(
                                &f.class,
                                &f.name.to_ascii_lowercase(),
                                this_val.as_ref(),
                                &argv,
                            ) {
                                NativeResult::Value(v) => {
                                    Self::set_in(&mut self.instances, &mut frame!(), &dest, v)
                                }
                                NativeResult::Wait(secs) => {
                                    Self::set_in(
                                        &mut self.instances,
                                        &mut frame!(),
                                        &dest,
                                        Value::None,
                                    );
                                    t.wake_at = self.time + secs.max(0.0) as f64;
                                    let _ = this;
                                    return false;
                                }
                                NativeResult::WaitMenuMode(secs) => {
                                    Self::set_in(
                                        &mut self.instances,
                                        &mut frame!(),
                                        &dest,
                                        Value::None,
                                    );
                                    t.wake_at = self.menu_time + secs.max(0.0) as f64;
                                    t.menu_clock = true;
                                    return false;
                                }
                                NativeResult::WaitFor {
                                    key,
                                    timeout,
                                    value,
                                } => {
                                    Self::set_in(&mut self.instances, &mut frame!(), &dest, value);
                                    t.wake_at = self.time + timeout.max(0.0) as f64;
                                    t.waiting_for = Some(key);
                                    t.wait_dest = Some(dest.clone());
                                    return false;
                                }
                            }
                        }
                        Some((f, this, _)) => {
                            if t.frames.len() >= MAX_DEPTH {
                                self.warn_once(format!("stack overflow calling {}", f.name));
                                return true;
                            }
                            let nf = self.make_frame(f, this, &argv, Some(dest));
                            t.frames.push(nf);
                        }
                        None => {
                            Self::set_in(&mut self.instances, &mut frame!(), &dest, Value::None)
                        }
                    }
                }
                26 => {
                    let v = a(self, &frame!(), 0);
                    let f = t.frames.pop().unwrap();
                    if let (Some(caller), Some(ret)) = (t.frames.last_mut(), &f.ret) {
                        Self::set_in(&mut self.instances, caller, ret, v);
                    }
                }
                27 => {
                    let r = Value::str(&format!(
                        "{}{}",
                        a(self, &frame!(), 1),
                        a(self, &frame!(), 2)
                    ));
                    Self::set_in(&mut self.instances, &mut frame!(), &op.args[0], r);
                }
                28 | 29 => {
                    // propget [obj, dest]; propset [obj, value]
                    let obj = a(self, &frame!(), 0);
                    let name = op.name.clone().unwrap_or_else(|| Arc::from(""));
                    let Value::Object(o, cls) = obj else {
                        if op.code == 28 {
                            Self::set_in(
                                &mut self.instances,
                                &mut frame!(),
                                &op.args[1],
                                Value::None,
                            );
                        }
                        continue;
                    };
                    let ic = self.instance_class(host, o, &cls);
                    match self.find_property(host, &ic, &name) {
                        Some((_, Some(var), _)) => {
                            if op.code == 28 {
                                let v = self
                                    .instances
                                    .get(&(o, ic.clone()))
                                    .and_then(|i| i.vars.get(&var).cloned())
                                    .unwrap_or_default();
                                Self::set_in(&mut self.instances, &mut frame!(), &op.args[1], v);
                            } else {
                                let v = a(self, &frame!(), 1);
                                if let Some(inst) = self.instances.get_mut(&(o, ic.clone())) {
                                    inst.vars.insert(var, v);
                                }
                            }
                        }
                        Some((_, None, (read, write))) => {
                            let handler = if op.code == 28 { read } else { write };
                            if let Some(f) = handler {
                                let args = if op.code == 29 {
                                    vec![a(self, &frame!(), 1)]
                                } else {
                                    Vec::new()
                                };
                                let ret = if op.code == 28 {
                                    Some(op.args[1].clone())
                                } else {
                                    None
                                };
                                let nf = self.make_frame(f, Some((o, ic)), &args, ret);
                                t.frames.push(nf);
                            }
                        }
                        None => {
                            self.warn_once(format!("unknown property {ic}.{name}"));
                            if op.code == 28 {
                                Self::set_in(
                                    &mut self.instances,
                                    &mut frame!(),
                                    &op.args[1],
                                    Value::None,
                                );
                            }
                        }
                    }
                }
                30 => {
                    let n = a(self, &frame!(), 1).as_int().clamp(0, 128) as usize;
                    let ty = self.arg_type(host, &frame!(), &op.args[0]);
                    let elem = match ty.trim_end_matches("[]") {
                        "int" => Value::Int(0),
                        "float" => Value::Float(0.0),
                        "bool" => Value::Bool(false),
                        "string" => Value::str(""),
                        _ => Value::None,
                    };
                    let arr = Value::Array(Rc::new(RefCell::new(vec![elem; n])));
                    Self::set_in(&mut self.instances, &mut frame!(), &op.args[0], arr);
                }
                31 => {
                    let n = match a(self, &frame!(), 1) {
                        Value::Array(x) => x.borrow().len() as i32,
                        _ => 0,
                    };
                    Self::set_in(
                        &mut self.instances,
                        &mut frame!(),
                        &op.args[0],
                        Value::Int(n),
                    );
                }
                32 => {
                    let v = match (a(self, &frame!(), 1), a(self, &frame!(), 2)) {
                        (Value::Array(x), i) => x
                            .borrow()
                            .get(i.as_int().max(0) as usize)
                            .cloned()
                            .unwrap_or_default(),
                        _ => Value::None,
                    };
                    Self::set_in(&mut self.instances, &mut frame!(), &op.args[0], v);
                }
                33 => {
                    if let Value::Array(x) = a(self, &frame!(), 0) {
                        let i = a(self, &frame!(), 1).as_int();
                        let v = a(self, &frame!(), 2);
                        if let Some(slot) = x.borrow_mut().get_mut(i.max(0) as usize) {
                            *slot = v;
                        }
                    }
                }
                34 | 35 => {
                    let r = match a(self, &frame!(), 0) {
                        Value::Array(x) => {
                            let needle = a(self, &frame!(), 2);
                            let start = a(self, &frame!(), 3).as_int();
                            let v = x.borrow();
                            if op.code == 34 {
                                (start.max(0) as usize..v.len())
                                    .find(|&i| v[i] == needle)
                                    .map(|i| i as i32)
                                    .unwrap_or(-1)
                            } else {
                                let s = if start < 0 {
                                    v.len() as i32 - 1
                                } else {
                                    start.min(v.len() as i32 - 1)
                                };
                                (0..=s.max(-1))
                                    .rev()
                                    .find(|&i| i >= 0 && v[i as usize] == needle)
                                    .unwrap_or(-1)
                            }
                        }
                        _ => -1,
                    };
                    Self::set_in(
                        &mut self.instances,
                        &mut frame!(),
                        &op.args[1],
                        Value::Int(r),
                    );
                }
                other => {
                    self.warn_once(format!("bad opcode {other}"));
                    return true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variables_start_at_their_type_default() {
        // An `Int` with no initial value compares equal to 0 (`if pDone == 0`).
        assert!(type_default("int") == Value::Int(0));
        assert!(type_default("float") == Value::Float(0.0));
        assert!(type_default("bool") == Value::Bool(false));
        assert!(type_default("string") == Value::str(""));
        assert!(type_default("actor") == Value::None);
        assert!(type_default("int[]") == Value::None);
    }
}
