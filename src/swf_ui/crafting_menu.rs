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
        Ok(Self {
            menu,
            station,
            kind,
            selected: None,
            category: 0,
        })
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
            "SetSelectedItem" => self.selected = index(),
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
