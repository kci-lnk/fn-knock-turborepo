<template>
  <div class="space-y-6">
    <ConfigCollapsibleCard
      title="子域模式配置"
      :configured="isSubdomainModeConfigured"
      :ready="!configStore.isLoading"
      edit-label="编辑配置"
      summary-class="text-xs text-muted-foreground truncate max-w-full"
      expanded-content-class="p-0 sm:p-0"
      actions-class="border-t bg-muted/30 px-4 py-4 sm:px-6 flex flex-col-reverse items-stretch gap-2 rounded-b-lg sm:flex-row sm:items-center sm:justify-end"
    >
      <template #summary>
        <template v-if="savedRootDomain">
          根域名 {{ savedRootDomain }}
          <span v-if="authServiceMapping">
            · 鉴权服务 {{ authServiceMapping.host }}
          </span>
          <span v-else> · 鉴权服务未配置 </span>
        </template>
        <template v-else>还未完成根域名配置</template>
      </template>

      <template #default>
        <div class="divide-y divide-border">
          <div class="p-4 sm:p-6">
            <div class="space-y-1">
              <h3 class="text-base font-semibold">子域模式配置</h3>
              <p class="text-sm text-muted-foreground">
                这里只保留子域模式最常用的配置。你通常只需要填好根域名
              </p>
            </div>
          </div>

          <div class="grid gap-4 p-4 sm:p-6">
            <div class="max-w-xs space-y-2">
              <Label for="root-domain">域名</Label>
              <Input
                id="root-domain"
                v-model="modeForm.root_domain"
                placeholder="example.com"
              />
              <p class="text-xs text-muted-foreground">
                如填写 example.com
                后，后续新增映射时，你只需要填写子域名前缀，系统会自动拼接到这个根域名下面，比如
                fnos.example.com
              </p>
            </div>
            <div class="rounded-lg border px-4 py-3">
              <div
                class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
              >
                <div class="space-y-1">
                  <Label>当前鉴权服务</Label>
                  <div class="text-sm">
                    <template v-if="authServiceMapping">
                      <div class="break-all font-medium">
                        {{ authServiceMapping.host }}
                      </div>
                      <div class="break-all text-muted-foreground">
                        {{ authServiceMapping.target }}
                      </div>
                      <div class="mt-1 text-xs text-muted-foreground">
                        在尚未登录时，会自动跳转到
                        <code>https://{{ authServiceMapping.host }}</code>
                        完成登录。
                      </div>
                    </template>
                    <p v-else class="text-muted-foreground">还没有鉴权服务。</p>
                  </div>
                </div>

                <div class="flex flex-col items-end gap-2">
                  <Badge
                    :variant="authServiceMapping ? 'secondary' : 'outline'"
                  >
                    {{ authServiceMapping ? "已配置" : "未配置" }}
                  </Badge>

                  <ConfirmDangerPopover
                    v-if="authServiceMapping"
                    title="确认删除鉴权服务？"
                    :description="`将删除 ${authServiceMapping.host} 对应的鉴权映射。删除后需要重新添加鉴权服务。`"
                    confirm-text="删除鉴权服务"
                    :loading="isSavingMappings"
                    :disabled="isSavingMappings"
                    :on-confirm="async () => void (await removeAuthService())"
                    content-class="w-72 text-left"
                  >
                    <template #trigger>
                      <Button
                        variant="ghost"
                        size="sm"
                        class="h-auto p-0 text-destructive hover:bg-transparent hover:text-destructive/90"
                        :disabled="isSavingMappings"
                      >
                        删除鉴权服务
                      </Button>
                    </template>
                  </ConfirmDangerPopover>
                </div>
              </div>
            </div>
          </div>
        </div>
      </template>

      <template #actions="{ collapse }">
        <Button variant="outline" @click="collapse">折叠</Button>
        <Button
          variant="outline"
          :disabled="isSavingMode || !isModeDirty"
          @click="resetModeForm"
        >
          放弃更改
        </Button>
        <Button
          :disabled="isSavingMode || !isModeValid || !isModeDirty"
          @click="saveMode"
        >
          <span
            v-if="isSavingMode"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          保存配置
        </Button>
      </template>
    </ConfigCollapsibleCard>

    <Card>
      <CardHeader>
        <CardTitle class="flex items-center justify-between">
          <span>映射管理</span>
          <div class="flex items-center gap-2">
            <DocsLinkButton :href="docsUrls.guides.subdomainProxy" />
            <Button
              v-if="!authServiceMapping"
              :disabled="!canManageNewMappings || isSavingMappings"
              variant="default"
              @click="addAuthService"
            >
              <ShieldCheck class="mr-2 h-4 w-4" />
              添加鉴权服务
            </Button>
            <div v-if="authServiceMapping" class="flex items-center">
              <Button
                :variant="discoverButtonVariant"
                :disabled="!canManageNewMappings || isDiscovering"
                class="rounded-r-none"
                @click="openDiscoverDialog"
              >
                <Search class="mr-2 h-4 w-4" />
                {{ isDiscovering ? "发现中..." : "一键发现" }}
              </Button>
              <DropdownMenu>
                <DropdownMenuTrigger as-child>
                  <Button
                    :variant="discoverButtonVariant"
                    size="icon"
                    :class="[
                      'rounded-l-none border-l px-2',
                      discoverButtonDividerClass,
                    ]"
                  >
                    <ChevronDown class="h-4 w-4" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  <DropdownMenuItem
                    v-if="authServiceMapping"
                    variant="destructive"
                    :disabled="isSavingMappings || isClearingAllSubdomainConfig"
                    @select="openClearAllConfigDialog"
                  >
                    <Trash2 class="mr-2 h-4 w-4" />
                    清空所有配置
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    :disabled="configStore.isLoading"
                    @click="openCreateDialog"
                  >
                    <Plus class="mr-2 h-4 w-4" />
                    添加映射
                  </DropdownMenuItem>
                  <DropdownMenuItem @click="syncRoutes" :disabled="isSyncing">
                    <RefreshCw
                      class="mr-2 h-4 w-4"
                      :class="{ 'animate-spin': isSyncing }"
                    />
                    {{ isSyncing ? "同步中..." : "同步路由" }}
                  </DropdownMenuItem>
                  <DropdownMenuSeparator />
                  <DropdownMenuItem
                    :disabled="isRefreshingTitles || allMappings.length === 0"
                    @select="refreshAllTitles"
                  >
                    <Image
                      class="mr-2 h-4 w-4"
                      :class="{ 'animate-pulse': isRefreshingTitles }"
                    />
                    {{ isRefreshingTitles ? "刷新中..." : "刷新图标和标题" }}
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    :disabled="
                      isExportingBookmarks || visibleMappings.length === 0
                    "
                    @select="exportBookmarks"
                  >
                    <Download
                      class="mr-2 h-4 w-4"
                      :class="{ 'animate-pulse': isExportingBookmarks }"
                    />
                    {{ isExportingBookmarks ? "导出中..." : "导出为书签" }}
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
          </div>
        </CardTitle>
        <CardDescription>
          每个子域名都会直接映射到一个本地 HTTP
          服务。新增时只需要填写子域名前缀，根域名会自动补齐。
        </CardDescription>
      </CardHeader>
      <CardContent class="space-y-4">
        <SearchInput
          v-model="searchQuery"
          placeholder="搜索标题、子域名或目标地址..."
          class="max-w-xs"
        />
        <p
          v-if="visibleMappings.length > 1"
          class="text-xs text-muted-foreground"
        >
          拖动左侧把手可调整显示顺序
        </p>
        <p
          v-if="!savedRootDomain || isRootDomainPendingSave"
          class="text-xs text-amber-600"
        >
          {{
            !savedRootDomain
              ? "请先在上方保存根域名，再添加或发现 Host 映射。"
              : "根域名有未保存的修改，请先保存后再添加或发现 Host 映射。"
          }}
        </p>

        <div class="overflow-hidden rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead class="w-[36px]"></TableHead>
                <TableHead class="w-[22px]">Icon</TableHead>
                <TableHead class="min-w-[12rem]">标题</TableHead>
                <TableHead>域名</TableHead>
                <TableHead>目标</TableHead>
                <TableHead>状态</TableHead>
                <TableHead class="text-right">操作</TableHead>
              </TableRow>
            </TableHeader>
            <VueDraggable
              v-model="draggableVisibleMappings"
              tag="tbody"
              class="[&_tr:last-child]:border-0"
              handle=".mapping-drag-handle"
              ghost-class="bg-muted/60"
              chosen-class="bg-muted/80"
              :animation="180"
              :disabled="isSavingMappings || filteredMappings.length < 2"
              @end="saveMappingOrder"
            >
              <TableRow v-if="filteredMappings.length === 0">
                <TableCell
                  colspan="7"
                  class="py-8 text-center text-muted-foreground"
                >
                  还没有配置任何 Host 映射。
                </TableCell>
              </TableRow>
              <TableRow
                v-for="mapping in draggableVisibleMappings"
                :key="mapping.host"
                class="group"
              >
                <TableCell class="w-[36px]">
                  <button
                    type="button"
                    class="mapping-drag-handle -ml-1 inline-flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition hover:bg-muted hover:text-foreground disabled:pointer-events-none disabled:opacity-40"
                    :disabled="isSavingMappings || filteredMappings.length < 2"
                    aria-label="拖动排序"
                  >
                    <GripVertical class="h-4 w-4" />
                  </button>
                </TableCell>
                <TableCell class="w-[22px]">
                  <img
                    v-if="
                      getMappingFaviconSrc(mapping) && !isFaviconBroken(mapping)
                    "
                    :src="getMappingFaviconSrc(mapping)"
                    :alt="`${getMappingTitleForDisplay(mapping)} favicon`"
                    class="h-4 w-4 object-contain"
                    @error="markFaviconBroken(mapping)"
                  />
                </TableCell>
                <TableCell
                  class="max-w-[16rem] text-sm"
                  :title="getMappingTitleForDisplay(mapping)"
                >
                  <div class="max-w-[16rem]">
                    <InlineCommentEditor
                      :text="getMappingDisplayTitle(mapping)"
                      placeholder="输入展示标题..."
                      empty-text="未获取"
                      :save="
                        (value) => saveMappingTitleOverride(mapping, value)
                      "
                    />
                  </div>
                </TableCell>
                <TableCell class="break-all font-medium">
                  {{ formatHostWithAccessEntryPort(mapping.host) }}
                </TableCell>
                <TableCell>{{ mapping.target }}</TableCell>
                <TableCell class="min-w-[3rem]">
                  <div
                    class="flex min-w-[3rem] flex-wrap items-center gap-2 text-xs text-muted-foreground"
                  >
                    <Badge
                      v-if="isAuthServiceTarget(mapping.target)"
                      variant="default"
                    >
                      鉴权服务
                    </Badge>
                    <ShieldCheck v-if="mapping.use_auth" class="h-3.5 w-3.5" />
                    <Badge v-else variant="secondary">公开访问</Badge>
                    <PanelsTopLeft
                      v-if="mapping.use_auth && !mapping.suppress_toolbar"
                      class="h-3.5 w-3.5"
                    />
                  </div>
                </TableCell>
                <TableCell class="text-right">
                  <div class="flex justify-end gap-2">
                    <Button
                      variant="ghost"
                      size="sm"
                      @click="openEditDialog(mapping)"
                    >
                      编辑
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                      :disabled="isSavingMappings"
                      @click="openDeleteMappingDialog(mapping.host)"
                    >
                      删除
                    </Button>
                  </div>
                </TableCell>
              </TableRow>
            </VueDraggable>
          </Table>
        </div>
      </CardContent>
    </Card>

    <Dialog :open="isDialogOpen" @update:open="handleDialogOpenChange">
      <DialogContent class="sm:max-w-[520px]">
        <DialogHeader>
          <DialogTitle>
            {{ editingHost ? "编辑 Host 映射" : "添加 Host 映射" }}
          </DialogTitle>
          <DialogDescription> 业务域名默认会走统一登录流程 </DialogDescription>
        </DialogHeader>
        <div class="grid gap-4 py-4">
          <div class="space-y-2">
            <div class="flex items-center justify-between gap-3">
              <Label for="mapping-display-title">展示标题</Label>
              <Button
                variant="link"
                size="sm"
                class="h-auto p-0 text-xs"
                :disabled="
                  !canRefreshMappingMetadata || isRefreshingMappingMetadata
                "
                @click="refreshMappingMetadata"
              >
                <RefreshCw
                  v-if="isRefreshingMappingMetadata"
                  class="mr-1 h-3.5 w-3.5 animate-spin"
                />
                {{ isRefreshingMappingMetadata ? "刷新中..." : "刷新标题" }}
              </Button>
            </div>
            <Input
              id="mapping-display-title"
              v-model="mappingForm.title_override"
              placeholder="为空时自动使用抓取到的页面标题"
            />
            <p class="text-xs text-muted-foreground">
              这里只修改当前系统内的展示标题，不会改动目标服务本身。留空时自动使用抓取到的页面标题。
              <span v-if="mappingResolvedTitle">
                当前抓取标题：{{ mappingResolvedTitle }}
              </span>
              <span v-else-if="mappingForm.target.trim()">
                当前还没有抓取到标题，可点击右侧刷新。
              </span>
            </p>
          </div>

          <div class="space-y-2">
            <div
              class="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between"
            >
              <div class="space-y-1">
                <Label for="mapping-subdomain">
                  {{ mappingInputLabel }}
                </Label>
                <p class="text-xs text-muted-foreground">
                  {{ mappingModeDescription }}
                </p>
              </div>
              <div
                class="flex items-center justify-between rounded-md border bg-muted/20 px-3 py-2 sm:w-auto sm:justify-start sm:gap-2 sm:border-0 sm:bg-transparent sm:px-0 sm:py-0"
              >
                <span class="whitespace-nowrap text-xs text-muted-foreground">
                  固定后缀
                </span>
                <Switch
                  id="mapping-use-root-domain"
                  v-model="isUsingRootDomainSuffix"
                  :disabled="!canUseRootDomainSuffix"
                />
              </div>
            </div>
            <template v-if="mappingInputMode === 'subdomain'">
              <div class="flex items-stretch rounded-md border">
                <Input
                  id="mapping-subdomain"
                  v-model="mappingSubdomain"
                  placeholder="redis"
                  class="rounded-none border-0 shadow-none focus-visible:ring-0"
                />
                <div
                  class="flex items-center border-l bg-muted/30 px-3 text-sm text-muted-foreground"
                >
                  .{{ savedRootDomain }}
                </div>
              </div>
              <p class="text-xs text-muted-foreground">
                最终地址：{{ composedPreviewHost || "未填写" }}
              </p>
            </template>
            <template v-else>
              <Input
                id="mapping-subdomain"
                v-model="mappingSubdomain"
                placeholder="auth.other-domain.example"
              />
              <p class="text-xs text-muted-foreground">
                {{ fullHostInputHint }}
              </p>
            </template>
          </div>

          <div class="space-y-2">
            <Label for="mapping-target">目标</Label>
            <ProxyTargetInputField
              v-model="mappingForm.target"
              input-id="mapping-target"
              protocol-id="mapping-target-protocol"
              placeholder="127.0.0.1:5173"
            />
          </div>

          <div
            class="flex items-center justify-between rounded-lg border px-4 py-3"
          >
            <div class="space-y-1">
              <Label for="mapping-auth">要求登录</Label>
              <p class="text-xs text-muted-foreground">
                安全性, 未登录用户会被要求登录才可以访问
              </p>
            </div>
            <Switch
              id="mapping-auth"
              v-model="mappingForm.use_auth"
              :disabled="isMappingAuthService"
            />
          </div>

          <div
            class="flex items-center justify-between rounded-lg border px-4 py-3"
          >
            <div class="space-y-1">
              <Label for="mapping-toolbar">显示小工具</Label>
              <p class="text-xs text-muted-foreground">
                当开启时，在完成登录后，一般在右下角显示一个可以快速切换应用的小图标
              </p>
            </div>
            <Switch id="mapping-toolbar" v-model="showToolbar" />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" @click="closeDialog">取消</Button>
          <Button
            :disabled="!isMappingValid || isSavingMappings"
            @click="saveMapping"
          >
            保存映射
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <Dialog
      :open="isDeleteDialogOpen"
      @update:open="handleDeleteDialogOpenChange"
    >
      <DialogContent class="sm:max-w-[440px]">
        <DialogHeader>
          <DialogTitle>{{ deleteDialogTitle }}</DialogTitle>
          <DialogDescription>
            {{ deleteDialogDescription }}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" @click="closeDeleteDialog">取消</Button>
          <Button
            variant="destructive"
            :disabled="isSavingMappings || isClearingAllSubdomainConfig"
            @click="confirmDelete"
          >
            <span
              v-if="isSavingMappings || isClearingAllSubdomainConfig"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
            ></span>
            {{ deleteDialogConfirmLabel }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <Dialog
      :open="isDiscoverDialogOpen"
      @update:open="handleDiscoverDialogOpenChange"
    >
      <DialogContent class="flex max-h-[85vh] flex-col sm:max-w-[820px]">
        <DialogHeader>
          <div
            class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
          >
            <div class="space-y-1">
              <DialogTitle>一键发现本地服务</DialogTitle>
              <DialogDescription>
                扫描本地端口并生成建议子域名，最终会自动拼接到
                <code>.{{ savedRootDomain }}</code> 下。
              </DialogDescription>
            </div>
            <Button
              class="w-full sm:w-auto"
              variant="outline"
              :disabled="isDiscovering"
              @click="triggerScan"
            >
              <RefreshCw
                class="mr-2 h-4 w-4"
                :class="{ 'animate-spin': isDiscovering }"
              />
              {{ isDiscovering ? "扫描中..." : "刷新服务" }}
            </Button>
          </div>
        </DialogHeader>

        <div class="flex-1 overflow-y-auto overflow-x-hidden py-2">
          <div
            v-if="isDiscovering"
            class="flex flex-col items-center justify-center py-16 space-y-4"
          >
            <RefreshCw class="h-8 w-8 animate-spin text-muted-foreground" />
            <p class="text-sm text-muted-foreground">
              正在探测端口服务，这可能需要几秒钟...
            </p>
          </div>

          <div
            v-else-if="discoveredData && discoveredData.services.length === 0"
            class="text-center py-16 text-muted-foreground"
          >
            {{
              discoveredData.foundServices > 0
                ? "本次扫描到的服务都已添加到 Host 映射中。"
                : "未探测到任何可代理的服务。"
            }}
          </div>

          <div v-else-if="discoveredData" class="rounded-md border">
            <Table class="min-w-[42rem]">
              <TableHeader>
                <TableRow>
                  <TableHead class="w-[50px] text-center">
                    <input
                      type="checkbox"
                      class="h-4 w-4 cursor-pointer"
                      :checked="isAllSelected"
                      @change="onToggleAllDiscoverSelect"
                    />
                  </TableHead>
                  <TableHead class="w-[80px]">端口</TableHead>
                  <TableHead class="w-[100px]">状态</TableHead>
                  <TableHead class="min-w-[10rem]">服务标识</TableHead>
                  <TableHead class="w-[260px] min-w-[18rem]">
                    建议子域名
                  </TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                <TableRow
                  v-for="(svc, index) in discoveredData.services"
                  :key="`${svc.port}-${index}`"
                >
                  <TableCell class="text-center">
                    <input
                      type="checkbox"
                      class="h-4 w-4 cursor-pointer"
                      :value="svc"
                      v-model="selectedServices"
                    />
                  </TableCell>
                  <TableCell class="font-medium">{{ svc.port }}</TableCell>
                  <TableCell>
                    <span
                      v-if="svc.httpStatus === 401"
                      class="text-amber-600 bg-amber-500/10 text-xs px-2 py-0.5 rounded"
                    >
                      需认证
                    </span>
                    <span
                      v-else
                      class="text-green-600 bg-green-500/10 text-xs px-2 py-0.5 rounded"
                    >
                      {{ svc.httpStatus }}
                    </span>
                  </TableCell>
                  <TableCell class="min-w-[10rem] text-sm">
                    {{ svc.detail.label || svc.detail.name || "未知服务" }}
                  </TableCell>
                  <TableCell class="min-w-[18rem]">
                    <div
                      class="flex min-w-[18rem] items-stretch rounded-md border"
                    >
                      <Input
                        v-model="svc.suggestedSubdomain"
                        placeholder="service"
                        class="h-8 rounded-none border-0 text-sm shadow-none focus-visible:ring-0"
                        :class="{
                          'border-destructive focus-visible:ring-destructive':
                            selectedServices.includes(svc) &&
                            !svc.suggestedSubdomain.trim(),
                        }"
                      />
                      <div
                        class="flex shrink-0 items-center border-l bg-muted/30 px-3 text-xs text-muted-foreground"
                      >
                        .{{ savedRootDomain }}
                      </div>
                    </div>
                  </TableCell>
                </TableRow>
              </TableBody>
            </Table>
          </div>
        </div>

        <DialogFooter class="sm:justify-between items-center mt-2">
          <span class="text-sm text-muted-foreground">
            <template v-if="discoveredData">
              已扫描 {{ discoveredData.totalPortsScanned }} 个端口，选中
              {{ selectedServices.length }} /
              {{ discoveredData.services.length }}
              项
            </template>
          </span>
          <div class="space-x-2">
            <Button variant="outline" @click="dismissDiscoverDialog">
              取消
            </Button>
            <Button
              :disabled="
                isDiscovering ||
                selectedServices.length === 0 ||
                !isDiscoverSelectionValid ||
                isSavingMappings
              "
              @click="saveDiscoveredServices"
            >
              添加选中项
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import {
  ChevronDown,
  Download,
  GripVertical,
  Image,
  PanelsTopLeft,
  Plus,
  RefreshCw,
  Search,
  ShieldCheck,
  Trash2,
} from "lucide-vue-next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { VueDraggable } from "vue-draggable-plus";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import InlineCommentEditor from "@admin-shared/components/InlineCommentEditor.vue";
import ProxyTargetInputField from "@admin-shared/components/common/ProxyTargetInputField.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { toast } from "@admin-shared/utils/toast";
import { useDiscoverServicesSelection } from "@admin-shared/composables/useDiscoverServicesSelection";
import { extractPortFromTarget } from "@admin-shared/utils/extractPortFromTarget";
import { useConfigStore } from "../store/config";
import {
  ConfigAPI,
  ScanAPI,
  SystemAPI,
  type DiscoveredServiceInfo,
  type ScanDiscoverResponse,
} from "../lib/api";
import { docsUrls } from "../lib/docs";
import type { HostMapping, SubdomainModeConfig } from "../types";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { downloadBlob } from "@admin-shared/utils/downloadBlob";

type MappingInputMode = "subdomain" | "full_host";

type DiscoveredHostService = DiscoveredServiceInfo & {
  suggestedSubdomain: string;
};

type DiscoveredHostResponse = Omit<ScanDiscoverResponse, "services"> & {
  services: DiscoveredHostService[];
};

type DeleteDialogState =
  | {
      kind: "auth_service";
      host: string;
    }
  | {
      kind: "clear_all";
      step: 1 | 2;
    }
  | {
      kind: "mapping";
      host: string;
    };

const configStore = useConfigStore();

const normalizeHostLike = (value: string): string =>
  value
    .trim()
    .toLowerCase()
    .replace(/^[a-z]+:\/\//i, "")
    .replace(/\/.*$/, "")
    .replace(/\.+$/, "");

const normalizeRootDomainValue = (value: string): string =>
  normalizeHostLike(value);

const stripRootDomainSuffix = (value: string, rootDomain: string): string => {
  const normalized = normalizeHostLike(value);
  const normalizedRoot = normalizeRootDomainValue(rootDomain);
  if (!normalizedRoot) return normalized;
  if (normalized === normalizedRoot) return "";
  if (normalized.endsWith(`.${normalizedRoot}`)) {
    return normalized.slice(0, -1 * (normalizedRoot.length + 1));
  }
  return normalized;
};

const composeHostFromSubdomain = (
  subdomain: string,
  rootDomain: string,
): string => {
  const normalizedRoot = normalizeRootDomainValue(rootDomain);
  const normalizedSubdomain = stripRootDomainSuffix(subdomain, normalizedRoot);
  if (!normalizedRoot || !normalizedSubdomain) return "";
  return `${normalizedSubdomain}.${normalizedRoot}`;
};

const extractSubdomainFromHost = (
  value: string,
  rootDomain: string,
): string | null => {
  const normalizedHost = normalizeHostLike(value);
  const normalizedRoot = normalizeRootDomainValue(rootDomain);
  if (!normalizedHost || !normalizedRoot) return null;
  if (!normalizedHost.endsWith(`.${normalizedRoot}`)) return null;

  const subdomain = normalizedHost.slice(0, -1 * (normalizedRoot.length + 1));
  return subdomain || null;
};

const resolveMappingEditorState = (
  host: string,
  rootDomain: string,
): { mode: MappingInputMode; value: string } => {
  const subdomain = extractSubdomainFromHost(host, rootDomain);
  if (subdomain) {
    return {
      mode: "subdomain",
      value: subdomain,
    };
  }

  return {
    mode: "full_host",
    value: normalizeHostLike(host),
  };
};

const buildSuggestedSubdomain = (service: DiscoveredServiceInfo): string => {
  const candidates = [
    service.detail.rule.path,
    service.detail.label,
    service.detail.name,
    `app-${service.port}`,
  ];

  for (const candidate of candidates) {
    const normalized = String(candidate ?? "")
      .trim()
      .replace(/^\/+|\/+$/g, "")
      .replace(/\//g, "-")
      .replace(/\s+/g, "-")
      .replace(/[^a-zA-Z0-9-]+/g, "-")
      .replace(/-+/g, "-")
      .replace(/^-+|-+$/g, "")
      .toLowerCase();

    if (normalized) return normalized;
  }

  return `app-${service.port}`;
};

const parseTargetPort = (target: string): number | null => {
  const normalizedTarget = target.trim();
  if (!normalizedTarget) return null;

  const explicitPort = extractPortFromTarget(normalizedTarget);
  if (
    explicitPort !== null &&
    Number.isFinite(explicitPort) &&
    explicitPort > 0
  ) {
    return explicitPort;
  }

  try {
    const parsed = new URL(normalizedTarget);
    if (parsed.protocol === "https:") return 443;
    if (parsed.protocol === "http:") return 80;
  } catch {
    // ignore
  }

  return null;
};

const createDefaultModeForm = (): SubdomainModeConfig => ({
  root_domain: "",
  auth_host: "",
  auth_target: "http://localhost:7997",
  cookie_domain: "",
  public_auth_base_url: "",
  auth_cache_ttl_seconds: 1,
  auth_cache_unauthorized_ttl_seconds: 1,
  default_access_mode: "login_first",
  auto_add_whitelist_on_login: true,
  passkey_rp_mode: "auth_host",
  passkey_rp_id: "",
});

const DEFAULT_AUTH_SUBDOMAIN = "auth";
const DEFAULT_ACCESS_MODE: HostMapping["access_mode"] = "login_first";

const createDefaultMapping = (): HostMapping => ({
  host: "",
  target: "",
  use_auth: true,
  access_mode: DEFAULT_ACCESS_MODE,
  suppress_toolbar: false,
  preserve_host: false,
  service_role: "app",
  title: "",
  title_override: "",
  favicon: "",
});

const searchQuery = ref("");
const isDialogOpen = ref(false);
const deleteDialogState = ref<DeleteDialogState | null>(null);
const editingHost = ref<string | null>(null);
const mappingInputMode = ref<MappingInputMode>("subdomain");
const mappingSubdomain = ref("");
const accessEntryPort = ref("7999");
const brokenFaviconKeys = ref(new Set<string>());
const draggableVisibleMappings = ref<HostMapping[]>([]);
const mappingMetadataTarget = ref("");
const modeForm = reactive<SubdomainModeConfig>(createDefaultModeForm());
const mappingForm = reactive<HostMapping>(createDefaultMapping());

const currentModeConfig = computed(
  () => configStore.config?.subdomain_mode ?? createDefaultModeForm(),
);
const authServicePort = computed(
  () => parseTargetPort(currentModeConfig.value.auth_target) ?? 7997,
);
const isAuthServiceTarget = (target: string): boolean =>
  parseTargetPort(target) === authServicePort.value;
const savedRootDomain = computed(() =>
  normalizeRootDomainValue(currentModeConfig.value.root_domain),
);
const currentDraftRootDomain = computed(() =>
  normalizeRootDomainValue(modeForm.root_domain),
);
const isRootDomainPendingSave = computed(
  () => currentDraftRootDomain.value !== savedRootDomain.value,
);
const canUseRootDomainSuffix = computed(
  () => Boolean(savedRootDomain.value) && !isRootDomainPendingSave.value,
);
const canManageNewMappings = computed(
  () => Boolean(savedRootDomain.value) && !isRootDomainPendingSave.value,
);
const allMappings = computed(() => configStore.config?.host_mappings ?? []);
const existingMappingPorts = computed(() => {
  const ports = new Set<number>();

  for (const mapping of allMappings.value) {
    const port = extractPortFromTarget(mapping.target);
    if (port !== null) {
      ports.add(port);
    }
  }

  return ports;
});
const authServiceMapping = computed(
  () =>
    allMappings.value.find((mapping) => isAuthServiceTarget(mapping.target)) ??
    null,
);
const discoverButtonVariant = computed(() =>
  authServiceMapping.value ? "default" : "secondary",
);
const discoverButtonDividerClass = computed(() =>
  authServiceMapping.value
    ? "border-primary-foreground/20"
    : "border-border/70",
);
const isSubdomainModeConfigured = computed(() => {
  const config = currentModeConfig.value;
  return Boolean(
    savedRootDomain.value ||
    normalizeHostLike(config.auth_host) ||
    authServiceMapping.value,
  );
});
const isMappingAuthService = computed(() =>
  isAuthServiceTarget(mappingForm.target),
);
const mappingResolvedTitle = computed(() =>
  mappingMetadataTarget.value === mappingForm.target.trim()
    ? mappingForm.title.trim()
    : "",
);
const canRefreshMappingMetadata = computed(() => {
  const target = mappingForm.target.trim();
  if (!target) return false;

  try {
    const parsed = new URL(target);
    return parsed.protocol === "http:" || parsed.protocol === "https:";
  } catch {
    return false;
  }
});
const showToolbar = computed({
  get: () => !mappingForm.suppress_toolbar,
  set: (value: boolean) => {
    mappingForm.suppress_toolbar = !value;
  },
});
const isDeleteDialogOpen = computed(() => deleteDialogState.value !== null);
const deleteDialogTitle = computed(() => {
  const target = deleteDialogState.value;
  if (!target) return "";

  if (target.kind === "auth_service") {
    return "确认删除鉴权服务？";
  }

  if (target.kind === "clear_all") {
    return target.step === 1 ? "确认清空所有配置？" : "请再次确认清空所有配置";
  }

  return "确认删除 Host 映射？";
});
const deleteDialogDescription = computed(() => {
  const target = deleteDialogState.value;
  if (!target) return "";

  if (target.kind === "auth_service") {
    return `将删除 ${target.host} 对应的鉴权映射。删除后需要重新添加鉴权服务`;
  }

  if (target.kind === "clear_all") {
    const mappingsCount = allMappings.value.length;
    return target.step === 1
      ? `这会删除鉴权服务和 ${mappingsCount} 条 Host 映射。根域名及其他子域模式配置会保留。点击“继续确认”后，还需要再确认一次。`
      : "这是最后一次确认。确认后会立即清空鉴权服务和全部子域映射，但不会修改子域模式配置，此操作不可恢复。";
  }

  return `您即将删除 Host 映射 ${target.host}，此操作不可逆转。`;
});
const deleteDialogConfirmLabel = computed(() => {
  const target = deleteDialogState.value;
  if (!target) return "确认";

  if (target.kind === "auth_service") {
    return "删除鉴权服务";
  }

  if (target.kind === "clear_all") {
    return target.step === 1 ? "继续确认" : "确认清空";
  }

  return "删除映射";
});
const isUsingRootDomainSuffix = computed({
  get: () =>
    mappingInputMode.value === "subdomain" && canUseRootDomainSuffix.value,
  set: (value: boolean) => {
    setMappingInputMode(value ? "subdomain" : "full_host");
  },
});
const mappingModeDescription = computed(() => {
  if (mappingInputMode.value === "subdomain" && canUseRootDomainSuffix.value) {
    return `当前使用固定后缀 .${savedRootDomain.value}。`;
  }

  if (canUseRootDomainSuffix.value) {
    return `当前为自定义完整域名，不会自动拼接 .${savedRootDomain.value}。`;
  }

  if (!savedRootDomain.value) {
    return "当前没有已保存的固定后缀，只能输入完整域名。";
  }

  return "根域名有未保存修改，暂时只能输入完整域名。";
});
const mappingInputLabel = computed(() =>
  mappingInputMode.value === "subdomain" ? "子域名前缀" : "完整域名",
);
const fullHostInputHint = computed(() => {
  if (canUseRootDomainSuffix.value) {
    return `按输入的 Host 直接保存，不自动拼接 .${savedRootDomain.value}。`;
  }

  return "按输入的 Host 直接保存。";
});
const composedPreviewHost = computed(() => {
  if (mappingInputMode.value === "full_host") {
    return normalizeHostLike(mappingSubdomain.value) || "";
  }
  return composeHostFromSubdomain(
    mappingSubdomain.value,
    savedRootDomain.value,
  );
});
const displayAccessEntryPort = computed(
  () => accessEntryPort.value.trim() || "7999",
);
const shouldOmitAccessEntryPort = computed(() => {
  const parsedPort = Number.parseInt(displayAccessEntryPort.value, 10);
  return parsedPort === 80 || parsedPort === 443;
});
const formatHostWithAccessEntryPort = (host: string): string =>
  shouldOmitAccessEntryPort.value
    ? host
    : `${host}:${displayAccessEntryPort.value}`;
const buildBookmarkExportFilename = (rootDomain: string): string => {
  const normalizedRootDomain = rootDomain
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9.-]+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-+|-+$/g, "");

  return normalizedRootDomain
    ? `fn-knock-bookmarks-${normalizedRootDomain}.html`
    : "fn-knock-bookmarks.html";
};
const getMappingDisplayTitle = (mapping: HostMapping): string =>
  mapping.title_override.trim() || mapping.title.trim();
const getMappingTitleForDisplay = (mapping: HostMapping): string =>
  getMappingDisplayTitle(mapping) || "未获取";
const getMappingFaviconSrc = (mapping: HostMapping): string => {
  const favicon = mapping.favicon.trim();
  return /^data:image\//i.test(favicon) ? favicon : "";
};
const getFaviconKey = (mapping: HostMapping): string =>
  `${mapping.host}::${getMappingFaviconSrc(mapping)}`;
const isFaviconBroken = (mapping: HostMapping): boolean =>
  brokenFaviconKeys.value.has(getFaviconKey(mapping));
const markFaviconBroken = (mapping: HostMapping) => {
  const next = new Set(brokenFaviconKeys.value);
  next.add(getFaviconKey(mapping));
  brokenFaviconKeys.value = next;
};
const visibleMappings = computed(() =>
  allMappings.value.filter((mapping) => !isAuthServiceTarget(mapping.target)),
);

const filteredMappings = computed(() => {
  const query = searchQuery.value.trim().toLowerCase();
  if (!query) return visibleMappings.value;
  return visibleMappings.value.filter(
    (mapping) =>
      getMappingDisplayTitle(mapping).toLowerCase().includes(query) ||
      formatHostWithAccessEntryPort(mapping.host)
        .toLowerCase()
        .includes(query) ||
      mapping.host.toLowerCase().includes(query) ||
      mapping.target.toLowerCase().includes(query),
  );
});

const syncDraggableVisibleMappings = () => {
  draggableVisibleMappings.value = [...filteredMappings.value];
};

const isModeValid = computed(() => true);

const isModeDirty = computed(
  () => JSON.stringify(modeForm) !== JSON.stringify(currentModeConfig.value),
);

const resolveDefaultAuthServiceTarget = (): string => {
  const configuredTarget =
    modeForm.auth_target?.trim() ||
    currentModeConfig.value.auth_target?.trim() ||
    createDefaultModeForm().auth_target;

  try {
    const parsed = new URL(configuredTarget);
    const port =
      parsed.port ||
      (parsed.protocol === "https:"
        ? "443"
        : parsed.protocol === "http:"
          ? "80"
          : "");

    if (!port) return configuredTarget;

    const normalized = new URL(`http://localhost:${port}`);
    normalized.pathname =
      parsed.pathname && parsed.pathname !== "/" ? parsed.pathname : "/";
    normalized.search = parsed.search;
    normalized.hash = parsed.hash;
    return normalized
      .toString()
      .replace(/\/$/, normalized.pathname === "/" ? "" : normalized.pathname);
  } catch {
    return configuredTarget || createDefaultModeForm().auth_target;
  }
};

const isMappingValid = computed(() => {
  const host =
    mappingInputMode.value === "full_host"
      ? normalizeHostLike(mappingSubdomain.value)
      : composeHostFromSubdomain(mappingSubdomain.value, savedRootDomain.value);
  const target = mappingForm.target.trim();

  if (!host || !target) return false;
  if (mappingInputMode.value === "subdomain" && !canUseRootDomainSuffix.value) {
    return false;
  }

  try {
    const parsed = new URL(target);
    return parsed.protocol === "http:" || parsed.protocol === "https:";
  } catch {
    return false;
  }
});

const { isPending: isSavingMode, run: runSaveMode } = useAsyncAction({
  onError: (error) => {
    toast.error("保存失败", {
      description: extractErrorMessage(error, "子域模式配置保存失败"),
    });
  },
});

const { isPending: isSavingMappings, run: runSaveMappings } = useAsyncAction({
  onError: (error) => {
    toast.error("保存失败", {
      description: extractErrorMessage(error, "Host 映射保存失败"),
    });
  },
});

const {
  isPending: isClearingAllSubdomainConfig,
  run: runClearAllSubdomainConfig,
} = useAsyncAction({
  onError: (error) => {
    toast.error("清空失败", {
      description: extractErrorMessage(error, "清空子域配置失败"),
    });
  },
});

const { isPending: isSyncing, run: runSyncRoutes } = useAsyncAction({
  onError: (error) => {
    toast.error("同步失败", {
      description: extractErrorMessage(error, "同步网关配置失败"),
    });
  },
});

const { isPending: isRefreshingTitles, run: runRefreshTitles } = useAsyncAction(
  {
    onError: (error) => {
      toast.error("刷新失败", {
        description: extractErrorMessage(error, "批量刷新图标和标题失败"),
      });
    },
  },
);

const {
  isPending: isRefreshingMappingMetadata,
  run: runRefreshMappingMetadata,
} = useAsyncAction({
  onError: (error) => {
    toast.error("刷新失败", {
      description: extractErrorMessage(error, "目标地址标题刷新失败"),
    });
  },
});

const { isPending: isExportingBookmarks, run: runExportBookmarks } =
  useAsyncAction({
    onError: (error) => {
      toast.error("导出失败", {
        description: extractErrorMessage(error, "导出书签失败"),
      });
    },
  });

const { isPending: isDiscovering, run: runDiscoverServices } = useAsyncAction({
  onError: (error) => {
    toast.error("发现失败", {
      description: extractErrorMessage(error, "本地服务扫描失败"),
    });
  },
});

const applyModeForm = (next: SubdomainModeConfig) => {
  modeForm.root_domain = next.root_domain;
  modeForm.auth_host = next.auth_host;
  modeForm.auth_target = next.auth_target;
  modeForm.cookie_domain = next.cookie_domain;
  modeForm.public_auth_base_url = next.public_auth_base_url;
  modeForm.auth_cache_ttl_seconds = next.auth_cache_ttl_seconds;
  modeForm.auth_cache_unauthorized_ttl_seconds =
    next.auth_cache_unauthorized_ttl_seconds;
  modeForm.default_access_mode = next.default_access_mode;
  modeForm.auto_add_whitelist_on_login = next.auto_add_whitelist_on_login;
  modeForm.passkey_rp_mode = next.passkey_rp_mode;
  modeForm.passkey_rp_id = next.passkey_rp_id || "";
};

watch(
  () => configStore.config?.subdomain_mode,
  (next) => {
    if (next) {
      applyModeForm(next);
    }
  },
  { immediate: true },
);

watch(
  filteredMappings,
  () => {
    syncDraggableVisibleMappings();
  },
  { immediate: true },
);

const {
  open: isDiscoverDialogOpen,
  discoveredData,
  selectedServices,
  isAllSelected,
  isSelectionValid: isDiscoverSelectionValid,
  setAllSelected,
  resetSelection,
  setDiscoveredData,
  openDialog: openDiscoverDialogState,
  closeDialog: closeDiscoverDialog,
} = useDiscoverServicesSelection<DiscoveredHostService, DiscoveredHostResponse>(
  {
    getPath: (service) => service.suggestedSubdomain,
  },
);

onMounted(async () => {
  if (!configStore.config) {
    await configStore.loadConfig();
  }
  void loadAccessEntryPort();
});

async function loadAccessEntryPort() {
  try {
    const info = await SystemAPI.getAccessEntry();
    accessEntryPort.value = info.port.trim() || "7999";
  } catch (error) {
    console.warn("load access entry port failed:", error);
  }
}

function resetModeForm() {
  applyModeForm(currentModeConfig.value);
}

function setMappingInputMode(nextMode: MappingInputMode) {
  if (nextMode === "subdomain" && !canUseRootDomainSuffix.value) {
    mappingInputMode.value = "full_host";
    return;
  }

  if (nextMode === mappingInputMode.value) return;

  const currentValue = mappingSubdomain.value;
  if (nextMode === "full_host") {
    mappingSubdomain.value =
      mappingInputMode.value === "subdomain"
        ? composeHostFromSubdomain(currentValue, savedRootDomain.value) ||
          normalizeHostLike(currentValue)
        : normalizeHostLike(currentValue);
    mappingInputMode.value = "full_host";
    return;
  }

  const extractedSubdomain = extractSubdomainFromHost(
    currentValue,
    savedRootDomain.value,
  );

  mappingInputMode.value = "subdomain";
  mappingSubdomain.value = extractedSubdomain ?? "";

  if (currentValue.trim() && !extractedSubdomain) {
    toast.info("已切换为固定后缀模式", {
      description: `当前完整域名不在 .${savedRootDomain.value} 下，请重新填写子域名前缀。`,
    });
  }
}

async function saveMode() {
  if (!isModeValid.value || !isModeDirty.value) return;
  await runSaveMode(async () => {
    const result = await configStore.saveSubdomainMode({
      ...modeForm,
      root_domain: modeForm.root_domain.trim().toLowerCase(),
      auth_host: modeForm.auth_host.trim().toLowerCase(),
      auth_target: modeForm.auth_target.trim(),
      cookie_domain: modeForm.cookie_domain.trim(),
      public_auth_base_url: modeForm.public_auth_base_url.trim(),
      auth_cache_ttl_seconds: Math.max(
        0,
        Math.floor(Number(modeForm.auth_cache_ttl_seconds) || 0),
      ),
      auth_cache_unauthorized_ttl_seconds: Math.max(
        0,
        Math.floor(Number(modeForm.auth_cache_unauthorized_ttl_seconds) || 0),
      ),
      passkey_rp_id: (modeForm.passkey_rp_id || "").trim().toLowerCase(),
    });
    toast.success("子域模式配置已保存");
    if (result?.ssl_auto_selection?.message) {
      if (result.ssl_auto_selection.applied) {
        toast.success(result.ssl_auto_selection.message, {
          description: result.ssl_auto_selection.label
            ? `已切换到证书：${result.ssl_auto_selection.label}`
            : undefined,
        });
      } else {
        toast.error("SSL 自动切换未完成", {
          description: result.ssl_auto_selection.message,
        });
      }
    }
  });
}

function openCreateDialog() {
  editingHost.value = null;
  mappingInputMode.value = canUseRootDomainSuffix.value
    ? "subdomain"
    : "full_host";
  mappingSubdomain.value = "";
  mappingMetadataTarget.value = "";
  Object.assign(mappingForm, createDefaultMapping());
  isDialogOpen.value = true;
}

function openEditDialog(mapping: HostMapping) {
  editingHost.value = mapping.host;
  const editorState = resolveMappingEditorState(
    mapping.host,
    canUseRootDomainSuffix.value ? savedRootDomain.value : "",
  );
  mappingInputMode.value = editorState.mode;
  mappingSubdomain.value = editorState.value;

  Object.assign(mappingForm, { ...mapping });
  mappingMetadataTarget.value = mapping.target.trim();
  isDialogOpen.value = true;
}

function closeDialog() {
  isDialogOpen.value = false;
  editingHost.value = null;
  mappingInputMode.value = canUseRootDomainSuffix.value
    ? "subdomain"
    : "full_host";
  mappingSubdomain.value = "";
  mappingMetadataTarget.value = "";
  Object.assign(mappingForm, createDefaultMapping());
}

function closeDeleteDialog() {
  deleteDialogState.value = null;
}

function handleDialogOpenChange(nextOpen: boolean) {
  if (!nextOpen) {
    closeDialog();
  }
}

function handleDeleteDialogOpenChange(nextOpen: boolean) {
  if (!nextOpen) {
    closeDeleteDialog();
  }
}

function normalizeMapping(input: HostMapping): HostMapping {
  const normalizedTarget = input.target.trim();
  const hasFreshMetadata = mappingMetadataTarget.value === normalizedTarget;
  const serviceRole = isAuthServiceTarget(normalizedTarget) ? "auth" : "app";
  const host =
    mappingInputMode.value === "full_host"
      ? normalizeHostLike(mappingSubdomain.value)
      : composeHostFromSubdomain(mappingSubdomain.value, savedRootDomain.value);

  return {
    host,
    target: normalizedTarget,
    use_auth: serviceRole === "auth" ? false : input.use_auth,
    access_mode:
      serviceRole === "auth"
        ? DEFAULT_ACCESS_MODE
        : input.access_mode || DEFAULT_ACCESS_MODE,
    suppress_toolbar: serviceRole === "auth" ? false : input.suppress_toolbar,
    preserve_host: input.preserve_host === true,
    service_role: serviceRole,
    title: hasFreshMetadata ? input.title.trim() : "",
    title_override: input.title_override.trim(),
    favicon: hasFreshMetadata ? input.favicon.trim() : "",
  };
}

const hasSameMappingOrder = (left: HostMapping[], right: HostMapping[]) =>
  left.length === right.length &&
  left.every((mapping, index) => mapping.host === right[index]?.host);

const mergeFilteredMappingsOrder = (
  nextFiltered: HostMapping[],
): HostMapping[] => {
  const filteredHostSet = new Set(
    filteredMappings.value.map((item) => item.host),
  );
  let nextFilteredIndex = 0;
  const nextVisible = visibleMappings.value.map((mapping) =>
    filteredHostSet.has(mapping.host)
      ? (nextFiltered[nextFilteredIndex++] ?? mapping)
      : mapping,
  );

  let nextVisibleIndex = 0;
  return allMappings.value.map((mapping) =>
    isAuthServiceTarget(mapping.target)
      ? mapping
      : (nextVisible[nextVisibleIndex++] ?? mapping),
  );
};

async function saveMappingOrder() {
  const next = mergeFilteredMappingsOrder(draggableVisibleMappings.value);
  if (hasSameMappingOrder(next, allMappings.value)) {
    syncDraggableVisibleMappings();
    return;
  }

  const saved = await runSaveMappings(async () => {
    await configStore.saveHostMappings(next);
    toast.success("子域映射顺序已更新");
    return true;
  });

  if (saved !== true) {
    syncDraggableVisibleMappings();
  }
}

async function saveMappingTitleOverride(mapping: HostMapping, value: string) {
  const nextTitleOverride = value.trim();
  const currentTitleOverride = mapping.title_override.trim();
  const currentFetchedTitle = mapping.title.trim();
  if (
    nextTitleOverride === currentTitleOverride ||
    (!currentTitleOverride &&
      (nextTitleOverride === currentFetchedTitle || !nextTitleOverride))
  ) {
    return;
  }

  if (isSavingMappings.value) {
    throw new Error("当前有其他映射正在保存，请稍后重试");
  }

  try {
    await configStore.saveHostMappings(
      allMappings.value.map((item) =>
        item.host === mapping.host
          ? { ...item, title_override: nextTitleOverride }
          : item,
      ),
    );
    toast.success("展示标题已更新");
  } catch (error) {
    toast.error("保存失败", {
      description: extractErrorMessage(error, "展示标题保存失败"),
    });
    throw error;
  }
}

async function refreshMappingMetadata() {
  if (!canRefreshMappingMetadata.value) return;

  await runRefreshMappingMetadata(
    () => ConfigAPI.fetchHostMappingMetadata(mappingForm.target.trim()),
    {
      onSuccess: (metadata) => {
        mappingMetadataTarget.value = mappingForm.target.trim();
        mappingForm.title = metadata.title.trim();
        mappingForm.favicon = metadata.favicon.trim();
        brokenFaviconKeys.value = new Set();
        toast.success("目标地址标题已刷新", {
          description: metadata.title.trim()
            ? `当前抓取标题：${metadata.title.trim()}`
            : "目标地址未返回可用标题，已仅更新抓取结果。",
        });
      },
    },
  );
}

async function addAuthService() {
  if (!canManageNewMappings.value) {
    toast.error("暂时无法添加鉴权服务", {
      description: !savedRootDomain.value
        ? "请先保存根域名配置。"
        : "根域名有未保存修改，请先保存后再添加鉴权服务。",
    });
    return;
  }

  if (authServiceMapping.value) {
    toast.error("鉴权服务已存在", {
      description: `当前已配置 ${authServiceMapping.value.host} 作为鉴权服务。`,
    });
    return;
  }

  const host = composeHostFromSubdomain(
    DEFAULT_AUTH_SUBDOMAIN,
    savedRootDomain.value,
  );
  const target = resolveDefaultAuthServiceTarget();

  if (!host) {
    toast.error("默认鉴权服务生成失败", {
      description: "请先确认根域名已正确保存。",
    });
    return;
  }

  const duplicateHost = allMappings.value.find((item) => item.host === host);
  if (duplicateHost) {
    toast.error("默认鉴权子域已存在", {
      description: `${host} 已存在，请将该映射的 Target 调整到鉴权端口。`,
    });
    return;
  }

  await runSaveMappings(async () => {
    await configStore.saveHostMappings([
      ...allMappings.value,
      {
        host,
        target,
        use_auth: false,
        access_mode: DEFAULT_ACCESS_MODE,
        suppress_toolbar: false,
        preserve_host: false,
        service_role: "auth",
        title: "",
        title_override: "",
        favicon: "",
      },
    ]);

    toast.success("鉴权服务已添加", {
      description: `${host} -> ${target}`,
    });
  });
}

function openClearAllConfigDialog() {
  if (allMappings.value.length === 0) {
    toast.error("当前没有可清空的子域映射");
    return;
  }

  deleteDialogState.value = {
    kind: "clear_all",
    step: 1,
  };
}

function openDeleteMappingDialog(host: string) {
  deleteDialogState.value = {
    kind: "mapping",
    host,
  };
}

async function removeAuthService(): Promise<boolean> {
  if (!authServiceMapping.value) {
    toast.error("当前没有鉴权服务");
    return false;
  }

  const authHost = authServiceMapping.value.host;

  const removed = await runSaveMappings(async () => {
    await configStore.saveHostMappings(
      allMappings.value.filter((item) => !isAuthServiceTarget(item.target)),
    );

    toast.success("鉴权服务已删除", {
      description: authHost,
    });

    return true;
  });

  return removed === true;
}

async function clearAllSubdomainConfig(): Promise<boolean> {
  const mappingsCount = allMappings.value.length;

  const cleared = await runClearAllSubdomainConfig(async () => {
    await configStore.saveHostMappings([]);

    toast.success("鉴权服务和子域映射已清空", {
      description:
        mappingsCount > 0
          ? `已清理 ${mappingsCount} 条 Host 映射，子域模式配置保持不变。`
          : "子域模式配置保持不变。",
    });

    return true;
  });

  return cleared === true;
}

async function saveMapping() {
  if (!isMappingValid.value) return;

  const normalized = normalizeMapping(mappingForm);
  const duplicateHost = allMappings.value.find(
    (item) => item.host === normalized.host && item.host !== editingHost.value,
  );
  if (duplicateHost) {
    toast.error("Host 已存在", {
      description: `${normalized.host} 已经配置过映射。`,
    });
    return;
  }

  const duplicateAuthService = allMappings.value.find(
    (item) =>
      isAuthServiceTarget(item.target) && item.host !== editingHost.value,
  );
  if (normalized.service_role === "auth" && duplicateAuthService) {
    toast.error("鉴权服务已存在", {
      description: `当前已配置 ${duplicateAuthService.host} 作为鉴权服务，请先调整那条映射。`,
    });
    return;
  }

  await runSaveMappings(async () => {
    const next = [...allMappings.value];
    const index = editingHost.value
      ? next.findIndex((item) => item.host === editingHost.value)
      : -1;

    if (index >= 0) {
      next[index] = normalized;
    } else {
      next.push(normalized);
    }

    await configStore.saveHostMappings(next);
    toast.success(index >= 0 ? "Host 映射已更新" : "Host 映射已添加");
    closeDialog();
  });
}

async function removeMapping(host: string): Promise<boolean> {
  const target = allMappings.value.find((item) => item.host === host);
  if (!target) return false;

  const removed = await runSaveMappings(async () => {
    await configStore.saveHostMappings(
      allMappings.value.filter((item) => item.host !== host),
    );
    toast.success("Host 映射已删除");

    return true;
  });

  return removed === true;
}

async function confirmDelete() {
  const target = deleteDialogState.value;
  if (!target) return;

  if (target.kind === "clear_all") {
    if (target.step === 1) {
      deleteDialogState.value = {
        kind: "clear_all",
        step: 2,
      };
      return;
    }

    const cleared = await clearAllSubdomainConfig();
    if (cleared) {
      closeDeleteDialog();
    }
    return;
  }

  const removed =
    target.kind === "auth_service"
      ? await removeAuthService()
      : await removeMapping(target.host);

  if (removed) {
    closeDeleteDialog();
  }
}

const onToggleAllDiscoverSelect = (event: Event) => {
  const checked = (event.target as HTMLInputElement).checked;
  setAllSelected(checked);
};

function dismissDiscoverDialog() {
  setDiscoveredData(null);
  closeDiscoverDialog(true);
}

const handleDiscoverDialogOpenChange = (nextOpen: boolean) => {
  if (!nextOpen) {
    dismissDiscoverDialog();
  }
};

function openDiscoverDialog() {
  if (!canManageNewMappings.value) {
    toast.error("暂时无法发现服务", {
      description: !savedRootDomain.value
        ? "请先保存根域名配置。"
        : "根域名有未保存修改，请先保存后再发现服务。",
    });
    return;
  }

  openDiscoverDialogState();
  if (!discoveredData.value) {
    void triggerScan();
  }
}

async function triggerScan() {
  resetSelection();
  await runDiscoverServices(() => ScanAPI.discover(), {
    onSuccess: (data) => {
      const nextData: DiscoveredHostResponse = {
        ...data,
        services: data.services
          .map((service) => ({
            ...service,
            detail: {
              ...service.detail,
              rule: { ...service.detail.rule },
            },
            suggestedSubdomain: buildSuggestedSubdomain(service),
          }))
          .filter((service) => !existingMappingPorts.value.has(service.port)),
      };
      setDiscoveredData(nextData);
      selectedServices.value = nextData.services.filter((service) =>
        Boolean(service.suggestedSubdomain.trim()),
      );
    },
  });
}

const collectDuplicateValues = (values: string[]): string[] => {
  const seen = new Set<string>();
  const duplicates = new Set<string>();
  for (const value of values) {
    if (!value) continue;
    if (seen.has(value)) {
      duplicates.add(value);
      continue;
    }
    seen.add(value);
  }
  return [...duplicates];
};

async function saveDiscoveredServices() {
  if (!isDiscoverSelectionValid.value || !savedRootDomain.value) return;

  const candidateHosts = selectedServices.value.map((service) =>
    composeHostFromSubdomain(service.suggestedSubdomain, savedRootDomain.value),
  );
  const existingHostSet = new Set(allMappings.value.map((item) => item.host));
  const duplicateHosts = [
    ...new Set([
      ...candidateHosts.filter((host) => existingHostSet.has(host)),
      ...collectDuplicateValues(candidateHosts),
    ]),
  ];

  if (duplicateHosts.length > 0) {
    toast.error("发现结果包含重复 Host", {
      description: duplicateHosts.join("、"),
    });
    return;
  }

  await runSaveMappings(async () => {
    const next = [...allMappings.value];

    for (const service of selectedServices.value) {
      next.push({
        host: composeHostFromSubdomain(
          service.suggestedSubdomain,
          savedRootDomain.value,
        ),
        target: `http://127.0.0.1:${service.port}/`,
        use_auth: service.detail.rule.use_auth,
        access_mode: DEFAULT_ACCESS_MODE,
        suppress_toolbar: false,
        preserve_host: false,
        service_role: "app",
        title: "",
        title_override: "",
        favicon: "",
      });
    }

    await configStore.saveHostMappings(next);
    toast.success(`已添加 ${selectedServices.value.length} 条 Host 映射`);
    dismissDiscoverDialog();
  });
}

async function syncRoutes() {
  await runSyncRoutes(() => ConfigAPI.syncRoutes(), {
    onSuccess: (result) => {
      if (result.success) {
        toast.success("已同步到网关", {
          description: `路径路由 ${result.data?.synced_rules ?? 0} 条，Host 路由 ${result.data?.synced_host_rules ?? 0} 条。`,
        });
        return;
      }
      toast.error("同步失败", {
        description: result.message || "网关未返回成功结果",
      });
    },
  });
}

async function refreshAllTitles() {
  await runRefreshTitles(() => configStore.refreshAllHostMappingTitles(), {
    onSuccess: (summary) => {
      toast.success("图标和标题刷新完成", {
        description: `更新 ${summary.updated} 条，失败 ${summary.failed} 条，跳过 ${summary.skipped} 条。`,
      });
      brokenFaviconKeys.value = new Set();
    },
  });
}

async function exportBookmarks() {
  await runExportBookmarks(() => ConfigAPI.downloadHostMappingBookmarks(), {
    onSuccess: (blob) => {
      downloadBlob(blob, buildBookmarkExportFilename(savedRootDomain.value));
      toast.success("书签已导出", {
        description: `共导出 ${visibleMappings.value.length} 条子域映射。`,
      });
    },
  });
}
</script>
