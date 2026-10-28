# Acoustic-space and output survey (2026-10-27)

`vrm-tool aspc-survey <Data>` finds 52 ASPC records, 761 interior cells,
769 cells with XCAS, 148 placed ASPC references, 47 with nonzero rotation.
All surveyed placed markers have a 32-byte XPRM primitive. Their base OBND
is a generic marker bound, not the placed acoustic volume. For example
00105888 uses 0010273E: the base OBND is +/-512, while XPRM's extents differ.
Therefore the implementation corrects the plan by using placed XPRM extents,
with OBND only as a fallback, and the existing reference transform convention.

[REFR revision 2354868](https://en.uesp.net/w/index.php?oldid=2354868) documents
XPRM's first three floats as half bounds. [ASPC revision 3089064](https://en.uesp.net/w/index.php?oldid=3089064)
documents XCAS-related space data. Reproducible surveys inspect assets only.

SOPM ONAM bytes have values 0, 50, 75, 100 (counts 431/132/8/269), supporting
percentage levels. Region RDSA contains looping beds with chance 1 and
one-shot gusts/birds/crumbling with fractional chances (e.g. 0.02–0.10).
The two kinds should not be treated as a single weighted playlist.
