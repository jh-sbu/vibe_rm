# Voice priority and budget

[SNDR revision 2309034](https://en.uesp.net/w/index.php?oldid=2309034) and the
BNAM asset survey expose priority but do not specify the voice limit or
comparison direction. `sndr-stats` found priorities 64/68/88/100/128/192/255
(counts 529/4/53/27/2338/228/7). Vocal/shout descriptors commonly use 64,
regional wind beds/gusts 192: we choose smaller numbers as higher priority.

The provisional cap is 64 mixed non-exempt voices. Ties prefer current
smoothed gain times category gain times falloff; exact ties prefer earlier
voice IDs. Unselected one-shots are dropped, loops become virtual and keep
sample time (including rate and loop regions), then return when a slot frees.
Paused voices hold time and do not consume audible slots. Descriptor-free
music and dialogue are exempt. The original cap, exact priority direction,
and virtualization/fade policy have not been measured in SE.

Weather beds are **not exempt**: they keep their descriptors' priorities (wind
commonly 192) and can become virtual when higher-priority sounds fill the
budget. Their clocks keep advancing and they return when a slot frees. The
public SNDR priority documentation and asset survey do not establish a weather
exemption; applying the same budget is our provisional choice, not observed
SE behavior. The earlier unconditional weather exemption is no longer used.
