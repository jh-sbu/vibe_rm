//! NAVI layout coverage and preferred-path table counts on game assets.
use anyhow::Result;
pub fn run(data: &std::path::Path) -> Result<()> {
    let names = esp::BASE_MASTERS
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let lo = esp::LoadOrder::load(data, &names)?;
    let index = esp::navi::Index::load(&lo);
    for (plugin, good, bad) in &index.coverage {
        println!("{plugin}: {good} NVMI, {bad} invalid");
    }
    let links: usize = index.infos.values().map(|i| i.links.len()).sum();
    let doors: usize = index.infos.values().map(|i| i.doors.len()).sum();
    println!(
        "{} merged meshes of {} NAVM; {links} links, {doors} doors, {} preferred paths",
        index.infos.len(),
        lo.ids_of_type(b"NAVM").len(),
        index.preferred_paths.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires installed vanilla Skyrim SE masters"]
    fn vanilla_incremental_navi_entries_parse_in_every_master() {
        let data = vfs::locate_data_dir().unwrap();
        let names = esp::BASE_MASTERS
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let lo = esp::LoadOrder::load(&data, &names).unwrap();
        let index = esp::navi::Index::load(&lo);
        assert!(index.coverage.iter().all(|(_, _, bad)| *bad == 0));
        assert!(index.infos.len() > 18000);
        assert!(index.preferred_paths.len() >= 500);
        assert!(index.infos.keys().any(|f| f.0 >> 24 == 0));
        assert!(index.infos.keys().any(|f| f.0 >> 24 == 4));
    }
}
