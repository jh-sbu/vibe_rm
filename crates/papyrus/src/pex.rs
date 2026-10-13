//! Compiled Papyrus script (`.pex`) parsing and writing. Skyrim PEX files
//! are big-endian.

use crate::{Error, Result};

/// The magic number opening every PEX file.
pub const MAGIC: u32 = 0xFA57_C0DE;
/// Skyrim's format version (major, minor) and game id.
pub const VERSION: (u8, u8) = (3, 2);
pub const GAME_SKYRIM: u16 = 1;

/// Opcodes, as [`OP_NAMES`] names them.
pub mod op {
    pub const NOP: u8 = 0;
    pub const IADD: u8 = 1;
    pub const FADD: u8 = 2;
    pub const ISUB: u8 = 3;
    pub const FSUB: u8 = 4;
    pub const IMUL: u8 = 5;
    pub const FMUL: u8 = 6;
    pub const IDIV: u8 = 7;
    pub const FDIV: u8 = 8;
    pub const IMOD: u8 = 9;
    pub const NOT: u8 = 10;
    pub const INEG: u8 = 11;
    pub const FNEG: u8 = 12;
    pub const ASSIGN: u8 = 13;
    pub const CAST: u8 = 14;
    pub const CMP_EQ: u8 = 15;
    pub const CMP_LT: u8 = 16;
    pub const CMP_LE: u8 = 17;
    pub const CMP_GT: u8 = 18;
    pub const CMP_GE: u8 = 19;
    pub const JMP: u8 = 20;
    pub const JMPT: u8 = 21;
    pub const JMPF: u8 = 22;
    pub const CALLMETHOD: u8 = 23;
    pub const CALLPARENT: u8 = 24;
    pub const CALLSTATIC: u8 = 25;
    pub const RETURN: u8 = 26;
    pub const STRCAT: u8 = 27;
    pub const PROPGET: u8 = 28;
    pub const PROPSET: u8 = 29;
    pub const ARRAY_CREATE: u8 = 30;
    pub const ARRAY_LENGTH: u8 = 31;
    pub const ARRAY_GETELEMENT: u8 = 32;
    pub const ARRAY_SETELEMENT: u8 = 33;
    pub const ARRAY_FINDELEMENT: u8 = 34;
    pub const ARRAY_RFINDELEMENT: u8 = 35;
}

#[derive(Debug, Clone, PartialEq)]
pub enum Data {
    Null,
    Identifier(u16),
    String(u16),
    Int(i32),
    Float(f32),
    Bool(bool),
}

#[derive(Debug, Clone)]
pub struct Instruction {
    pub op: u8,
    pub args: Vec<Data>,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub return_type: u16,
    pub doc: u16,
    pub user_flags: u32,
    pub flags: u8,
    pub params: Vec<(u16, u16)>,
    pub locals: Vec<(u16, u16)>,
    pub code: Vec<Instruction>,
}

impl Function {
    pub fn is_global(&self) -> bool {
        self.flags & 1 != 0
    }
    pub fn is_native(&self) -> bool {
        self.flags & 2 != 0
    }
}

#[derive(Debug, Clone)]
pub struct Variable {
    pub name: u16,
    pub type_name: u16,
    pub user_flags: u32,
    pub init: Data,
}

#[derive(Debug, Clone)]
pub struct Property {
    pub name: u16,
    pub type_name: u16,
    pub doc: u16,
    pub user_flags: u32,
    pub flags: u8,
    pub auto_var: Option<u16>,
    pub read: Option<Function>,
    pub write: Option<Function>,
}

#[derive(Debug, Clone)]
pub struct State {
    pub name: u16,
    pub functions: Vec<(u16, Function)>,
}

#[derive(Debug, Clone)]
pub struct Object {
    pub name: u16,
    pub parent: u16,
    pub doc: u16,
    pub user_flags: u32,
    pub auto_state: u16,
    pub variables: Vec<Variable>,
    pub properties: Vec<Property>,
    pub states: Vec<State>,
}

/// The file header: format version, when and where it was compiled.
#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub major: u8,
    pub minor: u8,
    pub game: u16,
    /// Unix time.
    pub compile_time: u64,
    pub user: String,
    pub machine: String,
}

/// A function's source line of each instruction.
#[derive(Debug, Clone, PartialEq)]
pub struct DebugFunction {
    pub object: u16,
    pub state: u16,
    pub function: u16,
    /// 0 a function, 1 a property getter, 2 a setter.
    pub kind: u8,
    pub lines: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DebugInfo {
    /// Unix time the source was modified.
    pub modification_time: u64,
    pub functions: Vec<DebugFunction>,
}

#[derive(Debug, Clone)]
pub struct Pex {
    pub header: Header,
    pub source: String,
    pub strings: Vec<String>,
    pub debug: Option<DebugInfo>,
    /// User flags the compiler knew: name, bit index (`hidden` 0,
    /// `conditional` 1).
    pub user_flags: Vec<(u16, u8)>,
    pub objects: Vec<Object>,
}

impl Pex {
    pub fn str(&self, i: u16) -> &str {
        self.strings
            .get(i as usize)
            .map(String::as_str)
            .unwrap_or("")
    }
}

/// Number of fixed arguments per opcode, and whether it takes varargs.
pub fn op_args(op: u8) -> Option<(usize, bool)> {
    Some(match op {
        0 => (0, false),
        1..=9 => (3, false),
        10..=14 => (2, false),
        15..=19 => (3, false),
        20 => (1, false),
        21 | 22 => (2, false),
        23 => (3, true),
        24 => (2, true),
        25 => (3, true),
        26 => (1, false),
        27..=29 => (3, false),
        30 | 31 => (2, false),
        32 | 33 => (3, false),
        34 | 35 => (4, false),
        _ => return None,
    })
}

pub const OP_NAMES: [&str; 36] = [
    "nop",
    "iadd",
    "fadd",
    "isub",
    "fsub",
    "imul",
    "fmul",
    "idiv",
    "fdiv",
    "imod",
    "not",
    "ineg",
    "fneg",
    "assign",
    "cast",
    "cmp_eq",
    "cmp_lt",
    "cmp_le",
    "cmp_gt",
    "cmp_ge",
    "jmp",
    "jmpt",
    "jmpf",
    "callmethod",
    "callparent",
    "callstatic",
    "return",
    "strcat",
    "propget",
    "propset",
    "array_create",
    "array_length",
    "array_getelement",
    "array_setelement",
    "array_findelement",
    "array_rfindelement",
];

struct R<'a> {
    d: &'a [u8],
    p: usize,
}

impl R<'_> {
    fn need(&self, n: usize) -> Result<()> {
        if self.p + n > self.d.len() {
            Err(Error::Corrupt(format!("unexpected end at {}", self.p)))
        } else {
            Ok(())
        }
    }
    fn u8(&mut self) -> Result<u8> {
        self.need(1)?;
        self.p += 1;
        Ok(self.d[self.p - 1])
    }
    fn u16(&mut self) -> Result<u16> {
        self.need(2)?;
        self.p += 2;
        Ok(u16::from_be_bytes([self.d[self.p - 2], self.d[self.p - 1]]))
    }
    fn u32(&mut self) -> Result<u32> {
        self.need(4)?;
        self.p += 4;
        Ok(u32::from_be_bytes(
            self.d[self.p - 4..self.p].try_into().unwrap(),
        ))
    }
    fn u64(&mut self) -> Result<u64> {
        self.need(8)?;
        self.p += 8;
        Ok(u64::from_be_bytes(
            self.d[self.p - 8..self.p].try_into().unwrap(),
        ))
    }
    fn wstring(&mut self) -> Result<String> {
        let n = self.u16()? as usize;
        self.need(n)?;
        let s = self.d[self.p..self.p + n]
            .iter()
            .map(|&c| c as char)
            .collect();
        self.p += n;
        Ok(s)
    }
    fn data(&mut self) -> Result<Data> {
        Ok(match self.u8()? {
            0 => Data::Null,
            1 => Data::Identifier(self.u16()?),
            2 => Data::String(self.u16()?),
            3 => Data::Int(self.u32()? as i32),
            4 => Data::Float(f32::from_bits(self.u32()?)),
            5 => Data::Bool(self.u8()? != 0),
            t => return Err(Error::Corrupt(format!("bad variable data type {t}"))),
        })
    }
    fn function(&mut self) -> Result<Function> {
        let return_type = self.u16()?;
        let doc = self.u16()?;
        let user_flags = self.u32()?;
        let flags = self.u8()?;
        let np = self.u16()?;
        let mut params = Vec::with_capacity(np as usize);
        for _ in 0..np {
            params.push((self.u16()?, self.u16()?));
        }
        let nl = self.u16()?;
        let mut locals = Vec::with_capacity(nl as usize);
        for _ in 0..nl {
            locals.push((self.u16()?, self.u16()?));
        }
        let ni = self.u16()?;
        let mut code = Vec::with_capacity(ni as usize);
        for _ in 0..ni {
            let op = self.u8()?;
            let (n, var) = op_args(op).ok_or_else(|| Error::Corrupt(format!("bad opcode {op}")))?;
            let mut args = Vec::with_capacity(n + 2);
            for _ in 0..n {
                args.push(self.data()?);
            }
            if var {
                let count = self.data()?;
                let k = match count {
                    Data::Int(k) => k.max(0) as usize,
                    _ => return Err(Error::Corrupt("vararg count is not an integer".into())),
                };
                args.push(count);
                for _ in 0..k {
                    args.push(self.data()?);
                }
            }
            code.push(Instruction { op, args });
        }
        Ok(Function {
            return_type,
            doc,
            user_flags,
            flags,
            params,
            locals,
            code,
        })
    }
}

pub fn parse(d: &[u8]) -> Result<Pex> {
    let mut r = R { d, p: 0 };
    if r.u32()? != MAGIC {
        return Err(Error::Corrupt("bad magic".into()));
    }
    let major = r.u8()?;
    let minor = r.u8()?;
    let game = r.u16()?;
    let compile_time = r.u64()?;
    let source = r.wstring()?;
    let header = Header {
        major,
        minor,
        game,
        compile_time,
        user: r.wstring()?,
        machine: r.wstring()?,
    };
    let n = r.u16()?;
    let mut strings = Vec::with_capacity(n as usize);
    for _ in 0..n {
        strings.push(r.wstring()?);
    }
    let debug = if r.u8()? != 0 {
        let modification_time = r.u64()?;
        let nf = r.u16()?;
        let mut functions = Vec::with_capacity(nf as usize);
        for _ in 0..nf {
            let object = r.u16()?;
            let state = r.u16()?;
            let function = r.u16()?;
            let kind = r.u8()?;
            let ic = r.u16()?;
            let mut lines = Vec::with_capacity(ic as usize);
            for _ in 0..ic {
                lines.push(r.u16()?);
            }
            functions.push(DebugFunction {
                object,
                state,
                function,
                kind,
                lines,
            });
        }
        Some(DebugInfo {
            modification_time,
            functions,
        })
    } else {
        None
    };
    let nu = r.u16()?;
    let mut user_flags = Vec::with_capacity(nu as usize);
    for _ in 0..nu {
        user_flags.push((r.u16()?, r.u8()?));
    }
    let no = r.u16()?;
    let mut objects = Vec::with_capacity(no as usize);
    for _ in 0..no {
        let name = r.u16()?;
        let _size = r.u32()?;
        let parent = r.u16()?;
        let doc = r.u16()?;
        let user_flags = r.u32()?;
        let auto_state = r.u16()?;
        let nv = r.u16()?;
        let mut variables = Vec::with_capacity(nv as usize);
        for _ in 0..nv {
            variables.push(Variable {
                name: r.u16()?,
                type_name: r.u16()?,
                user_flags: r.u32()?,
                init: r.data()?,
            });
        }
        let np = r.u16()?;
        let mut properties = Vec::with_capacity(np as usize);
        for _ in 0..np {
            let name = r.u16()?;
            let type_name = r.u16()?;
            let doc = r.u16()?;
            let user_flags = r.u32()?;
            let flags = r.u8()?;
            let mut p = Property {
                name,
                type_name,
                doc,
                user_flags,
                flags,
                auto_var: None,
                read: None,
                write: None,
            };
            if flags & 4 != 0 {
                p.auto_var = Some(r.u16()?);
            } else {
                if flags & 1 != 0 {
                    p.read = Some(r.function()?);
                }
                if flags & 2 != 0 {
                    p.write = Some(r.function()?);
                }
            }
            properties.push(p);
        }
        let ns = r.u16()?;
        let mut states = Vec::with_capacity(ns as usize);
        for _ in 0..ns {
            let sname = r.u16()?;
            let nf = r.u16()?;
            let mut functions = Vec::with_capacity(nf as usize);
            for _ in 0..nf {
                let fname = r.u16()?;
                functions.push((fname, r.function()?));
            }
            states.push(State {
                name: sname,
                functions,
            });
        }
        objects.push(Object {
            name,
            parent,
            doc,
            user_flags,
            auto_state,
            variables,
            properties,
            states,
        });
    }
    Ok(Pex {
        header,
        source,
        strings,
        debug,
        user_flags,
        objects,
    })
}

/// Big-endian output, as [`R`] reads it.
struct W(Vec<u8>);

impl W {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_be_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_be_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_be_bytes());
    }
    fn count(&mut self, n: usize, what: &str) -> Result<()> {
        let n = u16::try_from(n).map_err(|_| Error::Write(format!("{n} {what}")))?;
        self.u16(n);
        Ok(())
    }
    /// A string of one byte per character, as [`R::wstring`] reads it.
    fn wstring(&mut self, s: &str) -> Result<()> {
        let b = s
            .chars()
            .map(u8::try_from)
            .collect::<std::result::Result<Vec<u8>, _>>()
            .map_err(|_| Error::Write(format!("{s:?}: not one byte per character")))?;
        self.count(b.len(), "bytes in a string")?;
        self.0.extend_from_slice(&b);
        Ok(())
    }
    fn data(&mut self, d: &Data) {
        match *d {
            Data::Null => self.u8(0),
            Data::Identifier(i) => {
                self.u8(1);
                self.u16(i);
            }
            Data::String(i) => {
                self.u8(2);
                self.u16(i);
            }
            Data::Int(v) => {
                self.u8(3);
                self.u32(v as u32);
            }
            Data::Float(v) => {
                self.u8(4);
                self.u32(v.to_bits());
            }
            Data::Bool(b) => {
                self.u8(5);
                self.u8(b.into());
            }
        }
    }
    fn function(&mut self, f: &Function) -> Result<()> {
        self.u16(f.return_type);
        self.u16(f.doc);
        self.u32(f.user_flags);
        self.u8(f.flags);
        self.count(f.params.len(), "parameters")?;
        for &(n, t) in &f.params {
            self.u16(n);
            self.u16(t);
        }
        self.count(f.locals.len(), "locals")?;
        for &(n, t) in &f.locals {
            self.u16(n);
            self.u16(t);
        }
        self.count(f.code.len(), "instructions")?;
        for ins in &f.code {
            let (n, var) =
                op_args(ins.op).ok_or_else(|| Error::Write(format!("bad opcode {}", ins.op)))?;
            // Fixed arguments, then for varargs a count and that many more.
            let extra = match ins.args.get(n) {
                Some(&Data::Int(k)) if var => k.max(0) as usize + 1,
                _ => 0,
            };
            if ins.args.len() != n + extra || (var && extra == 0) {
                return Err(Error::Write(format!(
                    "{}: {} arguments",
                    OP_NAMES[ins.op as usize],
                    ins.args.len()
                )));
            }
            self.u8(ins.op);
            for a in &ins.args {
                self.data(a);
            }
        }
        Ok(())
    }
}

/// The file [`parse`] reads back as `p`. Vanilla scripts write back
/// byte for byte.
pub fn write(p: &Pex) -> Result<Vec<u8>> {
    let mut w = W(Vec::new());
    w.u32(MAGIC);
    w.u8(p.header.major);
    w.u8(p.header.minor);
    w.u16(p.header.game);
    w.u64(p.header.compile_time);
    w.wstring(&p.source)?;
    w.wstring(&p.header.user)?;
    w.wstring(&p.header.machine)?;
    w.count(p.strings.len(), "strings")?;
    for s in &p.strings {
        w.wstring(s)?;
    }
    match &p.debug {
        None => w.u8(0),
        Some(d) => {
            w.u8(1);
            w.u64(d.modification_time);
            w.count(d.functions.len(), "debug functions")?;
            for f in &d.functions {
                w.u16(f.object);
                w.u16(f.state);
                w.u16(f.function);
                w.u8(f.kind);
                w.count(f.lines.len(), "lines")?;
                for &l in &f.lines {
                    w.u16(l);
                }
            }
        }
    }
    w.count(p.user_flags.len(), "user flags")?;
    for &(name, bit) in &p.user_flags {
        w.u16(name);
        w.u8(bit);
    }
    w.count(p.objects.len(), "objects")?;
    for o in &p.objects {
        w.u16(o.name);
        // The object's size, counting this field: patched below.
        let start = w.0.len();
        w.u32(0);
        w.u16(o.parent);
        w.u16(o.doc);
        w.u32(o.user_flags);
        w.u16(o.auto_state);
        w.count(o.variables.len(), "variables")?;
        for v in &o.variables {
            w.u16(v.name);
            w.u16(v.type_name);
            w.u32(v.user_flags);
            w.data(&v.init);
        }
        w.count(o.properties.len(), "properties")?;
        for pr in &o.properties {
            w.u16(pr.name);
            w.u16(pr.type_name);
            w.u16(pr.doc);
            w.u32(pr.user_flags);
            w.u8(pr.flags);
            if pr.flags & 4 != 0 {
                let var = pr
                    .auto_var
                    .ok_or_else(|| Error::Write("auto property without a variable".into()))?;
                w.u16(var);
            } else {
                for (bit, f) in [(1, &pr.read), (2, &pr.write)] {
                    match f {
                        Some(f) if pr.flags & bit != 0 => w.function(f)?,
                        None if pr.flags & bit == 0 => {}
                        _ => {
                            return Err(Error::Write(
                                "property accessors disagree with its flags".into(),
                            ));
                        }
                    }
                }
            }
        }
        w.count(o.states.len(), "states")?;
        for st in &o.states {
            w.u16(st.name);
            w.count(st.functions.len(), "functions")?;
            for (name, f) in &st.functions {
                w.u16(*name);
                w.function(f)?;
            }
        }
        let size = (w.0.len() - start) as u32;
        w.0[start..start + 4].copy_from_slice(&size.to_be_bytes());
    }
    Ok(w.0)
}

/// Human-readable disassembly, for tooling.
pub fn disassemble(p: &Pex) -> String {
    use std::fmt::Write;
    let mut o = String::new();
    let data = |d: &Data| -> String {
        match d {
            Data::Null => "none".into(),
            Data::Identifier(i) => p.str(*i).to_owned(),
            Data::String(i) => format!("{:?}", p.str(*i)),
            Data::Int(v) => v.to_string(),
            Data::Float(v) => format!("{v:?}"),
            Data::Bool(b) => b.to_string(),
        }
    };
    for obj in &p.objects {
        let _ = writeln!(
            o,
            "scriptname {} extends {}",
            p.str(obj.name),
            p.str(obj.parent)
        );
        for v in &obj.variables {
            let _ = writeln!(
                o,
                "  var {} {} = {}",
                p.str(v.type_name),
                p.str(v.name),
                data(&v.init)
            );
        }
        for pr in &obj.properties {
            let _ = writeln!(
                o,
                "  property {} {} auto={:?}",
                p.str(pr.type_name),
                p.str(pr.name),
                pr.auto_var.map(|a| p.str(a))
            );
        }
        for st in &obj.states {
            let _ = writeln!(o, "  state {:?}", p.str(st.name));
            for (fname, f) in &st.functions {
                let params: Vec<String> = f
                    .params
                    .iter()
                    .map(|(n, t)| format!("{} {}", p.str(*t), p.str(*n)))
                    .collect();
                let _ = writeln!(
                    o,
                    "    function {} {}({}) flags={}",
                    p.str(f.return_type),
                    p.str(*fname),
                    params.join(", "),
                    f.flags
                );
                for (i, ins) in f.code.iter().enumerate() {
                    let args: Vec<String> = ins.args.iter().map(data).collect();
                    let _ = writeln!(
                        o,
                        "      {i:4} {} {}",
                        OP_NAMES[ins.op as usize],
                        args.join(", ")
                    );
                }
            }
        }
    }
    o
}
