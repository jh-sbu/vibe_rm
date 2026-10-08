//! Actor value indices (`ptActorValue` condition parameters, `AVIF` order) and the
//! names scripts and the console use for them.

/// Names by index, as the Creation Kit and Papyrus spell them.
pub const NAMES: [&str; 96] = [
    "Aggression",
    "Confidence",
    "Energy",
    "Morality",
    "Mood",
    "Assistance",
    "OneHanded",
    "TwoHanded",
    "Marksman",
    "Block",
    "Smithing",
    "HeavyArmor",
    "LightArmor",
    "Pickpocket",
    "Lockpicking",
    "Sneak",
    "Alchemy",
    "Speechcraft",
    "Alteration",
    "Conjuration",
    "Destruction",
    "Illusion",
    "Restoration",
    "Enchanting",
    "Health",
    "Magicka",
    "Stamina",
    "HealRate",
    "MagickaRate",
    "StaminaRate",
    "SpeedMult",
    "InventoryWeight",
    "CarryWeight",
    "CritChance",
    "MeleeDamage",
    "UnarmedDamage",
    "Mass",
    "VoicePoints",
    "VoiceRate",
    "DamageResist",
    "PoisonResist",
    "FireResist",
    "ElectricResist",
    "FrostResist",
    "MagicResist",
    "DiseaseResist",
    "PerceptionCondition",
    "EnduranceCondition",
    "LeftAttackCondition",
    "RightAttackCondition",
    "LeftMobilityCondition",
    "RightMobilityCondition",
    "BrainCondition",
    "Paralysis",
    "Invisibility",
    "NightEye",
    "DetectLifeRange",
    "WaterBreathing",
    "WaterWalking",
    "IgnoreCrippledLimbs",
    "Fame",
    "Infamy",
    "JumpingBonus",
    "WardPower",
    "RightItemCharge",
    "ArmorPerks",
    "ShieldPerks",
    "WardDeflection",
    "Variable01",
    "Variable02",
    "Variable03",
    "Variable04",
    "Variable05",
    "Variable06",
    "Variable07",
    "Variable08",
    "Variable09",
    "Variable10",
    "BowSpeedBonus",
    "FavorActive",
    "FavorsPerDay",
    "FavorsPerDayTimer",
    "LeftItemCharge",
    "AbsorbChance",
    "Blindness",
    "WeaponSpeedMult",
    "ShoutRecoveryMult",
    "BowStaggerBonus",
    "Telekinesis",
    "FavorPointsBonus",
    "LastBribedIntimidated",
    "LastFlattered",
    "MovementNoiseMult",
    "BypassVendorStolenCheck",
    "BypassVendorKeywordCheck",
    "WaitingForPlayer",
];

pub const AGGRESSION: u32 = 0;
pub const CONFIDENCE: u32 = 1;
pub const ASSISTANCE: u32 = 5;
/// The first skill (one-handed); the 18 skills follow in `NPC_` `DNAM` order.
pub const FIRST_SKILL: u32 = 6;
pub const BLOCK: u32 = 9;
pub const HEAVY_ARMOR: u32 = 11;
pub const LIGHT_ARMOR: u32 = 12;
pub const SNEAK: u32 = 15;
pub const LAST_SKILL: u32 = 23;
pub const HEALTH: u32 = 24;
pub const MAGICKA: u32 = 25;
pub const STAMINA: u32 = 26;
pub const HEAL_RATE: u32 = 27;
pub const MAGICKA_RATE: u32 = 28;
pub const STAMINA_RATE: u32 = 29;
pub const SPEED_MULT: u32 = 30;
pub const CARRY_WEIGHT: u32 = 32;
pub const MASS: u32 = 36;

/// The name of an actor value, if it is one of the known ones.
pub fn name(index: u32) -> Option<&'static str> {
    NAMES.get(index as usize).copied()
}

/// The index of an actor value by name (any case), with the names the game
/// shows for skills it spells differently internally.
pub fn index(name: &str) -> Option<u32> {
    let alias = match name.to_ascii_lowercase().as_str() {
        "archery" => "Marksman",
        "speech" => "Speechcraft",
        "pickpocketing" => "Pickpocket",
        "lockpick" => "Lockpicking",
        "resistfire" => "FireResist",
        "resistshock" => "ElectricResist",
        "resistfrost" => "FrostResist",
        "resistmagic" => "MagicResist",
        "resistdisease" => "DiseaseResist",
        "criticalchance" => "CritChance",
        _ => name,
    };
    NAMES
        .iter()
        .position(|n| n.eq_ignore_ascii_case(alias))
        .map(|i| i as u32)
}

#[cfg(test)]
mod tests {
    #[test]
    fn indices() {
        assert_eq!(super::index("health"), Some(super::HEALTH));
        assert_eq!(super::index("Archery"), Some(8));
        assert_eq!(super::index("variable05"), Some(72));
        assert_eq!(super::name(super::CARRY_WEIGHT), Some("CarryWeight"));
        assert_eq!(super::name(95), Some("WaitingForPlayer"));
    }
}
