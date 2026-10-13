//! Building scripts instruction by instruction, for test mods: a
//! [`ScriptBuilder`] assembles one object into a [`Pex`] laid out as the
//! Creation Kit's compiler lays it out (its user flags, `GetState` /
//! `GotoState`, `::NoneVar`, auto properties backed by `::<name>_var`).
//!
//! ```
//! use vrm_papyrus::build::{FunctionBuilder, ScriptBuilder, discard, id, string};
//!
//! let mut s = ScriptBuilder::new("HelloScript", "Quest");
//! let mut f = FunctionBuilder::new("None");
//! f.call_static("Debug", "Notification", discard(), [string("Hello")]);
//! s.function("OnInit", f);
//! let bytes = vrm_papyrus::pex::write(&s.build().unwrap()).unwrap();
//! ```

use std::collections::HashMap;

use crate::pex::{
    self, Data, DebugFunction, DebugInfo, Function, GAME_SKYRIM, Header, Instruction, Object, Pex,
    Property, State, VERSION, Variable, op,
};
use crate::{Error, Result};

/// The local a call's unused result goes to.
pub const NONE_VAR: &str = "::NoneVar";

/// An instruction argument, by name rather than string table index.
#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    None,
    /// A variable, parameter, local, `self`, or a class / function name.
    Id(String),
    Str(String),
    Int(i32),
    Float(f32),
    Bool(bool),
}

/// A variable, parameter or local by name (`self` is one too).
pub fn id(name: &str) -> Arg {
    Arg::Id(name.to_owned())
}

/// A string literal.
pub fn string(s: &str) -> Arg {
    Arg::Str(s.to_owned())
}

/// Where a call's result goes when it isn't used.
pub fn discard() -> Arg {
    id(NONE_VAR)
}

/// The variable behind auto property `name` ([`ScriptBuilder::auto_property`]).
pub fn auto_var(name: &str) -> Arg {
    Arg::Id(format!("::{name}_var"))
}

impl From<i32> for Arg {
    fn from(v: i32) -> Self {
        Arg::Int(v)
    }
}

impl From<f32> for Arg {
    fn from(v: f32) -> Self {
        Arg::Float(v)
    }
}

impl From<bool> for Arg {
    fn from(v: bool) -> Self {
        Arg::Bool(v)
    }
}

/// A place in a function's code to jump to, bound with [`FunctionBuilder::bind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Label(usize);

#[derive(Debug, Clone)]
enum Operand {
    Arg(Arg),
    /// A jump's offset, relative to the jump.
    Label(Label),
}

/// A function's signature and code.
#[derive(Debug, Clone)]
pub struct FunctionBuilder {
    return_type: String,
    flags: u8,
    params: Vec<(String, String)>,
    locals: Vec<(String, String)>,
    code: Vec<(u8, Vec<Operand>)>,
    labels: Vec<Option<usize>>,
}

impl FunctionBuilder {
    /// A function returning `return_type` (`None` for nothing).
    pub fn new(return_type: &str) -> Self {
        FunctionBuilder {
            return_type: return_type.to_owned(),
            flags: 0,
            params: Vec::new(),
            locals: Vec::new(),
            code: Vec::new(),
            labels: Vec::new(),
        }
    }

    /// Declared `Global`: called on the class, with no `self`.
    pub fn global(mut self) -> Self {
        self.flags |= 1;
        self
    }

    pub fn param(&mut self, name: &str, type_name: &str) -> Arg {
        self.params.push((name.to_owned(), type_name.to_owned()));
        id(name)
    }

    /// A local variable; the compiler's temporaries are named `::temp<n>`.
    pub fn local(&mut self, name: &str, type_name: &str) -> Arg {
        self.locals.push((name.to_owned(), type_name.to_owned()));
        id(name)
    }

    /// Any instruction, its arguments as [`pex::op_args`] lays them out.
    pub fn op(&mut self, op: u8, args: impl IntoIterator<Item = Arg>) -> &mut Self {
        self.code
            .push((op, args.into_iter().map(Operand::Arg).collect()));
        self
    }

    fn call(&mut self, op: u8, fixed: Vec<Arg>, args: impl IntoIterator<Item = Arg>) -> &mut Self {
        let args: Vec<Arg> = args.into_iter().collect();
        let mut all = fixed;
        all.push(Arg::Int(args.len() as i32));
        all.extend(args);
        self.op(op, all)
    }

    /// `dest = obj.name(args)`.
    pub fn call_method(
        &mut self,
        obj: Arg,
        name: &str,
        dest: Arg,
        args: impl IntoIterator<Item = Arg>,
    ) -> &mut Self {
        self.call(op::CALLMETHOD, vec![id(name), obj, dest], args)
    }

    /// `dest = class.name(args)`, a global function.
    pub fn call_static(
        &mut self,
        class: &str,
        name: &str,
        dest: Arg,
        args: impl IntoIterator<Item = Arg>,
    ) -> &mut Self {
        self.call(op::CALLSTATIC, vec![id(class), id(name), dest], args)
    }

    /// `dest = parent.name(args)`.
    pub fn call_parent(
        &mut self,
        name: &str,
        dest: Arg,
        args: impl IntoIterator<Item = Arg>,
    ) -> &mut Self {
        self.call(op::CALLPARENT, vec![id(name), dest], args)
    }

    pub fn assign(&mut self, dest: Arg, value: Arg) -> &mut Self {
        self.op(op::ASSIGN, [dest, value])
    }

    /// `dest = value as <dest's type>`.
    pub fn cast(&mut self, dest: Arg, value: Arg) -> &mut Self {
        self.op(op::CAST, [dest, value])
    }

    /// `dest = a + b` on strings.
    pub fn strcat(&mut self, dest: Arg, a: Arg, b: Arg) -> &mut Self {
        self.op(op::STRCAT, [dest, a, b])
    }

    pub fn ret(&mut self, value: Arg) -> &mut Self {
        self.op(op::RETURN, [value])
    }

    /// A label to bind later, where jumps to it land.
    pub fn label(&mut self) -> Label {
        self.labels.push(None);
        Label(self.labels.len() - 1)
    }

    /// Lands `label` on the next instruction.
    pub fn bind(&mut self, label: Label) -> &mut Self {
        assert!(self.labels[label.0].is_none(), "label bound twice");
        self.labels[label.0] = Some(self.code.len());
        self
    }

    pub fn jmp(&mut self, to: Label) -> &mut Self {
        self.code.push((op::JMP, vec![Operand::Label(to)]));
        self
    }

    /// Jump if `cond` is true.
    pub fn jmpt(&mut self, cond: Arg, to: Label) -> &mut Self {
        self.code
            .push((op::JMPT, vec![Operand::Arg(cond), Operand::Label(to)]));
        self
    }

    /// Jump if `cond` is false.
    pub fn jmpf(&mut self, cond: Arg, to: Label) -> &mut Self {
        self.code
            .push((op::JMPF, vec![Operand::Arg(cond), Operand::Label(to)]));
        self
    }

    /// Whether `::NoneVar` is used without being declared.
    fn needs_none_var(&self) -> bool {
        let none_var = Arg::Id(NONE_VAR.to_owned());
        !self.locals.iter().any(|(n, _)| n == NONE_VAR)
            && self
                .code
                .iter()
                .flat_map(|(_, a)| a)
                .any(|a| matches!(a, Operand::Arg(a) if *a == none_var))
    }
}

/// An auto property or variable's initial value.
#[derive(Debug, Clone)]
struct Var {
    name: String,
    type_name: String,
    init: Arg,
}

/// One script object: its variables, auto properties and functions by state.
#[derive(Debug, Clone)]
pub struct ScriptBuilder {
    name: String,
    parent: String,
    auto_state: String,
    variables: Vec<Var>,
    /// Name, type: each backed by the variable `::<name>_var`.
    properties: Vec<(String, String)>,
    /// State (`""` the empty state), then its functions.
    states: Vec<(String, Vec<(String, FunctionBuilder)>)>,
}

impl ScriptBuilder {
    /// `Scriptname name extends parent`.
    pub fn new(name: &str, parent: &str) -> Self {
        ScriptBuilder {
            name: name.to_owned(),
            parent: parent.to_owned(),
            auto_state: String::new(),
            variables: Vec::new(),
            properties: Vec::new(),
            states: vec![(String::new(), Vec::new())],
        }
    }

    /// The state an instance starts in (`Auto State`).
    pub fn auto_state(&mut self, state: &str) -> &mut Self {
        self.auto_state = state.to_owned();
        self.state_index(state);
        self
    }

    /// A script variable; `init` is [`Arg::None`] for the type's default.
    pub fn variable(&mut self, name: &str, type_name: &str, init: Arg) -> Arg {
        self.variables.push(Var {
            name: name.to_owned(),
            type_name: type_name.to_owned(),
            init,
        });
        id(name)
    }

    /// `type_name Property name Auto`, which a form's VMAD fills; returns
    /// its variable, as [`auto_var`].
    pub fn auto_property(&mut self, name: &str, type_name: &str) -> Arg {
        let var = auto_var(name);
        let Arg::Id(v) = &var else { unreachable!() };
        self.variable(v, type_name, Arg::None);
        self.properties
            .push((name.to_owned(), type_name.to_owned()));
        var
    }

    /// A function (or event) in the empty state.
    pub fn function(&mut self, name: &str, f: FunctionBuilder) -> &mut Self {
        self.state_function("", name, f)
    }

    /// A function (or event) in `state`.
    pub fn state_function(&mut self, state: &str, name: &str, f: FunctionBuilder) -> &mut Self {
        let i = self.state_index(state);
        self.states[i].1.push((name.to_owned(), f));
        self
    }

    fn state_index(&mut self, state: &str) -> usize {
        match self.states.iter().position(|(s, _)| s == state) {
            Some(i) => i,
            None => {
                self.states.push((state.to_owned(), Vec::new()));
                self.states.len() - 1
            }
        }
    }

    /// The compiled script, compiled now by `vibe_rm`.
    pub fn build(&self) -> Result<Pex> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let mut b = Build::default();
        let name = b.str(&self.name)?;
        b.str("")?;

        // What every compiled script has: `GetState` and `GotoState`, on the
        // state the VM keeps as `::State`.
        let mut get_state = FunctionBuilder::new("String");
        get_state.ret(id("::State"));
        let mut goto_state = FunctionBuilder::new("None");
        let new_state = goto_state.param("newState", "String");
        goto_state
            .call_method(id("self"), "onEndState", discard(), [])
            .assign(id("::State"), new_state)
            .call_method(id("self"), "onBeginState", discard(), []);
        let mut empty_state = vec![
            ("GetState".to_owned(), get_state, false),
            ("GotoState".to_owned(), goto_state, false),
        ];
        empty_state.retain(|(n, _, _)| {
            !self.states[0]
                .1
                .iter()
                .any(|(m, _)| m.eq_ignore_ascii_case(n))
        });
        // The user flags the compiler always lists.
        let user_flags = vec![(b.str("conditional")?, 1), (b.str("hidden")?, 0)];

        let mut debug = Vec::new();
        let mut states = Vec::new();
        for (i, (sname, functions)) in self.states.iter().enumerate() {
            let state = b.str(sname)?;
            let mut out = Vec::new();
            let generated = if i == 0 { &empty_state[..] } else { &[] };
            let ours = functions.iter().map(|(n, f)| (n.clone(), f.clone(), true));
            for (fname, f, numbered) in generated.iter().cloned().chain(ours) {
                let fid = b.str(&fname)?;
                let f = b.function(&f).map_err(|e| match e {
                    Error::Write(m) => Error::Write(format!("{}.{fname}: {m}", self.name)),
                    e => e,
                })?;
                // Our "source lines" are instruction numbers; the
                // compiler's own functions have none.
                let lines = if numbered {
                    (1..=f.code.len() as u16).collect()
                } else {
                    Vec::new()
                };
                debug.push(DebugFunction {
                    object: name,
                    state,
                    function: fid,
                    kind: 0,
                    lines,
                });
                out.push((fid, f));
            }
            states.push(State {
                name: state,
                functions: out,
            });
        }

        let mut variables = Vec::new();
        for v in &self.variables {
            variables.push(Variable {
                name: b.str(&v.name)?,
                type_name: b.str(&v.type_name)?,
                user_flags: 0,
                init: b.data(&v.init)?,
            });
        }
        let mut properties = Vec::new();
        for (pname, ty) in &self.properties {
            let Arg::Id(var) = auto_var(pname) else {
                unreachable!()
            };
            properties.push(Property {
                name: b.str(pname)?,
                type_name: b.str(ty)?,
                doc: b.str("")?,
                user_flags: 0,
                // Readable, writable, auto.
                flags: 7,
                auto_var: Some(b.str(&var)?),
                read: None,
                write: None,
            });
        }
        let object = Object {
            name,
            parent: b.str(&self.parent)?,
            doc: b.str("")?,
            user_flags: 0,
            auto_state: b.str(&self.auto_state)?,
            variables,
            properties,
            states,
        };
        Ok(Pex {
            header: Header {
                major: VERSION.0,
                minor: VERSION.1,
                game: GAME_SKYRIM,
                compile_time: now,
                user: "vibe_rm".into(),
                machine: "vibe_rm".into(),
            },
            source: format!("{}.psc", self.name),
            strings: b.strings,
            debug: Some(DebugInfo {
                modification_time: now,
                functions: debug,
            }),
            user_flags,
            objects: vec![object],
        })
    }
}

/// The string table as it fills.
#[derive(Default)]
struct Build {
    strings: Vec<String>,
    index: HashMap<String, u16>,
}

impl Build {
    fn str(&mut self, s: &str) -> Result<u16> {
        if let Some(&i) = self.index.get(s) {
            return Ok(i);
        }
        let i = u16::try_from(self.strings.len())
            .map_err(|_| Error::Write("over 65536 strings".into()))?;
        self.strings.push(s.to_owned());
        self.index.insert(s.to_owned(), i);
        Ok(i)
    }

    fn data(&mut self, a: &Arg) -> Result<Data> {
        Ok(match a {
            Arg::None => Data::Null,
            Arg::Id(s) => Data::Identifier(self.str(s)?),
            Arg::Str(s) => Data::String(self.str(s)?),
            Arg::Int(v) => Data::Int(*v),
            Arg::Float(v) => Data::Float(*v),
            Arg::Bool(v) => Data::Bool(*v),
        })
    }

    fn function(&mut self, f: &FunctionBuilder) -> Result<Function> {
        let mut locals = f.locals.clone();
        if f.needs_none_var() {
            locals.push((NONE_VAR.to_owned(), "None".to_owned()));
        }
        let mut code = Vec::with_capacity(f.code.len());
        for (at, (op, operands)) in f.code.iter().enumerate() {
            let mut args = Vec::with_capacity(operands.len());
            for o in operands {
                args.push(match o {
                    Operand::Arg(a) => self.data(a)?,
                    Operand::Label(l) => {
                        let to = f.labels[l.0]
                            .ok_or_else(|| Error::Write(format!("label {} never bound", l.0)))?;
                        Data::Int(to as i32 - at as i32)
                    }
                });
            }
            let (n, var) = pex::op_args(*op).ok_or_else(|| Error::Write(format!("opcode {op}")))?;
            if args.len() < n || (!var && args.len() != n) {
                return Err(Error::Write(format!(
                    "{} takes {n} arguments, given {}",
                    pex::OP_NAMES[*op as usize],
                    args.len()
                )));
            }
            code.push(Instruction { op: *op, args });
        }
        let mut pairs = |v: &[(String, String)]| -> Result<Vec<(u16, u16)>> {
            v.iter()
                .map(|(n, t)| Ok((self.str(n)?, self.str(t)?)))
                .collect()
        };
        let params = pairs(&f.params)?;
        let locals = pairs(&locals)?;
        Ok(Function {
            return_type: self.str(&f.return_type)?,
            doc: self.str("")?,
            user_flags: 0,
            flags: f.flags,
            params,
            locals,
            code,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::{Host, NativeResult, Vm};
    use crate::{ObjectId, Value};

    /// Loads the built scripts; records `Debug.Trace` and answers
    /// `Game.GetPlayer` / `Actor.GetLevel`.
    struct TestHost {
        scripts: HashMap<String, Vec<u8>>,
        traced: Vec<String>,
    }

    impl Host for TestHost {
        fn load_script(&mut self, name: &str) -> Option<Vec<u8>> {
            self.scripts.get(&name.to_ascii_lowercase()).cloned()
        }
        fn call_native(
            &mut self,
            class: &str,
            func: &str,
            _this: Option<&Value>,
            args: &[Value],
        ) -> NativeResult {
            match (class, func.to_ascii_lowercase().as_str()) {
                ("debug", "trace") => {
                    if let Some(Value::String(s)) = args.first() {
                        self.traced.push(s.to_string());
                    }
                    NativeResult::Value(Value::None)
                }
                ("game", "getplayer") => {
                    NativeResult::Value(Value::Object(ObjectId::Form(0x14), "Actor".into()))
                }
                ("actor", "getlevel") => NativeResult::Value(Value::Int(7)),
                _ => NativeResult::Value(Value::None),
            }
        }
        fn is_native_type(&self, _obj: ObjectId, _class: &str) -> bool {
            true
        }
    }

    /// A native function: name, return type, parameters (name, type).
    type Native<'a> = (&'a str, &'a str, &'a [(&'a str, &'a str)]);

    fn natives(class: &str, functions: &[Native]) -> Vec<u8> {
        // Native classes as the vanilla scripts declare them: global
        // natives (flags 3) or native methods (2).
        let mut s = ScriptBuilder::new(class, "");
        for (name, ret, params) in functions {
            let mut f = FunctionBuilder::new(ret);
            for (p, t) in *params {
                f.param(p, t);
            }
            f.flags = if class == "Actor" { 2 } else { 3 };
            s.function(name, f);
        }
        let mut p = s.build().unwrap();
        // Native classes have neither GetState nor GotoState.
        p.objects[0].states[0]
            .functions
            .retain(|(_, f)| f.is_native());
        pex::write(&p).unwrap()
    }

    #[test]
    fn a_built_script_runs() {
        // OnInit: trace "level " + Game.GetPlayer().GetLevel(); then trace
        // whether the auto property Count is over 2, by a conditional jump.
        let mut s = ScriptBuilder::new("BuildTestScript", "Quest");
        let count = s.auto_property("Count", "Int");
        let mut f = FunctionBuilder::new("None");
        let player = f.local("::temp0", "Actor");
        let level = f.local("::temp1", "Int");
        let text = f.local("::temp2", "String");
        let over = f.local("::temp3", "Bool");
        let (no, end) = (f.label(), f.label());
        f.call_static("Game", "GetPlayer", player.clone(), [])
            .call_method(player, "GetLevel", level.clone(), [])
            .cast(text.clone(), level)
            .strcat(text.clone(), string("level "), text)
            .call_static("Debug", "Trace", discard(), [id("::temp2"), 0.into()])
            .op(op::CMP_GT, [over.clone(), count, 2.into()])
            .jmpf(over, no)
            .call_static("Debug", "Trace", discard(), [string("over"), 0.into()])
            .jmp(end)
            .bind(no)
            .call_static("Debug", "Trace", discard(), [string("not over"), 0.into()])
            .bind(end)
            .ret(Arg::None);
        s.function("OnInit", f);
        let pex = s.build().unwrap();
        let bytes = pex::write(&pex).unwrap();
        // It reads back as built, and writes back the same.
        assert_eq!(pex::write(&pex::parse(&bytes).unwrap()).unwrap(), bytes);

        let mut host = TestHost {
            scripts: HashMap::from([
                ("buildtestscript".to_owned(), bytes),
                ("quest".to_owned(), natives("Quest", &[])),
                (
                    "debug".to_owned(),
                    natives(
                        "Debug",
                        &[("Trace", "None", &[("s", "String"), ("i", "Int")])],
                    ),
                ),
                (
                    "game".to_owned(),
                    natives("Game", &[("GetPlayer", "Actor", &[])]),
                ),
                (
                    "actor".to_owned(),
                    natives("Actor", &[("GetLevel", "Int", &[])]),
                ),
            ]),
            traced: Vec::new(),
        };
        let mut vm = Vm::new();
        let quest = ObjectId::Form(0x0100_0800);
        assert!(vm.attach(
            &mut host,
            quest,
            "BuildTestScript",
            &[("Count".to_owned(), Value::Int(3))]
        ));
        assert_eq!(vm.send_event(&mut host, quest, "OnInit", vec![]), 1);
        vm.run(&mut host, 0.0, 1000);
        assert_eq!(host.traced, ["level 7", "over"]);
    }

    #[test]
    fn unbound_labels_are_an_error() {
        let mut s = ScriptBuilder::new("BadScript", "Quest");
        let mut f = FunctionBuilder::new("None");
        let l = f.label();
        f.jmp(l);
        s.function("OnInit", f);
        assert!(s.build().is_err());
    }
}
