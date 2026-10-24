# Forty further PEX natives

Eight independent commits of five items. Public references are authored notes
under refs/pex-natives/batch*.md (explicitly force-added because refs is ignored).
No game assets or engine binary data are included or inspected. Unresolved
behavior is recorded in known_ambiguities/remaining-natives.md.

| Batch | Five items | Validation |
| --- | --- | --- |
| 1 | Alias.RegisterForSingleUpdateGameTime, Alias.RegisterForUpdateGameTime, Alias.UnregisterForUpdateGameTime, Game.GetGameSettingString, Ingredient.IsHostile | 2 focused GPU tests passed |
| 2 | Quest.IsStopped, Game.IsFastTravelControlsEnabled, ObjectReference.GetKey, ObjectReference.GetVoiceType, Cell.IsAttached | focused GPU test passed |
| 3 | Location.IsChild, Location.HasCommonParent, Location.HasRefType, Package.GetTemplate, Package.GetOwningQuest | focused GPU test passed |
| 4 | ObjectReference.GetAllItemsCount, ObjectReference.IsContainerEmpty, Actor.GetGoldAmount, Actor.WornHasKeyword, Actor.GetEquippedArmorInSlot | focused GPU test passed |
| 5 | Actor.IsChild, Actor.IsOverEncumbered, Actor.GetCurrentPackage, Actor.GetDialogueTarget, ObjectReference.GetCurrentScene | focused GPU test passed |
| 6 | ObjectReference.GetWorldSpace, ObjectReference.GetScale, ObjectReference.GetHeight, ObjectReference.GetWidth, ObjectReference.GetLength | focused GPU test passed |
| 7 | Form.GetGoldValue, ActorBase.GetDeadCount, TopicInfo.GetOwningQuest | focused GPU test passed |
| 8 | Actor.IsRunning, Actor.IsSprinting, Actor.IsAlarmed, Actor.IsArrestingTarget, ObjectReference.Is3DLoaded | focused GPU test passed |

## Final validation

- Nine new focused synthetic integration tests passed, with GPU checks run at
  the end of their respective batches.
- `cargo test --workspace` with host graphics access: all pass (engine 423
  passed, 245 ignored).
- `cargo test --bin vibe_rm -- --include-ignored --test-threads=1` with host
  graphics access: 668 passed, zero failures or ignored tests.
- The sandbox workspace run hit 26 pre-existing GPU initialization failures:
  fallback adapter sampled-texture limit 16 versus renderer requirement 17.
  The host rerun passed those tests without source changes.
- `rustfmt --check` on all changed Rust files and `git diff --check`: pass.
- Audited all 15 changed paths: Rust source and authored Markdown only. No game
  assets, engine binaries, database, dump, or copied game scripts were added.
- The table contains exactly 38 distinct vanilla natives in eight batches,
  including completion of existing placeholder native handlers.

Unresolved original-engine details are explicit in
[remaining-natives.md](../known_ambiguities/remaining-natives.md). Passing our
synthetic tests is not a claim of behavioral verification in Skyrim SE.
