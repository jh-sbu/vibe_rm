# Find, Acquire and Flee

Examined vanilla SE masters and PEX game assets externally with vrm-tool,
2026-11-06. No assets or disassemblies are stored here.

Acquire template 00019713 is Travel → (Find ∥ Wander) → Acquire.
Find takes search location, criterion, ObjectList, own-inventory and optional
randomization flags. Acquire consumes that same list, quantity and theft flags.
MQ101RalofGetDoorKey (0008C816) selects KEYM 000B5178, quantity 1, search
location alias 173, stealing/pickpocketing enabled, killing disabled.
MQ101 (0003372B) alias 173 is BarracksRoomSoldier01, created with that key.
Aliases 173/174 attach MQ101EnemyScript with onDeathStage 280. Its OnDeath
calls MQ101QuestScript.SetStageOnDeadCount; stage280Target is 2.
Scene MQ101KeepIntroSceneB (0004E1E2) waits for stage 280 at phase 28;
phase 29 checks whether the player has the key (stage 260); action 40 in
phase 30 assigns Ralof the Acquire package. Thus its vanilla use requires
corpse inventory search/transfer rather than an assumed live pickpocket.
The fixture checks these record links; PEX was examined separately.

The five-master census has 215 Object IDs: FACT 54, FLST 17, FURN 6,
KEYM 1, NPC_ 6, SHOU 55, SPEL 70, WEAP 6. Faction/list IDs cannot be
treated as ordinary base equality. Object type 18 occurs only in creature
UseWeapon packages; their template has no Find/Acquire preparation.

FleeFrom 000197F1 finds a list, then Flee takes list, destination,
FleeFrom flag, two radii, distance, false, forget-dead and quiet.
FleeTo 000C7039 has the same layout, with final CountsAsIsFleeing flag.
KeepDistanceFrom 00046BA9 supplies a direct target instead of a list and
conditions its Flee branch on proximity.

Ownership evidence: [UESP Skyrim:Crime, revision 3639929](https://en.uesp.net/w/index.php?oldid=3639929),
read from the local index, says owned-item taking is theft and dead NPC
inventory taking is not. Its follower crime rules concern player commands;
it does not specify autonomous NPC theft reactions.
