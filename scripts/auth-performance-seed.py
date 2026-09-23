#!/usr/bin/env python3
"""Seed ONLY a stopped, harness-owned synthetic database; never a load client."""
import argparse
import datetime
import hashlib
import json
import pathlib
import sqlite3
import time


def seed(directory, scenario, sessions, renewals, cache_ttl=1):
    directory = pathlib.Path(directory).resolve()
    marker = directory / ".auth-performance-owned"
    if marker.read_text().strip() != "synthetic-auth-performance-v1":
        raise ValueError("not an auth-performance synthetic directory")
    db_path = directory / "state.sqlite3"
    if db_path.is_symlink() or not db_path.is_file():
        raise ValueError("expected initialized local database")
    now = int(time.time())
    expiry = (now + 86400) * 1000
    iso = lambda seconds: datetime.datetime.fromtimestamp(seconds, datetime.timezone.utc).isoformat().replace("+00:00", "Z")
    encode = lambda value: json.dumps(value, separators=(",", ":"))
    digest = lambda value: hashlib.sha256(value.encode()).hexdigest()
    db = sqlite3.connect(db_path)
    db.execute("PRAGMA foreign_keys=ON")

    def key(name, kind, expires=None):
        db.execute("INSERT OR REPLACE INTO kv_keys VALUES (?,?,?)", (name, kind, expires))

    def string(name, value, expires=None):
        key(name, "string", expires)
        db.execute("INSERT OR REPLACE INTO kv_strings VALUES (?,?)", (name, encode(value)))

    def zset(name, member, score):
        db.execute("INSERT OR IGNORE INTO kv_keys VALUES (?, 'zset', NULL)", (name,))
        db.execute("INSERT OR REPLACE INTO kv_zset VALUES (?,?,?)", (name, member, score))

    config = json.loads(db.execute("SELECT document_json FROM config_documents WHERE singleton=1").fetchone()[0])
    policy = {"enabled": True, "policy_version": "authperf-policy-v1", "idle_ttl_seconds": 86400,
              "max_lifetime_seconds": 172800, "groups": [{"id": "authperf-group", "conditions": [
                  {"id": "issue", "target": "request_header", "operator": "equals", "name": "X-Authperf-Issue", "values": ["allow"]}]}]}
    config.update({"run_type": 3, "auto_manage_firewall": False,
                   "subdomain_mode": {**config.get("subdomain_mode", {}), "auth_cache_ttl_seconds": cache_ttl, "auth_cache_unauthorized_ttl_seconds": cache_ttl},
                   "reverse_proxy_throttle": {"enabled": False, "requests_per_second": 500, "burst": 1000, "block_seconds": 30},
                   "auth_credential_settings": {**config.get("auth_credential_settings", {}), "post_login_ip_grant_mode": "disabled", "session_ip_mobility_enabled": False},
                   "host_mappings": [{"host": "protected.authperf.test", "target": "http://127.0.0.1:28081", "target_type": "proxy", "use_auth": True, "suppress_toolbar": True},
                                     {"host": "grant.authperf.test", "target": "http://127.0.0.1:28081", "target_type": "proxy", "use_auth": True, "suppress_toolbar": True, "advanced_auth": policy}]})
    string("fn_knock:config", config)
    db.execute("UPDATE config_documents SET document_json=?,revision=revision+1,updated_at_ms=? WHERE singleton=1", (encode(config), now * 1000))
    totp = {"id": "authperf-totp", "secret": "JBSWY3DPEHPK3PXP", "comment": "Synthetic benchmark", "createdAt": iso(now), "access_scopes": [], "subdomain_access": {"mode": "all", "hosts": []}}
    string("fn_knock:totps", [totp])
    for index in range(sessions):
        sid = f"authperf-session-{index}"
        ip = "198.18.0.1" if index == 0 and scenario != "auto_ip_miss" else "198.18.0.2"
        value = {"totpId": totp["id"], "method": "TOTP", "credentialId": totp["id"], "credentialName": "Synthetic benchmark", "grantType": "browser_session", "ip": ip,
                 "userAgent": "auth-performance", "loginTime": iso(now), "expiresAt": iso(now + 86400), "subdomainAccess": {"mode": "all", "hosts": []}}
        if scenario.startswith("auto_ip"):
            value.update({"grantType": "login_ip_grant", "postLoginIpGrantMode": "follow_session"})
        string("fn_knock:session:" + sid, value, expiry)
        aggregate = {"session_id": sid, "session": {"value": value, "expires_at_ms": expiry}, "binding_index": [], "bindings": [], "active_ips": [], "pending_whitelist": [], "whitelist_owners": []}
        db.execute("INSERT OR REPLACE INTO mobility_session_aggregates VALUES (?,?,?,?)", (sid, encode(aggregate), expiry, now * 1000))
    host = "grant.authperf.test"
    tokens = ["authperf-grant-hit", "authperf-grant-preflight"]
    if scenario == "grant_renewal":
        tokens += [f"authperf-grant-load-{i}" for i in range(renewals)]
        tokens += [f"authperf-grant-warm-{i}" for i in range(renewals)]
    for token in tokens:
        last_access = now - 120 if scenario == "grant_renewal" else now
        grant = {"host": host, "policy_version": policy["policy_version"], "group_id": "authperf-group", "issued_at": now - 180, "last_access_at": last_access, "hard_expires_at": now + 86400}
        hashed = digest(token)
        grant_key = "fn_knock:auth:subdomain_rule_grant:" + hashed
        string(grant_key, grant, expiry)
        zset("fn_knock:auth:subdomain_rule_grant_active:" + digest(host), grant_key, now + 86400)
        db.execute("INSERT OR REPLACE INTO subdomain_rule_grants VALUES (?,?,?,?,?,?,?,?,?)", (hashed, host, policy["policy_version"], "authperf-group", grant["issued_at"], last_access, now + 86400, expiry, now * 1000))
        db.execute("INSERT OR REPLACE INTO subdomain_rule_grant_active_entries VALUES (?,?,?,?)", (digest(host), hashed, now + 86400, now * 1000))
    if scenario.startswith("auto_ip"):
        record = {"id": "authperf-auto", "ip": "198.18.0.1", "targetType": "ip", "expireAt": now + 86400, "source": "auto", "createdAt": now, "status": "active"}
        key("fn_knock:whitelist:records", "hash")
        db.execute("INSERT INTO kv_hash VALUES (?,?,?)", ("fn_knock:whitelist:records", record["id"], encode(record)))
        for name, member in [("fn_knock:whitelist:ips", record["ip"]), ("fn_knock:whitelist:ip_records:" + record["ip"], record["id"])]:
            key(name, "set")
            db.execute("INSERT INTO kv_set VALUES (?,?)", (name, member))
        zset("fn_knock:whitelist:record_order", record["id"], now)
        zset("fn_knock:whitelist:expiry", record["id"], now + 86400)
        db.execute("INSERT OR REPLACE INTO whitelist_documents VALUES ('record',?,?,?,?,?,?)", (record["id"], encode(record), now, now + 86400, "active", now * 1000))
    db.commit()
    db.close()
    return {"scenario": scenario, "sessions": sessions, "renewal_tokens_per_phase": renewals, "created_at": iso(now), "expires_at": iso(now + 86400)}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory")
    parser.add_argument("scenario")
    parser.add_argument("--sessions", type=int, default=64)
    parser.add_argument("--renewals", type=int, default=4096)
    parser.add_argument("--cache-ttl", type=int, choices=[0, 1], default=1)
    args = parser.parse_args()
    if not 1 <= args.sessions <= 10000 or not 1 <= args.renewals <= 100000:
        parser.error("sessions must be 1..10000 and renewals 1..100000")
    print(json.dumps(seed(args.directory, args.scenario, args.sessions, args.renewals, args.cache_ttl)))
