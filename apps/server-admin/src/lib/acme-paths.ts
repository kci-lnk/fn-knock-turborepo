import fs from "node:fs";
import path from "node:path";
import { dataPath } from "./AppDirManager";

export const ACME_HOME_DIR = path.join(dataPath, ".acme.sh");
export const ACME_EXECUTABLE_PATH = path.join(ACME_HOME_DIR, "acme.sh");

export const resolveBundledAcmeZipPath = (metaDir: string): string | null => {
  const envPath = process.env.ACME_BUNDLE_ZIP?.trim();
  const candidates = [
    envPath,
    path.join(metaDir, "resources", "acmesh.zip"),
    path.join(metaDir, "../resources/acmesh.zip"),
    path.join(metaDir, "../../resources/acmesh.zip"),
    path.join(metaDir, "../../../resources/acmesh.zip"),
    path.join(process.cwd(), "resources/acmesh.zip"),
    path.join(process.cwd(), "apps/server-admin/resources/acmesh.zip"),
    path.join(process.cwd(), "server/server-admin/resources/acmesh.zip"),
  ].filter((v): v is string => !!v);

  const seen = new Set<string>();
  for (const p of candidates) {
    const resolved = path.resolve(p);
    if (seen.has(resolved)) continue;
    seen.add(resolved);
    if (fs.existsSync(resolved)) return resolved;
  }
  return null;
};
