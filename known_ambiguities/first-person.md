# First-person character choices

Supporting research: [the asset survey](../research/first-person.md) and
[the implementation plan](../plans/first-person.md). These choices are pending
comparison with vanilla Skyrim SE, not deliberate deviations.

- **Armor without MOD4/MOD5.** Use only explicit first-person models, falling
  across sex when necessary. Omit addons with neither, even though
  [UESP ARMA revision 3089062](https://en.uesp.net/w/index.php?oldid=3089062)
  describes MOD2/3 as serving all viewpoints. The survey finds headgear,
  jewelry, feet and creature parts among the 866 omitted addons, but also
  body/hand parts. Drawing those third-person meshes around the eye is not
  justified by the separate arms rig. Exact vanilla suppression is unresolved;
  the original-game probe asks about helmets and sheathed arms.
