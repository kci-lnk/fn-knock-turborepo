import NProgress from "nprogress";
import { createRouter, createWebHashHistory } from "vue-router";
import Layout from "../views/Layout.vue";
import { useConfigStore } from "../store/config";
import { pinia } from "../store";
import { isRouteNavigating, pendingNavPath } from "./navigation-state";
import { toast } from "@admin-shared/utils/toast";

NProgress.configure({
  showSpinner: false,
  minimum: 0.12,
  easing: "ease",
  speed: 420,
  trickleSpeed: 160,
});

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    {
      path: "/",
      component: Layout,
      children: [
        {
          path: "",
          name: "Dashboard",
          component: () => import("../views/Dashboard.vue"),
        },
        {
          path: "dashboard",
          redirect: "/",
        },
        {
          path: "whitelist",
          name: "Whitelist",
          component: () => import("../views/IPWhitelist.vue"),
        },
        {
          path: "proxy",
          name: "ReverseProxy",
          component: () => import("../views/ReverseProxy.vue"),
        },
        {
          path: "subdomains",
          name: "SubdomainProxy",
          component: () => import("../views/SubdomainProxy.vue"),
        },
        {
          path: "streams",
          name: "StreamMappings",
          component: () => import("../views/StreamMappings.vue"),
        },
        {
          path: "ssl",
          name: "SSLSettings",
          component: () => import("../views/SSLSettings.vue"),
        },
        {
          path: "mode",
          name: "RunMode",
          component: () =>
            import("../views/system-settings/RunModeSettings.vue"),
        },
        {
          path: "auth",
          name: "AuthSettings",
          component: () => import("../views/AuthSettings.vue"),
        },
        {
          path: "auth/passkeys/:totpId",
          name: "PasskeySettings",
          component: () => import("../views/PasskeySettings.vue"),
        },
        {
          path: "logs",
          name: "AuthLogs",
          component: () => import("../views/AuthLogs.vue"),
        },
        {
          path: "request-logs",
          name: "GatewayRequestLogs",
          component: () => import("../views/GatewayRequestLogs.vue"),
        },
        {
          path: "system",
          name: "SystemSettings",
          component: () => import("../views/SystemSettings.vue"),
        },
        {
          path: "sessions",
          name: "SessionManagement",
          component: () => import("../views/SessionManagement.vue"),
        },
        {
          path: "sessions/mobility/:id",
          name: "SessionMobility",
          component: () =>
            import("../views/session-management/mobility/SessionMobilityPage.vue"),
        },
        {
          path: "terminal",
          name: "WebTerminal",
          component: () => import("../views/WebTerminal.vue"),
        },
        {
          path: "tunnel",
          name: "IntranetTunnel",
          component: () => import("../views/Tunnel.vue"),
        },
        {
          path: "ddns",
          name: "DDNSManagement",
          component: () => import("../views/DDNSManagement.vue"),
        },
        {
          path: "about",
          name: "About",
          component: () => import("../views/AboutUpdate.vue"),
        },
      ],
    },
  ],
});

router.beforeEach(async (to, from) => {
  const isPageNavigation = to.fullPath !== from.fullPath;

  if (isPageNavigation) {
    isRouteNavigating.value = true;
    NProgress.start();
  }

  if (to.path !== "/" && to.path !== "/dashboard" && to.path !== "/streams") {
    return true;
  }

  const configStore = useConfigStore(pinia);
  if (!configStore.config) {
    await configStore.loadConfig();
  }

  if (configStore.config?.run_type === 0) {
    if (to.path === "/streams") {
      return "/whitelist";
    }
    return "/whitelist";
  }

  const isProtocolMappingVisible =
    configStore.config?.run_type === 3 &&
    configStore.config?.protocol_mapping_feature?.enabled === true;

  if (to.path === "/streams" && !isProtocolMappingVisible) {
    if (configStore.config?.run_type === 1) {
      return "/proxy";
    }
    if (configStore.config?.run_type === 3) {
      return {
        path: "/system",
        query: {
          tab: "features",
        },
      };
    }
    return "/";
  }

  return true;
});

router.afterEach(() => {
  isRouteNavigating.value = false;
  pendingNavPath.value = null;
  NProgress.done();
});

router.onError((error) => {
  isRouteNavigating.value = false;
  pendingNavPath.value = null;
  NProgress.done();

  const message = error instanceof Error ? error.message : "";
  const isDynamicImportFailure =
    message.includes("Failed to fetch dynamically imported module") ||
    message.includes("Importing a module script failed");

  if (import.meta.env.DEV && isDynamicImportFailure) {
    toast.info("检测到开发环境依赖缓存已过期，正在刷新页面...");
    window.setTimeout(() => {
      window.location.reload();
    }, 120);
  }
});

export default router;
