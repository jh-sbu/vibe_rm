# Fifteen native continuity fixes

Advances Roadmap Next 9 (saves), with one independent commit per item.
Native policies do not establish Skyrim SE or original `.ess` parity.
Synthetic fixtures only; no game assets or engine binaries are needed.

Planned items: trespass progression; private homes; owner warnings and cadence;
guard pursuits; resistance and yield; active jail sentences; jail day count;
delayed activations; random streams; critical-hit memory; player sneak stance;
attack recovery; map camera preference; active image-space modifiers; script fades.

Completed items follow. Final validation is recorded at the end.

1. Trespass warning level and next escalation deadline (`TRSP`). Implemented replacement restoration, legacy defaults, strict decoding and a synthetic capture/read/apply regression.

2. Package-owned private homes (`PHOM`). Implemented replacement restoration, legacy defaults, strict decoding and a synthetic capture/read/apply regression.

3. Owners asking the player to leave and their next dialogue deadline (`HWAR`). Implemented replacement restoration, legacy defaults, strict decoding and a synthetic capture/read/apply regression.

4. Guards approaching for arrest and remaining bark cooldown (`GPRS`). Implemented replacement restoration, legacy defaults, strict decoding and a synthetic capture/read/apply regression.

5. Resisted factions, one-yield gate and sheathed-weapon timer (`ARRY`). Implemented replacement restoration, legacy defaults, strict decoding and a synthetic capture/read/apply regression.

6. Jail sentences, owed bounty and queued prison transport (`JSEN`). Implemented replacement restoration, legacy defaults, strict decoding and a synthetic capture/read/apply regression.

7. Accumulated jail days read by GetDaysInJail (`JDYS`). Implemented replacement restoration, legacy defaults, strict decoding and a synthetic capture/read/apply regression.

8. Ordered delayed parent activations and absolute deadlines (`ACTQ`). Implemented replacement restoration, legacy defaults, strict decoding and a synthetic capture/read/apply regression.
