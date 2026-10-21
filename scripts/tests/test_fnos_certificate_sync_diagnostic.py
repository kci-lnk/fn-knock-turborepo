import base64
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "fnos-certificate-sync-diagnostic.py"
SPEC = importlib.util.spec_from_file_location("diagnostic", SCRIPT)
DIAGNOSTIC = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DIAGNOSTIC)


def encoded(value):
    return base64.b64encode(json.dumps(value).encode()).decode()


class DiagnosticTests(unittest.TestCase):
    def setUp(self):
        self.entries = [
            {"certificate": "/secret-a", "used": False, "appFlag": 0},
            {"certificate": "/secret-b", "used": False, "appFlag": 0},
        ]

    def compare(self, current):
        return DIAGNOSTIC.compare(json.dumps(current).encode(), encoded(self.entries))

    def test_formatting_passes_but_reordering_does_not(self):
        formatted = json.dumps(self.entries, indent=2, sort_keys=True).encode()
        result = DIAGNOSTIC.compare(formatted, encoded(self.entries))
        self.assertEqual(result["classification"], "formatting_only")
        self.assertTrue(result["matches_current_guard"])
        reordered = self.compare(self.entries[::-1])
        self.assertEqual(reordered["classification"], "entry_order_only")
        self.assertFalse(reordered["matches_current_guard"])

    def test_binding_changes_and_unrelated_additions_are_reported_without_values(self):
        current = [dict(entry) for entry in self.entries]
        current[0]["used"] = True
        current[0]["private-unknown-field"] = "private-value"
        current.append({"certificate": "/secret-c"})
        report = self.compare(current)
        self.assertEqual(report["added_entries"], 1)
        self.assertEqual(report["changed_field_counts"], {"<other>": 1, "used": 1})
        self.assertNotIn("secret", json.dumps(report))
        self.assertNotIn("private", json.dumps(report))

    def test_duplicate_entries_are_not_silently_collapsed(self):
        report = self.compare([self.entries[0], self.entries[0]])
        self.assertEqual(report["classification"], "content_changed")
        self.assertTrue(report["entry_identity_ambiguous"])

    def test_cli_is_read_only_and_does_not_decode_private_key_backups(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            journal = root / "1789713601726-test.json"
            index = root / "index.json"
            journal.write_text(json.dumps({"completed": False, "files": [
                {"path": DIAGNOSTIC.INDEX, "before": encoded([]), "after": encoded(self.entries)},
                {"path": "/private.key", "before": "PRIVATE KEY NOT EVEN BASE64", "after": None},
            ]}))
            index.write_text(json.dumps(self.entries))
            before = {p: p.read_bytes() for p in root.iterdir()}
            result = subprocess.run([
                sys.executable, str(SCRIPT), "--data-dir", directory,
                "--journal", str(journal), "--index", str(index),
            ], capture_output=True, text=True, check=True)
            self.assertEqual(before, {p: p.read_bytes() for p in root.iterdir()})
            report = json.loads(result.stdout)["journals"][0]
            self.assertTrue(report["index_content_guard_would_pass"])
            self.assertEqual(report["filename_timestamp_utc"], "2026-09-18T06:40:01.726000+00:00")
            self.assertNotIn("PRIVATE KEY", result.stdout + result.stderr)
            self.assertNotIn("secret", result.stdout + result.stderr)

    def test_missing_and_malformed_index(self):
        self.assertFalse(DIAGNOSTIC.compare(b"[]", None)["matches_current_guard"])
        with self.assertRaises(ValueError):
            DIAGNOSTIC.compare(b"{}", encoded([]))


if __name__ == "__main__":
    unittest.main()
