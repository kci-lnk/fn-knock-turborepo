#!/usr/bin/env python3
"""Prepare the 2026-09-23 post-main bundle. Never builds, uploads, or runs load."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys

RUST = "5fddf896eaf9b1cf3eb300c08315320498c943b8"
GO = "4d15fa32764e26df58b16930d0e2503a90880002"
BASE_RUST = "d4f8805f39d9f4480bf83f4382ba59e5f49e7dc4"
BASE_GO = "92d4c0cb5495d57801d52893a8f0e8496a1c9182"
ROUTES = "bootstrap,session_hit,grant_hit,auto_ip_hit"
TOOLS = [
    "run-auth-performance-isolated.sh", "auth-performance.mjs",
    "auth-performance-lib.mjs", "auth-performance-worker.mjs",
    "auth-performance-seed.py", "auth-performance-profile.mjs",
    "auth-performance-recovery.mjs", "auth-performance-soak.mjs",
    "check-auth-performance.mjs", "check-auth-performance-plan.mjs",
    "report-auth-performance.mjs",
]


def load(path):
    return json.loads(Path(path).read_text())


def digest(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path("/Users/edgeware/Local/fn-knock-turborepo"))
    parser.add_argument("--base-config", required=True, type=Path)
    parser.add_argument("--candidate-name", required=True)
    parser.add_argument("--output", required=True, type=Path, help="new local bundle directory")
    parser.add_argument("--remote-bundle", required=True, type=Path, help="new target plan directory; never scripts4/scripts5")
    parser.add_argument("--tools-dir", required=True, type=Path, help="local copy of root's frozen scripts5 snapshot")
    parser.add_argument("--remote-tools", required=True, type=Path, help="existing frozen scripts5 directory")
    parser.add_argument("--remote-compiler-prefix", required=True, type=Path, help="e.g. REMOTE_ROOT/compiler5; appends -s/-2/-3/server-admin-rs")
    parser.add_argument("--main-results", required=True, type=Path)
    parser.add_argument("--candidate-go-manifest", required=True, type=Path)
    parser.add_argument("--candidate-go-sha256", required=True, help="root-confirmed matched-flags Go artifact SHA256")
    parser.add_argument("--baseline-rust-artifact", required=True, type=Path)
    parser.add_argument("--baseline-go-artifact", required=True, type=Path)
    parser.add_argument("--rust-build-root", type=Path, default=Path("/tmp/fn-knock-auth-artifacts4-20260923"))
    parser.add_argument("--params-build-root", type=Path, default=Path("/tmp/fn-knock-auth-artifacts4-params-20260923"))
    args = parser.parse_args()
    require(not args.output.exists(), "output exists; prepare a new immutable bundle")
    require(re.fullmatch(r"[A-Za-z0-9_-]+", args.candidate_name), "unsafe candidate name")
    for target in (args.remote_bundle, args.remote_tools, args.remote_compiler_prefix, args.main_results):
        require(target.is_absolute(), "all target paths must be absolute")
    require(not {"scripts4", "scripts5"}.intersection(args.remote_bundle.parts), "tool snapshots are immutable")
    require(args.remote_tools.name == "scripts5", "this post5 plan uses root's scripts5 snapshot")
    require(re.fullmatch(r"[0-9a-f]{64}", args.candidate_go_sha256), "invalid confirmed Go SHA256")
    config = load(args.base_config)
    require(len(config["candidates"]) == 1, "base config must select exactly one candidate")
    candidate = config["candidates"][0]
    require(candidate["name"] == args.candidate_name, "candidate name mismatch")
    for variant, rust, go in ((config["baseline"], BASE_RUST, BASE_GO), (candidate, RUST, GO)):
        require(variant["metadata"]["main_commit"] == rust, "Rust source mismatch")
        require(variant["metadata"]["go_commit"] == go, "Go source mismatch")
        require(variant["metadata"]["opt_level"] == "z", "primary comparison must use opt-level z")
        require(variant["metadata"]["lto"] == "fat" and variant["metadata"]["codegen_units"] == 1, "profile mismatch")
        require(set(variant.get("env", {})) <= {"FN_KNOCK_SQLITE_AUTH_READERS", "FN_KNOCK_TOKIO_WORKER_THREADS"}, "unexpected variant environment override")
    plan = load(args.repo / "docs/experiments/2026-09-23-auth-performance/EXPERIMENT_PLAN.json")
    require(plan["candidate"]["rust"] == RUST and plan["candidate"]["go"] == GO, "experiment plan source mismatch")
    require(plan["candidate"]["auth_readers"] == 1, "the accepted default reader remains one")

    artifacts = {}
    def artifact(remote, local, expected=None):
        actual = digest(local)
        require(expected is None or actual == expected, f"artifact hash mismatch: {local}")
        artifacts[str(remote)] = {"sha256": actual, "bytes": Path(local).stat().st_size, "local_source": str(local)}

    artifact(config["baseline"]["rust"], args.baseline_rust_artifact)
    artifact(config["baseline"]["go"], args.baseline_go_artifact)
    baseline_build = load(args.baseline_rust_artifact.parent.parent / "compiler-build-manifest.json")
    baseline_result = load(args.baseline_rust_artifact.parent / "result.json")
    require(baseline_build["source_commit"] == BASE_RUST and baseline_build["gateway_commit"] == BASE_GO, "baseline Rust build source mismatch")
    require(baseline_result["status"] == "complete" and baseline_result["exit_code"] == 0, "baseline Rust build is incomplete")
    require(baseline_result["sha256"] == artifacts[config["baseline"]["rust"]]["sha256"], "baseline Rust build hash mismatch")
    for key, value in (("OPT_LEVEL", "z"), ("LTO", "fat"), ("CODEGEN_UNITS", "1")):
        require(baseline_result["environment"]["CARGO_PROFILE_RELEASE_" + key] == value, "baseline Rust build profile mismatch")
    go_manifest = load(args.candidate_go_manifest)
    require(go_manifest["source_commit"] == GO, "candidate Go manifest source mismatch")
    require(go_manifest["sha256"] == args.candidate_go_sha256, "unconfirmed Go build manifest")
    # The final5 amendment explicitly requires the baseline's stripping/version flags.
    require(go_manifest.get("role") == "candidate", "use the final candidate/build.json, not the combined manifest")
    argv = go_manifest.get("command_argv", [])
    require(isinstance(argv, list) and argv.count("-ldflags") == 1, "missing Go link flags argv")
    flags = shlex.split(argv[argv.index("-ldflags") + 1])
    require(flags == ["-s", "-w", "-X", "go-reauth-proxy/pkg/version.Version=2.4.15", "-X", f"go-reauth-proxy/pkg/version.Commit={GO}"], "Go build does not record the matched baseline link flags")
    artifact(candidate["go"], Path(go_manifest["binary"]), args.candidate_go_sha256)
    builds = {"z": load(args.rust_build_root / "compiler-build-manifest.json")}
    builds["params"] = load(args.params_build_root / "compiler-build-manifest.json")
    build_results = {}
    for level in ("z", "s", "2", "3"):
        directory = args.rust_build_root if level == "z" else args.params_build_root
        build = builds["z" if level == "z" else "params"]
        require(build["source_commit"] == RUST and build["gateway_commit"] == GO, "Rust build source mismatch")
        result = load(directory / f"compiler-{level}" / "result.json")
        require(result["status"] == "complete" and result["exit_code"] == 0, f"compiler-{level} is unfinished")
        env = result["environment"]
        require(env["CARGO_PROFILE_RELEASE_OPT_LEVEL"] == level and env["CARGO_PROFILE_RELEASE_LTO"] == "fat" and env["CARGO_PROFILE_RELEASE_CODEGEN_UNITS"] == "1", "Rust build profile mismatch")
        remote = candidate["rust"] if level == "z" else f"{args.remote_compiler_prefix}-{level}/server-admin-rs"
        artifact(remote, directory / f"compiler-{level}" / "server-admin-rs", result["sha256"])
        build_results[level] = result
    require(builds["z"]["toolchain"] == builds["params"]["toolchain"], "Rust toolchain differs")
    require(builds["z"]["toolchain"] == baseline_build["toolchain"], "baseline Rust toolchain differs")
    require(builds["z"]["cargo_lock_sha256"] == builds["params"]["cargo_lock_sha256"], "Rust Cargo.lock differs")
    for role, variant in (("baseline", config["baseline"]), ("candidate", candidate)):
        for component in ("go", "rust"):
            pinned = plan[role].get("artifact_sha256", {}).get(component)
            require(pinned == artifacts[variant[component]]["sha256"], f"{role}/{component} differs from the final frozen plan artifact hash")
    tools_manifest = load(args.tools_dir / "tools-manifest.json")
    tool_hashes = {name: digest(args.tools_dir / name) for name in TOOLS}
    for name, value in tool_hashes.items():
        require(tools_manifest["files"].get(name) == value, f"scripts5 snapshot manifest mismatch: {name}")

    def normalize(variant, reader=None, capacity=None):
        value = copy.deepcopy(variant)
        value["env"] = {"FN_KNOCK_TOKIO_WORKER_THREADS": "2"}
        if reader is not None:
            value["env"]["FN_KNOCK_SQLITE_AUTH_READERS"] = str(reader)
        if capacity is not None:
            value["env"]["FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT"] = str(capacity)
            value["metadata"]["bridge_capacity"] = "explicit32 recovery fixture; excluded from performance claims"
        return value

    configs = {}
    primary = copy.deepcopy(config)
    primary["baseline"] = normalize(config["baseline"])
    primary["candidates"] = [normalize(candidate, 1)]
    configs["primary"] = primary
    anchor = normalize(candidate, 1)
    anchor["name"] = f"{args.candidate_name}-z-reader1"
    for kind, values in (("reader", ("2", "4")), ("compiler", ("s", "2", "3"))):
        for value in values:
            variant = normalize(candidate, int(value) if kind == "reader" else 1)
            level = "z" if kind == "reader" else value
            reader = value if kind == "reader" else "1"
            variant["name"] = f"{args.candidate_name}-{level}-reader{reader}"
            variant["metadata"]["opt_level"] = level
            variant["metadata"]["rust_opt_level"] = level
            if kind == "compiler":
                variant["rust"] = f"{args.remote_compiler_prefix}-{level}/server-admin-rs"
            configs[f"{kind}-{value}"] = {**copy.deepcopy(primary), "baseline": copy.deepcopy(anchor), "candidates": [variant]}
    recovery = copy.deepcopy(primary)
    recovery["baseline"] = normalize(config["baseline"], capacity=32)
    recovery["candidates"] = [normalize(candidate, 1, capacity=32)]
    configs["recovery"] = recovery

    jobs = []
    def job(name, group, config_key, routes, gate="smoke", **overrides):
        options = {"routes": routes, "pairs": 1, "warmup": 5, "seconds": 15,
                   "concurrency": 16, "clients": 2, "accounts": 1000, "sessions": 1000,
                   "grants": 100, "credential-kind": "totp", "cache-ttl": 0,
                   "renewals": 64, "profile": 0, "recovery-probe": 0,
                   "roles": "baseline,candidate"}
        options.update(overrides)
        jobs.append({"name": name, "group": group, "config": config_key,
                     "gate": gate, "options": options,
                     "performance_acceptance": gate in ("cache", "renewal")})

    job("cache", "cache", "primary", "session_hit", "cache", pairs=6, warmup=20, seconds=60, **{"cache-ttl": 1})
    job("renewal", "renewal", "primary", "grant_renewal", "renewal", pairs=6, warmup=20, seconds=60, grants=1)
    for size in (1000, 10000):
        job(f"grants-{size}", "grants", "primary", "grant_hit", concurrency=1, grants=size)
    for key in ("reader-2", "reader-4", "compiler-s", "compiler-2", "compiler-3"):
        job(f"explore-{key}", "params", key, ROUTES)
    job("profile-primary", "profile", "primary", "grant_hit,session_hit,auto_ip_hit", warmup=20, seconds=30, profile=1, **{"profile-idle": 5})
    for key in ("reader-2", "reader-4"):
        job(f"profile-{key}", "profile", key, "grant_hit,auto_ip_hit", warmup=20, seconds=30, profile=1, **{"profile-idle": 5})
    job("soak", "soak", "primary", "auto_ip_hit", "soak", warmup=20, seconds=1800, roles="candidate")
    job("recovery", "recovery", "recovery", "auto_ip_hit", warmup=1, seconds=3, **{"recovery-probe": 1})

    args.output.mkdir(parents=True)
    (args.output / "configs").mkdir()
    shutil.copy2(Path(__file__).with_name("run-post.sh"), args.output / "run-post.sh")
    shutil.copy2(__file__, args.output / "prepare-post.py")
    shutil.copy2(Path(__file__).with_name("README.md"), args.output / "README.md")
    write_json(args.output / "EXPERIMENT_PLAN.json", plan)
    for name, value in configs.items():
        write_json(args.output / "configs" / f"{name}.json", value)
    write_json(args.output / "jobs.json", jobs)
    # A generated shell contains only literal arguments; no runtime eval or shell interpolation.
    lines = ["# Generated immutable job list; sourced only after bundle hash verification."]
    for group in ("cache", "renewal", "grants", "params", "profile", "soak", "recovery"):
        lines.append(f"run_{group}() {{")
        for item in (item for item in jobs if item["group"] == group):
            argv = [item["name"], item["config"], item["gate"]]
            argv += [word for key, value in item["options"].items() for word in ("--" + key, str(value))]
            lines.append("  run_case " + shlex.join(argv))
        lines.append("}")
    (args.output / "jobs.sh").write_text("\n".join(lines) + "\n")
    files = {str(p.relative_to(args.output)): digest(p) for p in sorted(args.output.rglob("*")) if p.is_file()}
    write_json(args.output / "bundle-manifest.json", {
        "schema_version": 1, "status": "prepared_not_executed",
        "rust_source": RUST, "go_source": GO,
        "candidate_name": args.candidate_name,
        "remote_bundle": str(args.remote_bundle), "remote_tools": str(args.remote_tools),
        "remote_compiler_prefix": str(args.remote_compiler_prefix),
        "main_results": str(args.main_results), "input_config": str(args.base_config),
        "prepare_argv": sys.argv,
        "input_config_sha256": digest(args.base_config),
        "preparation_repo_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=args.repo, text=True).strip(),
        "tools_local_snapshot": str(args.tools_dir),
        "tools_snapshot_manifest": tools_manifest,
        "files": files, "tools": tool_hashes, "artifacts": artifacts,
        "rust_builds": builds, "rust_results": build_results, "go_build": go_manifest,
        "baseline_rust_build": baseline_build, "baseline_rust_result": baseline_result,
        "notes": ["No upload, compilation or experiment was executed by preparation.",
                  "This plan references root's frozen scripts5; scripts4 and all v4 results are untouched.",
                  "Parameter baseline is final z/reader1; only one parameter changes per comparison.",
                  "Grant scaling uses concurrency1, configured clients2 but one active worker."]})
    print(args.output / "bundle-manifest.json")


if __name__ == "__main__":
    main()
