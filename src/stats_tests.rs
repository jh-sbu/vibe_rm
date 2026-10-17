//! Actual gameplay/native/console actions against generated records only.
//! Engine owns a Renderer, so this integration test explicitly needs a GPU
//! adapter (software Vulkan is sufficient). It never loads game data.

use super::*;
use crate::engine::PLAYER_REF;
use crate::magic::{archetype, spell_type};
use crate::spells::{CONCENTRATION, LEFT, RIGHT};
use esp::actor_value as av;
use esp::writer::{Node, RecordBuilder, write_plugin};

const FIRE: FormId = FormId(0x810);
const SPARK: FormId = FormId(0x811);
const HEAL: FormId = FormId(0x812);
const CHANNEL: FormId = FormId(0x813);
const SHOUT: FormId = FormId(0x820);
const WORD: FormId = FormId(0x830);

struct Fixture(std::path::PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture() -> (Fixture, Engine) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "vrm-magic-stat-actions-{}-{stamp}",
        std::process::id()
    )));
    std::fs::create_dir(&fixture.0).unwrap();
    let effect = |id, school: u32| {
        let mut data = [0; 0x6c];
        data[4..8].copy_from_slice(&1.0f32.to_le_bytes()); // base cost
        data[0x0c..0x10].copy_from_slice(&school.to_le_bytes());
        data[0x10..0x14].copy_from_slice(&u32::MAX.to_le_bytes());
        data[0x40..0x44].copy_from_slice(&archetype::VALUE_MODIFIER.to_le_bytes());
        data[0x44..0x48].copy_from_slice(&av::MAGICKA.to_le_bytes());
        RecordBuilder::new(b"MGEF", FormId(id)).field(b"DATA", &data)
    };
    let spell = |id: FormId, tag, name, effect: u32, kind: u32, casting: u32| {
        let mut spit = [0; 36];
        spit[0..4].copy_from_slice(&10u32.to_le_bytes());
        spit[4..8].copy_from_slice(&1u32.to_le_bytes()); // authored cost
        spit[8..12].copy_from_slice(&kind.to_le_bytes());
        spit[12..16].copy_from_slice(&0.5f32.to_le_bytes());
        spit[16..20].copy_from_slice(&casting.to_le_bytes());
        let mut efit = [0; 12];
        efit[0..4].copy_from_slice(&1.0f32.to_le_bytes());
        efit[8..12].copy_from_slice(&1u32.to_le_bytes());
        RecordBuilder::new(tag, id)
            .zstring(b"FULL", name)
            .field(b"SPIT", &spit)
            .u32(b"EFID", effect)
            .field(b"EFIT", &efit)
    };
    let mut snam = Vec::new();
    snam.extend_from_slice(&WORD.0.to_le_bytes());
    // Deliberately normal SPELL, to catch counting the shout's backing spell.
    snam.extend_from_slice(&FIRE.0.to_le_bytes());
    snam.extend_from_slice(&1.0f32.to_le_bytes());
    let records = [
        Node::top(
            b"MGEF",
            vec![
                effect(0x800, 20), // Destruction
                effect(0x801, 22), // Restoration
            ],
        ),
        Node::top(
            b"SPEL",
            vec![
                spell(FIRE, b"SPEL", "Test Fire", 0x800, spell_type::SPELL, 1),
                spell(SPARK, b"SPEL", "Test Spark", 0x800, spell_type::SPELL, 1),
                spell(HEAL, b"SPEL", "Test Heal", 0x801, spell_type::SPELL, 1),
                spell(
                    CHANNEL,
                    b"SPEL",
                    "Test Channel",
                    0x801,
                    spell_type::SPELL,
                    CONCENTRATION,
                ),
                spell(
                    FormId(0x814),
                    b"SPEL",
                    "Test Ability",
                    0x801,
                    spell_type::ABILITY,
                    0,
                ),
                spell(
                    FormId(0x815),
                    b"SPEL",
                    "Test Power",
                    0x801,
                    spell_type::POWER,
                    1,
                ),
            ],
        ),
        Node::top(
            b"SCRL",
            vec![spell(
                FormId(0x816),
                b"SCRL",
                "Test Scroll",
                0x801,
                spell_type::SPELL,
                1,
            )],
        ),
        Node::top(
            b"SHOU",
            vec![
                RecordBuilder::new(b"SHOU", SHOUT)
                    .zstring(b"FULL", "Test Shout")
                    .field(b"SNAM", &snam),
            ],
        ),
        Node::top(
            b"WOOP",
            vec![RecordBuilder::new(b"WOOP", WORD).zstring(b"FULL", "Test Word")],
        ),
        Node::top(
            b"AVIF",
            vec![
                RecordBuilder::new(b"AVIF", FormId(0x446 + 22))
                    .zstring(b"FULL", "Translated Restoration"),
            ],
        ),
        Node::top(b"NPC_", vec![RecordBuilder::new(b"NPC_", FormId(0x898))]),
        Node::top(
            b"ACHR",
            vec![RecordBuilder::new(b"ACHR", FormId(0x899)).u32(b"NAME", 0x898)],
        ),
    ];
    std::fs::write(
        fixture.0.join("Stats.esp"),
        write_plugin(
            0,
            &[],
            "test",
            "synthetic magic statistic actions",
            0x900,
            &records,
        ),
    )
    .unwrap();
    let lo = esp::LoadOrder::load(&fixture.0, &["Stats.esp".into()]).unwrap();
    let vfs = vfs::Vfs::new(&fixture.0, &[]);
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        compatible_surface: None,
        apply_limit_buckets: false,
        ..Default::default()
    }))
    .expect("a headless GPU adapter is required for this ignored test");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::TEXTURE_COMPRESSION_BC,
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .unwrap();
    let renderer =
        crate::render::Renderer::new(device, queue, wgpu::TextureFormat::Rgba8Unorm, 16, 16);
    let mut engine = Engine::new(lo, vfs, renderer, 12.0, None, 0);
    engine.set_actor_value(PLAYER_REF, av::MAGICKA, 1000.0);
    (fixture, engine)
}

fn spell_uses(e: &Engine, spell: FormId) -> u64 {
    e.scripts
        .magic_usage
        .spells
        .counts
        .get(&spell)
        .copied()
        .unwrap_or(0)
}

fn release_spell(e: &mut Engine, spell: FormId, held: f32) {
    e.equip_spell(PLAYER_REF, spell, RIGHT);
    assert!(e.player_cast_press(RIGHT));
    e.update_casting(held);
    assert!(e.player_cast_release(RIGHT));
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn magic_favorites_follow_actual_actions_without_duplicate_landings() {
    let (_fixture, mut e) = fixture();
    assert_eq!(e.stat("Favorite Spell"), StatValue::Name(String::new()));
    release_spell(&mut e, FIRE, 0.1); // too early
    assert_eq!(spell_uses(&e, FIRE), 0);
    release_spell(&mut e, FIRE, 0.5);
    assert_eq!(spell_uses(&e, FIRE), 1);
    assert_eq!(
        e.stat("Favorite Spell"),
        StatValue::Name("Test Fire".into())
    );
    e.set_actor_value(PLAYER_REF, av::MAGICKA, 0.0);
    assert!(e.player_cast_press(RIGHT));
    assert!(!e.player_cast_release(RIGHT)); // no channel started
    assert_eq!(spell_uses(&e, FIRE), 1);
    e.set_actor_value(PLAYER_REF, av::MAGICKA, 1000.0);

    // Holding and repeated effects count once, independently in either hand.
    e.equip_spell(PLAYER_REF, CHANNEL, RIGHT);
    assert!(e.player_cast_press(RIGHT));
    assert_eq!(spell_uses(&e, CHANNEL), 0);
    assert!(e.player_cast_release(RIGHT)); // released before any paid delivery
    assert!(e.player_cast_press(RIGHT));
    e.update_casting(0.1);
    for _ in 0..8 {
        e.update_casting(0.25);
    }
    assert_eq!(spell_uses(&e, CHANNEL), 1);
    assert!(e.player_cast_release(RIGHT));
    e.equip_spell(PLAYER_REF, CHANNEL, LEFT);
    assert!(e.player_cast_press(RIGHT));
    assert!(e.player_cast_press(LEFT));
    e.update_casting(0.1);
    assert_eq!(spell_uses(&e, CHANNEL), 3);
    assert!(e.player_cast_release(RIGHT));
    assert!(e.player_cast_release(LEFT));
    assert_eq!(
        e.stat("Favorite School"),
        StatValue::Name("Translated Restoration".into())
    );

    // The school aggregates uses of different spells, rather than simply
    // reporting the favorite spell's school.
    release_spell(&mut e, SPARK, 0.5);
    release_spell(&mut e, SPARK, 0.5);
    release_spell(&mut e, FIRE, 0.5);
    assert_eq!(
        e.stat("Favorite School"),
        StatValue::Name("Destruction".into())
    );
    assert_eq!(
        e.stat("Favorite Spell"),
        StatValue::Name("Test Channel".into())
    );

    // Plain effects landing, NPC casts and invalid forms leave usage alone.
    assert!(e.apply_item(HEAL, Some(PLAYER_REF), PLAYER_REF));
    assert!(e.cast_item(HEAL, Some(FormId(0x899)), PLAYER_REF));
    assert!(!e.cast_item(FormId(0x999), Some(PLAYER_REF), PLAYER_REF));
    assert_eq!(spell_uses(&e, HEAL), 0);
    for id in [0x814, 0x815, 0x816] {
        assert!(e.cast_item(FormId(id), Some(PLAYER_REF), PLAYER_REF));
        assert_eq!(spell_uses(&e, FormId(id)), 0);
    }

    // Exercise the actual Papyrus native route and console command.
    let this = e.object_value(HEAL);
    let player = e.object_value(PLAYER_REF);
    crate::script::natives::call(&mut e, "spell", "cast", Some(&this), &[player.clone()]);
    assert_eq!(spell_uses(&e, HEAL), 1);
    let npc = e.object_value(FormId(0x899));
    crate::script::natives::call(
        &mut e,
        "spell",
        "remotecast",
        Some(&this),
        &[npc.clone(), player.clone(), player.clone()],
    );
    assert_eq!(spell_uses(&e, HEAL), 2);
    crate::script::natives::call(
        &mut e,
        "spell",
        "remotecast",
        Some(&this),
        &[player.clone(), npc, player],
    );
    assert_eq!(spell_uses(&e, HEAL), 2);
    crate::console::execute(&mut e, "player.cast 00000812 player");
    assert_eq!(spell_uses(&e, HEAL), 3);
    assert_eq!(
        e.stat("Favorite Spell"),
        StatValue::Name("Test Channel".into())
    ); // tied
    release_spell(&mut e, HEAL, 0.5);
    assert_eq!(
        e.stat("Favorite Spell"),
        StatValue::Name("Test Heal".into())
    );
    assert_eq!(e.query_stat("Favorite Spell"), 0);

    // Learning/unlocking does not shout. Cooldown and key-up after auto-release
    // do not shout twice; the backing normal spell is not a spell-use action.
    e.equip_shout(PLAYER_REF, SHOUT);
    e.player_voice_press();
    e.player_voice_release();
    assert_eq!(e.query_stat("Times Shouted"), 0);
    e.unlock_word(WORD);
    e.player_voice_press();
    e.update_shouts(0.01); // one-word shout automatically releases
    e.player_voice_release();
    assert_eq!(e.query_stat("Times Shouted"), 1);
    e.player_voice_press();
    e.player_voice_release();
    assert_eq!(e.query_stat("Times Shouted"), 1);
    assert_eq!(
        e.stat("Favorite Shout"),
        StatValue::Name("Test Shout".into())
    );
    assert_eq!(spell_uses(&e, FIRE), 2);
}
