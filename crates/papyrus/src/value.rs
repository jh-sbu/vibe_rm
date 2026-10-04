use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

/// Identity of a scriptable object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ObjectId {
    /// A form or reference, by global FormID.
    Form(u32),
    /// A quest alias (ReferenceAlias / LocationAlias).
    Alias { quest: u32, alias: u32 },
    /// An active magic effect instance.
    Effect(u64),
}

pub type Array = Rc<RefCell<Vec<Value>>>;

#[derive(Clone, Debug, Default)]
pub enum Value {
    #[default]
    None,
    Bool(bool),
    Int(i32),
    Float(f32),
    String(Arc<str>),
    /// Object handle with the script class it is viewed as.
    Object(ObjectId, Arc<str>),
    Array(Array),
}

impl PartialEq for Value {
    fn eq(&self, o: &Value) -> bool {
        use Value::*;
        match (self, o) {
            (None, None) => true,
            (Bool(a), Bool(b)) => a == b,
            (Int(a), Int(b)) => a == b,
            (Float(a), Float(b)) => a == b,
            (Int(a), Float(b)) | (Float(b), Int(a)) => *a as f32 == *b,
            (String(a), String(b)) => a.eq_ignore_ascii_case(b),
            (Object(a, _), Object(b, _)) => a == b,
            (Array(a), Array(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl Value {
    pub fn str(s: &str) -> Value {
        Value::String(Arc::from(s))
    }
    pub fn as_bool(&self) -> bool {
        match self {
            Value::None => false,
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            Value::Float(f) => *f != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::Object(..) => true,
            Value::Array(a) => !a.borrow().is_empty(),
        }
    }
    pub fn as_int(&self) -> i32 {
        match self {
            Value::Bool(b) => *b as i32,
            Value::Int(i) => *i,
            Value::Float(f) => *f as i32,
            Value::String(s) => s.trim().parse::<i32>().or_else(|_| s.trim().parse::<f32>().map(|f| f as i32)).unwrap_or(0),
            _ => 0,
        }
    }
    pub fn as_float(&self) -> f32 {
        match self {
            Value::Bool(b) => *b as i32 as f32,
            Value::Int(i) => *i as f32,
            Value::Float(f) => *f,
            Value::String(s) => s.trim().parse().unwrap_or(0.0),
            _ => 0.0,
        }
    }
    pub fn as_object(&self) -> Option<ObjectId> {
        match self {
            Value::Object(o, _) => Some(*o),
            _ => None,
        }
    }
    pub fn as_form(&self) -> Option<u32> {
        match self {
            Value::Object(ObjectId::Form(f), _) => Some(*f),
            _ => None,
        }
    }
    pub fn is_none(&self) -> bool {
        matches!(self, Value::None)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::None => write!(f, "None"),
            Value::Bool(b) => write!(f, "{}", if *b { "True" } else { "False" }),
            Value::Int(i) => write!(f, "{i}"),
            Value::Float(x) => write!(f, "{x:.6}"),
            Value::String(s) => write!(f, "{s}"),
            Value::Object(ObjectId::Form(id), c) => write!(f, "[{c} <{id:08X}>]"),
            Value::Object(o, c) => write!(f, "[{c} {o:?}]"),
            Value::Array(a) => {
                write!(f, "[")?;
                for (i, v) in a.borrow().iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
        }
    }
}
