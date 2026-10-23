//! Optional native-save adjuncts. Every payload is validated before live state
//! changes. Absence replaces the destination state with the legacy default.
//! These policies do not describe Skyrim's .ess representation; see saves.md.
use super::codec::{Corrupt, Reader, Result, Writer};
use super::file::SaveFile;
use crate::engine::Engine;
use esp::FormId;
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct Continuity {
    pub cleared_locations: std::collections::HashSet<FormId>,
    pub location_keyword_data: HashMap<(FormId, FormId), f32>,
    pub crime_factions: HashMap<FormId, Option<FormId>>,
    pub harvested: HashMap<FormId, f64>,
}

fn optional<'a>(file: &'a SaveFile, tag: &[u8; 4]) -> Result<Option<Reader<'a>>> {
    let mut sections = file.sections.iter().filter(|(t, _)| t == tag);
    let first = sections.next();
    if sections.next().is_some() {
        return Err(Corrupt(format!(
            "duplicate {} section",
            String::from_utf8_lossy(tag)
        )));
    }
    Ok(first.map(|(_, data)| Reader::new(data)))
}
fn nonnull(id: FormId) -> Result<()> {
    if id.is_null() {
        return Err(Corrupt("null continuity form".into()));
    }
    Ok(())
}
fn time(value: f64) -> Result<()> {
    if !value.is_finite() || value < 0.0 {
        return Err(Corrupt("invalid continuity time".into()));
    }
    Ok(())
}
fn ordered<K: Ord + Copy>(previous: &mut Option<K>, key: K) -> Result<()> {
    if previous.is_some_and(|p| p >= key) {
        return Err(Corrupt("duplicate or unsorted continuity key".into()));
    }
    *previous = Some(key);
    Ok(())
}
fn sorted<K: Ord + Copy, V>(map: &HashMap<K, V>) -> Vec<(K, &V)> {
    let mut entries: Vec<_> = map.iter().map(|(&k, v)| (k, v)).collect();
    entries.sort_by_key(|(k, _)| *k);
    entries
}
impl Continuity {
    pub fn capture(e: &Engine) -> Self {
        Self {
            cleared_locations: e.cleared_locations.clone(),
            location_keyword_data: e.location_keyword_data.clone(),
            crime_factions: e.crime.crime_factions.clone(),
            harvested: e.harvests.harvested.clone(),
        }
    }
    pub fn write(&self, file: &mut SaveFile) {
        let mut w = Writer::default();
        w.len(self.crime_factions.len());
        for (actor, faction) in sorted(&self.crime_factions) {
            w.form(actor);
            w.opt_form(*faction);
        }
        file.add(b"CRFC", w.0);
        let mut w = Writer::default();
        w.len(self.location_keyword_data.len());
        for ((location, keyword), value) in sorted(&self.location_keyword_data) {
            w.form(location);
            w.form(keyword);
            w.f32(*value);
        }
        file.add(b"LKWD", w.0);
        let mut w = Writer::default();
        let mut ids: Vec<_> = self.cleared_locations.iter().copied().collect();
        ids.sort();
        w.len(ids.len());
        for id in ids {
            w.form(id);
        }
        file.add(b"CLRD", w.0);
        let mut w = Writer::default();
        w.len(self.harvested.len());
        for (plant, at) in sorted(&self.harvested) {
            w.form(plant);
            w.f64(*at);
        }
        file.add(b"HRVT", w.0);
    }
    pub fn read(file: &SaveFile) -> Result<Self> {
        let mut s = Self::default();
        if let Some(mut r) = optional(file, b"HRVT")? {
            let mut previous = None;
            for _ in 0..r.count(12)? {
                let plant = r.form()?;
                ordered(&mut previous, plant)?;
                s.harvested.insert(plant, r.f64()?);
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"CLRD")? {
            let mut previous = None;
            for _ in 0..r.count(4)? {
                let id = r.form()?;
                ordered(&mut previous, id)?;
                s.cleared_locations.insert(id);
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"LKWD")? {
            let mut previous = None;
            for _ in 0..r.count(12)? {
                let key = (r.form()?, r.form()?);
                ordered(&mut previous, key)?;
                s.location_keyword_data.insert(key, r.f32()?);
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"CRFC")? {
            let mut previous = None;
            for _ in 0..r.count(8)? {
                let actor = r.form()?;
                ordered(&mut previous, actor)?;
                s.crime_factions.insert(actor, r.opt_form()?);
            }
            r.finish()?;
        }
        s.validate()?;
        Ok(s)
    }
    pub fn validate(&self) -> Result<()> {
        for (&actor, &faction) in &self.crime_factions {
            nonnull(actor)?;
            if let Some(faction) = faction {
                nonnull(faction)?;
            }
        }
        for (&(location, keyword), &value) in &self.location_keyword_data {
            nonnull(location)?;
            nonnull(keyword)?;
            if !value.is_finite() {
                return Err(Corrupt("nonfinite location keyword data".into()));
            }
        }
        for &id in &self.cleared_locations {
            nonnull(id)?;
        }
        for (&plant, &at) in &self.harvested {
            nonnull(plant)?;
            time(at)?;
        }
        Ok(())
    }
    pub fn apply(self, e: &mut Engine) {
        e.crime.crime_factions = self.crime_factions;
        e.location_keyword_data = self.location_keyword_data;
        e.cleared_locations = self.cleared_locations;
        e.harvests.harvested = self.harvested;
    }
}
