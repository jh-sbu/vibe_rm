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
