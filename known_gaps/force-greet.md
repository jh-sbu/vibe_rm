# Force greet packages

Implemented in `src/ai/greet.rs` (inputs parsed in `src/ai/package.rs`). Based on
the ForceGreet / ForceGreetFromSitting procedure templates as the data lists them
(their inputs, the conditions on their procedure tree) and how the base game's
packages fill those inputs (`vrm-tool force-greets <data>`).

## What is known

- ForceGreet inputs: Topic (`PDTO`: a topic, or a subtype such as `HELO`), NPC
  Wait Location (default near current, 150), Trigger Location "Player here causes
  forcegreet" (default near current, 500), Forcegreet Distance "Don't change ref,
  just radius" (the player, 300), Player must be detected? (false), Greet Using
  Preferred Path?, Sandbox While Waiting? and the sandbox "Allow ..." inputs.
- The template's tree: a Travel procedure to the force greet distance, then the
  ForceGreet procedure, while (the player is detected, or detection isn't asked
  for) and the player is in the trigger location and not moving into a new space,
  lacks a keyword (`000DD631`) and isn't riding (unless "Forcegreet if player on
  horseback?"). Otherwise Sandbox (when "Sandbox While Waiting?") or Travel to the
  wait location.
- ForceGreetFromSitting: Topic, a sit target, a trigger location and "OBS Player
  must be detected?"; the tree's fallback is Sit.
- Of 88 + 20 NPC package slots, most force greets are quest alias packages
  (`ALPC`), which the engine doesn't run yet.

## Open questions

- Greeting again: after a forced conversation the greeter waits 10 s before it
  may come again (while the package still applies and the player is still in the
  trigger location). Whether the game waits, and for how long, is unverified.
- Detection is approximated: within the combat detection distance with a clear
  line from the greeter's eyes to the player's. There is no sneaking detection.
  The distance (1400 units) has no source; see the roadmap's Detection item.
- Not modelled: "moving into a new space", the player keyword check, riding,
  "Greet Using Preferred Path?", ForceGreetWaitSitting (treated as standing).
- Trigger locations by alias (kind 8) fall back to the greeter's editor location;
  "in cell" triggers use the package location rule (the editor location with a
  wide radius) rather than "the player is in that cell".
- Seated greeters speak from the seat once the player is in the trigger location
  (the template has no force greet distance); not seen in game here: the
  carriage drivers' package (`CartDriverOnCartForcegreet`) also needs the player
  sitting in the carriage (`GetSitting` on the player == 3), and the player
  doesn't sit yet.
