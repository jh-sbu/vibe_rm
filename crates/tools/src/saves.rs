//! Reading Skyrim SE's saves (`ess`): what `save-info` prints, and finding
//! the saves the game made.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// The game's Saves directory: `$SKYRIM_SAVES`, then Proton's prefix under
/// the common Steam paths.
pub fn locate_saves_dir() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("SKYRIM_SAVES") {
        return Some(PathBuf::from(p)).filter(|p| p.is_dir());
    }
    let home = std::env::var("HOME").ok()?;
    let saves = "steamapps/compatdata/489830/pfx/drive_c/users/steamuser/Documents/My Games/Skyrim Special Edition/Saves";
    [
        ".local/share/Steam",
        ".steam/steam",
        ".var/app/com.valvesoftware.Steam/.local/share/Steam",
    ]
    .iter()
    .map(|root| Path::new(&home).join(root).join(saves))
    .find(|p| p.is_dir())
}

/// The `.ess` files in a directory, newest first; a file is itself.
pub fn saves_in(path: &Path) -> Result<Vec<PathBuf>> {
    if !path.is_dir() {
        return Ok(vec![path.to_owned()]);
    }
    let mut saves = Vec::new();
    for e in std::fs::read_dir(path).with_context(|| path.display().to_string())? {
        let p = e?.path();
        if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("ess")) {
            saves.push((std::fs::metadata(&p)?.modified()?, p));
        }
    }
    saves.sort_by_key(|s| std::cmp::Reverse(s.0));
    Ok(saves.into_iter().map(|(_, p)| p).collect())
}

/// A save's summary: who, where, when, its plugins and what it holds.
pub fn info(save: &ess::Save) -> Result<String> {
    let mut o = String::new();
    let h = &save.header;
    writeln!(
        o,
        "save {} (version {}, form version {}): {}, level {} {} {}, {:.0}/{:.0} xp",
        h.save_number,
        h.version,
        save.form_version,
        h.player_name,
        h.player_level,
        if h.player_sex == 0 { "male" } else { "female" },
        h.player_race,
        h.player_cur_exp,
        h.player_lvl_up_exp,
    )?;
    writeln!(
        o,
        "in {}, play time {}, saved at unix time {}",
        h.player_location,
        h.game_date,
        h.unix_time()
    )?;
    if let Some(ess::PlayerLocation {
        world,
        grid,
        position: pos,
    }) = save.player_location()?
    {
        let world = world.map_or("none".into(), |w| format!("{w:08X}"));
        writeln!(
            o,
            "worldspace {world}, cell {grid:?}, position ({:.0}, {:.0}, {:.0})",
            pos[0], pos[1], pos[2]
        )?;
    }
    let g = save.global_variables()?;
    if let (Some(days), Some(hour), Some(day), Some(month), Some(year)) = (
        g.get(&0x39),
        g.get(&0x38),
        g.get(&0x37),
        g.get(&0x36),
        g.get(&0x35),
    ) {
        writeln!(
            o,
            "game time: day {day} of month {month} (from 0), year {year}, hour {hour:.2}; \
             {days:.3} days passed, timescale {}",
            g.get(&0x3A).copied().unwrap_or(f32::NAN)
        )?;
    }
    writeln!(o, "plugins: {}", save.plugins.join(", "))?;
    writeln!(o, "light plugins: {}", save.light_plugins.join(", "))?;
    let stats: Vec<String> = save
        .misc_stats()?
        .into_iter()
        .filter(|s| s.2 != 0)
        .map(|(name, _, v)| format!("{name} {v}"))
        .collect();
    writeln!(o, "stats: {}", stats.join(", "))?;
    writeln!(o, "global data:")?;
    for (kind, d) in &save.global_data {
        let name = ess::global::name(*kind).unwrap_or("?");
        writeln!(o, "  {kind:5} {name:24} {:9} bytes", d.len())?;
    }
    let mut kinds = BTreeMap::new();
    for f in &save.change_forms {
        let name = ess::CHANGE_FORM_TYPES
            .get(f.kind as usize)
            .copied()
            .unwrap_or("?");
        *kinds.entry(name).or_insert(0) += 1;
    }
    let mut kinds: Vec<_> = kinds.into_iter().collect();
    kinds.sort_by_key(|k| std::cmp::Reverse(k.1));
    let kinds: Vec<String> = kinds.iter().map(|(k, n)| format!("{k} {n}")).collect();
    writeln!(
        o,
        "{} change forms: {}",
        save.change_forms.len(),
        kinds.join(", ")
    )?;
    let p = save.papyrus()?;
    writeln!(
        o,
        "papyrus: {} script types, {} instances, {} arrays",
        p.scripts.len(),
        p.instances.len(),
        p.arrays.len()
    )?;
    Ok(o)
}
