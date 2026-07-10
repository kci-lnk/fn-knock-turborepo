import { readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { messages } from "../src/locales.ts";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(scriptDirectory, "../../..");
const outputPath = resolve(
  repositoryRoot,
  "apps/server-admin-rs/src/infra/server_i18n.json",
);

const sourceMessages = Object.fromEntries(
  Object.entries(messages).map(([locale, catalog]) => [locale, catalog.server]),
);

const isRecord = (value) =>
  value !== null && typeof value === "object" && !Array.isArray(value);

const preserveExistingKeyOrder = (existing, source) => {
  if (!isRecord(existing) || !isRecord(source)) {
    return source;
  }

  const result = {};
  for (const key of Object.keys(existing)) {
    if (Object.hasOwn(source, key)) {
      result[key] = preserveExistingKeyOrder(existing[key], source[key]);
    }
  }
  for (const key of Object.keys(source)) {
    if (!Object.hasOwn(result, key)) {
      result[key] = source[key];
    }
  }
  return result;
};

const existingMessages = JSON.parse(await readFile(outputPath, "utf8"));
const serverMessages = preserveExistingKeyOrder(
  existingMessages,
  sourceMessages,
);

await writeFile(
  outputPath,
  `${JSON.stringify(serverMessages, null, 2)}\n`,
  "utf8",
);

console.log(`[i18n] exported server catalogs to ${outputPath}`);
