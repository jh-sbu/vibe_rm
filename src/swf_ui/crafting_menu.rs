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
    last_items: Vec<(FormId, i32)>,
    improvements: Vec<crate::crafting::Improvement>,
    ingredients: Vec<crate::items::ListedItem>,
    selected_ingredients: Vec<FormId>,
}
impl CraftingMenu {
    pub fn open(
        ui: &UiSwf,
        size: (u32, u32),
        e: &mut Engine,
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
            last_items: Vec::new(),
            improvements: Vec::new(),
            ingredients: Vec::new(),
            selected_ingredients: Vec::new(),
        };
        result.rebuild(e);
        Ok(result)
    }
    pub fn rebuild(&mut self, e: &mut Engine) {
        if self.kind == 5 {
            self.rebuild_alchemy(e);
            return;
        }
        if matches!(self.kind, 2 | 7) {
            self.rebuild_improvements(e);
            return;
        }
        if self.kind != 1 {
            return;
        }
        self.recipes = e.recipes_at(self.station);
        self.last_items = e
            .inventories
            .get(&crate::engine::PLAYER_REF)
            .map(|i| i.items.clone())
            .unwrap_or_default();
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
    pub fn update(&mut self, e: &mut Engine) {
        if matches!(self.kind, 1 | 2 | 5 | 7)
            && e.inventories
                .get(&crate::engine::PLAYER_REF)
                .is_some_and(|i| i.items != self.last_items)
        {
            self.rebuild(e);
        }
    }
    fn request_improvement(&mut self, e: &mut Engine, index: Option<usize>) {
        let index = match self.menu.get(&format!("{M}.ItemList.selectedEntry.vrmRow")) {
            Value::Number(n) if n >= 0. => Some(n as usize),
            _ => index.or(self.selected),
        };
        let Some(r) = index.and_then(|i| self.improvements.get(i)) else {
            return;
        };
        if r.preview.quality <= r.row.copy.quality
            || !e
                .inventories
                .get(&crate::engine::PLAYER_REF)
                .is_some_and(|i| r.recipe.affordable(i))
        {
            return;
        }
        let question = e
            .gmst_string("sSmithingConfirm")
            .unwrap_or_else(|| "Do you want to improve this item?".into());
        let buttons = vec![
            e.gmst_string("sYes").unwrap_or_else(|| "Yes".into()),
            e.gmst_string("sNo").unwrap_or_else(|| "No".into()),
        ];
        e.ask(
            question,
            buttons,
            crate::messages::BoxAction::ImproveItem {
                station: self.station,
                item: r.row.item,
                owner: r.row.owner,
                key: r.row.copy.fingerprint(),
            },
        );
    }
    fn request_recipe(&mut self, e: &mut Engine, index: Option<usize>) {
        let selected = self.menu.get(&format!("{M}.ItemList.selectedEntry.vrmRow"));
        let index = match selected {
            Value::Number(n) if n >= 0. => Some(n as usize),
            _ => index.or(self.selected),
        };
        let Some(r) = index.and_then(|i| self.recipes.get(i)) else {
            return;
        };
        if !e
            .inventories
            .get(&crate::engine::PLAYER_REF)
            .is_some_and(|i| r.affordable(i))
        {
            return;
        }
        let title = e
            .gmst_string("sConstructibleMenuConfirm")
            .unwrap_or_else(|| "Do you want to create this item?".into());
        let yes = e.gmst_string("sYes").unwrap_or_else(|| "Yes".into());
        let no = e.gmst_string("sNo").unwrap_or_else(|| "No".into());
        e.ask(
            title,
            vec![yes, no],
            crate::messages::BoxAction::CraftRecipe {
                station: self.station,
                recipe: r.id,
            },
        );
    }
    fn rebuild_alchemy(&mut self, e: &mut Engine) {
        self.ingredients = e.alchemy_ingredients();
        self.selected_ingredients
            .retain(|f| self.ingredients.iter().any(|r| r.item == *f));
        self.last_items = e
            .inventories
            .get(&crate::engine::PLAYER_REF)
            .map(|i| i.items.clone())
            .unwrap_or_default();
        let mut effects = std::collections::BTreeSet::new();
        for row in &self.ingredients {
            if let Some(m) = e.magic_item(row.item) {
                for (i, effect) in m.effects.iter().enumerate() {
                    if e.effect_learned(row.item, i) {
                        effects.insert(effect.effect);
                    }
                }
            }
        }
        let filters: std::collections::BTreeMap<_, _> = effects
            .into_iter()
            .take(255)
            .enumerate()
            .map(|(i, f)| (f, (i + 1) as u8))
            .collect();
        let mut categories = vec![
            text(
                &e.gmst_string("sIngredients")
                    .unwrap_or_else(|| "Ingredients".into()),
            ),
            Value::Number(u32::MAX as f64),
            Value::Bool(true),
        ];
        for (&effect, &id) in &filters {
            categories.extend([
                text(&e.form_name(effect)),
                num(id as f32),
                Value::Bool(true),
            ]);
        }
        let entries: Vec<_> = self
            .ingredients
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let mut ids = [0u8; 4];
                if let Some(m) = e.magic_item(row.item) {
                    for (index, effect) in m.effects.iter().take(4).enumerate() {
                        if e.effect_learned(row.item, index) {
                            ids[index] = filters.get(&effect.effect).copied().unwrap_or(0);
                        }
                    }
                }
                super::item_menu::object([
                    ("text", text(&row.info.name)),
                    ("count", num(row.count as f32)),
                    ("filterFlag", Value::Number(u32::from_le_bytes(ids) as f64)),
                    (
                        "enabled",
                        Value::Bool(
                            self.selected_ingredients.contains(&row.item)
                                || self.selected_ingredients.iter().all(|f| {
                                    !e.knowledge
                                        .failed_alchemy
                                        .contains(&((*f).min(row.item), (*f).max(row.item)))
                                }),
                        ),
                    ),
                    (
                        "isSelected",
                        Value::Bool(self.selected_ingredients.contains(&row.item)),
                    ),
                    ("vrmRow", num(i as f32)),
                ])
            })
            .collect();
        self.menu.invoke(
            &format!("{M}.SetPartitionedFilterMode"),
            &[Value::Bool(true)],
        );
        self.menu.callback("SetCategoriesList", &categories);
        let list = format!("{M}.ItemList.entryList");
        self.menu.invoke(&format!("{list}.splice"), &[num(0.)]);
        if !entries.is_empty() {
            self.menu.invoke(&format!("{list}.push"), &entries);
        }
        self.menu
            .invoke(&format!("{M}.UpdateItemList"), &[Value::Bool(true)]);
        self.display(e);
    }
    fn rebuild_improvements(&mut self, e: &mut Engine) {
        self.improvements = e.improvements_at(self.station);
        self.last_items = e
            .inventories
            .get(&crate::engine::PLAYER_REF)
            .map(|i| i.items.clone())
            .unwrap_or_default();
        let entries: Vec<_> = self
            .improvements
            .iter()
            .enumerate()
            .map(|(i, r)| {
                super::item_menu::object([
                    (
                        "text",
                        text(
                            &e.copy_item_info(r.row.item, &r.preview)
                                .map(|i| i.name)
                                .unwrap_or_default(),
                        ),
                    ),
                    ("count", num(r.row.count as f32)),
                    (
                        "enabled",
                        Value::Bool(
                            r.preview.quality > r.row.copy.quality
                                && e.inventories
                                    .get(&crate::engine::PLAYER_REF)
                                    .is_some_and(|i| r.recipe.affordable(i)),
                        ),
                    ),
                    ("vrmRow", num(i as f32)),
                ])
            })
            .collect();
        let list = format!("{M}.ItemList.entryList");
        self.menu.invoke(&format!("{list}.splice"), &[num(0.)]);
        if !entries.is_empty() {
            self.menu.invoke(&format!("{list}.push"), &entries);
        }
        self.menu
            .invoke(&format!("{M}.UpdateItemList"), &[Value::Bool(false)]);
    }
    fn display(&self, e: &Engine) {
        if self.kind == 5 {
            self.menu.set(
                &format!("{M}.bCanCraft"),
                Value::Bool(self.selected_ingredients.len() >= 2),
            );
            self.menu.invoke(&format!("{M}.UpdateButtonText"), &[]);
            if let Some(result) = e.potion_result(&self.selected_ingredients) {
                let names = result
                    .effects
                    .iter()
                    .map(|effect| {
                        let known = self.selected_ingredients.iter().any(|&ingredient| {
                            e.magic_item(ingredient).is_some_and(|m| {
                                m.effects.iter().enumerate().any(|(i, ef)| {
                                    ef.effect == effect.effect && e.effect_learned(ingredient, i)
                                })
                            })
                        });
                        if known {
                            e.form_name(effect.effect)
                        } else {
                            "?".into()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                self.menu
                    .set(&format!("{M}.AdditionalDescription.text"), text(&names));
                let known = result.effects.iter().all(|effect| {
                    self.selected_ingredients.iter().any(|&ingredient| {
                        e.magic_item(ingredient).is_some_and(|m| {
                            m.effects.iter().enumerate().any(|(i, ef)| {
                                ef.effect == effect.effect && e.effect_learned(ingredient, i)
                            })
                        })
                    })
                });
                let name = if known {
                    result.name.clone()
                } else {
                    e.lo.find_editor_id("DefaultPotion")
                        .map(|f| e.form_name(f))
                        .unwrap_or_else(|| "Unknown Potion".into())
                };
                let card = super::item_menu::object([
                    ("name", text(&name)),
                    ("count", num(1.)),
                    ("value", num(if known { result.value as f32 } else { 0. })),
                    ("weight", num(0.5)),
                    ("type", num(6.)),
                    ("effects", text(&names)),
                ]);
                self.menu
                    .invoke(&format!("{M}.ItemInfo.SetItemInfo"), &[card]);
                return;
            }
            if let Some(row) = self.selected.and_then(|i| self.ingredients.get(i)) {
                let card = super::item_menu::item_card(e, row, None, false);
                self.menu
                    .invoke(&format!("{M}.ItemInfo.SetItemInfo"), &[card]);
            }
            return;
        }

        if matches!(self.kind, 2 | 7) {
            if let Some(r) = self.selected.and_then(|i| self.improvements.get(i)) {
                let mut row = r.row.clone();
                row.copy = r.preview.clone();
                row.info = e
                    .copy_item_info(row.item, &row.copy)
                    .unwrap_or_else(|| row.info.clone());
                let card = super::item_menu::item_card(e, &row, None, false);
                self.menu
                    .invoke(&format!("{M}.ItemInfo.SetItemInfo"), &[card]);
                let inv = e.inventories.get(&crate::engine::PLAYER_REF);
                self.menu.set(
                    &format!("{M}.bCanCraft"),
                    Value::Bool(
                        r.preview.quality > r.row.copy.quality
                            && inv.is_some_and(|i| r.recipe.affordable(i)),
                    ),
                );
                let inputs = r
                    .recipe
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
                self.menu.invoke(&format!("{M}.UpdateButtonText"), &[]);
            }
            return;
        }
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
            "ChooseItem" if self.kind == 5 => {
                let picked = match self.menu.get(&format!("{M}.ItemList.selectedEntry.vrmRow")) {
                    Value::Number(n) if n >= 0. => Some(n as usize),
                    _ => index(),
                };
                if let Some(form) = picked.and_then(|i| self.ingredients.get(i)).map(|r| r.item) {
                    if let Some(i) = self.selected_ingredients.iter().position(|f| *f == form) {
                        self.selected_ingredients.remove(i);
                    } else if self.selected_ingredients.len() < 3 {
                        self.selected_ingredients.push(form);
                    }
                    self.rebuild_alchemy(e);
                }
            }
            "CloseMenu" => e.menu = None,
            "SetSelectedCategory" => self.category = index().unwrap_or(0),
            "SetSelectedItem" => {
                self.selected = match self.menu.get(&format!("{M}.ItemList.selectedEntry.vrmRow")) {
                    Value::Number(n) if n >= 0. => Some(n as usize),
                    _ => index(),
                };
                self.display(e);
            }
            "CanFadeItemInfo" => {
                self.menu
                    .callback("SetCanFadeItemInfo", &[Value::Bool(true)]);
            }
            "ShowItem3D" | "StartMouseRotation" | "StopMouseRotation" => {
                log::debug!("crafting preview {}: {:?}", c.name, c.args)
            }
            "ChooseItem" | "CraftSelectedItem" | "CraftButtonPress"
                if matches!(self.kind, 2 | 7) =>
            {
                self.request_improvement(e, index());
            }
            "ChooseItem" | "CraftSelectedItem" | "CraftButtonPress" if self.kind == 1 => {
                self.request_recipe(e, index());
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
