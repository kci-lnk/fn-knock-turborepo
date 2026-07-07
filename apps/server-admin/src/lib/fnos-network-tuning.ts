import { spawn } from "node:child_process";
import { mkdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import { dirname } from "node:path";

import {
  configManager,
  type FnosNetworkTuningConfig,
} from "./redis";
import {
  getRuntimeProfile,
  type RuntimeProfile,
} from "./runtime-profile";
import { collectStreamOutput, waitForProcessExit } from "./runtime";

export const FNOS_NETWORK_TUNING_SYSCTL_PATH =
  process.env.FN_KNOCK_NETWORK_SYSCTL_PATH?.trim() ||
  "/etc/sysctl.d/99-fn-knock-network.conf";

const TCP_CONGESTION_CONTROL_KEY = "net.ipv4.tcp_congestion_control";
const TCP_AVAILABLE_CONGESTION_CONTROL_KEY =
  "net.ipv4.tcp_available_congestion_control";
const DEFAULT_QDISC_KEY = "net.core.default_qdisc";
const TCP_MTU_PROBING_KEY = "net.ipv4.tcp_mtu_probing";

export interface FnosNetworkTuningPatch {
  bbr_enabled?: boolean;
  mtu_probing_enabled?: boolean;
}

export interface CommandResult {
  code: number;
  stdout: string;
  stderr: string;
}

export interface FnosNetworkTuningKernelState {
  tcp_congestion_control: string | null;
  tcp_available_congestion_control: string[];
  default_qdisc: string | null;
  tcp_mtu_probing: string | null;
  bbr_module_loaded: boolean;
  bbr_supported: boolean;
  bbr_active: boolean;
  mtu_probing_active: boolean;
}

export interface FnosNetworkTuningStatus {
  available: boolean;
  blocked_reason_code: FnosNetworkTuningBlockedReasonCode | null;
  blocked_reason: string | null;
  managed_config_path: string;
  config: FnosNetworkTuningConfig;
  state: FnosNetworkTuningKernelState;
  bbr: {
    desired_enabled: boolean;
    active: boolean;
    supported: boolean;
    module_loaded: boolean;
    current_congestion_control: string | null;
    current_default_qdisc: string | null;
    available_congestion_control: string[];
  };
  mtu_probing: {
    desired_enabled: boolean;
    active: boolean;
    current_value: string | null;
  };
  last_error: string | null;
}

export type FnosNetworkTuningBlockedReasonCode =
  | "deployment"
  | "platform"
  | "permission";

type ConfigAccess = {
  getConfig: () => Promise<FnosNetworkTuningConfig>;
  updateConfig: (
    patch: Partial<FnosNetworkTuningConfig>,
  ) => Promise<FnosNetworkTuningConfig>;
};

type RuntimeTransitionTargets = {
  disabled_bbr_congestion_control?: string;
  disabled_bbr_default_qdisc?: string;
  disabled_tcp_mtu_probing?: string;
};

export type FnosNetworkTuningDeps = Partial<{
  configAccess: ConfigAccess;
  getRuntimeProfile: () => RuntimeProfile;
  managedConfigPath: string;
  runCommand: (command: string, args: string[]) => Promise<CommandResult>;
  readFile: (path: string) => Promise<string>;
  writeFile: (path: string, content: string) => Promise<void>;
  rename: (oldPath: string, newPath: string) => Promise<void>;
  mkdir: (path: string) => Promise<void>;
  rm: (path: string) => Promise<void>;
}>;

export class FnosNetworkTuningUnavailableError extends Error {
  constructor(
    message: string,
    readonly reasonCode: FnosNetworkTuningBlockedReasonCode,
  ) {
    super(message);
    this.name = "FnosNetworkTuningUnavailableError";
  }
}

const trimValue = (value: string | null | undefined): string | null => {
  const normalized = String(value ?? "").trim();
  return normalized || null;
};

const boolPatchValue = (value: unknown): boolean | undefined =>
  typeof value === "boolean" ? value : undefined;

const defaultConfigAccess: ConfigAccess = {
  getConfig: () => configManager.getFnosNetworkTuningConfig(),
  updateConfig: (patch) => configManager.updateFnosNetworkTuningConfig(patch),
};

const runCommand = async (
  command: string,
  args: string[],
): Promise<CommandResult> => {
  const proc = spawn(command, args, {
    env: process.env,
    stdio: ["ignore", "pipe", "pipe"],
  });

  const [stdout, stderr, code] = await Promise.all([
    collectStreamOutput(proc.stdout),
    collectStreamOutput(proc.stderr),
    waitForProcessExit(proc),
  ]);

  return {
    code,
    stdout: stdout.trimEnd(),
    stderr: stderr.trimEnd(),
  };
};

export class FnosNetworkTuningService {
  private readonly configAccess: ConfigAccess;
  private readonly getProfile: () => RuntimeProfile;
  private readonly managedConfigPath: string;
  private readonly run: (command: string, args: string[]) => Promise<CommandResult>;
  private readonly readTextFile: (path: string) => Promise<string>;
  private readonly writeTextFile: (path: string, content: string) => Promise<void>;
  private readonly renameFile: (oldPath: string, newPath: string) => Promise<void>;
  private readonly makeDir: (path: string) => Promise<void>;
  private readonly removeFile: (path: string) => Promise<void>;
  private updateLock: Promise<void> = Promise.resolve();

  constructor(deps: FnosNetworkTuningDeps = {}) {
    this.configAccess = deps.configAccess ?? defaultConfigAccess;
    this.getProfile = deps.getRuntimeProfile ?? getRuntimeProfile;
    this.managedConfigPath =
      deps.managedConfigPath ?? FNOS_NETWORK_TUNING_SYSCTL_PATH;
    this.run = deps.runCommand ?? runCommand;
    this.readTextFile = deps.readFile ?? ((path) => readFile(path, "utf-8"));
    this.writeTextFile =
      deps.writeFile ?? ((path, content) => writeFile(path, content));
    this.renameFile = deps.rename ?? rename;
    this.makeDir =
      deps.mkdir ??
      (async (path) => {
        await mkdir(path, { recursive: true });
      });
    this.removeFile = deps.rm ?? ((path) => rm(path, { force: true }));
  }

  normalizePatch(patch: FnosNetworkTuningPatch): FnosNetworkTuningPatch {
    return {
      bbr_enabled: boolPatchValue(patch.bbr_enabled),
      mtu_probing_enabled: boolPatchValue(patch.mtu_probing_enabled),
    };
  }

  private getBlockedReasonCode(): FnosNetworkTuningBlockedReasonCode | null {
    const profile = this.getProfile();

    if (profile.deployment_target !== "fpk") {
      return "deployment";
    }
    if (!profile.is_linux) {
      return "platform";
    }
    if (!profile.is_root_process) {
      return "permission";
    }

    return null;
  }

  private getBlockedReason(): string | null {
    const code = this.getBlockedReasonCode();
    if (code === "deployment") {
      return "FNOS network tuning is only available in FPK deployments.";
    }
    if (code === "platform") {
      return "FNOS network tuning requires a Linux host.";
    }
    if (code === "permission") {
      return "FNOS network tuning requires root permission.";
    }
    return null;
  }

  isWriteAvailable(): boolean {
    return this.getBlockedReasonCode() === null;
  }

  private assertWriteAvailable(): void {
    const blockedReasonCode = this.getBlockedReasonCode();
    if (blockedReasonCode) {
      throw new FnosNetworkTuningUnavailableError(
        this.getBlockedReason() ?? "FNOS network tuning is unavailable.",
        blockedReasonCode,
      );
    }
  }

  private async runExclusive<T>(operation: () => Promise<T>): Promise<T> {
    const previousLock = this.updateLock;
    let releaseLock: () => void = () => {};
    this.updateLock = new Promise<void>((resolve) => {
      releaseLock = resolve;
    });

    await previousLock;
    try {
      return await operation();
    } finally {
      releaseLock();
    }
  }

  private summarizeResult(result: CommandResult): string {
    return `${result.stderr}\n${result.stdout}`
      .trim()
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean)
      .slice(-8)
      .join(" | ")
      .slice(0, 500);
  }

  private async runRequired(
    command: string,
    args: string[],
    failureMessage: string,
  ): Promise<CommandResult> {
    const result = await this.run(command, args);
    if (result.code === 0) {
      return result;
    }

    const detail = this.summarizeResult(result);
    throw new Error(detail ? `${failureMessage}: ${detail}` : failureMessage);
  }

  private async readSysctlValue(key: string): Promise<string | null> {
    try {
      const result = await this.run("sysctl", ["-n", key]);
      if (result.code !== 0) {
        return null;
      }
      return trimValue(result.stdout);
    } catch {
      return null;
    }
  }

  private async setSysctlValue(key: string, value: string): Promise<void> {
    await this.runRequired(
      "sysctl",
      ["-w", `${key}=${value}`],
      `Failed to set ${key}`,
    );
  }

  private uniqueCandidates(
    values: Array<string | null | undefined>,
  ): string[] {
    return [
      ...new Set(
        values
          .map((value) => value?.trim() || "")
          .filter((value) => value.length > 0),
      ),
    ];
  }

  private async setSysctlValueFromCandidates(
    key: string,
    candidates: string[],
  ): Promise<string> {
    let lastError: unknown = null;

    for (const candidate of candidates) {
      try {
        await this.setSysctlValue(key, candidate);
        return candidate;
      } catch (error) {
        lastError = error;
      }
    }

    if (lastError instanceof Error) {
      throw lastError;
    }
    throw new Error(`Failed to set ${key}`);
  }

  private async readBbrModuleLoaded(): Promise<boolean> {
    try {
      const modules = await this.readTextFile("/proc/modules");
      return modules
        .split(/\r?\n/)
        .some((line) => line.split(/\s+/)[0] === "tcp_bbr");
    } catch {
      return false;
    }
  }

  async readKernelState(): Promise<FnosNetworkTuningKernelState> {
    const [
      congestionControl,
      availableCongestionRaw,
      defaultQdisc,
      mtuProbing,
      bbrModuleLoaded,
    ] = await Promise.all([
      this.readSysctlValue(TCP_CONGESTION_CONTROL_KEY),
      this.readSysctlValue(TCP_AVAILABLE_CONGESTION_CONTROL_KEY),
      this.readSysctlValue(DEFAULT_QDISC_KEY),
      this.readSysctlValue(TCP_MTU_PROBING_KEY),
      this.readBbrModuleLoaded(),
    ]);

    const availableCongestionControl = (availableCongestionRaw ?? "")
      .split(/\s+/)
      .map((value) => value.trim())
      .filter(Boolean);
    const bbrSupported =
      availableCongestionControl.includes("bbr") ||
      congestionControl === "bbr";

    return {
      tcp_congestion_control: congestionControl,
      tcp_available_congestion_control: availableCongestionControl,
      default_qdisc: defaultQdisc,
      tcp_mtu_probing: mtuProbing,
      bbr_module_loaded: bbrModuleLoaded,
      bbr_supported: bbrSupported,
      bbr_active: congestionControl === "bbr" && defaultQdisc === "fq",
      mtu_probing_active: mtuProbing === "1",
    };
  }

  private renderManagedConfig(config: FnosNetworkTuningConfig): string {
    const lines = [
      "# Managed by fn-knock. Do not edit this file manually.",
      "# Source: System settings -> FNOS network tuning.",
    ];

    if (config.bbr_enabled) {
      lines.push(
        `${DEFAULT_QDISC_KEY}=fq`,
        `${TCP_CONGESTION_CONTROL_KEY}=bbr`,
      );
    }

    lines.push(
      `${TCP_MTU_PROBING_KEY}=${config.mtu_probing_enabled ? "1" : "0"}`,
    );

    return `${lines.join("\n")}\n`;
  }

  private async writeManagedConfig(
    config: FnosNetworkTuningConfig,
  ): Promise<void> {
    const content = this.renderManagedConfig(config);
    if (!content) {
      await this.removeFile(this.managedConfigPath);
      return;
    }

    await this.makeDir(dirname(this.managedConfigPath));
    const tmpPath = `${this.managedConfigPath}.${process.pid}.${Date.now()}.tmp`;
    await this.writeTextFile(tmpPath, content);
    await this.renameFile(tmpPath, this.managedConfigPath);
  }

  private buildNextConfig(
    previousConfig: FnosNetworkTuningConfig,
    patch: FnosNetworkTuningPatch,
    beforeState: FnosNetworkTuningKernelState,
  ): FnosNetworkTuningConfig {
    const next: FnosNetworkTuningConfig = {
      ...previousConfig,
      last_error: null,
    };

    if (patch.bbr_enabled !== undefined) {
      if (patch.bbr_enabled && !previousConfig.bbr_enabled) {
        next.previous_tcp_congestion_control =
          beforeState.tcp_congestion_control;
        next.previous_default_qdisc = beforeState.default_qdisc;
      }
      next.bbr_enabled = patch.bbr_enabled;
    }

    if (patch.mtu_probing_enabled !== undefined) {
      if (patch.mtu_probing_enabled && !previousConfig.mtu_probing_enabled) {
        next.previous_tcp_mtu_probing = beforeState.tcp_mtu_probing;
      }
      next.mtu_probing_enabled = patch.mtu_probing_enabled;
    }

    return next;
  }

  private async ensureBbrSupported(): Promise<FnosNetworkTuningKernelState> {
    try {
      await this.run("modprobe", ["tcp_bbr"]);
    } catch {
      // Continue and verify through tcp_available_congestion_control below.
    }

    const state = await this.readKernelState();
    if (!state.bbr_supported) {
      throw new Error("The host kernel does not expose tcp_bbr.");
    }
    return state;
  }

  private getCongestionFallback(availableCongestionControl: string[]): string {
    if (availableCongestionControl.includes("cubic")) {
      return "cubic";
    }
    return (
      availableCongestionControl.find((value) => value && value !== "bbr") ||
      "cubic"
    );
  }

  private async applyRuntimeTransition(
    previousConfig: FnosNetworkTuningConfig,
    nextConfig: FnosNetworkTuningConfig,
    patch: FnosNetworkTuningPatch,
    beforeState: FnosNetworkTuningKernelState,
  ): Promise<RuntimeTransitionTargets> {
    const targets: RuntimeTransitionTargets = {};

    if (patch.bbr_enabled === true) {
      await this.ensureBbrSupported();
      await this.setSysctlValue(DEFAULT_QDISC_KEY, "fq");
      await this.setSysctlValue(TCP_CONGESTION_CONTROL_KEY, "bbr");
    } else if (patch.bbr_enabled === false) {
      const fallbackCongestion = this.getCongestionFallback(
        beforeState.tcp_available_congestion_control,
      );
      targets.disabled_bbr_congestion_control =
        await this.setSysctlValueFromCandidates(
          TCP_CONGESTION_CONTROL_KEY,
          this.uniqueCandidates([
            nextConfig.previous_tcp_congestion_control !== "bbr"
              ? nextConfig.previous_tcp_congestion_control
              : null,
            fallbackCongestion,
          ]),
        );
      targets.disabled_bbr_default_qdisc =
        await this.setSysctlValueFromCandidates(
          DEFAULT_QDISC_KEY,
          this.uniqueCandidates([
            nextConfig.previous_default_qdisc,
            "pfifo_fast",
          ]),
        );
    } else if (nextConfig.bbr_enabled && !previousConfig.bbr_enabled) {
      await this.ensureBbrSupported();
      await this.setSysctlValue(DEFAULT_QDISC_KEY, "fq");
      await this.setSysctlValue(TCP_CONGESTION_CONTROL_KEY, "bbr");
    }

    if (patch.mtu_probing_enabled === true) {
      await this.setSysctlValue(TCP_MTU_PROBING_KEY, "1");
    } else if (patch.mtu_probing_enabled === false) {
      await this.setSysctlValue(TCP_MTU_PROBING_KEY, "0");
      targets.disabled_tcp_mtu_probing = "0";
    }

    return targets;
  }

  private async restoreRuntime(
    previousConfig: FnosNetworkTuningConfig,
    beforeState: FnosNetworkTuningKernelState,
  ): Promise<void> {
    if (previousConfig.bbr_enabled) {
      await this.ensureBbrSupported();
      await this.setSysctlValue(DEFAULT_QDISC_KEY, "fq");
      await this.setSysctlValue(TCP_CONGESTION_CONTROL_KEY, "bbr");
    } else {
      if (beforeState.tcp_congestion_control) {
        await this.setSysctlValue(
          TCP_CONGESTION_CONTROL_KEY,
          beforeState.tcp_congestion_control,
        );
      }
      if (beforeState.default_qdisc) {
        await this.setSysctlValue(DEFAULT_QDISC_KEY, beforeState.default_qdisc);
      }
    }

    if (previousConfig.mtu_probing_enabled) {
      await this.setSysctlValue(TCP_MTU_PROBING_KEY, "1");
    } else {
      await this.setSysctlValue(TCP_MTU_PROBING_KEY, "0");
    }
  }

  private verifyState(
    config: FnosNetworkTuningConfig,
    patch: FnosNetworkTuningPatch,
    state: FnosNetworkTuningKernelState,
    targets: RuntimeTransitionTargets = {},
  ): void {
    if (config.bbr_enabled && !state.bbr_active) {
      throw new Error("BBR was requested but the active kernel state is not bbr/fq.");
    }

    if (patch.bbr_enabled === false) {
      const expectedCongestion =
        targets.disabled_bbr_congestion_control ??
        config.previous_tcp_congestion_control;
      const expectedQdisc =
        targets.disabled_bbr_default_qdisc ?? config.previous_default_qdisc;
      if (
        expectedCongestion &&
        expectedCongestion !== state.tcp_congestion_control
      ) {
        throw new Error("BBR rollback did not restore the previous congestion control.");
      }
      if (expectedQdisc && expectedQdisc !== state.default_qdisc) {
        throw new Error("BBR rollback did not restore the previous qdisc.");
      }
      if (!expectedCongestion && state.tcp_congestion_control === "bbr") {
        throw new Error("BBR rollback did not leave bbr congestion control.");
      }
    }

    if (config.mtu_probing_enabled && state.tcp_mtu_probing !== "1") {
      throw new Error("MTU probing was requested but tcp_mtu_probing is not 1.");
    }

    if (patch.mtu_probing_enabled === false) {
      const expectedMtu = targets.disabled_tcp_mtu_probing ?? "0";
      if (state.tcp_mtu_probing !== expectedMtu) {
        throw new Error("MTU probing rollback did not restore the expected value.");
      }
    }
  }

  private async markFailure(
    previousConfig: FnosNetworkTuningConfig,
    beforeState: FnosNetworkTuningKernelState,
    message: string,
  ): Promise<void> {
    try {
      await this.writeManagedConfig(previousConfig);
      await this.restoreRuntime(previousConfig, beforeState);
    } catch (rollbackError) {
      const rollbackMessage =
        rollbackError instanceof Error ? rollbackError.message : String(rollbackError);
      message = `${message}; rollback failed: ${rollbackMessage}`;
    }

    try {
      await this.configAccess.updateConfig({
        ...previousConfig,
        last_error: message,
      });
    } catch {
      // The original sysctl error is more useful to callers.
    }
  }

  async getStatus(): Promise<FnosNetworkTuningStatus> {
    const [config, state] = await Promise.all([
      this.configAccess.getConfig(),
      this.readKernelState(),
    ]);
    return this.buildStatus(config, state);
  }

  async update(
    rawPatch: FnosNetworkTuningPatch,
  ): Promise<FnosNetworkTuningStatus> {
    return this.runExclusive(() => this.updateLocked(rawPatch));
  }

  private async updateLocked(
    rawPatch: FnosNetworkTuningPatch,
  ): Promise<FnosNetworkTuningStatus> {
    this.assertWriteAvailable();

    const patch = this.normalizePatch(rawPatch);
    const previousConfig = await this.configAccess.getConfig();
    const beforeState = await this.readKernelState();
    const nextConfig = this.buildNextConfig(previousConfig, patch, beforeState);

    try {
      const transitionTargets = await this.applyRuntimeTransition(
        previousConfig,
        nextConfig,
        patch,
        beforeState,
      );
      const verifiedState = await this.readKernelState();
      this.verifyState(nextConfig, patch, verifiedState, transitionTargets);
      const savedConfig = await this.configAccess.updateConfig({
        ...nextConfig,
        last_error: null,
      });
      await this.writeManagedConfig(savedConfig);
      return this.buildStatus(savedConfig, verifiedState);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      await this.markFailure(previousConfig, beforeState, message);
      throw error;
    }
  }

  private buildStatus(
    config: FnosNetworkTuningConfig,
    state: FnosNetworkTuningKernelState,
  ): FnosNetworkTuningStatus {
    const blockedReasonCode = this.getBlockedReasonCode();

    return {
      available: blockedReasonCode === null,
      blocked_reason_code: blockedReasonCode,
      blocked_reason: this.getBlockedReason(),
      managed_config_path: this.managedConfigPath,
      config,
      state,
      bbr: {
        desired_enabled: config.bbr_enabled,
        active: state.bbr_active,
        supported: state.bbr_supported,
        module_loaded: state.bbr_module_loaded,
        current_congestion_control: state.tcp_congestion_control,
        current_default_qdisc: state.default_qdisc,
        available_congestion_control: state.tcp_available_congestion_control,
      },
      mtu_probing: {
        desired_enabled: config.mtu_probing_enabled,
        active: state.mtu_probing_active,
        current_value: state.tcp_mtu_probing,
      },
      last_error: config.last_error,
    };
  }
}

export const fnosNetworkTuningService = new FnosNetworkTuningService();
