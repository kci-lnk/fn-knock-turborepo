#!/usr/bin/env python3
"""Read-only fnOS index/journal comparison; never prints certificate/key material.

Run on the affected NAS with Python 3. With no arguments, inspect pending journals
in the standard fn-knock data directory. This does not recover or modify anything.
Only the network_cert_all.conf entry is decoded, never other journal file backups.
"""

import argparse
import base64
from collections import Counter
from datetime import datetime, timezone
import json
from pathlib import Path
import sys


INDEX = "/usr/trim/etc/network_cert_all.conf"
DATA_DIR = "/usr/local/apps/@appdata/fn-knock"
KNOWN_FIELDS = {
    "certificate", "privateKey", "fullchain", "domain", "san",
    "validFrom", "validTo", "sum", "used", "appFlag",
}
MAX_BYTES = 64 * 1024 * 1024


def read_bytes(path):
    if not path.is_file():
        raise ValueError("Expected regular file")
    with path.open("rb") as stream:
        value = stream.read(MAX_BYTES + 1)
    if len(value) > MAX_BYTES:
        raise ValueError("Input exceeds size limit")
    return value


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False)


def parse_index(raw):
    value = json.loads(raw)
    if not isinstance(value, list) or not all(isinstance(e, dict) for e in value):
        raise ValueError("Expected index object array")
    return value


def keyed(entries):
    result = {}
    for entry in entries:
        key = entry.get("certificate")
        if not isinstance(key, str) or not key or key in result:
            return None
        result[key] = entry
    return result


def compare(current_bytes, expected_encoded):
    if expected_encoded is None:
        return {"classification": "expected_index_absent", "matches_current_guard": False}
    expected_bytes = base64.b64decode(expected_encoded, validate=True)
    current = parse_index(current_bytes)
    expected = parse_index(expected_bytes)
    if current_bytes == expected_bytes:
        kind = "identical_bytes"
    elif canonical(current) == canonical(expected):
        kind = "formatting_only"
    elif Counter(map(canonical, current)) == Counter(map(canonical, expected)):
        kind = "entry_order_only"
    else:
        kind = "content_changed"
    report = {
        "classification": kind,
        "matches_current_guard": kind in ("identical_bytes", "formatting_only"),
        "expected_entries": len(expected),
        "current_entries": len(current),
    }
    old, live = keyed(expected), keyed(current)
    if old is None or live is None:
        report["entry_identity_ambiguous"] = True
        return report
    report["added_entries"] = len(live.keys() - old.keys())
    report["removed_entries"] = len(old.keys() - live.keys())
    changed = Counter()
    for key in old.keys() & live.keys():
        for field in old[key].keys() | live[key].keys():
            if (field not in old[key] or field not in live[key]
                    or canonical(old[key][field]) != canonical(live[key][field])):
                # Do not echo arbitrary field names supplied by an input file.
                changed[field if field in KNOWN_FIELDS else "<other>"] += 1
    report["changed_field_counts"] = dict(sorted(changed.items()))
    return report


def inspect_journal(path, journal, current_bytes):
    changes = [entry for entry in journal["files"] if entry.get("path") == INDEX]
    if len(changes) != 1:
        raise ValueError("Expected exactly one certificate index recovery entry")
    change = changes[0]
    report = {
        "completed": journal["completed"],
        "has_gateway_snapshot": journal.get("gateway_before") is not None,
        "before": compare(current_bytes, change["before"]),
        "after": compare(current_bytes, change["after"]),
    }
    prefix = path.name.split("-", 1)[0]
    if prefix.isdigit() and len(prefix) == 13:
        report["filename_timestamp_utc"] = datetime.fromtimestamp(
            int(prefix) / 1000, timezone.utc
        ).isoformat()
    report["index_content_guard_would_pass"] = any(
        report[state]["matches_current_guard"] for state in ("before", "after")
    )
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--data-dir", type=Path, default=Path(DATA_DIR))
    parser.add_argument("--journal", type=Path, help="Inspect one journal, including completed ones")
    parser.add_argument("--index", type=Path, default=Path(INDEX))
    args = parser.parse_args()
    try:
        current_bytes = read_bytes(args.index)
        parse_index(current_bytes)
        root = args.data_dir / "fnos-certificate-sync"
        paths = [args.journal] if args.journal else sorted((root / "transactions").glob("*.json"))
        reports = []
        for path in paths:
            journal = json.loads(read_bytes(path))
            if args.journal or journal["completed"] is False:
                reports.append(inspect_journal(path, journal, current_bytes))
        print(json.dumps({
            "read_only": True,
            "scope": "Index content only; does not validate database, permissions, TLS or recovery safety",
            "automatic_pause_marker_exists": (root / "automatic-pause.json").exists(),
            "transactions_directory_exists": (root / "transactions").is_dir(),
            "journals": reports,
        }, indent=2))
        return 0
    except (OSError, ValueError, KeyError, TypeError, AttributeError, OverflowError) as error:
        # Raw exception messages can include input fragments; output only their type.
        print(json.dumps({"error_type": type(error).__name__,
                          "message": "Cannot inspect inputs; check paths, permissions and JSON/base64 structure"}),
              file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
