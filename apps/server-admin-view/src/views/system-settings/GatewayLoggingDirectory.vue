<script setup lang="ts">
import { computed, nextTick, onMounted, reactive, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import StaticPathBrowser from "../subdomain-proxy/SubdomainMappingStaticPathBrowser.vue";
import { useStaticPathBrowser } from "../subdomain-proxy/useStaticPathBrowser";

const props = defineProps<{
  actualDirectory: string;
  defaultDirectory: string;
  disabled: boolean;
}>();
const model = defineModel<string>({ required: true });
const { t } = useI18n();
const route = useRoute();
const open = ref(false);
const editor = reactive(
  useStaticPathBrowser({
    active: computed(() => open.value),
    applyPath: (path) => {
      model.value = path;
    },
    currentTargetType: computed(() => "directory" as const),
    isDialogOpen: open,
    openView: () => {
      open.value = true;
    },
    returnBasicView: () => {
      open.value = false;
    },
    translate: (key, params) => (params ? t(key, params) : t(key)),
  }),
);
const browse = () => {
  open.value = true;
  editor.openPathBrowser(
    "directory",
    model.value || props.actualDirectory || props.defaultDirectory,
  );
};
const focusDirectory = async () => {
  if (route.hash !== "#gateway-log-directory") return;
  await nextTick();
  const input = document.getElementById("gateway-log-directory");
  input?.scrollIntoView({ block: "center" });
  input?.focus({ preventScroll: true });
};
onMounted(focusDirectory);
watch(() => route.hash, focusDirectory);
</script>

<template>
  <div class="space-y-3 p-6">
    <Label for="gateway-log-directory" class="text-base">{{
      t("admin.gatewayLogging.directoryLabel")
    }}</Label>
    <div class="flex flex-col gap-2 sm:flex-row">
      <Input
        id="gateway-log-directory"
        v-model="model"
        class="min-w-0 font-mono"
        :disabled="disabled"
        :placeholder="defaultDirectory"
        autocomplete="off"
        autocapitalize="none"
        :spellcheck="false"
        aria-describedby="gateway-log-directory-help"
      />
      <div class="flex shrink-0 gap-2">
        <Button variant="outline" :disabled="disabled" @click="browse">{{
          t("admin.gatewayLogging.browseDirectory")
        }}</Button>
        <Button
          variant="outline"
          :disabled="disabled || !model"
          @click="model = ''"
          >{{ t("admin.gatewayLogging.restoreDefaultDirectory") }}</Button
        >
      </div>
    </div>
    <div
      id="gateway-log-directory-help"
      class="space-y-1 break-all text-sm text-muted-foreground"
    >
      <p>
        {{
          t("admin.gatewayLogging.actualDirectory", {
            path: actualDirectory || "—",
          })
        }}
      </p>
      <p>
        {{
          t("admin.gatewayLogging.defaultDirectory", {
            path: defaultDirectory || "—",
          })
        }}
      </p>
      <p>{{ t("admin.gatewayLogging.directoryHelp") }}</p>
    </div>
    <Dialog v-model:open="open">
      <DialogContent class="flex max-h-[85dvh] flex-col sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>{{
            t("admin.gatewayLogging.browseDirectory")
          }}</DialogTitle>
          <DialogDescription>{{
            t("admin.gatewayLogging.directoryBrowserHelp")
          }}</DialogDescription>
        </DialogHeader>
        <div class="min-h-0 overflow-y-auto">
          <StaticPathBrowser
            :editor="editor"
            :hint="t('admin.gatewayLogging.directoryBrowserHelp')"
          />
        </div>
        <DialogFooter>
          <Button variant="outline" @click="editor.cancel">{{
            t("common.cancel")
          }}</Button>
          <Button
            :disabled="!editor.canConfirm"
            @click="editor.confirmSelection"
            >{{ t("admin.gatewayLogging.selectDirectory") }}</Button
          >
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
