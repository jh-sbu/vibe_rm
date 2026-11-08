//! The player's appearance as engine state (`plans/character-creation.md`,
//! step 5): race, sex, weight, head parts, face sliders and presets, texture
//! set, hair colour, skin tone and tint layers. Until something changes it
//! (the race menu, `SetRace`), the player's NPC record gives it. Saved as
//! `PAPP`.

use esp::{FormId, LoadOrder};

use crate::engine::Engine;
use crate::save::codec::{Reader, Writer};

/// What the player looks like: a FaceGen face (`facegen::Face`), the skin
/// tone (NPC_ `QNAM`, RGB 0..1) and the tint layers drawn over it.
#[derive(Debug, Clone, PartialEq)]
pub struct Appearance {
    pub face: facegen::Face,
    pub skin: [f32; 3],
    pub tints: Vec<facegen::TintLayer>,
}

impl Appearance {
    /// The appearance an NPC_ record gives.
    pub fn of_npc(lo: &LoadOrder, npc: FormId) -> Option<Appearance> {
        Some(Appearance {
            face: facegen::Face::of_npc(lo, npc)?,
            skin: facegen::npc_skin_tone(lo, npc).unwrap_or([1.0; 3]),
            tints: facegen::npc_tint_layers(lo, npc),
        })
    }

    pub fn write(&self, w: &mut Writer) {
        let f = &self.face;
        w.form(f.race);
        w.bool(f.female);
        w.f32(f.weight);
        w.len(f.parts.len());
        for &p in &f.parts {
            w.form(p);
        }
        for &s in &f.sliders {
            w.f32(s);
        }
        w.i32(f.nose);
        w.i32(f.eyes);
        w.i32(f.mouth);
        w.form(f.texture_set.unwrap_or_default());
        w.form(f.hair_color.unwrap_or_default());
        for &c in &self.skin {
            w.f32(c);
        }
        w.len(self.tints.len());
        for t in &self.tints {
            w.u16(t.index);
            for &c in &t.color {
                w.u8(c);
            }
            w.f32(t.value);
            w.u16(t.preset as u16);
        }
    }

    pub fn read(r: &mut Reader<'_>) -> crate::save::codec::Result<Appearance> {
        let race = r.form()?;
        let female = r.bool()?;
        let weight = r.f32()?;
        let mut parts = Vec::new();
        for _ in 0..r.count(4)? {
            parts.push(r.form()?);
        }
        let mut sliders = [0.0; 19];
        for s in &mut sliders {
            *s = r.f32()?;
        }
        let (nose, eyes, mouth) = (r.i32()?, r.i32()?, r.i32()?);
        let some = |f: FormId| (!f.is_null()).then_some(f);
        let texture_set = some(r.form()?);
        let hair_color = some(r.form()?);
        let skin = [r.f32()?, r.f32()?, r.f32()?];
        let mut tints = Vec::new();
        for _ in 0..r.count(12)? {
            let index = r.u16()?;
            let color = [r.u8()?, r.u8()?, r.u8()?, r.u8()?];
            let value = r.f32()?;
            let preset = r.u16()? as i16;
            tints.push(facegen::TintLayer {
                index,
                color,
                value,
                preset,
            });
        }
        Ok(Appearance {
            face: facegen::Face {
                race,
                female,
                parts,
                sliders,
                nose,
                eyes,
                mouth,
                weight,
                texture_set,
                hair_color,
            },
            skin,
            tints,
        })
    }
}

impl Engine {
    /// The player's appearance: as last set, else their NPC record's.
    pub fn player_appearance(&self) -> Option<Appearance> {
        self.player_appearance
            .clone()
            .or_else(|| Appearance::of_npc(&self.lo, crate::player_body::PLAYER_NPC))
    }

    /// Change the player's appearance and rebuild their body with it.
    pub fn set_player_appearance(&mut self, a: Appearance) {
        if self.player_appearance.as_ref() == Some(&a) {
            return;
        }
        self.player_appearance = Some(a);
        self.build_player_body();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appearances_round_trip_through_their_save_payload() {
        let mut sliders = [0.0; 19];
        sliders[3] = -0.5;
        let a = Appearance {
            face: facegen::Face {
                race: FormId(0x13746),
                female: true,
                parts: vec![FormId(0x51407), FormId(0xE7AEE)],
                sliders,
                nose: 5,
                eyes: -1,
                mouth: 10,
                weight: 0.25,
                texture_set: Some(FormId(0x3B521)),
                hair_color: None,
            },
            skin: [0.7, 0.6, 0.5],
            tints: vec![facegen::TintLayer {
                index: 8,
                color: [172, 0, 0, 0],
                value: 0.19,
                preset: -1,
            }],
        };
        let mut w = Writer::default();
        a.write(&mut w);
        let mut r = Reader::new(&w.0);
        assert_eq!(Appearance::read(&mut r).unwrap(), a);
        r.finish().unwrap();
    }
}
