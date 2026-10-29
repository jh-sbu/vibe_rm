//! Independently readable journal metadata. Optional for pre-journal saves.
use super::codec::{Corrupt, Reader, Result, Writer};
use crate::engine::{Engine, PLAYER_REF};
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Metadata {
    pub character: u64,
    pub file_num: u32,
    pub name: String,
    pub race: String,
    pub level: u16,
    pub location: String,
    pub game_date: String,
    pub play_seconds: u64,
    pub written: u64,
    pub plugins_hash: u64,
    pub thumbnail: Option<Thumbnail>,
}
/// Stable, order-sensitive FNV-1a hash, with separators between plugin names.
pub fn plugin_hash(plugins: &[String]) -> u64 {
    plugins
        .iter()
        .flat_map(|p| p.bytes().chain([0]))
        .fold(0xcbf29ce484222325, |h, b| {
            (h ^ b as u64).wrapping_mul(0x100000001b3)
        })
}
pub fn new_character_id() -> u64 {
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    now.rotate_left(13)
        ^ ((std::process::id() as u64) << 32)
        ^ SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
impl Metadata {
    pub fn display_text(&self, name: &str) -> String {
        if name.starts_with("quicksave") {
            "[Quicksave]".into()
        } else if name.starts_with("autosave") {
            "[Autosave]".into()
        } else {
            format!("{} - {} - {}", self.name, self.location, self.game_date)
        }
    }

    pub fn capture(e: &Engine) -> Self {
        let plugins: Vec<_> =
            e.lo.plugins()
                .iter()
                .map(|p| p.plugin.name().to_string())
                .collect();
        Self {
            character: e.scripts.character_id,
            name: e
                .actor_name(PLAYER_REF)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "Prisoner".into()),
            race: e
                .npc_race(esp::FormId(7))
                .map(|r| e.form_name(r))
                .unwrap_or_else(|| "?".into()),
            level: e.skills.level + e.skills.pending as u16,
            location: e.location_name(),
            game_date: e.date_text(),
            play_seconds: e.scripts.real_time as u64,
            written: 0,
            plugins_hash: plugin_hash(&plugins),
            thumbnail: e.save_thumbnail.clone(),
            ..Default::default()
        }
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.u32(3);
        w.u64(self.character);
        w.u32(self.file_num);
        w.str(&self.name);
        w.str(&self.race);
        w.u16(self.level);
        w.str(&self.location);
        w.str(&self.game_date);
        w.u64(self.play_seconds);
        w.u64(self.written);
        w.u64(self.plugins_hash);
        w.0
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let mut r = Reader::new(bytes);
        let version = r.u32()?;
        if !matches!(version, 1..=3) {
            return Err(Corrupt("unknown META version".into()));
        }
        let mut result = Self {
            character: r.u64()?,
            file_num: r.u32()?,
            name: r.str()?,
            race: r.str()?,
            level: r.u16()?,
            location: r.str()?,
            game_date: r.str()?,
            play_seconds: r.u64()?,
            written: r.u64()?,
            plugins_hash: r.u64()?,
            thumbnail: None,
        };
        if version == 2 && r.bool()? {
            let width = r.u32()?;
            let height = r.u32()?;
            let len = r.count(1)?;
            if width == 0
                || height == 0
                || width > 1024
                || height > 1024
                || len != width as usize * height as usize * 4
            {
                return Err(Corrupt("invalid thumbnail dimensions".into()));
            }
            let mut rgba = Vec::with_capacity(len);
            for _ in 0..len {
                rgba.push(r.u8()?);
            }
            result.thumbnail = Some(Thumbnail {
                width,
                height,
                rgba,
            });
        }
        r.finish()?;
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thumbnails_fit_both_bounds_without_changing_aspect() {
        for (width, height, expected) in [
            (800, 600, (288, 216)),
            (1920, 1080, (384, 216)),
            (600, 800, (162, 216)),
            (1000, 400, (384, 154)),
            (4, 4, (4, 4)),
        ] {
            let image =
                Thumbnail::downscale(&vec![17; width * height * 4], width as u32, height as u32)
                    .unwrap();
            assert_eq!((image.width, image.height), expected);
            let png = image.encode_png().unwrap();
            assert_eq!(Thumbnail::decode_png(&png).unwrap(), image);
            if width > 4 {
                assert!(png.len() < image.rgba.len() / 10);
            }
        }
        assert!(Thumbnail::decode_png(b"invalid").is_err());
    }

    #[test]
    fn legacy_metadata_keeps_raw_previews_readable() {
        let image = Thumbnail::downscale(&[17; 64], 4, 4).unwrap();
        let mut bytes = Metadata::default().encode();
        bytes[..4].copy_from_slice(&2u32.to_le_bytes());
        let mut w = Writer(bytes);
        w.bool(true);
        w.u32(image.width);
        w.u32(image.height);
        w.len(image.rgba.len());
        w.0.extend_from_slice(&image.rgba);
        assert_eq!(Metadata::decode(&w.0).unwrap().thumbnail, Some(image));
        let mut v1 = Metadata::default().encode();
        v1[..4].copy_from_slice(&1u32.to_le_bytes());
        assert!(Metadata::decode(&v1).unwrap().thumbnail.is_none());
    }

    #[test]
    fn previews_are_separate_lazy_and_crc_checked() {
        let image = Thumbnail::downscale(&vec![17; 800 * 600 * 4], 800, 600).unwrap();
        let meta = Metadata {
            thumbnail: Some(image.clone()),
            ..Default::default()
        };
        let mut file = super::super::file::SaveFile::default();
        file.add(b"META", meta.encode());
        file.add(b"THMB", image.encode_png().unwrap());
        file.add(b"HEAD", vec![0; 200]);
        let bytes = file.to_bytes();
        let header_end = 28 + meta.encode().len();
        assert!(header_end < 256);
        let prefix = super::super::file::SaveFile::read_prefix(&bytes[..header_end]).unwrap();
        assert!(
            Metadata::decode(prefix.section(b"META").unwrap())
                .unwrap()
                .thumbnail
                .is_none()
        );
        let mut input = std::io::Cursor::new(&bytes);
        assert_eq!(read_thumbnail(&mut input).unwrap(), Some(image));
        assert_eq!(
            input.position() as usize,
            header_end + 12 + file.section(b"THMB").unwrap().len()
        );
        let mut damaged = bytes;
        damaged[header_end + 12] ^= 1;
        assert!(read_thumbnail(damaged.as_slice()).is_err());
        // Damage outside META neither increases listing I/O nor corrupts its row data.
        assert!(super::super::file::SaveFile::read_prefix(damaged.as_slice()).is_ok());
        file.sections.remove(1);
        assert!(
            read_thumbnail(file.to_bytes().as_slice())
                .unwrap()
                .is_none()
        );
        let oversized = Thumbnail {
            width: 1025,
            height: 1,
            rgba: vec![0; 1025 * 4],
        };
        assert!(Thumbnail::decode_png(&oversized.encode_png().unwrap()).is_err());
    }

    #[test]
    fn metadata_round_trip_and_version() {
        let m = Metadata {
            character: 77,
            file_num: 7,
            name: "Prisoner".into(),
            race: "Nord".into(),
            level: 9,
            location: "Riverwood".into(),
            game_date: "Sundas".into(),
            play_seconds: 123,
            written: 456,
            plugins_hash: plugin_hash(&["Skyrim.esm".into()]),
            thumbnail: Some(Thumbnail::downscale(&[17; 64], 4, 4).unwrap()),
        };
        let mut header = m.clone();
        header.thumbnail = None;
        assert_eq!(Metadata::decode(&m.encode()).unwrap(), header);
        let thumbnail = m.thumbnail.as_ref().unwrap();
        let png = thumbnail.encode_png().unwrap();
        assert_eq!(Thumbnail::decode_png(&png).unwrap(), *thumbnail);
        let mut b = m.encode();
        b[0] = 99;
        assert!(Metadata::decode(&b).is_err());
    }
}
#[cfg(test)]
mod listing_tests {
    use super::*;
    use crate::save::{file::SaveFile, save_list_in};
    #[test]
    fn header_listing_supports_legacy_damage_and_plugin_changes() {
        let dir = std::env::temp_dir().join(format!("vrm-meta-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let plugins = vec!["Skyrim.esm".to_string()];
        let mut old = SaveFile::default();
        let mut w = Writer::default();
        w.len(1);
        w.str(&plugins[0]);
        w.bool(false);
        old.add(b"HEAD", w.0);
        std::fs::write(dir.join("legacy.vrs"), old.to_bytes()).unwrap();
        let meta = Metadata {
            plugins_hash: plugin_hash(&plugins),
            thumbnail: None,
            ..Default::default()
        };
        let mut f = SaveFile::default();
        f.add(b"META", meta.encode());
        f.add(b"BODY", vec![0; 200]);
        let bytes = f.to_bytes();
        // The prefix reader does not even require the later snapshot bytes.
        assert!(SaveFile::read_prefix(&bytes[..28 + meta.encode().len()]).is_ok());
        std::fs::write(dir.join("new.vrs"), &bytes).unwrap();
        let mut bad = bytes;
        bad[30] ^= 1;
        std::fs::write(dir.join("bad.vrs"), bad).unwrap();
        std::fs::write(dir.join("Save 7 - old.vrs"), b"damaged").unwrap();
        assert_eq!(crate::save::next_file_num_in(&dir).unwrap(), 8);
        assert_eq!(crate::save::next_autosave_name_in(&dir), "autosave1");
        std::fs::write(dir.join("autosave1.vrs"), b"slot").unwrap();
        assert_eq!(crate::save::next_autosave_name_in(&dir), "autosave2");
        std::fs::remove_file(dir.join("autosave1.vrs")).unwrap();
        let rows = save_list_in(&dir, &plugins);
        assert_eq!(rows.len(), 4);
        assert!(rows.iter().find(|s| s.name == "bad").unwrap().corrupt);
        assert!(!rows.iter().find(|s| s.name == "legacy").unwrap().obsolete);
        assert!(
            save_list_in(&dir, &[])
                .iter()
                .filter(|s| !s.corrupt)
                .all(|s| s.obsolete)
        );
        assert!(crate::save::delete_save_in(&dir, "../outside").is_err());
        crate::save::delete_save_in(&dir, "legacy").unwrap();
        assert!(!dir.join("legacy.vrs").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

/// In-memory RGBA preview. New saves store it as PNG in a separate THMB section.
#[derive(Clone, Debug, PartialEq)]
pub struct Thumbnail {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
impl Thumbnail {
    pub fn encode_png(&self) -> std::result::Result<Vec<u8>, png::EncodingError> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, self.width, self.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.write_header()?.write_image_data(&self.rgba)?;
        }
        Ok(bytes)
    }

    pub fn decode_png(bytes: &[u8]) -> Result<Self> {
        let bad = |e: png::DecodingError| Corrupt(format!("invalid thumbnail PNG: {e}"));
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        decoder.set_limits(png::Limits {
            bytes: 8 * 1024 * 1024,
        });
        let mut reader = decoder.read_info().map_err(bad)?;
        let info = reader.info();
        if info.width == 0
            || info.height == 0
            || info.width > 1024
            || info.height > 1024
            || info.color_type != png::ColorType::Rgba
            || info.bit_depth != png::BitDepth::Eight
        {
            return Err(Corrupt("invalid thumbnail PNG dimensions or format".into()));
        }
        let mut rgba = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut rgba).map_err(bad)?;
        rgba.truncate(info.buffer_size());
        Ok(Self {
            width: info.width,
            height: info.height,
            rgba,
        })
    }

    pub fn downscale(rgba: &[u8], width: u32, height: u32) -> Option<Self> {
        if width == 0 || height == 0 || rgba.len() != width as usize * height as usize * 4 {
            return None;
        }
        let scale = (384. / width as f64).min(216. / height as f64).min(1.);
        let w = (width as f64 * scale).round().max(1.) as u32;
        let h = (height as f64 * scale).round().max(1.) as u32;
        let mut pixels = Vec::with_capacity(w as usize * h as usize * 4);
        for y in 0..h {
            for x in 0..w {
                let at = ((y as u64 * height as u64 / h as u64) * width as u64
                    + x as u64 * width as u64 / w as u64) as usize
                    * 4;
                pixels.extend_from_slice(&rgba[at..at + 4]);
            }
        }
        Some(Self {
            width: w,
            height: h,
            rgba: pixels,
        })
    }
}
/// New files place THMB immediately after META. Older META v2 embeds RGBA.
/// Listing reads only META; only the selected row reads and decodes its PNG.
pub fn read_thumbnail(mut input: impl std::io::Read) -> anyhow::Result<Option<Thumbnail>> {
    let prefix = super::file::SaveFile::read_prefix(&mut input)?;
    let Some(bytes) = prefix.section(b"META") else {
        return Ok(None);
    };
    let meta = Metadata::decode(bytes)?;
    if u32::from_le_bytes(bytes[..4].try_into()?) < 3 {
        return Ok(meta.thumbnail);
    }
    let mut section = [0; 12];
    input.read_exact(&mut section)?;
    if &section[..4] != b"THMB" {
        return Ok(None);
    }
    let len = u32::from_le_bytes(section[4..8].try_into()?) as usize;
    anyhow::ensure!(len <= 8 * 1024 * 1024, "thumbnail too large");
    let mut png = vec![0; len];
    input.read_exact(&mut png)?;
    anyhow::ensure!(
        super::file::crc32(&png) == u32::from_le_bytes(section[8..12].try_into()?),
        "damaged thumbnail"
    );
    Ok(Some(Thumbnail::decode_png(&png)?))
}

impl Engine {
    /// Render the world alone. Readback occurs only when a save or its live
    /// preview needs it, rather than on every journal opening/gameplay frame.
    pub fn capture_save_thumbnail(&mut self) {
        let camera = self.view_camera();
        let pixels = self
            .renderer
            .render_to_image(&self.scene, &camera, |_, _| {});
        self.save_thumbnail =
            Thumbnail::downscale(&pixels, self.renderer.width, self.renderer.height);
    }
}

/// UTC, bounded to Gregorian years the journal can display.
pub fn date_string(seconds: u64) -> String {
    if seconds > 253402300799 {
        return "?".into();
    }
    let mut days = seconds / 86400;
    let mut year = 1970u32;
    let leap = |y: u32| y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    while days >= if leap(year) { 366 } else { 365 } {
        days -= if leap(year) { 366 } else { 365 };
        year += 1;
    }
    let months = [
        31,
        if leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 0;
    while days >= months[month] {
        days -= months[month];
        month += 1;
    }
    format!(
        "{year:04}-{:02}-{:02} {:02}:{:02} UTC",
        month + 1,
        days + 1,
        seconds / 3600 % 24,
        seconds / 60 % 60
    )
}

#[cfg(test)]
mod date_tests {
    use super::*;
    #[test]
    fn utc_dates_are_readable_and_bounded() {
        assert_eq!(date_string(0), "1970-01-01 00:00 UTC");
        assert_eq!(date_string(68169600), "1972-02-29 00:00 UTC");
        assert_eq!(date_string(u64::MAX), "?");
    }
}
