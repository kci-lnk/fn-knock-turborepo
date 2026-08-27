#!/usr/bin/env node

import { readFileSync, writeFileSync } from "node:fs";

const [baselinePath, currentPath, filteredBaselinePath, filteredCurrentPath] =
  process.argv.slice(2);

if (
  !baselinePath ||
  !currentPath ||
  !filteredBaselinePath ||
  !filteredCurrentPath
) {
  throw new Error(
    "usage: filter-intentional-terminal-openapi-breaks.mjs <baseline> <current> <filtered-baseline> <filtered-current>",
  );
}

const TERMINAL_CONFIG_PATH = "/api/admin/config/terminal_feature";
const TERMINAL_PATH_PREFIX = "/api/admin/terminal";

const removeRetiredTerminalProperties = (value) => {
  if (!value || typeof value !== "object") return;
  if (Array.isArray(value)) {
    for (const item of value) removeRetiredTerminalProperties(item);
    return;
  }

  const properties = value.properties;
  if (properties && typeof properties === "object") {
    delete properties.terminal_feature;
    delete properties.terminalFeature;
    delete properties.terminal_available;
    delete properties.terminalAvailable;
  }
  if (Array.isArray(value.required)) {
    value.required = value.required.filter(
      (field) =>
        ![
          "terminal_feature",
          "terminalFeature",
          "terminal_available",
          "terminalAvailable",
        ].includes(field),
    );
  }
  for (const child of Object.values(value)) {
    removeRetiredTerminalProperties(child);
  }
};

const isLegacyTerminalContract = (document) =>
  Boolean(
    document.paths?.[TERMINAL_CONFIG_PATH] ||
      document.paths?.[`${TERMINAL_PATH_PREFIX}/status`] ||
      document.paths?.[`${TERMINAL_PATH_PREFIX}/tmux/install`] ||
      document.paths?.[`${TERMINAL_PATH_PREFIX}/sessions`]?.post,
  );

const isSshTerminalV1Contract = (document) =>
  Boolean(
    document.paths?.[`${TERMINAL_PATH_PREFIX}/targets/probe-host-key`]?.post &&
      document.paths?.[`${TERMINAL_PATH_PREFIX}/targets/test-connection`]
        ?.post &&
      document.paths?.[
        `${TERMINAL_PATH_PREFIX}/targets/{id}/sessions`
      ]?.post &&
      document.paths?.[
        `${TERMINAL_PATH_PREFIX}/attachments/{id}/events`
      ]?.get &&
      !isLegacyTerminalContract(document),
  );

const filteredContract = (document) => {
  for (const route of Object.keys(document.paths ?? {})) {
    if (
      route === TERMINAL_CONFIG_PATH ||
      route === TERMINAL_PATH_PREFIX ||
      route.startsWith(`${TERMINAL_PATH_PREFIX}/`)
    ) {
      delete document.paths[route];
    }
  }

  const schemas = document.components?.schemas ?? {};
  for (const name of Object.keys(schemas)) {
    if (name.startsWith("Terminal")) delete schemas[name];
  }
  removeRetiredTerminalProperties(document.components);
  return document;
};

const baseline = JSON.parse(readFileSync(baselinePath, "utf8"));
const current = JSON.parse(readFileSync(currentPath, "utf8"));
const isRecordedSshMigration =
  isLegacyTerminalContract(baseline) && isSshTerminalV1Contract(current);

const filteredBaseline = isRecordedSshMigration
  ? filteredContract(baseline)
  : baseline;
const filteredCurrent = isRecordedSshMigration
  ? filteredContract(current)
  : current;

writeFileSync(
  filteredBaselinePath,
  `${JSON.stringify(filteredBaseline, null, 2)}\n`,
);
writeFileSync(
  filteredCurrentPath,
  `${JSON.stringify(filteredCurrent, null, 2)}\n`,
);

if (isRecordedSshMigration) {
  console.log(
    "[openapi] filtered the recorded legacy-to-SSH terminal migration; all other operations remain covered by oasdiff",
  );
} else {
  console.log(
    "[openapi] no legacy-to-SSH migration detected; the full contract remains covered by oasdiff",
  );
}
