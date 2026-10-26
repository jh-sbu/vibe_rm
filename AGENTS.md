# Spell visuals implementation and research

Implement spell visuals according to the spell visuals plan, subject to all of
these constraints:

- Use only publicly available sources or game assets for research. Do not use
  leaked documents, even if publicly accessible. Public community research that
  deciphers engine behavior is permitted, subject to the restrictions below.
- Store external public references in `refs/`, which must remain uncommitted.
- If a public source is source code, it must be open source. Any use of that code
  must comply with its license terms and be compatible with this project's
  license. Check both before using it; public availability alone is insufficient.
- Game assets, including binary asset formats, may be examined, but must not be
  included in this repository, including under `refs/`.
- Do not examine the game engine's binary data at all: do not inspect engine
  executables, libraries, or engine code/data in memory. This prohibition does not
  extend to binary game assets.
- Attempt to resolve ambiguities using permitted public information or game-asset
  examination. If research remains inconclusive and a choice is made to get
  functionality working, record the choice, supporting research, and remaining
  uncertainty in `known_ambiguities/`. Record open questions without a decision
  elsewhere, in ROADMAP or the relevant planning document.
- Separate implementation work across commits as appropriate.

# Local UESP reference

For Skyrim mechanics and file-format research, use the local UESP index before
trying the UESP website. The current-page dump and generated SQLite database live
under ignored `refs/`; do not read or search the giant XML directly.

```sh
python3 scripts/uesp.py search "armor rating"
python3 scripts/uesp.py search "Mod File Format NPC" --namespace "Skyrim Mod" --title
python3 scripts/uesp.py read "Skyrim:Armor" --sections
python3 scripts/uesp.py read "Skyrim:Armor" --section "Armor Rating"
```

Search defaults to Skyrim, Skyrim Mod, Dragonborn, and Mod. Use `--namespace`
(repeatable) for other namespaces, including Template and Skyrim talk, or `--all`
for the whole wiki. Punctuation is ignored, so record signatures like `NPC_`
are searched as `NPC`. Talk pages can provide research leads, but distinguish their
discussion from documented behavior. Read follows redirects, prints revision
links, and returns bounded, numbered lines; use `--start-line` to continue.

The stored text is original wikitext: templates and Lua modules are unexpanded.
Read referenced Template/Module pages when needed. This is the en.uesp.net dump,
not a copy of the separate Creation Kit wiki. Cite the page's permanent revision
link in research notes and distinguish source claims from inference or behavior
verified in Skyrim SE.

If the database is absent, build it using the downloaded dump as described in
`scripts/README.md`. Commit tooling and documentation, not the dump or database.

# Research decisions and unfinished work

`known_ambiguities/` holds ONLY choices made to get functionality working after
attempting to check sources, where that research did not yield a concrete answer.
Do NOT put todo items or open questions without a decision in
`known_ambiguities/`.

Unfinished work and open research questions without a decision should be noted in
ROADMAP or the particular planning document being worked on.

Do NOT commit anything in `refs/`; the references found there are external and not
for committing. Anything that needs to be committed should be done elsewhere.
