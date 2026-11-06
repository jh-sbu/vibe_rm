# Off-screen navigation

Public references read through the local UESP index, 2026-11-06:
[NAVI revision 3089094](https://en.uesp.net/w/index.php?oldid=3089094),
[NVMI revision 2309008](https://en.uesp.net/w/index.php?oldid=2309008),
[NVPP revision 2309010](https://en.uesp.net/w/index.php?oldid=2309010).
Also checked xEdit's wbDefinitionsTES5.pas NAVI layout (MPL 2; project MIT;
layout facts only, no implementation copied). No engine binaries were examined.

NVMI carries a NAVM form, flags, approximate centre, preferred fraction,
edge/preferred-edge links, door reference links and optional island geometry,
then a parent world/cell or signed Y/X grid. The older UESP NVMI page describes
the preferred fraction as an unknown integer; xEdit's current layout identifies
it as float. Its worldspace wording singles out Tamriel, whereas xEdit and
vanilla data use coordinates for every non-null parent world.
NVPP stores arrays of preferred path forms and a road-marker form/index table.
These are preferred-path hints, not replacements for adjacency/door links.

External game-asset census (base five masters only): every master overrides
NAVI 00012FB4 with incremental entries. Their NVMI counts are Skyrim 15462,
Update 313, Dawnguard 1873, HearthFires 132, Dragonborn 1732; all parse.
Only the final winning record yields 1732 entries. Merging in load order and
applying NVSI/deleted NAVM filtering gives 18693 active entries, 33418 links,
2162 doors and 500 preferred-path groups, out of 19269 NAVM records including
deleted ones. Tool: `vrm-tool nav-census <data>`.
