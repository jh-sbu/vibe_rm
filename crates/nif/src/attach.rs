//! Attachment technique tags authored in model extra data.
use crate::{Block, ExtraData, Nif};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodeAttachTags {
    pub named_nodes: Vec<String>,
    pub multi_technique: bool,
    pub havok_geometry: bool,
}
pub type AttachTags = BTreeMap<i32, NodeAttachTags>;

pub fn attach_tags(nif: &Nif) -> AttachTags {
    let mut out = AttachTags::new();
    for (index, block) in nif.blocks.iter().enumerate() {
        let Some(av) = block.av() else { continue };
        let mut tags = NodeAttachTags::default();
        for &reference in &av.net.extra_data {
            let Some(Block::ExtraData(ExtraData::Strings { values, .. })) = nif.get(reference)
            else {
                continue;
            };
            for value in values {
                if let Some(target) = value.strip_prefix("NamedNode&") {
                    if !target.is_empty() && !tags.named_nodes.iter().any(|n| n == target) {
                        tags.named_nodes.push(target.to_owned());
                    }
                } else if value == "MultiTechnique" {
                    tags.multi_technique = true;
                } else if value == "HavokGeometry" {
                    tags.havok_geometry = true;
                }
            }
        }
        if tags != NodeAttachTags::default() {
            out.insert(index as i32, tags);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strings_extra_data_uses_inline_strings_and_rejects_truncation() {
        let mut bytes = 0u32.to_le_bytes().to_vec();
        bytes.extend(2u32.to_le_bytes());
        for s in ["NamedNode&NPC Head [Head]", "MultiTechnique"] {
            bytes.extend((s.len() as u32).to_le_bytes());
            bytes.extend(s.as_bytes());
        }
        let strings = vec!["AttachT".into()];
        let mut r = crate::reader::Reader::new(&bytes);
        r.strings = &strings;
        let block = crate::blocks::parse_block("NiStringsExtraData", &mut r).unwrap();
        assert_eq!(r.remaining(), 0);
        let Some(Block::ExtraData(ExtraData::Strings { name, values })) = block else {
            panic!()
        };
        assert_eq!(name, "AttachT");
        assert_eq!(values, ["NamedNode&NPC Head [Head]", "MultiTechnique"]);
        let mut short = crate::reader::Reader::new(&bytes[..bytes.len() - 1]);
        assert!(crate::blocks::parse_block("NiStringsExtraData", &mut short).is_err());
    }
}
