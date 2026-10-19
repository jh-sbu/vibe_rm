"""Small end-to-end fixtures for the reference importer and query CLI."""

import bz2
from contextlib import redirect_stdout, redirect_stderr
import io
from pathlib import Path
import sqlite3
import tempfile
import unittest
import xml.etree.ElementTree as ET

import uesp


class UespTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.db = self.root / "wiki.sqlite3"
        self.dump = self.root / "wiki.xml"
        root = ET.Element("mediawiki", xmlns="http://www.mediawiki.org/xml/export-0.10/")
        site = ET.SubElement(root, "siteinfo")
        ET.SubElement(site, "base").text = "https://en.uesp.net/wiki/Main_Page"
        namespaces = ET.SubElement(site, "namespaces")
        for n, name in [(134, "Skyrim"), (142, "Skyrim Mod"), (146, "Dragonborn"),
                        (160, "Mod"), (144, "Online"), (10, "Template")]:
            ET.SubElement(namespaces, "namespace", key=str(n)).text = name
        pages = [
            ("Skyrim:Armor", 134, "Lead & {{Template}}\n== Armor Rating ==\narmor rating formula\n"
             "=== Cap ===\nmaximum protection\n== Notes ==\nother notes", None),
            ("Skyrim:Armour", 134, "#REDIRECT [[Skyrim:Armor#Armor Rating]]", "Skyrim:Armor"),
            ("Online:Armor", 144, "armor rating online only", None),
            ("Skyrim Mod:Flags", 142, "record flags 0x0001 NPC_", None),
            ("Template:Long", 10, ("x" * 3000 + "\n") * 80, None),
            ("Skyrim:LoopA", 134, "", "Skyrim:LoopB"),
            ("Skyrim:LoopB", 134, "", "Skyrim:LoopA"),
            ("Skyrim:Empty", 134, "", None),
            ("Skyrim:Armour Rating", 134, "#REDIRECT [[Skyrim:Armor#Armor Rating]]", "Skyrim:Armor"),
        ]
        for i, (title, ns, text, redirect) in enumerate(pages, 1):
            page = ET.SubElement(root, "page")
            for tag, value in [("title", title), ("ns", str(ns)), ("id", str(i))]:
                ET.SubElement(page, tag).text = value
            if redirect:
                ET.SubElement(page, "redirect", title=redirect)
            rev = ET.SubElement(page, "revision")
            for tag, value in [("id", str(100 + i)), ("timestamp", "2026-10-11T00:00:00Z"),
                               ("model", "wikitext"), ("text", text)]:
                ET.SubElement(rev, tag).text = value
        ET.ElementTree(root).write(self.dump, encoding="utf-8", xml_declaration=True)
        code, _, err = self.run_cli("import", str(self.dump))
        self.assertEqual(code, 0, err)

    def run_cli(self, *args):
        out, err = io.StringIO(), io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            code = uesp.main(["--db", str(self.db), *args])
        return code, out.getvalue(), err.getvalue()

    def test_import_preserves_content_and_provenance(self):
        with sqlite3.connect(self.db) as db:
            self.assertEqual(db.execute("SELECT count(*) FROM pages").fetchone()[0], 9)
            self.assertIn("Lead & {{Template}}", db.execute(
                "SELECT text FROM pages WHERE page_id=1").fetchone()[0])
        code, out, _ = self.run_cli("read", "skyrim:armor", "--lines", "1")
        self.assertEqual(code, 0)
        self.assertIn("oldid=101", out)
        self.assertIn("Dump: wiki.xml", out)
        self.assertIn("--start-line 2", out)
        self.assertNotIn("2: ==", out)

    def test_search_scope_and_query_modes(self):
        code, out, err = self.run_cli("search", "armor rating")
        self.assertEqual(code, 0, err)
        self.assertIn("Skyrim:Armor", out)
        self.assertNotIn("Online:Armor", out)
        self.assertIn("Online:Armor", self.run_cli("search", "armor", "--all")[1])
        out = self.run_cli("search", "armor", "--namespace", "144")[1]
        self.assertIn("Online:Armor", out)
        self.assertNotIn("Skyrim:Armor", out)
        self.assertIn("Skyrim Mod:Flags", self.run_cli("search", "NPC_")[1])
        self.assertIn("No matches", self.run_cli("search", "formula", "--title")[1])
        self.assertIn("Skyrim:Armor", self.run_cli("search", '"armor rating"', "--query-syntax")[1])
        self.assertEqual(self.run_cli("search", '"', "--query-syntax")[0], 1)
        self.assertEqual(self.run_cli("search", "armor", "--namespace", "typo")[0], 1)
        out = self.run_cli("search", "armour")[1]
        self.assertEqual(out.count("Redirect: Skyrim:Armor#Armor Rating"), 1)

    def test_redirect_section_and_line_numbers(self):
        code, out, err = self.run_cli("read", "Skyrim:Armour")
        self.assertEqual(code, 0, err)
        self.assertIn("Redirect: Skyrim:Armour -> Skyrim:Armor", out)
        self.assertIn("2: == Armor Rating ==", out)
        self.assertIn("5: maximum protection", out)
        self.assertNotIn("other notes", out)
        self.assertIn("6: == Notes ==", self.run_cli(
            "read", "Skyrim:Armour", "--section", "Notes")[1])
        self.assertIn("2:", self.run_cli("read", "Skyrim:Armor", "--sections")[1])
        self.assertIn("#REDIRECT", self.run_cli("read", "Skyrim:Armour", "--no-redirect")[1])
        self.assertEqual(self.run_cli("read", "Skyrim:LoopA")[0], 1)
        self.assertEqual(self.run_cli("read", "Skyrim:Missing")[0], 1)
        self.assertEqual(self.run_cli("read", "Skyrim:Armor", "--section", "Missing")[0], 1)
        self.assertIn("Lines 0-0 of 0", self.run_cli("read", "Skyrim:Empty")[1])

    def test_failed_import_keeps_existing_database(self):
        before = self.db.read_bytes()
        self.dump.write_text("<mediawiki><page>", encoding="utf-8")
        code, _, _ = self.run_cli("import", str(self.dump))
        self.assertEqual(code, 1)
        self.assertEqual(self.db.read_bytes(), before)
        self.assertEqual(list(self.root.glob("*.tmp")), [])

    def test_compressed_import_and_readonly_queries(self):
        compressed = self.root / "wiki.xml.bz2"
        compressed.write_bytes(bz2.compress(self.dump.read_bytes()))
        self.assertEqual(self.run_cli("import", str(compressed))[0], 0)
        db = uesp.open_database(self.db)
        try:
            with self.assertRaises(sqlite3.OperationalError):
                db.execute("DELETE FROM pages")
        finally:
            db.close()

    def test_long_lines_can_be_exported_losslessly(self):
        result = self.run_cli("read", "Template:Long")[1]
        self.assertIn("line truncated", result)
        self.assertIn("--start-line", result)
        self.assertLess(len(result), 25000)
        output = self.root / "long.wiki"
        self.assertEqual(self.run_cli("export", "Template:Long", str(output))[0], 0)
        self.assertEqual(output.read_text(), ("x" * 3000 + "\n") * 80)
        self.assertEqual(self.run_cli("export", "Template:Long", str(output))[0], 1)


if __name__ == "__main__":
    unittest.main()
