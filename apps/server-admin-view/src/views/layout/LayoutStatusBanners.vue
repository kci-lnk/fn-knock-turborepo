<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { useConfigStore } from "@/store/config";
import { useSystemClockStore } from "@/store/systemClock";
import { useUpdateStore } from "@/store/update";

const props = defineProps<{
  navigateTo: (path: string) => Promise<void>;
}>();

const { t } = useI18n();
const configStore = useConfigStore();
const systemClockStore = useSystemClockStore();
const updateStore = useUpdateStore();

const systemClockBannerTitle = computed(() => {
  const status = systemClockStore.status;
  if (!status) return "";
  if (status.timezoneMismatch && status.timeMismatch) {
    return t("admin.banner.clockImmediate");
  }
  if (status.timezoneMismatch) return t("admin.banner.timezoneMismatch");
  return t("admin.banner.clockMismatch");
});

const systemClockBannerDescription = computed(() => {
  const status = systemClockStore.status;
  if (!status) return "";
  const messages = status.issues.map((issue) => issue.message);
  if (status.lastCheckError) {
    messages.push(
      t("admin.banner.lastCheckFailed", { error: status.lastCheckError }),
    );
  }
  if (!configStore.canSyncSystemClock) {
    messages.push(t("admin.banner.hostSyncUnsupported"));
  }
  return messages.join(" ");
});

const systemClockBannerMeta = computed(() => {
  const status = systemClockStore.status;
  if (!status) return "";
  const parts: string[] = [];
  if (status.systemBeijingTime) {
    parts.push(
      t("admin.banner.systemBeijingTime", { time: status.systemBeijingTime }),
    );
  }
  if (status.remoteBeijingTime) {
    parts.push(
      t("admin.banner.remoteBeijingTime", { time: status.remoteBeijingTime }),
    );
  }
  if (status.systemTimeZone) {
    parts.push(
      t("admin.banner.systemTimeZone", { timezone: status.systemTimeZone }),
    );
  }
  if (status.networkSource) {
    parts.push(
      t("admin.banner.networkSource", { source: status.networkSource }),
    );
  }
  return parts.join(" · ");
});

const updateBannerDescription = computed(() => {
  if (configStore.canSelfUpdate) {
    return updateStore.isForceUpdate
      ? t("admin.banner.importantUpdate")
      : t("admin.banner.normalUpdate");
  }
  if (configStore.isOpenWrtDeployment) {
    return t("admin.banner.openWrtUpdateInfo");
  }
  if (configStore.isDockerDeployment) {
    return t("admin.banner.dockerUpdateInfo");
  }
  if (configStore.isSynologyDeployment) {
    return t("admin.banner.synologyUpdateInfo");
  }
  if (configStore.isDesktopUpdateManaged) {
    return t("admin.banner.windowsUpdateInfo");
  }
  return t("admin.banner.genericUpdateInfo");
});

const startUpdate = async () => {
  await props.navigateTo("/about");
  if (configStore.canSelfUpdate) {
    await updateStore.checkAndDownload();
  }
};
</script>

<template>
  <div
    v-if="
      configStore.canSyncSystemClock &&
      systemClockStore.shouldShowBanner &&
      systemClockStore.status
    "
    :class="[
      'mx-auto mt-3 mb-6 w-full max-w-7xl rounded-lg border px-4 py-3',
      systemClockStore.status.timeMismatch
        ? 'border-destructive/35 bg-destructive/10 text-destructive'
        : 'border-amber-500/35 bg-amber-500/10 text-amber-900 dark:text-amber-200',
    ]"
  >
    <div
      class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
    >
      <div class="space-y-1">
        <p class="text-sm font-semibold">{{ systemClockBannerTitle }}</p>
        <p class="text-xs leading-5">{{ systemClockBannerDescription }}</p>
        <p
          v-if="systemClockBannerMeta"
          class="text-[11px] leading-5 opacity-85"
        >
          {{ systemClockBannerMeta }}
        </p>
      </div>
      <div class="flex items-center gap-2">
        <Button
          variant="outline"
          size="sm"
          class="bg-background/80"
          :disabled="
            systemClockStore.isRefreshing || systemClockStore.isSyncing
          "
          @click="systemClockStore.refresh(true)"
        >
          {{ t("common.refreshStatus") }}
        </Button>
        <Button
          size="sm"
          :variant="
            systemClockStore.status.timeMismatch ? 'destructive' : 'default'
          "
          :disabled="systemClockStore.isSyncing"
          @click="systemClockStore.sync()"
        >
          {{ t("common.syncNow") }}
        </Button>
      </div>
    </div>
  </div>

  <div
    v-if="updateStore.shouldShowBanner && updateStore.status"
    :class="[
      'mx-auto mt-3 mb-6 w-full max-w-7xl rounded-lg border px-4 py-3',
      updateStore.isForceUpdate
        ? 'border-destructive/35 bg-destructive/10 text-destructive'
        : 'border-amber-500/35 bg-amber-500/10 text-amber-900 dark:text-amber-200',
    ]"
  >
    <div
      class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
    >
      <div class="space-y-1">
        <p class="text-sm font-semibold">
          {{
            t("admin.banner.updateFound", {
              latest: updateStore.status.latest?.version || "",
              current: updateStore.status.localVersion,
            })
          }}
        </p>
        <p class="text-xs">{{ updateBannerDescription }}</p>
      </div>
      <div class="flex items-center gap-2">
        <Button
          variant="outline"
          size="sm"
          class="bg-background/80"
          @click="navigateTo('/about')"
        >
          {{ t("common.viewDetails") }}
        </Button>
        <Button
          v-if="configStore.canSelfUpdate"
          size="sm"
          :variant="updateStore.isForceUpdate ? 'destructive' : 'default'"
          @click="startUpdate"
        >
          {{ t("common.updateNow") }}
        </Button>
      </div>
    </div>
  </div>
</template>
