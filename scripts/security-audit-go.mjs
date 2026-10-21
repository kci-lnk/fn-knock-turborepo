import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const scanner = "golang.org/x/vuln/cmd/govulncheck@v1.9.0";

// govulncheck and go list emit a stream of indented JSON objects, not JSONL.
export function parseJsonStream(text) {
  const values = [];
  let start = -1,
    depth = 0,
    quoted = false,
    escaped = false;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (start < 0) {
      if (/\s/.test(ch)) continue;
      if (ch !== "{") throw new Error("Unexpected non-JSON scanner output");
      start = i;
    }
    if (quoted) {
      if (escaped) escaped = false;
      else if (ch === "\\") escaped = true;
      else if (ch === '"') quoted = false;
    } else if (ch === '"') quoted = true;
    else if (ch === "{") depth++;
    else if (ch === "}" && --depth === 0) {
      values.push(JSON.parse(text.slice(start, i + 1)));
      start = -1;
    }
  }
  if (start >= 0 || !values.length) throw new Error("Incomplete JSON report");
  return values;
}

export function validateGoBuildInfo(info, commit, goVersion, modules) {
  const settings = Object.fromEntries(
    (info.Settings ?? []).map(({ Key, Value }) => [Key, Value]),
  );
  if (
    info.Path !== "go-reauth-proxy/cmd/server" ||
    info.Main?.Path !== "go-reauth-proxy"
  ) {
    throw new Error("Artifact is not the gateway server");
  }
  if (
    settings["vcs.revision"] !== commit ||
    settings["vcs.modified"] !== "false"
  ) {
    throw new Error("Artifact source revision is stale or dirty");
  }
  if (info.GoVersion !== goVersion)
    throw new Error(
      `Artifact toolchain ${info.GoVersion} differs from audited ${goVersion}`,
    );
  if (
    !settings.GOOS ||
    !settings.GOARCH ||
    settings.CGO_ENABLED !== "0" ||
    settings["-tags"]
  ) {
    throw new Error(
      "Missing target metadata or unsupported CGO/build-tag configuration",
    );
  }
  if (!Array.isArray(info.Deps) || !info.Deps.length)
    throw new Error("Missing artifact dependencies");
  for (const dep of info.Deps) {
    const source = modules.find((entry) => entry.Path === dep.Path);
    if (
      !source ||
      source.Version !== dep.Version ||
      source.Replace?.Path !== dep.Replace?.Path ||
      (source.Replace?.Version || "(devel)") !==
        (dep.Replace?.Version || "(devel)") ||
      (dep.Sum && source.Sum !== dep.Sum)
    ) {
      throw new Error(
        `Artifact dependency differs from audited source: ${dep.Path}`,
      );
    }
  }
  return settings;
}

export function goFindings(messages, mode, imports) {
  if (
    !messages.some(
      (m) =>
        m.config?.scanner_name === "govulncheck" && m.config.scan_mode === mode,
    ) ||
    !messages.some((m) => Array.isArray(m.SBOM?.modules))
  ) {
    throw new Error("Missing govulncheck configuration or SBOM");
  }
  const findings = messages.filter((m) => m.finding).map((m) => m.finding);
  const rejected = new Set();
  const notImported = new Set();
  for (const finding of findings) {
    if (
      !finding.osv ||
      !Array.isArray(finding.trace) ||
      !finding.trace.length
    ) {
      throw new Error("Malformed govulncheck finding");
    }
    // Stripped binaries conservatively report wildcard OpenPGP symbols even
    // when that package is absent. Require same-revision, same-target source
    // evidence from go list; never suppress a real OpenPGP import or other CVE.
    if (
      mode === "binary" &&
      finding.osv === "GO-2026-5932" &&
      finding.trace[0].module === "golang.org/x/crypto" &&
      finding.trace[0].version === "v0.57.0" &&
      finding.trace.length === 1 &&
      (!finding.trace[0].package ||
        finding.trace[0].package === "golang.org/x/crypto/openpgp" ||
        finding.trace[0].package.startsWith("golang.org/x/crypto/openpgp/")) &&
      (!finding.trace[0].function || finding.trace[0].function.endsWith("*")) &&
      imports instanceof Set &&
      imports.size > 0 &&
      ![...imports].some(
        (p) =>
          p === "golang.org/x/crypto/openpgp" ||
          p.startsWith("golang.org/x/crypto/openpgp/"),
      )
    ) {
      notImported.add(finding.osv);
      continue;
    }
    if (mode === "binary" || finding.trace.some((frame) => frame.function))
      rejected.add(finding.osv);
  }
  return { rejected: [...rejected], notImported: [...notImported], findings };
}

function main() {
  const repository = path.resolve(
    process.env.FN_KNOCK_GO_REAUTH_PROXY_DIR ??
      path.join(root, "..", "Go-Reauth-Proxy"),
  );
  const reports = path.resolve(
    process.env.FN_KNOCK_SECURITY_REPORT_DIR ??
      path.join(root, "dist", "security-audit"),
  );
  mkdirSync(reports, { recursive: true });
  const run = (command, args, env = {}) => {
    const result = spawnSync(command, args, {
      cwd: repository,
      encoding: "utf8",
      maxBuffer: 64 * 1024 * 1024,
      env: { ...process.env, GOFLAGS: "-mod=readonly", ...env },
    });
    if (result.error || result.status !== 0) {
      throw new Error(
        `${command} ${args.join(" ")} failed: ${result.error?.message ?? result.stderr}`,
      );
    }
    return result.stdout;
  };
  const save = (name, data) =>
    writeFileSync(
      path.join(reports, name),
      typeof data === "string" ? data : `${JSON.stringify(data, null, 2)}\n`,
    );
  const expected =
    process.env.FN_KNOCK_GATEWAY_COMMIT ??
    process.env.FN_KNOCK_GO_SOURCE_COMMIT ??
    JSON.parse(readFileSync(path.join(root, "version.json"))).gatewayCommit;
  const commit = run("git", ["rev-parse", "HEAD"]).trim();
  if (
    commit !== expected ||
    run("git", ["status", "--porcelain", "--untracked-files=normal"]).trim()
  ) {
    throw new Error(
      "Gateway checkout must be clean and match the selected release commit",
    );
  }
  const goVersion = run("go", ["env", "GOVERSION"]).trim();
  const scannerDirectory = path.join(reports, "tools");
  mkdirSync(scannerDirectory, { recursive: true });
  const hostOS = run("go", ["env", "GOHOSTOS"]).trim();
  const hostArch = run("go", ["env", "GOHOSTARCH"]).trim();
  run("go", ["install", scanner], {
    GOBIN: scannerDirectory,
    GOOS: hostOS,
    GOARCH: hostArch,
  });
  const scannerBinary = path.join(
    scannerDirectory,
    hostOS === "windows" ? "govulncheck.exe" : "govulncheck",
  );
  const sourceCache = new Map();
  const auditSource = (env = {}) => {
    const key = `${env.GOOS ?? process.platform}-${env.GOARCH ?? process.arch}`;
    if (sourceCache.has(key)) return sourceCache.get(key);
    console.log(`[security] Go source ${commit} (${key}, ${goVersion})`);
    const raw = run(scannerBinary, ["-json", "./..."], env);
    save(`go-source-${key}.json`, raw);
    const result = goFindings(parseJsonStream(raw), "source");
    if (result.rejected.length)
      throw new Error(
        `Reachable Go vulnerabilities: ${result.rejected.join(", ")}`,
      );
    const imports = new Set(
      run("go", ["list", "-deps", "-f", "{{.ImportPath}}", "./..."], env)
        .trim()
        .split(/\r?\n/)
        .filter(Boolean),
    );
    if (!imports.has("go-reauth-proxy/cmd/server"))
      throw new Error("Incomplete gateway import evidence");
    const modules = parseJsonStream(
      run("go", ["list", "-m", "-json", "all"], env),
    );
    const evidence = {
      commit,
      goVersion,
      env,
      imports: [...imports],
      modules,
      findings: result.findings,
    };
    save(`go-source-${key}-evidence.json`, evidence);
    sourceCache.set(key, { imports, modules });
    return { imports, modules };
  };
  const args = process.argv.slice(2);
  if (!args.length) auditSource();
  else {
    if (args.shift() !== "--binary" || !args.length)
      throw new Error("Usage: security-audit-go.mjs [--binary FILE ...]");
    for (const file of args) {
      const binary = path.resolve(file);
      const info = JSON.parse(run("go", ["version", "-m", "-json", binary]));
      const settings = Object.fromEntries(
        (info.Settings ?? []).map(({ Key, Value }) => [Key, Value]),
      );
      // Reject stale artifacts before scanning any target-specific source.
      if (settings["vcs.revision"] !== commit || info.GoVersion !== goVersion)
        throw new Error(`Stale gateway artifact: ${binary}`);
      const env = {
        GOOS: settings.GOOS,
        GOARCH: settings.GOARCH,
        CGO_ENABLED: "0",
      };
      if (settings.GOARM) env.GOARM = settings.GOARM;
      const { imports, modules } = auditSource(env);
      validateGoBuildInfo(info, commit, goVersion, modules);
      const raw = run(scannerBinary, ["-mode=binary", "-json", binary]);
      const digest = createHash("sha256")
        .update(readFileSync(binary))
        .digest("hex");
      const name = `${path.basename(binary)}-${digest.slice(0, 12)}`;
      save(`${name}.json`, raw);
      const result = goFindings(parseJsonStream(raw), "binary", imports);
      save(`${name}-evidence.json`, {
        commit,
        sha256: digest,
        info,
        ...result,
      });
      if (result.rejected.length)
        throw new Error(
          `Artifact vulnerabilities: ${result.rejected.join(", ")}`,
        );
      console.log(
        `[security] ${binary}: passed; absent-package findings=${result.notImported.join(",") || "none"}`,
      );
    }
  }
  if (
    run("git", ["rev-parse", "HEAD"]).trim() !== commit ||
    run("git", ["status", "--porcelain", "--untracked-files=normal"]).trim()
  ) {
    throw new Error("Gateway source changed during audit");
  }
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href
) {
  try {
    main();
  } catch (error) {
    console.error(`[security] ${error.message}`);
    process.exitCode = 1;
  }
}
