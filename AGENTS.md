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

`known_ambiguities/` holds ONLY decisions made after attempting to check sources,
where that research did not yield a concrete answer. Do NOT put any todo items in
`known_ambiguities/`.

Unfinished work should be noted in ROADMAP or the particular planning document
being worked on.

Do NOT commit anything in `refs/`; the references found there are external and not
for committing. Anything that needs to be committed should be done elsewhere.
