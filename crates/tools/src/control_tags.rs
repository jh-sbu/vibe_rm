//! Survey bracketed text in winning records, classified against shipped Interface tables.
use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
struct Event {
    context: usize,
    name: String,
    columns: [String; 3],
}

fn events(text: &str) -> Vec<Event> {
    let mut result = Vec::new();
    let (mut context, mut boundary) = (0, false);
    for line in text.lines().map(str::trim) {
        if line.is_empty() {
            boundary = !result.is_empty();
            continue;
        }
        if line.starts_with("//") {
            continue;
        }
        let cols: Vec<_> = line
            .split('\t')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if cols.len() < 4 {
            continue;
        }
        if boundary {
            context += 1;
            boundary = false;
        }
        result.push(Event {
            context,
            name: cols[0].into(),
            columns: std::array::from_fn(|i| cols[i + 1].into()),
        });
    }
    result
}

fn key_names(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| {
            // Tab-separated: some names hold spaces (`Prev Track`).
            if line.trim_start().starts_with("//") {
                return None;
            }
            let mut cols = line.split('\t').map(str::trim).filter(|c| !c.is_empty());
            Some((cols.next()?.to_ascii_lowercase(), cols.next()?.to_owned()))
        })
        .collect()
}

fn classify(tag: &str, events: &[Event], keys: &[BTreeMap<String, String>; 2]) -> String {
    let matches: Vec<_> = events
        .iter()
        .filter(|e| e.name.eq_ignore_ascii_case(tag))
        .collect();
    if let Some(first) = matches.first() {
        return format!(
            "event context={} (all={}) keyboard={} mouse={} gamepad={}",
            first.context,
            matches
                .iter()
                .map(|e| e.context.to_string())
                .collect::<Vec<_>>()
                .join(","),
            first.columns[0],
            first.columns[1],
            first.columns[2]
        );
    }
    for (device, table) in ["keyboard", "mouse"].into_iter().zip(keys) {
        if let Some(code) = table.get(&tag.to_ascii_lowercase()) {
            return format!("key {device} {code}");
        }
    }
    "nothing".into()
}

fn tags(text: &str) -> Vec<&str> {
    let mut rest = text;
    let mut result = Vec::new();
    while let Some(start) = rest.find('[') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find(']') else { break };
        // A nested opener starts a new candidate; prose brackets are intentionally retained.
        let candidate = rest[..end].rsplit('[').next().unwrap();
        if !candidate.is_empty() {
            result.push(candidate);
        }
        rest = &rest[end + 1..];
    }
    result
}

pub fn run(args: &[String]) -> Result<()> {
    anyhow::ensure!(args.len() == 1, "usage: control-tags <Data>");
    let data = std::path::Path::new(&args[0]);
    let names = esp::LoadOrder::default_plugin_list(data, None);
    let mut lo = esp::LoadOrder::load(data, &names)?;
    let vfs = vfs::Vfs::new(data, &names);
    lo.load_strings("english", |p| vfs.read(p));
    // Read the archive directly: a loose/custom map must not change shipped classification.
    let archive = bsa::Archive::open(data.join("Skyrim - Interface.bsa"))?;
    let table = |name: &str| -> Result<String> {
        let path = format!("interface/controls/pc/{name}.txt");
        let bytes = archive
            .read(&path)?
            .with_context(|| format!("missing {path}"))?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    };
    let events = events(&table("controlmap")?);
    let keys = [
        key_names(&table("keyboard_english")?),
        key_names(&table("mouse")?),
    ];
    let help: BTreeSet<_> = lo
        .default_objects()
        .get(b"HMPC")
        .and_then(|id| lo.get(*id))
        .map(|r| {
            r.subrecords()
                .filter(|s| s.tag.0 == *b"LNAM" && s.data.len() == 4)
                .map(|s| r.fid(esp::FormId(s.u32(0))))
                .collect()
        })
        .unwrap_or_default();
    let mut counts: BTreeMap<String, (usize, BTreeMap<String, usize>)> = BTreeMap::new();
    let mut records = 0;
    for (kind, fields) in [
        (b"MESG", &[*b"DESC", *b"FULL", *b"ITXT"][..]),
        (b"LSCR", &[*b"DESC"][..]),
        (b"QUST", &[*b"FULL", *b"NNAM", *b"CNAM"][..]),
        (b"BOOK", &[*b"DESC", *b"FULL"][..]),
    ] {
        for id in lo.ids_of_type(kind) {
            let Some(r) = lo.get(*id) else { continue };
            records += 1;
            for sr in r.subrecords().filter(|s| fields.contains(&s.tag.0)) {
                let source = format!(
                    "{}{}:{}",
                    String::from_utf8_lossy(kind),
                    if help.contains(id) { "(help)" } else { "" },
                    sr.tag
                );
                for tag in tags(&lo.lstring(&r, sr.data)) {
                    let (total, sources) = counts.entry(tag.to_owned()).or_default();
                    *total += 1;
                    *sources.entry(source.clone()).or_default() += 1;
                }
            }
        }
    }
    println!("Plugins: {}", names.join(", "));
    println!(
        "English; {records} winning records; {} HMPC help forms; shipped archive map (no custom bindings)",
        help.len()
    );
    println!(
        "Counts are occurrences, including repeated fields; help MESG counted once. Brackets may be prose, not controls."
    );
    println!(
        "tag\tcount\tsources\tclassification (first context; raw columns retain references/chords)"
    );
    for (tag, (total, sources)) in counts {
        let sources = sources
            .iter()
            .map(|(s, n)| format!("{s}={n}"))
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "[{tag}]\t{total}\t{sources}\t{}",
            classify(&tag, &events, &keys)
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classification_and_contexts() {
        let events = events(
            "// main\nActivate\t0x12\t0xff\t0x1000\n\n// item\nXButton\t!0,Activate\t0xff\t0x4000\nActivate\t0xff\t0xff\t0xff\n",
        );
        let keys = [
            key_names("E\t0x12\t69\nXButton\t0x99\nPrev Track\t0x90"),
            key_names("// Mouse\tCode\nMouse1\t0x0\t0"),
        ];
        assert!(classify("activate", &events, &keys).contains("context=0 (all=0,1)"));
        assert!(classify("XButton", &events, &keys).starts_with("event context=1"));
        assert_eq!(classify("Mouse1", &events, &keys), "key mouse 0x0");
        assert_eq!(classify("E", &events, &keys), "key keyboard 0x12");
        assert_eq!(classify("Prev Track", &events, &keys), "key keyboard 0x90");
        assert_eq!(classify("Left Thumbstick", &events, &keys), "nothing");
    }
    #[test]
    fn bracket_candidates() {
        assert_eq!(
            tags("é [Activate] [Activate] [] [outer [Mouse1] [unfinished"),
            ["Activate", "Activate", "Mouse1"]
        );
    }
}
