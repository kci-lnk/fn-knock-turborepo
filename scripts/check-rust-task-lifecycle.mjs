import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const sourceRoot = path.resolve("apps/server-admin-rs/src");
const maxDirectSpawnCallSites = 120;

// Direct spawns are limited to explicitly audited owners, request-scoped
// fan-out, subprocess pipe/wait tasks, platform entry points, and tests.
// Reducing a count is always allowed; adding a call site or a new file requires
// an intentional lifecycle review and a budget update here.
const auditedBudgets = new Map(
  Object.entries({
    "app.rs": 2,
    // Local HTTP fixtures; every server task is joined or awaited by its test.
    "auth/fnos_share_bypass.rs": 5,
    "auth/mobility.rs": 1,
    // Cancellation and concurrency probes; every handle is aborted or awaited.
    "auth/mobility/tests.rs": 5,
    // Cancellation and queue probes; every test-owned handle is aborted/awaited.
    "auth/password.rs": 3,
    // One returned bridge owner plus three test-only waiters; every waiter is awaited.
    "auth/routes/bridge.rs": 4,
    "certificates/acme/jobs.rs": 5,
    "certificates/auto_https.rs": 1,
    "config/runtime/tests.rs": 5,
    "ddns/routes/tests.rs": 2,
    "discovery/cidr.rs": 1,
    "discovery/scan_assets/tests.rs": 6,
    "gateway/proxy_config.rs": 1,
    "gateway/proxy_config/tests.rs": 8,
    "infra/background_tasks.rs": 1,
    // Test-only webhook HTTP fixtures; each task is awaited or explicitly aborted.
    "notifications/routes/tests.rs": 4,
    // Test-only local HTTP fixtures; every returned handle is awaited by its test.
    "panel_sync/tests.rs": 2,
    "runtime_health.rs": 1,
    // Serialization and concurrency probes; every handle is awaited.
    "security/whitelist/tests.rs": 2,
    "storage/redis_compat/tests/migrations.rs": 2,
    // Test-only concurrency and local fixture tasks; every handle is joined,
    // awaited, or explicitly aborted by the owning test.
    "storage/redis_store/tests/aggregates.rs": 2,
    "storage/redis_store/tests/analytics.rs": 1,
    "storage/redis_store/tests/core.rs": 2,
    "storage/redis_store/tests/events_notifications.rs": 4,
    "storage/redis_store/tests/identity.rs": 5,
    "storage/redis_store/tests/mobility.rs": 1,
    "storage/redis_store/tests/mobility_reconcile.rs": 2,
    "storage/redis_store/tests/notification_runtime.rs": 3,
    "storage/redis_store/tests/security.rs": 3,
    // Primary-executor blocker returned to each test and explicitly released/awaited.
    "storage/redis_store/tests/support.rs": 1,
    // Includes cancelled archive/HTTP callers, each aborted and awaited before releasing its worker.
    "system/maintenance/tests.rs": 5,
    // One session owner retained by TerminalRuntime and joined on terminate/shutdown,
    // one initialization progress bridge returned to and awaited by that owner, and
    // one test-only actor retained by the same runtime task registry, and one
    // abort/join cancellation probe owned by AbortOnDropHandle until cleanup.
    "system/terminal/runtime.rs": 4,
    // Test-only russh server fixture; every returned handle is explicitly aborted.
    "system/terminal/ssh.rs": 1,
    "system/update.rs": 1,
    "tunnels/cloudflared/cloudflare_api.rs": 7,
    "tunnels/cloudflared/managed.rs": 5,
    "tunnels/supervisor.rs": 6,
    "windows_service.rs": 1,
    // Test-only gRPC fixture and old/new collector owners; workers are joined,
    // and the returned fixture server is aborted by each owning test.
    "waf/routes/worker_tests.rs": 3,
    "wol/dispatch.rs": 2,
    "wol/integrations.rs": 1,
    // Test-only russh fixture; each returned handle is explicitly aborted by its test.
    "wol/ssh.rs": 1,
  }),
);

function rustFiles(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) return rustFiles(absolute);
    return entry.isFile() && entry.name.endsWith(".rs") ? [absolute] : [];
  });
}

const violations = [];
let total = 0;
for (const absolute of rustFiles(sourceRoot)) {
  const source = fs.readFileSync(absolute, "utf8");
  const count = source.match(/\btokio::spawn\s*\(/g)?.length ?? 0;
  if (count === 0) continue;
  const relative = path
    .relative(sourceRoot, absolute)
    .split(path.sep)
    .join("/");
  const budget = auditedBudgets.get(relative);
  total += count;
  if (budget === undefined) {
    violations.push(`${relative}: ${count} unaudited direct spawn call(s)`);
  } else if (count > budget) {
    violations.push(
      `${relative}: ${count} direct spawns exceed audited budget ${budget}`,
    );
  }
}

if (total > maxDirectSpawnCallSites) {
  violations.push(
    `repository total: ${total} direct spawns exceed fixed audited total ${maxDirectSpawnCallSites}`,
  );
}

if (violations.length > 0) {
  console.error("Rust task lifecycle audit failed:");
  for (const violation of violations) console.error(`- ${violation}`);
  process.exit(1);
}

console.log(
  `Rust task lifecycle audit passed (${total} direct spawn call sites; new sites require explicit review).`,
);
