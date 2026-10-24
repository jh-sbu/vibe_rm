# Ten common natives

Advances Roadmap Next 4 (AI depth), 5 (game logic) and 9 (saves), with one
commit per item. Items were chosen by how often vanilla scripts call them:
`vrm-tool pex-natives` (new) lists every native the game's scripts declare
with the number of calls naming it, and these were the most called that did
nothing. Per the user's direction, open questions were added to
`known_ambiguities/` rather than settled by a test mod for now. Synthetic
fixtures only.

| Item | Completed behavior | Section / ambiguity |
| --- | --- | --- |
| 1 | Quest aliases' Essential / Protected flags (`FNAM` 0x40 / 0x400) hold while filled and running; `IsEssential`, `IsProtected`, `Kill`, damage and bleeding out share one answer | `protection.md` |
| 2 | `ActorBase.SetEssential` / `SetProtected` / `SetInvulnerable` and their reads; invulnerable actors' health can't be lowered | `BFLG`; `protection.md` |
| 3 | `Actor.SetGhost` / `IsGhost`, `GetIsGhost` (237), the base's ghost flag; blows pass ghosts by | `GHST`; `protection.md` |
| 4 | `Actor.GetRace` / `ActorBase.GetRace` | `actor-natives.md` |
| 5 | `ObjectReference.GetNthLinkedRef` | `actor-natives.md` |
| 6 | `FormList.AddForm` / `RemoveAddedForm` / `Revert` / `Find`; every list reader sees additions | `FLAD`; `formlists.md` |
| 7 | `Actor.StopCombatAlarm`: fights, searches and pursuits of the actor end | `actor-natives.md` |
| 8 | `Actor.SetOutfit`: the old outfit's armor goes, the new one's is worn | `OUTF`; `outfits.md` |
| 9 | `ObjectReference.Delete` for references made while playing | `created-refs.md` |
| 10 | `Actor.EnableAI` | `AIOF`; `actor-natives.md` |

## Tests

- `protection::tests`: alias flags with quest start / stop, the player-only
  rule, conditions 354 and 476; base flag natives across actors of a base;
  invulnerable NPCs and the player against blows and `DamageActorValue`;
  ghosts per reference with `GetIsGhost`.
- `script::actor_natives_tests`: race through templates, linked-ref chains,
  `StopCombatAlarm` for the player and an NPC, outfit swaps (sleep outfits
  ignored, shared pieces kept), `Delete` (alias-held and plugin references),
  `EnableAI`.
- `formlists::tests`: additions after the plugin's, duplicates, removal of
  added forms only, `Revert`, `Find`.
- `save::tests`: `BFLG`, `GHST`, `FLAD`, `OUTF` and `AIOF` round trips,
  every truncated prefix, null forms and invalid entries, plus
  replace-on-load and legacy clearing in a running engine.

## Final validation

- `cargo test --workspace`: all pass (vibe_rm 417 passed, 227 ignored).
- `cargo test --bin vibe_rm -- --include-ignored`: 644 passed.
- `cargo fmt` and `git diff --check` are clean.
