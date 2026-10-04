//! Compiled Papyrus script (`.pex`) parsing. Skyrim PEX files are big-endian.

use crate::{Error, Result};

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

#[derive(Debug, Clone)]
pub struct Pex {
    pub source: String,
    pub strings: Vec<String>,
    pub objects: Vec<Object>,
}

impl Pex {
    pub fn str(&self, i: u16) -> &str {
        self.strings.get(i as usize).map(String::as_str).unwrap_or("")
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
    "nop", "iadd", "fadd", "isub", "fsub", "imul", "fmul", "idiv", "fdiv", "imod", "not", "ineg", "fneg", "assign", "cast",
    "cmp_eq", "cmp_lt", "cmp_le", "cmp_gt", "cmp_ge", "jmp", "jmpt", "jmpf", "callmethod", "callparent", "callstatic",
    "return", "strcat", "propget", "propset", "array_create", "array_length", "array_getelement", "array_setelement",
    "array_findelement", "array_rfindelement",
];

struct R<'a> {
    d: &'a [u8],
    p: usize,
}

impl R<'_> {
    fn need(&self, n: usize) -> Result<()> {
        if self.p + n > self.d.len() { Err(Error::Corrupt(format!("unexpected end at {}", self.p))) } else { Ok(()) }
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
        Ok(u32::from_be_bytes(self.d[self.p - 4..self.p].try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64> {
        self.need(8)?;
        self.p += 8;
        Ok(u64::from_be_bytes(self.d[self.p - 8..self.p].try_into().unwrap()))
    }
    fn wstring(&mut self) -> Result<String> {
        let n = self.u16()? as usize;
        self.need(n)?;
        let s = self.d[self.p..self.p + n].iter().map(|&c| c as char).collect();
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
        Ok(Function { return_type, doc, user_flags, flags, params, locals, code })
    }
}

pub fn parse(d: &[u8]) -> Result<Pex> {
    let mut r = R { d, p: 0 };
    if r.u32()? != 0xFA57_C0DE {
        return Err(Error::Corrupt("bad magic".into()));
    }
    let _major = r.u8()?;
    let _minor = r.u8()?;
    let _game = r.u16()?;
    let _time = r.u64()?;
    let source = r.wstring()?;
    let _user = r.wstring()?;
    let _machine = r.wstring()?;
    let n = r.u16()?;
    let mut strings = Vec::with_capacity(n as usize);
    for _ in 0..n {
        strings.push(r.wstring()?);
    }
    if r.u8()? != 0 {
        r.u64()?;
        let nf = r.u16()?;
        for _ in 0..nf {
            r.u16()?;
            r.u16()?;
            r.u16()?;
            r.u8()?;
            let ic = r.u16()? as usize;
            r.need(ic * 2)?;
            r.p += ic * 2;
        }
    }
    let nu = r.u16()?;
    for _ in 0..nu {
        r.u16()?;
        r.u8()?;
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
            variables.push(Variable { name: r.u16()?, type_name: r.u16()?, user_flags: r.u32()?, init: r.data()? });
        }
        let np = r.u16()?;
        let mut properties = Vec::with_capacity(np as usize);
        for _ in 0..np {
            let name = r.u16()?;
            let type_name = r.u16()?;
            let doc = r.u16()?;
            let user_flags = r.u32()?;
            let flags = r.u8()?;
            let mut p = Property { name, type_name, doc, user_flags, flags, auto_var: None, read: None, write: None };
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
            states.push(State { name: sname, functions });
        }
        objects.push(Object { name, parent, doc, user_flags, auto_state, variables, properties, states });
    }
    Ok(Pex { source, strings, objects })
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
        let _ = writeln!(o, "scriptname {} extends {}", p.str(obj.name), p.str(obj.parent));
        for v in &obj.variables {
            let _ = writeln!(o, "  var {} {} = {}", p.str(v.type_name), p.str(v.name), data(&v.init));
        }
        for pr in &obj.properties {
            let _ = writeln!(o, "  property {} {} auto={:?}", p.str(pr.type_name), p.str(pr.name), pr.auto_var.map(|a| p.str(a)));
        }
        for st in &obj.states {
            let _ = writeln!(o, "  state {:?}", p.str(st.name));
            for (fname, f) in &st.functions {
                let params: Vec<String> = f.params.iter().map(|(n, t)| format!("{} {}", p.str(*t), p.str(*n))).collect();
                let _ = writeln!(o, "    function {} {}({}) flags={}", p.str(f.return_type), p.str(*fname), params.join(", "), f.flags);
                for (i, ins) in f.code.iter().enumerate() {
                    let args: Vec<String> = ins.args.iter().map(data).collect();
                    let _ = writeln!(o, "      {i:4} {} {}", OP_NAMES[ins.op as usize], args.join(", "));
                }
            }
        }
    }
    o
}
