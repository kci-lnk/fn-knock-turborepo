<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import InlineCommentEditor from "@admin-shared/components/InlineCommentEditor.vue";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { toast } from "@admin-shared/utils/toast";
import type { SessionRecord } from "../../types";
import { SessionAPI } from "../../lib/api";
import { Eye, GitBranch, Trash2 } from "lucide-vue-next";
import RefreshButton from "@/components/RefreshButton.vue";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useConfigStore } from "../../store/config";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import DetailDialog from "@admin-shared/components/common/DetailDialog.vue";
import DetailFieldsGrid from "@admin-shared/components/common/DetailFieldsGrid.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { buildDetailFields } from "@admin-shared/utils/buildDetailFields";
import { formatDateTimeSafe } from "@admin-shared/utils/formatDateTimeSafe";
import FnosAttachmentIndicator from "./FnosAttachmentIndicator.vue";
import trimMediaLogoUrl from "@/assets/trim-media-logo.png";

const router = useRouter();
const { t, locale } = useI18n();
const sessions = ref<SessionRecord[]>([]);
const showDetail = ref(false);
const detailSession = ref<SessionRecord | null>(null);

const { isPending: isLoading, run: runLoadSessions } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.sessions.loadFailed"), {
      description: extractErrorMessage(error, t("admin.sessions.loadFailed")),
    });
  },
});

const { isPending: isKicking, run: runKickSession } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.sessions.kickFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.sessions.operationFailed"),
      ),
    });
  },
});

const { run: runUpdateComment } = useAsyncAction({
  rethrow: true,
});

const configStore = useConfigStore();

const detailFieldDefinitions = [
  { key: "id", labelKey: "admin.sessions.table.sessionId" },
  { key: "method", labelKey: "admin.sessions.table.loginMethod" },
  { key: "credentialName", labelKey: "admin.sessions.table.credentialName" },
  { key: "comment", labelKey: "admin.sessions.table.comment" },
  { key: "ip", labelKey: "admin.sessions.table.currentIp" },
  { key: "ipLocation", labelKey: "admin.sessions.table.ipLocation" },
  { key: "userAgent", labelKey: "User-Agent" },
  { key: "loginTime", labelKey: "admin.sessions.table.loginTime" },
  { key: "expiresAt", labelKey: "admin.sessions.table.expiresAt" },
] as const;

const localizedDetailFieldDefinitions = computed(() =>
  detailFieldDefinitions.map((field) => ({
    key: field.key,
    label: field.labelKey === "User-Agent" ? field.labelKey : t(field.labelKey),
  })),
);

const hasSessions = computed(() => sessions.value.length > 0);

const detailItems = computed(() => {
  return buildDetailFields(
    detailSession.value,
    localizedDetailFieldDefinitions.value,
    {
      format: (key, value) => {
        if (key === "loginTime" || key === "expiresAt") {
          return formatDateTimeSafe(
            value as string | number | Date | null | undefined,
            { locale: locale.value },
          );
        }
        return value;
      },
    },
  );
});

const middleEllipsis = (text: string, max = 16) => {
  if (!text) return "";
  if (text.length <= max) return text;
  const head = Math.ceil((max - 1) / 2);
  const tail = Math.floor((max - 1) / 2);
  return `${text.slice(0, head)}……${text.slice(text.length - tail)}`;
};

async function fetchSessions() {
  await runLoadSessions(async () => {
    const nextSessions = await SessionAPI.list();
    sessions.value = Array.isArray(nextSessions) ? nextSessions : [];
  });
}

function openDetail(session: SessionRecord) {
  detailSession.value = session;
  showDetail.value = true;
}

function openMobility(session: SessionRecord) {
  router.push(`/sessions/mobility/${encodeURIComponent(session.id)}`);
}

async function kickSession(sessionId: string) {
  await runKickSession(() => SessionAPI.kick(sessionId), {
    onSuccess: async () => {
      sessions.value = sessions.value.filter(
        (session) => session.id !== sessionId,
      );
      if (detailSession.value?.id === sessionId) {
        detailSession.value = null;
        showDetail.value = false;
      }
      toast.success(t("admin.sessions.kicked"));
      await fetchSessions();
    },
  });
}

async function updateComment(sessionId: string, comment: string) {
  const target = sessions.value.find((session) => session.id === sessionId);
  if (target && (target.comment ?? "") === comment) {
    return;
  }

  await runUpdateComment(() => SessionAPI.updateComment(sessionId, comment), {
    onSuccess: (updated) => {
      if (target) {
        Object.assign(target, updated);
      }
      if (detailSession.value?.id === sessionId) {
        detailSession.value = {
          ...detailSession.value,
          ...updated,
        };
      }
      toast.success(t("admin.sessions.commentUpdated"));
    },
    onError: (error) => {
      throw new Error(
        extractErrorMessage(error, t("admin.sessions.updateCommentFailed")),
      );
    },
  });
}

watch(
  () => configStore.config?.run_type,
  (runType) => {
    if (runType === 1 || runType === 3) {
      void fetchSessions();
    }
  },
  { immediate: true },
);
</script>

<template>
  <div class="space-y-3">
    <div class="flex items-center justify-between">
      <div class="text-sm text-muted-foreground">
        {{ t("admin.sessions.activeCount", { count: sessions.length }) }}
      </div>
      <RefreshButton
        :loading="isLoading"
        :disabled="isLoading"
        @click="fetchSessions"
      />
    </div>

    <div class="overflow-hidden rounded-md border">
      <TooltipProvider>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead class="w-[150px]">{{
                t("admin.sessions.table.sessionId")
              }}</TableHead>
              <TableHead>{{ t("admin.sessions.table.credential") }}</TableHead>
              <TableHead>{{ t("admin.sessions.table.comment") }}</TableHead>
              <TableHead>{{ t("admin.sessions.table.currentIp") }}</TableHead>
              <TableHead>{{ t("admin.sessions.table.loginTime") }}</TableHead>
              <TableHead>{{ t("admin.sessions.table.expiresAt") }}</TableHead>
              <TableHead class="w-[210px] text-right">{{
                t("admin.sessions.table.actions")
              }}</TableHead>
            </TableRow>
          </TableHeader>

          <TableBody v-if="hasSessions">
            <TableRow v-for="session in sessions" :key="session.id">
              <TableCell>
                <Tooltip>
                  <TooltipTrigger as-child>
                    <div class="cursor-help font-mono text-xs">
                      {{ middleEllipsis(session.id, 16) }}
                    </div>
                  </TooltipTrigger>
                  <TooltipContent>
                    <p class="break-all font-mono text-xs">{{ session.id }}</p>
                  </TooltipContent>
                </Tooltip>
              </TableCell>

              <TableCell>
                <div class="flex items-center gap-2">
                  <div class="text-sm">{{ session.credentialName }}</div>
                  <FnosAttachmentIndicator
                    v-if="session.fnosAttachments?.length"
                    :attachments="session.fnosAttachments"
                  />
                  <FnosAttachmentIndicator
                    v-if="session.trimMediaAttachments?.length"
                    :attachments="session.trimMediaAttachments"
                    :icon-url="trimMediaLogoUrl"
                    :icon-alt="t('admin.sessions.attachments.trimMediaIconAlt')"
                    :title="t('admin.sessions.attachments.trimMediaTitle')"
                    :trigger-label="
                      t('admin.sessions.attachments.trimMediaTriggerLabel')
                    "
                    :item-label="
                      t('admin.sessions.attachments.trimMediaItemLabel')
                    "
                    :footer-text="
                      t('admin.sessions.attachments.trimMediaFooter')
                    "
                  />
                </div>
              </TableCell>

              <TableCell class="min-w-[180px]">
                <InlineCommentEditor
                  :text="session.comment"
                  :save="(value) => updateComment(session.id, value)"
                />
              </TableCell>

              <TableCell>
                <Tooltip>
                  <TooltipTrigger as-child>
                    <div class="cursor-help font-mono text-sm">
                      {{ middleEllipsis(session.ip, 24) }}
                    </div>
                  </TooltipTrigger>
                  <TooltipContent>
                    <p class="break-all font-mono text-xs">{{ session.ip }}</p>
                  </TooltipContent>
                </Tooltip>
                <div
                  v-if="session.ipLocation"
                  class="line-clamp-1 text-xs text-muted-foreground"
                >
                  {{ session.ipLocation }}
                </div>
              </TableCell>

              <TableCell>
                <div class="text-sm">
                  <HumanFriendlyTime
                    :value="session.loginTime"
                    :locale="locale"
                  />
                </div>
              </TableCell>

              <TableCell>
                <div class="text-sm">
                  <HumanFriendlyTime
                    :value="session.expiresAt"
                    :locale="locale"
                  />
                </div>
              </TableCell>

              <TableCell class="text-right">
                <div class="flex justify-end gap-2">
                  <Button
                    variant="outline"
                    size="sm"
                    class="gap-1.5"
                    @click="openMobility(session)"
                  >
                    <GitBranch class="h-4 w-4" />
                    {{ t("admin.sessions.mobility") }}
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    class="gap-1.5"
                    @click="openDetail(session)"
                  >
                    <Eye class="h-4 w-4" />
                    {{ t("admin.sessions.detail") }}
                  </Button>
                  <ConfirmDangerPopover
                    :title="t('admin.sessions.confirmKickTitle')"
                    :description="t('admin.sessions.confirmKickDescription')"
                    :confirm-text="t('admin.sessions.confirmKick')"
                    :loading="isKicking"
                    :disabled="isKicking"
                    :on-confirm="() => kickSession(session.id)"
                  >
                    <template #trigger>
                      <Button
                        variant="destructive"
                        size="sm"
                        :disabled="isKicking"
                        class="gap-1.5"
                      >
                        <Trash2 class="h-4 w-4" />
                        {{ t("admin.sessions.kick") }}
                      </Button>
                    </template>
                  </ConfirmDangerPopover>
                </div>
              </TableCell>
            </TableRow>
          </TableBody>

          <TableBody v-else>
            <TableRow>
              <TableCell
                colspan="8"
                class="py-6 text-center text-muted-foreground"
              >
                {{ t("admin.sessions.empty") }}
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </TooltipProvider>
    </div>

    <DetailDialog
      :open="showDetail"
      :title="t('admin.sessions.detailTitle')"
      :description="t('admin.sessions.detailDescription')"
      max-width-class="sm:max-w-[500px]"
      @update:open="showDetail = $event"
    >
      <div v-if="detailSession">
        <DetailFieldsGrid :items="detailItems" layout="compact" />
      </div>
    </DetailDialog>
  </div>
</template>
