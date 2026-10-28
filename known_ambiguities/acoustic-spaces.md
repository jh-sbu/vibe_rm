# Acoustic-space choices

Research and source links: research/audio/acoustic-spaces.md. XPRM supplies
placed volumes; base OBND is a fallback. Rotated/scaled containing markers
supersede a cell's XCAS; we choose smallest volume, ties by reference ID.
Overlap precedence is not publicly specified. Interiors fall back to XCAS;
exteriors only use placed markers. Missing spaces use DOBJ RVBT with no bed.
Reference volume transforms are taken from the placed asset; moving acoustic
markers at runtime has not been observed and is not modeled.

Space beds crossfade over 1.5 seconds. No inspected source supplies the time;
this is a provisional listening choice. RDAT is resolved only in interiors
and is fed to the region-sound scheduler in step 5d.

## RDSA chance and placement

The survey finds chance-1 looping beds alongside fractional one-shot chances.
We admit a loop probabilistically when it first becomes eligible, keeping it
until its source/weather conditions stop matching. One-shots roll their chance
once per second, independently, and do not accumulate checks in menus. The
interval is not sourced. SNDR conditions are re-evaluated and weather flags
filter both kinds. Duplicate sounds from overlapping regions play once.
Interior space RDAT supplies its region; exterior cell XCLR supplies regions.
Positioned one-shots choose an azimuth around the listener at the midpoint of
their output-model distances; 2D output models ignore that position. Neither
interval nor placement is publicly specified and needs listening comparison.
