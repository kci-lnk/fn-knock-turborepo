import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, it } from "node:test";

const sourceRoot = fileURLToPath(new URL("../src", import.meta.url));
const packageRoot = fileURLToPath(new URL("../../../packages", import.meta.url));
const sourceExtensions = [".ts", ".tsx", ".vue"];
const resolutionSuffixes = [
  "",
  ".ts",
  ".tsx",
  ".vue",
  "/index.ts",
  "/index.tsx",
  "/index.vue",
];
const importPattern =
  /(?:import|export)\s+(?:type\s+)?(?:[^"']*?\s+from\s+)?["']([^"']+)["']/gu;

const collectSourceFiles = (root: string): string[] =>
  readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const absolutePath = path.join(root, entry.name);
    if (entry.isDirectory()) return collectSourceFiles(absolutePath);
    return sourceExtensions.some((extension) => entry.name.endsWith(extension))
      ? [absolutePath]
      : [];
  });

const files = collectSourceFiles(sourceRoot);
const fileSet = new Set(files);
const resolveAppImport = (from: string, specifier: string) => {
  const base = specifier.startsWith("@/")
    ? path.join(sourceRoot, specifier.slice(2))
    : specifier.startsWith(".")
      ? path.resolve(path.dirname(from), specifier)
      : null;
  if (!base) return null;
  return (
    resolutionSuffixes
      .map((suffix) => `${base}${suffix}`)
      .find((candidate) => fileSet.has(candidate)) ?? null
  );
};
const importsFor = (file: string) =>
  [...readFileSync(file, "utf8").matchAll(importPattern)].map(
    (match) => match[1] ?? "",
  );
const graph = new Map(
  files.map((file) => [
    file,
    importsFor(file)
      .map((specifier) => resolveAppImport(file, specifier))
      .filter((dependency): dependency is string => Boolean(dependency)),
  ]),
);

describe("frontend module boundaries", () => {
  it("keeps the admin frontend local import graph acyclic", () => {
    const visiting = new Set<string>();
    const visited = new Set<string>();
    const stack: string[] = [];
    const cycles: string[] = [];
    const visit = (file: string) => {
      if (visiting.has(file)) {
        const cycleStart = stack.indexOf(file);
        cycles.push(
          [...stack.slice(cycleStart), file]
            .map((item) => path.relative(sourceRoot, item))
            .join(" -> "),
        );
        return;
      }
      if (visited.has(file)) return;
      visiting.add(file);
      stack.push(file);
      graph.get(file)?.forEach(visit);
      stack.pop();
      visiting.delete(file);
      visited.add(file);
    };
    files.forEach(visit);
    assert.deepEqual(cycles, []);
  });

  it("does not let shared app layers depend on route views", () => {
    const sharedRoots = ["components", "composables", "lib", "store"].map(
      (directory) => path.join(sourceRoot, directory) + path.sep,
    );
    const viewsRoot = path.join(sourceRoot, "views") + path.sep;
    const violations = [...graph].flatMap(([file, dependencies]) => {
      if (!sharedRoots.some((root) => file.startsWith(root))) return [];
      return dependencies
        .filter((dependency) => dependency.startsWith(viewsRoot))
        .map(
          (dependency) =>
            `${path.relative(sourceRoot, file)} -> ${path.relative(sourceRoot, dependency)}`,
        );
    });
    assert.deepEqual(violations, []);
  });

  it("does not let shared workspace packages import application source", () => {
    const violations = collectSourceFiles(packageRoot).flatMap((file) =>
      importsFor(file)
        .filter((specifier) => /(?:^|\/)apps\/server-(?:admin|auth)-view/u.test(specifier))
        .map(
          (specifier) =>
            `${path.relative(packageRoot, file)} imports ${specifier}`,
        ),
    );
    assert.deepEqual(violations, []);
  });
});
