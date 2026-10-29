//! The authored crafting movie. All state-changing actions stay in the engine.
use super::{Buttons, num, text};
use crate::engine::Engine;
use esp::FormId;
use ui_swf::{GameCall, Menu, UiSwf, Value};
pub(super) const M: &str = "_root.Menu";

pub(super) struct CraftingMenu {
    pub menu: Menu,
    pub station: FormId,
    pub kind: u8,
    pub selected: Option<usize>,
    pub category: usize,
    pub recipes: Vec<crate::world::crafting::Recipe>,
}
impl CraftingMenu {
    pub fn open(
        ui: &UiSwf,
        size: (u32, u32),
        e: &Engine,
        station: FormId,
        kind: u8,
        buttons: std::sync::Arc<Buttons>,
    ) -> anyhow::Result<Self> {
        let frame = match kind {
            1 => "ConstructibleObject",
            2 | 7 => "Smithing",
            3 => "EnchantConstruct",
            5 => "Alchemy",
            _ => anyhow::bail!("unsupported workbench {kind}"),
        };
        let menu = ui.open("craftingmenu.swf", size)?;
        super::set_button_mapping(&menu, &buttons);
        menu.set_responder(move |call, _| match call.name.as_str() {
            "CanFadeItemInfo" => Some(vec![Value::Bool(true)]),
            _ => super::respond(&buttons, call),
        });
        menu.invoke(&format!("{M}.gotoAndStop"), &[text(frame)]);
        menu.invoke(&format!("{M}.Initialize"), &[]);
        menu.invoke(&format!("{M}.SetPlatform"), &[num(0.), Value::Bool(false)]);
        let description = match kind {
            1 => "sConstructibleMenuDescription",
            2 | 7 => "sSmithingMenuDescription",
            3 => "sEnchantMenuDescription",
            _ => "sAlchemyMenuDescription",
        };
        menu.set(
            &format!("{M}.MenuDescription.text"),
            text(&e.gmst_string(description).unwrap_or_default()),
        );
        menu.set(
            &format!("{M}.MenuName.text"),
            text(
                &e.base_of(station)
                    .map(|f| e.form_name(f))
                    .unwrap_or_default(),
            ),
        );
        menu.set(
            &format!("{M}.ButtonText"),
            Value::List(vec![
                text(&e.gmst_string("sSelect").unwrap_or_else(|| "Select".into())),
                text(&e.gmst_string("sExit").unwrap_or_else(|| "Exit".into())),
                text(""),
                text(&e.gmst_string("sCraft").unwrap_or_else(|| "Craft".into())),
            ]),
        );
        menu.set(&format!("{M}.bCanCraft"), Value::Bool(false));
        menu.invoke(&format!("{M}.UpdateButtonText"), &[]);
        menu.callback(
            "SetCategoriesList",
            &[
                text(&e.gmst_string("sAll").unwrap_or_else(|| "All".into())),
                num(65535.),
                Value::Bool(true),
            ],
        );
        let mut result = Self {
            menu,
            station,
            kind,
            selected: None,
            category: 0,
            recipes: Vec::new(),
        };
        result.rebuild(e);
        Ok(result)
    }
    pub fn rebuild(&mut self, e: &Engine) {
        if self.kind != 1 {
            return;
        }
        self.recipes = e.recipes_at(self.station);
        let mut categories = std::collections::BTreeMap::new();
        let mut entries = Vec::new();
        for (i, r) in self.recipes.iter().enumerate() {
            let Some(info) = crate::world::inventory::item_info(&e.lo, r.output) else {
                continue;
            };
            let (name, flag) = e.crafting_category(r.output);
            categories.insert(name, flag);
            let can = e
                .inventories
                .get(&crate::engine::PLAYER_REF)
                .is_some_and(|inv| r.affordable(inv));
            entries.push(super::item_menu::object([
                ("text", text(&info.name)),
                ("count", num(r.count as f32)),
                ("filterFlag", Value::Number(flag as f64)),
                ("enabled", Value::Bool(can)),
                ("isEnabled", Value::Bool(can)),
                ("vrmRow", num(i as f32)),
            ]));
        }
        let mut cats = vec![
            text(&e.gmst_string("sAll").unwrap_or_else(|| "All".into())),
            Value::Number(0x3FFFFFFFu32 as f64),
            Value::Bool(true),
        ];
        for (name, flag) in categories {
            cats.extend([text(&name), Value::Number(flag as f64), Value::Bool(false)]);
        }
        self.menu.invoke(&format!("{M}.PreRebuildList"), &[]);
        self.menu.callback("SetCategoriesList", &cats);
        let list = format!("{M}.ItemList.entryList");
        self.menu.invoke(&format!("{list}.splice"), &[num(0.)]);
        if !entries.is_empty() {
            self.menu.invoke(&format!("{list}.push"), &entries);
        }
        self.menu
            .invoke(&format!("{M}.UpdateItemList"), &[Value::Bool(true)]);
        self.menu
            .invoke(&format!("{M}.PostRebuildList"), &[Value::Bool(true)]);
        self.display(e);
    }
    fn display(&self, e: &Engine) {
        let Some(r) = self.selected.and_then(|i| self.recipes.get(i)) else {
            self.menu.set(&format!("{M}.bCanCraft"), Value::Bool(false));
            return;
        };
        if let Some(info) = crate::world::inventory::item_info(&e.lo, r.output) {
            let row = crate::items::ListedItem {
                item: r.output,
                owner: None,
                count: r.count as i32,
                copy: Default::default(),
                info,
            };
            let card = super::item_menu::item_card(e, &row, None, false);
            self.menu
                .invoke(&format!("{M}.ItemInfo.SetItemInfo"), &[card]);
        }
        let inv = e.inventories.get(&crate::engine::PLAYER_REF);
        let inputs = r
            .inputs
            .iter()
            .map(|&(item, n)| {
                super::item_menu::object([
                    ("Name", text(&e.form_name(item))),
                    ("RequiredCount", num(n as f32)),
                    ("PlayerCount", num(inv.map_or(0, |i| i.count(item)) as f32)),
                ])
            })
            .collect();
        self.menu.invoke(
            &format!("{M}.UpdateIngredients"),
            &[
                text(
                    &e.gmst_string("sRequires")
                        .unwrap_or_else(|| "Requires".into()),
                ),
                Value::List(inputs),
                Value::Bool(true),
            ],
        );
        self.menu.set(
            &format!("{M}.bCanCraft"),
            Value::Bool(inv.is_some_and(|i| r.affordable(i))),
        );
        self.menu.invoke(&format!("{M}.UpdateButtonText"), &[]);
    }
    pub fn answer(&mut self, e: &mut Engine, c: &GameCall) -> bool {
        let index = || {
            c.args.first().and_then(|v| match v {
                Value::Number(n) if *n >= 0. && n.is_finite() => Some(*n as usize),
                _ => None,
            })
        };
        match c.name.as_str() {
            "CloseMenu" => e.menu = None,
            "SetSelectedCategory" => self.category = index().unwrap_or(0),
            "SetSelectedItem" => {
                self.selected = index();
                self.display(e);
            }
            "CanFadeItemInfo" => {
                self.menu
                    .callback("SetCanFadeItemInfo", &[Value::Bool(true)]);
            }
            "ShowItem3D" | "StartMouseRotation" | "StopMouseRotation" => {
                log::debug!("crafting preview {}: {:?}", c.name, c.args)
            }
            "ChooseItem" | "CraftSelectedItem" | "CraftButtonPress" | "AuxButtonPress"
            | "EndItemRename" | "CalculateCharge" | "SliderClose" => {}
            _ => return false,
        }
        true
    }
    pub fn key(&self, code: winit::keyboard::KeyCode) -> bool {
        use winit::keyboard::KeyCode;
        let method = match code {
            KeyCode::Escape => "onExitButtonPress",
            KeyCode::KeyR => "onCraftButtonPress",
            KeyCode::KeyF => "onAuxButtonPress",
            _ => return false,
        };
        self.menu.invoke(&format!("{M}.{method}"), &[]);
        true
    }
}
