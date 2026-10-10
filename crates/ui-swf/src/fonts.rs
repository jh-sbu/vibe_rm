//! Scaleform's font setup and text translation, which Ruffle doesn't do.

use ruffle_core::Player;
use ruffle_core::backend::ui::FontDefinition;
use ruffle_core::font::DefaultFont;

use crate::files::InterfaceFiles;

/// `interface/fontconfig.txt`: the font libraries (`fontlib "Interface\fonts_en.swf"`)
/// and the names menus ask for mapped to their fonts (`map "$EverywhereFont" =
/// "Futura CondensedLight" Normal`).
pub struct Fonts {
    /// The font libraries, decompressed.
    libraries: Vec<swf::SwfBuf>,
    /// (alias, font name, bold).
    maps: Vec<(String, String, bool)>,
}

impl Fonts {
    pub fn load(files: &InterfaceFiles) -> anyhow::Result<Self> {
        let config = files
            .get("fontconfig.txt")
            .ok_or_else(|| anyhow::anyhow!("no interface/fontconfig.txt"))?;
        let config = String::from_utf8_lossy(&config);
        let mut libraries = Vec::new();
        let mut maps = Vec::new();
        for line in config.lines() {
            let q: Vec<&str> = line.split('"').collect();
            if line.starts_with("fontlib") && q.len() >= 2 {
                let Some(data) = files.get(q[1]) else {
                    log::warn!("fontconfig.txt: no font library {}", q[1]);
                    continue;
                };
                libraries.push(swf::decompress_swf(&data[..])?);
            } else if line.starts_with("map") && q.len() >= 4 {
                maps.push((
                    q[1].to_string(),
                    q[3].to_string(),
                    line.trim_end().ends_with("Bold"),
                ));
            }
        }
        Ok(Fonts { libraries, maps })
    }

    /// Gives a player the libraries' fonts as device fonts, under their own names
    /// and the aliases mapped to them, and `$EverywhereFont` for Flash's default
    /// fonts (an empty text field's Times New Roman).
    pub fn register(&self, player: &mut Player) -> anyhow::Result<()> {
        let parsed: Vec<_> = self
            .libraries
            .iter()
            .map(swf::parse_swf)
            .collect::<Result<_, _>>()?;
        let fonts: Vec<&swf::Font> = parsed
            .iter()
            .flat_map(|s| s.tags.iter())
            .filter_map(|t| match t {
                swf::Tag::DefineFont2(f) if !f.glyphs.is_empty() => Some(&**f),
                _ => None,
            })
            .collect();
        for f in &fonts {
            player.register_device_font(FontDefinition::SwfTag((*f).clone(), swf::UTF_8));
        }
        for (alias, name, bold) in &self.maps {
            let named = |f: &&&swf::Font| f.name.to_str_lossy(swf::UTF_8) == name.as_str();
            let Some(f) = fonts
                .iter()
                .filter(named)
                .find(|f| f.flags.contains(swf::FontFlag::IS_BOLD) == *bold)
                .or_else(|| fonts.iter().find(named))
            else {
                log::debug!("fontconfig.txt: {alias}: no font {name:?}");
                continue;
            };
            let mut f = (*f).clone();
            f.name = swf::SwfStr::from_utf8_str(alias);
            // Asked for plain or bold, the alias is this face.
            for b in [false, true] {
                f.flags.set(swf::FontFlag::IS_BOLD, b);
                player.register_device_font(FontDefinition::SwfTag(f.clone(), swf::UTF_8));
            }
        }
        for f in [
            DefaultFont::Serif,
            DefaultFont::Sans,
            DefaultFont::Typewriter,
        ] {
            player.set_default_font(f, vec!["$EverywhereFont".into()]);
        }
        Ok(())
    }
}

/// `interface/translate_<language>.txt`: UTF-16 lines of `$KEY<tab>text`.
pub fn translations(files: &InterfaceFiles, language: &str) -> Vec<(String, String)> {
    let Some(bytes) = files.get(&format!("translate_{language}.txt")) else {
        log::warn!("no interface/translate_{language}.txt");
        return Vec::new();
    };
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let text = String::from_utf16_lossy(units.strip_prefix(&[0xFEFF]).unwrap_or(&units));
    text.lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(k, v)| (k.to_string(), v.trim_end_matches('\r').to_string()))
        .collect()
}
