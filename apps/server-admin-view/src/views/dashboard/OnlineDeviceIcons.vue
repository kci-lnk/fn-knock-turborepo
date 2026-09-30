<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  Bot,
  Chrome,
  CircleHelp,
  LaptopMinimal,
  PanelsTopLeft,
  Smartphone,
  Tablet,
  Terminal,
} from "lucide-vue-next";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type { DashboardOnlineIp } from "@/types";

const props = defineProps<{ devices?: DashboardOnlineIp["devices"] }>();
const { t } = useI18n();
const platforms = [
  { type: "macos", name: "macOS", icon: LaptopMinimal },
  { type: "windows", name: "Windows", icon: PanelsTopLeft },
  { type: "iphone", name: "iPhone", icon: Smartphone },
  { type: "ipad", name: "iPad", icon: Tablet },
  { type: "android", name: "Android", icon: Bot },
  { type: "linux", name: "Linux", icon: Terminal },
  { type: "chromeos", name: "ChromeOS", icon: Chrome },
  { type: "unknown", name: "", icon: CircleHelp },
];
const entries = computed(() => {
  const counts = new Map<string, number>();
  for (const device of props.devices ?? []) {
    if (!Number.isSafeInteger(device.count) || device.count <= 0) continue;
    const type = platforms.some((platform) => platform.type === device.type)
      ? device.type
      : "unknown";
    counts.set(type, (counts.get(type) ?? 0) + device.count);
  }
  return platforms.flatMap((platform) => {
    const count = counts.get(platform.type);
    if (!count) return [];
    const name = platform.name || t("admin.dashboard.onlineIps.unknownDevice");
    return [
      {
        ...platform,
        count,
        label: t("admin.dashboard.onlineIps.deviceCount", { name, count }),
      },
    ];
  });
});
</script>

<template>
  <span
    v-if="entries.length"
    class="inline-flex max-w-full flex-wrap items-center gap-x-1.5 gap-y-1 text-muted-foreground"
    data-testid="online-device-icons"
  >
    <TooltipProvider :delay-duration="200">
      <Tooltip v-for="entry in entries" :key="entry.type">
        <TooltipTrigger as-child>
          <span
            role="img"
            tabindex="0"
            :aria-label="entry.label"
            :data-device="entry.type"
            class="inline-flex shrink-0 items-center gap-0.5 rounded-sm font-sans text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            <component :is="entry.icon" class="size-3.5" aria-hidden="true" />
            <span v-if="entry.count > 1" aria-hidden="true" class="tabular-nums"
              >×{{ entry.count }}</span
            >
          </span>
        </TooltipTrigger>
        <TooltipContent>{{ entry.label }}</TooltipContent>
      </Tooltip>
    </TooltipProvider>
  </span>
</template>
