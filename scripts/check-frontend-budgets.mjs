#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { gzipSync } from "node:zlib";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const budgets = [
  {
    name: "admin",
    directory: "apps/server-admin-view/dist",
    limit: 255 * 1024,
  },
  { name: "auth", directory: "apps/server-auth-view/dist", limit: 157 * 1024 },
];

function fail(message) {
  throw new Error(`[frontend-budget] ${message}`);
}

function referencedInitialFiles(html) {
  const files = new Set(["index.html"]);
  const expression =
    /<(?:script|link)\b[^>]*(?:src|href)=["']([^"']+)["'][^>]*>/gi;
  for (const match of html.matchAll(expression)) {
    const reference = match[1];
    if (/^(?:https?:|data:|\/\/)/i.test(reference)) continue;
    const clean = reference
      .split(/[?#]/, 1)[0]
      .replace(/^\.\//, "")
      .replace(/^\//, "");
    if (clean) files.add(clean);
  }
  return files;
}

let failed = false;
for (const budget of budgets) {
  const directory = path.join(root, budget.directory);
  const htmlPath = path.join(directory, "index.html");
  let html;
  try {
    html = readFileSync(htmlPath, "utf8");
  } catch (error) {
    fail(
      `missing ${path.relative(root, htmlPath)}; build the frontend first (${error.code})`,
    );
  }

  let compressedBytes = 0;
  const files = referencedInitialFiles(html);
  for (const relativePath of files) {
    const absolutePath = path.join(directory, relativePath);
    try {
      compressedBytes += gzipSync(readFileSync(absolutePath), {
        level: 9,
      }).byteLength;
    } catch (error) {
      fail(
        `${budget.name} initial asset is missing: ${relativePath} (${error.code})`,
      );
    }
  }

  const kib = (compressedBytes / 1024).toFixed(1);
  const limitKib = (budget.limit / 1024).toFixed(0);
  console.log(
    `[frontend-budget] ${budget.name}: ${kib} KiB gzip / ${limitKib} KiB (${files.size} files)`,
  );
  if (compressedBytes > budget.limit) failed = true;
}

if (failed) fail("one or more initial-page gzip budgets were exceeded");
