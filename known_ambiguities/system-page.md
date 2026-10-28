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

## Look sensitivity

The movie slider is 0–1 in twentieths and installed preferences default to
fMouseHeadingSensitivity=0.0125. No asset specifies its mapping. Use linear
0–0.025, centered on that default; a 0.2 conversion retains the host's prior
0.0025 radians per mouse unit at default. Invert Y reverses only mouse pitch.
The HUD has FloatingQuestMarker_mc but no host floating marker population;
the floating-marker toggle is stored until that rendering path exists.

## Difficulty damage paths

UESP Damage revision 3485809 documents player/NPC tables, followers/summons,
fall damage capped above Adept and Equilibrium's health cost. Ravage Health
revision 3468118, lines 30–34, explicitly excludes its maximum-health loss.
Implement scaling once at combat damage for weapons/bashes/traps and before
magic health deltas for spells, damage poisons, hazards and dual modifiers
(including Equilibrium). Absorb explicitly bypasses scaling; recovered/held
modifiers such as Ravage, actor-value script edits, healing, drowning and
scripted Kill bypass it. These path boundaries are implementation inferences
where the broad documented rule does not enumerate the engine paths.

Use fDiffMultHPToPC{VE,E,N,H,VH,L} for player-taken damage and
fDiffMultHPByPC{VE,E,N,H,VH,L} for NPC-taken damage. Missing values use UESP's
0.5/0.75/1/1.5/2/3 and 2/1.5/1/0.75/0.5/0.25 respectively. These setting
names are inferred from the data's fDiffMultHPToPCL; only that value (3) was
found in Skyrim.esm. XP difficulty multipliers remain an open research question.

## Brightness mapping

The SWF supplies a normalized slider and launcher preferences fGamma=1.0,
without endpoint mappings. Use a linear 0.5–1.5 range centered at 1.0. Apply
pow(display_color,1/gamma) after image-space tone/contrast mapping, before
the explicit fade-to-black overlay, preserving a black loading/sleep fade.

## Distance slider mappings and transition

Installed INIs establish defaults but not each normalized slider's endpoints.
Until SE values arrive, use linear ranges in settings.rs SPECS: bound-radius
actor/item/object multipliers 1–15; grass 0–7000; shadow 0–16000; light
0–7000; specularity 0–4000; tree switch 0–9999999. Fade ordinary geometry
over the final 20 percent using stable screen-door coverage, then cull. This
transition width and endpoint policy are host choices, not established SE
shader behavior. Defaults retain the documented INI values.
