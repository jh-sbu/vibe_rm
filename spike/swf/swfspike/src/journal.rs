//! Fixed host data for clicking through the journal without the engine.
use ruffle_core::{
    context::UpdateContext,
    external::{self, Value},
};
use std::sync::atomic::{AtomicBool, Ordering};
pub static ENABLED: AtomicBool = AtomicBool::new(false);
const M: &str = "_root.QuestJournalFader.Menu_mc";
const S: &str = "_root.QuestJournalFader.Menu_mc.SystemFader.Page_mc";
fn n(n: f64) -> Value {
    Value::Number(n)
}
fn t(s: &str) -> Value {
    Value::String(s.into())
}
fn obj(fields: &[(&str, Value)]) -> Value {
    Value::Object(
        fields
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect(),
    )
}
pub fn start() -> Vec<String> {
    vec![
        format!("call:{M}.InitExtensions"),
        format!("call:{M}.SetPlatform|0|false"),
        "cb:RestoreSavedSettings|2|false".into(),
    ]
}
pub fn respond(c: &mut UpdateContext<'_>, name: &str, args: &[Value]) -> bool {
    if !ENABLED.load(Ordering::Relaxed) {
        return false;
    }
    let set = |c: &mut UpdateContext<'_>, path: &str, value| {
        external::set_avm1_variable(c, &format!("{S}.{path}"), value)
    };
    let invoke = |c: &mut UpdateContext<'_>, path: &str, values: Vec<Value>| {
        external::invoke_avm1(c, &format!("{S}.{path}"), values)
    };
    let callback = |c: &mut UpdateContext<'_>, name: &str, values: Vec<Value>| {
        if let Some(cb) = c.external_interface.get_callback("call") {
            cb.call(c, "call", std::iter::once(t(name)).chain(values));
        }
    };
    let answer = match name {
        "ShouldShowMod" | "ShouldShowKinectTunerOption" => Some(vec![Value::Bool(false)]),
        "RequestIsOnPC" => Some(vec![Value::Bool(true)]),
        "SetVersionText" => {
            set(c, "VersionText.text", t("VibeRM journal driver"));
            None
        }
        "SetSaveDisabled" => {
            set(c, "CategoryList.entryList.3.disabled", Value::Bool(true));
            None
        }
        "SAVE" | "LOAD" | "CharacterSelected" => {
            let rows = (1..=3)
                .map(|i| {
                    obj(&[
                        ("text", t(&format!("Prisoner - Riverwood {i}"))),
                        ("fileNum", n(i as f64)),
                        ("name", t("Prisoner")),
                        ("raceName", t("Nord")),
                        ("level", n(7.)),
                        ("playTime", t("01:23:45")),
                        ("dateString", t("2026-10-28")),
                    ])
                })
                .collect();
            set(
                c,
                "SaveLoadListHolder.SaveLoadList_mc.entryList",
                Value::List(rows),
            );
            callback(
                c,
                "onSaveLoadBatchComplete",
                vec![Value::Bool(true), n(3.), n(3.)],
            );
            None
        }
        "RequestGameplayOptions" => {
            for (i, v) in [
                (0, 0.),
                (1, 0.5),
                (6, 2.),
                (7, 1.),
                (8, 1.),
                (9, 1.),
                (10, 1.),
                (11, 6.),
            ] {
                for (key, value) in [("ID", n(i as f64)), ("value", n(v)), ("defaultVal", n(v))] {
                    set(
                        c,
                        &format!("OptionsListsPanel.OptionsLists.List_mc.entryList.{i}.{key}"),
                        value,
                    );
                }
            }
            None
        }
        "PopulateHelpTopics" => {
            invoke(
                c,
                "HelpList.entryList.push",
                vec![obj(&[("text", t("Getting Started")), ("index", n(0.))])],
            );
            None
        }
        "RequestHelpText" => {
            invoke(c, "HelpTitleText.SetText", vec![t("Getting Started")]);
            invoke(
                c,
                "HelpText.SetText",
                vec![t(
                    "Explore Skyrim. Use the journal to save, change settings and read help.",
                )],
            );
            None
        }
        "IsOKtoLoad" => {
            callback(c, "ConfirmOKToLoad", vec![]);
            None
        }
        "SaveSettings" | "SaveControls" => {
            callback(c, "SettingsSaved", vec![]);
            None
        }
        "UseCurrentCharacterFilter" | "ForceStopSaveListLoading" | "OptionChange" => None,
        _ => return false,
    };
    if let (Some(Value::Number(id)), Some(answer)) = (args.first(), answer)
        && let Some(cb) = c.external_interface.get_callback("respond")
    {
        cb.call(c, "respond", std::iter::once(n(*id)).chain(answer));
    }
    true
}
