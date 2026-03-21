<template>
  <div class="flex min-h-dvh w-full flex-col bg-muted/40">
    <div
      v-if="configStore.isLoading"
      class="ml-4 text-sm text-muted-foreground animate-pulse"
    >
      加载配置中...
    </div>
    <div v-if="configStore.isError" class="ml-4 text-sm text-destructive">
      加载配置失败
    </div>

    <div
      class="sticky top-0 z-20 border-b bg-background/95 backdrop-blur sm:hidden"
    >
      <div class="mx-auto flex h-14 max-w-7xl items-center gap-2 px-4">
        <Button variant="ghost" size="icon" @click="isMobileNavOpen = true">
          <Menu class="h-5 w-5" />
          <span class="sr-only">打开导航菜单</span>
        </Button>
        <p class="truncate text-sm font-medium">{{ currentNavLabel }}</p>
      </div>
    </div>

    <Sheet v-model:open="isMobileNavOpen">
      <SheetContent side="left" class="w-[66vw] max-w-[240px] p-0">
        <SheetHeader class="sr-only">
          <SheetTitle>导航菜单</SheetTitle>
        </SheetHeader>
        <div class="flex h-full flex-col">
          <div class="border-b px-4 py-3 text-sm font-semibold">导航菜单</div>
          <nav class="flex-1 space-y-2 overflow-y-auto p-3">
            <Button
              v-for="item in navItems"
              :key="item.path"
              :variant="isNavActive(item.path) ? 'default' : 'ghost'"
              class="w-full justify-start gap-3"
              @click="navigateTo(item.path)"
            >
              <component :is="item.icon" class="h-4 w-4" />
              <span>{{ item.name }}</span>
            </Button>
          </nav>
          <div class="border-t p-3">
            <p
              class="mb-2 text-center text-[11px] font-medium tracking-[0.12em] text-primary/70"
            >
              <a
                :href="APP_GITHUB_URL"
                target="_blank"
                rel="noopener noreferrer"
                class="inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 transition-colors hover:text-foreground hover:bg-background/70"
                title="打开 GitHub 项目页"
              >
                <Github class="h-3.5 w-3.5" />
                <span>{{ currentVersionLabel }}</span>
              </a>
            </p>
            <div class="flex justify-center pb-10">
              <Button
                variant="secondary"
                class="w-auto min-w-28 justify-center px-5"
                @click="navigateTo('/about')"
              >
                系统更新
              </Button>
            </div>
          </div>
        </div>
      </SheetContent>
    </Sheet>

    <div
      class="mx-auto flex w-full max-w-7xl min-w-0 flex-1 min-h-0 flex-col gap-4 px-4 py-4 sm:flex-row sm:gap-6 sm:px-6 sm:py-6"
    >
      <aside
        class="hidden w-[136px] shrink-0 sm:sticky sm:top-6 sm:block sm:h-[calc(100dvh-3rem)]"
      >
        <div class="flex h-full min-h-0 flex-col gap-3">
          <nav
            class="flex min-h-0 flex-1 flex-col items-center gap-2 overflow-y-auto"
          >
            <Button
              v-for="item in navItems"
              :key="item.path"
              :variant="isNavActive(item.path) ? 'default' : 'ghost'"
              :class="[
                'w-[92%] justify-start gap-1.5 px-2 transition-[transform,box-shadow,background-color,color] duration-150',
                isNavActive(item.path)
                  ? 'shadow-sm shadow-primary/15'
                  : 'hover:-translate-y-[1px]',
              ]"
              @click="navigateTo(item.path)"
            >
              <component :is="item.icon" class="h-4 w-4" />
              <span>{{ item.name }}</span>
            </Button>
          </nav>
          <div>
            <p
              class="mb-2 text-center text-[11px] font-medium tracking-[0.12em] text-primary/70"
            >
              <a
                :href="APP_GITHUB_URL"
                target="_blank"
                rel="noopener noreferrer"
                class="inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 transition-colors hover:text-foreground hover:bg-background/70"
                title="打开 GitHub 项目页"
              >
                <Github class="h-3.5 w-3.5" />
                <span>{{ currentVersionLabel }}</span>
              </a>
            </p>
            <div class="flex justify-center">
              <Button
                variant="secondary"
                class="h-8 w-auto min-w-0 justify-center px-2.5"
                @click="navigateTo('/about')"
              >
                系统更新
              </Button>
            </div>
          </div>
        </div>
      </aside>

      <main
        class="flex-1 w-full min-w-0 min-h-0 overflow-y-auto"
        :aria-busy="isRouteNavigating"
      >
        <div
          v-if="updateStore.shouldShowBanner && updateStore.status"
          :class="[
            'mx-auto mt-3 mb-6 w-full max-w-7xl rounded-lg border px-4 py-3',
            updateStore.isForceUpdate
              ? 'border-red-300 bg-red-50 text-red-800'
              : 'border-amber-300 bg-amber-50 text-amber-900',
          ]"
        >
          <div
            class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
          >
            <div class="space-y-1">
              <p class="text-sm font-semibold">
                检测到新版本 {{ updateStore.status.latest?.version }}（当前
                {{ updateStore.status.localVersion }}）
              </p>
              <p class="text-xs">
                {{
                  updateStore.isForceUpdate
                    ? "重要更新，请尽快安装。"
                    : "可前往关于页查看详情并更新。"
                }}
              </p>
            </div>
            <div class="flex items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                class="bg-white/80"
                @click="goToAbout"
              >
                查看详情
              </Button>
              <Button
                size="sm"
                :variant="updateStore.isForceUpdate ? 'destructive' : 'default'"
                @click="startUpdateFromBanner"
              >
                立即更新
              </Button>
            </div>
          </div>
        </div>
        <div
          v-if="isRouteNavigating"
          class="mx-auto mb-4 flex w-full max-w-7xl justify-end"
        >
          <div
            class="inline-flex items-center gap-2 rounded-full border border-border/70 bg-background/88 px-3 py-1.5 text-xs text-muted-foreground shadow-sm backdrop-blur"
          >
            <span
              class="h-1.5 w-1.5 rounded-full bg-primary animate-pulse"
            ></span>
            <span>页面切换中...</span>
          </div>
        </div>
        <RouterView v-if="!configStore.isLoading && !configStore.isError" />
        <div
          v-else-if="configStore.isLoading"
          class="flex h-full min-h-[400px] items-center justify-center"
        >
          <div
            class="h-8 w-8 animate-spin rounded-full border-b-2 border-primary"
          ></div>
        </div>
      </main>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { isNavigationFailure, useRoute, useRouter } from "vue-router";
import { useConfigStore } from "../store/config";
import { useUpdateStore } from "../store/update";
import { isRouteNavigating, pendingNavPath } from "../router/navigation-state";
import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
const APP_GITHUB_URL = "https://github.com/kci-lnk/fn-knock-turborepo";
import {
  LayoutDashboard,
  ShieldCheck,
  Lock,
  Route as RouteIcon,
  Cable,
  Key,
  Github,
  FileText,
  ScrollText,
  Settings,
  Terminal,
  Users,
  Globe,
  Menu,
} from "lucide-vue-next";

const router = useRouter();
const route = useRoute();
const configStore = useConfigStore();
const updateStore = useUpdateStore();
const isMobileNavOpen = ref(false);

onMounted(() => {
  void configStore.loadConfig();
  void updateStore.initialize();
});

onUnmounted(() => {
  updateStore.stopPolling();
});

const navigateTo = async (path: string) => {
  isMobileNavOpen.value = false;
  if (route.path === path) return;
  pendingNavPath.value = path;
  const failure = await router.push(path);
  if (isNavigationFailure(failure)) {
    pendingNavPath.value = null;
  }
};

const goToAbout = () => {
  void navigateTo("/about");
};

watch(
  () => route.path,
  () => {
    isMobileNavOpen.value = false;
  },
);

const startUpdateFromBanner = async () => {
  await navigateTo("/about");
  await updateStore.checkAndDownload();
};

const isNavActive = (path: string) => {
  const activePath = pendingNavPath.value ?? route.path;
  if (activePath === path) return true;
  if (path === "/") return activePath === "/";
  return activePath.startsWith(`${path}/`);
};

const navItems = computed(() => {
  const items = [
    { name: "IP白名单", path: "/whitelist", icon: ShieldCheck },
    { name: "SSL证书", path: "/ssl", icon: Lock },
  ];
  if (
    configStore.config?.run_type === 1 ||
    configStore.config?.run_type === 3
  ) {
    items.unshift({ name: "控制台", path: "/", icon: LayoutDashboard });
  }
  items.push({ name: "动态域名", path: "/ddns", icon: Globe });
  if (configStore.config?.run_type === 1) {
    items.splice(1, 0, { name: "映射管理", path: "/proxy", icon: RouteIcon });
    items.splice(2, 0, { name: "会话管理", path: "/sessions", icon: Users });
    items.splice(2, 0, { name: "内网穿透", path: "/tunnel", icon: Cable });
  } else if (configStore.config?.run_type === 3) {
    items.splice(1, 0, {
      name: "子域映射",
      path: "/subdomains",
      icon: RouteIcon,
    });
    items.splice(2, 0, { name: "会话管理", path: "/sessions", icon: Users });
  }
  items.push({ name: "认证配置", path: "/auth", icon: Key });
  items.push({ name: "登录日志", path: "/logs", icon: FileText });
  if (configStore.config?.gateway_logging?.enabled) {
    items.push({ name: "请求日志", path: "/request-logs", icon: ScrollText });
  }
  if (configStore.config?.terminal_feature?.enabled) {
    items.push({ name: "Web终端", path: "/terminal", icon: Terminal });
  }
  items.push({ name: "系统设置", path: "/system", icon: Settings });
  return items;
});

const currentNavLabel = computed(() => {
  const activeItem = navItems.value.find((item) => isNavActive(item.path));
  return activeItem?.name ?? "管理后台";
});

const currentVersionLabel = computed(() => {
  const version = updateStore.status?.localVersion?.trim();
  return version ? `v${version}` : "";
});
</script>
