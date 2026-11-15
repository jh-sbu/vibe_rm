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

/// The location index (editor locations and ref types), read on first use.
#[test]
#[ignore = "needs Skyrim SE Data and a headless GPU adapter"]
fn location_index_benchmark() {
    let e = vanilla();
    let n = time("location index", || e.location_ref_types().len());
    eprintln!("{n} references with ref types");
}

/// Every vanilla `Near` alias filled about one centre in the loaded area
/// round Riverwood: the centre is the loaded reference most linked to.
#[test]
#[ignore = "needs Skyrim SE Data and a headless GPU adapter"]
fn near_fill_benchmark() {
    let mut e = vanilla();
    let tamriel = esp::FormId(0x3C);
    e.enter_exterior(tamriel, glam::Vec3::new(21500.0, -46000.0, 0.0), 0.0)
        .unwrap();
    let loaded = e.loaded_refs();
    let mut links: std::collections::HashMap<esp::FormId, usize> = Default::default();
    for &r in &loaded {
        if let Some(t) = e.linked_ref(r, None) {
            *links.entry(t).or_default() += 1;
        }
    }
    let (&centre, count) = links.iter().max_by_key(|x| (x.1, x.0)).unwrap();
    eprintln!(
        "{} loaded references, {} cells; centre {centre} linked from {count}, {} near it",
        loaded.len(),
        e.cells.len(),
        e.refs_near(centre).len()
    );
    let mut near = Vec::new();
    for &q in e.lo.ids_of_type(b"QUST") {
        for a in e.alias_specs(q).iter() {
            if let crate::aliases::Fill::Near(c) = a.fill {
                near.push((q, c, a.clone()));
            }
        }
    }
    let reserved = Default::default();
    time("location index", || e.ref_types(centre).len());
    time("reference links", || {
        e.ref_links().linked_from(centre).len()
    });
    let start = std::time::Instant::now();
    let mut fills = Vec::new();
    for (q, c, a) in &near {
        e.scripts
            .quests
            .entry(*q)
            .or_default()
            .aliases
            .insert(*c, centre);
        fills.push(e.fill_alias(*q, a, &reserved));
        e.scripts.quests.remove(q);
    }
    let filled = fills.iter().flatten().count();
    eprintln!(
        "{} near fills ({filled} filled): {:.2} ms each",
        near.len(),
        start.elapsed().as_secs_f64() * 1000.0 / near.len() as f64
    );
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    fills.hash(&mut h);
    eprintln!("fills digest {:016x}", h.finish());
}
