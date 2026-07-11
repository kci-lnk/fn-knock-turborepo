import { invoke } from "@tauri-apps/api/core";
import "./style.css";

type ListenerScope = "loopback" | "all";

interface RuntimeConfig {
  schema_version: number;
  onboarding_complete: boolean;
  admin_port: number;
  backend_port: number;
  auth_port: number;
  grpc_port: number;
  proxy_port: number;
  listener_scope: ListenerScope;
}

interface PortStatus {
  name: string;
  port: number;
  available: boolean;
}

interface DesktopStatus {
  version: string;
  service_state: string;
  ready: boolean;
  ready_detail: string | null;
  failure: string | null;
  data_dir: string;
  install_dir: string;
  firewall_rule_enabled: boolean;
  config: RuntimeConfig;
  ports: PortStatus[];
}

interface UpdateMetadata {
  version: string;
  notes: string | null;
}

const app = document.querySelector<HTMLElement>("#app");

if (!app) {
  throw new Error("missing app root");
}

app.innerHTML = `
  <section class="shell">
    <header class="hero">
      <div class="brand-mark" aria-hidden="true">F</div>
      <div>
        <p class="eyebrow">FNKNOCK FOR WINDOWS</p>
        <h1>网关运行状态</h1>
        <p class="subtitle">关闭此窗口不会停止代理服务。</p>
      </div>
      <span id="state-badge" class="badge badge-waiting">检查中</span>
    </header>

    <div id="notice" class="notice" hidden></div>

    <section id="onboarding" class="card onboarding-card" hidden>
      <p class="eyebrow">首次设置 · 第 1 步（共 2 步）</p>
      <h2>确认网关端口与访问范围</h2>
      <p>请先确认代理端口、监听范围和防火墙状态；保存并重启后，将进入管理密码设置。</p>
    </section>

    <section class="card status-grid" aria-live="polite">
      <div>
        <span class="label">Windows 服务</span>
        <strong id="service-state">检查中…</strong>
      </div>
      <div>
        <span class="label">完整就绪</span>
        <strong id="ready-state">检查中…</strong>
      </div>
      <div>
        <span class="label">防火墙规则</span>
        <strong id="firewall-state">检查中…</strong>
      </div>
      <div>
        <span class="label">版本</span>
        <strong id="version-state">—</strong>
      </div>
    </section>

    <section class="card">
      <div class="section-heading">
        <div>
          <h2>运行配置</h2>
          <p>发生端口冲突时可在此修复；保存需要 UAC 授权。</p>
        </div>
        <button id="save-config" class="button button-secondary" type="button">保存并重启</button>
      </div>
      <form id="runtime-form" class="form-grid">
        <label>管理界面<input name="admin_port" type="number" min="1024" max="65535" required /></label>
        <label>Rust API<input name="backend_port" type="number" min="1024" max="65535" required /></label>
        <label>认证服务<input name="auth_port" type="number" min="1024" max="65535" required /></label>
        <label>Go gRPC<input name="grpc_port" type="number" min="1024" max="65535" required /></label>
        <label>代理端口<input name="proxy_port" type="number" min="1" max="65535" required /></label>
        <label>
          代理监听范围
          <select name="listener_scope">
            <option value="loopback">仅本机（推荐）</option>
            <option value="all">局域网</option>
          </select>
        </label>
      </form>
      <div id="port-list" class="port-list"></div>
    </section>

    <footer class="actions">
      <button id="open-admin" class="button button-primary" type="button" disabled>打开管理台</button>
      <button id="start-service" class="button button-secondary" type="button">启动服务</button>
      <button id="restart-service" class="button button-secondary" type="button">重启服务</button>
      <button id="check-update" class="button button-quiet" type="button">检查更新</button>
      <button id="export-diagnostics" class="button button-quiet" type="button">导出诊断</button>
    </footer>

    <p id="paths" class="paths"></p>
  </section>
`;

const byId = <T extends HTMLElement>(id: string) => {
  const element = document.querySelector<T>(`#${id}`);
  if (!element) throw new Error(`missing element #${id}`);
  return element;
};

const form = byId<HTMLFormElement>("runtime-form");
const notice = byId<HTMLDivElement>("notice");
const stateBadge = byId<HTMLSpanElement>("state-badge");
const openAdminButton = byId<HTMLButtonElement>("open-admin");
let latestStatus: DesktopStatus | null = null;
let formDirty = false;
let openedAutomatically = false;
let statusRefreshPromise: Promise<void> | null = null;

function showNotice(message: string, kind: "info" | "error" = "info") {
  notice.textContent = message;
  notice.className = `notice notice-${kind}`;
  notice.hidden = false;
}

function clearNotice() {
  notice.hidden = true;
}

function input(name: keyof RuntimeConfig) {
  const element = form.elements.namedItem(name);
  if (
    !(
      element instanceof HTMLInputElement ||
      element instanceof HTMLSelectElement
    )
  ) {
    throw new Error(`missing form field ${name}`);
  }
  return element;
}

function writeConfig(config: RuntimeConfig) {
  if (formDirty) return;
  input("admin_port").value = String(config.admin_port);
  input("backend_port").value = String(config.backend_port);
  input("auth_port").value = String(config.auth_port);
  input("grpc_port").value = String(config.grpc_port);
  input("proxy_port").value = String(config.proxy_port);
  input("listener_scope").value = config.listener_scope;
  formDirty = false;
}

function readConfig(): RuntimeConfig {
  const config: RuntimeConfig = {
    schema_version: 1,
    onboarding_complete: latestStatus?.config.onboarding_complete ?? false,
    admin_port: Number(input("admin_port").value),
    backend_port: Number(input("backend_port").value),
    auth_port: Number(input("auth_port").value),
    grpc_port: Number(input("grpc_port").value),
    proxy_port: Number(input("proxy_port").value),
    listener_scope: input("listener_scope").value as ListenerScope,
  };

  const ports = [
    config.admin_port,
    config.backend_port,
    config.auth_port,
    config.grpc_port,
    config.proxy_port,
  ];
  if (new Set(ports).size !== ports.length) {
    throw new Error("五个端口必须互不相同");
  }
  if (
    ports.some((port) => !Number.isInteger(port) || port < 1 || port > 65535)
  ) {
    throw new Error("端口必须是 1–65535 之间的整数");
  }
  if (
    [
      config.admin_port,
      config.backend_port,
      config.auth_port,
      config.grpc_port,
    ].some((port) => port < 1024)
  ) {
    throw new Error("内部端口必须大于或等于 1024");
  }
  return config;
}

function renderStatus(status: DesktopStatus) {
  latestStatus = status;
  writeConfig(status.config);
  byId("service-state").textContent = status.service_state;
  byId("ready-state").textContent = status.ready
    ? "Rust、Go 与认证桥均正常"
    : status.ready_detail || "尚未就绪";
  byId("firewall-state").textContent = status.firewall_rule_enabled
    ? "Domain / Private 已启用"
    : "未启用";
  byId("version-state").textContent = status.version;
  byId("paths").textContent =
    `程序：${status.install_dir} · 数据：${status.data_dir}`;

  stateBadge.textContent = status.ready ? "运行正常" : status.service_state;
  stateBadge.className = status.ready
    ? "badge badge-ready"
    : status.failure
      ? "badge badge-error"
      : "badge badge-waiting";
  const onboarding = byId<HTMLElement>("onboarding");
  onboarding.hidden = status.config.onboarding_complete;
  byId<HTMLButtonElement>("save-config").textContent = status.config
    .onboarding_complete
    ? "保存并重启"
    : "保存并继续";
  openAdminButton.disabled =
    !status.ready || !status.config.onboarding_complete;

  const portList = byId<HTMLDivElement>("port-list");
  portList.innerHTML = status.ports
    .map(
      (port) => `
        <span class="port ${port.available ? "port-free" : "port-used"}">
          ${port.name} : ${port.port} · ${port.available ? "空闲" : status.ready ? "服务占用" : "冲突"}
        </span>
      `,
    )
    .join("");

  if (status.failure) {
    showNotice(status.failure, "error");
  }
}

async function refreshStatusOnce() {
  try {
    const status = await invoke<DesktopStatus>("get_status");
    renderStatus(status);
    if (
      status.ready &&
      status.config.onboarding_complete &&
      !openedAutomatically
    ) {
      openedAutomatically = true;
      await invoke("open_admin");
    }
  } catch (error) {
    stateBadge.textContent = "状态不可用";
    stateBadge.className = "badge badge-error";
    showNotice(String(error), "error");
  }
}

function refreshStatus(): Promise<void> {
  if (!statusRefreshPromise) {
    statusRefreshPromise = refreshStatusOnce().finally(() => {
      statusRefreshPromise = null;
    });
  }
  return statusRefreshPromise;
}

async function runAction(
  button: HTMLButtonElement,
  action: () => Promise<void>,
) {
  const label = button.textContent;
  button.disabled = true;
  button.textContent = "处理中…";
  clearNotice();
  try {
    await action();
  } catch (error) {
    showNotice(String(error), "error");
  } finally {
    button.disabled = false;
    button.textContent = label;
    await refreshStatus();
  }
}

byId<HTMLButtonElement>("open-admin").addEventListener("click", () => {
  void invoke("open_admin").catch((error) =>
    showNotice(String(error), "error"),
  );
});

byId<HTMLButtonElement>("start-service").addEventListener("click", (event) => {
  void runAction(event.currentTarget as HTMLButtonElement, async () => {
    await invoke("start_service");
    showNotice("已请求启动 FnKnock 服务。");
  });
});

byId<HTMLButtonElement>("restart-service").addEventListener(
  "click",
  (event) => {
    void runAction(event.currentTarget as HTMLButtonElement, async () => {
      await invoke("restart_service");
      showNotice("已请求重启 FnKnock 服务。");
    });
  },
);

byId<HTMLButtonElement>("save-config").addEventListener("click", (event) => {
  void runAction(event.currentTarget as HTMLButtonElement, async () => {
    const config = readConfig();
    config.onboarding_complete = true;
    await invoke("save_runtime_config", { config });
    formDirty = false;
    showNotice("运行配置已保存，服务正在重启。");
  });
});

byId<HTMLButtonElement>("check-update").addEventListener("click", (event) => {
  void runAction(event.currentTarget as HTMLButtonElement, async () => {
    const update = await invoke<UpdateMetadata | null>("check_for_update");
    if (!update) {
      showNotice("当前已是最新稳定版本。");
      return;
    }
    const confirmed = window.confirm(
      `发现 ${update.version}。安装时会关闭 GUI、停止服务并创建数据快照，是否继续？`,
    );
    if (!confirmed) return;
    await invoke("install_update");
  });
});

byId<HTMLButtonElement>("export-diagnostics").addEventListener(
  "click",
  (event) => {
    void runAction(event.currentTarget as HTMLButtonElement, async () => {
      const path = await invoke<string>("export_diagnostics");
      showNotice(`诊断文件已导出：${path}`);
    });
  },
);

form.addEventListener("input", () => {
  formDirty = true;
});

void refreshStatus();
window.setInterval(() => void refreshStatus(), 2_000);
