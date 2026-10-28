# System page choices

## Save rows and numbering

The installed SaveLoadPanel reads text, fileNum, name, raceName, level,
playTime and dateString and truncates text to 20 characters. It does not define
how the host builds the label or orders files. Pending the SE screenshot in
plans/system-page.md, manual rows use “name - location - game date”, newest
write first, ties by descending filename. Quick/autosaves use their type label.
META retains a global increasing number on overwrite; new manual files use
“Save N - name location”. Legacy rows use filename and unknown metadata.
These native .vrs filenames are our format, not a claim about ESS naming.

## Save on Pause

The movie provides a 5/10/15/30/45/60/Disabled stepper; launcher preferences
have bSaveOnPause=0 and fAutosaveEveryXMins=15. Until direct SE observation,
opening either the journal or tween after the interval requests an autosave.
Use monotonic elapsed real time since any successful save, including time in
menus, reset on load. Repeated frames of an open menu do not repeat requests.

## Autosave rotation

The plan reports SE players seeing Autosave1–Autosave3; the asset/INI contract
does not specify retention. Choose three native slots, autosave1–autosave3,
using an empty slot first then replacing the least recently modified. Preserve
legacy autosave.vrs and manual/quicksave slots. Exact SE count awaits observation.

## Missing preference default

Neither installed preference file supplies fSpecularLODStartFade. Use 2000,
the Skyrim_Default.ini fSpecularLODMaxStartFade value, pending SE observation.
The other defaults and their sections come directly from the installed files
as recorded in research/ui/system-page.md and settings.rs SPECS.
