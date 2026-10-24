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
    pub script_fade: Option<crate::imagespace::GameFade>,
    pub image_modifiers: Vec<crate::imagespace::Active>,
    pub map_camera: Option<(glam::Vec2, f32, f32)>,
    pub attack_recovery: f32,
    pub sneaking: bool,
    pub critical_hits: std::collections::HashSet<FormId>,
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
    /// Caught-pickpocket victims (`PKCT`) with their memory (`PKTM`; legacy
    /// saves have none: caught as they load).
    pub pickpocket_caught: HashMap<FormId, Option<crate::crime::PickpocketCaught>>,
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
    pub active_effects: (Vec<crate::magic::ActiveEffect>, u64),
    /// Value changes still tapering off: the effect as it landed, the rate it
    /// started at and the seconds since.
    pub tapers: Vec<(crate::magic::ActiveEffect, f32, f32)>,
    /// Objects' open states from their behaviour graphs' events.
    pub graph_open: HashMap<FormId, crate::world::animated::OpenState>,
    /// Factions' reactions towards others changed by scripts.
    pub faction_reactions: HashMap<(FormId, FormId), crate::factions::Reaction>,
    /// Factions' player flags changed by scripts.
    pub faction_player_flags: HashMap<FormId, u8>,
    /// Actors scripts had attacked on sight.
    pub attacked_on_sight: std::collections::HashSet<FormId>,
    /// Actor bases' `ACBS` flags scripts set or cleared, by (base, flag).
    pub base_flags: HashMap<(FormId, u32), bool>,
    /// References `SetGhost` made ghosts or not.
    pub ghosts: HashMap<FormId, bool>,
    /// Forms scripts added to form lists, by list, in the order added.
    pub formlist_additions: HashMap<FormId, Vec<FormId>>,
    /// Outfits scripts gave actors.
    pub outfits: HashMap<FormId, FormId>,
    /// Actors whose AI scripts turned off.
    pub ai_off: std::collections::HashSet<FormId>,
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
/// The least an active effect takes (`put_effect`).
const EFFECT_BYTES: usize = 54;

fn put_effect(w: &mut Writer, x: &crate::magic::ActiveEffect) {
    w.u64(x.id);
    w.form(x.target);
    w.opt_form(x.caster);
    w.form(x.item);
    w.u32(x.index as u32);
    w.form(x.effect);
    w.f32(x.magnitude);
    w.f32(x.duration);
    w.f32(x.elapsed);
    w.u8(x.active as u8
        | (x.scripted as u8) << 1
        | (x.quiet as u8) << 2
        | (x.dual as u8) << 3
        | (x.channelled as u8) << 4);
    w.f32(x.held[0]);
    w.f32(x.held[1]);
    w.bool(x.at.is_some());
    if let Some(at) = x.at {
        w.vec3(at);
    }
    w.opt_form(x.summoned);
}

fn get_effect(r: &mut Reader) -> Result<crate::magic::ActiveEffect> {
    let id = r.u64()?;
    let target = r.form()?;
    let caster = r.opt_form()?;
    let item = r.form()?;
    let index = r.u32()? as usize;
    let effect = r.form()?;
    let (magnitude, duration, elapsed) = (r.f32()?, r.f32()?, r.f32()?);
    let bits = r.u8()?;
    if bits >= 1 << 5 {
        return Err(Corrupt("unknown active effect flags".into()));
    }
    let held = [r.f32()?, r.f32()?];
    let at = if r.bool()? { Some(r.vec3()?) } else { None };
    Ok(crate::magic::ActiveEffect {
        id,
        target,
        caster,
        item,
        index,
        effect,
        magnitude,
        duration,
        elapsed,
        active: bits & 1 != 0,
        held,
        scripted: bits & 2 != 0,
        done: false,
        quiet: bits & 4 != 0,
        at,
        summoned: r.opt_form()?,
        dual: bits & 8 != 0,
        channelled: bits & 16 != 0,
    })
}

/// An effect's own values are finite and its forms named.
fn valid_effect(x: &crate::magic::ActiveEffect) -> Result<()> {
    nonnull(x.target)?;
    nonnull(x.item)?;
    nonnull(x.effect)?;
    let finite = x.magnitude.is_finite()
        && x.held.iter().all(|h| h.is_finite())
        && x.at.is_none_or(|at| at.is_finite());
    // Constant effects last forever; the rest have a finite duration.
    let duration = x.duration >= 0.0 && !x.duration.is_nan();
    if !finite || !duration || x.caster.is_some_and(|c| c.is_null()) {
        return Err(Corrupt("invalid active magic effect".into()));
    }
    time(x.elapsed as f64)
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
            script_fade: e.imagespace.fade,
            image_modifiers: e.imagespace.active.clone(),
            map_camera: e
                .map_view
                .as_ref()
                .map(|v| (v.center, v.zoom, v.pitch))
                .or(e.map_start)
                .filter(|(at, zoom, pitch)| at.is_finite() && zoom.is_finite() && pitch.is_finite())
                .map(|(at, zoom, pitch)| {
                    (
                        at,
                        zoom.clamp(0.0, 1.0),
                        pitch.clamp(crate::map_view::MIN_PITCH, crate::map_view::MAX_PITCH),
                    )
                }),
            attack_recovery: e.player_attack_recovery.max(0.0),
            sneaking: e.player.sneaking,
            critical_hits: e.critically_hit.clone(),
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
            pickpocket_caught: e
                .crime
                .pickpocket_caught
                .iter()
                .map(|(&id, &c)| (id, Some(c)))
                .collect(),
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
            active_effects: e.saved_active_effects(),
            tapers: e.saved_tapers(),
            graph_open: e.world_state.graph_open.clone(),
            faction_reactions: e.factions.reactions.clone(),
            faction_player_flags: e.factions.player_flags.clone(),
            attacked_on_sight: e.factions.attacked_on_sight.clone(),
            base_flags: e.protection.base_flags.clone(),
            ghosts: e.protection.ghosts.clone(),
            formlist_additions: e.formlists.added.clone(),
            outfits: e.outfits.set.clone(),
            ai_off: e.ai_off.clone(),
        }
    }
    pub fn write(&self, file: &mut SaveFile) {
        let mut w = Writer::default();
        w.bool(self.script_fade.is_some());
        if let Some(f) = self.script_fade {
            w.bool(f.out);
            w.bool(f.color == [0.0; 3]);
            w.f32(f.delay);
            w.f32(f.duration);
            w.f32(f.t);
        }
        file.add(b"GFAD", w.0);
        let mut w = Writer::default();
        w.len(self.image_modifiers.len());
        for a in &self.image_modifiers {
            w.form(a.imad);
            w.f32(a.t);
            w.f32(a.strength);
            w.f32(a.fade_in);
            w.bool(a.fade_out.is_some());
            if let Some((t, d)) = a.fade_out {
                w.f32(t);
                w.f32(d);
            }
            w.bool(a.cross_fade);
        }
        file.add(b"IMOD", w.0);
        let mut w = Writer::default();
        w.bool(self.map_camera.is_some());
        if let Some((at, zoom, pitch)) = self.map_camera {
            w.f32(at.x);
            w.f32(at.y);
            w.f32(zoom);
            w.f32(pitch);
        }
        file.add(b"MCAM", w.0);
        let mut w = Writer::default();
        w.f32(self.attack_recovery);
        file.add(b"ATRC", w.0);
        let mut w = Writer::default();
        w.bool(self.sneaking);
        file.add(b"SNEK", w.0);
        let mut w = Writer::default();
        let mut ids: Vec<_> = self.critical_hits.iter().copied().collect();
        ids.sort();
        w.len(ids.len());
        for id in ids {
            w.form(id);
        }
        file.add(b"LHCR", w.0);
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
            w.f32(p.bark_in);
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
        let caught = sorted(&self.pickpocket_caught);
        w.len(caught.len());
        for (id, _) in &caught {
            w.form(*id);
        }
        file.add(b"PKCT", w.0);
        let mut w = Writer::default();
        let remembered: Vec<_> = caught
            .iter()
            .filter_map(|&(id, c)| Some((id, (*c)?)))
            .collect();
        w.len(remembered.len());
        for (id, c) in remembered {
            w.form(id);
            w.f64(c.at);
            w.bool(c.away);
        }
        file.add(b"PKTM", w.0);
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
        let mut w = Writer::default();
        let (effects, next_id) = &self.active_effects;
        w.u64(*next_id);
        w.len(effects.len());
        for x in effects {
            put_effect(&mut w, x);
        }
        file.add(b"AEFF", w.0);
        let mut w = Writer::default();
        w.len(self.tapers.len());
        for (x, rate, elapsed) in &self.tapers {
            put_effect(&mut w, x);
            w.f32(*rate);
            w.f32(*elapsed);
        }
        file.add(b"TAPR", w.0);
        let mut w = Writer::default();
        let open = sorted(&self.graph_open);
        w.len(open.len());
        for (id, s) in open {
            w.form(id);
            w.u8(*s as u8);
        }
        file.add(b"GOPN", w.0);
        let mut w = Writer::default();
        w.len(self.faction_reactions.len());
        for ((from, to), r) in sorted(&self.faction_reactions) {
            w.form(from);
            w.form(to);
            w.i32(r.modifier);
            w.u32(r.combat);
        }
        file.add(b"FRCT", w.0);
        let mut w = Writer::default();
        w.len(self.faction_player_flags.len());
        for (faction, flags) in sorted(&self.faction_player_flags) {
            w.form(faction);
            w.u8(*flags);
        }
        file.add(b"FPLR", w.0);
        let mut w = Writer::default();
        let mut ids: Vec<_> = self.attacked_on_sight.iter().copied().collect();
        ids.sort();
        w.len(ids.len());
        for id in ids {
            w.form(id);
        }
        file.add(b"AOSG", w.0);
        let mut w = Writer::default();
        w.len(self.base_flags.len());
        for ((base, flag), on) in sorted(&self.base_flags) {
            w.form(base);
            w.u32(flag);
            w.bool(*on);
        }
        file.add(b"BFLG", w.0);
        let mut w = Writer::default();
        w.len(self.ghosts.len());
        for (actor, on) in sorted(&self.ghosts) {
            w.form(actor);
            w.bool(*on);
        }
        file.add(b"GHST", w.0);
        let mut w = Writer::default();
        w.len(self.formlist_additions.len());
        for (list, forms) in sorted(&self.formlist_additions) {
            w.form(list);
            w.len(forms.len());
            for &f in forms {
                w.form(f);
            }
        }
        file.add(b"FLAD", w.0);
        let mut w = Writer::default();
        w.len(self.outfits.len());
        for (actor, outfit) in sorted(&self.outfits) {
            w.form(actor);
            w.form(*outfit);
        }
        file.add(b"OUTF", w.0);
        let mut w = Writer::default();
        let mut ids: Vec<_> = self.ai_off.iter().copied().collect();
        ids.sort();
        w.len(ids.len());
        for id in ids {
            w.form(id);
        }
        file.add(b"AIOF", w.0);
    }
    pub fn read(file: &SaveFile) -> Result<Self> {
        let mut s = Self::default();
        if let Some(mut r) = optional(file, b"GFAD")? {
            if r.bool()? {
                s.script_fade = Some(crate::imagespace::GameFade {
                    out: r.bool()?,
                    color: if r.bool()? { [0.0; 3] } else { [1.0; 3] },
                    delay: r.f32()?,
                    duration: r.f32()?,
                    t: r.f32()?,
                });
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"IMOD")? {
            for _ in 0..r.count(18)? {
                s.image_modifiers.push(crate::imagespace::Active {
                    imad: r.form()?,
                    t: r.f32()?,
                    strength: r.f32()?,
                    fade_in: r.f32()?,
                    fade_out: if r.bool()? {
                        Some((r.f32()?, r.f32()?))
                    } else {
                        None
                    },
                    cross_fade: r.bool()?,
                });
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"MCAM")? {
            if r.bool()? {
                s.map_camera = Some((glam::Vec2::new(r.f32()?, r.f32()?), r.f32()?, r.f32()?));
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"ATRC")? {
            s.attack_recovery = r.f32()?;
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"SNEK")? {
            s.sneaking = r.bool()?;
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"LHCR")? {
            let mut previous = None;
            for _ in 0..r.count(4)? {
                let id = r.form()?;
                ordered(&mut previous, id)?;
                s.critical_hits.insert(id);
            }
            r.finish()?;
        }
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
        if let Some(mut r) = optional(file, b"AEFF")? {
            let next_id = r.u64()?;
            let mut effects = Vec::new();
            for _ in 0..r.count(EFFECT_BYTES)? {
                effects.push(get_effect(&mut r)?);
            }
            r.finish()?;
            s.active_effects = (effects, next_id);
        }
        if let Some(mut r) = optional(file, b"GOPN")? {
            let mut previous = None;
            for _ in 0..r.count(5)? {
                let id = r.form()?;
                ordered(&mut previous, id)?;
                let state = crate::world::animated::OpenState::from_code(r.u8()?)
                    .ok_or_else(|| Corrupt("invalid graph open state".into()))?;
                s.graph_open.insert(id, state);
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"FRCT")? {
            let mut previous = None;
            for _ in 0..r.count(16)? {
                let key = (r.form()?, r.form()?);
                ordered(&mut previous, key)?;
                let reaction = crate::factions::Reaction {
                    modifier: r.i32()?,
                    combat: r.u32()?,
                };
                s.faction_reactions.insert(key, reaction);
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"AOSG")? {
            let mut previous = None;
            for _ in 0..r.count(4)? {
                let id = r.form()?;
                ordered(&mut previous, id)?;
                s.attacked_on_sight.insert(id);
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"AIOF")? {
            let mut previous = None;
            for _ in 0..r.count(4)? {
                let id = r.form()?;
                ordered(&mut previous, id)?;
                s.ai_off.insert(id);
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"OUTF")? {
            let mut previous = None;
            for _ in 0..r.count(8)? {
                let actor = r.form()?;
                ordered(&mut previous, actor)?;
                s.outfits.insert(actor, r.form()?);
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"FLAD")? {
            let mut previous = None;
            for _ in 0..r.count(8)? {
                let list = r.form()?;
                ordered(&mut previous, list)?;
                let mut forms = Vec::new();
                for _ in 0..r.count(4)? {
                    forms.push(r.form()?);
                }
                s.formlist_additions.insert(list, forms);
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"GHST")? {
            let mut previous = None;
            for _ in 0..r.count(5)? {
                let actor = r.form()?;
                ordered(&mut previous, actor)?;
                s.ghosts.insert(actor, r.bool()?);
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"BFLG")? {
            let mut previous = None;
            for _ in 0..r.count(9)? {
                let key = (r.form()?, r.u32()?);
                ordered(&mut previous, key)?;
                s.base_flags.insert(key, r.bool()?);
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"FPLR")? {
            let mut previous = None;
            for _ in 0..r.count(5)? {
                let faction = r.form()?;
                ordered(&mut previous, faction)?;
                s.faction_player_flags.insert(faction, r.u8()?);
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"TAPR")? {
            for _ in 0..r.count(EFFECT_BYTES + 8)? {
                let x = get_effect(&mut r)?;
                s.tapers.push((x, r.f32()?, r.f32()?));
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
                s.pickpocket_caught.insert(id, None);
            }
            r.finish()?;
        }
        if let Some(mut r) = optional(file, b"PKTM")? {
            let mut previous = None;
            for _ in 0..r.count(13)? {
                let id = r.form()?;
                ordered(&mut previous, id)?;
                let c = crate::crime::PickpocketCaught {
                    at: r.f64()?,
                    away: r.bool()?,
                };
                match s.pickpocket_caught.get_mut(&id) {
                    Some(slot) => *slot = Some(c),
                    None => return Err(Corrupt("pickpocket memory of no victim".into())),
                }
            }
            r.finish()?;
            if s.pickpocket_caught.values().any(Option::is_none) {
                return Err(Corrupt("pickpocket victim without its memory".into()));
            }
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
        let (effects, next_id) = &self.active_effects;
        let mut previous = None;
        let mut summons = std::collections::HashSet::new();
        for x in effects {
            ordered(&mut previous, x.id)?;
            valid_effect(x)?;
            if x.id == 0 || x.id > *next_id || x.summoned.is_some_and(|r| !summons.insert(r)) {
                return Err(Corrupt("invalid active magic effect".into()));
            }
        }
        for &id in self.graph_open.keys() {
            nonnull(id)?;
        }
        for (&(from, to), r) in &self.faction_reactions {
            nonnull(from)?;
            nonnull(to)?;
            if r.combat > crate::factions::MAX_COMBAT_REACTION {
                return Err(Corrupt("invalid faction combat reaction".into()));
            }
        }
        for &id in &self.attacked_on_sight {
            nonnull(id)?;
        }
        for &id in &self.ai_off {
            nonnull(id)?;
        }
        for (&actor, &outfit) in &self.outfits {
            nonnull(actor)?;
            nonnull(outfit)?;
        }
        for (&list, forms) in &self.formlist_additions {
            nonnull(list)?;
            if forms.is_empty() {
                return Err(Corrupt("empty form list additions".into()));
            }
            for (i, &f) in forms.iter().enumerate() {
                nonnull(f)?;
                if forms[..i].contains(&f) {
                    return Err(Corrupt("form added to a list twice".into()));
                }
            }
        }
        for &actor in self.ghosts.keys() {
            nonnull(actor)?;
        }
        for &(base, flag) in self.base_flags.keys() {
            nonnull(base)?;
            if flag.count_ones() != 1 || flag & !crate::protection::acbs::CHANGEABLE != 0 {
                return Err(Corrupt("unknown actor base flag".into()));
            }
        }
        for (&faction, &flags) in &self.faction_player_flags {
            nonnull(faction)?;
            if flags & !crate::factions::PLAYER_FLAGS != 0 {
                return Err(Corrupt("unknown faction player flags".into()));
            }
        }
        for (x, rate, elapsed) in &self.tapers {
            valid_effect(x)?;
            if !rate.is_finite() {
                return Err(Corrupt("invalid taper rate".into()));
            }
            time(*elapsed as f64)?;
        }
        if let Some(f) = self.script_fade {
            time(f.delay as f64)?;
            time(f.duration as f64)?;
            time(f.t as f64)?;
            if f.color != [0.0; 3] && f.color != [1.0; 3] {
                return Err(Corrupt("invalid screen fade color".into()));
            }
        }
        let mut ids = std::collections::HashSet::new();
        let mut cross_fades = 0;
        for a in &self.image_modifiers {
            nonnull(a.imad)?;
            time(a.t as f64)?;
            time(a.fade_in as f64)?;
            if !a.strength.is_finite() || !ids.insert(a.imad) {
                return Err(Corrupt("invalid image modifier".into()));
            }
            if let Some((t, d)) = a.fade_out {
                time(t as f64)?;
                time(d as f64)?;
            }
            if a.cross_fade && a.fade_out.is_none() {
                cross_fades += 1;
            }
        }
        if cross_fades > 1 {
            return Err(Corrupt("multiple current cross-fade modifiers".into()));
        }
        if let Some((at, zoom, pitch)) = self.map_camera {
            if !at.is_finite()
                || !(0.0..=1.0).contains(&zoom)
                || !(crate::map_view::MIN_PITCH..=crate::map_view::MAX_PITCH).contains(&pitch)
            {
                return Err(Corrupt("invalid map camera".into()));
            }
        }
        time(self.attack_recovery as f64)?;
        for &id in &self.critical_hits {
            nonnull(id)?;
        }
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
        for (&id, c) in &self.pickpocket_caught {
            nonnull(id)?;
            if let Some(c) = c {
                time(c.at)?;
            }
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
        e.imagespace.fade = self.script_fade;
        e.clear_image_shakes_for_load();
        e.imagespace.active = self.image_modifiers;
        e.map_start = self.map_camera;
        e.close_map_view();
        if e.menu == Some(crate::items::Menu::Map) {
            e.menu = None;
        }
        e.player_attack_recovery = self.attack_recovery;
        e.player_swing = None;
        e.player_attack_held = None;
        e.player_blocking = false;
        // Graph attack phases are not saved; don't finish the old game's swing.
        e.player_body = None;
        e.player.sneaking = self.sneaking;
        e.player.crouch = self.sneaking as u8 as f32;
        e.player.moving = false;
        e.player.running = false;
        e.player.sprinting = false;
        e.critically_hit = self.critical_hits;
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
        let now = e.game_hours_total();
        e.crime.pickpocket_caught = self
            .pickpocket_caught
            .into_iter()
            .map(|(id, c)| {
                let c = c.unwrap_or(crate::crime::PickpocketCaught {
                    at: now,
                    away: false,
                });
                (id, c)
            })
            .collect();
        e.crime.crime_factions = self.crime_factions;
        e.location_keyword_data = self.location_keyword_data;
        e.cleared_locations = self.cleared_locations;
        e.harvests.harvested = self.harvested;
        e.restore_active_effects(self.active_effects.0, self.active_effects.1);
        e.restore_tapers(self.tapers);
        // Before the save's cells load: graphs built afresh replay these.
        e.world_state.graph_open = self.graph_open;
        e.restore_faction_reactions(self.faction_reactions);
        e.restore_faction_player_flags(self.faction_player_flags);
        e.restore_attacked_on_sight(self.attacked_on_sight);
        e.restore_base_flags(self.base_flags);
        e.restore_ghosts(self.ghosts);
        e.restore_formlist_additions(self.formlist_additions);
        e.restore_outfits(self.outfits);
        e.ai_off = self.ai_off;
    }
}
