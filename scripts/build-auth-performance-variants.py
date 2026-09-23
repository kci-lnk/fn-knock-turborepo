#!/usr/bin/env python3
"""Build serial Linux amd64 variants of one frozen source at opt-level z/s/2/3."""

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
import time


MINIMUM_FREE_BYTES = 5 * 1024**3
LEVELS = ("z", "s", "2", "3")
TOOLCHAIN_COMMANDS = {
    "cargo": ["cargo", "--version"],
    "rustc": ["rustc", "--version", "--verbose"],
    "zig": ["zig", "version"],
    "rustup_active": ["rustup", "show", "active-toolchain"],
    "cargo_zigbuild": ["cargo-zigbuild", "--version"],
}


def commit_sha(value):
    if not re.fullmatch(r"[0-9a-fA-F]{40}", value):
        raise argparse.ArgumentTypeError("must be a full 40-character commit SHA")
    return value.lower()


def parse_args():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True, help="clean frozen worktree")
    parser.add_argument(
        "--target", type=Path, required=True,
        help="exclusive Cargo cache; created if absent",
    )
    parser.add_argument(
        "--artifacts", type=Path, required=True,
        help="output directory; existing results are never replaced",
    )
    parser.add_argument("--expected-source-commit", type=commit_sha, required=True)
    parser.add_argument("--gateway-commit", type=commit_sha, required=True)
    parser.add_argument("--jobs", type=int, default=2)
    parser.add_argument("--levels", nargs="+", choices=LEVELS, default=list(LEVELS))
    args = parser.parse_args()
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    if len(set(args.levels)) != len(args.levels):
        parser.error("--levels must not contain duplicates")
    for name in ("source", "target", "artifacts"):
        setattr(args, name, getattr(args, name).resolve())
    return args


def capture(command, source, environment):
    return subprocess.check_output(
        command, cwd=source, env=environment, text=True, stderr=subprocess.STDOUT
    ).strip()


def file_sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def utc_now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def write_json(path, value):
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")
    temporary.replace(path)


def check_source(args, environment, lock_sha256=None):
    head = capture(["git", "rev-parse", "HEAD"], args.source, environment)
    dirty = capture(["git", "status", "--porcelain"], args.source, environment)
    if head != args.expected_source_commit or dirty:
        raise RuntimeError("frozen source HEAD changed or worktree is not clean")
    lockfile = args.source / "apps/server-admin-rs/Cargo.lock"
    if lock_sha256 is not None and file_sha256(lockfile) != lock_sha256:
        raise RuntimeError("frozen Cargo.lock changed")


def toolchain_versions(source, environment):
    return {
        name: capture(command, source, environment)
        for name, command in TOOLCHAIN_COMMANDS.items()
    }


def available_space(args):
    free = shutil.disk_usage(args.target).free
    artifact_free = shutil.disk_usage(args.artifacts).free
    if min(free, artifact_free) < MINIMUM_FREE_BYTES:
        raise RuntimeError(
            f"low disk: target={free} bytes, artifacts={artifact_free} bytes; "
            f"require at least {MINIMUM_FREE_BYTES} bytes on both filesystems"
        )
    return free, artifact_free


def build_variants(args):
    manifest_path = args.artifacts / "compiler-build-manifest.json"
    if manifest_path.exists():
        raise RuntimeError("manifest already exists; use a fresh artifact directory")
    for level in args.levels:
        if (args.artifacts / f"compiler-{level}").exists():
            raise RuntimeError(f"compiler-{level} already exists; existing results are preserved")

    # Capture the environment once so only opt-level changes between variants.
    inherited_environment = dict(os.environ)
    check_source(args, inherited_environment)
    lock_sha256 = file_sha256(args.source / "apps/server-admin-rs/Cargo.lock")
    versions = toolchain_versions(args.source, inherited_environment)
    args.target.mkdir(parents=True, exist_ok=True)
    args.artifacts.mkdir(parents=True, exist_ok=True)
    base_environment = {
        "CARGO_TARGET_DIR": str(args.target),
        "FN_KNOCK_GATEWAY_COMMIT": args.gateway_commit,
        "CARGO_BUILD_JOBS": str(args.jobs),
        "CARGO_PROFILE_RELEASE_LTO": "fat",
        "CARGO_PROFILE_RELEASE_CODEGEN_UNITS": "1",
    }
    command = [
        "cargo", "zigbuild", "--locked", "--release",
        "--manifest-path", str(args.source / "apps/server-admin-rs/Cargo.toml"),
        "--bin", "server-admin-rs", "--target", "x86_64-unknown-linux-gnu",
    ]
    manifest = {
        "started_utc": utc_now(),
        "status": "running",
        "source_dir": str(args.source),
        "source_commit": args.expected_source_commit,
        "gateway_commit": args.gateway_commit,
        "target_dir": str(args.target),
        "levels": args.levels,
        "toolchain": versions,
        "cargo_lock_sha256": lock_sha256,
        "base_environment": base_environment,
        "command_argv": command,
        "minimum_free_bytes": MINIMUM_FREE_BYTES,
        "variants": [],
    }
    write_json(manifest_path, manifest)
    entry = None
    try:
        for level in args.levels:
            if (args.artifacts / "STOP_AFTER_CURRENT").exists():
                manifest.update(
                    status="paused", paused_before_variant=level, paused_utc=utc_now(),
                )
                write_json(manifest_path, manifest)
                print("PAUSED", manifest_path, flush=True)
                return 0
            entry = None
            free, artifact_free = available_space(args)
            check_source(args, inherited_environment, lock_sha256)
            if toolchain_versions(args.source, inherited_environment) != versions:
                raise RuntimeError("toolchain changed")

            directory = args.artifacts / f"compiler-{level}"
            directory.mkdir(exist_ok=False)
            environment = {**base_environment, "CARGO_PROFILE_RELEASE_OPT_LEVEL": level}
            command_text = "env " + shlex.join(
                [f"{key}={value}" for key, value in environment.items()] + command
            )
            (directory / "command.txt").write_text(
                "cd " + shlex.quote(str(args.source)) + "\n" + command_text + "\n",
                encoding="utf-8",
            )
            entry = {
                "opt_level": level,
                "status": "running",
                "started_utc": utc_now(),
                "free_bytes_before": free,
                "artifact_free_bytes_before": artifact_free,
                "environment": environment,
                "command": command_text,
                "log": str(directory / "build.log"),
            }
            manifest["variants"].append(entry)
            write_json(manifest_path, manifest)
            print("START", level, "free_gib", round(free / 1024**3, 2), flush=True)
            start = time.monotonic()
            with (directory / "build.log").open("w", encoding="utf-8") as log:
                result = subprocess.run(
                    command, cwd=args.source,
                    env={**inherited_environment, **environment},
                    stdout=log, stderr=subprocess.STDOUT,
                )
            entry.update(
                elapsed_seconds=time.monotonic() - start,
                exit_code=result.returncode,
                finished_utc=utc_now(),
            )
            if result.returncode:
                raise RuntimeError(
                    f"opt-level {level} build failed with exit code {result.returncode}"
                )

            # Copy before the next level can reuse Cargo's output path.
            artifact = directory / "server-admin-rs"
            shutil.copy2(
                args.target / "x86_64-unknown-linux-gnu/release/server-admin-rs", artifact,
            )
            entry.update(
                artifact=str(artifact), sha256=file_sha256(artifact),
                size_bytes=artifact.stat().st_size,
                free_bytes_after=shutil.disk_usage(args.target).free,
            )
            check_source(args, inherited_environment, lock_sha256)
            if toolchain_versions(args.source, inherited_environment) != versions:
                raise RuntimeError("toolchain changed during build")
            entry["status"] = "complete"
            write_json(directory / "result.json", entry)
            write_json(manifest_path, manifest)
            print(
                "DONE", level, "seconds", round(entry["elapsed_seconds"], 2),
                "bytes", entry["size_bytes"], "sha256", entry["sha256"], flush=True,
            )
    except (Exception, KeyboardInterrupt) as error:
        manifest.update(
            status="interrupted" if isinstance(error, KeyboardInterrupt) else "failed",
            error=str(error),
            finished_utc=utc_now(),
        )
        if entry is not None and entry["status"] == "running":
            entry.update(status=manifest["status"], error=str(error))
        write_json(manifest_path, manifest)
        raise

    manifest.update(status="complete", finished_utc=utc_now())
    write_json(manifest_path, manifest)
    print("COMPLETE", manifest_path, flush=True)
    return 0


def main():
    args = parse_args()
    try:
        return build_variants(args)
    except KeyboardInterrupt:
        print("Interrupted; existing logs and artifacts are preserved.", file=sys.stderr)
        return 130
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"Build preparation failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
