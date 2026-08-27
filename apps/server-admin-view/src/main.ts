import { createApp } from "vue";
import "./assets/index.css";
import "nprogress/nprogress.css";
import App from "./App.vue";
import router from "./router";
import { pinia } from "./store";
import { createFnKnockI18n } from "@fn-knock/i18n/vue/admin";
import { applyStoredThemeMode } from "@/components/ui/theme-toggle";
import { applyAppearanceConfig } from "@admin-shared/composables/useAppearanceState";
import {
  buildCacheBustedApplicationUrl,
  claimChunkReload,
  isDynamicImportFailure,
} from "./lib/update-reload";

applyStoredThemeMode();
applyAppearanceConfig();

const bootstrap = async () => {
  const app = createApp(App);
  const i18n = await createFnKnockI18n({ scope: "admin" });
  app.use(pinia);
  app.use(router);
  app.use(i18n);

  app.mount("#app");
};

const renderBootstrapFailure = (error: unknown) => {
  const root = document.getElementById("app");
  if (!root) return;

  const panel = document.createElement("main");
  panel.setAttribute("role", "alert");
  panel.style.cssText =
    "min-height:100dvh;display:grid;place-items:center;padding:24px;background:#0a0a0a;color:#fafafa;font-family:system-ui,sans-serif";

  const content = document.createElement("section");
  content.style.cssText =
    "width:min(100%,440px);padding:24px;border:1px solid #404040;border-radius:14px;background:#171717";

  const title = document.createElement("h1");
  title.textContent = "页面资源加载失败";
  title.style.cssText = "margin:0 0 10px;font-size:20px";

  const message = document.createElement("p");
  message.textContent =
    "缓存中的页面资源可能已经过期。请重新加载；如果仍然失败，请清理此站点的数据。";
  message.style.cssText =
    "margin:0 0 18px;color:#d4d4d4;line-height:1.6;font-size:14px";

  const retry = document.createElement("button");
  retry.type = "button";
  retry.textContent = "重新加载";
  retry.style.cssText =
    "min-height:44px;padding:0 18px;border:0;border-radius:9px;background:#fafafa;color:#171717;font-weight:600;cursor:pointer";
  retry.addEventListener("click", () => {
    window.location.replace(
      buildCacheBustedApplicationUrl(window.location.href, Date.now(), "chunk"),
    );
  });

  content.append(title, message, retry);
  panel.append(content);
  root.replaceChildren(panel);
  console.error("Failed to bootstrap admin application", error);
};

const recoverBootstrap = (error: unknown) => {
  let reloadStorage: Storage | null = null;
  try {
    reloadStorage = window.sessionStorage;
  } catch {
    // The cache-busting query parameter still prevents a reload loop when
    // session storage is unavailable.
  }

  if (
    isDynamicImportFailure(error) &&
    claimChunkReload(window.location.href, reloadStorage)
  ) {
    window.location.replace(
      buildCacheBustedApplicationUrl(window.location.href, Date.now(), "chunk"),
    );
    return;
  }

  renderBootstrapFailure(error);
};

void bootstrap().catch(recoverBootstrap);
