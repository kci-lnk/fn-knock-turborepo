import { messages } from "../src/locales.ts";

const locales = Object.keys(messages);
const referenceLocale = "zh-CN";
const reference = messages[referenceLocale];

const flatten = (value, prefix = "", out = {}) => {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    for (const [key, child] of Object.entries(value)) {
      flatten(child, prefix ? `${prefix}.${key}` : key, out);
    }
    return out;
  }
  out[prefix] = String(value ?? "");
  return out;
};

const placeholders = (value) =>
  [...String(value).matchAll(/\{([A-Za-z0-9_]+)\}/g)]
    .map((match) => match[1])
    .sort();

const referenceFlat = flatten(reference);
const referenceKeys = Object.keys(referenceFlat).sort();
let failed = false;

for (const locale of locales) {
  const flat = flatten(messages[locale]);
  const keys = Object.keys(flat).sort();
  const missing = referenceKeys.filter((key) => !(key in flat));
  const extra = keys.filter((key) => !(key in referenceFlat));

  if (missing.length > 0 || extra.length > 0) {
    failed = true;
    console.error(`[i18n] ${locale} key mismatch`);
    for (const key of missing) console.error(`  missing: ${key}`);
    for (const key of extra) console.error(`  extra: ${key}`);
  }

  for (const key of referenceKeys) {
    if (!(key in flat)) continue;
    const expected = placeholders(referenceFlat[key]).join(",");
    const actual = placeholders(flat[key]).join(",");
    if (expected !== actual) {
      failed = true;
      console.error(
        `[i18n] ${locale}.${key} placeholder mismatch: expected {${expected}}, got {${actual}}`,
      );
    }
  }
}

if (failed) process.exit(1);
console.log(`[i18n] ${locales.length} locales passed key and placeholder checks.`);
