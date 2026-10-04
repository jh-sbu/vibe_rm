//! Mapping between record types and Papyrus native script classes.

/// The most specific native Papyrus class for a record type.
pub fn class_for_tag(tag: &[u8; 4]) -> &'static str {
    match tag {
        b"REFR" | b"PGRE" | b"PHZD" | b"PMIS" | b"PARW" | b"PBAR" | b"PBEA" | b"PCON" | b"PFLA" => "ObjectReference",
        b"ACHR" => "Actor",
        b"NPC_" => "ActorBase",
        b"QUST" => "Quest",
        b"GLOB" => "GlobalVariable",
        b"KYWD" => "Keyword",
        b"LCRT" => "LocationRefType",
        b"LCTN" => "Location",
        b"CELL" => "Cell",
        b"WRLD" => "WorldSpace",
        b"FACT" => "Faction",
        b"MESG" => "Message",
        b"SNDR" | b"SOUN" => "Sound",
        b"SPEL" => "Spell",
        b"MGEF" => "MagicEffect",
        b"ARMO" => "Armor",
        b"ARMA" => "ArmorAddon",
        b"WEAP" => "Weapon",
        b"BOOK" => "Book",
        b"ALCH" => "Potion",
        b"INGR" => "Ingredient",
        b"MISC" => "MiscObject",
        b"KEYM" => "Key",
        b"DOOR" => "Door",
        b"LIGH" => "Light",
        b"STAT" | b"MSTT" => "Static",
        b"ACTI" => "Activator",
        b"TACT" => "TalkingActivator",
        b"FURN" => "Furniture",
        b"CONT" => "Container",
        b"IDLE" => "Idle",
        b"PACK" => "Package",
        b"DIAL" => "Topic",
        b"INFO" => "TopicInfo",
        b"SCEN" => "Scene",
        b"FLST" => "FormList",
        b"LVLI" => "LeveledItem",
        b"LVLN" => "LeveledActor",
        b"LVSP" => "LeveledSpell",
        b"RACE" => "Race",
        b"OTFT" => "Outfit",
        b"ECZN" => "EncounterZone",
        b"IMAD" => "ImageSpaceModifier",
        b"RFCT" => "VisualEffect",
        b"EFSH" => "EffectShader",
        b"SHOU" => "Shout",
        b"WOOP" => "WordOfPower",
        b"WTHR" => "Weather",
        b"AMMO" => "Ammo",
        b"SCRL" => "Scroll",
        b"SLGM" => "SoulGem",
        b"ENCH" => "Enchantment",
        b"PERK" => "Perk",
        b"CLAS" => "Class",
        b"HAZD" => "Hazard",
        b"EXPL" => "Explosion",
        b"PROJ" => "Projectile",
        b"MUSC" => "MusicType",
        b"SNCT" => "SoundCategory",
        b"TXST" => "TextureSet",
        b"COBJ" => "ConstructibleObject",
        b"HDPT" => "HeadPart",
        b"FLOR" => "Flora",
        b"TREE" => "TreeObject",
        b"ASTP" => "AssociationType",
        b"CSTY" => "CombatStyle",
        b"EQUP" => "EquipSlot",
        b"SPGD" => "ShaderParticleGeometry",
        b"IMGS" => "ImageSpace",
        b"VTYP" => "VoiceType",
        _ => "Form",
    }
}

/// Native class inheritance (child -> parent), lower case.
pub fn native_parent(class: &str) -> Option<&'static str> {
    Some(match class {
        "actor" => "objectreference",
        "objectreference" => "form",
        "actorbase" => "form",
        "referencealias" | "locationalias" => "alias",
        "activator" | "talkingactivator" | "furniture" | "flora" => "form",
        "potion" | "ingredient" | "scroll" | "soulgem" | "book" | "armor" | "weapon" | "ammo" | "key" | "miscobject" => "form",
        "alias" | "activemagiceffect" => return None,
        "form" => return None,
        _ => "form",
    })
}

/// Whether an object of native class `actual` can be viewed as `wanted`.
pub fn native_is_a(actual: &str, wanted: &str) -> bool {
    let mut cur = Some(actual.to_ascii_lowercase());
    let w = wanted.to_ascii_lowercase();
    for _ in 0..8 {
        let Some(c) = cur else { return false };
        if c == w {
            return true;
        }
        // TalkingActivator extends Activator; Static/Door/... are plain forms.
        if c == "talkingactivator" && w == "activator" {
            return true;
        }
        cur = native_parent(&c).map(str::to_owned);
    }
    false
}
