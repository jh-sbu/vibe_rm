//! Costs of the engine's whole-load-order indexes, against the installed
//! vanilla masters. Optional benchmarks: they need Skyrim SE Data and a
//! headless GPU adapter, and never copy the game's files.
use crate::engine::Engine;

fn vanilla() -> Engine {
    let data = vfs::locate_data_dir().expect("Skyrim SE Data");
    let names: Vec<_> = esp::BASE_MASTERS.iter().map(|s| s.to_string()).collect();
    crate::stats::action_tests::headless_engine_plugins(&data, &names)
}

fn time<T>(what: &str, f: impl FnOnce() -> T) -> T {
    let start = std::time::Instant::now();
    let out = f();
    eprintln!("{what}: {:.1} ms", start.elapsed().as_secs_f64() * 1000.0);
    out
}

/// The enable-parent index the first `Disable` of a session reads.
#[test]
#[ignore = "needs Skyrim SE Data and a headless GPU adapter"]
fn enable_children_benchmark() {
    let e = vanilla();
    let n = time("enable children", || e.enable_children().len());
    eprintln!("{n} enable parents");
}
