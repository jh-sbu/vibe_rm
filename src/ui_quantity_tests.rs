use super::*;
use esp::FormId;
use esp::writer::{Node, RecordBuilder, write_plugin};

const ITEM: FormId = FormId(0x800);
const OWNER: FormId = FormId(0x801);
const ACTOR: FormId = FormId(0x802);
const CHEST: FormId = FormId(0x803);
const CHEST_BASE: FormId = FormId(0x804);
const CELL: FormId = FormId(0x805);

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
    let dir = Fixture(
        std::env::temp_dir().join(format!("vrm-ui-quantity-{}-{stamp}", std::process::id())),
    );
    std::fs::create_dir(&dir.0).unwrap();
    let cell_record = RecordBuilder::new(b"CELL", CELL).field(b"DATA", &[0x21, 0]);
    let chest_ref = RecordBuilder::new(b"REFR", CHEST).form(b"NAME", CHEST_BASE);
    let records = [
        Node::top(
            b"MISC",
            vec![
                RecordBuilder::new(b"MISC", ITEM)
                    .zstring(b"EDID", "QuantityTestItem")
                    .zstring(b"FULL", "Quantity Test Item")
                    .field(
                        b"DATA",
                        &[
                            100i32.to_le_bytes().as_slice(),
                            1.0f32.to_le_bytes().as_slice(),
                        ]
                        .concat(),
                    ),
            ],
        ),
        Node::top(
            b"NPC_",
            vec![RecordBuilder::new(b"NPC_", OWNER).zstring(b"EDID", "QuantityOwner")],
        ),
        Node::top(
            b"ACHR",
            vec![RecordBuilder::new(b"ACHR", ACTOR).form(b"NAME", OWNER)],
        ),
        Node::top(
            b"CONT",
            vec![RecordBuilder::new(b"CONT", CHEST_BASE).zstring(b"FULL", "Test Chest")],
        ),
        Node::Group {
            label: *b"CELL",
            group_type: 0,
            children: vec![
                Node::Record(cell_record),
                Node::Group {
                    label: CELL.0.to_le_bytes(),
                    group_type: esp::group_type::CELL_CHILDREN,
                    children: vec![Node::Group {
                        label: CELL.0.to_le_bytes(),
                        group_type: esp::group_type::CELL_TEMPORARY,
                        children: vec![Node::Record(chest_ref)],
                    }],
                },
            ],
        },
    ];
    std::fs::write(
        dir.0.join("Quantity.esp"),
        write_plugin(0, &[], "test", "quantity picker", 0x900, &records),
    )
    .unwrap();
    let engine = crate::stats::action_tests::headless_engine(&dir.0, "Quantity.esp");
    (dir, engine)
}

fn pending(
    menu: crate::items::Menu,
    source: FormId,
    destination: Option<FormId>,
    owner: Option<FormId>,
    amount: i32,
) -> PendingQuantity {
    PendingQuantity {
        menu,
        source,
        destination,
        item: ITEM,
        owner,
        available: 9,
        amount,
    }
}

#[test]
fn quantity_chooser_threshold_and_shift_all_policy() {
    assert!(matches!(
        choose_quantity(
            crate::items::Menu::Inventory,
            crate::engine::PLAYER_REF,
            None,
            ITEM,
            None,
            5,
            false,
        ),
        QuantityChoice::Immediate(1)
    ));
    assert!(matches!(
        choose_quantity(
            crate::items::Menu::Inventory,
            crate::engine::PLAYER_REF,
            None,
            ITEM,
            None,
            6,
            false,
        ),
        QuantityChoice::Choose(PendingQuantity {
            amount: 1,
            available: 6,
            ..
        })
    ));
    assert!(matches!(
        choose_quantity(
            crate::items::Menu::Inventory,
            crate::engine::PLAYER_REF,
            None,
            ITEM,
            None,
            6,
            true,
        ),
        QuantityChoice::Immediate(6)
    ));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn quantity_confirmation_moves_the_selected_ownership_row_and_clamps_to_current_count() {
    let (_dir, mut e) = fixture();
    e.add_item(crate::engine::PLAYER_REF, ITEM, 3);
    e.inventory_mut(crate::engine::PLAYER_REF)
        .add_owned(ITEM, Some(OWNER), 8);

    let mut selection = QuantitySelection::default();
    selection.open(pending(
        crate::items::Menu::Container(CHEST),
        crate::engine::PLAYER_REF,
        Some(CHEST),
        Some(OWNER),
        5,
    ));
    assert_eq!(
        selection.confirm(&mut e, Some(crate::items::Menu::Container(CHEST))),
        QuantityResult::Moved(5)
    );
    assert_eq!(
        e.inventory_mut(crate::engine::PLAYER_REF)
            .count_owned(ITEM, None),
        3
    );
    assert_eq!(
        e.inventory_mut(crate::engine::PLAYER_REF)
            .count_owned(ITEM, Some(OWNER)),
        3
    );
    assert_eq!(e.inventory_mut(CHEST).count_owned(ITEM, Some(OWNER)), 5);

    // Inventory changed while the chooser was open: take no more than remains.
    selection.open(pending(
        crate::items::Menu::Container(CHEST),
        crate::engine::PLAYER_REF,
        Some(CHEST),
        Some(OWNER),
        9,
    ));
    e.remove_stack(
        crate::engine::PLAYER_REF,
        ITEM,
        1,
        Some(Some(OWNER)),
        None,
        None,
    );
    assert_eq!(
        selection.confirm(&mut e, Some(crate::items::Menu::Container(CHEST))),
        QuantityResult::Moved(2)
    );
    assert_eq!(e.inventory_mut(CHEST).count_owned(ITEM, Some(OWNER)), 7);
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn quantity_cancellation_and_stale_menu_or_stack_do_not_transfer_items() {
    let (_dir, mut e) = fixture();
    e.add_item(crate::engine::PLAYER_REF, ITEM, 8);
    let mut selection = QuantitySelection::default();
    selection.open(pending(
        crate::items::Menu::Container(CHEST),
        crate::engine::PLAYER_REF,
        Some(CHEST),
        None,
        4,
    ));
    selection.cancel();
    assert_eq!(e.item_count(crate::engine::PLAYER_REF, ITEM), 8);
    assert_eq!(e.item_count(CHEST, ITEM), 0);

    selection.open(pending(
        crate::items::Menu::Container(CHEST),
        crate::engine::PLAYER_REF,
        Some(CHEST),
        None,
        4,
    ));
    assert_eq!(
        selection.confirm(&mut e, Some(crate::items::Menu::Inventory)),
        QuantityResult::Stale
    );
    selection.open(pending(
        crate::items::Menu::Container(CHEST),
        crate::engine::PLAYER_REF,
        Some(CHEST),
        Some(OWNER),
        4,
    ));
    assert_eq!(
        selection.confirm(&mut e, Some(crate::items::Menu::Container(CHEST))),
        QuantityResult::Stale
    );
    assert_eq!(e.item_count(crate::engine::PLAYER_REF, ITEM), 8);
    assert_eq!(e.item_count(CHEST, ITEM), 0);
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn quantity_pickpocket_uses_the_selected_amount() {
    let (_dir, mut e) = fixture();
    e.add_item(ACTOR, ITEM, 8);
    e.set_actor_value(
        crate::engine::PLAYER_REF,
        esp::actor_value::PICKPOCKET,
        1000.0,
    );
    e.rng = 1; // first pickpocket roll is 65; the high skill caps the chance at 90%.
    let mut selection = QuantitySelection::default();
    selection.open(pending(
        crate::items::Menu::Pickpocket(ACTOR),
        ACTOR,
        Some(crate::engine::PLAYER_REF),
        None,
        6,
    ));
    assert_eq!(
        selection.confirm(&mut e, Some(crate::items::Menu::Pickpocket(ACTOR))),
        QuantityResult::PickpocketAttempt(true)
    );
    assert_eq!(e.item_count(ACTOR, ITEM), 2);
    assert_eq!(e.item_count(crate::engine::PLAYER_REF, ITEM), 6);

    e.rng = 1;
    selection.open(pending(
        crate::items::Menu::Pickpocket(ACTOR),
        crate::engine::PLAYER_REF,
        Some(ACTOR),
        Some(OWNER),
        3,
    ));
    assert_eq!(
        selection.confirm(&mut e, Some(crate::items::Menu::Pickpocket(ACTOR))),
        QuantityResult::PickpocketAttempt(true)
    );
    assert_eq!(e.item_count(crate::engine::PLAYER_REF, ITEM), 3);
    assert_eq!(e.item_count(ACTOR, ITEM), 5);
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn quantity_drop_removes_only_the_selected_ownership_row() {
    let (_dir, mut e) = fixture();
    e.location = crate::engine::Location::Interior(CELL);
    e.add_item(crate::engine::PLAYER_REF, ITEM, 3);
    e.inventory_mut(crate::engine::PLAYER_REF)
        .add_owned(ITEM, Some(OWNER), 6);
    let mut selection = QuantitySelection::default();
    selection.open(pending(
        crate::items::Menu::Inventory,
        crate::engine::PLAYER_REF,
        None,
        Some(OWNER),
        4,
    ));
    assert_eq!(
        selection.confirm(&mut e, Some(crate::items::Menu::Inventory)),
        QuantityResult::Moved(4)
    );
    assert_eq!(
        e.inventory_mut(crate::engine::PLAYER_REF)
            .count_owned(ITEM, None),
        3
    );
    assert_eq!(
        e.inventory_mut(crate::engine::PLAYER_REF)
            .count_owned(ITEM, Some(OWNER)),
        2
    );
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn escape_and_tab_cancel_the_picker_without_closing_or_mutating_its_parent_menu() {
    let (_dir, mut e) = fixture();
    e.add_item(crate::engine::PLAYER_REF, ITEM, 8);
    e.menu = Some(crate::items::Menu::Container(CHEST));

    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        compatible_surface: None,
        apply_limit_buckets: false,
        ..Default::default()
    }))
    .expect("a headless GPU adapter is required for this ignored test");
    let (device, _queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::TEXTURE_COMPRESSION_BC,
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .unwrap();
    let mut ui = Ui::new(&device, wgpu::TextureFormat::Rgba8Unorm);
    let initial_count = e.item_count(crate::engine::PLAYER_REF, ITEM);

    for key in [egui::Key::Escape, egui::Key::Tab] {
        ui.quantity.open(pending(
            crate::items::Menu::Container(CHEST),
            crate::engine::PLAYER_REF,
            Some(CHEST),
            None,
            4,
        ));
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            events: vec![egui::Event::Key {
                key,
                physical_key: Some(key),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::default(),
            }],
            ..Default::default()
        };
        let mut output = ui.build(raw, &mut e);
        output.textures_delta.clear();
        assert!(ui.quantity.pending.is_none());
        assert_eq!(e.menu, Some(crate::items::Menu::Container(CHEST)));
        assert_eq!(e.item_count(crate::engine::PLAYER_REF, ITEM), initial_count);
        assert_eq!(e.item_count(CHEST, ITEM), 0);
    }
}
