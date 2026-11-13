//! The player's live conversation (optional `CONV`): an engine-owned layout,
//! not a claim about how Skyrim SE saves dialogue. Loading opens it again
//! once its speaker has loaded, on the INFO being said, without running the
//! INFO's begin fragment; the response being said starts over.

use esp::FormId;

use super::codec::{Corrupt, Reader, Result, Writer};
use super::file::SaveFile;
use crate::engine::{Engine, PLAYER_REF};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SavedConversation {
    pub speaker: FormId,
    /// The topic and INFO being said, if any.
    pub said: Option<(FormId, FormId)>,
    /// The first of the INFO's responses still to say (the one being said,
    /// which starts over).
    pub resume: u32,
    /// A goodbye line was said: it ends.
    pub ending: bool,
}

/// The conversation on now, unless it is an arrest (those don't resume:
/// `known_ambiguities/saves.md`).
pub(crate) fn capture(e: &Engine) -> Option<SavedConversation> {
    let c = e.conversation.as_ref()?;
    if e.crime
        .arrests
        .arresting
        .is_some_and(|(g, _)| g == c.npc_ref)
    {
        return None;
    }
    let said = c.info.as_ref().map(|(t, i)| (t.id, i.id));
    let resume = c.info.as_ref().map_or(0, |(_, i)| {
        let left = c.queue.len() + c.current.is_some() as usize;
        i.responses.len().saturating_sub(left) as u32
    });
    Some(SavedConversation {
        speaker: c.npc_ref,
        said,
        resume,
        ending: c.ending,
    })
}

pub(crate) fn write(value: Option<&SavedConversation>, file: &mut SaveFile) {
    let Some(c) = value else { return };
    let mut w = Writer::default();
    w.form(c.speaker);
    w.bool(c.said.is_some());
    if let Some((topic, info)) = c.said {
        w.form(topic);
        w.form(info);
    }
    w.u32(c.resume);
    w.bool(c.ending);
    file.add(b"CONV", w.0);
}

pub(crate) fn read(file: &SaveFile) -> Result<Option<SavedConversation>> {
    if file.sections.iter().filter(|(t, _)| t == b"CONV").count() > 1 {
        return Err(Corrupt("duplicate CONV".into()));
    }
    let Some(bytes) = file.section(b"CONV") else {
        return Ok(None);
    };
    let mut r = Reader::new(bytes);
    let speaker = r.form()?;
    let said = if r.bool()? {
        Some((r.form()?, r.form()?))
    } else {
        None
    };
    let resume = r.u32()?;
    let ending = r.bool()?;
    r.finish()?;
    if speaker.is_null()
        || speaker == PLAYER_REF
        || said.is_some_and(|(t, i)| t.is_null() || i.is_null())
        || (said.is_none() && resume != 0)
    {
        return Err(Corrupt("invalid CONV".into()));
    }
    Ok(Some(SavedConversation {
        speaker,
        said,
        resume,
        ending,
    }))
}

impl Engine {
    /// Open a saved conversation again: now if its speaker is loaded, else
    /// once its cell loads.
    pub(crate) fn restore_saved_conversation(&mut self, saved: Option<SavedConversation>) {
        self.world_state.pending_conversation = None;
        let Some(saved) = saved else { return };
        if self.actor_ref(saved.speaker).is_none() {
            self.world_state.pending_conversation = Some(saved);
            return;
        }
        self.resume_conversation(&saved);
    }
}
