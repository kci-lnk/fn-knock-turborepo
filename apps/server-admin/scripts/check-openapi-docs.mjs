import { createRequire } from "node:module";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { readdirSync, readFileSync, statSync } from "node:fs";

const require = createRequire(import.meta.url);
const ts = require("typescript");

const scriptDir = dirname(fileURLToPath(import.meta.url));
const serverRoot = resolve(scriptDir, "..");
const srcRoot = resolve(serverRoot, "src");
const routeRoots = [resolve(srcRoot, "routes"), resolve(srcRoot, "plugins")];
const openApiFile = resolve(srcRoot, "lib/openapi.ts");

const routeMethods = new Set([
  "get",
  "post",
  "put",
  "patch",
  "delete",
  "options",
  "head",
]);

const docHelpersPattern = /\b(routeDoc|withRouteDoc|hideFromDocs)\b/;

const readSourceFile = (filePath) =>
  ts.createSourceFile(
    filePath,
    readFileSync(filePath, "utf8"),
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TS,
  );

const walkTsFiles = (dir) => {
  const result = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const filePath = resolve(dir, entry.name);
    if (entry.isDirectory()) {
      result.push(...walkTsFiles(filePath));
    } else if (entry.isFile() && filePath.endsWith(".ts")) {
      result.push(filePath);
    }
  }
  return result;
};

const getLine = (sourceFile, position) =>
  sourceFile.getLineAndCharacterOfPosition(position).line + 1;

const isElysiaConstructor = (node) =>
  ts.isNewExpression(node) &&
  ts.isIdentifier(node.expression) &&
  node.expression.text === "Elysia";

const isElysiaChain = (node) => {
  if (isElysiaConstructor(node)) return true;
  if (!ts.isCallExpression(node)) return false;
  const expression = node.expression;
  return (
    ts.isPropertyAccessExpression(expression) &&
    isElysiaChain(expression.expression)
  );
};

const isRoutePath = (node) =>
  node &&
  (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node));

const hasRouteDoc = (node, sourceFile) =>
  node.arguments.some((arg) => docHelpersPattern.test(arg.getText(sourceFile)));

const collectKnownTags = () => {
  const sourceFile = readSourceFile(openApiFile);
  const tags = new Set();

  const visit = (node) => {
    if (
      ts.isPropertyAssignment(node) &&
      ts.isIdentifier(node.name) &&
      node.name.text === "name" &&
      ts.isStringLiteral(node.initializer)
    ) {
      tags.add(node.initializer.text);
    }
    ts.forEachChild(node, visit);
  };

  visit(sourceFile);
  return tags;
};

const collectRouteChecks = (filePath) => {
  const sourceFile = readSourceFile(filePath);
  const missingDocs = [];
  const usedTags = [];

  const visit = (node) => {
    if (isElysiaConstructor(node)) {
      const options = node.arguments?.[0];
      if (options && ts.isObjectLiteralExpression(options)) {
        for (const property of options.properties) {
          if (
            ts.isPropertyAssignment(property) &&
            ts.isIdentifier(property.name) &&
            property.name.text === "tags" &&
            ts.isArrayLiteralExpression(property.initializer)
          ) {
            for (const tag of property.initializer.elements) {
              if (ts.isStringLiteral(tag)) {
                usedTags.push({
                  filePath,
                  line: getLine(sourceFile, tag.getStart(sourceFile)),
                  tag: tag.text,
                });
              }
            }
          }
        }
      }
    }

    if (
      ts.isCallExpression(node) &&
      ts.isPropertyAccessExpression(node.expression)
    ) {
      const method = node.expression.name.text;
      const receiver = node.expression.expression;
      if (
        routeMethods.has(method) &&
        isRoutePath(node.arguments[0]) &&
        isElysiaChain(receiver) &&
        !hasRouteDoc(node, sourceFile)
      ) {
        missingDocs.push({
          filePath,
          line: getLine(sourceFile, node.expression.name.getStart(sourceFile)),
          method: method.toUpperCase(),
          path: node.arguments[0].text,
        });
      }
    }

    ts.forEachChild(node, visit);
  };

  visit(sourceFile);
  return { missingDocs, usedTags };
};

const toDisplayPath = (filePath, line) =>
  `${relative(serverRoot, filePath)}:${line}`;

const main = () => {
  const knownTags = collectKnownTags();
  const files = routeRoots
    .filter((dir) => statSync(dir, { throwIfNoEntry: false })?.isDirectory())
    .flatMap(walkTsFiles);

  const missingDocs = [];
  const usedTags = [];
  for (const filePath of files) {
    const result = collectRouteChecks(filePath);
    missingDocs.push(...result.missingDocs);
    usedTags.push(...result.usedTags);
  }

  const unknownTags = usedTags.filter(({ tag }) => !knownTags.has(tag));

  if (missingDocs.length === 0 && unknownTags.length === 0) {
    console.log(
      `OpenAPI docs check passed (${files.length} files, ${knownTags.size} known tags).`,
    );
    return;
  }

  if (missingDocs.length > 0) {
    console.error("Routes missing routeDoc/withRouteDoc/hideFromDocs:");
    for (const item of missingDocs) {
      console.error(
        `- ${toDisplayPath(item.filePath, item.line)} ${item.method} ${item.path}`,
      );
    }
  }

  if (unknownTags.length > 0) {
    console.error("Route tags missing from OpenAPI tag lists:");
    for (const item of unknownTags) {
      console.error(`- ${toDisplayPath(item.filePath, item.line)} ${item.tag}`);
    }
  }

  process.exitCode = 1;
};

main();
