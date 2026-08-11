#!/usr/bin/env node

import { brotliCompressSync, constants, gzipSync } from "node:zlib";
import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";

const root = path.resolve(process.argv[2] ?? "dist");
const compressibleExtensions = new Set([
  ".css",
  ".html",
  ".js",
  ".json",
  ".svg",
  ".txt",
  ".wasm",
]);

function filesIn(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const target = path.join(directory, entry.name);
    return entry.isDirectory() ? filesIn(target) : [target];
  });
}

let generated = 0;
for (const file of filesIn(root)) {
  if (
    !compressibleExtensions.has(path.extname(file)) ||
    statSync(file).size < 1024
  ) {
    continue;
  }
  const source = readFileSync(file);
  const gzip = gzipSync(source, { level: 9 });
  const brotli = brotliCompressSync(source, {
    params: {
      [constants.BROTLI_PARAM_QUALITY]: 11,
      [constants.BROTLI_PARAM_MODE]: constants.BROTLI_MODE_TEXT,
    },
  });
  if (gzip.byteLength < source.byteLength) {
    writeFileSync(`${file}.gz`, gzip);
    generated += 1;
  }
  if (brotli.byteLength < source.byteLength) {
    writeFileSync(`${file}.br`, brotli);
    generated += 1;
  }
}

console.log(
  `[precompress] generated ${generated} gzip/brotli assets in ${root}`,
);
