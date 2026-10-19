# Local UESP search

`uesp.py` requires Python 3.9+ with SQLite 3.35+ and FTS5 support and no third-party Python
packages. All commands work offline. Paths to the default database are relative
to the repository, regardless of the current working directory.

Build the index from the manually downloaded **current-page** MediaWiki dump:

```sh
python3 scripts/uesp.py import refs/uespwiki-2026-10-11-current.xml
# A .xml.bz2 file can be imported directly, without extracting it first:
python3 scripts/uesp.py import refs/uespwiki-2026-10-11-current.xml.bz2
```

Use whichever source file you have. The importer streams pages, retains all
namespaces and original wikitext, and builds title and full-text indexes in
`refs/uesp.sqlite3`. It preserves page IDs, revision IDs, timestamps, content
models, redirects, namespace names, and source-file metadata. It rejects dumps
with multiple revisions per page. XML escapes are decoded by the XML parser;
wiki markup is preserved. Templates, Lua, and other wiki code are not executed.

Re-run import to refresh from a newer dump, or when the tool reports an
unsupported schema after an update. A temporary database is built and
checked alongside the destination, then atomically replaces it. A failed import
leaves the existing database intact. Allow space for the source, the new database,
and any existing database during rebuilding. A forcibly killed process may leave
a `uesp.sqlite3.*.tmp` file that can be removed after confirming no import is running.
Queries open the database read-only. `refs/` is already ignored by Git.

```sh
python3 scripts/uesp.py info
python3 scripts/uesp.py search "armor rating" --limit 5
python3 scripts/uesp.py search "Mod File Format" --namespace "Skyrim Mod" --title
python3 scripts/uesp.py search "armor" --namespace Skyrim --namespace "Skyrim talk"
python3 scripts/uesp.py search "GetActorValue" --all
python3 scripts/uesp.py search '"armor rating" OR damage' --query-syntax
python3 scripts/uesp.py read "Skyrim:Armor" --sections
python3 scripts/uesp.py read "Skyrim:Armor" --section "Armor Rating"
python3 scripts/uesp.py read "Skyrim:Armor" --start-line 81 --lines 80
python3 scripts/uesp.py export "Skyrim:Armor" /tmp/uesp-armor.wiki
```

Search defaults to the Skyrim, Skyrim Mod, Dragonborn, and Mod namespaces.
Namespace names are case-insensitive; numeric IDs also work (`info` lists them).
`--all` includes everything, including talk, users, templates, and other games.
Plain queries require all whitespace-separated terms; punctuation follows FTS5's
Unicode tokenizer. Use `--query-syntax` for phrases, OR, NOT, NEAR, and prefixes
(e.g. `animat*`). Record signatures lose their punctuation, so `NPC_` is searched
as `NPC`; add `--title` with the page name (`Mod File Format NPC`) to find a record page.
Ranking adds a title score, weighted above body text and normalized by title length
(a separate title index), to a body score; partial title matches earn partial credit,
so a long article is not outranked by short stubs that share a word. Redirects
rank by title alone.
Redirect aliases with the same destination and section are collapsed within a
bounded candidate pool (20 times the requested limit).
Search returns 5 snippets by default, at most 50; it does not expand redirects or
templates, so use `read` to inspect a result.

Read tries the exact title first, then a case-insensitive lookup with underscores
treated as spaces. It follows redirects (including section fragments), detects
loops, and prints page/revision metadata and a permanent source URL. Use
`--no-redirect` to inspect redirect source. Section matching uses the literal wiki
heading text, ignoring case and normalizing underscores/spaces. A section includes
its subsections and ends at the next heading of equal or higher level. Headings
are detected from source lines; they are not a full MediaWiki rendering. For
duplicate or markup-heavy headings, use `--sections` and numbered line ranges.
An explicit `--section` takes priority over redirect fragments.

Read returns 80 lines by default, at most 200, with a 24,000-character body budget,
and truncates individual lines after 2,000 characters. `--start-line` always uses the original page's line numbers,
even with `--section`. Heading lists are also bounded by `--lines`. `export` writes
the complete original page text to a **new** file, without following redirects;
it refuses to overwrite files. This provides access to long tables without
flooding an agent's context.

The content is wikitext, not rendered HTML or Markdown. Template-generated facts
may require reading the corresponding `Template:` or `Module:` pages. The dump
does not supply image binaries or the separate ck.uesp.net wiki. Preserve revision
links in research notes, and keep original source text alongside any later derived
summaries or rendering.

To use another database, put the global option **before** the command:

```sh
python3 scripts/uesp.py --db /tmp/uesp.sqlite3 import refs/uespwiki-2026-10-11-current.xml
python3 scripts/uesp.py --db /tmp/uesp.sqlite3 search "armor"
```

Run the fixture tests with:

```sh
python3 -m unittest discover -s scripts -p 'test_uesp.py'
```
