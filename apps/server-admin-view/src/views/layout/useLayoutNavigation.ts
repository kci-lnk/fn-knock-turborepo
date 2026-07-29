import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import {
  BellRing,
  FileKey2,
  FileSearch,
  Fingerprint,
  Globe2,
  LayoutDashboard,
  Network,
  RadioTower,
  Route as RouteIcon,
  ServerCog,
  Settings2,
  ShieldAlert,
  ShieldBan,
  ShieldCheck,
  SquareTerminal,
  UsersRound,
} from "lucide-vue-next";
import { pendingNavPath } from "@/router/navigation-state";
import {
  isAnySubdomainRoutingMode,
  isReverseProxySubdomainMode,
} from "@/lib/reverse-proxy-submode";
import { useConfigStore } from "@/store/config";
import { useUpdateStore } from "@/store/update";
import { orderSidebarNavItems, type SidebarNavItem } from "./sidebarNavigation";
import { privilegedNavigationVisibility } from "./runtime-navigation";

export const useLayoutNavigation = () => {
  const route = useRoute();
  const configStore = useConfigStore();
  const updateStore = useUpdateStore();
  const { t } = useI18n();

  const isNavActive = (path: string) => {
    const activePath = pendingNavPath.value ?? route.path;
    if (activePath === path) return true;
    if (path === "/") return activePath === "/";
    return activePath.startsWith(`${path}/`);
  };

  const navItems = computed(() => {
    const privilegedNavigation = privilegedNavigationVisibility({
      canUseSshSecurity: configStore.canUseSshSecurity,
      sshSecurityEnabled: configStore.config?.ssh_security?.enabled === true,
      canUseTerminal: configStore.canUseTerminal,
      terminalEnabled: configStore.config?.terminal_feature?.enabled === true,
    });
    const items: SidebarNavItem[] = [
      {
        id: "ip_whitelist",
        name: t("admin.nav.ipWhitelist"),
        path: "/whitelist",
        icon: ShieldCheck,
      },
      {
        id: "ssl_certificate",
        name: t("admin.nav.sslCert"),
        path: "/ssl",
        icon: FileKey2,
      },
    ];
    if (
      configStore.config?.run_type === 1 ||
      configStore.config?.run_type === 3
    ) {
      items.unshift({
        id: "dashboard",
        name: t("admin.nav.dashboard"),
        path: "/",
        icon: LayoutDashboard,
      });
    }
    items.push({
      id: "ddns",
      name: t("admin.nav.ddns"),
      path: "/ddns",
      icon: Network,
    });
    if (configStore.config?.run_type === 1) {
      const isSubdomainMode = isReverseProxySubdomainMode(configStore.config);
      items.splice(1, 0, {
        id: "route_mapping",
        name: isSubdomainMode
          ? t("admin.nav.subdomainMapping")
          : t("admin.nav.pathMapping"),
        path: isSubdomainMode ? "/subdomains" : "/proxy",
        icon: isSubdomainMode ? Globe2 : RouteIcon,
      });
      const showTunnel =
        configStore.canUseFrpc || configStore.canUseCloudflared;
      if (showTunnel) {
        items.splice(2, 0, {
          id: "tunnel",
          name: t("admin.nav.tunnel"),
          path: "/tunnel",
          icon: RadioTower,
        });
      }
      items.splice(showTunnel ? 3 : 2, 0, {
        id: "sessions",
        name: t("admin.nav.sessions"),
        path: "/sessions",
        icon: UsersRound,
      });
    } else if (isAnySubdomainRoutingMode(configStore.config)) {
      const showProtocolMapping =
        configStore.config?.protocol_mapping_feature?.enabled === true ||
        (configStore.config?.stream_mappings?.length ?? 0) > 0;
      items.splice(1, 0, {
        id: "route_mapping",
        name: t("admin.nav.subdomainMapping"),
        path: "/subdomains",
        icon: Globe2,
      });
      if (showProtocolMapping) {
        items.splice(2, 0, {
          id: "protocol_mapping",
          name: t("admin.nav.protocolMapping"),
          path: "/streams",
          icon: ServerCog,
        });
      }
      items.splice(showProtocolMapping ? 3 : 2, 0, {
        id: "sessions",
        name: t("admin.nav.sessions"),
        path: "/sessions",
        icon: UsersRound,
      });
    }
    items.push({
      id: "auth",
      name: t("admin.nav.authConfig"),
      path: "/auth",
      icon: Fingerprint,
    });
    if (privilegedNavigation.sshSecurity) {
      items.push({
        id: "ssh_security",
        name: t("admin.nav.sshSecurity"),
        path: "/ssh-security",
        icon: ShieldBan,
      });
    }
    items.push({
      id: "events",
      name: t("admin.nav.events"),
      path: "/events",
      icon: BellRing,
    });
    if (configStore.config?.gateway_logging?.enabled) {
      items.push({
        id: "gateway_request_logs",
        name: t("admin.nav.requestLogs"),
        path: "/request-logs",
        icon: FileSearch,
      });
    }
    if (configStore.config?.waf?.enabled) {
      items.push({
        id: "waf_logs",
        name: t("admin.nav.wafLogs"),
        path: "/waf-logs",
        icon: ShieldAlert,
      });
    }
    if (privilegedNavigation.terminal) {
      items.push({
        id: "web_terminal",
        name: t("admin.nav.webTerminal"),
        path: "/terminal",
        icon: SquareTerminal,
      });
    }
    items.push({
      id: "system_settings",
      name: t("admin.nav.systemSettings"),
      path: "/system",
      icon: Settings2,
    });
    return orderSidebarNavItems(
      items,
      configStore.config?.dashboard_display?.sidebar_menu_order,
    );
  });

  const currentNavLabel = computed(() => {
    const activeItem = navItems.value.find((item) => isNavActive(item.path));
    return activeItem?.name ?? t("common.managementConsole");
  });

  const currentVersionLabel = computed(() => {
    const version = updateStore.status?.localVersion?.trim();
    return version ? `v${version}` : "";
  });

  const aboutEntryLabel = computed(() => t("admin.nav.systemUpdate"));

  return {
    aboutEntryLabel,
    currentNavLabel,
    currentVersionLabel,
    isNavActive,
    navItems,
  };
};
