#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const budgets = [
  {
    path: "apps/server-admin-rs/src/tunnels/cloudflared/optimization.rs",
    maxLines: 4_350,
  },
  {
    path: "apps/server-admin-rs/src/tunnels/cloudflared/optimization/api.rs",
    maxLines: 575,
  },
  {
    path: "apps/server-admin-rs/src/tunnels/cloudflared/optimization/scheduler.rs",
    maxLines: 500,
  },
  {
    path: "apps/server-admin-view/src/lib/api/config.ts",
    maxLines: 1_450,
  },
  {
    path: "apps/server-admin-view/src/lib/api/system.ts",
    maxLines: 300,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainMappingsCard.vue",
    maxLines: 1_000,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainMappingNotices.vue",
    maxLines: 65,
  },
  {
    path: "apps/server-admin-view/src/views/subdomain-proxy/SubdomainAdvancedAuth.vue",
    maxLines: 950,
  },
  {
    path: "apps/server-admin-view/src/views/WOLManagement.vue",
    maxLines: 1_000,
  },
  {
    path: "apps/server-admin-view/src/views/wol-management/WOLPortalSettingsDialog.vue",
    maxLines: 75,
  },
  {
    path: "apps/server-admin-view/src/views/tunnel/cloudflare/CloudflareOptimizationCard.vue",
    maxLines: 1_000,
  },
];

const countLines = (content) => {
  if (!content) return 0;
  const newlines = content.match(/\n/g)?.length ?? 0;
  return content.endsWith("\n") ? newlines : newlines + 1;
};

const failures = [];
for (const budget of budgets) {
  const absolutePath = path.join(root, budget.path);
  let lines;
  try {
    lines = countLines(readFileSync(absolutePath, "utf8"));
  } catch (error) {
    failures.push(
      `${budget.path} cannot be read (${error.code ?? error.message})`,
    );
    continue;
  }
  console.log(
    `[source-hotspot] ${budget.path}: ${lines}/${budget.maxLines} lines`,
  );
  if (lines > budget.maxLines) {
    failures.push(
      `${budget.path} has ${lines} lines (limit ${budget.maxLines})`,
    );
  }
}

if (failures.length > 0) {
  throw new Error(`[source-hotspot] ${failures.join("; ")}`);
}
