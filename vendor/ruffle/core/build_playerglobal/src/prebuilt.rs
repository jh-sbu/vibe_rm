//! A prebuilt AVM2 playerglobal, so that building `core` doesn't need Java.
//!
//! `build_avm2_playerglobal` compiles `core/src/avm2/globals/` with `asc.jar`,
//! which needs Java. Its output (`playerglobal_avm2.swf`, `native_table.rs`,
//! `playerglobal_import.abc`) only depends on those sources, `asc.jar` and the
//! code below that writes it, and is the same from build to build. A copy of
//! it lives in `core/prebuilt/playerglobal/`, with a hash of those inputs in
//! `inputs.fnv`; `core/build.rs` uses the copy while the hash still matches.
//! `core/prebuilt/playerglobal/regenerate.sh` (with Java) refreshes it.

use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// The files `build_avm2_playerglobal` writes, which the copy holds.
pub const AVM2_PLAYERGLOBAL_FILES: [&str; 3] = [
    "playerglobal_avm2.swf",
    "native_table.rs",
    "playerglobal_import.abc",
];

/// The hash file in the prebuilt directory.
const HASH_FILE: &str = "inputs.fnv";

/// Where the copy lives, from the repository root.
pub fn prebuilt_avm2_playerglobal_dir(repo_root: &Path) -> PathBuf {
    repo_root.join("core/prebuilt/playerglobal")
}

/// The files the AVM2 playerglobal is built from, from the repository root:
/// the ActionScript sources, `asc.jar`, the code that runs it, and the code
/// that turns its output into the files.
fn avm2_inputs(repo_root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut inputs = Vec::new();
    for entry in WalkDir::new(repo_root.join("core/src/avm2/globals")) {
        let entry = entry?;
        if entry.file_type().is_file() && entry.path().extension().is_some_and(|e| e == "as") {
            inputs.push(entry.path().strip_prefix(repo_root).unwrap().to_path_buf());
        }
    }
    inputs.extend(
        [
            "core/build_playerglobal/src/avm2.rs",
            "tools/asc/asc.jar",
            "tools/asc/src/lib.rs",
            "swf/src/write.rs",
            "swf/src/avm2/read.rs",
            "swf/src/avm2/types.rs",
            "swf/src/avm2/write.rs",
        ]
        .map(PathBuf::from),
    );
    inputs.sort();
    Ok(inputs)
}

/// A hash (FNV-1a, 64 bits, as hex) of the inputs' paths and contents.
pub fn avm2_inputs_hash(repo_root: &Path) -> std::io::Result<String> {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for &b in bytes {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for input in avm2_inputs(repo_root)? {
        // Forward slashes, so the hash is the same on every platform.
        let name = input.to_string_lossy().replace('\\', "/");
        let contents = fs::read(repo_root.join(&input))?;
        feed(name.as_bytes());
        feed(&[0]);
        feed(&(contents.len() as u64).to_le_bytes());
        feed(&contents);
    }
    Ok(format!("{hash:016x}"))
}

/// Copies the prebuilt AVM2 playerglobal into `out_dir` when it's up to date
/// with its inputs. False, copying nothing, when it isn't (or isn't there).
pub fn copy_prebuilt_avm2_playerglobal(
    repo_root: &Path,
    out_dir: &Path,
) -> Result<bool, Box<dyn std::error::Error>> {
    let dir = prebuilt_avm2_playerglobal_dir(repo_root);
    let Ok(recorded) = fs::read_to_string(dir.join(HASH_FILE)) else {
        return Ok(false);
    };
    if recorded.trim() != avm2_inputs_hash(repo_root)? {
        return Ok(false);
    }
    for file in AVM2_PLAYERGLOBAL_FILES {
        fs::copy(dir.join(file), out_dir.join(file))?;
    }
    Ok(true)
}

/// Builds the AVM2 playerglobal (with Java) into the prebuilt directory and
/// records its inputs' hash.
pub fn write_prebuilt_avm2_playerglobal(
    repo_root: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = prebuilt_avm2_playerglobal_dir(repo_root);
    fs::create_dir_all(&dir)?;
    crate::build_avm2_playerglobal(repo_root, &dir, false)?;
    fs::write(dir.join(HASH_FILE), avm2_inputs_hash(repo_root)? + "\n")?;
    Ok(())
}
