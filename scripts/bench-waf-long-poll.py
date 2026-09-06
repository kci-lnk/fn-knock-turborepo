#!/usr/bin/env python3
"""Run isolated Rust workers against the real Go WAF queue over loopback gRPC.

Build the Go internal/wafwaitfixture binary and Rust runtime-test test binaries
first. No production process or configuration is touched. Parallel mode runs
both arms concurrently; use sequential mode for less host-load interference.
"""
import argparse
import csv
import json
import os
from pathlib import Path
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rust-test-bin", required=True)
    parser.add_argument("--go-fixture", required=True)
    parser.add_argument("--output-dir", required=True)
    parser.add_argument("--idle-seconds", type=int, default=600)
    parser.add_argument("--parallel", action="store_true")
    args = parser.parse_args()
    output = Path(args.output_dir).resolve()
    output.mkdir(parents=True, exist_ok=True)
    (output / "run.json").write_text(json.dumps({
        "args": vars(args), "system": list(os.uname()),
        "sampling": "ps every 5 seconds; RSS in KiB; CPU time is cumulative and includes startup",
        "scope": "isolated real Rust scheduler + SQLite and Go EventStore; synthetic events; no deployed gateway",
    }, indent=2))
    processes = []
    files = []
    arm_started = {}
    started = time.monotonic()
    try:
        with (output / "process-samples.csv").open("w", newline="") as stream, (output / "phase-resources.jsonl").open("w") as phase_stream:
            samples = csv.writer(stream)
            samples.writerow(["elapsed_seconds", "mode", "component", "pid", "rss_kib", "cpu_percent", "cpu_time"])

            def start(mode):
                go_log = (output / f"{mode}-go.stderr").open("w")
                rust_log = (output / f"{mode}-rust.log").open("w")
                files.extend([go_log, rust_log])
                go = subprocess.Popen([args.go_fixture], stdout=subprocess.PIPE, stderr=go_log, text=True)
                processes.append(go)
                endpoints = json.loads(go.stdout.readline())
                env = os.environ.copy()
                env.update(FN_KNOCK_WAF_FIXTURE_RPC=endpoints["rpc"],
                           FN_KNOCK_WAF_FIXTURE_CONTROL=endpoints["control"],
                           FN_KNOCK_WAF_AB_MODE=mode,
                           FN_KNOCK_WAF_AB_IDLE_SECONDS=str(args.idle_seconds))
                rust = subprocess.Popen([args.rust_test_bin, "--ignored", "--exact",
                    "waf::routes::tests::worker::waf_long_polling_ab", "--nocapture"],
                    env=env, stdout=rust_log, stderr=subprocess.STDOUT)
                processes.append(rust)
                arm_started[mode] = time.monotonic()
                print(json.dumps({"mode": mode, "rust_pid": rust.pid, "go_pid": go.pid}), flush=True)
                return mode, go, rust

            def monitor(arms):
                seen = {mode: 0 for mode, _, _ in arms}
                next_sample = 0.0
                while any(rust.poll() is None for _, _, rust in arms):
                    now = time.monotonic()
                    for mode, go, rust in arms:
                        if now >= next_sample:
                            for component, proc in [("go", go), ("rust", rust)]:
                                if proc.poll() is not None:
                                    continue
                                result = subprocess.run(["ps", "-p", str(proc.pid), "-o", "pid=,rss=,pcpu=,time="], capture_output=True, text=True)
                                fields = result.stdout.split()
                                if len(fields) == 4:
                                    samples.writerow([round(time.monotonic()-started, 3), mode, component, *fields])
                        lines = (output / f"{mode}-rust.log").read_text().splitlines(keepends=True)
                        for line in lines[seen[mode]:]:
                            if not line.endswith("\n"):
                                break
                            seen[mode] += 1
                            if "WAF_AB " not in line:
                                continue
                            record = json.loads(line.split("WAF_AB ", 1)[1])
                            if record["phase"] not in ("idle_done", "events", "stopped"):
                                continue
                            resources = {}
                            for component, proc in [("go", go), ("rust", rust)]:
                                result = subprocess.run(["ps", "-p", str(proc.pid), "-o", "rss=,time="], capture_output=True, text=True)
                                fields = result.stdout.split()
                                if len(fields) == 2:
                                    resources[component] = {"rss_kib": int(fields[0]), "cpu_time": fields[1]}
                            phase_stream.write(json.dumps({"mode": mode, "record": record, "resources": resources}) + "\n")
                            phase_stream.flush()
                    if now >= next_sample:
                        stream.flush()
                        next_sample = now + 5
                    if now - started > args.idle_seconds * 2 + 180:
                        raise TimeoutError("WAF A/B exceeded its deadline")
                    # Closely observe workload boundaries after the idle phase;
                    # periodic ps sampling remains at five-second intervals.
                    near_events = any(now - arm_started[mode] >= args.idle_seconds - 5 for mode, _, rust in arms if rust.poll() is None)
                    time.sleep(.05 if near_events else 5)
                for mode, go, rust in arms:
                    if rust.returncode != 0:
                        raise RuntimeError(f"{mode} arm failed; see {mode}-rust.log")
                    go.terminate()
                    go.wait(timeout=10)

            if args.parallel:
                monitor([start("old"), start("new")])
            else:
                for mode in ["old", "new"]:
                    monitor([start(mode)])
        print(f"A/B completed: {output}", flush=True)
    finally:
        for proc in processes:
            if proc.poll() is None:
                proc.terminate()
                try:
                    proc.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    proc.wait()
        for file in files:
            file.close()


if __name__ == "__main__":
    main()
