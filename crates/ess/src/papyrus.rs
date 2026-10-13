//! The save's Papyrus section (global data 1001), as far as the scripts'
//! variables: the string table, the script types with their variables'
//! names, each script instance with its variables' values, and the arrays.
//! Running stacks and the rest are left alone.

use std::collections::HashMap;

use crate::{Cursor, Error, RefId, Result};

/// A script type: its name, parent and own variables (name, type).
#[derive(Debug, Clone)]
pub struct Script {
    pub name: String,
    pub parent: String,
    pub members: Vec<(String, String)>,
}

/// A script attached to a form, with its variables' values: the parents'
/// first, then its own, as [`Papyrus::members`] names them. Auto
/// properties' values are in their hidden `::name_var` variables.
#[derive(Debug, Clone)]
pub struct Instance {
    pub id: u64,
    pub script: String,
    pub form: RefId,
    pub variables: Vec<Variable>,
}

/// A variable's value.
#[derive(Debug, Clone, PartialEq)]
pub enum Variable {
    Null,
    /// An object: its class and handle (for a form, the form id is the low
    /// 32 bits; 0 handle or the null handle when None).
    Object(String, u64),
    String(String),
    Int(i32),
    Float(f32),
    Bool(bool),
    /// An array: its element type (`Object` arrays name the class) and id
    /// (see [`Papyrus::arrays`]).
    Array(String, u64),
}

/// An array's elements.
#[derive(Debug, Clone)]
pub struct Array {
    pub id: u64,
    pub elements: Vec<Variable>,
}

#[derive(Debug, Clone)]
pub struct Papyrus {
    pub version: u16,
    pub scripts: Vec<Script>,
    pub instances: Vec<Instance>,
    pub arrays: Vec<Array>,
}

/// The string table and what it takes to read variables.
struct Reader<'a> {
    c: Cursor<'a>,
    strings: Vec<String>,
}

impl Reader<'_> {
    fn string(&mut self) -> Result<String> {
        let i = self.c.u32()? as usize;
        self.strings
            .get(i)
            .cloned()
            .ok_or_else(|| Error::Corrupt(format!("string {i} of {}", self.strings.len())))
    }

    fn variable(&mut self) -> Result<Variable> {
        Ok(match self.c.u8()? {
            0 => {
                self.c.u32()?;
                Variable::Null
            }
            1 => {
                let class = self.string()?;
                Variable::Object(class, self.c.u64()?)
            }
            2 => Variable::String(self.string()?),
            3 => Variable::Int(self.c.u32()? as i32),
            4 => Variable::Float(self.c.f32()?),
            5 => Variable::Bool(self.c.u32()? != 0),
            11 => {
                let class = self.string()?;
                Variable::Array(class, self.c.u64()?)
            }
            t @ 12..=15 => {
                let ty = ["String", "Int", "Float", "Bool"][t as usize - 12];
                Variable::Array(ty.into(), self.c.u64()?)
            }
            t => {
                return Err(Error::Corrupt(format!(
                    "variable type {t} at {}",
                    self.c.p - 1
                )));
            }
        })
    }

    /// An instance's or reference's data: id, its variables.
    fn object_data(&mut self) -> Result<(u64, Vec<Variable>)> {
        let id = self.c.u64()?;
        let flag = self.c.u8()?;
        let _type = self.string()?;
        let _unknown = self.c.u32()?;
        if flag & 4 != 0 {
            self.c.u32()?;
        }
        let count = self.c.u32()?;
        let variables = (0..count)
            .map(|_| self.variable())
            .collect::<Result<Vec<_>>>()?;
        Ok((id, variables))
    }
}

impl Papyrus {
    pub fn parse(d: &[u8]) -> Result<Papyrus> {
        let mut c = Cursor::new(d);
        let version = c.u16()?;
        let strings = (0..c.u32()?)
            .map(|_| c.wstring())
            .collect::<Result<Vec<_>>>()?;
        let mut r = Reader { c, strings };

        let mut scripts = Vec::new();
        for _ in 0..r.c.u32()? {
            let name = r.string()?;
            let parent = r.string()?;
            let members = (0..r.c.u32()?)
                .map(|_| Ok((r.string()?, r.string()?)))
                .collect::<Result<Vec<_>>>()?;
            scripts.push(Script {
                name,
                parent,
                members,
            });
        }

        let mut instances = Vec::new();
        for _ in 0..r.c.u32()? {
            let id = r.c.u64()?;
            let script = r.string()?;
            let _unknown_bits = r.c.u16()?;
            let _unknown = r.c.u16()?;
            let form = r.c.ref_id()?;
            let _unknown = r.c.u8()?;
            instances.push(Instance {
                id,
                script,
                form,
                variables: Vec::new(),
            });
        }
        // References (structs the VM made): id, type.
        let references = r.c.u32()?;
        for _ in 0..references {
            r.c.u64()?;
            r.string()?;
        }
        // Arrays: id, element type, length.
        let mut array_info = Vec::new();
        for _ in 0..r.c.u32()? {
            let id = r.c.u64()?;
            let ty = r.c.u8()?;
            if ty == 1 {
                r.string()?;
            }
            array_info.push((id, r.c.u32()?));
        }
        let _runtime = r.c.u32()?;
        // Running scripts (stacks): id, type. Their ids are 32 bits.
        for _ in 0..r.c.u32()? {
            r.c.u32()?;
            r.c.u8()?;
        }

        for i in &mut instances {
            let (id, variables) = r.object_data()?;
            if id != i.id {
                return Err(Error::Corrupt(format!(
                    "instance {:#x}'s data is {id:#x}'s",
                    i.id
                )));
            }
            i.variables = variables;
        }
        for _ in 0..references {
            r.object_data()?;
        }
        let mut arrays = Vec::new();
        for (id, len) in array_info {
            if r.c.u64()? != id {
                return Err(Error::Corrupt(format!("array {id:#x}'s data")));
            }
            let elements = (0..len).map(|_| r.variable()).collect::<Result<Vec<_>>>()?;
            arrays.push(Array { id, elements });
        }

        Ok(Papyrus {
            version,
            scripts,
            instances,
            arrays,
        })
    }

    /// A script type by name (case-insensitive, as Papyrus names are).
    pub fn script(&self, name: &str) -> Option<&Script> {
        self.scripts
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
    }

    /// A script's variables (name, type) in the order its instances hold
    /// them: its parents' first. Parents the save has no type for (native
    /// classes such as `Quest`) have none.
    pub fn members(&self, script: &str) -> Vec<(String, String)> {
        let mut chain = Vec::new();
        let mut s = self.script(script);
        while let Some(t) = s {
            if chain.len() > 64 {
                break;
            }
            chain.push(t);
            s = (!t.parent.is_empty())
                .then(|| self.script(&t.parent))
                .flatten();
        }
        chain
            .iter()
            .rev()
            .flat_map(|t| t.members.iter().cloned())
            .collect()
    }

    /// The instances of a script.
    pub fn instances_of<'a>(&'a self, script: &'a str) -> impl Iterator<Item = &'a Instance> {
        self.instances
            .iter()
            .filter(move |i| i.script.eq_ignore_ascii_case(script))
    }

    /// An instance's variables by name (lower case), with their values.
    pub fn variables(&self, instance: &Instance) -> HashMap<String, Variable> {
        self.members(&instance.script)
            .into_iter()
            .zip(instance.variables.iter().cloned())
            .map(|((name, _), v)| (name.to_ascii_lowercase(), v))
            .collect()
    }

    /// An array by id.
    pub fn array(&self, id: u64) -> Option<&Array> {
        self.arrays.iter().find(|a| a.id == id)
    }
}
