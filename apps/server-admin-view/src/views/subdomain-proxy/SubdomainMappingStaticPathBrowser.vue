<script setup lang="ts">
import { computed, type UnwrapNestedRefs } from "vue";
import { useI18n } from "vue-i18n";
import {
  ChevronLeft,
  ChevronRight,
  File,
  Folder,
  FolderRoot,
  Loader2,
  RefreshCw,
  RotateCcw,
  TriangleAlert,
} from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { useConfigStore } from "@/store/config";
import type { StaticPathBrowseEntry } from "@/lib/api/config";
import type { useStaticPathBrowser } from "./useStaticPathBrowser";

const { editor } = defineProps<{
  editor: UnwrapNestedRefs<ReturnType<typeof useStaticPathBrowser>>;
}>();
const { locale, t } = useI18n();
const configStore = useConfigStore();

const browserPathLabel = computed(
  () =>
    editor.currentPath ??
    t("admin.subdomainProxy.staticServe.browser.rootLocation"),
);

const formatSize = (size: number | null) => {
  if (size === null) return "—";
  if (!Number.isFinite(size) || size < 0) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const exponent =
    size === 0
      ? 0
      : Math.min(units.length - 1, Math.floor(Math.log(size) / Math.log(1024)));
  return `${new Intl.NumberFormat(String(locale.value), {
    maximumFractionDigits: exponent === 0 ? 0 : 1,
  }).format(size / 1024 ** exponent)} ${units[exponent]}`;
};

const formatModified = (value: string | null) => {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "—";
  return new Intl.DateTimeFormat(String(locale.value), {
    dateStyle: "short",
    timeStyle: "short",
  }).format(date);
};

const isEntryDisabled = (entry: StaticPathBrowseEntry) =>
  !(
    (entry.entry_type === "directory" && entry.navigable) ||
    (editor.targetType === "file" &&
      entry.entry_type === "file" &&
      entry.selectable)
  );
const entryAriaLabel = (entry: StaticPathBrowseEntry) =>
  t(
    entry.entry_type === "directory"
      ? "admin.subdomainProxy.staticServe.browser.openDirectoryAria"
      : "admin.subdomainProxy.staticServe.browser.selectFileAria",
    { name: entry.name },
  );
</script>

<template>
  <div class="space-y-4 pb-5 pt-5" data-testid="static-path-browser">
    <div class="space-y-1">
      <p class="text-sm text-muted-foreground">
        {{ t("admin.subdomainProxy.staticServe.browser.hint") }}
      </p>
      <p
        v-if="configStore.isDockerDeployment"
        class="text-xs leading-5 text-amber-700 dark:text-amber-300"
      >
        {{ t("admin.subdomainProxy.staticServe.browser.dockerHint") }}
      </p>
    </div>

    <div class="space-y-2 rounded-lg border bg-muted/10 p-3">
      <div class="flex flex-wrap items-center gap-2">
        <Button
          type="button"
          size="sm"
          variant="outline"
          :disabled="editor.isLoading"
          @click="editor.navigateRoot"
        >
          <FolderRoot class="mr-1.5 h-4 w-4" />
          {{ t("admin.subdomainProxy.staticServe.browser.root") }}
        </Button>
        <Button
          type="button"
          size="sm"
          variant="outline"
          :disabled="editor.isLoading || editor.parentPath === null"
          @click="editor.navigateParent"
        >
          <ChevronLeft class="mr-1.5 h-4 w-4" />
          {{ t("admin.subdomainProxy.staticServe.browser.parent") }}
        </Button>
        <Button
          type="button"
          size="icon"
          variant="ghost"
          class="ml-auto h-8 w-8"
          :aria-label="t('admin.subdomainProxy.staticServe.browser.refresh')"
          :disabled="editor.isLoading"
          @click="editor.refresh"
        >
          <RefreshCw class="h-4 w-4" />
        </Button>
      </div>

      <nav
        class="flex min-w-0 items-center gap-1 overflow-x-auto pb-1 text-sm"
        :aria-label="
          t('admin.subdomainProxy.staticServe.browser.breadcrumbsAria')
        "
      >
        <template
          v-for="(breadcrumb, index) in editor.breadcrumbs"
          :key="`${breadcrumb.path}-${index}`"
        >
          <ChevronRight v-if="index > 0" class="h-3.5 w-3.5 shrink-0" />
          <button
            type="button"
            class="max-w-48 shrink-0 truncate rounded px-1.5 py-1 font-mono text-xs hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:text-foreground disabled:opacity-100"
            :disabled="
              editor.isLoading || breadcrumb.path === editor.currentPath
            "
            :title="breadcrumb.path"
            @click="editor.navigateBreadcrumb(breadcrumb.path)"
          >
            {{ breadcrumb.name }}
          </button>
        </template>
        <span
          v-if="editor.breadcrumbs.length === 0"
          class="truncate font-mono text-xs text-muted-foreground"
          :title="browserPathLabel"
        >
          {{ browserPathLabel }}
        </span>
      </nav>
    </div>

    <div
      class="relative min-h-56 overflow-hidden rounded-lg border bg-background"
      :aria-busy="editor.isLoading"
    >
      <div
        class="grid grid-cols-[minmax(0,1fr)_6rem_10rem] border-b bg-muted/40 px-3 py-2 text-xs font-medium text-muted-foreground max-sm:grid-cols-1"
      >
        <span>{{ t("admin.subdomainProxy.staticServe.browser.name") }}</span>
        <span class="text-right max-sm:hidden">
          {{ t("admin.subdomainProxy.staticServe.browser.size") }}
        </span>
        <span class="text-right max-sm:hidden">
          {{ t("admin.subdomainProxy.staticServe.browser.modified") }}
        </span>
      </div>

      <div
        v-if="editor.isLoading && !editor.result"
        class="flex min-h-48 items-center justify-center gap-2 text-sm text-muted-foreground"
        role="status"
      >
        <Loader2 class="h-4 w-4 animate-spin" />
        {{ t("admin.subdomainProxy.staticServe.browser.loading") }}
      </div>

      <div
        v-else-if="editor.loadError"
        class="flex min-h-48 flex-col items-center justify-center gap-3 px-6 py-8 text-center"
        role="alert"
      >
        <TriangleAlert class="h-6 w-6 text-destructive" />
        <p class="max-w-md text-sm text-destructive">
          {{ editor.loadError }}
        </p>
        <Button
          type="button"
          size="sm"
          variant="outline"
          @click="editor.refresh"
        >
          <RotateCcw class="mr-1.5 h-4 w-4" />
          {{ t("admin.subdomainProxy.staticServe.browser.retry") }}
        </Button>
      </div>

      <div
        v-else-if="editor.entries.length === 0"
        class="flex min-h-48 items-center justify-center px-6 text-center text-sm text-muted-foreground"
      >
        {{ t("admin.subdomainProxy.staticServe.browser.empty") }}
      </div>

      <div v-else class="divide-y" :class="editor.isLoading && 'opacity-50'">
        <button
          v-for="entry in editor.entries"
          :key="entry.path"
          type="button"
          class="grid w-full grid-cols-[minmax(0,1fr)_6rem_10rem] items-center px-3 py-2.5 text-left text-sm transition-colors hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50 max-sm:grid-cols-1"
          :class="
            editor.selectedPath === entry.path &&
            'bg-primary/10 text-primary hover:bg-primary/15'
          "
          :disabled="editor.isLoading || isEntryDisabled(entry)"
          :aria-label="entryAriaLabel(entry)"
          :aria-pressed="
            entry.entry_type === 'file'
              ? editor.selectedPath === entry.path
              : undefined
          "
          @click="editor.activateEntry(entry)"
        >
          <span class="flex min-w-0 items-center gap-2">
            <Folder
              v-if="entry.entry_type === 'directory'"
              class="h-4 w-4 shrink-0 text-amber-600"
            />
            <File v-else class="h-4 w-4 shrink-0 text-muted-foreground" />
            <span class="truncate" :title="entry.path">{{ entry.name }}</span>
          </span>
          <span class="text-right text-xs text-muted-foreground max-sm:hidden">
            {{
              entry.entry_type === "directory"
                ? "—"
                : formatSize(entry.size_bytes)
            }}
          </span>
          <span class="text-right text-xs text-muted-foreground max-sm:hidden">
            {{ formatModified(entry.modified_at) }}
          </span>
        </button>
      </div>

      <div
        v-if="editor.result"
        class="flex items-center justify-between gap-3 border-t bg-muted/20 px-3 py-2"
      >
        <Button
          type="button"
          size="sm"
          variant="ghost"
          :disabled="editor.isLoading || !editor.previousCursor"
          @click="editor.loadPreviousPage"
        >
          <ChevronLeft class="mr-1 h-4 w-4" />
          {{ t("admin.subdomainProxy.staticServe.browser.previousPage") }}
        </Button>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          :disabled="editor.isLoading || !editor.nextCursor"
          @click="editor.loadNextPage"
        >
          {{ t("admin.subdomainProxy.staticServe.browser.nextPage") }}
          <ChevronRight class="ml-1 h-4 w-4" />
        </Button>
      </div>
    </div>

    <div class="space-y-1 text-xs" aria-live="polite">
      <p class="text-muted-foreground">
        {{ t("admin.subdomainProxy.staticServe.browser.selectedPath") }}
        <span class="break-all font-mono text-foreground">
          {{
            editor.selectionPath ??
            t("admin.subdomainProxy.staticServe.browser.noSelection")
          }}
        </span>
      </p>
      <p v-if="editor.confirmError" class="text-destructive" role="alert">
        {{ editor.confirmError }}
      </p>
    </div>
  </div>
</template>
