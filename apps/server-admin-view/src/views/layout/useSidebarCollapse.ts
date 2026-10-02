import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useConfigStore } from "@/store/config";
import { toast } from "@admin-shared/utils/toast";

export const useSidebarCollapse = () => {
  const configStore = useConfigStore();
  const { t } = useI18n();
  const pendingCollapsed = ref<boolean | null>(null);
  const collapsed = computed(
    () =>
      pendingCollapsed.value ??
      configStore.config?.dashboard_display?.sidebar_collapsed === true,
  );
  const isSaving = computed(() => pendingCollapsed.value !== null);
  const canToggle = computed(
    () =>
      Boolean(configStore.config) &&
      !configStore.isLoading &&
      !configStore.isError &&
      !isSaving.value,
  );

  const toggle = async () => {
    if (!canToggle.value) return;
    pendingCollapsed.value = !collapsed.value;
    try {
      await configStore.saveDashboardDisplayConfig({
        sidebar_collapsed: pendingCollapsed.value,
      });
    } catch (error) {
      toast.error(t("admin.nav.sidebarSaveFailed"), {
        description:
          error instanceof Error ? error.message : t("common.tryLater"),
      });
    } finally {
      pendingCollapsed.value = null;
    }
  };

  return { collapsed, canToggle, isSaving, toggle };
};
