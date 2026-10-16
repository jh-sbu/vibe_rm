# Relationship associations

## Public sources

- [Creation Kit: Association Type](https://ck.uesp.net/wiki/Association_Type):
  association types describe social connections; their Family Association
  checkbox is used by `HasFamilyRelationship`. Parent/child labels also
  describe non-familial roles such as boss/employee.
- [xEdit Skyrim record definitions](https://github.com/TES5Edit/TES5Edit/blob/dev-4.1.5/Core/wbDefinitionsTES5.pas):
  `RELA DATA` stores parent NPC, child NPC, rank (`u16`), an unknown byte,
  flags (`u8`), and an `ASTP` FormID. Secret is bit 7 of the data flags;
  xEdit also names record-header bit 6 Secret. `ASTP DATA` is a `u32`
  with Family Association in bit 0.
- [Papyrus Actor API](https://papyrus.bellcube.dev/skyrimse/script/actor/):
  `HasAssociation` and `HasFamilyRelationship` accept an optional other
  actor; omission checks for a relationship with anyone.
- [SKSE's public vanilla command list](https://skse.silverlock.org/vanilla_commands.html):
  the condition counterparts `HasAssociationType` and
  `HasFamilyRelationship` take an actor; separate `...Any` conditions
  omit that actor. `HasParentRelationship` is also called `IsParent`.

No game assets or executable data were used for this implementation. Tests
generate small synthetic plugins in temporary directories, including a
reordered master table to verify relationship and condition FormID remapping.

## Implemented

`HasAssociationType` (258), `HasFamilyRelationship` (259),
`HasAssociationTypeAny` (617), and `HasFamilyRelationshipAny` (618) query
direct authored `RELA` links through their `ASTP`. Both ends can query the
association. Any-family checks use the ASTP family bit, independent of rank;
changing rank does not change the authored association. Multiple links for
the same NPC pair are retained. No transitive relatives are inferred.

The cache retains the parent/child order and both documented secret flag
representations. `relationship_is_secret` exposes the combined authored
metadata for future social knowledge handling; this does not implement
secrecy behavior. Direct rank and association queries currently include
secret links.

## Unresolved

- `HasParentRelationship` (261): the available public descriptions say
  only that it checks a parent relationship. They do not settle whether
  it tests the RELA parent endpoint for all association types, only family
  associations, or the specific biological parent/child association; nor
  do they unambiguously identify which actor is the parent. The condition
  remains unsupported rather than adopting one interpretation.
- Secret relationships: the flag layouts are known, but the consulted
  sources do not settle which social behaviors suppress knowledge of the
  relationship, or whether direct family/association/rank/extreme queries
  filter it. No filtering or social knowledge propagation is introduced.
  The relationship between the header and DATA secret flags is also
  unresolved; the metadata API reports true when either is set.
- Whether any-family queries count dead relatives, and whether these
  queries follow NPC templates, is not established. Current queries use
  the referenced actor's actual base NPC and authored links, without
  filtering deaths or traversing templates.
