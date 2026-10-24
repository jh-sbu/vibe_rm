//! Forms scripts add to form lists (`FormList.AddForm`), after the plugin's
//! (`LNAM`), as CommonLibSSE's `BGSListForm::ForEachForm` walks them; only
//! those can be taken out again (`RemoveAddedForm`, `Revert`). Saved: `FLAD`.
//! Choices: `known_ambiguities/formlists.md`.

use std::collections::HashMap;

use esp::FormId;

use crate::engine::Engine;

#[derive(Default)]
pub struct FormLists {
    /// The forms scripts added to each list, in the order added.
    pub(crate) added: HashMap<FormId, Vec<FormId>>,
}

impl Engine {
    /// A form list's forms: the plugin's, then those scripts added.
    pub fn formlist(&self, f: FormId) -> Vec<FormId> {
        let mut forms: Vec<FormId> = match self.lo.get(f) {
            Some(rec) => rec
                .subrecords()
                .filter(|s| s.tag.0 == *b"LNAM")
                .map(|s| rec.fid(s.form_id(0)))
                .collect(),
            None => Vec::new(),
        };
        if let Some(added) = self.formlists.added.get(&f) {
            forms.extend_from_slice(added);
        }
        forms
    }

    /// `FormList.AddForm`: a form not in the list already goes at its end.
    pub fn add_to_formlist(&mut self, list: FormId, form: FormId) {
        if list.is_null() || form.is_null() || self.formlist(list).contains(&form) {
            return;
        }
        self.formlists.added.entry(list).or_default().push(form);
    }

    /// `FormList.RemoveAddedForm`: takes out a form a script added; the
    /// plugin's stay.
    pub fn remove_added_form(&mut self, list: FormId, form: FormId) {
        if let Some(added) = self.formlists.added.get_mut(&list) {
            added.retain(|&f| f != form);
            if added.is_empty() {
                self.formlists.added.remove(&list);
            }
        }
    }

    /// `FormList.Revert`: takes out every form scripts added.
    pub fn revert_formlist(&mut self, list: FormId) {
        self.formlists.added.remove(&list);
    }

    /// Restore the forms scripts added that a save kept, in place of this
    /// session's.
    pub(crate) fn restore_formlist_additions(&mut self, added: HashMap<FormId, Vec<FormId>>) {
        self.formlists.added = added;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp::writer::{Node, RecordBuilder, write_plugin};
    use papyrus::{NativeResult, Value};

    const LIST: FormId = FormId(0x800);
    const A: FormId = FormId(0x801);
    const B: FormId = FormId(0x802);
    const C: FormId = FormId(0x803);

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
        let f = Fixture(
            std::env::temp_dir().join(format!("vrm-formlists-{}-{stamp}", std::process::id())),
        );
        std::fs::create_dir(&f.0).unwrap();
        let records = [
            Node::top(
                b"MISC",
                vec![
                    RecordBuilder::new(b"MISC", A),
                    RecordBuilder::new(b"MISC", B),
                    RecordBuilder::new(b"MISC", C),
                ],
            ),
            Node::top(
                b"FLST",
                vec![RecordBuilder::new(b"FLST", LIST).form(b"LNAM", A)],
            ),
        ];
        std::fs::write(
            f.0.join("FormLists.esp"),
            write_plugin(0, &[], "test", "synthetic form lists", 0x900, &records),
        )
        .unwrap();
        let e = crate::stats::action_tests::headless_engine(&f.0, "FormLists.esp");
        (f, e)
    }

    fn call(e: &mut Engine, func: &str, args: &[FormId]) -> Value {
        let this = e.object_value(LIST);
        let args: Vec<Value> = args.iter().map(|&f| e.object_value(f)).collect();
        match crate::script::natives::call(e, "formlist", func, Some(&this), &args) {
            NativeResult::Value(v) => v,
            _ => panic!("FormList.{func} didn't return"),
        }
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn scripts_add_and_take_out_their_own_forms() {
        let (_f, mut e) = fixture();
        call(&mut e, "addform", &[B]);
        call(&mut e, "addform", &[C]);
        call(&mut e, "addform", &[A]); // Already listed by the plugin.
        call(&mut e, "addform", &[B]); // Already added.
        assert_eq!(e.formlist(LIST), [A, B, C]);
        assert_eq!(call(&mut e, "getsize", &[]), Value::Int(3));
        assert_eq!(call(&mut e, "find", &[C]), Value::Int(2));
        assert_eq!(call(&mut e, "hasform", &[B]), Value::Bool(true));
        call(&mut e, "removeaddedform", &[A]); // The plugin's stays.
        call(&mut e, "removeaddedform", &[B]);
        assert_eq!(e.formlist(LIST), [A, C]);
        assert_eq!(call(&mut e, "find", &[B]), Value::Int(-1));
        call(&mut e, "revert", &[]);
        assert_eq!(e.formlist(LIST), [A]);
        assert!(e.formlists.added.is_empty());
    }
}
