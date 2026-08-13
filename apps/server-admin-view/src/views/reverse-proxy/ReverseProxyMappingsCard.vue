<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import { docsUrls } from "@/lib/docs";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import PagedTableFooter from "@admin-shared/components/list/PagedTableFooter.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import {
  ChevronDown,
  Plus,
  RefreshCw,
  Search,
  SlidersHorizontal,
} from "lucide-vue-next";
import { isWebSocketProxyTargetUrl } from "@admin-shared/utils/proxyTargetInput";
import type { ReverseProxyPageModel } from "./useReverseProxyPage";

defineProps<{ model: ReverseProxyPageModel }>();
const { t } = useI18n();
</script>

<template>
  <Card class="mb-6">
    <CardHeader>
      <CardTitle class="flex items-center justify-between">
        <span>{{ t("admin.reverseProxy.title") }}</span>
        <div class="flex items-center gap-2">
          <DocsLinkButton :href="docsUrls.guides.reverseProxy" />
          <div class="flex">
            <Button class="rounded-r-none" @click="model.openDiscoverDialog">
              <Search class="mr-2 h-4 w-4" />
              {{ t("admin.reverseProxy.discover") }}
            </Button>
            <DropdownMenu>
              <DropdownMenuTrigger as-child>
                <Button
                  variant="default"
                  size="icon"
                  :aria-label="t('common.moreActions')"
                  class="rounded-l-none border-l border-primary-foreground/20 px-2"
                >
                  <ChevronDown class="h-4 w-4" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuItem
                  :disabled="model.isDiscovering"
                  @select="model.isScanIntensityDialogOpen = true"
                >
                  <SlidersHorizontal class="mr-2 h-4 w-4" />
                  {{ t("admin.scanIntensity.title") }}
                </DropdownMenuItem>
                <DropdownMenuItem @click="model.openAddDialog">
                  <Plus class="mr-2 h-4 w-4" />
                  {{ t("admin.reverseProxy.addMapping") }}
                </DropdownMenuItem>
                <DropdownMenuItem
                  :disabled="model.isSyncing"
                  @click="model.syncRoutes"
                >
                  <RefreshCw
                    class="mr-2 h-4 w-4"
                    :class="{ 'animate-spin': model.isSyncing }"
                  />
                  {{
                    model.isSyncing
                      ? t("admin.reverseProxy.syncing")
                      : t("admin.reverseProxy.syncRoutes")
                  }}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </div>
      </CardTitle>
      <CardDescription>
        {{
          t("admin.reverseProxy.description", {
            port: model.accessEntryPort,
          })
        }}
      </CardDescription>
    </CardHeader>

    <CardContent>
      <div class="mb-4 flex items-center space-x-2">
        <SearchInput
          :model-value="model.searchQuery"
          :placeholder="t('admin.reverseProxy.searchPlaceholder')"
          class="max-w-xs"
          @update:model-value="model.setSearchQuery"
        />
      </div>

      <div class="overflow-x-auto rounded-md border">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{{ t("admin.reverseProxy.columns.path") }}</TableHead>
              <TableHead>{{ t("admin.reverseProxy.columns.target") }}</TableHead>
              <TableHead>{{ t("admin.reverseProxy.columns.options") }}</TableHead>
              <TableHead class="text-right">
                {{ t("admin.reverseProxy.columns.actions") }}
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-if="model.paginatedMappings.length === 0">
              <TableCell
                colspan="4"
                class="py-6 text-center text-muted-foreground"
              >
                {{ t("admin.reverseProxy.empty") }}
              </TableCell>
            </TableRow>
            <TableRow
              v-for="mapping in model.paginatedMappings"
              :key="mapping.path"
              class="group transition-colors"
            >
              <TableCell class="font-medium">{{ mapping.path }}</TableCell>
              <TableCell>{{ mapping.target }}</TableCell>
              <TableCell>
                <div
                  class="flex flex-wrap gap-2 whitespace-normal text-xs text-muted-foreground"
                >
                  <Badge
                    v-if="model.isDefaultRoute(mapping.path)"
                    variant="secondary"
                    class="border border-emerald-500/30 bg-emerald-500/10 text-emerald-700"
                  >
                    {{ t("admin.reverseProxy.defaultRoute") }}
                  </Badge>
                  <span
                    v-if="
                      mapping.rewrite_html &&
                      !isWebSocketProxyTargetUrl(mapping.target)
                    "
                    class="rounded bg-muted px-2 py-0.5"
                  >
                    {{ t("admin.reverseProxy.rewriteHtml") }}
                  </span>
                  <span
                    v-if="mapping.use_auth"
                    class="rounded bg-muted px-2 py-0.5"
                  >
                    {{ t("admin.reverseProxy.authRequiredShort") }}
                  </span>
                  <span
                    v-if="
                      mapping.use_root_mode &&
                      !isWebSocketProxyTargetUrl(mapping.target)
                    "
                    class="rounded bg-muted px-2 py-0.5"
                  >
                    {{ t("admin.reverseProxy.rootMode") }}
                  </span>
                  <span
                    v-if="mapping.strip_path"
                    class="rounded bg-muted px-2 py-0.5"
                  >
                    {{ t("admin.reverseProxy.stripPath") }}
                  </span>
                </div>
              </TableCell>
              <TableCell class="text-right">
                <div class="flex justify-end gap-1">
                  <Button
                    variant="outline"
                    size="sm"
                    class="mr-2"
                    :class="{
                      'border-border text-muted-foreground hover:text-foreground':
                        model.isDefaultRoute(mapping.path),
                    }"
                    @click="
                      model.isDefaultRoute(mapping.path)
                        ? model.requestClearDefaultRoute(mapping)
                        : model.requestSetDefaultRoute(mapping)
                    "
                  >
                    {{
                      model.isDefaultRoute(mapping.path)
                        ? t("admin.reverseProxy.clearDefaultRoute")
                        : t("admin.reverseProxy.setDefaultRoute")
                    }}
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    @click="model.openEditDialog(mapping)"
                  >
                    {{ t("admin.reverseProxy.edit") }}
                  </Button>
                  <ConfirmDangerPopover
                    :title="t('admin.reverseProxy.deleteConfirmTitle')"
                    :description="
                      t('admin.reverseProxy.deleteDescription', {
                        path: mapping.path,
                      })
                    "
                    :loading="model.removingPath === mapping.path"
                    :disabled="model.removingPath === mapping.path"
                    :on-confirm="() => model.removeMapping(mapping)"
                    content-class="w-60 text-left"
                  >
                    <template #trigger>
                      <Button
                        variant="destructive-outline"
                        size="sm"
                        :disabled="model.removingPath === mapping.path"
                      >
                        {{ t("admin.reverseProxy.delete") }}
                      </Button>
                    </template>
                  </ConfirmDangerPopover>
                </div>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </div>

      <PagedTableFooter
        class="mt-4 rounded-md border"
        :total="model.filteredMappings.length"
        :page="model.currentPage"
        :limit="model.limit"
        :items-per-page="model.parsedLimit"
        @update:page="model.handlePageChange"
        @update:limit="model.handleLimitChange"
      />
    </CardContent>
  </Card>
</template>
