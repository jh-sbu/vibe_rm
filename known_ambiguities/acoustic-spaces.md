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
