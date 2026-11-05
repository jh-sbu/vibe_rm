# Package data and targets (2026-11-05)

## Public references and assets examined

- Local UESP PACK page, [revision 2309013](https://en.uesp.net/w/index.php?oldid=2309013),
  Public Package Data: ANAM describes Bool/Float/Int values stored in CNAM,
  locations in PLDT, references/selectors in PTDA and topics in PDTO/TPIC.
  These documented numeric input types have no global reference variant.
- Local UESP FURN page, [revision 3098570](https://en.uesp.net/w/index.php?oldid=3098570):
  FNPR marker types distinguish sitting and sleeping. Loaded furniture also
  supplies its model marker types, permissions and reservations.
- Public [xEdit Skyrim definitions](https://github.com/TES5Edit/TES5Edit/blob/dev-4.1.6/Core/wbDefinitionsTES5.pas),
  `wbObjectTypeEnum` and PACK's public data union: Object Type selector numbers
  are separate from condition function GetIsObjectType's FormType numbers.
  The checked source header is MPL 2.0; only layout/enum facts were consulted,
  with no implementation copied into this MIT project.
- External vanilla `Package.pex`, inspected using `pex-dump`: its native
  functions are GetTemplate and GetOwningQuest. The full `pex-natives` census
  of declarations in installed scripts exposes no package-data setter.
  EvaluatePackage requests evaluation; it does not change the stored inputs.

## Census and implementation consequence

`vrm-tool pack-census <Data> --masters` reads all 7,573 PACK records from the
five masters. Comparing concrete and template UNAM input indices finds **zero
concrete packages omitting a template input**. Default inheritance therefore
adds mod support without changing a vanilla package's authored input values.
Synthetic tests cover inherited numeric values and targets, explicit false and
null overrides, plugin-relative FormIDs and expanded Stacked branch conditions.

No vanilla runtime mutation of numeric package data was found. This does not
prove the original engine cannot mutate internal package data; it establishes
that neither the documented record types examined nor vanilla Papyrus exposes
the proposed setter/global-backed input mechanism. Dynamic globals in CTDA,
quest aliases and linked-reference targets continue to resolve through their
existing paths; those are not mutations of a PACK's numeric input values.

Nearest-target rules and uncertain aggregate type interpretation are recorded
in `known_ambiguities/package-trees.md`. Location kinds 4/5/7/11 and target kind
5 remain outside this implementation as specified by the plan.
