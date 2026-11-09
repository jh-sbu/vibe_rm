//! Line-of-sight and tracked-stat registrations on generated records only.
//! Engine owns a renderer, so these need a headless GPU adapter.

use std::sync::Arc;

use esp::FormId;
use esp::writer::{Node, RecordBuilder, write_plugin};
use papyrus::{ObjectId, Value};

use super::LosMode;
use crate::ai::ActorRuntime;
use crate::engine::Engine;

const NPC: FormId = FormId(0x800);
const WATCHER: FormId = FormId(0x801);
const WATCHED: FormId = FormId(0x802);

struct Fixture(std::path::PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture(name: &str) -> (Fixture, Engine) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let f = Fixture(std::env::temp_dir().join(format!(
        "vrm-registrations-{name}-{}-{stamp}",
        std::process::id()
    )));
    std::fs::create_dir(&f.0).unwrap();
    let records = [
        Node::top(
            b"NPC_",
            vec![RecordBuilder::new(b"NPC_", NPC).field(b"ACBS", &[0u8; 24])],
        ),
        Node::top(
            b"ACHR",
            vec![
                RecordBuilder::new(b"ACHR", WATCHER).form(b"NAME", NPC),
                RecordBuilder::new(b"ACHR", WATCHED).form(b"NAME", NPC),
            ],
        ),
    ];
    std::fs::write(
        f.0.join("Registrations.esp"),
        write_plugin(0, &[], "test", "synthetic registrations", 0x900, &records),
    )
    .unwrap();
    let mut e = crate::stats::action_tests::headless_engine(&f.0, "Registrations.esp");
    let key = crate::render::CellKey::Interior(FormId(0x820));
    for r in [WATCHER, WATCHED] {
        let a = ActorRuntime::new(
            r,
            NPC,
            Arc::new(crate::world::skeleton::Skeleton::default()),
            glam::Mat4::IDENTITY,
            Vec::new(),
            0.0,
        );
        e.actor_cells.insert(r, key);
        e.cells.entry(key).or_default().actors.push(a);
    }
    // The watcher faces north; the watched stands 200 ahead.
    e.actor_mut(WATCHER).unwrap().exec.heading = 0.0;
    e.actor_mut(WATCHED).unwrap().exec.pos = glam::Vec3::new(0.0, 200.0, 0.0);
    (f, e)
}

fn events(e: &mut Engine, name: &str) -> Vec<(ObjectId, Vec<Value>)> {
    let out = e
        .scripts
        .pending_events
        .iter()
        .filter(|(_, ev, _)| ev == name)
        .map(|(o, _, args)| (*o, args.clone()))
        .collect();
    e.scripts.pending_events.clear();
    out
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn los_registrations_send_gains_and_losses_to_who_registered() {
    let (_f, mut e) = fixture("los");
    let listener = ObjectId::Form(WATCHED.0);
    e.register_for_los(listener, WATCHER, WATCHED, LosMode::Both);
    e.register_for_los(ObjectId::Form(WATCHER.0), WATCHER, WATCHED, LosMode::Lost);
    e.update_los();
    // At once, the gain to the repeating registration only.
    let gained = events(&mut e, "OnGainLOS");
    assert_eq!(gained.len(), 1);
    assert_eq!(gained[0].0, listener);
    assert_eq!(
        gained[0].1,
        vec![e.object_value(WATCHER), e.object_value(WATCHED)]
    );
    e.update_los();
    assert!(events(&mut e, "OnGainLOS").is_empty());
    // Behind the watcher now: both hear the loss, and the single one ends.
    e.actor_mut(WATCHED).unwrap().exec.pos = glam::Vec3::new(0.0, -200.0, 0.0);
    e.update_los();
    assert_eq!(events(&mut e, "OnLostLOS").len(), 2);
    assert_eq!(e.scripts.los.len(), 1);
    e.unregister_for_los(listener, WATCHER, WATCHED);
    assert!(e.scripts.los.is_empty());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn tracked_stats_reach_listeners_for_the_listed_stats_only() {
    let (_f, mut e) = fixture("stats");
    let listener = ObjectId::Form(WATCHER.0);
    e.register_for_tracked_stats(listener, true);
    e.increment_stat("Locks Picked", 2);
    e.increment_stat("Hours Slept", 1);
    let sent = events(&mut e, "OnTrackedStatsEvent");
    assert_eq!(
        sent,
        vec![(listener, vec![Value::str("Locks Picked"), Value::Int(2)])]
    );
    e.raise_stat_to("Most Gold Carried", 50);
    e.raise_stat_to("Most Gold Carried", 40);
    assert_eq!(events(&mut e, "OnTrackedStatsEvent").len(), 1);
    e.register_for_tracked_stats(listener, false);
    e.increment_stat("Locks Picked", 1);
    assert!(events(&mut e, "OnTrackedStatsEvent").is_empty());
}
