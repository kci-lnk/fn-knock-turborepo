<script setup lang="ts">
import { computed, reactive, ref, useId } from "vue";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useConfigStore } from "@/store/config";
import { acmeCertificateOutputPaths } from "@/lib/acme-download";
import StaticPathBrowser from "../subdomain-proxy/SubdomainMappingStaticPathBrowser.vue";
import { useStaticPathBrowser } from "../subdomain-proxy/useStaticPathBrowser";

const props = defineProps<{ domain: string; disabled?: boolean }>();
const enabled = defineModel<boolean>("enabled", { required: true });
const directory = defineModel<string>("directory", { required: true });
const { t } = useI18n();
const configStore = useConfigStore();
const id = useId();
const open = ref(false);
const paths = computed(() =>
  acmeCertificateOutputPaths(directory.value, props.domain),
);
const editor = reactive(
  useStaticPathBrowser({
    active: computed(() => open.value),
    applyPath: (path) => {
      directory.value = path;
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
function browse() {
  open.value = true;
  editor.openPathBrowser("directory", directory.value.trim());
}
</script>

<template>
  <section
    class="grid gap-3 rounded-xl border bg-muted/15 p-4"
    data-testid="acme-file-output-settings"
  >
    <div class="flex items-center justify-between gap-4">
      <Label :for="`${id}-enabled`">{{
        t("admin.acmeFileOutput.title")
      }}</Label>
      <Switch :id="`${id}-enabled`" v-model="enabled" :disabled="disabled" />
    </div>
    <p class="text-xs leading-5 text-muted-foreground">
      {{ t("admin.acmeFileOutput.description") }}
    </p>
    <template v-if="enabled">
      <Label :for="`${id}-directory`">{{
        t("admin.acmeFileOutput.directory")
      }}</Label>
      <div class="flex flex-col gap-2 sm:flex-row">
        <Input
          :id="`${id}-directory`"
          v-model="directory"
          class="min-w-0 font-mono"
          :disabled="disabled"
          autocomplete="off"
          autocapitalize="none"
          :spellcheck="false"
          :aria-describedby="`${id}-help`"
        />
        <Button
          type="button"
          variant="outline"
          :disabled="disabled"
          @click="browse"
          >{{ t("admin.acmeFileOutput.browse") }}</Button
        >
      </div>
      <div
        :id="`${id}-help`"
        class="space-y-1 text-xs leading-5 text-muted-foreground"
      >
        <p>{{ t("admin.acmeFileOutput.directoryHelp") }}</p>
        <p v-if="configStore.isDockerDeployment">
          {{ t("admin.subdomainProxy.staticServe.browser.dockerHint") }}
        </p>
        <div
          v-if="directory.trim() && domain.trim()"
          class="space-y-1 break-all font-mono"
          data-testid="acme-output-paths"
        >
          <p>{{ paths.certificatePath }}</p>
          <p>{{ paths.privateKeyPath }}</p>
        </div>
        <p>{{ t("admin.acmeFileOutput.overwriteHelp") }}</p>
      </div>
    </template>
    <Dialog v-model:open="open">
      <DialogContent class="flex max-h-[85dvh] flex-col sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>{{ t("admin.acmeFileOutput.browse") }}</DialogTitle>
          <DialogDescription>{{
            t("admin.acmeFileOutput.browserHelp")
          }}</DialogDescription>
        </DialogHeader>
        <div class="min-h-0 overflow-y-auto">
          <StaticPathBrowser
            :editor="editor"
            :hint="t('admin.acmeFileOutput.browserHelp')"
          />
        </div>
        <DialogFooter>
          <Button type="button" variant="outline" @click="editor.cancel">{{
            t("common.cancel")
          }}</Button>
          <Button
            type="button"
            :disabled="!editor.canConfirm"
            @click="editor.confirmSelection"
            >{{ t("admin.acmeFileOutput.selectDirectory") }}</Button
          >
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </section>
</template>
