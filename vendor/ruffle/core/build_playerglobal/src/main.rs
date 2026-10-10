//! Manually builds `playerglobal_avm1.swf` and `playerglobal_avm2.swf` without building the `core` crate.
//! This binary is invoked as:
//! `cargo run --package=build_playerglobal <repo_root> <out_dir>`
//! where `<repo_root>` is the location of the Ruffle repository,
//! and `out_dir` is the directory where the two SWFs should
//! be written

mod cli;

use clap::Parser;

use cli::Commands;
use std::path::{Path, PathBuf};

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = root.join("../../").canonicalize().unwrap();

    let args = cli::Cli::parse();
    match args.command {
        Commands::Compile { out_dir } => {
            build_playerglobal::build_avm2_playerglobal(&repo_root, Path::new(&out_dir), false)
                .unwrap();
        }
        Commands::Prebuild => {
            build_playerglobal::write_prebuilt_avm2_playerglobal(&repo_root).unwrap();
            println!(
                "wrote {} ({})",
                build_playerglobal::prebuilt_avm2_playerglobal_dir(&repo_root).display(),
                build_playerglobal::avm2_inputs_hash(&repo_root).unwrap()
            );
        }
    }
}
