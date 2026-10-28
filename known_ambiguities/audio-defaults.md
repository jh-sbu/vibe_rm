# Sparse default-object records

[UESP DOBJ revision 3089074](https://en.uesp.net/w/index.php?oldid=3089074)
describes a keyed lookup table. Installed SE's winning SurvivalMode.esl DOBJ
has 2928 DNAM bytes, mostly zero-key slots, and Survival-specific keys; it
contains none of the audio keys. The plan's winning-record-only rule would
therefore erase the base game's audio defaults. We choose last named value
per key across load order, skipping zero keys and preserving named nulls.
The data supports incremental keyed updates but the original engine's exact
merge behavior has not been observed. This is a necessary implementation
choice after checking public documentation and the assets.
