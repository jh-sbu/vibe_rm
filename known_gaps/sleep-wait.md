# Sleeping and waiting (sleepwaitmenu.swf)

Implemented in `src/sleep.rs` (the hours passing, the reasons not to) and
`src/swf_ui/sleep_wait_menu.rs` (the menu). Sources: the movie's
ActionScript (`SleepWaitMenu`: `SetSleeping`, `SetCurrentTime`,
`SetPlatform`; `OK(hours)` with its controls disabled, `Cancel`); the game
settings `sNoWait*` / `sNoSleep*` (the reasons, in that wording); the
control map (`Wait` on T); the CK wiki for `RegisterForSleep`,
`OnSleepStart(afSleepStartTime, afDesiredSleepEndTime)` (game days),
`OnSleepStop(abInterrupted)` and `SetActorOwner` / `SetFactionOwner`; the
game's scripts (`PlayerSleepQuestScript` gives Rested from `OnSleepStop`,
`RentRoomScript` gives the player the inn's bed with `SetActorOwner`).

## Unconfirmed: choices made

- **Opening.** T opens the wait menu; activating a bed (furniture with a
  sleep marker) opens the sleep menu, without getting into the bed first. A
  jail cell's bed serves the sentence instead (see `crime.md`).
- **The reasons not to**, checked in this order, each shown as a
  notification: the player in combat or searched for (`HostileActorsNear`),
  off the ground (`InAir`; not with noclip), guards pursuing (`WhileAlarm`),
  a trespass warning (`WarnToLeave`), trespassing (`Trespass`), a detrimental
  effect on health running (`TakingHealthDamage`), and for a bed one owned by
  someone else (`InOwnedBed`). `InCell` (places where waiting isn't allowed)
  isn't checked: where the data says so isn't known.
- **The hours pass** one at a time, 0.2 real seconds each, with the menu up
  and its clock following; they can't be cancelled once started. Each hour:
  the clock and the weather move on; the magic effects on the player run for
  the hour's seconds (180 at the timescale of 20), so those that would end
  do; the player's health, magicka and stamina fill; Hours Waiting or Hours
  Slept counts one.
- **The other actors** are put where their schedules have them at the end
  of each hour, not walked there. Those not loaded are where their packages
  name a place, as when their cells load. Those loaded whose place is
  elsewhere go; those whose place is a loaded cell turn up there (a random
  spot when the package names only the cell); those whose package changed,
  or who are well away from its location (half as far again as its radius,
  at least 144), start over at that location, already in their furniture as
  when a cell loads. The dead, those fighting or bleeding out, those the
  console sent somewhere and those following or escorting the player are
  left be, as are actors not yet given a package.
- **Their health and stamina** come back for each hour's seconds (180) at
  their rates, as they would standing about: those loaded out of combat
  (or slower in it), those away that left hurt too. Those bleeding out stay
  down. Shops restocking isn't done (see ROADMAP).
- **Not done while the hours pass:** nothing interrupts the wait
  (`OnSleepStop` always says not interrupted); the screen doesn't fade while
  sleeping.
- **Sleep events.** Scripts registered with `RegisterForSleep` hear
  `OnSleepStart(now, now + hours / 24)` when sleeping starts and
  `OnSleepStop(false)` when it ends; waiting sends neither.
- **Owners set by scripts** (`SetActorOwner`, `SetFactionOwner` on
  references and cells; none clears) override the data's for the player's
  checks (beds, trespass, locks, stealing prompts, the Story Manager's
  owner); NPCs choosing furniture still go by the data's.
- egui (`--egui-hud`) has no sleep / wait menu; holding T there still runs
  time a hundred times faster.
