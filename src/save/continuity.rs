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
    pub random_streams: Option<(u64, u64)>,
    pub activations: Vec<(f64, FormId, FormId)>,
    pub jail_days: i32,
    pub sentence: (Option<crate::arrest::Sentence>, Option<FormId>),
    pub resistance: (std::collections::HashSet<FormId>, bool, f32),
    pub pursuits: HashMap<FormId, crate::arrest::Pursuit>,
    pub home_warnings: (HashMap<FormId, FormId>, f64),
    pub private_homes: HashMap<FormId, FormId>,
    pub trespass: Option<crate::crime::Trespass>,
    pub cleared_locations: std::collections::HashSet<FormId>,
    pub location_keyword_data: HashMap<(FormId, FormId), f32>,
    pub crime_factions: HashMap<FormId, Option<FormId>>,
    pub pickpocket_caught: std::collections::HashSet<FormId>,
    pub assaulted: std::collections::HashSet<FormId>,
    pub player_assailants: std::collections::HashSet<FormId>,
    pub unreported: Vec<crate::crime::Unreported>,
    pub last_crime: HashMap<FormId, crate::crime::CrimeType>,
    pub controls: u16,
    pub cart_mode: bool,
    pub voice_recovery: (f32, f32),
    pub stamina_wait: f32,
    pub magicka_wait: f32,
    pub journal_memory: Option<(bool, u64, u64)>,
    pub magic_usage: crate::stats::SavedMagicUsage,
    pub camera: Option<(bool, f32)>,
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
/// Per-quest sequence values can come from older saves or test setups whose
/// allocator was not retained. Capture and restore use the same canonical floor.
fn sequence_floors(e: &Engine) -> (u64, u64) {
    (
        e.scripts
            .quests
            .values()
            .map(|q| q.journal_seq)
            .max()
            .unwrap_or(0),
        e.scripts
            .quests
            .values()
            .flat_map(|q| q.filled_at.values())
            .copied()
            .max()
            .unwrap_or(0),
    )
}
impl Continuity {
    pub fn capture(e: &Engine) -> Self {
        let (journal_floor, fill_floor) = sequence_floors(e);
        Self {
            random_streams: Some((e.rng, e.cond_rng.get())),
            activations: e.activation.pending.clone(),
            jail_days: e.crime.arrests.days_in_jail,
            sentence: (e.crime.arrests.jailed, e.crime.arrests.pending),
            resistance: (
                e.crime.arrests.resisting.clone(),
                e.crime.arrests.yield_attempted,
                e.crime.arrests.sheathed_for.max(0.0),
            ),
            pursuits: e.crime.arrests.alarmed.clone(),
            home_warnings: (
                e.crime.warned_homes.clone(),
                e.crime.time_to_go_next.max(0.0),
            ),
            private_homes: e.crime.private_homes.clone(),
            trespass: e.crime.trespass,
            cleared_locations: e.cleared_locations.clone(),
            location_keyword_data: e.location_keyword_data.clone(),
            crime_factions: e.crime.crime_factions.clone(),
            pickpocket_caught: e.crime.pickpocket_caught.clone(),
            assaulted: e.crime.assaulted.clone(),
            player_assailants: e.crime.player_assailants.clone(),
            unreported: e.crime.unreported.clone(),
            last_crime: e.crime.last_crime.clone(),
            controls: e.disabled_controls.saved_bits(),
            cart_mode: e.hud_cart_mode,
            voice_recovery: (e.shouts.recovery, e.shouts.recovery_total),
            stamina_wait: e.player_stamina_wait.max(0.0),
            magicka_wait: e.casting.saved_regen_wait(),
            journal_memory: Some((
                e.scripts.misc_objectives_hidden,
                e.scripts.journal_seq.max(journal_floor),
                e.scripts.fill_seq.max(fill_floor),
            )),
            magic_usage: e.scripts.magic_usage.saved(),
            camera: Some((e.third_person, e.third_person_distance)),
            harvested: e.harvests.harvested.clone(),
        }
    }
    pub fn write(&self, file: &mut SaveFile) {
        if let Some((rng, cond)) = self.random_streams {
            let mut w = Writer::default();
            w.u64(rng);
            w.u64(cond);
            file.add(b"RNGS", w.0);
        }
        let mut w = Writer::default();
        w.len(self.activations.len());
        for &(at, child, parent) in &self.activations {
            w.f64(at);
            w.form(child);
            w.form(parent);
        }
        file.add(b"ACTQ", w.0);
        let mut w = Writer::default();
        w.i32(self.jail_days);
        file.add(b"JDYS", w.0);
        let mut w = Writer::default();
        w.opt_form(self.sentence.1);
        w.bool(self.sentence.0.is_some());
        if let Some(s) = self.sentence.0 {
            w.form(s.faction);
            w.i32(s.days);
            w.form(s.inside);
            for value in [
                s.owed.violent,
                s.owed.nonviolent,
                s.owed.infamy_violent,
                s.owed.infamy_nonviolent,
            ] {
                w.i32(value);
            }
        }
        file.add(b"JSEN", w.0);
        let mut w = Writer::default();
        w.bool(self.resistance.1);
        w.f32(self.resistance.2);
        let mut ids: Vec<_> = self.resistance.0.iter().copied().collect();
        ids.sort();
        w.len(ids.len());
        for id in ids {
            w.form(id);
        }
        file.add(b"ARRY", w.0);
        let mut w = Writer::default();
        w.len(self.pursuits.len());
        for (guard, p) in sorted(&self.pursuits) {
            w.form(guard);
            w.form(p.faction);
            w.f32(p.bark_in.max(0.0));
        }
        file.add(b"GPRS", w.0);
        let mut w = Writer::default();
        w.f64(self.home_warnings.1);
        w.len(self.home_warnings.0.len());
        for (actor, cell) in sorted(&self.home_warnings.0) {
            w.form(actor);
            w.form(*cell);
        }
        file.add(b"HWAR", w.0);
        let mut w = Writer::default();
        w.len(self.private_homes.len());
        for (actor, cell) in sorted(&self.private_homes) {
            w.form(actor);
            w.form(*cell);
        }
        file.add(b"PHOM", w.0);
        let mut w = Writer::default();
        w.bool(self.trespass.is_some());
        if let Some(t) = self.trespass {
            w.form(t.cell);
            w.i32(t.level);
            w.f64(t.next);
        }
        file.add(b"TRSP", w.0);
        if let Some((third, distance)) = self.camera {
            let mut w = Writer::default();
            w.bool(third);
            w.f32(distance);
            file.add(b"CAMR", w.0);
        }
        let mut w = Writer::default();
        for history in [
            &self.magic_usage.spells,
            &self.magic_usage.schools,
            &self.magic_usage.shouts,
        ] {
            w.len(history.counts.len());
            for &(id, count) in &history.counts {
                w.u32(id);
                w.u64(count);
            }
            w.u32(history.favorite.unwrap_or(0));
        }
        file.add(b"MUSE", w.0);
        if let Some((hidden, journal, fills)) = self.journal_memory {
            let mut w = Writer::default();
            w.bool(hidden);
            w.u64(journal);
            w.u64(fills);
            file.add(b"JMEM", w.0);
        }
        let mut w = Writer::default();
        w.f32(self.magicka_wait);
        file.add(b"MREG", w.0);
        let mut w = Writer::default();
        w.f32(self.stamina_wait);
        file.add(b"SREG", w.0);
        let mut w = Writer::default();
        w.f32(self.voice_recovery.0);
        w.f32(self.voice_recovery.1);
        file.add(b"VREC", w.0);
        let mut w = Writer::default();
        w.u16(self.controls);
        w.bool(self.cart_mode);
        file.add(b"CTRL", w.0);
        let mut w = Writer::default();
        w.len(self.last_crime.len());
        for (faction, kind) in sorted(&self.last_crime) {
            w.form(faction);
            w.u8(*kind as u8);
        }
        file.add(b"LCRM", w.0);
        let mut w = Writer::default();
        w.len(self.unreported.len());
        for crime in &self.unreported {
            w.form(crime.faction);
            w.i32(crime.gold);
            w.bool(crime.violent);
            w.len(crime.witnesses.len());
            for &witness in &crime.witnesses {
                w.form(witness);
            }
        }
        file.add(b"UWIT", w.0);
        let mut w = Writer::default();
        for set in [&self.assaulted, &self.player_assailants] {
            let mut ids: Vec<_> = set.iter().copied().collect();
            ids.sort();
            w.len(ids.len());
            for id in ids {
                w.form(id);
            }
        }
        file.add(b"ASLT", w.0);
        let mut w = Writer::default();
        let mut ids: Vec<_> = self.pickpocket_caught.iter().copied().collect();
        ids.sort();
        w.len(ids.len());
        for id in ids {
            w.form(id);
        }
        file.add(b"PKCT", w.0);
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
        if let Some(mut r) = optional(file, b"RNGS")? {
            s.random_streams = Some((r.u64()?, r.u64()?));
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"ACTQ")? {
            for _ in 0..r.count(16)? {
                s.activations.push((r.f64()?, r.form()?, r.form()?));
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"JDYS")? {
            s.jail_days = r.i32()?;
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"JSEN")? {
            s.sentence.1 = r.opt_form()?;
            if r.bool()? {
                s.sentence.0 = Some(crate::arrest::Sentence {
                    faction: r.form()?,
                    days: r.i32()?,
                    inside: r.form()?,
                    owed: crate::crime::Bounty {
                        violent: r.i32()?,
                        nonviolent: r.i32()?,
                        infamy_violent: r.i32()?,
                        infamy_nonviolent: r.i32()?,
                    },
                });
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"ARRY")? {
            s.resistance.1 = r.bool()?;
            s.resistance.2 = r.f32()?;
            let mut previous = None;
            for _ in 0..r.count(4)? {
                let id = r.form()?;
                ordered(&mut previous, id)?;
                s.resistance.0.insert(id);
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"GPRS")? {
            let mut previous = None;
            for _ in 0..r.count(12)? {
                let guard = r.form()?;
                ordered(&mut previous, guard)?;
                s.pursuits.insert(
                    guard,
                    crate::arrest::Pursuit {
                        faction: r.form()?,
                        bark_in: r.f32()?,
                    },
                );
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"HWAR")? {
            s.home_warnings.1 = r.f64()?;
            let mut previous = None;
            for _ in 0..r.count(8)? {
                let actor = r.form()?;
                ordered(&mut previous, actor)?;
                s.home_warnings.0.insert(actor, r.form()?);
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"PHOM")? {
            let mut previous = None;
            for _ in 0..r.count(8)? {
                let actor = r.form()?;
                ordered(&mut previous, actor)?;
                s.private_homes.insert(actor, r.form()?);
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"TRSP")? {
            if r.bool()? {
                s.trespass = Some(crate::crime::Trespass {
                    cell: r.form()?,
                    level: r.i32()?,
                    next: r.f64()?,
                });
            }
            r.finish()?;
        }
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
        if let Some(mut r) = optional(file, b"PKCT")? {
            let mut previous = None;
            for _ in 0..r.count(4)? {
                let id = r.form()?;
                ordered(&mut previous, id)?;
                s.pickpocket_caught.insert(id);
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"ASLT")? {
            for set in [&mut s.assaulted, &mut s.player_assailants] {
                let mut previous = None;
                for _ in 0..r.count(4)? {
                    let id = r.form()?;
                    ordered(&mut previous, id)?;
                    set.insert(id);
                }
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"UWIT")? {
            for _ in 0..r.count(13)? {
                let faction = r.form()?;
                let gold = r.i32()?;
                let violent = r.bool()?;
                let witnesses = (0..r.count(4)?)
                    .map(|_| r.form())
                    .collect::<Result<Vec<_>>>()?;
                s.unreported.push(crate::crime::Unreported {
                    faction,
                    gold,
                    violent,
                    witnesses,
                });
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"LCRM")? {
            use crate::crime::CrimeType::*;
            let mut previous = None;
            for _ in 0..r.count(5)? {
                let faction = r.form()?;
                ordered(&mut previous, faction)?;
                let kind = match r.u8()? {
                    0 => Steal,
                    1 => Pickpocket,
                    2 => Trespass,
                    3 => Attack,
                    4 => Murder,
                    5 => Escape,
                    6 => Werewolf,
                    _ => return Err(Corrupt("invalid saved crime type".into())),
                };
                s.last_crime.insert(faction, kind);
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"CTRL")? {
            s.controls = r.u16()?;
            s.cart_mode = r.bool()?;
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"VREC")? {
            s.voice_recovery = (r.f32()?, r.f32()?);
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"SREG")? {
            s.stamina_wait = r.f32()?;
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"MREG")? {
            s.magicka_wait = r.f32()?;
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"JMEM")? {
            s.journal_memory = Some((r.bool()?, r.u64()?, r.u64()?));
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"MUSE")? {
            for history in [
                &mut s.magic_usage.spells,
                &mut s.magic_usage.schools,
                &mut s.magic_usage.shouts,
            ] {
                for _ in 0..r.count(12)? {
                    history.counts.push((r.u32()?, r.u64()?));
                }
                let favorite = r.u32()?;
                history.favorite = (favorite != 0).then_some(favorite);
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"CAMR")? {
            s.camera = Some((r.bool()?, r.f32()?));
            r.finish()?;
        }
        s.validate()?;
        Ok(s)
    }
    pub fn validate(&self) -> Result<()> {
        if self.random_streams.is_some_and(|(rng, _)| rng == 0) {
            return Err(Corrupt("zero random generator state".into()));
        }
        for &(at, child, parent) in &self.activations {
            time(at)?;
            nonnull(child)?;
            nonnull(parent)?;
        }
        if self.jail_days < 0 {
            return Err(Corrupt("negative jail days".into()));
        }
        if let Some(id) = self.sentence.1 {
            nonnull(id)?;
        }
        if let Some(s) = self.sentence.0 {
            nonnull(s.faction)?;
            nonnull(s.inside)?;
            if !(1..=7).contains(&s.days)
                || [
                    s.owed.violent,
                    s.owed.nonviolent,
                    s.owed.infamy_violent,
                    s.owed.infamy_nonviolent,
                ]
                .iter()
                .any(|&v| v < 0)
                || s.owed.violent.checked_add(s.owed.nonviolent).is_none()
            {
                return Err(Corrupt("invalid jail sentence".into()));
            }
        }
        time(self.resistance.2 as f64)?;
        for &id in &self.resistance.0 {
            nonnull(id)?;
        }
        for (&guard, p) in &self.pursuits {
            nonnull(guard)?;
            nonnull(p.faction)?;
            time(p.bark_in as f64)?;
        }
        time(self.home_warnings.1)?;
        for (&actor, &cell) in &self.home_warnings.0 {
            nonnull(actor)?;
            nonnull(cell)?;
        }
        for (&actor, &cell) in &self.private_homes {
            nonnull(actor)?;
            nonnull(cell)?;
        }
        if let Some(t) = self.trespass {
            nonnull(t.cell)?;
            time(t.next)?;
            if !(0..=2).contains(&t.level) {
                return Err(Corrupt("invalid trespass level".into()));
            }
        }
        if self.camera.is_some_and(|(_, distance)| {
            !distance.is_finite()
                || !(crate::player_body::THIRD_PERSON_MIN..=crate::player_body::THIRD_PERSON_MAX)
                    .contains(&distance)
        }) {
            return Err(Corrupt("invalid saved camera distance".into()));
        }
        crate::stats::MagicUsage::restore(&self.magic_usage)
            .map_err(|err| Corrupt(err.to_string()))?;
        if self
            .journal_memory
            .is_some_and(|(_, journal, fills)| journal == u64::MAX || fills == u64::MAX)
        {
            return Err(Corrupt("exhausted journal/alias sequence".into()));
        }
        time(self.magicka_wait as f64)?;
        time(self.stamina_wait as f64)?;
        let (remaining, total) = self.voice_recovery;
        time(remaining as f64)?;
        time(total as f64)?;
        if remaining > total {
            return Err(Corrupt("voice recovery exceeds total".into()));
        }
        if self.controls & !0x1ff != 0 {
            return Err(Corrupt("unknown disabled-control bits".into()));
        }
        for &faction in self.last_crime.keys() {
            nonnull(faction)?;
        }
        for crime in &self.unreported {
            nonnull(crime.faction)?;
            if crime.gold <= 0 || crime.witnesses.is_empty() {
                return Err(Corrupt("invalid unreported crime".into()));
            }
            for &witness in &crime.witnesses {
                nonnull(witness)?;
            }
        }
        for &id in self.assaulted.iter().chain(&self.player_assailants) {
            nonnull(id)?;
        }
        for &id in &self.pickpocket_caught {
            nonnull(id)?;
        }
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
        let (rng, cond) = self.random_streams.unwrap_or((1, 0x9E37_79B9_7F4A_7C15));
        e.rng = rng;
        e.cond_rng.set(cond);
        e.activation.restore_pending(self.activations);
        e.crime.arrests.days_in_jail = self.jail_days;
        e.crime.arrests.jailed = self.sentence.0;
        e.crime.arrests.pending = self.sentence.1;
        e.crime.arrests.resisting = self.resistance.0;
        e.crime.arrests.yield_attempted = self.resistance.1;
        e.crime.arrests.sheathed_for = self.resistance.2;
        e.crime.arrests.alarmed = self.pursuits;
        // Dialogue itself is transient; allow the restored guards to approach again.
        e.crime.arrests.arresting = None;
        e.crime.reacting_victim = None;
        e.crime.warned_homes = self.home_warnings.0;
        e.crime.time_to_go_next = self.home_warnings.1;
        e.crime.private_homes = self.private_homes;
        e.crime.trespass = self.trespass;
        let (third, distance) = self
            .camera
            .unwrap_or((false, crate::player_body::THIRD_PERSON_DEFAULT));
        e.set_third_person(third);
        e.third_person_distance = distance;
        e.third_person_combat = 0.0;
        let (hidden, journal, fills) = self.journal_memory.unwrap_or_default();
        e.scripts.misc_objectives_hidden = hidden;
        // Older saves contain per-quest sequences but omitted their allocators.
        let (journal_floor, fill_floor) = sequence_floors(e);
        e.scripts.journal_seq = journal.max(journal_floor);
        e.scripts.fill_seq = fills.max(fill_floor);
        e.casting.restore_regen_wait(self.magicka_wait);
        e.player_stamina_wait = self.stamina_wait;
        e.shouts
            .restore_recovery(self.voice_recovery.0, self.voice_recovery.1);
        e.disabled_controls = crate::player::DisabledControls::from_saved_bits(self.controls);
        e.hud_cart_mode = self.cart_mode;
        e.crime.last_crime = self.last_crime;
        e.crime.unreported = self.unreported;
        e.crime.assaulted = self.assaulted;
        e.crime.player_assailants = self.player_assailants;
        e.crime.pickpocket_caught = self.pickpocket_caught;
        e.crime.crime_factions = self.crime_factions;
        e.location_keyword_data = self.location_keyword_data;
        e.cleared_locations = self.cleared_locations;
        e.harvests.harvested = self.harvested;
    }
}
