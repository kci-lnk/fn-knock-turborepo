#!/usr/bin/env python3
"""Prepare/run isolated SQLite statement-count test overlays on two pinned sources.

Preparation never compiles. Execution is sequential and uses one exact test per
copied test binary. This is an operation diagnostic, not a throughput benchmark.
"""

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import time


BASELINE = "d4f8805f39d9f4480bf83f4382ba59e5f49e7dc4"
TEST_NAME = "auth::routes::sql_statement_diagnostic::operation_statement_counts"
MIN_FREE = 5 * 1024**3
OVERLAYS = Path(__file__).resolve().with_suffix("")


def commit_sha(value):
    if not re.fullmatch(r"[0-9a-f]{40}", value):
        raise argparse.ArgumentTypeError("expected a lowercase full 40-character commit SHA")
    return value


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def tree_sha256(root):
    digest = hashlib.sha256()
    for path in sorted(root.rglob("*")):
        if path.is_file():
            digest.update(str(path.relative_to(root)).encode() + b"\0")
            digest.update(bytes.fromhex(sha256(path)))
    return digest.hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def check_space(path):
    free = shutil.disk_usage(path).free
    if free < MIN_FREE:
        raise RuntimeError(f"less than 5 GiB free at {path}: {free} bytes")
    return free


def replace_once(path, old, new):
    text = path.read_text()
    if text.count(old) != 1:
        raise RuntimeError(f"overlay anchor must occur exactly once: {path}: {old!r}")
    path.write_text(text.replace(old, new))


def extract_archive(archive, destination):
    # These archives come from pinned, local commits. Reject links/special files
    # and traversal explicitly, including on Python versions without data_filter.
    with tarfile.open(archive) as tar:
        for member in tar.getmembers():
            target = (destination / member.name).resolve()
            if not target.is_relative_to(destination) or not (member.isfile() or member.isdir()):
                raise RuntimeError(f"unsupported archive entry: {member.name}")
        tar.extractall(destination)


def apply_overlay(source):
    src = source / "apps/server-admin-rs/src"
    module = '\n#[cfg(test)]\npub(crate) mod sql_statement_diagnostic;\n'
    compat = src / "storage/redis_compat.rs"
    compat.write_text(compat.read_text() + module)
    connection = src / "storage/redis_compat/connection.rs"
    anchor = "        manager.initialize_readers().await?;"
    replace_once(connection, anchor, anchor + "\n        #[cfg(test)]\n        manager.sql_diagnostic_attach().await?;")
    routes = src / "auth/routes.rs"
    routes.write_text(routes.read_text() + '\n#[cfg(test)]\nmod sql_statement_diagnostic;\n')
    mobility = src / "auth/mobility.rs"
    mobility.write_text(mobility.read_text() + '\n#[cfg(test)]\npub(crate) use restore::list_active_sessions_by_ip as sql_diagnostic_list_active_sessions_by_ip;\n')
    trace = src / "storage/redis_compat/sql_statement_diagnostic.rs"
    shutil.copyfile(OVERLAYS / "trace_overlay.rs", trace)
    operations = src / "auth/routes/sql_statement_diagnostic.rs"
    shutil.copyfile(OVERLAYS / "operations_overlay.rs", operations)
    has_context = (src / "auth/request_context.rs").exists()
    if has_context:
        replace_once(operations, "    let _ = state;\n    future.await", "    crate::auth::request_context::scope(state, future).await")
    modified = [compat, connection, routes, mobility, trace, operations]
    return {
        "request_context_scope": has_context,
        "modified_files": {str(path.relative_to(source)): sha256(path) for path in modified},
    }


def prepare(args):
    args.output.mkdir(parents=True, exist_ok=False)
    check_space(args.output)
    manifest = {
        "schema_version": 1,
        "prepared_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "status": "preparing",
        "driver_sha256": sha256(Path(__file__).resolve()),
        "overlay_sha256": {p.name: sha256(p) for p in sorted(OVERLAYS.glob("*.rs"))},
        "scope": "method-level SQL statement executions after warmup; not HTTP/RPC SQL per request",
        "auth_readers": 1,
        "test": TEST_NAME,
        "variants": [],
    }
    manifest_path = args.output / "manifest.json"
    write_json(manifest_path, manifest)
    for label, commit in [("baseline", args.baseline_commit), ("candidate", args.candidate_commit)]:
        actual = subprocess.check_output(["git", "rev-parse", commit], cwd=args.repo, text=True).strip()
        if actual != commit:
            raise RuntimeError("source commit mismatch")
        source = args.output / "sources" / label
        source.mkdir(parents=True)
        archive = args.output / f"{label}-source.tar"
        with archive.open("wb") as stream:
            subprocess.run(["git", "archive", "--format=tar", commit], cwd=args.repo, stdout=stream, check=True)
        extract_archive(archive, source)
        variant = {"label": label, "source_commit": commit, "source": str(source),
                   "archive_sha256": sha256(archive), "archive_bytes": archive.stat().st_size}
        variant.update(apply_overlay(source))
        variant["overlaid_tree_sha256"] = tree_sha256(source)
        variant["cargo_lock_sha256"] = sha256(source / "apps/server-admin-rs/Cargo.lock")
        manifest["variants"].append(variant)
        write_json(manifest_path, manifest)
    manifest["status"] = "prepared"
    write_json(manifest_path, manifest)
    print(manifest_path)


def run(args):
    manifest_path = args.output / "manifest.json"
    manifest = json.loads(manifest_path.read_text())
    if manifest["status"] != "prepared":
        raise RuntimeError("run requires a freshly prepared directory; use a new output after failure")
    if manifest["driver_sha256"] != sha256(Path(__file__).resolve()):
        raise RuntimeError("driver changed after preparation")
    overrides = [key for key in os.environ if key.startswith("CARGO_PROFILE_") or key in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET")]
    if overrides:
        raise RuntimeError(f"remove build-profile overrides first: {overrides}")
    args.target.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    environment.update(CARGO_TARGET_DIR=str(args.target), CARGO_BUILD_JOBS=str(args.jobs), FN_KNOCK_SQLITE_AUTH_READERS="1")
    manifest["toolchain"] = {tool: subprocess.check_output(command, text=True).strip()
        for tool, command in {"cargo": ["cargo", "--version"], "rustc": ["rustc", "--version", "--verbose"]}.items()}
    manifest["target"] = str(args.target)
    manifest["jobs"] = args.jobs
    manifest["status"] = "running"
    write_json(manifest_path, manifest)
    try:
        for variant in manifest["variants"]:
            variant["free_bytes_before"] = {"target": check_space(args.target), "output": check_space(args.output)}
            source = Path(variant["source"])
            if tree_sha256(source) != variant["overlaid_tree_sha256"]:
                raise RuntimeError("archived source tree changed after preparation")
            for relative, expected in variant["modified_files"].items():
                if sha256(source / relative) != expected:
                    raise RuntimeError(f"modified overlay source: {relative}")
            if sha256(source / "apps/server-admin-rs/Cargo.lock") != variant["cargo_lock_sha256"]:
                raise RuntimeError("Cargo.lock changed")
            directory = args.output / variant["label"]
            directory.mkdir()
            command = ["cargo", "test", "--locked", "--manifest-path", str(source / "apps/server-admin-rs/Cargo.toml"), "--lib", "--no-run", "--message-format=json"]
            variant["build_command"] = command
            variant["build_environment"] = {key: environment[key] for key in ("CARGO_TARGET_DIR", "CARGO_BUILD_JOBS", "FN_KNOCK_SQLITE_AUTH_READERS")}
            started = time.monotonic()
            with (directory / "build.jsonl").open("w") as stdout, (directory / "build.log").open("w") as stderr:
                subprocess.run(command, cwd=source, env=environment, stdout=stdout, stderr=stderr, check=True)
            variant["build_seconds"] = time.monotonic() - started
            if tree_sha256(source) != variant["overlaid_tree_sha256"]:
                raise RuntimeError("archived source tree changed during compilation")
            executables = []
            for line in (directory / "build.jsonl").read_text().splitlines():
                message = json.loads(line)
                if message.get("reason") == "compiler-artifact" and message.get("executable") and message.get("profile", {}).get("test") and message.get("target", {}).get("name") == "server_admin_rs":
                    executables.append(Path(message["executable"]))
            if len(executables) != 1:
                raise RuntimeError(f"expected one library test binary, got {executables}")
            check_space(args.output)
            binary = directory / "server-admin-rs-sql-diagnostic.test"
            shutil.copy2(executables[0], binary)
            variant["binary_sha256"] = sha256(binary)
            variant["binary_bytes"] = binary.stat().st_size
            output = directory / "statements.json"
            test_environment = environment.copy()
            test_environment.update(FN_KNOCK_SQL_DIAGNOSTIC_OUT=str(output), FN_KNOCK_SQL_DIAGNOSTIC_SOURCE=variant["source_commit"])
            test_command = [str(binary), TEST_NAME, "--exact", "--nocapture", "--test-threads=1"]
            variant["test_command"] = test_command
            variant["test_environment"] = {key: test_environment[key] for key in ("FN_KNOCK_SQLITE_AUTH_READERS", "FN_KNOCK_SQL_DIAGNOSTIC_OUT", "FN_KNOCK_SQL_DIAGNOSTIC_SOURCE")}
            write_json(manifest_path, manifest)
            with (directory / "test.log").open("w") as stream:
                subprocess.run(test_command, cwd=source, env=test_environment, stdout=stream, stderr=subprocess.STDOUT, check=True)
            report = json.loads(output.read_text())
            if report["source_commit"] != variant["source_commit"] or len(report["measurements"]) != 9:
                raise RuntimeError("diagnostic identity/row count mismatch")
            variant["statement_report_sha256"] = sha256(output)
            variant["status"] = "complete"
            write_json(manifest_path, manifest)
        manifest["status"] = "complete"
    except Exception as error:
        manifest["status"] = "failed"
        manifest["error"] = str(error)
        raise
    finally:
        write_json(manifest_path, manifest)
    print(manifest_path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    preparation = commands.add_parser("prepare", help="archive pinned sources and apply test-only overlays; no build")
    preparation.add_argument("--repo", required=True, type=Path)
    preparation.add_argument("--output", required=True, type=Path, help="new output directory")
    preparation.add_argument("--baseline-commit", type=commit_sha, default=BASELINE)
    preparation.add_argument("--candidate-commit", type=commit_sha, required=True)
    execution = commands.add_parser("run", help="sequential native compilation and isolated diagnostic tests")
    execution.add_argument("--output", required=True, type=Path)
    execution.add_argument("--target", required=True, type=Path, help="exclusive native Cargo target directory")
    execution.add_argument("--jobs", type=int, default=2)
    args = parser.parse_args()
    args.output = args.output.resolve()
    if args.command == "prepare":
        args.repo = args.repo.resolve()
        prepare(args)
    else:
        if args.jobs < 1:
            parser.error("--jobs must be positive")
        args.target = args.target.resolve()
        run(args)


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
