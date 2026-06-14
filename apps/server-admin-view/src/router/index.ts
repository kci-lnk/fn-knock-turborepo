import NProgress from "nprogress";
import { createRouter, createWebHashHistory } from "vue-router";
import Layout from "../views/Layout.vue";
import { useConfigStore } from "../store/config";
import { useDockerAdminAuthStore } from "../store/dockerAdminAuth";
import { pinia } from "../store";
import { isRouteNavigating, pendingNavPath } from "./navigation-state";
import { toast } from "@admin-shared/utils/toast";
import { browserT } from "@fn-knock/i18n/vue";
import {
  isAnySubdomainRoutingMode,
  isReverseProxySubdomainMode,
} from "../lib/reverse-proxy-submode";

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
          path: "auth/oidc-providers",
          name: "OIDCProviderSettings",
          component: () => import("../views/OIDCProviderSettings.vue"),
        },
        {
          path: "events",
          name: "EventCenter",
          component: () => import("../views/EventCenter.vue"),
        },
        {
          path: "ssh-security",
          name: "SSHSecurity",
          component: () => import("../views/SSHSecurity.vue"),
        },
        {
          path: "logs",
          redirect: "/events",
        },
        {
          path: "request-logs",
          name: "GatewayRequestLogs",
          component: () => import("../views/GatewayRequestLogs.vue"),
        },
        {
          path: "waf-logs",
          name: "WAFLogs",
          component: () => import("../views/WAFLogs.vue"),
        },
        {
          path: "system",
          name: "SystemSettings",
          component: () => import("../views/SystemSettings.vue"),
        },
        {
          path: "system/gateway-visibility",
          name: "GatewayVisibilitySettings",
          component: () =>
            import("../views/system-settings/GatewayVisibilitySettings.vue"),
        },
        {
          path: "system/gateway-proxy-headers",
          name: "GatewayProxyHeadersSettings",
          component: () =>
            import("../views/system-settings/GatewayProxyHeadersSettings.vue"),
        },
        {
          path: "system/gateway-host-response",
          name: "GatewayHostResponseSettings",
          component: () =>
            import("../views/system-settings/GatewayHostResponseSettings.vue"),
        },
        {
          path: "system/gateway-locations",
          name: "GatewayLocationsSettings",
          component: () =>
            import("../views/system-settings/GatewayLocationsSettings.vue"),
        },
        {
          path: "system/smart-connect",
          name: "SmartConnectSettings",
          component: () =>
            import("../views/system-settings/SmartConnectSettings.vue"),
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
          path: "tunnel/frp/instances/new",
          name: "FrpcInstanceCreate",
          component: () => import("../views/tunnel/frp/FrpcInstancePage.vue"),
        },
        {
          path: "tunnel/frp/instances/:id",
          name: "FrpcInstanceDetail",
          component: () => import("../views/tunnel/frp/FrpcInstancePage.vue"),
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

  const dockerAdminAuthStore = useDockerAdminAuthStore(pinia);
  if (!dockerAdminAuthStore.isBootstrapped) {
    try {
      await dockerAdminAuthStore.bootstrap();
    } catch (error) {
      console.error("Failed to bootstrap docker admin auth in router", error);
      return true;
    }
  }

  if (dockerAdminAuthStore.isEnabled && !dockerAdminAuthStore.isAuthenticated) {
    return true;
  }

  if (
    to.path !== "/" &&
    to.path !== "/dashboard" &&
    to.path !== "/streams" &&
    to.path !== "/proxy" &&
    to.path !== "/subdomains" &&
    to.path !== "/terminal" &&
    to.path !== "/ssh-security" &&
    to.path !== "/tunnel" &&
    !to.path.startsWith("/tunnel/") &&
    to.path !== "/system/smart-connect"
  ) {
    return true;
  }

  const configStore = useConfigStore(pinia);
  if (!configStore.config) {
    await configStore.loadConfig();
  }

  if (configStore.config?.run_type === 0) {
    if (to.path === "/terminal") {
      return true;
    }
    return "/whitelist";
  }

  const isSubdomainRoutingMode = isAnySubdomainRoutingMode(configStore.config);

  if (to.path === "/proxy" && configStore.config?.run_type === 1) {
    if (isReverseProxySubdomainMode(configStore.config)) {
      return "/subdomains";
    }
    return true;
  }

  if (to.path === "/proxy") {
    return isSubdomainRoutingMode ? "/subdomains" : "/whitelist";
  }

  if (to.path === "/subdomains" && !isSubdomainRoutingMode) {
    return configStore.config?.run_type === 1 ? "/proxy" : "/whitelist";
  }

  if (
    (to.path === "/tunnel" || to.path.startsWith("/tunnel/")) &&
    configStore.config?.run_type !== 1
  ) {
    return "/system";
  }

  if (to.path === "/terminal" && !configStore.canUseTerminal) {
    return "/system";
  }

  if (
    to.path === "/ssh-security" &&
    (configStore.isDockerDeployment ||
      configStore.config?.ssh_security?.enabled !== true)
  ) {
    return {
      path: "/system",
      query: {
        tab: "features",
      },
    };
  }

  if (to.path === "/system/smart-connect" && !configStore.canUseSmartConnect) {
    return {
      path: "/system",
      query: {
        tab: "features",
      },
    };
  }

  const isProtocolMappingVisible =
    configStore.config?.run_type === 3 &&
    configStore.config?.protocol_mapping_feature?.enabled === true;

  if (to.path === "/streams" && !isProtocolMappingVisible) {
    if (configStore.config?.run_type === 1) {
      return isReverseProxySubdomainMode(configStore.config)
        ? "/subdomains"
        : "/proxy";
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
    toast.info(browserT("admin.route.devCacheExpired"));
    window.setTimeout(() => {
      window.location.reload();
    }, 120);
  }
});

export default router;
