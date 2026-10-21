#!/usr/bin/env python3
"""Archive only the diagnosed September 18 transaction; never restore certificates.

Turn off automatic synchronization in the UI first. Retains the pause marker,
certificate files, registry, and every byte of the original transaction journal.
All preconditions are specific to this incident, not a generic force-recovery tool.
"""

import base64
from collections import Counter
import fcntl
import json
import os
from pathlib import Path
import subprocess
import tempfile


ROOT = Path("/usr/local/apps/@appdata/fn-knock/fnos-certificate-sync")
INDEX = Path("/usr/trim/etc/network_cert_all.conf")
GATEWAY = Path("/usr/trim/etc/network_gateway_cert.conf")
NAME = "1789713601726-d9bcc601-75c8-49b1-ae61-cc0c441b77ae.json"


class Stop(Exception):
    pass


def require(condition, message):
    if not condition:
        raise Stop(message)


def read(path):
    require(path.is_file() and not path.is_symlink(), "Missing or unsafe file")
    with path.open("rb") as stream:
        data = stream.read(64 * 1024 * 1024 + 1)
    require(len(data) <= 64 * 1024 * 1024, "Input exceeds size limit")
    return data


def canonical(value):
    return json.dumps(value, sort_keys=True, allow_nan=False)


def database():
    sql = """
BEGIN READ ONLY;
SET LOCAL statement_timeout = '5s';
SELECT json_build_object(
  'cert', (
    SELECT coalesce(json_agg(t ORDER BY id), '[]'::json)
    FROM (
      SELECT id, certificate, private_key, issuer_certificate
      FROM public.cert
    ) t
  ),
  'used', (
    SELECT coalesce(json_agg(cert_id ORDER BY cert_id), '[]'::json)
    FROM public.cert_used_config
  ),
  'renew', (
    SELECT coalesce(json_agg(t ORDER BY id), '[]'::json)
    FROM (SELECT id, cert_id FROM public.cert_renew) t
  )
);
COMMIT;
"""
    result = subprocess.run(
        ["sudo", "-u", "postgres", "psql", "-d", "trim_connect",
         "-XqAt", "-v", "ON_ERROR_STOP=1"],
        input=sql, capture_output=True, text=True, timeout=20,
    )
    require(result.returncode == 0, "Read-only database check failed")
    return json.loads(result.stdout)


def retire():
    target = ROOT / "transactions" / NAME
    pause = ROOT / "automatic-pause.json"
    require(pause.is_file() and not pause.is_symlink(), "Pause marker missing or unsafe")
    lock_path = ROOT / "execution.lock"
    require(lock_path.is_file() and not lock_path.is_symlink(), "Unsafe sync lock")
    with lock_path.open("r+b") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        pending = []
        for path in sorted((ROOT / "transactions").glob("*.json")):
            value = json.loads(read(path))
            require(type(value["completed"]) is bool, "Invalid transaction state")
            if not value["completed"]:
                pending.append(path)
        require(pending == [target], "Pending transaction differs from diagnosed incident")
        journal_bytes = read(target)
        journal = json.loads(journal_bytes)
        changes = [f for f in journal["files"] if f.get("path") == str(INDEX)]
        require(len(changes) == 1, "Invalid index recovery record")
        before = json.loads(base64.b64decode(changes[0]["before"], validate=True))
        after = json.loads(base64.b64decode(changes[0]["after"], validate=True))
        rows = journal["rows"]
        require(all(r["table"] in ("cert", "cert_renew") for r in rows), "Unknown table")
        cert_rows = [r for r in rows if r["table"] == "cert"]
        require(len(cert_rows) == 4
                and sum(r["before"] is None for r in cert_rows) == 3
                and all(r["after"] is not None for r in cert_rows), "Unexpected operations")
        ids = {r["id"] for r in cert_rows}
        affected_paths = {
            r[side]["certificate"]
            for r in cert_rows for side in ("before", "after") if r.get(side) is not None
        }
        touched_files = {f["path"] for f in journal["files"] if f["path"] != str(INDEX)}
        expected = [e for e in before if e["certificate"] not in affected_paths]
        index_bytes, gateway_bytes = read(INDEX), read(GATEWAY)
        current, gateway = json.loads(index_bytes), json.loads(gateway_bytes)
        require(len(before) == 3 and len(after) == 6
                and len(expected) == 2 and len(current) == 2, "Unexpected index counts")
        require(Counter(map(canonical, current)) == Counter(map(canonical, expected)),
                "Current index differs from surviving original entries")
        db = database()
        require(len(db["cert"]) == 2
                and {r["certificate"] for r in db["cert"]} == {e["certificate"] for e in current},
                "Database and current index disagree")
        require(not any(r["id"] in ids for r in db["cert"]), "Old certificate ID still exists")
        require(not any(i in ids for i in db["used"]), "Old certificate has service bindings")
        renewal_ids = {r["id"] for r in rows if r["table"] == "cert_renew"}
        require(not any(r["cert_id"] in ids or r["id"] in renewal_ids for r in db["renew"]),
                "Old renewal record still exists")
        references = set()
        for collection, keys in (
            (db["cert"], ("certificate", "private_key", "issuer_certificate")),
            (current, ("certificate", "privateKey", "fullchain")),
            (gateway, ("cert", "key")),
        ):
            for entry in collection:
                references.update(entry[k] for k in keys
                                  if isinstance(entry.get(k), str) and entry[k])
        require(not references.intersection(touched_files | affected_paths),
                "Old certificate files are referenced by current configuration")
        require(read(target) == journal_bytes and read(INDEX) == index_bytes
                and read(GATEWAY) == gateway_bytes and database() == db and pause.is_file(),
                "State changed during inspection")
        archive = Path(tempfile.mkdtemp(prefix="retired-transaction-", dir=ROOT))
        for name, content in (
            ("current-network_cert_all.conf", index_bytes),
            ("current-network_gateway_cert.conf", gateway_bytes),
        ):
            with (archive / name).open("xb") as stream:
                stream.write(content)
                stream.flush()
                os.fsync(stream.fileno())
        os.rename(target, archive / NAME)
        for directory in (archive, ROOT / "transactions", ROOT):
            fd = os.open(directory, os.O_RDONLY | os.O_DIRECTORY)
            try:
                os.fsync(fd)
            finally:
                os.close(fd)
        return archive


if __name__ == "__main__":
    try:
        archived = retire()
        print("ARCHIVED:", archived)
        print("Database, certificates, index and services unchanged. Pause marker retained.")
        print("Keep auto-sync OFF. Refresh the UI, then click Retry Recovery.")
    except Stop as error:
        print("STOPPED:", error)
        raise SystemExit(1)
    except Exception as error:
        print("INCOMPLETE:", type(error).__name__)
        print("Keep any archive directory and report this output; do not retry blindly.")
        raise SystemExit(1)
