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
