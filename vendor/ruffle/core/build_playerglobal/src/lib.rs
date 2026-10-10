//! An internal Ruffle utility to build our AVM1 and AVM2 playerglobals

mod avm1;
mod avm2;
mod prebuilt;

pub use avm1::build_avm1_playerglobal;
pub use avm2::build_avm2_playerglobal;
pub use prebuilt::{
    AVM2_PLAYERGLOBAL_FILES, avm2_inputs_hash, copy_prebuilt_avm2_playerglobal,
    prebuilt_avm2_playerglobal_dir, write_prebuilt_avm2_playerglobal,
};
