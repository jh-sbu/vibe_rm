# Actor and reference natives without a better home

Vanilla call counts are `vrm-tool pex-natives` (calls are matched by name, so
names shared between classes share their count).

## `GetRace`

- `Actor.GetRace` and `ActorBase.GetRace` (90 calls) return the race the
  NPC's templates give (`RNAM` through template flag 0x1, Use Traits), the
  same lookup the renderer and impacts use; a form that is no NPC returns
  None.
- Unconfirmed: what `ActorBase.GetRace` returns for a base whose traits come
  from a leveled list (we follow the first pick, as for a reference with no
  seed), and whether it returns the base's own `RNAM` instead. `SetRace` is
  not supported, so the race never changes.
