//! PC manual topics are MESG entries in the HMPC default form list.
use crate::engine::Engine;
#[derive(Clone, Debug)]
pub struct Topic {
    pub title: String,
    pub body: String,
}
impl Engine {
    pub fn help_topics(&mut self) -> Vec<Topic> {
        let Some(list) = self.lo.default_objects().get(b"HMPC").copied() else {
            return Vec::new();
        };
        let mut seen = std::collections::HashSet::new();
        let mut topics = Vec::new();
        for id in self.formlist(list) {
            if !seen.insert(id) {
                continue;
            }
            let Some(rec) = self.lo.get(id).filter(|r| r.tag().0 == *b"MESG") else {
                continue;
            };
            let title = rec
                .get(b"FULL")
                .map(|d| self.lo.lstring(&rec, d))
                .unwrap_or_default();
            if title.trim().is_empty() {
                continue;
            }
            let body = rec
                .get(b"DESC")
                .map(|d| self.lo.lstring(&rec, d))
                .unwrap_or_default();
            let quest = rec
                .get(b"QNAM")
                .and_then(|d| d.get(..4))
                .map(|d| rec.fid(esp::FormId(u32::from_le_bytes(d.try_into().unwrap()))));
            let body = self.replace_text_tags_preserving_controls(&body, quest);
            topics.push(Topic { title, body });
        }
        topics
    }
}
