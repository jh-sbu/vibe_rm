# Follow-on: loading screens

**Status: planned.** Separate from the completed journal System page.

1. Research the installed `loadingmenu.swf` contract and vanilla `LSCR`
   conditions/models using game assets and the local UESP index. Document
   calls, callbacks, selection eligibility and text tags without committing
   assets or disassembly. Resolve timing questions with public information
   or a user-run SE observation.
2. Add an application loading state around native save loads and load doors.
   Keep movie advancement and drawing responsive while loading; commit the
   frame/state boundary separately from asynchronous streaming work. Preserve
   the active game if preflight validation fails, and show a recoverable error
   when loading fails.
3. Supply eligible LSCR text/model presentation and the movie's progress and
   close handshakes. Report loading-menu state to the existing audio director
   so `PDLC` categories pause correctly, including on failure/cancellation.
4. Verify with synthetic eligibility tests, a native-save load, interior and
   exterior doors, a failed load and offscreen captures. Keep screenshots and
   generated fixtures under ignored `tests/assets/` or `/tmp`.

Do not infer the original progress weighting, random selection or dwell time
from engine binaries. Record implemented fallbacks in `known_ambiguities/`;
keep unanswered questions here. Run `cargo fmt` before every commit.
