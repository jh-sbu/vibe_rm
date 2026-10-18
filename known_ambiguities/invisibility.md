# Invisibility interruption

Implemented in `src/invisibility.rs`, with action hooks in `src/ai/combat.rs`,
`src/ai/archery.rs`, `src/casting.rs`, `src/magic.rs`, `src/engine.rs` and
`src/activation.rs`. Only public sources were consulted for this change. No
engine binary data was examined, no original-game runtime measurement was
made, and the tests generate their own records without game assets.

## Public evidence

- The game's Invisibility spell description, reproduced on the public
  [Invisibility spell page](https://elderscrolls.fandom.com/wiki/Invisibility_(Skyrim)),
  says that activation or attacking ends it. That page distinguishes drawing
  a bow from firing it, and preparing magic from casting it, and describes
  recasting invisibility as renewing the effect.
- [UESP's Invisibility effect page](https://en.uesp.net/wiki/Skyrim:Invisibility_(effect)),
  as quoted in this public [Arqade answer](https://gaming.stackexchange.com/questions/332698/how-does-invisibility-work-in-skyrim),
  lists activating objects, non-invisibility magic and character interactions.
  Direct fetching of the UESP page was blocked during this change; the public
  quotation was consulted instead.
- The Unofficial Skyrim Patch authors' public
  [version history](https://www.afkmods.com/Unofficial%20Skyrim%20Patch%20Version%20History.html)
  records their `TGDoorCloserTriggerScript` fix (bug 19360): using the player
  as a script activation's activator aborts invisibility. This is first-hand
  compatibility evidence for the explicit `Activate(player)` hook.
- A player's public [attack/recast experiment](https://www.reddit.com/r/skyrim/comments/161dlma/)
  describes a charged invisibility spell held through a melee swing, the
  attack ending the old effect, and the following spell renewing it. It
  supports retaining invisibility while preparing a cast and resolving the
  triggering attack before a later recast; it does not establish precise
  graph-event boundaries.

## Implemented boundaries

- Only the actor's finite-duration invisibility effects end. Other effects
  from the same item remain. Each ended effect uses the existing finish path
  to restore its held actor value and run `OnEffectFinish` when scripted.
- Player and NPC combat melee swings interrupt at their resolved hit frame,
  including when the target has disappeared, is out of reach, or is missed.
  A hit's existing sneak eligibility is evaluated before interruption. A
  rejected/cancelled action with no resolved hit frame leaves it alone.
  Player bashes use their existing immediate resolution boundary, including
  a miss; attacks refused by fighting controls never reach that boundary.
- Arrows interrupt the shooter when the actual projectile is spawned,
  including practice shots. Drawing, nocking, cancelled draws, and releases
  which spawn nothing do not. Impact does not interrupt again, so an old shot
  cannot end invisibility acquired since release. Arrow sneak eligibility
  still uses the existing detection-at-impact policy.
- Hand spells, powers and NPC combat spells interrupt at actual delivery,
  before their effects land or their projectile is created; charging alone
  does not. Script/console `cast_item` uses the same rule for a valid magic
  item. Passive abilities, enchantment hits and incoming effects do not count
  as new casts by their recipient.
- Every non-invisibility concentration delivery interrupts, including after
  new invisibility is acquired during an ongoing channel. This is an explicit
  policy; public descriptions do not establish that timing case.
- A self-delivered item with any invisibility-archetype effect is exempt as
  a renewal. This is a metadata classification, without evaluating that
  effect's conditions. It preserves an old source through a different
  invisibility recast; normal same-item effect replacement still applies.
- Player activation interrupts after disabled-control and scene-block checks
  but before script dispatch/default activation. No target or a disabled
  Activate control leaves it intact. Scripts' explicit `Activate(player)`
  interrupts when the target reference exists; activation as someone else
  does not reveal the player.

## Remaining ambiguities and scope

- Exact attack-start, hit-frame and detection ordering is unconfirmed. The
  first resolved hand tests sneaking while invisible; later hands in a
  multi-hit/dual attack see the new state. A swing cancelled before its hit
  frame currently preserves invisibility. Harmless UseWeapon melee practice
  does not enter the combat hit-frame path. Their original behavior needs
  public observations or permitted asset evidence.
- Constant ability/enchantment invisibility is left alone. Ending it without
  a sourced suppression/re-entry rule would let `sync_abilities` immediately
  add it again or invent a permanent suppression policy. Direct script actor
  value writes also remain independent of active-effect interruption.
- Mixed self spells with both invisibility and other effects, failed renewal
  conditions, scripted casts whose effects all fail, and concentration
  timing need observation. A valid scripted cast currently interrupts even
  if it has no successful effect; an unknown item does not.
- Potion/food consumption is unchanged. Public pages disagree: UESP's quoted
  list includes potions, while the spell page lists consuming items among
  actions that preserve invisibility. Other inventory interactions, dialogue
  started outside the Activate path, grabbing, crafting, and NPC activation
  need their own supported boundaries.
- Blocked default activation and locked objects still reach scripts and
  interrupt through the chosen activation boundary. The exact distinction
  between an attempted interaction and a completed interaction is unsourced.
