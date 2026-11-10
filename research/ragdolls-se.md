# Vanilla SE ragdoll probe (pending observations)

Build, without installing or launching Skyrim:

```
cargo build -p vrm-tools
target/debug/vrm-tool test-mod ragdolls "<Skyrim SE>/Data" tests/assets/VrmRagdolls.esp
```

Generated plugin/scripts stay in ignored `tests/assets/`. The user installs/runs
these manually on a throwaway save, on flat open ground. The engine does not run
the original game. The quest is start-game-enabled; after its instructions,
use `set VrmRagRun to <n>` (no `cqf`):

| Phase | Action |
|---|---|
| 1–4 | New bandit: pushes 5, 10, 15, -10 |
| 5–10 | New bandit: area magnitude .25, .5, .75, .99, 1, 1.5; radius 800 |
| 11 | Flame atronach (No Knockdowns race): force 15 |
| 12–13 | New bandit: 1 or 5 seconds of paralysis |
| 14 | Try constraints standing, down, then removing after recovery, using a separate corpse's hand as the other endpoint |
| 15–16 | ForceAdd on a fresh living bandit; ForceRemove on that same actor |
| 17 | Kill the pushed bandit after 0.5 seconds; observe pose continuity |
| 18–19 | Fresh pushed actor, save request during fall; then manual reload/unload readings |
| 20 | Request a save containing reports |
| 21–22 | Mammoth / dwarven centurion: force 15 |

Save results go in `tests/results/`, then run
`target/debug/vrm-tool test-results tests/results`.

Pages (`ResultPage1` etc.) contain at most 20 lines each. Sampling nominally every
0.25 seconds continues until a recovery-end event or 80 samples. The script
records xyz, weapon drawn, both knock-condition values and animation events with
real timestamps. `GetKnockedStateEnum` and `GetKnockedState` are conditions, not
vanilla Papyrus natives: a harmless spell with Script-archetype effects (no
attached scripts, actor values, visuals or skill XP) probes conditions
725/107 for values 0–8. Dispatch/wait latency limits sampling precision; event
registration timestamps are independent. A missing result is reported as -1,
not interpreted as normal. Actor AI is disabled to distinguish physical travel
from walking. Repeat with AI enabled if that changes recovery behavior.

Please also observe weapon retention, front/back clip choice, whether the caller
is staggered by its area call, and death-pose continuity. Repeat area tests with
actors at other distances and the player as a target. The automated constraint
creates a separate bandit corpse for its other hand endpoint and records whether
it is dead before attempting attachment. Both endpoints then have rigid bodies
when the target is down; a standing player is not a valid actor endpoint. The
corpse and any remaining constraint are removed when changing phases. That
connection can itself change the target's motion: don't use phase 14's travel
to calibrate push forces.

Regenerate the probe after the audit corrections: earlier builds inadvertently
used Fortify One-Handed effects for state markers and a standing player as the
constraint endpoint. The corrected markers set MGEF DATA's archetype at 0x40
to Script (1), set skill/resistance/actor values to None, and have no VMAD.

No SE results have been supplied for this probe. No force mapping, timing,
legacy-state scale or living ForceAdd/Remove policy is claimed as verified.
