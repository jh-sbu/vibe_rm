//! Papyrus scripting: PEX loading and writing, and a virtual machine.

pub mod build;
pub mod pex;
pub mod value;
pub mod vm;

pub use value::{Array, ObjectId, Value};
pub use vm::{Arg, Host, NativeResult, SavedFrame, SavedThread, SavedVm, Vm};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("corrupt script: {0}")]
    Corrupt(String),
    #[error("script error: {0}")]
    Runtime(String),
    #[error("can't write script: {0}")]
    Write(String),
}

pub type Result<T> = std::result::Result<T, Error>;
