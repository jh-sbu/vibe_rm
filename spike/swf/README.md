# SWF UI spike (Ruffle)

A spike: Skyrim's Interface `.swf` menus (Scaleform, ActionScript 2) run through
[Ruffle](https://github.com/ruffle-rs/ruffle) v0.7.1 with Scaleform changes
(`vendor/ruffle`), outside the engine. The harness isn't part of the engine's
workspace.

## Run

```sh
./setup.sh      # build the harness against vendor/ruffle (its `spike` feature on)
./extract.sh    # Interface .swf/.txt from Skyrim - Interface.bsa into data/ ($SKYRIM_DATA or the Steam default)
./run-hud.sh    # drive the HUD; out/hud.png, out/hud-end.png and timings
./view.sh hudmenu      # the HUD in a window, in real time (view-hud.sh is the same)
./view.sh messagebox   # a message box answered with the mouse, keys or gamepad
./view.sh racesex_menu # character creation's name popup: type a name
```

## Watching it live

`view.sh <menu>` opens a 1280×720 window playing `data/interface/<menu>.swf` in
real time (it calls `swfspike view`). Over SSH, prefix it with
`WAYLAND_DISPLAY=wayland-0`; the window then opens on the machine's own monitor.

The mouse, keyboard and gamepad go to the movie for every menu. Q cycles the
quality and F10 quits. Esc no longer quits, because it's Cancel in the menus.
After the menu name, `view.sh` takes the same `call:` / `cb:` / `set:` ops as
`run`, applied once at start.

### Input

The menus' CLIK `InputDelegate` reads Flash key codes and maps them to
navigation:

- 38/40/37/39: the arrows;
- 13: ENTER; 27: ESCAPE; 9: TAB (SHIFT_TAB with shift); 8: BACK;
- 36/35/33/34: home, end, page up, page down;
- 96–107: Scaleform's gamepad codes (GAMEPAD_A, B, X, Y, L1, R1, L2, R2, L3,
  R3, START, BACK).

The game's "Menu Mode" bindings come from `Interface/Controls/PC/controlmap.txt`:

- Up / Down / Left / Right are the Forward / Back / Strafe keys (W/S/A/D);
- Accept is Activate (E); Cancel is Tween Menu or Pause (Tab, Esc);
- on the gamepad: A accepts, B cancels, the d-pad and left stick navigate.

How the game turns these into key codes isn't public. The viewer sends:

| Input | Movie sees |
|---|---|
| W / S / A / D, arrows | the arrow codes (W/A/S/D also as themselves, after) |
| E, Enter | 13 ENTER (E also as itself) |
| Tab, Esc | 9 TAB, 27 ESCAPE |
| other letters, digits, space, shift, backspace, home/end/page | themselves |
| gamepad A | 13 ENTER |
| gamepad B, X, Y, LB, RB, LT, RT, Start, Back | 97–103, 106, 107 |
| d-pad, left stick (past half way) | the arrow codes |

Three of these are guesses:

- **Gamepad A as ENTER.** CLIK buttons only accept ENTER, so with
  GAMEPAD_A (96) a gamepad couldn't accept a message box.
- **Letters both ways.** W/A/S/D/E go both as their menu meaning and as
  themselves: the message box takes Y, N and A as shortcuts for Yes, No and
  Yes to All.
- **L3 / R3.** Codes 104/105 aren't sent; Ruffle has no buttons for them.

**Text input mode.** While an input text field has the focus (the game's
text input mode, which a menu asks for with `SetAllowTextInput`):

- keys go through as themselves, without the menu-mode translation (typing "e"
  isn't Accept); Enter, Esc, Tab and the arrows still reach the menu;
- typed characters (winit's key text, so shift and layouts work) are Ruffle
  `TextInput` events;
- Backspace, Delete, the arrows (with Ctrl for words, Shift to select),
  Home/End and Ctrl+A/C/V/X are `TextControl` edits;
- the viewer's own letter keys (Q, the drivers') are off; F2 and F10 still work.

The gamepad is read with `gilrs`. It hasn't been tried with a real gamepad
yet; the harness's `pad:` op injects the same events Ruffle gets.

### hudmenu

Keys play the game's side:

| Key | Does |
|---|---|
| 1 / 2 / 3 | drain health / magicka / stamina by 15 (the meter fades in, animates, fades out) |
| R | restore all three |
| Left / Right (held) | turn, 90° a second; the compass follows |
| M | next notification message |
| L | next location name |
| S | subtitle on/off |
| C | crosshair target (Iron Sword) on/off |

### messagebox

The game's side opens a question:

- `SetMessage(text, html)` on `_root.MessageMenu`;
- `setIsVertical`, then `setButtons(false, label, label, ...)`, through the
  menu's `GameDelegate` callbacks.

The first button starts focused (`setButtons(true, …)`). Hovering or moving
the focus highlights a button with arrows either side, and the menu asks for
the `UIMenuFocus` sound. Answers:

- clicking a button, or Accept (E, Enter, gamepad A) on the focused one, makes
  the menu call `buttonPress(index)`;
- Cancel (Tab, Esc, gamepad B) gives the cancel option once
  `setIsCancellable(true, index)` is set;
- Y / N answer Yes / No.

The viewer prints the answer and opens the next question 0.8 s later. F2 skips
to the next question. The questions lay their buttons out side by side, as the
game does; one has three buttons and an HTML line break.

The real game closes the menu on `buttonPress` and sends the index to whoever
asked (a script's `Message.Show`, the wait dialog, ...). Here the box is only
refilled.

### racesex_menu (the name popup)

Character creation's name popup is `TextEntry.swf`'s `TextEntryClip`, which
`racesex_menu.swf` imports and shows as `NameEntryInstance`. The game's side:

1. `_root.InitExtensions()` and `_root.SetPlatform(0, false)` (PC: keyboard art);
2. `SetNameText` / `SetRaceText` on `RaceSexMenuBaseInstance.RaceSexPanelsInstance`
   (the NAME and RACE labels; here "Prisoner" and "Nord");
3. the `ShowTextEntry(true)` then `ShowTextEntryField` callbacks. The popup
   clears and focuses its field, fades in and asks for `SetAllowTextInput`.

Answers:

| Input | The menu calls |
|---|---|
| Enter, or clicking Accept | `ChangeName(name)`; once faded out, `ChangeName()` again |
| Tab, or clicking Cancel | `ChangeName()` |
| Accept with the field empty | nothing (the name must not be empty) |

The second, empty `ChangeName` after an accept is the menu's own doing: its
fade-out handler reads `this._TextEntryField` while `this` is the popup itself.
The viewer prints the name, puts it in the NAME label and opens the popup again
1.5 s later. F2 opens it by hand.

The rest of the race menu shows its placeholder contents ("FILTER",
"WidePanelEntry"): the game fills those lists, and the spike doesn't.

### Timings

Each second the title bar and stdout show:

- fps;
- `movie frames N at X ms`: ticks that ran a movie frame (24 a second), and their average cost;
- `other ticks`: ticks between movie frames;
- `render+present`: the render call, including the swapchain present. With
  vsync this is mostly waiting for it.

A second stdout line splits each tick into sockets, timers and `update()`, and
`update()` into its mouse hit-test and its GC step.

Environment switches:

- `SPIKE_NOVSYNC=1`: presents uncapped (Immediate or Mailbox), so
  `render+present` shows the real cost.
- `SPIKE_TICK_DUE=1`: calls `Player::tick` only when a movie frame is due.
  Every `tick` call pays a mouse hit-test and a GC step (about 0.55 ms
  together), frame or not.
- `QUALITY`, `SPIKE_INLINE_BLENDS`, `SPIKE_NOFILTERS`: as for `run-hud.sh`.

Keys aren't forwarded to the movie, so the menus' own keyboard and gamepad
navigation (CLIK `handleInput`) can't be tried yet.

`run-hud.sh` options (environment):

- `QUALITY=low|medium|high|best`: Ruffle's stage quality (MSAA 1/2/4/4). Default high.
- `SPIKE_INLINE_BLENDS=1`: draws the Add-blend objects inline rather than through
  a full-stage offscreen surface. This only measures the offscreen cost; the
  result isn't correct.
- `SPIKE_NOFILTERS=1`: drops filters (drop shadows, glows). Measurement only.

The `split` line gives per-frame averages over 600 frames:

- `run_frame`: ActionScript and timeline (CPU).
- `render`: building draw commands (CPU).
- `gpu-wait`: blocking until the GPU finishes the frame. This includes a fixed
  sync round trip (about 0.4 ms on an empty stage on the Barcelo iGPU) that the
  engine wouldn't pay.
- `cache redraws`: filtered objects re-rendered, a running total.

The harness calls `run_frame` once per rendered frame. In the engine,
`tick(dt)` would advance the movie at its own 24 fps.

## The harness

```sh
B=swfspike/target/release/swfspike
$B dis   data/interface/hudmenu.swf          # AS2 disassembly (the game<->menu contract)
$B fonts data/interface/gfxfontlib.swf       # fonts, imports, exports
$B run   data/interface/<menu>.swf out.png [ops...]
```

`run` ops, applied in order after 5 frames:

- `call:path|arg|...`: calls an ActionScript function, as the game's Invoke does.
  Arguments: numbers, `true`/`false`, `null`, `undefined`, anything else is a string.
- `cb:name|arg|...`: calls a handler the menu registered with
  `GameDelegate.addCallBack`, through its ExternalInterface `call` callback, as
  the game does.
- `move:x|y`, `click:x|y`: the mouse, in stage pixels.
- `key:KeyD`, `keydown:Enter`, `keyup:Enter`: a key by its winit name, sent as
  the viewer would send it.
- `pad:south`: a gamepad button pressed and released (`east`, `dpad-left`, ...).
- `type:Lydia`: text typed into the focused field.
- `text:Backspace`, `text:ArrowLeft+shift`, `text:KeyA+ctrl`: an editing key.
- `tree:`: prints the display tree (names, character ids, positions). It needs
  the `avm_debug` build, like `avmdebug:`.
- `avmdebug:on|off`: Ruffle's per-action ActionScript trace. It needs the build
  with `--features avm_debug`, e.g.
  `cargo build --release --features avm_debug --target-dir target-avmdebug`
  in `swfspike/`, run with `RUST_LOG=ruffle_core::avm1=debug`.
- `set:path|value`, `get:path`: SetVariable / GetVariable.
- `frames:N`: runs N frames.
- `png:file`: captures a frame.
- `quality:q`: sets the stage quality.
- `blends:`: one frame's offscreen blend surfaces, by mode.
- `split:N`, `bench:N`: timings.

Calls from the menu to the game (`GameDelegate.call`, which goes through
`ExternalInterface.call(name, uid, ...args)`) print as `[game] ...`. The capture
is composited over a dark grey backdrop.

## The Ruffle changes

`vendor/ruffle` is upstream v0.7.1 (see its `VENDOR.md`); each change is a
commit of its own on top (`git log -- vendor/ruffle`):

1. `Player::invoke_avm1`, `get_avm1_variable`, `set_avm1_variable`: the host
   calls into the movie (Ruffle's avm1 module is private).
2. A translation table (`Player::set_translations`): a text field whose plain
   text is a `$KEY` shows its translation, HTML fields included.
3. `gotoAndStop` / `gotoAndPlay` with a fractional frame number truncates it.
   Flash Player ignores such a call; Skyrim's meters depend on it.
4. Imports:
   - nested imports (`ItemCard.swf` importing from `gfxfontlib.swf`): the outer
     movie keeps waiting until the inner import's exports are registered
     (`finish_import_chain`);
   - an imported movie preloads to its end (`preload_import`). One `preload`
     can stop after finishing a sprite it was in the middle of, and nothing
     called it again, so the race menu waited on `TextEntry.swf` forever;
   - the importer's exports are copied into the imported movie only once that
     movie has defined all of its own characters. Before, an importer export
     took an ID first: inside the race menu, the popup's text input (id 28)
     became the race menu's category list (also id 28) and the button labels
     became static graphics. That was the "Character ID collision" errors.
5. `TextField.numLines` and `TextField.getLineMetrics(i)` for AS2. They are
   Scaleform extensions (AS3 has them); the message box sizes itself with them.
6. `scale9Grid` drawing. Ruffle stored the grid but drew the clip scaled
   whole, so the message box's selection arrows stretched into the labels and
   its frame corners stretched. A scaled clip with a grid now draws once per
   cell, masked to it: corners at their size, edges stretched one way, the
   middle both.
7. Scaleform's `Selection` and `System.capabilities` extensions, for one
   controller:
   - `getControllerFocusGroup` (0), `getControllerMaskByFocusGroup` (1),
     `getFocusBitmask`;
   - `findFocus(nav, context, loop, start, ...)`: the directional search CLIK's
     FocusHandler navigates with, on Ruffle's arrow navigation ordering
     (`loop` isn't done);
   - `Selection.numFocusGroups` and `System.capabilities.numControllers` (1).
     CLIK's `focused` setter loops over them, so without them nothing it
     focuses (the name field) got the focus.
8. Enter / Space press the focused clip even without Flash's yellow focus
   highlight, unless it's a text field, where they're text. CLIK hides the
   highlight; the message box empties its buttons' keyboard `handlePress` and
   accepts through this press.
9. `Stage.visibleRect` and `Stage.safeRect`: the stage area the viewport shows,
   as `{x, y, width, height}`. The safe rect is the whole of it, since
   Skyrim's safe-zone inset isn't known. `GlobalFunc.Lock` anchors elements to
   them; without them the race menu's bottom bar left the screen.
10. The spike's instrumentation, marked `SPIKE`, behind the `spike` cargo
    feature of `ruffle_core` and `ruffle_render_wgpu` (off by default; the
    harness turns it on):
   - the `SPIKE_INLINE_BLENDS`, `SPIKE_NOFILTERS` and `SPIKE_NOVSYNC` switches;
   - the cache-redraw and blend counters;
   - the tick timing breakdown;
   - `Player::spike_display_tree` (the `tree:` op).

The harness does the font mapping from `fontconfig.txt` itself
(`swfspike/src/fontconfig.rs`). It registers each fontlib's `DefineFont` tags
as device fonts under their `$Alias` names.

## Known issues

- Game files are lower case in the archive while the menus load mixed-case
  names; `extract.sh` symlinks the ones seen so far. A loader reading from the
  BSA case-insensitively would replace this.
- Ruffle logs "Character ID collision" when a movie imports several fonts from
  `gfxfontlib.swf` in separate import tags (the inventory's `ItemCard.swf`):
  the same font registered again. The race menu's collisions were a real bug
  (change 4).
- The crosshair's "undefined" is from guessed `SetCrosshairTarget` arguments.
- Menus other than the HUD and the message box load but stay blank until
  something opens them and feeds them data. That per-menu contract is unwritten.
- The input translation is partly guessed (see "Input"), and it hasn't been
  tried with a real gamepad.
- Every non-Normal blend mode gets a full-stage offscreen surface; this is most
  of the HUD's GPU time.
- Each `Player::tick` call hit-tests the mouse against the display list and
  steps the GC, even when no movie frame runs. The engine should tick only when
  a frame is due, and skip the mouse update for menus that take no mouse.
