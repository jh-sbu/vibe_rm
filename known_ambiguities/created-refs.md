# References made while playing

## `Delete`

- `ObjectReference.Delete()` (438 calls by name, `vrm-tool pex-natives`;
  mostly markers, effects and actors scripts made with `PlaceAtMe`) removes
  a reference made while playing, lying in the world or an actor, at once:
  it leaves the world and its whereabouts, and scripts naming it get None.
  If a quest alias holds it, it is only disabled, and stays until the
  quest's clean-up (aliases' own creations) or for good.
- References from plugins are left as they are, and so are references made
  inside containers (the container's inventory counts the item; nothing
  ties the count to the reference).
- Unconfirmed: the game's rule is said to be "flagged for delete, deleted
  once nothing holds it" (script variables and properties as well as
  aliases); no accessible source describes it. A script still holding a
  deleted reference here sees calls on it do nothing. Also unconfirmed:
  whether `Delete` on a plugin's reference disables it, does nothing (our
  choice) or deletes it from the save, and what happens to a created item
  in a container.
