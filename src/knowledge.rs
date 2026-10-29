//! What the player knows (`Ingredient.LearnEffect`, `Form.PlayerKnows`), and
//! small session state scripts set: INI settings (`Utility.SetINI*`). Saved:
//! nothing new (learned effects ride in the world state). Choices:
//! `known_ambiguities/remaining-natives.md`.

use std::collections::HashMap;

use esp::FormId;

use crate::engine::Engine;

/// An INI setting a script set.
#[derive(Debug, Clone, PartialEq)]
pub enum IniValue {
    Bool(bool),
    Int(i32),
    Float(f32),
    Str(String),
}

#[derive(Default)]
pub struct Knowledge {
    pub(crate) crafting_help: std::collections::HashSet<[u8; 4]>,
    /// INI settings scripts set this session, by lower-cased name
    /// (`fGrassStartFadeDistance:Grass`). The game writes nothing back.
    pub(crate) ini: HashMap<String, IniValue>,
}

impl Engine {
    /// How many effects an ingredient has.
    fn ingredient_effect_count(&self, ingredient: FormId) -> usize {
        self.lo
            .get(ingredient)
            .filter(|r| r.tag().0 == *b"INGR")
            .map_or(0, |r| {
                r.subrecords().filter(|s| s.tag.0 == *b"EFID").count()
            })
    }

    /// `LearnEffect(aiIndex)`: the player knows an ingredient's effect.
    pub fn learn_effect(&mut self, ingredient: FormId, index: i32) {
        if let Ok(i) = usize::try_from(index)
            && i < self.ingredient_effect_count(ingredient)
        {
            self.world_state.learned_effects.insert((ingredient, i));
        }
    }

    /// `LearnNextEffect`: the first unknown effect, whose index is returned;
    /// -1 when they are all known.
    pub fn learn_next_effect(&mut self, ingredient: FormId) -> i32 {
        let next = (0..self.ingredient_effect_count(ingredient))
            .find(|&i| !self.effect_learned(ingredient, i));
        match next {
            Some(i) => {
                self.world_state.learned_effects.insert((ingredient, i));
                i as i32
            }
            None => -1,
        }
    }

    /// `LearnAllEffects`.
    pub fn learn_all_effects(&mut self, ingredient: FormId) {
        for i in 0..self.ingredient_effect_count(ingredient) {
            self.world_state.learned_effects.insert((ingredient, i));
        }
    }

    /// `Form.PlayerKnows`: the "Known" flag of a word of power (the player
    /// knows it), a magic effect (the player has learned it from an
    /// ingredient) or an enchantment (nothing here teaches one: false).
    pub fn player_knows(&self, form: FormId) -> bool {
        match self.lo.tag_of(form).map(|t| t.0) {
            Some(t) if &t == b"WOOP" => self.shouts.known.contains(&form),
            Some(t) if &t == b"MGEF" => self.world_state.learned_effects.iter().any(|&(ing, i)| {
                self.magic_item(ing)
                    .and_then(|m| m.effects.get(i).map(|e| e.effect))
                    == Some(form)
            }),
            _ => false,
        }
    }

    /// `Utility.SetINI*`.
    pub fn set_ini(&mut self, name: &str, value: IniValue) {
        self.knowledge.ini.insert(name.to_ascii_lowercase(), value);
    }

    /// What a script set a setting to (nothing consumes settings yet).
    #[allow(dead_code)]
    pub fn ini(&self, name: &str) -> Option<&IniValue> {
        self.knowledge.ini.get(&name.to_ascii_lowercase())
    }

    /// `GetWarmthRating`: warmth is part of Survival Mode (a Creation Club
    /// item), which is not here, so nothing is warm.
    pub fn warmth_rating(&self, _form: FormId) -> f32 {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp::writer::{Node, RecordBuilder, write_plugin};
    use papyrus::{NativeResult, Value};

    const INGREDIENT: FormId = FormId(0x800);
    const FIRST: FormId = FormId(0x810);
    const SECOND: FormId = FormId(0x811);
    const THIRD: FormId = FormId(0x812);
    const WORD: FormId = FormId(0x820);
    const OTHER_WORD: FormId = FormId(0x821);
    const CATEGORY: FormId = FormId(0x830);
    const NOT_A_CATEGORY: FormId = FormId(0x831);

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
            std::env::temp_dir().join(format!("vrm-knowledge-{}-{stamp}", std::process::id())),
        );
        std::fs::create_dir(&f.0).unwrap();
        let efit = [0u8; 12];
        let mut ingredient = RecordBuilder::new(b"INGR", INGREDIENT);
        for effect in [FIRST, SECOND, THIRD] {
            ingredient = ingredient.form(b"EFID", effect).field(b"EFIT", &efit);
        }
        let records = [
            Node::top(
                b"MGEF",
                vec![
                    RecordBuilder::new(b"MGEF", FIRST),
                    RecordBuilder::new(b"MGEF", SECOND),
                    RecordBuilder::new(b"MGEF", THIRD),
                ],
            ),
            Node::top(b"INGR", vec![ingredient]),
            Node::top(
                b"WOOP",
                vec![
                    RecordBuilder::new(b"WOOP", WORD),
                    RecordBuilder::new(b"WOOP", OTHER_WORD),
                ],
            ),
            Node::top(
                b"SNCT",
                vec![
                    RecordBuilder::new(b"SNCT", CATEGORY),
                    RecordBuilder::new(b"MISC", NOT_A_CATEGORY),
                ],
            ),
        ];
        std::fs::write(
            f.0.join("Knowledge.esp"),
            write_plugin(0, &[], "test", "synthetic knowledge", 0x900, &records),
        )
        .unwrap();
        let e = crate::stats::action_tests::headless_engine(&f.0, "Knowledge.esp");
        (f, e)
    }

    fn call(e: &mut Engine, class: &str, func: &str, on: Option<FormId>, args: &[Value]) -> Value {
        let this = on.map(|f| e.object_value(f));
        match crate::script::natives::call(e, class, func, this.as_ref(), args) {
            NativeResult::Value(v) => v,
            _ => panic!("{class}.{func} didn't return"),
        }
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn ingredients_teach_their_effects_one_at_a_time_or_all_at_once() {
        let (_f, mut e) = fixture();
        let ing = Some(INGREDIENT);
        let learn = |e: &mut Engine, i| call(e, "ingredient", "learneffect", ing, &[Value::Int(i)]);
        learn(&mut e, 1);
        learn(&mut e, 7); // no such effect
        learn(&mut e, -1);
        assert!(!e.effect_learned(INGREDIENT, 0) && e.effect_learned(INGREDIENT, 1));
        assert_eq!(e.world_state.learned_effects.len(), 1);
        // The next unknown one: 0, then 2, then none.
        assert_eq!(
            call(&mut e, "ingredient", "learnnexteffect", ing, &[]),
            Value::Int(0)
        );
        assert_eq!(
            call(&mut e, "ingredient", "learnnexteffect", ing, &[]),
            Value::Int(2)
        );
        assert_eq!(
            call(&mut e, "ingredient", "learnnexteffect", ing, &[]),
            Value::Int(-1)
        );
        e.world_state.learned_effects.clear();
        call(&mut e, "ingredient", "learnalleffects", ing, &[]);
        assert_eq!(e.world_state.learned_effects.len(), 3);
        // Not an ingredient: nothing to learn.
        assert_eq!(
            call(&mut e, "ingredient", "learnnexteffect", Some(WORD), &[]),
            Value::Int(-1)
        );
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn the_player_knows_the_words_and_effects_they_have_learned() {
        let (_f, mut e) = fixture();
        let knows =
            |e: &mut Engine, f| call(e, "form", "playerknows", Some(f), &[]) == Value::Bool(true);
        assert!(!knows(&mut e, WORD) && !knows(&mut e, SECOND) && !knows(&mut e, INGREDIENT));
        e.teach_word(WORD);
        assert!(knows(&mut e, WORD) && !knows(&mut e, OTHER_WORD));
        e.learn_effect(INGREDIENT, 1);
        assert!(knows(&mut e, SECOND));
        assert!(!knows(&mut e, FIRST) && !knows(&mut e, THIRD));
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn game_times_print_as_a_date_and_a_24_hour_clock() {
        let (_f, mut e) = fixture();
        e.day = 0;
        e.hour = 8.0;
        let time = |e: &mut Engine, days: f32| {
            call(
                e,
                "utility",
                "gametimetostring",
                None,
                &[Value::Float(days)],
            )
        };
        assert_eq!(time(&mut e, 0.0), Value::str("08/17/0201 00:00"));
        assert_eq!(time(&mut e, 1.5), Value::str("08/18/0201 12:00"));
        assert_eq!(time(&mut e, 14.75), Value::str("08/31/0201 18:00"));
        assert_eq!(time(&mut e, 15.0), Value::str("09/01/0201 00:00"));
        // A year of 365 days later is the same day a year on.
        assert_eq!(time(&mut e, 365.0), Value::str("08/17/0202 00:00"));
        // Nonsense is the beginning; a huge time is held to a thousand years.
        assert_eq!(time(&mut e, f32::NAN), Value::str("08/17/0201 00:00"));
        assert_eq!(time(&mut e, -3.0), Value::str("08/17/0201 00:00"));
        let Value::String(far) = time(&mut e, 1.0e12) else {
            panic!("not a string")
        };
        assert!(far.contains('/'));
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn sound_categories_take_mute_pause_volume_and_frequency() {
        let (_f, mut e) = fixture();
        let on = Some(CATEGORY);
        let state = |e: &Engine| e.sound_category(CATEGORY);
        assert_eq!(state(&e), crate::audio::CategoryState::default());
        call(&mut e, "soundcategory", "mute", on, &[]);
        call(&mut e, "soundcategory", "pause", on, &[]);
        call(
            &mut e,
            "soundcategory",
            "setvolume",
            on,
            &[Value::Float(0.25)],
        );
        call(
            &mut e,
            "soundcategory",
            "setfrequency",
            on,
            &[Value::Float(0.5)],
        );
        let s = state(&e);
        assert!(s.muted && s.paused && s.volume == 0.25 && s.frequency == 0.5);
        call(&mut e, "soundcategory", "unmute", on, &[]);
        call(&mut e, "soundcategory", "unpause", on, &[]);
        call(
            &mut e,
            "soundcategory",
            "setvolume",
            on,
            &[Value::Float(7.0)],
        );
        call(
            &mut e,
            "soundcategory",
            "setfrequency",
            on,
            &[Value::Float(f32::NAN)],
        );
        let s = state(&e);
        assert!(!s.muted && !s.paused && s.volume == 1.0 && s.frequency == 1.0);
        // Only sound categories take it.
        call(&mut e, "soundcategory", "mute", Some(NOT_A_CATEGORY), &[]);
        assert!(!e.sound_category(NOT_A_CATEGORY).muted);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn ini_settings_are_kept_for_the_session_without_regard_to_case() {
        let (_f, mut e) = fixture();
        let name = || Value::str("fGrassStartFadeDistance:Grass");
        call(
            &mut e,
            "utility",
            "setinifloat",
            None,
            &[name(), Value::Float(7000.0)],
        );
        assert_eq!(
            e.ini("FGRASSSTARTFADEDISTANCE:GRASS"),
            Some(&IniValue::Float(7000.0))
        );
        call(
            &mut e,
            "utility",
            "setinibool",
            None,
            &[Value::str("bA:B"), Value::Bool(true)],
        );
        call(
            &mut e,
            "utility",
            "setiniint",
            None,
            &[Value::str("iA:B"), Value::Int(3)],
        );
        call(
            &mut e,
            "utility",
            "setinistring",
            None,
            &[Value::str("sA:B"), Value::str("x")],
        );
        assert_eq!(e.ini("ba:b"), Some(&IniValue::Bool(true)));
        assert_eq!(e.ini("ia:b"), Some(&IniValue::Int(3)));
        assert_eq!(e.ini("sa:b"), Some(&IniValue::Str("x".into())));
        assert_eq!(e.ini("missing"), None);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn warmth_is_zero_and_preloading_does_nothing() {
        let (_f, mut e) = fixture();
        for class in ["armor", "light", "actor"] {
            assert_eq!(
                call(&mut e, class, "getwarmthrating", Some(INGREDIENT), &[]),
                Value::Float(0.0)
            );
        }
        assert_eq!(
            call(&mut e, "spell", "preload", Some(INGREDIENT), &[]),
            Value::None
        );
        assert_eq!(
            call(&mut e, "spell", "unload", Some(INGREDIENT), &[]),
            Value::None
        );
    }
}
