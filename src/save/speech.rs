//! Optional PDLG continuity: semantic response selection, never audio handles.
use super::codec::{Corrupt, Reader, Result, Writer};
use crate::ai::speech::{Owner, SavedSpeech};

pub(super) fn validate(states: &[SavedSpeech]) -> Result<()> {
    let mut previous = None;
    for s in states {
        if s.actor.is_null()
            || s.owner.package.is_null()
            || s.topic.is_null()
            || s.info.is_null()
            || s.listener.is_some_and(|r| r.is_null())
            || !s.owner.started_at.is_finite()
            || s.owner.started_at < 0.
            || s.owner.key.len() > 4096
            || s.owner.path.len() > 64
            || previous.is_some_and(|p| p >= s.actor)
        {
            return Err(Corrupt("invalid or unsorted package dialogue".into()));
        }
        previous = Some(s.actor);
    }
    Ok(())
}

pub(super) fn put(states: &[SavedSpeech]) -> Vec<u8> {
    let mut w = Writer::default();
    w.len(states.len());
    for s in states {
        w.form(s.actor);
        w.form(s.owner.package);
        w.f64(s.owner.started_at);
        w.str(&s.owner.key);
        w.len(s.owner.path.len());
        for &i in &s.owner.path {
            w.u32(i);
        }
        w.u64(s.owner.revision);
        w.form(s.topic);
        w.form(s.info);
        w.u32(s.response);
        w.opt_form(s.listener);
        w.bool(s.look);
        w.bool(s.finish);
        w.bool(s.force_subtitle);
    }
    w.0
}

pub(super) fn get(r: &mut Reader) -> Result<Vec<SavedSpeech>> {
    let mut states = Vec::new();
    for _ in 0..r.count(47)? {
        let actor = r.form()?;
        let package = r.form()?;
        let started_at = r.f64()?;
        let bytes = r.bytes()?;
        if bytes.len() > 4096 {
            return Err(Corrupt("package dialogue key too long".into()));
        }
        let key = std::str::from_utf8(bytes)
            .map_err(|_| Corrupt("invalid package dialogue key".into()))?
            .to_owned();
        let n = r.count(4)?;
        if n > 64 {
            return Err(Corrupt("package dialogue path too deep".into()));
        }
        let path = (0..n).map(|_| r.u32()).collect::<Result<_>>()?;
        states.push(SavedSpeech {
            actor,
            owner: Owner {
                package,
                started_at,
                key,
                path,
                revision: r.u64()?,
            },
            topic: r.form()?,
            info: r.form()?,
            response: r.u32()?,
            listener: r.opt_form()?,
            look: r.bool()?,
            finish: r.bool()?,
            force_subtitle: r.bool()?,
        });
    }
    validate(&states)?;
    Ok(states)
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp::FormId;

    fn speech() -> SavedSpeech {
        SavedSpeech {
            actor: FormId(1),
            owner: Owner {
                package: FormId(2),
                started_at: 12.,
                key: "Say#0".into(),
                path: vec![1, 2],
                revision: 3,
            },
            topic: FormId(4),
            info: FormId(5),
            response: 1,
            listener: Some(FormId(6)),
            look: true,
            finish: true,
            force_subtitle: true,
        }
    }

    #[test]
    fn package_dialogue_round_trips_and_rejects_truncation_duplicates_and_invalid_owner() {
        let state = speech();
        let bytes = put(&[state.clone()]);
        let mut r = Reader::new(&bytes);
        assert_eq!(get(&mut r).unwrap(), vec![state.clone()]);
        r.finish().unwrap();
        for n in 0..bytes.len() {
            assert!(get(&mut Reader::new(&bytes[..n])).is_err());
        }
        assert!(validate(&[state.clone(), state.clone()]).is_err());
        let mut bad = state.clone();
        bad.owner.path = vec![0; 65];
        assert!(validate(&[bad]).is_err());
        let mut bad = state;
        bad.owner.started_at = f64::NAN;
        assert!(validate(&[bad]).is_err());
    }
}
