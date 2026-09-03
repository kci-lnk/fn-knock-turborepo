import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import {
  BellRing,
  ChartNoAxesCombined,
  FileKey2,
  Fingerprint,
  Globe2,
  LayoutDashboard,
  Network,
  RadioTower,
  MonitorUp,
  Route as RouteIcon,
  Settings2,
  ShieldBan,
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
    if (path === "/mappings") {
      return (
        activePath === path ||
        activePath.startsWith("/subdomains/") ||
        activePath.startsWith("/streams/")
      );
    }
    if (activePath === path) return true;
    if (path === "/") return activePath === "/";
    return activePath.startsWith(`${path}/`);
  };

  const navItems = computed(() => {
    const privilegedNavigation = privilegedNavigationVisibility({
      canUseSshSecurity: configStore.canUseSshSecurity,
      sshSecurityEnabled: configStore.config?.ssh_security?.enabled === true,
    });
    const items: SidebarNavItem[] = [
      {
        id: "sessions",
        name: t("admin.nav.sessions"),
        path: "/sessions",
        icon: UsersRound,
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
    if (configStore.config?.wol_feature?.enabled === true) {
      items.push({
        id: "wol",
        name: t("admin.nav.wol"),
        path: "/wol",
        icon: MonitorUp,
      });
    }
    if (configStore.config?.run_type === 1) {
      const isSubdomainMode = isReverseProxySubdomainMode(configStore.config);
      items.splice(1, 0, {
        id: "route_mapping",
        name: isSubdomainMode
          ? t("admin.nav.mappingManagement")
          : t("admin.nav.pathMapping"),
        path: isSubdomainMode ? "/mappings" : "/proxy",
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
    } else if (isAnySubdomainRoutingMode(configStore.config)) {
      items.splice(1, 0, {
        id: "route_mapping",
        name: t("admin.nav.mappingManagement"),
        path: "/mappings",
        icon: Globe2,
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
    items.push({
      id: "gateway_request_logs",
      name: t("admin.nav.requestLogs"),
      path: "/request-analysis",
      icon: ChartNoAxesCombined,
    });
    items.push({
      id: "web_terminal",
      name: t("admin.nav.webTerminal"),
      path: "/terminal",
      icon: SquareTerminal,
    });
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
