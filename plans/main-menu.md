# Follow-on: the main menu

**Status: planned.** Separate from the journal System page; ROADMAP New Game.

1. Read the installed `startmenu.swf` contract, translations and required
   callbacks. Establish Continue, New, Load, Credits and Quit behavior using
   permitted public sources/assets. Exclude Bethesda.net/Creations features.
2. Add an application startup/menu state, keeping the developer cell/world
   launch and `--load` paths available. Continue selects the newest compatible
   native save; Load reuses metadata, character filtering and screenshot APIs.
   Confirm selection/error behavior against the movie before deciding fallbacks.
3. Implement Quit to Main Menu as a complete session teardown: pending saves,
   VM, actors/cells, audio, transient menus and callbacks. Keep user settings
   and control bindings. Replace the System page's inert quit-list index with
   its actual Main Menu entry and enable the callback only once teardown works.
4. Connect New to a separately researched new-game initialization/placement
   path. Its character identity is created once, then retained through saves.
   Do not treat the current developer launch as evidence for MQ101 or Helgen.
5. Verify Continue with no/obsolete/corrupt saves, Load failures, repeated
   game-to-main-menu cycles, Credits/Quit and a new game's independent identity.
   Use synthetic state tests and offscreen captures; commit no game assets.

Keep this plan's unanswered questions here and implemented evidence-based
fallbacks in `known_ambiguities/`. Run `cargo fmt` before every commit.
