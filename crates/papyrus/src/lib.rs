//! Papyrus scripting: PEX loading and a virtual machine.

pub mod pex;
pub mod value;
pub mod vm;

pub use value::{Array, ObjectId, Value};
pub use vm::{Host, NativeResult, Vm};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("corrupt script: {0}")]
    Corrupt(String),
    #[error("script error: {0}")]
    Runtime(String),
}

pub type Result<T> = std::result::Result<T, Error>;
