# Scroll casting movement hold

Surveyed 2026-11-02 using `vrm-tool magic <Data> --scrolls`, reading game
records at their external Steam Data path. No game assets are stored here.

The default installed load order contains Skyrim, Update, Dawnguard,
HearthFires, Dragonborn, Fishing, Survival Mode, Rare Curios, Saints & Seducers,
and `_ResourcePack`. Of 104 SCRL records, 102 have SPIT casting type 3 and two
have type 2: `TGDetectLifeScroll` (001019AE) and
`DLC1dunRedwaterDenTelekinesisScroll` (02014042). The five Saints & Seducers
scrolls all have type 3; excluding those leaves 99 records, including the same
two concentration records. These are record counts, not a claim that every
record is obtainable or usable by the player.

Stored SPIT charge times range from zero to three seconds. Seven records have
three seconds: Bane of the Undead, Mass Paralysis, Harmony, Blizzard, and the
three elemental Thralls. This is stored data, not a measured full animation
length or a bound on how long the player may keep the button held.

[UESP's SCRL format](https://en.uesp.net/w/index.php?oldid=2334437)
describes automatic charge-time calculation from effects and identifies type 3
as scroll-specific, while leaving its exact meaning uncertain. The survey
reports raw SPIT values; it does not establish whether every stored charge time
is the effective in-game time. The existing runtime uses SPIT charge time.

[UESP's Scrolls page](https://en.uesp.net/w/index.php?oldid=3543120),
“Using Scrolls”, establishes the walking/running restriction during casting,
release to cast, and preservation on interruption. It does not specify turning,
jumping, sneaking, swimming, airborne behavior, or the precise animation frame
at which movement becomes available. The implementation choices for those
boundaries are recorded in `known_ambiguities/scrolls.md`; manual SE observation
remains in `plans/scroll-casting-hold.md`.
