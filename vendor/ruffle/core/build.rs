use std::path::{Path, PathBuf};

fn main() {
    let repo_root = Path::new("../");
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());

    build_playerglobal::build_avm1_playerglobal(repo_root, &out_dir)
        .expect("Failed to build playerglobal_avm1");
    // The AVM2 playerglobal is compiled with asc.jar, which needs Java. A copy
    // of it in core/prebuilt/playerglobal is used while it's up to date with
    // its sources (see build_playerglobal's prebuilt module), unless
    // RUFFLE_PLAYERGLOBAL_FROM_SOURCE is set or the stubs report (known_stubs)
    // is wanted, which only a build from source makes.
    let from_source = std::env::var_os("RUFFLE_PLAYERGLOBAL_FROM_SOURCE").is_some()
        || cfg!(feature = "known_stubs");
    let prebuilt = !from_source
        && build_playerglobal::copy_prebuilt_avm2_playerglobal(repo_root, &out_dir)
            .expect("Failed to copy the prebuilt playerglobal_avm2");
    if !prebuilt {
        if !from_source {
            println!(
                "cargo:warning=core/prebuilt/playerglobal is out of date with its sources; \
                 building playerglobal_avm2 with Java (regenerate.sh there refreshes it)"
            );
        }
        build_playerglobal::build_avm2_playerglobal(
            repo_root,
            &out_dir,
            cfg!(feature = "known_stubs"),
        )
        .unwrap_or_else(|e| {
            panic!(
                "Failed to build playerglobal_avm2: {e}\n\
                 The prebuilt copy in core/prebuilt/playerglobal doesn't match its sources \
                 (or RUFFLE_PLAYERGLOBAL_FROM_SOURCE is set), so it has to be built with Java. \
                 With Java installed, core/prebuilt/playerglobal/regenerate.sh refreshes the copy."
            )
        });
    }

    println!(
        "cargo:rustc-env=RUFFLE_PLAYERGLOBAL_ABC_PATH={}/playerglobal_import.abc",
        out_dir.to_str().unwrap()
    );

    // This is overly conservative - it will cause us to rebuild playerglobals
    // if *any* files in these directories change, not just .as files.
    // However, this script is fast to run, so it shouldn't matter in practice.
    // If Cargo ever adds glob support to 'rerun-if-changed', we should use it.
    println!("cargo:rerun-if-changed=src/avm1/globals/");
    println!("cargo:rerun-if-changed=src/avm2/globals/");
    println!("cargo:rerun-if-changed=prebuilt/playerglobal/");
    println!("cargo:rerun-if-env-changed=RUFFLE_PLAYERGLOBAL_FROM_SOURCE");
}
