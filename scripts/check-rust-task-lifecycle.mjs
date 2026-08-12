import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const sourceRoot = path.resolve("apps/server-admin-rs/src");

// Direct spawns are limited to explicitly audited owners, request-scoped
// fan-out, subprocess pipe/wait tasks, platform entry points, and tests.
// Reducing a count is always allowed; adding a call site or a new file requires
// an intentional lifecycle review and a budget update here.
const auditedBudgets = new Map(
  Object.entries({
    "app.rs": 2,
    "auth/fnos_share_bypass.rs": 1,
    "auth/mobility.rs": 1,
    "auth/mobility/tests.rs": 3,
    "auth/routes/bridge.rs": 1,
    "certificates/acme/jobs.rs": 5,
    "certificates/auto_https.rs": 1,
    "config/runtime/tests.rs": 5,
    "ddns/routes/tests.rs": 2,
    "discovery/cidr.rs": 1,
    "discovery/scan_assets/tests.rs": 6,
    "gateway/proxy_config.rs": 1,
    "gateway/proxy_config/tests.rs": 8,
    "infra/background_tasks.rs": 1,
    "notifications/routes/tests.rs": 1,
    "runtime_health.rs": 1,
    "security/whitelist/tests.rs": 1,
    "storage/redis_store/tests.rs": 22,
    "system/maintenance/tests.rs": 3,
    "system/update.rs": 1,
    "tunnels/cloudflared/cloudflare_api.rs": 6,
    "tunnels/cloudflared/managed.rs": 5,
    "tunnels/supervisor.rs": 6,
    "windows_service.rs": 1,
    "wol/dispatch.rs": 2,
    "wol/integrations.rs": 1,
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
  const relative = path.relative(sourceRoot, absolute).split(path.sep).join("/");
  const budget = auditedBudgets.get(relative);
  total += count;
  if (budget === undefined) {
    violations.push(`${relative}: ${count} unaudited direct spawn call(s)`);
  } else if (count > budget) {
    violations.push(`${relative}: ${count} direct spawns exceed audited budget ${budget}`);
  }
}

if (violations.length > 0) {
  console.error("Rust task lifecycle audit failed:");
  for (const violation of violations) console.error(`- ${violation}`);
  process.exit(1);
}

console.log(
  `Rust task lifecycle audit passed (${total} direct spawn call sites; new sites require explicit review).`,
);
