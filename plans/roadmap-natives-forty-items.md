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
