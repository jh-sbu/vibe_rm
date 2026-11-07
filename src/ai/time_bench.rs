//! Menu-time catch-up cost against the installed vanilla masters.
use crate::engine::Engine;
use esp::FormId;

fn vanilla() -> Engine {
    let data = vfs::locate_data_dir().expect("Skyrim SE Data");
    let names: Vec<_> = esp::BASE_MASTERS.iter().map(|s| s.to_string()).collect();
    let mut e = crate::stats::action_tests::headless_engine_plugins(&data, &names);
    e.hour = 7.0;
    e.refresh_whereabouts();
    e
}

/// Every actor's place and package, so runs can be compared.
fn outcome(e: &Engine) -> Vec<(FormId, String, Option<FormId>, bool)> {
    let mut v: Vec<_> = e
        .away
        .iter()
        .map(|(&r, x)| (r, format!("{:?}", x.place), x.package(), x.settled))
        .collect();
    v.sort_by_key(|o| o.0);
    v
}

/// Optional benchmark against externally installed assets; never copies them.
/// `VRM_MENU_ACTORS` (default 48, 0 for all) actors run away, then an hour
/// passes in a menu (`VRM_MENU_SECONDS`). `VRM_MENU_OUT` writes the outcome.
#[test]
#[ignore = "needs Skyrim SE Data and a headless GPU adapter"]
fn menu_time_benchmark() {
    let mut e = vanilla();
    let count: usize = std::env::var("VRM_MENU_ACTORS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(48);
    let seconds: f32 = std::env::var("VRM_MENU_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3600.0);
    let mut actors: Vec<_> = e.whereabouts.of.keys().copied().collect();
    actors.sort();
    if count > 0 {
        let step = (actors.len() / count).max(1);
        actors = actors.into_iter().step_by(step).take(count).collect();
    }
    let ran = actors.iter().filter(|&&r| e.run_away(r)).count();
    e.advance_menu_time(0.0);
    e.hour += seconds / 3600.0;
    let start = std::time::Instant::now();
    e.advance_menu_time(seconds);
    eprintln!(
        "menu time: {seconds} s with {ran} away actors took {:.3} s",
        start.elapsed().as_secs_f64()
    );
    let out = outcome(&e);
    let settled = out.iter().filter(|o| o.3).count();
    eprintln!("{} away, {settled} settled", out.len());
    if let Ok(path) = std::env::var("VRM_MENU_OUT") {
        let text: String = out.iter().map(|o| format!("{o:?}\n")).collect();
        std::fs::write(path, text).unwrap();
    }
}
