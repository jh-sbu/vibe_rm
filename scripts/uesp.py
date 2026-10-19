#!/usr/bin/env python3
"""Import and query a local UESP MediaWiki dump using only Python's stdlib."""

import argparse
import bz2
from datetime import datetime, timezone
import json
from pathlib import Path
import re
import sqlite3
import sys
import tempfile
from urllib.parse import quote
import xml.etree.ElementTree as ET


DEFAULT_DB = Path(__file__).resolve().parents[1] / "refs" / "uesp.sqlite3"
DEFAULT_NAMESPACES = ("Skyrim", "Skyrim Mod", "Dragonborn", "Mod")
SCHEMA_VERSION = "1"
HEADING = re.compile(r"^(={1,6})\s*(.*?)\s*\1\s*$")


def title_key(title):
    return " ".join(title.replace("_", " ").split()).casefold()


def redirect_target(page):
    target = page["redirect"]
    # MediaWiki's XML redirect attribute can omit the section in the wikitext.
    if target and "#" not in target:
        match = re.match(r"\s*#redirect\s*:?\s*\[\[([^\]|]+)", page["text"], re.IGNORECASE)
        if match:
            title, marker, fragment = match[1].partition("#")
            if marker and title_key(title) == title_key(target):
                return target + "#" + fragment.strip()
    return target


def import_dump(source, destination):
    source = source.resolve()
    destination = destination.resolve()
    if source == destination:
        raise ValueError("The database must not overwrite the source dump")
    stat = source.stat()
    destination.parent.mkdir(parents=True, exist_ok=True)
    # Build beside the destination, then replace it only after validation.
    with tempfile.NamedTemporaryFile(prefix=destination.name + ".", suffix=".tmp",
                                     dir=destination.parent, delete=False) as tmp:
        temporary = Path(tmp.name)
    db = None
    try:
        db = sqlite3.connect(temporary)
        db.executescript("""
            PRAGMA journal_mode=OFF;
            PRAGMA synchronous=OFF;
            PRAGMA cache_size=-32768;
            PRAGMA temp_store=FILE;
            CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE namespaces (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
            CREATE TABLE pages (
                page_id INTEGER PRIMARY KEY, namespace INTEGER NOT NULL,
                title TEXT NOT NULL, title_key TEXT NOT NULL,
                revision_id INTEGER, timestamp TEXT, redirect TEXT,
                model TEXT, text TEXT NOT NULL
            );
            CREATE VIRTUAL TABLE pages_fts USING fts5(
                title, text, content='pages', content_rowid='page_id',
                tokenize='unicode61'
            );
        """)
        opener = bz2.open if source.suffix == ".bz2" else open
        count = 0
        with opener(source, "rb") as stream:
            context = ET.iterparse(stream, events=("start", "end"))
            _, root = next(context)
            if root.tag.rsplit("}", 1)[-1] != "mediawiki":
                raise ValueError("Expected a MediaWiki XML dump")
            prefix = root.tag[:root.tag.index("}") + 1] if "}" in root.tag else ""
            for event, node in context:
                if event != "end":
                    continue
                if node.tag == prefix + "siteinfo":
                    namespaces = node.find(prefix + "namespaces")
                    if namespaces is not None:
                        db.executemany("INSERT INTO namespaces VALUES (?, ?)",
                                       [(int(n.get("key")), n.text or "") for n in namespaces])
                    db.execute("INSERT INTO metadata VALUES ('base_url', ?)",
                               (node.findtext(prefix + "base") or "",))
                    root.remove(node)
                elif node.tag == prefix + "page":
                    def field(name):
                        return node.findtext(prefix + name)

                    revisions = node.findall(prefix + "revision")
                    if len(revisions) != 1:
                        raise ValueError("Expected exactly one revision per page (a current dump)")
                    revision = revisions[0]
                    title = field("title")
                    redirect = node.find(prefix + "redirect")
                    db.execute("INSERT INTO pages VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)", (
                        int(field("id")), int(field("ns")), title, title_key(title),
                        int(revision.findtext(prefix + "id")),
                        revision.findtext(prefix + "timestamp"),
                        redirect.get("title") if redirect is not None else None,
                        revision.findtext(prefix + "model"),
                        revision.findtext(prefix + "text") or "",
                    ))
                    count += 1
                    root.remove(node)
                    if count % 25000 == 0:
                        db.commit()
                        print(f"Imported {count:,} pages…", file=sys.stderr, flush=True)
        if not count:
            raise ValueError("Dump contains no pages")
        db.executemany("INSERT INTO metadata VALUES (?, ?)", {
            "schema_version": SCHEMA_VERSION,
            "source": source.name,
            "source_bytes": str(stat.st_size),
            "source_mtime_ns": str(stat.st_mtime_ns),
            "imported_utc": datetime.now(timezone.utc).isoformat(),
            "page_count": str(count),
        }.items())
        db.commit()
        print("Building title and full-text indexes…", file=sys.stderr, flush=True)
        db.executescript("""
            CREATE UNIQUE INDEX pages_title ON pages(title);
            CREATE INDEX pages_title_key ON pages(title_key);
            CREATE INDEX pages_namespace ON pages(namespace);
            INSERT INTO pages_fts(pages_fts) VALUES ('rebuild');
        """)
        db.commit()
        print("Validating full-text index…", file=sys.stderr, flush=True)
        db.execute("INSERT INTO pages_fts(pages_fts, rank) VALUES ('integrity-check', 1)")
        if db.execute("PRAGMA quick_check").fetchone()[0] != "ok":
            raise ValueError("Database integrity check failed")
        db.commit()
        db.close()
        db = None
        temporary.replace(destination)
        print(f"Imported {count:,} pages into {destination} "
              f"({destination.stat().st_size / 1024**2:.1f} MiB)")
    finally:
        if db is not None:
            db.close()
        temporary.unlink(missing_ok=True)


def open_database(path):
    if not path.is_file():
        raise ValueError(f"Database not found: {path}. Run the import command first.")
    db = sqlite3.connect(path.resolve().as_uri() + "?mode=ro", uri=True)
    db.row_factory = sqlite3.Row
    version = db.execute("SELECT value FROM metadata WHERE key='schema_version'").fetchone()
    if version is None or version[0] != SCHEMA_VERSION:
        db.close()
        raise ValueError("Unsupported database schema; rebuild with the import command")
    return db


def namespace_ids(db, names):
    namespaces = dict(db.execute("SELECT id, name FROM namespaces"))
    result = []
    for name in names:
        found = [n for n, label in namespaces.items()
                 if str(n) == name or label.casefold() == name.casefold()]
        if not found:
            raise ValueError(f"Unknown namespace: {name!r}. See the info command.")
        result.extend(found)
    return result


def search(db, args):
    # Plain queries are ANDed literals; raw FTS syntax is explicitly opt-in.
    expression = args.query if args.query_syntax else " AND ".join(
        '"' + word.replace('"', '""') + '"' for word in args.query.split())
    if not expression.strip():
        raise ValueError("Search query must not be empty")
    if args.title:
        expression = f"title : ({expression})"
    params = [expression]
    restriction = ""
    if not args.all:
        ids = namespace_ids(db, args.namespace or DEFAULT_NAMESPACES)
        restriction = " AND p.namespace IN (" + ",".join("?" for _ in ids) + ")"
        params.extend(ids)
    # Fetch a bounded pool so spelling aliases can be collapsed without losing
    # their useful title matches. Redirect text supplies any omitted fragment.
    params.append(args.limit * 20)
    rows = db.execute("""
        SELECT p.title, p.revision_id, p.redirect,
               CASE WHEN p.redirect IS NOT NULL THEN p.text ELSE '' END AS text,
               snippet(pages_fts, 1, '[', ']', ' … ', 36) AS excerpt
        FROM pages_fts JOIN pages p ON p.page_id = pages_fts.rowid
        WHERE pages_fts MATCH ?
    """ + restriction + " ORDER BY bm25(pages_fts, 8.0, 1.0), p.page_id LIMIT ?", params).fetchall()
    if not rows:
        print("No matches. Try fewer terms, --title, another --namespace, or --all.")
    seen = set()
    for row in rows:
        target = redirect_target(row)
        key = title_key(target or row["title"])
        if key in seen:
            continue
        seen.add(key)
        print(f"{row['title']} (revision {row['revision_id']})")
        if target:
            print(f"  Redirect: {target}")
        excerpt = " ".join(row["excerpt"].split())
        print("  " + excerpt[:1000] + (" …" if len(excerpt) > 1000 else ""))
        print()
        if len(seen) == args.limit:
            break


def lookup(db, title):
    row = db.execute("SELECT * FROM pages WHERE title=?", (title,)).fetchone()
    if row is not None:
        return row
    rows = db.execute("SELECT * FROM pages WHERE title_key=? LIMIT 2",
                      (title_key(title),)).fetchall()
    if not rows:
        raise ValueError(f"Page not found: {title!r}. Use search to find its title.")
    if len(rows) > 1:
        raise ValueError(f"Ambiguous title: {title!r}. Use the exact capitalization.")
    return rows[0]


def headings(lines):
    return [(i, len(m[1]), m[2]) for i, line in enumerate(lines)
            if (m := HEADING.match(line))]


def read_page(db, args):
    title, _, fragment = args.title.partition("#")
    section = args.section if args.section is not None else fragment or None
    visited = set()
    chain = []
    while True:
        page = lookup(db, title)
        if page["page_id"] in visited or len(visited) >= 20:
            raise ValueError("Redirect loop or chain longer than 20 pages")
        visited.add(page["page_id"])
        if args.no_redirect or not page["redirect"]:
            break
        chain.append(page["title"])
        title, _, fragment = redirect_target(page).partition("#")
        if section is None and fragment:
            section = fragment

    lines = page["text"].splitlines()
    entries = headings(lines)
    start, stop = 0, len(lines)
    if section is not None:
        matches = [h for h in entries if title_key(h[2]) == title_key(section)]
        if len(matches) != 1:
            raise ValueError(f"Section missing or ambiguous: {section!r}. "
                             "Use read --sections, then read --start-line for duplicate headings.")
        start, level, _ = matches[0]
        stop = next((i for i, depth, _ in entries if i > start and depth <= level), len(lines))
    if args.start_line is not None:
        start = max(start, args.start_line - 1)
        if start >= stop:
            raise ValueError("Start line is beyond the selected page or section")

    if chain:
        print("Redirect: " + " -> ".join(chain + [page["title"]]))
    print(page["title"])
    print(f"https://en.uesp.net/w/index.php?title={quote(page['title'].replace(' ', '_'), safe=':')}")
    print(f"Revision: {page['revision_id']} ({page['timestamp']}) | Page: {page['page_id']}")
    print(f"Permanent link: https://en.uesp.net/w/index.php?oldid={page['revision_id']}")
    source = db.execute("SELECT value FROM metadata WHERE key='source'").fetchone()[0]
    print(f"Dump: {source} | Format: {page['model']} (templates are unexpanded)")
    if args.sections:
        selected = [h for h in entries if start <= h[0] < stop]
        for i, depth, heading in selected[:args.lines]:
            print(f"{i + 1}: {'  ' * (depth - 1)}{heading[:500]}")
        if not selected:
            print("No section headings.")
        elif len(selected) > args.lines:
            print(f"More headings: use --start-line {selected[args.lines][0] + 1}")
        return
    output = []
    size = 0
    end = start
    for i in range(start, min(start + args.lines, stop)):
        line = lines[i]
        if len(line) > 2000:
            line = line[:2000] + " … [line truncated; use export for complete text]"
        numbered = f"{i + 1}: {line}"
        if size + len(numbered) + 1 > 24000:
            break
        output.append(numbered)
        size += len(numbered) + 1
        end = i + 1
    print(f"Lines {start + 1 if lines else 0}-{end} of {len(lines)}:")
    for line in output:
        print(line)
    if end < stop:
        print(f"More: read {json.dumps(page['title'], ensure_ascii=False)} --start-line {end + 1}")


def export_page(db, args):
    page = lookup(db, args.title)
    # Exclusive creation avoids overwriting an existing reference or the database.
    with args.output.open("x", encoding="utf-8") as stream:
        stream.write(page["text"])
    print(f"Exported {page['title']} (revision {page['revision_id']}) to {args.output}")


def info(db):
    for row in db.execute("SELECT key, value FROM metadata ORDER BY key"):
        print(f"{row['key']}: {row['value']}")
    print("\nNamespaces (ID, pages, name):")
    for row in db.execute("""
        SELECT p.namespace, count(*) AS pages, n.name
        FROM pages p LEFT JOIN namespaces n ON n.id=p.namespace
        GROUP BY p.namespace ORDER BY p.namespace
    """):
        print(f"{row['namespace']:4} {row['pages']:8,}  {row['name'] or '(main)'}")


def bounded_int(low, high):
    def parse(value):
        number = int(value)
        if not low <= number <= high:
            raise argparse.ArgumentTypeError(f"must be between {low} and {high}")
        return number
    return parse


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--db", type=Path, default=DEFAULT_DB, help="database path (default: refs/uesp.sqlite3)")
    commands = parser.add_subparsers(dest="command", required=True)
    importer = commands.add_parser("import", help="atomically build/rebuild from current XML or XML.bz2")
    importer.add_argument("source", type=Path)
    finder = commands.add_parser("search", help="ranked search with bounded snippets")
    finder.add_argument("query")
    scope = finder.add_mutually_exclusive_group()
    scope.add_argument("--namespace", action="append", help="namespace name or ID; repeatable")
    scope.add_argument("--all", action="store_true", help="search every namespace, including talk pages")
    finder.add_argument("--title", action="store_true", help="search only titles")
    finder.add_argument("--query-syntax", action="store_true", help="interpret query as SQLite FTS5 syntax")
    finder.add_argument("--limit", type=bounded_int(1, 50), default=5)
    reader = commands.add_parser("read", help="read a page or section, following redirects")
    reader.add_argument("title")
    reader.add_argument("--section")
    reader.add_argument("--sections", action="store_true", help="list headings with page line numbers")
    reader.add_argument("--start-line", type=bounded_int(1, 2**31 - 1))
    reader.add_argument("--lines", type=bounded_int(1, 200), default=80)
    reader.add_argument("--no-redirect", action="store_true")
    exporter = commands.add_parser("export", help="write one page's complete original wikitext to a new file")
    exporter.add_argument("title")
    exporter.add_argument("output", type=Path)
    commands.add_parser("info", help="show dump provenance and namespace counts")
    args = parser.parse_args(argv)
    try:
        if args.command == "import":
            import_dump(args.source, args.db)
        else:
            db = open_database(args.db)
            try:
                {"search": search, "read": read_page, "export": export_page,
                 "info": lambda db, args: info(db)}[args.command](db, args)
            finally:
                db.close()
    except (OSError, ValueError, sqlite3.Error, ET.ParseError) as exc:
        print(f"uesp: {exc}", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print("uesp: interrupted", file=sys.stderr)
        return 130
    return 0


if __name__ == "__main__":
    sys.exit(main())
