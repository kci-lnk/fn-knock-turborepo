import { spawn } from "node:child_process";
import {
  mkdir,
  mkdtemp,
  readFile,
  rename,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { collectStreamOutput, fileExists, waitForProcessExit } from "./runtime";
import { tDefault } from "./i18n";

export const SMART_CONNECT_DNS_PORT = 53;
export const SMART_CONNECT_LOCAL_TTL_SECONDS = 30;
export const SMART_CONNECT_MANAGED_CONF_PATH =
  "/etc/dnsmasq.d/fn-knock-smart-connect.conf";

export type DnsmasqInstallStatus =
  | "uninstalled"
  | "installing"
  | "installed"
  | "error";

export interface DnsmasqInstallState {
  status: DnsmasqInstallStatus;
  progress: number;
  message: string;
}

export interface DnsmasqStatus {
  installed: boolean;
  service_active: boolean;
  initialized: boolean;
  version: string;
  install_state: DnsmasqInstallState;
}

type ExecResult = {
  code: number;
  stdout: string;
  stderr: string;
};

type DnsmasqExecutableInfo = {
  path: string;
  version: string;
};

const dnsmasqT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.dnsmasq.${key}`, params);

const DNSMASQ_EXECUTABLE_CANDIDATES = [
  "dnsmasq",
  "/usr/sbin/dnsmasq",
  "/usr/bin/dnsmasq",
] as const;
const DEBIAN_APT_GET_PATH = "/usr/bin/apt-get";
const DNSMASQ_SYSTEMD_UNIT_CANDIDATES = [
  "/etc/systemd/system/dnsmasq.service",
  "/lib/systemd/system/dnsmasq.service",
  "/usr/lib/systemd/system/dnsmasq.service",
] as const;
const DNSMASQ_INIT_SCRIPT_PATH = "/etc/init.d/dnsmasq";

const createDefaultInstallState = (): DnsmasqInstallState => ({
  status: "uninstalled",
  progress: 0,
  message: dnsmasqT("notDetectedInstallFirst"),
});

class DnsmasqManager {
  private executableInfoPromise: Promise<DnsmasqExecutableInfo | null> | null =
    null;
  private installPromise: Promise<void> | null = null;
  private installState: DnsmasqInstallState = createDefaultInstallState();

  private async runProcess(
    command: string,
    args: string[],
    options: { trimOutput?: boolean } = {},
  ): Promise<ExecResult> {
    const proc = spawn(command, args, {
      env: process.env,
      stdio: ["ignore", "pipe", "pipe"],
    });

    const [stdout, stderr, code] = await Promise.all([
      collectStreamOutput(proc.stdout),
      collectStreamOutput(proc.stderr),
      waitForProcessExit(proc),
    ]);

    if (options.trimOutput === false) {
      return { code, stdout, stderr };
    }

    return {
      code,
      stdout: stdout.trimEnd(),
      stderr: stderr.trimEnd(),
    };
  }

  private summarizeResult(result: ExecResult): string {
    const detail = `${result.stderr}\n${result.stdout}`.trim();
    if (!detail) {
      return "";
    }

    return detail
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean)
      .slice(-8)
      .join(" | ")
      .slice(0, 500);
  }

  private normalizeDnsmasqError(message: string, fallback: string): string {
    const detail = message.trim();
    const lower = detail.toLowerCase();

    if (
      lower.includes("address already in use") ||
      lower.includes("failed to create listening socket") ||
      lower.includes("failed to bind listening socket") ||
      lower.includes("permission denied")
    ) {
      return detail
        ? dnsmasqT("dnsPortUnavailableWithDetail", { detail })
        : dnsmasqT("dnsPortUnavailable");
    }

    return detail || fallback;
  }

  private async ensureProcessSucceeded(
    command: string,
    args: string[],
    failureMessage: string,
  ): Promise<ExecResult> {
    const result = await this.runProcess(command, args, { trimOutput: false });
    if (result.code === 0) {
      return {
        ...result,
        stdout: result.stdout.trimEnd(),
        stderr: result.stderr.trimEnd(),
      };
    }

    throw new Error(
      this.normalizeDnsmasqError(this.summarizeResult(result), failureMessage),
    );
  }

  private resetExecutableProbeCache(): void {
    this.executableInfoPromise = null;
  }

  private async detectExecutable(): Promise<DnsmasqExecutableInfo | null> {
    if (!this.executableInfoPromise) {
      this.executableInfoPromise = (async () => {
        for (const candidate of DNSMASQ_EXECUTABLE_CANDIDATES) {
          try {
            const result = await this.runProcess(candidate, ["--version"]);
            if (result.code !== 0) {
              continue;
            }

            const version =
              result.stdout.split(/\r?\n/)[0]?.trim() || "dnsmasq";
            return {
              path: candidate,
              version,
            };
          } catch {
            continue;
          }
        }

        return null;
      })();
    }

    const executable = await this.executableInfoPromise;
    if (!executable) {
      this.executableInfoPromise = null;
    }
    return executable;
  }

  private async ensureManagedDirectory(): Promise<void> {
    await mkdir(dirname(SMART_CONNECT_MANAGED_CONF_PATH), {
      recursive: true,
    });
  }

  private buildBootstrapConfig(): string {
    return [
      `local-ttl=${SMART_CONNECT_LOCAL_TTL_SECONDS}`,
      "listen-address=127.0.0.1",
      "bind-interfaces",
      "",
    ].join("\n");
  }

  private async hasSystemdUnit(): Promise<boolean> {
    for (const unitPath of DNSMASQ_SYSTEMD_UNIT_CANDIDATES) {
      if (await fileExists(unitPath)) {
        return true;
      }
    }
    return false;
  }

  private async hasInitScript(): Promise<boolean> {
    return fileExists(DNSMASQ_INIT_SCRIPT_PATH);
  }

  private async hasServiceDefinition(): Promise<boolean> {
    const [hasSystemdUnit, hasInitScript] = await Promise.all([
      this.hasSystemdUnit(),
      this.hasInitScript(),
    ]);
    return hasSystemdUnit || hasInitScript;
  }

  private getDetectedState(
    version: string,
    hasServiceDefinition: boolean,
  ): DnsmasqInstallState {
    return {
      status: "installed",
      progress: 100,
      message: hasServiceDefinition
        ? version
          ? dnsmasqT("detectedWithVersion", { version })
          : dnsmasqT("detected")
        : version
          ? dnsmasqT("missingServiceAutoComplete")
          : dnsmasqT("missingServiceAutoComplete"),
    };
  }

  private async ensureServicePackageInstalled(): Promise<void> {
    if (await this.hasServiceDefinition()) {
      return;
    }

    if (!(await fileExists(DEBIAN_APT_GET_PATH))) {
      throw new Error(dnsmasqT("servicePackageMissing"));
    }

    this.installState = {
      status: "installing",
      progress: 58,
      message: dnsmasqT("completingService"),
    };
    await this.ensureProcessSucceeded(
      DEBIAN_APT_GET_PATH,
      ["install", "-y", "dnsmasq"],
      dnsmasqT("completeServiceFailed"),
    );

    if (!(await this.hasServiceDefinition())) {
      throw new Error(dnsmasqT("serviceDefinitionMissingAfterInstall"));
    }
  }

  private async validateConfigContent(content: string): Promise<void> {
    const executable = await this.detectExecutable();
    if (!executable) {
      throw new Error(dnsmasqT("executableMissing"));
    }

    const tempDir = await mkdtemp(join(tmpdir(), "fn-knock-dnsmasq-"));
    const tempConfPath = join(tempDir, "dnsmasq.conf");

    try {
      await writeFile(tempConfPath, content, "utf8");
      await this.ensureProcessSucceeded(
        executable.path,
        ["--test", `--conf-file=${tempConfPath}`],
        dnsmasqT("configTestFailed"),
      );
    } finally {
      await rm(tempDir, { recursive: true, force: true });
    }
  }

  private async isManagedConfigWritable(): Promise<boolean> {
    try {
      await this.ensureManagedDirectory();
      const testPath = join(
        dirname(SMART_CONNECT_MANAGED_CONF_PATH),
        `.fn-knock-write-test-${Date.now()}`,
      );
      await writeFile(testPath, "", "utf8");
      await rm(testPath, { force: true });
      return true;
    } catch {
      return false;
    }
  }

  private async getServiceActive(): Promise<boolean> {
    if (await this.hasSystemdUnit()) {
      try {
        const result = await this.runProcess("systemctl", [
          "is-active",
          "--quiet",
          "dnsmasq",
        ]);
        if (result.code === 0) {
          return true;
        }
      } catch {
        // fall through
      }
    }

    if (await this.hasInitScript()) {
      try {
        const result = await this.runProcess("service", ["dnsmasq", "status"]);
        return result.code === 0;
      } catch {
        return false;
      }
    }

    return false;
  }

  private async restartService(): Promise<void> {
    const errors: string[] = [];

    if (await this.hasSystemdUnit()) {
      try {
        await this.ensureProcessSucceeded(
          "systemctl",
          ["restart", "dnsmasq"],
          dnsmasqT("restartFailed"),
        );
        return;
      } catch (error) {
        if (error instanceof Error && error.message.trim()) {
          errors.push(error.message.trim());
        }
      }
    }

    if (await this.hasInitScript()) {
      try {
        await this.ensureProcessSucceeded(
          "service",
          ["dnsmasq", "restart"],
          dnsmasqT("restartFailed"),
        );
        return;
      } catch (error) {
        if (error instanceof Error && error.message.trim()) {
          errors.push(error.message.trim());
        }
      }
    }

    if (errors.length === 0) {
      throw new Error(dnsmasqT("serviceDefinitionMissing"));
    }

    throw new Error(
      this.normalizeDnsmasqError(errors.join(" | "), dnsmasqT("restartFailed")),
    );
  }

  private async enableServiceOnBoot(): Promise<void> {
    if (await this.hasSystemdUnit()) {
      try {
        const result = await this.runProcess("systemctl", [
          "enable",
          "dnsmasq",
        ]);
        if (result.code !== 0) {
          console.warn(
            "[dnsmasq] failed to enable service on boot:",
            this.summarizeResult(result),
          );
        }
      } catch (error) {
        console.warn("[dnsmasq] skip enabling service on boot:", error);
      }
      return;
    }

    if (await this.hasInitScript()) {
      try {
        const result = await this.runProcess("update-rc.d", [
          "dnsmasq",
          "defaults",
        ]);
        if (result.code !== 0) {
          console.warn(
            "[dnsmasq] failed to register init script on boot:",
            this.summarizeResult(result),
          );
        }
      } catch (error) {
        console.warn("[dnsmasq] skip registering init script on boot:", error);
      }
    }
  }

  private getInstalledState(version: string): DnsmasqInstallState {
    return {
      status: "installed",
      progress: 100,
      message: version
        ? dnsmasqT("readyWithVersion", { version })
        : dnsmasqT("ready"),
    };
  }

  async getStatus(): Promise<DnsmasqStatus> {
    if (this.installState.status === "installing") {
      const executable = await this.detectExecutable();
      return {
        installed: executable !== null,
        service_active: await this.getServiceActive(),
        initialized: false,
        version: executable?.version || "",
        install_state: this.installState,
      };
    }

    const executable = await this.detectExecutable();
    if (!executable) {
      return {
        installed: false,
        service_active: false,
        initialized: false,
        version: "",
        install_state:
          this.installState.status === "error"
            ? this.installState
            : createDefaultInstallState(),
      };
    }

    const [serviceActive, writable, hasServiceDefinition] = await Promise.all([
      this.getServiceActive(),
      this.isManagedConfigWritable(),
      this.hasServiceDefinition(),
    ]);

    let initialized = false;
    if (writable) {
      try {
        await this.validateConfigContent(this.buildBootstrapConfig());
        initialized = true;
      } catch {
        initialized = false;
      }
    }

    return {
      installed: true,
      service_active: serviceActive,
      initialized,
      version: executable.version,
      install_state:
        (!serviceActive || !initialized) && this.installState.status === "error"
          ? this.installState
          : serviceActive && initialized
            ? this.getInstalledState(executable.version)
            : this.getDetectedState(executable.version, hasServiceDefinition),
    };
  }

  buildManagedConfig(selectedIpv4: string, domains: string[]): string {
    const normalizedIpv4 = selectedIpv4.trim();
    const normalizedDomains = Array.from(
      new Set(domains.map((item) => item.trim().toLowerCase()).filter(Boolean)),
    );
    const listenAddresses = Array.from(
      new Set(["127.0.0.1", normalizedIpv4].filter(Boolean)),
    );

    return [
      "# Managed by fn-knock smart connect. Do not edit manually.",
      `local-ttl=${SMART_CONNECT_LOCAL_TTL_SECONDS}`,
      `listen-address=${listenAddresses.join(",")}`,
      "bind-interfaces",
      ...normalizedDomains.flatMap((domain) => [
        `address=/${domain}/${normalizedIpv4}`,
        `local=/${domain}/`,
      ]),
      "",
    ].join("\n");
  }

  async applyManagedConfig(input: {
    selectedIpv4: string;
    domains: string[];
  }): Promise<void> {
    const nextContent = this.buildManagedConfig(
      input.selectedIpv4,
      input.domains,
    );

    await this.ensureManagedDirectory();
    await this.validateConfigContent(nextContent);

    const previousExists = await fileExists(SMART_CONNECT_MANAGED_CONF_PATH);
    const previousContent = previousExists
      ? await readFile(SMART_CONNECT_MANAGED_CONF_PATH, "utf8")
      : null;
    const stagePath = `${SMART_CONNECT_MANAGED_CONF_PATH}.tmp`;

    try {
      await writeFile(stagePath, nextContent, "utf8");
      await rename(stagePath, SMART_CONNECT_MANAGED_CONF_PATH);
      await this.restartService();
    } catch (error) {
      await rm(stagePath, { force: true });

      if (previousExists && previousContent !== null) {
        await writeFile(
          SMART_CONNECT_MANAGED_CONF_PATH,
          previousContent,
          "utf8",
        );
      } else {
        await rm(SMART_CONNECT_MANAGED_CONF_PATH, { force: true });
      }

      try {
        await this.restartService();
      } catch (rollbackError) {
        console.error(
          "[dnsmasq] failed to restart service while rolling back managed config:",
          rollbackError,
        );
      }

      throw error;
    }
  }

  async clearManagedConfig(): Promise<void> {
    const previousExists = await fileExists(SMART_CONNECT_MANAGED_CONF_PATH);
    if (!previousExists) {
      return;
    }

    const previousContent = await readFile(
      SMART_CONNECT_MANAGED_CONF_PATH,
      "utf8",
    );

    try {
      await rm(SMART_CONNECT_MANAGED_CONF_PATH, { force: true });
      await this.restartService();
    } catch (error) {
      await writeFile(SMART_CONNECT_MANAGED_CONF_PATH, previousContent, "utf8");
      try {
        await this.restartService();
      } catch (rollbackError) {
        console.error(
          "[dnsmasq] failed to restart service while restoring managed config:",
          rollbackError,
        );
      }
      throw error;
    }
  }

  private async installInBackground(): Promise<void> {
    try {
      this.installState = {
        status: "installing",
        progress: 15,
        message: dnsmasqT("refreshingApt"),
      };
      await this.ensureProcessSucceeded(
        DEBIAN_APT_GET_PATH,
        ["update"],
        dnsmasqT("aptUpdateFailed"),
      );

      this.installState = {
        status: "installing",
        progress: 55,
        message: dnsmasqT("installing"),
      };
      await this.ensureProcessSucceeded(
        DEBIAN_APT_GET_PATH,
        ["install", "-y", "dnsmasq"],
        dnsmasqT("aptInstallFailed"),
      );

      this.installState = {
        status: "installing",
        progress: 78,
        message: dnsmasqT("enablingService"),
      };
      await this.enableServiceOnBoot();

      this.installState = {
        status: "installing",
        progress: 90,
        message: dnsmasqT("verifyingService"),
      };
      this.resetExecutableProbeCache();
      await this.restartService();

      const executable = await this.detectExecutable();
      if (!executable) {
        throw new Error(dnsmasqT("installMissingAfterComplete"));
      }

      this.installState = this.getInstalledState(executable.version);
    } catch (error) {
      this.resetExecutableProbeCache();
      this.installState = {
        status: "error",
        progress: 0,
        message:
          error instanceof Error && error.message.trim()
            ? error.message.trim()
            : dnsmasqT("installFailed"),
      };
    }
  }

  private async initializeInBackground(): Promise<void> {
    try {
      this.installState = {
        status: "installing",
        progress: 20,
        message: dnsmasqT("checkingEnvironment"),
      };
      this.resetExecutableProbeCache();
      const executable = await this.detectExecutable();
      if (!executable) {
        throw new Error(dnsmasqT("notDetectedInstallFirst"));
      }

      this.installState = {
        status: "installing",
        progress: 45,
        message: dnsmasqT("validatingConfig"),
      };
      await this.ensureServicePackageInstalled();
      await this.ensureManagedDirectory();
      await this.validateConfigContent(this.buildBootstrapConfig());

      this.installState = {
        status: "installing",
        progress: 72,
        message: dnsmasqT("enablingService"),
      };
      await this.enableServiceOnBoot();

      this.installState = {
        status: "installing",
        progress: 90,
        message: dnsmasqT("startingService"),
      };
      await this.restartService();

      this.installState = this.getInstalledState(executable.version);
    } catch (error) {
      this.resetExecutableProbeCache();
      this.installState = {
        status: "error",
        progress: 0,
        message:
          error instanceof Error && error.message.trim()
            ? error.message.trim()
            : dnsmasqT("initializeFailed"),
      };
    }
  }

  async startInstall(): Promise<DnsmasqInstallState> {
    const status = await this.getStatus();
    if (status.install_state.status === "installing") {
      return status.install_state;
    }

    if (status.installed && status.service_active && status.initialized) {
      return status.install_state;
    }

    if (!this.installPromise) {
      this.installPromise = (
        status.installed
          ? this.initializeInBackground()
          : this.installInBackground()
      ).finally(() => {
        this.installPromise = null;
      });
    }

    return this.installState;
  }
}

export const dnsmasqManager = new DnsmasqManager();
