//! `test-results`: a test mod's reports, read from a save the game made
//! after the tests ran. A test mod's script keeps each report in a variable
//! named `Result<Name>` (a script named `Vrm...`), so a save holds them
//! word for word, in the order declared, however the game showed them.

use std::fmt::Write;

use ess::{Papyrus, RefId, Save, Variable};

/// One script instance's reports: script, the form it's on, reports (name
/// without `Result`, text).
pub struct Reports {
    pub script: String,
    pub form: Option<u32>,
    pub reports: Vec<(String, String)>,
}

impl Reports {
    /// Whether any test has reported (its variable isn't empty).
    pub fn any(&self) -> bool {
        self.reports.iter().any(|(_, r)| !r.is_empty())
    }
}

/// A variable as a report: strings as they are, the rest as Papyrus would
/// print them.
fn text(p: &Papyrus, v: &Variable) -> String {
    match v {
        Variable::Null => String::new(),
        Variable::String(s) => s.clone(),
        Variable::Int(i) => i.to_string(),
        Variable::Float(f) => format!("{f:.6}"),
        Variable::Bool(b) => if *b { "True" } else { "False" }.into(),
        Variable::Object(class, handle) => format!("[{class} <{:08X}>]", *handle as u32),
        Variable::Array(_, id) => match p.array(*id) {
            Some(a) => {
                let items: Vec<String> = a.elements.iter().map(|e| text(p, e)).collect();
                format!("[{}]", items.join(", "))
            }
            None => "None".into(),
        },
    }
}

/// The reports of every test mod script in a save.
pub fn read(save: &Save) -> anyhow::Result<Vec<Reports>> {
    Ok(read_papyrus(&save.papyrus()?, |r| save.form_id(r)))
}

/// The reports of every test mod script in a Papyrus section, with what
/// the save's form references stand for.
fn read_papyrus(p: &Papyrus, form_id: impl Fn(RefId) -> Option<u32>) -> Vec<Reports> {
    let mut out = Vec::new();
    for i in &p.instances {
        if !i.script.to_ascii_lowercase().starts_with("vrm") {
            continue;
        }
        let reports: Vec<(String, String)> = p
            .members(&i.script)
            .iter()
            .zip(&i.variables)
            .filter_map(|((name, _), v)| {
                let test = name
                    .get(..6)
                    .filter(|p| p.eq_ignore_ascii_case("result"))
                    .map(|_| &name[6..])?;
                Some((test.to_owned(), text(p, v)))
            })
            .collect();
        if !reports.is_empty() {
            out.push(Reports {
                script: i.script.clone(),
                form: form_id(i.form),
                reports,
            });
        }
    }
    out
}

/// The reports as text, each under its name.
pub fn format(reports: &[Reports]) -> String {
    let mut o = String::new();
    for r in reports {
        let form = r.form.map_or("?".into(), |f| format!("{f:08X}"));
        let _ = writeln!(o, "== {} on {form}", r.script);
        for (name, text) in &r.reports {
            if text.is_empty() {
                let _ = writeln!(o, "-- {name}: (no report: not run, or not finished)\n");
            } else {
                let _ = writeln!(o, "-- {name}:\n{}\n", text.trim_end());
            }
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Papyrus section with one test script instance, laid out as Skyrim
    /// SE saves it, and one vanilla script's.
    fn section() -> Vec<u8> {
        let strings = [
            "VrmAutoTestScript",
            "Quest",
            "Ran",
            "Bool",
            "ResultLevels",
            "String",
            "x1.5: 1 for 1.500\n",
            "ResultLocks",
            "Int[]",
            "DefaultScript",
        ];
        let mut b = Vec::new();
        let u32s = |b: &mut Vec<u8>, vs: &[u32]| {
            for v in vs {
                b.extend(v.to_le_bytes());
            }
        };
        b.extend(6u16.to_le_bytes());
        u32s(&mut b, &[strings.len() as u32]);
        for s in strings {
            b.extend((s.len() as u16).to_le_bytes());
            b.extend(s.as_bytes());
        }
        // Script types: name, parent, variables (name, type).
        u32s(&mut b, &[2, 0, 1, 3, 2, 3, 4, 5, 7, 8, 9, 1, 0]);
        // Instances: id, script, two unknowns, form (default 0x14), unknown.
        u32s(&mut b, &[2]);
        for (id, script) in [(0x1234u64, 0u32), (0x5678, 9)] {
            b.extend(id.to_le_bytes());
            u32s(&mut b, &[script, 0]);
            b.extend([0x40, 0x00, 0x14, 0]);
        }
        // References, arrays (id, Int, length), runtime, running scripts.
        u32s(&mut b, &[0, 1]);
        b.extend(0x99u64.to_le_bytes());
        b.push(3);
        u32s(&mut b, &[2, 0, 0]);
        // The instances' data: id, flag, type, unknown, variables.
        b.extend(0x1234u64.to_le_bytes());
        b.push(3);
        u32s(&mut b, &[0, 0, 3]);
        b.push(5);
        u32s(&mut b, &[1]);
        b.push(2);
        u32s(&mut b, &[6]);
        b.push(13);
        b.extend(0x99u64.to_le_bytes());
        b.extend(0x5678u64.to_le_bytes());
        b.push(3);
        u32s(&mut b, &[9, 0, 0]);
        // The array's data: id, elements.
        b.extend(0x99u64.to_le_bytes());
        for v in [7u32, 9] {
            b.push(3);
            u32s(&mut b, &[v]);
        }
        b
    }

    #[test]
    fn reports_read_from_the_script_variables() {
        let p = Papyrus::parse(&section()).unwrap();
        let reports = read_papyrus(&p, |r| (r.0 >> 22 == 1).then_some(r.0 & 0x3F_FFFF));
        assert_eq!(reports.len(), 1, "only the test script's");
        let r = &reports[0];
        assert_eq!(r.script, "VrmAutoTestScript");
        assert_eq!(r.form, Some(0x14));
        assert!(r.any());
        assert_eq!(
            r.reports,
            [
                ("Levels".to_owned(), "x1.5: 1 for 1.500\n".to_owned()),
                ("Locks".to_owned(), "[7, 9]".to_owned()),
            ]
        );
        assert_eq!(
            format(&reports),
            "== VrmAutoTestScript on 00000014\n-- Levels:\nx1.5: 1 for 1.500\n\n-- Locks:\n[7, 9]\n\n"
        );
    }
}
