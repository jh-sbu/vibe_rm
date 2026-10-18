# The menus' cursor (`cursormenu.swf`)

Implemented in `src/swf_ui/cursor_menu.rs`, with pointer tracking and drawing
in `src/swf_ui.rs` and window cursor visibility in `src/app.rs`. Only game
assets and the repository's vendored public Ruffle source were examined;
no engine executable, engine binary data, leaked documents or SDK-source
mirrors were inspected. No assets or asset disassembly were added to the repo.

## Asset evidence

The installed game's `Interface/cursormenu.swf`, already extracted under
ignored `spike/swf/data`, is a 640 by 480 movie at 24 fps. Its root ActionScript
creates a `mouseListener`, registers it with `Mouse.addListener`, and copies
the root `_xmouse` and `_ymouse` into `_root.mc_Cursor._x` and `._y` on mouse
movement. The clip's authored origin is the arrow tip; it starts at (320, 240).
There are no native calls or cursor visibility callbacks in this movie.
Inspection used the repository's `swfspike dis` and rendering harness; all
inspection output and screenshots went to `/tmp`.

The vendored Ruffle dispatches the AS2 `onMouseMove` listener before updating
its stored mouse position (`vendor/ruffle/core/src/player.rs`). The local
asset probe reproduced the resulting one-event lag. The cursor therefore
sets the same clip coordinates directly. `ui_swf::Menu::position_clip` uses
Ruffle's actual inverse stage view matrix, so the displayed tip follows the
same physical window pixels as menu hit testing, including scaling, centered
letterbox offsets and positions outside the movie's nominal stage.

## Unresolved behavior and implementation choices

- **Native scaling.** The movie does not establish the original engine's
  viewport alignment, cursor size at each resolution, DPI policy or possible
  cursor-size settings. The implementation retains the movie's default
  centered, aspect-preserving stage scaling rather than inventing native
  cursor dimensions. A controlled original-game comparison is still needed.
- **Visibility policy.** The movie does not say which menus show the cursor,
  whether keyboard/gamepad input hides it, or where it is placed on opening a
  menu. It is shown while an implemented SWF menu takes input and the pointer
  has a known position in the focused, uncaptured window. It is hidden in
  gameplay, while the console is open, and after leaving or losing focus;
  returning requires a fresh mouse move. The OS cursor is hidden only when
  the SWF cursor is submitted for rendering. Missing movies or a missing
  `mc_Cursor` clip retain the OS cursor. This is a practical fallback policy,
  not a claim about original-engine cursor lifecycle behavior.
- **Mods and alternate art.** The supported contract is the stock root-level
  `mc_Cursor` clip. A replacement movie with numeric clip coordinates but
  hidden/empty art or different root transforms may still suppress the OS
  cursor; arbitrary replacement movie validation is not implemented.
- **Offscreen screenshots.** `--menu-key click:x,y` supplies a known pointer
  position and draws the cursor after the other UI if a SWF menu remains up.
  Keyboard-only screenshots have no pointer position and retain no cursor art.
- **Gamepad.** This does not add virtual pointer movement or gamepad-driven
  cursor visibility; gamepad support remains a separate roadmap item.

Three synthetic tests cover initial/updated positions, permission and menu
gates, leaving/re-entering, focus loss, and non-finite positions. The opt-in
`installed_cursor_movie_positions_survive_resize` test uses locally supplied
Interface assets and a GPU to check the actual clip's position and rendered
texture at 1280x720 (including outside its 4:3 stage), 1920x1080, 720x1280 and
640x480. It requires `VRM_CURSOR_DATA` and `--include-ignored`; no game assets
are embedded in the tests. These checks validate this implementation's
coordinates and lifecycle policy, not the original game's exact appearance.
