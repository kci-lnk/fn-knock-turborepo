<script setup lang="ts">
import { computed } from "vue";
import { Plus } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import StreamBypassRuleGroupCard from "./StreamBypassRuleGroupCard.vue";
import {
  createStreamBypassRuleEditor,
  MAX_STREAM_BYPASS_GROUPS,
  type StreamBypassPolicyForm,
} from "./stream-bypass-policy-form";

const { form, saving, valueDrafts } = defineProps<{
  form: StreamBypassPolicyForm;
  saving: boolean;
  valueDrafts: Record<string, string>;
}>();
const { t } = useI18n();
const editor = createStreamBypassRuleEditor(form, valueDrafts);
const regionText = computed(() => ({
  add: t("admin.advancedAuth.addRegion"),
  addRegion: t("admin.advancedAuth.addRegion"),
  cancel: t("common.cancel"),
  dialogDescription: t("admin.advancedAuth.regionDialogDescription"),
  loadFailed: t("admin.advancedAuth.regionLoadFailed"),
  loadFailedDescription: t("admin.advancedAuth.regionLoadFailedDescription"),
  loading: t("common.loadingConfig"),
  noRegions: t("admin.advancedAuth.noRegions"),
  province: t("admin.advancedAuth.province"),
  retry: t("admin.advancedAuth.retry"),
  selectedCount: (count: number) =>
    t("admin.advancedAuth.selectedRegions", { count }),
  scope: t("admin.advancedAuth.scope"),
  selectCity: t("admin.advancedAuth.selectCity"),
  selectProvince: t("admin.advancedAuth.selectProvince"),
  selectProvinceFirst: t("admin.advancedAuth.selectProvinceFirst"),
  unavailable: t("admin.advancedAuth.unavailable"),
}));
</script>

<template>
  <section class="space-y-4">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div>
        <h2 class="text-base font-medium">
          {{ t("admin.streamMappings.policyRuleGroups") }}
        </h2>
        <p class="text-sm text-muted-foreground">
          {{ t("admin.streamMappings.bypassPolicyDescription") }}
        </p>
      </div>
      <Button
        variant="outline"
        class="w-full min-[480px]:w-auto"
        :disabled="form.groups.length >= MAX_STREAM_BYPASS_GROUPS || saving"
        @click="editor.addGroup"
      >
        <Plus class="mr-2 h-4 w-4" />
        {{ t("admin.advancedAuth.addOrGroup") }}
      </Button>
    </div>

    <div
      v-if="form.groups.length === 0"
      class="rounded-xl border border-dashed p-8 text-center text-sm text-muted-foreground"
    >
      {{ t("admin.streamMappings.policyNoGroups") }}
    </div>
    <div
      v-else
      class="relative space-y-4 sm:space-y-5 sm:pl-10 sm:before:absolute sm:before:inset-y-7 sm:before:left-4 sm:before:w-px sm:before:bg-border"
    >
      <div
        v-for="(group, groupIndex) in form.groups"
        :key="group.id"
        class="relative sm:before:absolute sm:before:top-7 sm:before:-left-6 sm:before:h-px sm:before:w-6 sm:before:bg-border"
      >
        <StreamBypassRuleGroupCard
          :editor="editor"
          :group="group"
          :group-index="groupIndex"
          :region-text="regionText"
          :saving="saving"
        />
      </div>
    </div>
  </section>
</template>
