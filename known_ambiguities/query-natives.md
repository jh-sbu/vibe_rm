# Record and location query natives

References: refs/pex-natives/. These choices have not been verified in Skyrim SE.

## unique

IsUnique reads the queried base directly, consistent with the public flag description. Whether unusual templated NPCs inherit uniqueness is unverified.

## class

GetClass follows Use Stats templates per the public NPC format. A leveled template is resolved with the existing base-query seed of zero; the original return for an unresolved leveled template is unverified.

## editor

GetEditorLocation delegates to the existing location index, including its created-reference and player policies. Their precise Skyrim behavior remains unverified.
