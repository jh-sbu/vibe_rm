# Flight controller choices

Research and asset identifiers: `research/dragon-flight.md`. The templates,
movement records and graph expose the operations but not the original engine's
steering, combat selection or completion rules. These choices make them run:

- Cruise at the race FLMV's walking speed, with heading changes limited by its
  turn rate. Orbit joins the midpoint of the authored radius band and circles
  it at the authored height. Hover finishes its procedure on arrival (96-unit
  tolerance), leaving the dragon hovering while the next leaf runs. Away
  flight uses the same three-dimensional controller, independent of NAVM.
  Waiting/sleeping advances that controller rather than building ground paths.
- Takeoff holds its transition for one second; landing waits 1.5 seconds after
  entering the landing phase and reaching the target. Ground/perch completion
  advances finite perch-and-shout sequences; standalone perches hold. Use
  discovered furniture events, with FlyStopPerchTower as the missing-way fallback.
  Glide begins two seconds into cruise. These timings are not measured in SE.
- CSFL hover, ground, perch and flying weights choose combat flight phases.
  Their duration uses fCombatHoverTimeMin scaled by the matching CSFL time
  multiplier, capped at the 3600-second flight clock limit (vanilla's 5.5
  gives at most 11 seconds; only a modded setting reaches the cap). Perches are chosen by nearest loaded dragon furniture, with stable
  ties. This weighting and landing search are choices, not documented algorithms.
  Missing combat ground/perch destinations fall back to orbit; with flight
  prohibited, no usable landing surface means holding in air. Forced markers constrain a
  landing; SetAllowFlying(false) requests it. Ex flags retain crash/search policy.
- FlightGrab uses the authored target and the no-collision-check Boolean, and
  attaches the victim at NPC RLegFoot. The graph's talon action supplies root
  motion and releases the victim on its payload notification; without a graph,
  use the clip's 1.067-second release time. Its cursor then advances to the next
  Patrol. Interrupting the grab or losing its carrier releases the attachment.
  The procedure does not independently kill a victim; scripts, spells and normal
  package fragments retain control of quest-specific outcomes. Exact damage and
  the original engine's collision criteria remain unmeasured.
- Shout casts the last non-null word in its SHOU, with at least one second of
  charge and the backing spell's longer charge time. It uses voice recovery,
  no magicka, a direct aim at the target, and the optional blocked/headtrack
  flags. Concentration lasts three seconds in ordinary concentration ticks.
  Cursor counts prevent recasting completed ticks after load; away shouts wait
  for their speaker to load. Combat voices choose the first known hostile SHOU
  in stable FormID order, take its last non-null word, require initial sight and
  range (4096 fallback), and respect a zero CSGD shout weight. They use the same
  charge/recovery/channel rules, and are dropped with combat when unloading.
  These selection, timing and word choices need SE comparison.
- FLGT persists flight permissions, state clocks, marker, perch, grab and combat
  phase. Actor execution already saves position, heading, patrol and cursor;
  VEHI saves the grabbed actor's attachment; existing ACST saves voice recovery.
  Body/graph state is reconstructed on attachment. Old saves have no FLGT.

Queued comparisons: Orbit band/height and turn limits, Hover and perch completion,
CSFL phase probabilities/times, allow-flying/landing-marker order, interrupted and
unloaded grabs, victim damage, Shout word/charge/concentration timing and a save
mid-charge. No original-game run has verified these choices yet.
