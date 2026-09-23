# Audit6 completed-scope closeout evidence

The user requested prompt wrap-up. At 2026-09-23 15:27:24 UTC the owned frozen runner received SIGTERM and cleaned its children. The original batch outcome remains false / exit 143 / soak-ttl1. The 30-minute soak did not complete and recovery was not started. This is NOT a passing 45-trial batch.

Completed scope: smoke18 plus TTL0/TTL1 six pairs each = 42 valid trials. Their frozen contracts and unchanged nonregression gates passed. All 42 raw trial metrics, aggregate metrics, source identities, gate/report/outcome files and interruption evidence are included. Original remote files remain untouched. Logs, databases, runtime configurations, credentials and binaries are excluded. Source SHA and sanitized collected SHA are tracked separately in collection.json; excluded run-log SHA are retained as declarations, not independently replayed content.

Offline replay requires Python3 and Node.js, no service or SSH:

```sh
python3 replay/replay-closeout.py --evidence . --plan frozen-plan --tools frozen-tools --out /tmp/NEW-audit6-replay
```

The output must say completed_scope_replay_passed=true AND full_batch_completed=false. It reruns all three frozen contracts/gates, maps report-only raw pointers to the collected files and compares regenerated Markdown byte-for-byte, and checks 42 trial directories / 84 product process identities. Original results and frozen tools are unchanged. The historical collector selftest is tied to the old261 spec and is intentionally not used; generic verify uses this explicit42 spec.

PAYLOAD-SHA256.json inventories every payload except itself. REPLAY-CHECK.json records the actual local replay. PRODUCTION-CHECK.json records unchanged production PIDs/start times and no remaining owned experiment processes. This archive is independent from the 273 earlier v5 trials; old v5 soaks do not establish a completed soak for Go669.
