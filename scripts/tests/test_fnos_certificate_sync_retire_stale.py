import base64
import copy
import fcntl
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "fnos-certificate-sync-retire-stale.py"


class RetirementTests(unittest.TestCase):
    def setUp(self):
        spec = importlib.util.spec_from_file_location("retirement", SCRIPT)
        self.module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.module)
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.module.ROOT = self.root
        self.module.INDEX = self.root / "index.json"
        self.module.GATEWAY = self.root / "gateway.json"
        (self.root / "transactions").mkdir()
        self.target = self.root / "transactions" / self.module.NAME
        for name in ("automatic-pause.json", "execution.lock", "registry.json"):
            (self.root / name).write_text("{}")
        self.live = [{"certificate": "/retained-1"}, {"certificate": "/retained-2"}]
        before = self.live + [{"certificate": "/old"}]
        after = before + [{"certificate": "/new-" + str(i)} for i in range(3)]
        encode = lambda value: base64.b64encode(json.dumps(value).encode()).decode()
        self.journal = {
            "completed": False,
            "files": [{"path": str(self.module.INDEX), "before": encode(before), "after": encode(after)}],
            "rows": [{"table": "cert", "id": 9, "before": {"certificate": "/old"}, "after": {"certificate": "/old"}}],
        }
        for i in range(3):
            self.journal["files"].append({"path": "/new-" + str(i)})
            self.journal["rows"].append({"table": "cert", "id": 10 + i, "before": None,
                                         "after": {"certificate": "/new-" + str(i)}})
            (self.root / ("orphan-" + str(i))).write_bytes(b"private key backup placeholder")
        self.target.write_text(json.dumps(self.journal))
        self.module.INDEX.write_text(json.dumps(self.live))
        self.module.GATEWAY.write_text("[]")
        self.db = {"cert": [dict(e, id=i + 1) for i, e in enumerate(self.live)], "used": [], "renew": []}
        self.module.database = lambda: copy.deepcopy(self.db)

    def snapshot(self):
        return {p.relative_to(self.root): p.read_bytes() for p in self.root.rglob("*") if p.is_file()}

    def assert_stopped_without_writes(self):
        before = self.snapshot()
        archives = list(self.root.glob("retired-transaction-*"))
        with self.assertRaises(self.module.Stop):
            self.module.retire()
        self.assertEqual(self.snapshot(), before)
        self.assertEqual(list(self.root.glob("retired-transaction-*")), archives)

    def test_archives_exact_journal_and_retains_pause_registry_and_files(self):
        before = self.snapshot()
        archive = self.module.retire()
        self.assertFalse(self.target.exists())
        self.assertEqual((archive / self.module.NAME).read_bytes(), before[self.target.relative_to(self.root)])
        self.assertEqual(archive.stat().st_mode & 0o777, 0o700)
        for path, data in before.items():
            if path != self.target.relative_to(self.root):
                self.assertEqual((self.root / path).read_bytes(), data)
        self.assert_stopped_without_writes()

    def test_pending_transaction_mismatch_stops(self):
        (self.root / "transactions" / "other.json").write_text(json.dumps(self.journal))
        self.assert_stopped_without_writes()

    def test_no_pause_marker_stops(self):
        (self.root / "automatic-pause.json").unlink()
        self.assert_stopped_without_writes()

    def test_existing_certificate_id_stops(self):
        self.db["cert"][0]["id"] = 9
        self.assert_stopped_without_writes()

    def test_service_binding_stops(self):
        self.db["used"] = [9]
        self.assert_stopped_without_writes()

    def test_renewal_stops(self):
        self.db["renew"] = [{"id": 100, "cert_id": 10}]
        self.assert_stopped_without_writes()

    def test_shared_key_stops(self):
        self.db["cert"][0]["private_key"] = "/new-1"
        self.assert_stopped_without_writes()

    def test_gateway_reference_stops(self):
        self.module.GATEWAY.write_text(json.dumps([{"cert": "/new-1", "key": "/key"}]))
        self.assert_stopped_without_writes()

    def test_index_change_stops(self):
        self.live[0]["used"] = True
        self.module.INDEX.write_text(json.dumps(self.live))
        self.assert_stopped_without_writes()

    def test_database_changes_between_reads_stop(self):
        values = iter([copy.deepcopy(self.db), dict(self.db, used=[9])])
        self.module.database = lambda: next(values)
        self.assert_stopped_without_writes()

    def test_existing_lock_prevents_archival(self):
        before = self.snapshot()
        with (self.root / "execution.lock").open("r+b") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with self.assertRaises(BlockingIOError):
                self.module.retire()
        self.assertEqual(self.snapshot(), before)


if __name__ == "__main__":
    unittest.main()
