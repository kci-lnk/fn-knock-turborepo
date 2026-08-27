#!/usr/bin/env node

import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const { version: appVersion } = JSON.parse(
  readFileSync(path.join(root, "version.json"), "utf8"),
);
const compressibleExtensions = new Set([
  ".css",
  ".html",
  ".js",
  ".json",
  ".svg",
  ".txt",
  ".wasm",
]);
const MAX_INITIAL_SCRIPT_BROTLI = 100 * 1024;
const MAX_ADMIN_ENTRY_IMPORTS = 16;
const MAX_ADMIN_HTML_MODULE_PRELOADS = 8;

const apps = [
  {
    name: "admin",
    directory: "apps/server-admin-view/dist",
    scenarios: [
      {
        name: "Dashboard+zh-CN",
        limit: 260 * 1024,
        fileLimit: 32,
        sources: [
          "/src/views/Dashboard.vue",
          "/messages/scopes/admin/zh-CN.ts",
        ],
      },
    ],
  },
  {
    name: "auth",
    directory: "apps/server-auth-view/dist",
    scenarios: [
      {
        name: "Home+zh-CN",
        limit: 125 * 1024,
        sources: ["/src/views/Home.vue", "/messages/scopes/auth/zh-CN.ts"],
      },
      {
        name: "LoginBase+zh-CN",
        limit: 155 * 1024,
        sources: ["/src/views/Login.vue", "/messages/scopes/auth/zh-CN.ts"],
      },
      {
        name: "Login+ALTCHA+zh-CN",
        limit: 180 * 1024,
        sources: [
          "/src/views/Login.vue",
          "/messages/scopes/auth/zh-CN.ts",
          "/node_modules/altcha/",
        ],
      },
      {
        name: "Login+PoW+zh-CN",
        limit: 175 * 1024,
        sources: ["/src/views/Login.vue", "/messages/scopes/auth/zh-CN.ts"],
        files: [/pow\.worker[^/]*\.js$/u],
      },
    ],
  },
];

const fail = (message) => {
  throw new Error(`[frontend-budget] ${message}`);
};

const walkFiles = (directory, relative = "") =>
  readdirSync(path.join(directory, relative), { withFileTypes: true }).flatMap(
    (entry) => {
      const next = path.join(relative, entry.name);
      return entry.isDirectory() ? walkFiles(directory, next) : [next];
    },
  );

const normalizedSource = (record) =>
  `/${String(record.src ?? "")
    .replaceAll("\\", "/")
    .replace(/^\/+/, "")}`;

let failed = false;
for (const app of apps) {
  const directory = path.join(root, app.directory);
  const manifestPath = path.join(directory, ".vite/manifest.json");
  if (!existsSync(manifestPath)) {
    fail(
      `missing ${path.relative(root, manifestPath)}; enable Vite manifest output`,
    );
  }
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  const entries = Object.entries(manifest);
  const entryKey = entries.find(([, record]) => record.isEntry)?.[0];
  if (!entryKey) fail(`${app.name} manifest has no entry chunk`);
  if (app.name === "admin") {
    const entryFile = String(manifest[entryKey].file ?? "");
    if (!entryFile.startsWith(`assets/v${appVersion}/`)) {
      console.error(
        `[frontend-budget] admin entry is outside the versioned asset namespace: ${entryFile}`,
      );
      failed = true;
    }
    const entryImports = manifest[entryKey].imports?.length ?? 0;
    if (entryImports > MAX_ADMIN_ENTRY_IMPORTS) {
      console.error(
        `[frontend-budget] admin entry has ${entryImports} direct imports (limit ${MAX_ADMIN_ENTRY_IMPORTS})`,
      );
      failed = true;
    }

    const html = readFileSync(path.join(directory, "index.html"), "utf8");
    const modulePreloads = html.match(/rel=["']modulepreload["']/gu)?.length ?? 0;
    if (modulePreloads > MAX_ADMIN_HTML_MODULE_PRELOADS) {
      console.error(
        `[frontend-budget] admin HTML has ${modulePreloads} module preloads (limit ${MAX_ADMIN_HTML_MODULE_PRELOADS})`,
      );
      failed = true;
    }
  }
  const allFiles = walkFiles(directory).map((value) =>
    value.replaceAll("\\", "/"),
  );

  const addRecord = (key, files, visited) => {
    if (visited.has(key)) return;
    const record = manifest[key];
    if (!record) fail(`${app.name} manifest import is missing: ${key}`);
    visited.add(key);
    if (record.file) files.add(record.file);
    for (const css of record.css ?? []) files.add(css);
    for (const asset of record.assets ?? []) files.add(asset);
    for (const imported of record.imports ?? [])
      addRecord(imported, files, visited);
  };

  for (const relativePath of allFiles) {
    if (relativePath.endsWith(".br") || relativePath.endsWith(".gz")) continue;
    if (!compressibleExtensions.has(path.extname(relativePath))) continue;
    if (
      !existsSync(path.join(directory, `${relativePath}.br`)) ||
      !existsSync(path.join(directory, `${relativePath}.gz`))
    ) {
      console.error(
        `[frontend-budget] ${app.name} build lacks gzip/brotli sidecars: ${relativePath}`,
      );
      failed = true;
    }
  }

  for (const scenario of app.scenarios) {
    const files = new Set(["index.html"]);
    const visited = new Set();
    addRecord(entryKey, files, visited);
    for (const source of scenario.sources) {
      const matches = entries.filter(([, record]) =>
        normalizedSource(record).includes(source),
      );
      if (matches.length === 0) {
        fail(
          `${app.name}/${scenario.name} source not found in manifest: ${source}`,
        );
      }
      for (const [key] of matches) addRecord(key, files, visited);
    }
    for (const expression of scenario.files ?? []) {
      const matches = allFiles.filter((file) => expression.test(file));
      if (matches.length === 0) {
        fail(`${app.name}/${scenario.name} asset not found: ${expression}`);
      }
      for (const file of matches) files.add(file);
    }

    let rawBytes = 0;
    let gzipBytes = 0;
    let brotliBytes = 0;
    for (const relativePath of files) {
      const absolutePath = path.join(directory, relativePath);
      if (!existsSync(absolutePath)) {
        fail(`${app.name}/${scenario.name} asset is missing: ${relativePath}`);
      }
      const rawSize = statSync(absolutePath).size;
      rawBytes += rawSize;
      const requiresSidecars = compressibleExtensions.has(
        path.extname(relativePath),
      );
      const gzipPath = `${absolutePath}.gz`;
      const brotliPath = `${absolutePath}.br`;
      if (
        requiresSidecars &&
        (!existsSync(gzipPath) || !existsSync(brotliPath))
      ) {
        fail(
          `${app.name}/${scenario.name} lacks gzip/brotli sidecars: ${relativePath}`,
        );
      }
      const gzipSize = existsSync(gzipPath) ? statSync(gzipPath).size : rawSize;
      const brotliSize = existsSync(brotliPath)
        ? statSync(brotliPath).size
        : rawSize;
      gzipBytes += gzipSize;
      brotliBytes += brotliSize;
      if (
        relativePath.endsWith(".js") &&
        brotliSize > MAX_INITIAL_SCRIPT_BROTLI
      ) {
        console.error(
          `[frontend-budget] ${app.name}/${scenario.name}: ${relativePath} is ${(brotliSize / 1024).toFixed(1)} KiB Brotli (limit 100 KiB)`,
        );
        failed = true;
      }
    }
    console.log(
      `[frontend-budget] ${app.name}/${scenario.name}: ${(rawBytes / 1024).toFixed(1)} KiB raw, ${(gzipBytes / 1024).toFixed(1)} KiB gzip, ${(brotliBytes / 1024).toFixed(1)} KiB br / ${(scenario.limit / 1024).toFixed(0)} KiB (${files.size} files)`,
    );
    if (brotliBytes > scenario.limit) failed = true;
    if (scenario.fileLimit && files.size > scenario.fileLimit) {
      console.error(
        `[frontend-budget] ${app.name}/${scenario.name}: ${files.size} files (limit ${scenario.fileLimit})`,
      );
      failed = true;
    }
  }
}

if (failed) fail("one or more route-level frontend budgets were exceeded");
