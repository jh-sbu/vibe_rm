//! Read-only surveys of audio game assets. No engine code or data is examined.
use anyhow::Result;
use esp::LoadOrder;

pub const DEFAULT_KEYS: &[&[u8; 4]] = &[
    b"DDSC", b"NDSC", b"SFDC", b"PIMC", b"PDMC", b"PDLC", b"MDSC", b"SMSC", b"SSSC", b"TSSC",
    b"DOP2", b"DOP3", b"POPM", b"P3OM", b"IOPM", b"RVBT", b"UWLS", b"BTMS", b"DTMS", b"SCMS",
    b"LUMS", b"DCMS", b"DFMS",
];
pub fn defaults(data: &str) -> Result<()> {
    let data = std::path::Path::new(data);
    let lo = LoadOrder::load(data, &LoadOrder::default_plugin_list(data, None))?;
    let objects = lo.default_objects();
    for key in DEFAULT_KEYS {
        let Some(&id) = objects.get(*key) else {
            continue;
        };
        let target = lo.get(id);
        println!(
            "{} {id} {} {}",
            String::from_utf8_lossy(&key[..]),
            target
                .as_ref()
                .map(|r| r.tag().to_string())
                .unwrap_or_default(),
            target
                .as_ref()
                .and_then(|r| r.editor_id())
                .unwrap_or_default()
        );
    }
    Ok(())
}

pub fn music(data: &str) -> Result<()> {
    let data = std::path::Path::new(data);
    let lo = LoadOrder::load(data, &LoadOrder::default_plugin_list(data, None))?;
    let mut kinds = std::collections::BTreeMap::<u32, usize>::new();
    for &id in lo.ids_of_type(b"MUST") {
        let Some(r) = lo.get(id) else { continue };
        let kind = r
            .get(b"CNAM")
            .filter(|d| d.len() >= 4)
            .map_or(0, |d| u32::from_le_bytes(d[..4].try_into().unwrap()));
        *kinds.entry(kind).or_default() += 1;
        let scalar = |tag| {
            r.get(tag)
                .filter(|d| d.len() >= 4)
                .map(|d| f32::from_le_bytes(d[..4].try_into().unwrap()))
        };
        let cues: Vec<f32> = r
            .get(b"FNAM")
            .unwrap_or_default()
            .chunks_exact(4)
            .map(|d| f32::from_le_bytes(d.try_into().unwrap()))
            .collect();
        let loops: Vec<u32> = r
            .get(b"LNAM")
            .unwrap_or_default()
            .chunks_exact(4)
            .map(|d| u32::from_le_bytes(d.try_into().unwrap()))
            .collect();
        let palette: Vec<u32> = r
            .get(b"SNAM")
            .unwrap_or_default()
            .chunks_exact(4)
            .map(|d| u32::from_le_bytes(d.try_into().unwrap()))
            .collect();
        if kind != 0x6ED7E048 || !loops.is_empty() || !cues.is_empty() || r.get(b"BNAM").is_some() {
            println!(
                "{id} {} kind={kind:08X} duration={:?} fade={:?} cues={cues:?} loops={loops:?} palette={palette:X?} btrack={}",
                r.editor_id().unwrap_or_default(),
                scalar(b"FLTV"),
                scalar(b"DNAM"),
                r.get(b"BNAM").is_some()
            );
        }
    }
    println!("MUST kinds: {kinds:X?}");
    Ok(())
}
