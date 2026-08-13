import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { SessionAPI } from "@/lib/api/sessions";
import type { SessionMobilityDetails, SessionRecord } from "@/types";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import {
  buildSessionMobilityTimeline,
  formatSessionMobilityDuration,
  formatSessionMobilitySource,
  getSessionMobilityTimelineSpan,
  middleEllipsis,
} from "./sessionMobilityModel";

type SortOrder = "asc" | "desc";

export const useSessionMobilityPage = () => {
  const route = useRoute();
  const { t, locale } = useI18n();
  const translate = (key: string, params?: Record<string, string | number>) =>
    params ? t(key, params) : t(key);
  const session = ref<SessionRecord | null>(null);
  const mobility = ref<SessionMobilityDetails | null>(null);
  const loadError = ref("");
  const sortOrder = ref<SortOrder>("desc");
  const { isPending: isLoading, run: runLoad } = useAsyncAction({
    onError: (error) => {
      loadError.value = extractErrorMessage(
        error,
        t("admin.sessions.mobilityPage.loadFailedFallback"),
      );
    },
  });

  const sessionId = computed(() => String(route.params.id || ""));
  const mobilitySummary = computed(
    () => mobility.value?.summary ?? session.value?.mobility ?? null,
  );
  const sortToggleLabel = computed(() =>
    sortOrder.value === "desc"
      ? t("admin.sessions.mobilityPage.sortDesc")
      : t("admin.sessions.mobilityPage.sortAsc"),
  );
  const headerDescription = computed(() => {
    if (!session.value) {
      return t("admin.sessions.mobilityPage.defaultDescription");
    }
    return t("admin.sessions.mobilityPage.sessionDescription", {
      name: session.value.credentialName,
      id: middleEllipsis(session.value.id, 20),
    });
  });
  const driftCountDescription = computed(() => {
    const count = mobilitySummary.value?.driftCount ?? 0;
    if (count === 0) return t("admin.sessions.mobilityPage.driftCountZero");
    if (count === 1) return t("admin.sessions.mobilityPage.driftCountOne");
    return t("admin.sessions.mobilityPage.driftCountMany", { count });
  });
  const chronologicalEntries = computed(() =>
    buildSessionMobilityTimeline(mobility.value?.events ?? [], translate),
  );
  const timelineEntries = computed(() => {
    const entries = [...chronologicalEntries.value];
    return sortOrder.value === "desc" ? entries.reverse() : entries;
  });
  const latestEntryId = computed(
    () =>
      chronologicalEntries.value[chronologicalEntries.value.length - 1]?.id ??
      "",
  );
  const timelineSpanMs = computed(() =>
    getSessionMobilityTimelineSpan(chronologicalEntries.value),
  );
  const timelineSpanLabel = computed(() =>
    timelineSpanMs.value <= 0
      ? t("admin.sessions.mobilityPage.noSpan")
      : formatSessionMobilityDuration(timelineSpanMs.value, translate),
  );
  const timelineSpanDescription = computed(() =>
    chronologicalEntries.value.length <= 1
      ? t("admin.sessions.mobilityPage.onlyLoginStart")
      : t("admin.sessions.mobilityPage.spanDescription"),
  );
  const lastEvent = computed(
    () =>
      chronologicalEntries.value[chronologicalEntries.value.length - 1] ?? null,
  );
  const lastEventTimeLabel = computed(() => {
    if (!lastEvent.value) return t("admin.sessions.mobilityPage.noRecord");
    return lastEvent.value.event.kind === "login"
      ? t("admin.sessions.mobilityPage.loginOnlyRecord")
      : "";
  });
  const lastEventTimeValue = computed(() => {
    if (!lastEvent.value || lastEvent.value.event.kind === "login") return null;
    return lastEvent.value.event.happenedAt;
  });
  const lastEventSourceLabel = computed(() => {
    if (!lastEvent.value) {
      return t("admin.sessions.mobilityPage.noChangeSource");
    }
    if (lastEvent.value.event.kind === "login") {
      return t("admin.sessions.mobilityPage.noIpChangeYet");
    }
    return t("admin.sessions.mobilityPage.sourcePrefix", {
      source: formatSessionMobilitySource(
        lastEvent.value.event.source,
        translate,
      ),
    });
  });

  const fetchData = async () => {
    if (!sessionId.value) return;
    loadError.value = "";
    session.value = null;
    mobility.value = null;
    await runLoad(async () => {
      const [sessionData, mobilityData] = await Promise.all([
        SessionAPI.get(sessionId.value),
        SessionAPI.getMobility(sessionId.value),
      ]);
      session.value = sessionData;
      mobility.value = mobilityData;
    });
  };
  const toggleSortOrder = () => {
    sortOrder.value = sortOrder.value === "desc" ? "asc" : "desc";
  };

  watch(sessionId, () => void fetchData(), { immediate: true });

  return {
    driftCountDescription,
    headerDescription,
    isLoading,
    lastEventSourceLabel,
    lastEventTimeLabel,
    lastEventTimeValue,
    latestEntryId,
    loadError,
    locale,
    mobilitySummary,
    session,
    sortOrder,
    sortToggleLabel,
    t,
    timelineEntries,
    timelineSpanDescription,
    timelineSpanLabel,
    toggleSortOrder,
  };
};
