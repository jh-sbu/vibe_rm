# Main menu deviations

Deliberate deviations from Skyrim SE that the user stated on 2026-10-30. The
SE behavior here was observed by the user in Skyrim SE.

## No Creations entry

SE's main menu lists Continue, New, Load, Creations, Credits and Quit. Creations
logs in to a Bethesda service. Creation purchasing is out of scope, so this
engine leaves the entry out: `sendMenuProperties` argument 8 is false
(`src/swf_ui/main_menu.rs`).

## No achievements warning on New

SE's New asks "Start a new game?" (`sConfirmNew`). When add-ons that disable
achievements are loaded, it says so as well (`sConfirmNewMods`: "Start a new
game? Creations are currently loaded. Note: Achievements are disabled.").
Steam achievements are out of scope, so this engine always asks with
`sConfirmNew`.

## Saves that need other plugins: plugins, not Creations

When a save needs plugins that aren't enabled, SE asks: "This save relies on
Creations you have already downloaded but are currently inactive: <list>
Activate now?". When a save needs plugins that aren't present, it asks: "This
save relies on missing content. The following item(s) are missing: <list>
Would you like to search for your unidentified content in the Creations
menu?".

This engine words both prompts in terms of plugins, not Creations. For missing
plugins it asks whether to continue loading, not whether to search the
Creations menu (`Engine::load_listed_save`; `plans/saves-remaining.md`,
step 8).

The user decided on 2026-11-15 that a plugin that is in Data but not
loaded counts as missing. Such a save gets the missing-plugins prompt
(continue loading without it?) instead of SE's "Activate now?", because
activating would mean rebuilding the engine with another plugin list at
runtime.
