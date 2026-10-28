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

pub fn spaces(data: &str) -> Result<()> {
    let data = std::path::Path::new(data);
    let lo = LoadOrder::load(data, &LoadOrder::default_plugin_list(data, None))?;
    let mut assigned = 0;
    let mut interior = 0;
    for &id in lo.ids_of_type(b"CELL") {
        let Some(r) = lo.get(id) else { continue };
        if r.get(b"DATA")
            .is_some_and(|d| d.first().is_some_and(|b| b & 1 != 0))
        {
            interior += 1;
        }
        if r.get(b"XCAS").is_some() {
            assigned += 1;
        }
    }
    let mut placed = 0;
    let mut rotated = 0;
    for &id in lo.ids_of_type(b"REFR") {
        let Some(r) = lo.get(id) else { continue };
        let Some(d) = r.get(b"NAME").filter(|d| d.len() >= 4) else {
            continue;
        };
        let base = r.fid(esp::FormId(u32::from_le_bytes(d[..4].try_into().unwrap())));
        if lo.tag_of(base).is_none_or(|t| t.0 != *b"ASPC") {
            continue;
        }
        placed += 1;
        if r.get(b"DATA").is_some_and(|d| {
            d.get(12..24).is_some_and(|d| {
                d.chunks_exact(4)
                    .any(|b| f32::from_le_bytes(b.try_into().unwrap()) != 0.0)
            })
        }) {
            rotated += 1;
        }
        println!(
            "placed {id} base={base} cell={:?} primitive={:?} scale={:?}",
            lo.cell_of_ref(id),
            r.get(b"XPRM").map(|d| d.len()),
            r.get(b"XSCL").map(|d| d.len())
        );
    }
    println!(
        "ASPC={} interior_cells={interior} XCAS_cells={assigned} placed={placed} rotated={rotated}",
        lo.ids_of_type(b"ASPC").len()
    );
    let mut onam = std::collections::BTreeMap::<u8, usize>::new();
    for &id in lo.ids_of_type(b"SOPM") {
        if let Some(r) = lo.get(id) {
            for &v in r.get(b"ONAM").unwrap_or_default() {
                *onam.entry(v).or_default() += 1;
            }
        }
    }
    println!("SOPM ONAM values: {onam:?}");
    for &id in lo.ids_of_type(b"REGN") {
        let Some(r) = lo.get(id) else { continue };
        let mut sound = false;
        for sr in r.subrecords() {
            if sr.tag.0 == *b"RDAT" {
                sound = sr.data.len() >= 4 && sr.u32(0) == 7;
            }
            if sr.tag.0 != *b"RDSA" || !sound {
                continue;
            }
            for e in sr.data.chunks_exact(12) {
                let id = r.fid(esp::FormId(u32::from_le_bytes(e[..4].try_into().unwrap())));
                let d = lo.get(id);
                let looping = d
                    .as_ref()
                    .and_then(|r| r.get(b"LNAM"))
                    .is_some_and(|d| d.len() >= 2 && d[1] & 0x38 != 0);
                println!(
                    "region {} sound={id} {} loop={looping} flags={} chance={}",
                    r.editor_id().unwrap_or_default(),
                    d.as_ref().and_then(|r| r.editor_id()).unwrap_or_default(),
                    u32::from_le_bytes(e[4..8].try_into().unwrap()),
                    f32::from_le_bytes(e[8..12].try_into().unwrap())
                );
            }
        }
    }
    Ok(())
}
