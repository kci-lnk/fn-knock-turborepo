import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";

const root = path.resolve(process.env.FN_KNOCK_ROOT_DIR);
if (process.argv[2] !== "generate") {
  throw new Error("expected generate mode");
}
const version = JSON.parse(
  await readFile(path.join(root, "version.json"), "utf8"),
).version;
await writeFile(
  path.join(root, "packages/api-contract/openapi.json"),
  `${JSON.stringify({ info: { version } }, null, 2)}\n`,
  "utf8",
);
