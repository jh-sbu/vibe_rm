# Escort and preferred paths

2026-11-06: examined vanilla PACK templates with pack-tree and all five masters
with SHOW=procedure:Escort pack-census. Six own-tree occurrences belong to
Escort 00023B73, EscortPlayerWhenNear 00069665, PatrolAndHerd 000964CF and
WEEscortPrisoner 0009D5A2. Their Escort argument ordering is selector, count,
destination, wait distance, min/max follower distances, horse, preferred path,
optional run-if-behind distance. PatrolAndHerd uses ObjectList 18 (Found Herd)
in both Travel and Escort, after Find with a faction criterion. Escort's
destination is its PathStart reference; a parallel Wait bounds the interval.

MQ102HadvarEscortPlayerToRiverwood 0005421A binds player 00000014, destination
00038B08 radius 150, wait 600, min/max 120/256, near-branch radius 1500,
preferred true and run-if-behind 500. Its Stacked tree switches Escort/Travel.

The current masters contain none of the optional InitialLocation/extra
Follower(s) names previously listed in the Escort ambiguity note. Optional
named template extensions are supported separately; native argument indices
are not guessed from those absent names.

Public layout facts checked against xEdit wbDefinitionsTES5.pas (MPL 2;
project MIT, no implementation copied), and the external vanilla NAVM assets:
preferred is triangle flag 0x0008. Public [UESP NAVM revision 3089096](https://en.uesp.net/w/index.php?oldid=3089096)
points to NVNM navigation data. Neither those records nor the public layout
specify a path-cost multiplier. The factor and pacing choices are recorded
in `known_ambiguities/escort.md` and covered by route/roster fixtures.
