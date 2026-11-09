# Sub graphs: SetSubGraphFloatVariable and PlaySubGraphAnimation

## Sources

The Papyrus Index (papyrus.bellcube.dev, consulted 2026-11-09):

- `Actor.SetSubGraphFloatVariable(asVariableName, afValue)`: sets a variable
  "on all of an actor's sub graphs". The CK wiki's note: vanilla meshes and
  scripts suggest armor addons and art objects can have behaviour graphs;
  these are the "sub-graphs" of the actors they are attached to.
- `Actor.PlaySubGraphAnimation(asEventName)`: "Send an event to the subgraphs
  of an actor."

## Game-asset observations (`vrm-tool pex-calls`)

- Variables: `ftoggleBlend` / `fdampRate` (and case variants), from the
  Dawnguard bats, mist form, gargoyles, reflect shield, Riekling barrels and
  spriggan effects.
- Events: `StopEffect`, `SkinFadeOut` / `SkinFadeIn`, `noGlow`, `glowRed`,
  `stage2` / `stage3`, `KillFX`, from dragons' blood and death, spriggans,
  the glowing sabre cat and deer, Dwarven centurions and Miraak.

## Choices

- The sub graphs are the behaviour graphs of the art objects (visual effects,
  magic effects' hit art) on the actor, which run here. Armor addons' graphs
  aren't run, so the creature skins' events (spriggans' `SkinFadeOut`, the
  centurions' and glowing animals') reach nothing yet: open in
  `plans/ROADMAP.md`.
- A variable set before an art object attaches doesn't reach it.
