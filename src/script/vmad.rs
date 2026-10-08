//! VMAD (virtual machine adapter) subrecords: scripts attached to forms.

use esp::{FormId, LoadedRecord};
use papyrus::{ObjectId, Value};

#[derive(Debug, Clone)]
pub enum PropValue {
    Object { form: FormId, alias: i16 },
    String(String),
    Int(i32),
    Float(f32),
    Bool(bool),
    Array(Vec<PropValue>),
}

#[derive(Debug, Clone)]
pub struct ScriptRef {
    pub name: String,
    pub properties: Vec<(String, PropValue)>,
}

#[derive(Debug, Clone)]
pub struct QuestFragment {
    pub stage: u16,
    pub log_entry: i32,
    pub script: String,
    pub function: String,
}

#[derive(Debug, Clone, Default)]
pub struct Vmad {
    pub scripts: Vec<ScriptRef>,
    /// QUST only: stage fragments and the script holding them.
    pub fragment_script: Option<String>,
    pub fragments: Vec<QuestFragment>,
    /// QUST only: scripts attached to aliases (alias id, scripts).
    pub alias_scripts: Vec<(u32, Vec<ScriptRef>)>,
    /// SCEN only: the scene's begin / end fragments and its phases'.
    pub scene: Option<SceneFragments>,
}

#[derive(Debug, Clone, Default)]
pub struct SceneFragments {
    pub script: String,
    pub begin: Option<String>,
    pub end: Option<String>,
    /// (phase, on completion (else on start), function)
    pub phases: Vec<(u8, bool, String)>,
}

struct R<'a> {
    d: &'a [u8],
    p: usize,
    format: i16,
}

impl R<'_> {
    fn ok(&self, n: usize) -> bool {
        self.p + n <= self.d.len()
    }
    fn u8(&mut self) -> Option<u8> {
        self.ok(1).then(|| {
            self.p += 1;
            self.d[self.p - 1]
        })
    }
    fn i16(&mut self) -> Option<i16> {
        self.ok(2).then(|| {
            self.p += 2;
            i16::from_le_bytes([self.d[self.p - 2], self.d[self.p - 1]])
        })
    }
    fn u16(&mut self) -> Option<u16> {
        self.i16().map(|v| v as u16)
    }
    fn u32(&mut self) -> Option<u32> {
        self.ok(4).then(|| {
            self.p += 4;
            u32::from_le_bytes(self.d[self.p - 4..self.p].try_into().unwrap())
        })
    }
    fn wstring(&mut self) -> Option<String> {
        let n = self.u16()? as usize;
        if !self.ok(n) {
            return None;
        }
        let s = esp::decode_zstring(&self.d[self.p..self.p + n]);
        self.p += n;
        Some(s)
    }
    fn object(&mut self) -> Option<(u32, i16)> {
        if self.format == 1 {
            let f = self.u32()?;
            let a = self.i16()?;
            self.u16()?;
            Some((f, a))
        } else {
            self.u16()?;
            let a = self.i16()?;
            let f = self.u32()?;
            Some((f, a))
        }
    }
    fn value(&mut self, ty: u8, rec: &LoadedRecord<'_>) -> Option<PropValue> {
        Some(match ty {
            1 => {
                let (f, a) = self.object()?;
                PropValue::Object {
                    form: rec.fid(FormId(f)),
                    alias: a,
                }
            }
            2 => PropValue::String(self.wstring()?),
            3 => PropValue::Int(self.u32()? as i32),
            4 => PropValue::Float(f32::from_bits(self.u32()?)),
            5 => PropValue::Bool(self.u8()? != 0),
            11..=15 => {
                let n = self.u32()? as usize;
                let mut v = Vec::with_capacity(n.min(4096));
                for _ in 0..n {
                    v.push(self.value(ty - 10, rec)?);
                }
                PropValue::Array(v)
            }
            _ => return None,
        })
    }
    fn scripts(&mut self, version: i16, rec: &LoadedRecord<'_>) -> Option<Vec<ScriptRef>> {
        let n = self.u16()?;
        let mut out = Vec::with_capacity(n as usize);
        for _ in 0..n {
            let name = self.wstring()?;
            if version >= 4 {
                self.u8()?;
            }
            let np = self.u16()?;
            let mut properties = Vec::with_capacity(np as usize);
            for _ in 0..np {
                let pname = self.wstring()?;
                let ty = self.u8()?;
                if version >= 4 {
                    self.u8()?;
                }
                let v = self.value(ty, rec)?;
                properties.push((pname, v));
            }
            out.push(ScriptRef { name, properties });
        }
        Some(out)
    }
}

pub fn parse(rec: &LoadedRecord<'_>) -> Option<Vmad> {
    let d = rec.get(b"VMAD")?;
    let mut r = R { d, p: 0, format: 2 };
    let version = r.i16()?;
    r.format = r.i16()?;
    let mut v = Vmad {
        scripts: r.scripts(version, rec)?,
        ..Default::default()
    };
    if rec.tag().0 == *b"QUST" && r.ok(3) {
        // Fragment data
        let _ver = r.u8()?;
        let n = r.u16()?;
        v.fragment_script = Some(r.wstring()?);
        for _ in 0..n {
            let stage = r.u16()?;
            r.i16()?;
            let log_entry = r.u32()? as i32;
            r.u8()?;
            let script = r.wstring()?;
            let function = r.wstring()?;
            v.fragments.push(QuestFragment {
                stage,
                log_entry,
                script,
                function,
            });
        }
        // Aliases
        if let Some(na) = r.u16() {
            for _ in 0..na {
                let Some((_, alias)) = r.object() else { break };
                let aver = r.i16().unwrap_or(version);
                r.format = r.i16().unwrap_or(r.format);
                let Some(scripts) = r.scripts(aver, rec) else {
                    break;
                };
                v.alias_scripts.push((alias as u32, scripts));
            }
        }
    }
    if rec.tag().0 == *b"SCEN" && r.ok(3) {
        v.scene = scene_fragments(&mut r);
    }
    Some(v)
}

/// Scene fragment data: a version byte, flags (1 begin, 2 end), the script, then
/// each of those fragments (a byte, script, function) and the phase fragments
/// (flags: 1 start, 2 completion; phase; four unknown bytes; script; function).
fn scene_fragments(r: &mut R<'_>) -> Option<SceneFragments> {
    r.u8()?;
    let flags = r.u8()?;
    let mut f = SceneFragments {
        script: r.wstring()?,
        ..Default::default()
    };
    for bit in [1, 2] {
        if flags & bit != 0 {
            r.u8()?;
            r.wstring()?;
            let func = Some(r.wstring()?);
            if bit == 1 {
                f.begin = func;
            } else {
                f.end = func;
            }
        }
    }
    let n = r.u16()?;
    for _ in 0..n {
        let pflags = r.u8()?;
        let phase = r.u8()?;
        r.u32()?;
        r.wstring()?;
        let func = r.wstring()?;
        f.phases.push((phase, pflags & 2 != 0, func));
    }
    Some(f)
}

/// Convert a VMAD property value into a VM value. Object values need the form's script class.
pub fn to_value(v: &PropValue, class_of: &dyn Fn(FormId) -> &'static str) -> Value {
    match v {
        PropValue::Object { form, alias } => {
            if form.is_null() {
                Value::None
            } else if *alias >= 0 {
                Value::Object(
                    ObjectId::Alias {
                        quest: form.0,
                        alias: *alias as u32,
                    },
                    "ReferenceAlias".into(),
                )
            } else {
                Value::Object(ObjectId::Form(form.0), class_of(*form).into())
            }
        }
        PropValue::String(s) => Value::str(s),
        PropValue::Int(i) => Value::Int(*i),
        PropValue::Float(f) => Value::Float(*f),
        PropValue::Bool(b) => Value::Bool(*b),
        PropValue::Array(items) => Value::Array(std::rc::Rc::new(std::cell::RefCell::new(
            items.iter().map(|i| to_value(i, class_of)).collect(),
        ))),
    }
}
