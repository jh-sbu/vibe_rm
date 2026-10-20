# Dialogue responses' speaker and listener idles

Implemented in `src/dialogue/mod.rs` (`Response::speaker_idle` /
`listener_idle`, read from the INFO) and `Engine::line_gesture`
(`src/ai/mod.rs`), used where conversation lines, scene lines and barks are
spoken. Source: UESP's [INFO
page](https://en.uesp.net/w/index.php?oldid=2346674): each response carries
`SNAM` "Idle Animations: Speaker" and `LNAM` "Idle Animations: Listener",
form ids; and the base game's data (`vrm-tool info-idles <data>`: of 44040
responses in 40928 INFO records, 475 name a speaker idle and one a listener
idle, all of them IDLE records: 37 `Dragon_PerchRoar`, 32 `IdlePointClose`, 25
`IdleGive`, 24 `IdleLuteStart`, 18 `IdleBook_Reading`...).

## Open (choices made here)

- **Instead of the random gesture.** Without an idle of its own a line gets a
  random gesture from the `ActionTalking` idle tree (already so). With one,
  the speaker's graph is sent the idle's event instead (`IdleSalute`,
  `IdlePointClose`...); a graph that doesn't take it (a dragon's
  `Dragon_PerchRoar` on a humanoid, a creature) gets the random gesture. That
  the named idle replaces rather than joins the generic gesture isn't
  documented.
- **How long it lasts.** The event is sent once when the response starts, as
  the random gestures are; a gesture that loops (`IdleBook_Reading`) goes on
  until the next response's gesture or the end of the conversation or scene
  line (`IdleStop`), not for the length of the voice file, and no pose is
  held through a pause between lines.
- **Listeners.** The listener idle isn't played: the one response in the base
  game that names one (`IdleEatSoup`) is the only evidence, and a
  conversation's listener is the player, who has no idle to play. Who listens
  in a scene or a bark isn't tracked either.
  A renewed local UESP search (2026-10-20) found the INFO field labels above
  and [SCEN's head-tracking alias](https://en.uesp.net/w/index.php?oldid=3089103),
  but no rule establishing that a scene action's head-tracking target is its
  dialogue listener. Head tracking alone is insufficient evidence to route an
  idle to that actor. Automatic listener choice, player listener support,
  overlap/cleanup and reaction timing remain research gates; no engine binary
  data was examined to resolve them.
