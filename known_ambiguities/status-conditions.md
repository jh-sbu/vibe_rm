# Platform, race, item and ownership status conditions

Implemented in `src/condition/mod.rs` (`function_value`); tests in
`src/condition/status_tests.rs`.

## Platform and gore

`IsWin32` (521), `IsXBox` (309), `IsPS3` (520) and `IsGoreDisabled` (547).

### What is known

- UESP Skyrim:Console (oldid 3628513, https://en.uesp.net/w/index.php?oldid=3628513):
  `IsWin32` "Returns true if the game is running on a Windows environment",
  `IsXBox` and `IsPS3` the same for their platforms.

### Unconfirmed choices

- This is a PC build, so `IsWin32` holds and the console checks fail. Whether
  Skyrim SE on Windows reports `IsWin32` true is documented only by the
  console wiki's description of the function, not observed.
- `IsGoreDisabled` has no description on UESP. Nothing here sets a gore
  setting, so it is always false (gore shown).
