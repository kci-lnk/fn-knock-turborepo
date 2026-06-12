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
          <span v-if="savedEdgeClientIpProviderLabel">
            · {{ savedEdgeClientIpProviderLabel }}
          </span>
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
                        {{
                          formatAuthServiceHostWithPublicPort(
                            authServiceMapping.host,
                          )
                        }}
                      </div>
                      <div class="mt-1 text-xs text-muted-foreground">
                        在尚未登录时，会自动跳转到
                        <code>
                          https://{{
                            formatAuthServiceHostWithPublicPort(
                              authServiceMapping.host,
                            )
                          }}
                        </code>
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
              <div
                v-if="!modeForm.edge_client_ip_enabled"
                class="mt-4 grid gap-3 border-t pt-4 sm:grid-cols-[minmax(0,1fr)_12rem] sm:items-end"
              >
                <div class="space-y-1">
                  <Label for="auth-service-public-port">
                    鉴权服务所使用的端口
                  </Label>
                  <p class="text-xs leading-5 text-muted-foreground">
                    路由器 10012 → 7999 时填写 10012；未修改时使用当前入口端口。
                  </p>
                </div>
                <Input
                  id="auth-service-public-port"
                  v-model.number="authServicePublicPort"
                  type="number"
                  min="1"
                  max="65535"
                  inputmode="numeric"
                  class="sm:max-w-48"
                />
              </div>
            </div>
            <div class="rounded-lg border px-4 py-4">
              <div class="flex flex-col gap-4">
                <div class="flex items-start justify-between gap-4">
                  <div class="space-y-1">
                    <Label for="edge-client-ip-enabled"
                      >边缘网络真实 IP 识别</Label
                    >
                    <p class="text-xs text-muted-foreground">
                      仅对子域模式生效。开启后，公开鉴权地址不再自动补访问端口。
                    </p>
                    <p class="text-xs text-muted-foreground">
                      你可以在下方选择真实 IP
                      头来源供应商，网关会按当前选择识别真实 IP，并通过
                      X-Forwarded-For 传给鉴权服务。
                    </p>
                    <p
                      v-if="!isEdgeClientIPModeEditable"
                      class="text-xs text-amber-600"
                    >
                      当前运行模式不是子域模式，这组设置暂时不会生效。
                    </p>
                  </div>
                  <Switch
                    id="edge-client-ip-enabled"
                    v-model="modeForm.edge_client_ip_enabled"
                    :disabled="!isEdgeClientIPModeEditable"
                  />
                </div>

                <div v-if="modeForm.edge_client_ip_enabled">
                  <div
                    class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between sm:gap-4"
                  ></div>

                  <div class="mt-4 grid grid-cols-1 gap-3 md:grid-cols-2">
                    <button
                      v-for="option in edgeClientIpProviderOptions"
                      :key="option.value"
                      type="button"
                      :disabled="!isEdgeClientIPModeEditable"
                      :class="[
                        'rounded-xl border p-4 text-left transition-colors disabled:cursor-not-allowed disabled:opacity-60',
                        activeEdgeClientIpProvider === option.value
                          ? 'border-primary bg-primary/5 shadow-sm'
                          : 'border-border bg-background hover:border-primary/40 hover:bg-muted/40',
                      ]"
                      @click="selectEdgeClientIpProvider(option.value)"
                    >
                      <div
                        class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
                      >
                        <div class="grid min-w-0 gap-1">
                          <div class="text-sm font-medium">
                            {{ option.label }}
                          </div>
                          <div class="text-xs text-muted-foreground">
                            {{ option.description }}
                          </div>
                          <div class="text-[11px] text-muted-foreground">
                            {{ option.headerHint }}
                          </div>
                        </div>
                        <span
                          :class="[
                            'self-start shrink-0 whitespace-nowrap rounded-full border px-2 py-0.5 text-[11px] font-medium',
                            activeEdgeClientIpProvider === option.value
                              ? 'border-primary/20 bg-primary/10 text-primary'
                              : 'border-border text-muted-foreground',
                          ]"
                        >
                          {{
                            activeEdgeClientIpProvider === option.value
                              ? "当前"
                              : "切换"
                          }}
                        </span>
                      </div>
                    </button>
                  </div>
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
          拖动左侧把手可调整显示顺序 ，如果发现无法反代，可以尝试
          <a
            href="#/system/gateway-proxy-headers"
            class="underline underline-offset-2 hover:text-foreground"
          >
            关闭代理头 </a
          >，亦或尝试

          <a
            href="#/system/gateway-host-response"
            class="underline underline-offset-2 hover:text-foreground"
          >
            关闭Host头
          </a>
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
          <Table container-class="mapping-table-scroll">
            <TableHeader>
              <TableRow>
                <TableHead
                  class="mapping-sticky-cell mapping-sticky-cell-1"
                ></TableHead>
                <TableHead
                  class="mapping-sticky-cell mapping-sticky-cell-2 mapping-icon-cell"
                >
                  <span class="sr-only">Icon</span>
                </TableHead>
                <TableHead
                  class="mapping-sticky-cell mapping-sticky-cell-3 mapping-title-cell"
                >
                  标题
                </TableHead>
                <TableHead>域名</TableHead>
                <TableHead>目标</TableHead>
                <TableHead class="w-[7rem] min-w-[7rem] max-w-[7rem]">
                  流量
                </TableHead>
                <TableHead class="w-[5.5rem] min-w-[5.5rem]">
                  状态
                </TableHead>
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
                  colspan="8"
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
                <TableCell
                  class="mapping-sticky-cell mapping-sticky-cell-1 mapping-icon-cell"
                >
                  <button
                    type="button"
                    class="mapping-drag-handle -ml-1 inline-flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition hover:bg-muted hover:text-foreground disabled:pointer-events-none disabled:opacity-40"
                    :disabled="isSavingMappings || filteredMappings.length < 2"
                    aria-label="拖动排序"
                  >
                    <GripVertical class="h-4 w-4" />
                  </button>
                </TableCell>
                <TableCell
                  class="mapping-sticky-cell mapping-sticky-cell-2 mapping-icon-cell"
                >
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
                  class="mapping-sticky-cell mapping-sticky-cell-3 mapping-title-cell text-sm"
                  :title="getMappingTitleForDisplay(mapping)"
                >
                  <div class="flex min-w-0 items-center gap-2">
                    <Popover
                      v-if="shouldShowProtocolHeadersWarning(mapping)"
                      :open="isProtocolHeadersWarningOpen(mapping.host)"
                      @update:open="
                        (nextOpen) =>
                          handleProtocolHeadersWarningOpenChange(
                            mapping.host,
                            nextOpen,
                          )
                      "
                    >
                      <PopoverAnchor as-child>
                        <button
                          type="button"
                          class="inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-md text-destructive transition-colors hover:bg-destructive/10 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-destructive/30"
                          :class="{
                            'bg-destructive/10': isProtocolHeadersWarningOpen(
                              mapping.host,
                            ),
                          }"
                          :aria-label="`${formatHostWithAccessEntryPort(mapping.host)} 的 Home Assistant 需要关闭协议头`"
                          @mouseenter="openProtocolHeadersWarning(mapping.host)"
                          @mouseleave="
                            scheduleCloseProtocolHeadersWarning(mapping.host)
                          "
                          @click="toggleProtocolHeadersWarning(mapping.host)"
                        >
                          <CircleAlert class="h-3.5 w-3.5" />
                        </button>
                      </PopoverAnchor>
                      <PopoverContent
                        side="top"
                        align="start"
                        class="w-72 border-destructive/20 text-left"
                        @mouseenter="openProtocolHeadersWarning(mapping.host)"
                        @mouseleave="
                          scheduleCloseProtocolHeadersWarning(mapping.host)
                        "
                      >
                        <div class="space-y-3">
                          <div class="space-y-1">
                            <div class="flex items-center gap-2">
                              <CircleAlert class="h-4 w-4 text-destructive" />
                              <p class="text-sm font-medium">
                                Home Assistant 需要关闭协议头
                              </p>
                            </div>
                            <p class="text-xs leading-5 text-muted-foreground">
                              检测到该应用是 Home Assistant。Home Assistant
                              在开启协议头时可能无法正常访问，建议前往“协议头”页面将该应用关闭。
                            </p>
                          </div>
                          <a
                            href="#/system/gateway-proxy-headers"
                            class="inline-flex rounded-md border border-destructive/20 bg-destructive/5 px-2.5 py-1.5 text-xs font-medium text-destructive transition hover:bg-destructive/10"
                          >
                            去关闭协议头
                          </a>
                        </div>
                      </PopoverContent>
                    </Popover>
                    <div class="min-w-0 flex-1">
                      <InlineCommentEditor
                        :text="getMappingDisplayTitle(mapping)"
                        placeholder="输入展示标题..."
                        empty-text="未获取"
                        :save="
                          (value) => saveMappingTitleOverride(mapping, value)
                        "
                      />
                    </div>
                  </div>
                </TableCell>
                <TableCell class="break-all font-medium">
                  <button
                    type="button"
                    class="break-all rounded-sm text-left transition-colors hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                    :title="`点击复制 ${formatHostWithAccessEntryPort(mapping.host)}`"
                    :aria-label="`复制域名 ${formatHostWithAccessEntryPort(mapping.host)}`"
                    @click="copyMappingHost(mapping)"
                  >
                    {{ formatHostWithAccessEntryPort(mapping.host) }}
                  </button>
                </TableCell>
                <TableCell>{{ mapping.target }}</TableCell>
                <TableCell class="w-[7rem] min-w-[7rem] max-w-[7rem]">
                  <HostTrafficActivity
                    :host="mapping.host"
                    :title="getMappingTitleForDisplay(mapping)"
                    :sample="getHostTrafficSample(mapping.host)"
                    :timestamp="trafficRealtimeStats?.timestamp ?? null"
                  />
                </TableCell>
                <TableCell class="w-[5.5rem] min-w-[5.5rem]">
                  <div
                    class="flex min-w-max flex-nowrap items-center gap-2 text-xs text-muted-foreground"
                  >
                    <Badge
                      v-if="isAuthServiceTarget(mapping.target)"
                      variant="default"
                    >
                      鉴权服务
                    </Badge>
                    <ShieldCheck
                      v-if="mapping.use_auth"
                      class="h-3.5 w-3.5 shrink-0"
                    />
                    <Badge v-else variant="secondary">公开访问</Badge>
                    <PanelsTopLeft
                      v-if="mapping.use_auth && !mapping.suppress_toolbar"
                      class="h-3.5 w-3.5 shrink-0"
                    />
                    <TooltipProvider
                      v-if="getLocationRulesCount(mapping) > 0"
                    >
                      <Tooltip
                        :open="isLocationRulesTooltipOpen(mapping.host)"
                        @update:open="
                          (nextOpen) =>
                            handleLocationRulesTooltipOpenChange(
                              mapping.host,
                              nextOpen,
                            )
                        "
                      >
                        <TooltipTrigger as-child>
                          <button
                            type="button"
                            class="inline-flex h-5 w-5 shrink-0 cursor-help items-center justify-center rounded-md transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
                            :aria-label="`${formatHostWithAccessEntryPort(mapping.host)} 有 ${getLocationRulesCount(mapping)} 条路径规则`"
                            @click="handleLocationRulesTooltipTriggerClick(mapping.host)"
                          >
                            <RouteIcon class="h-3.5 w-3.5" />
                          </button>
                        </TooltipTrigger>
                        <TooltipContent side="top" align="center">
                          <p>
                            已配置 {{ getLocationRulesCount(mapping) }} 条路径规则
                          </p>
                        </TooltipContent>
                      </Tooltip>
                    </TooltipProvider>
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
                      v-if="!isAuthServiceTarget(mapping.target)"
                      variant="ghost"
                      size="sm"
                      @click="openGatewayLocations(mapping.host)"
                    >
                      路径
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
      <DialogContent
        class="flex max-h-[85vh] flex-col gap-0 overflow-hidden overscroll-contain p-0 sm:max-w-[520px] max-sm:!inset-x-0 max-sm:!bottom-[var(--mapping-dialog-keyboard-inset)] max-sm:!top-auto max-sm:!max-w-none max-sm:!translate-x-0 max-sm:!translate-y-0 max-sm:max-h-[var(--mapping-dialog-mobile-max-height)] max-sm:rounded-b-none max-sm:border-b-0"
        :style="mappingDialogContentStyle"
        :show-close-button="false"
      >
        <div
          v-if="mappingDialogView === 'advanced'"
          class="shrink-0 border-b bg-background px-6 pb-3 pt-8"
        >
          <button
            type="button"
            class="-mx-2 inline-flex w-[calc(100%+1rem)] items-center gap-3 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            aria-label="返回基础信息"
            @click="returnMappingBasicView"
          >
            <ChevronLeft class="h-4 w-4 shrink-0" />
            <span class="text-sm font-semibold">高级配置</span>
          </button>
        </div>
        <div
          ref="mappingDialogScrollRef"
          class="relative min-h-0 flex-1 overscroll-contain overflow-x-hidden overflow-y-auto px-6 [overflow-anchor:none]"
          :style="mappingDialogScrollStyle"
          @focusin="handleMappingDialogFocusIn"
        >
          <Transition
            :enter-active-class="mappingViewTransitionEnterActiveClass"
            :leave-active-class="mappingViewTransitionLeaveActiveClass"
            :enter-from-class="mappingViewTransitionEnterFromClass"
            enter-to-class="translate-x-0 opacity-100"
            leave-from-class="translate-x-0 opacity-100"
            :leave-to-class="mappingViewTransitionLeaveToClass"
          >
            <div
              v-if="mappingDialogView === 'basic'"
              key="mapping-basic"
              class="grid gap-4 pb-4 pt-6"
            >
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
                    role="radiogroup"
                    aria-label="Host 输入模式"
                    class="grid w-full grid-cols-2 rounded-lg bg-muted p-[3px] text-muted-foreground sm:w-[216px]"
                  >
                    <button
                      type="button"
                      role="radio"
                      :aria-checked="mappingInputMode === 'subdomain'"
                      :disabled="!canUseRootDomainSuffix"
                      class="inline-flex h-8 items-center justify-center rounded-md px-2 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50"
                      :class="
                        mappingInputMode === 'subdomain'
                          ? 'bg-background text-foreground shadow-sm'
                          : 'hover:text-foreground'
                      "
                      @click="handleMappingInputModeChange('subdomain')"
                    >
                      固定后缀
                    </button>
                    <button
                      type="button"
                      role="radio"
                      :aria-checked="mappingInputMode === 'full_host'"
                      class="inline-flex h-8 items-center justify-center rounded-md px-2 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                      :class="
                        mappingInputMode === 'full_host'
                          ? 'bg-background text-foreground shadow-sm'
                          : 'hover:text-foreground'
                      "
                      @click="handleMappingInputModeChange('full_host')"
                    >
                      完整域名
                    </button>
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

              <Button
                type="button"
                variant="outline"
                class="h-auto w-full justify-between gap-3 px-4 py-3 text-left"
                @click="openMappingAdvancedView"
              >
                <span class="flex min-w-0 flex-1 items-start gap-3">
                  <Settings class="mt-0.5 h-4 w-4 text-muted-foreground" />
                  <span class="min-w-0 flex-1 space-y-1">
                    <span class="block text-sm font-medium">高级配置</span>
                    <span
                      class="block whitespace-normal break-words text-xs font-normal leading-5 text-muted-foreground"
                    >
                      {{ mappingAdvancedSummary }}
                    </span>
                  </span>
                </span>
                <ChevronRight class="h-4 w-4 shrink-0 text-muted-foreground" />
              </Button>
            </div>

            <div v-else key="mapping-advanced" class="space-y-4 pb-4 pt-4">
              <div class="space-y-3">
                <div class="rounded-lg border bg-muted/20 px-4 py-2.5">
                  <div
                    class="grid gap-3 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]"
                  >
                    <div class="min-w-0 space-y-1">
                      <p class="text-xs text-muted-foreground">Host</p>
                      <p class="truncate text-sm font-medium">
                        {{ mappingAdvancedHostLabel }}
                      </p>
                    </div>
                    <div class="min-w-0 space-y-1">
                      <p class="text-xs text-muted-foreground">目标</p>
                      <p class="truncate text-sm">
                        {{ mappingAdvancedTargetLabel }}
                      </p>
                    </div>
                  </div>
                </div>
              </div>

              <div class="space-y-3">
                <div
                  class="flex items-center justify-between gap-4 rounded-lg border px-4 py-3"
                >
                  <div class="min-w-0 space-y-1">
                    <Label for="mapping-auth">要求登录</Label>
                    <p class="text-xs leading-5 text-muted-foreground">
                      安全性，未登录用户会被要求登录才可以访问。
                    </p>
                  </div>
                  <Switch
                    id="mapping-auth"
                    v-model="mappingUseAuth"
                    :disabled="isMappingAuthService"
                  />
                </div>

                <div
                  class="flex items-center justify-between gap-4 rounded-lg border px-4 py-3"
                >
                  <div class="min-w-0 space-y-1">
                    <Label for="mapping-toolbar">显示传送门</Label>
                    <p class="text-xs leading-5 text-muted-foreground">
                      完成登录后显示快速切换应用的小图标。
                    </p>
                  </div>
                  <Switch id="mapping-toolbar" v-model="showToolbar" />
                </div>

                <div
                  v-if="canShowBasicAuthInjection"
                  class="space-y-3 rounded-lg border px-4 py-3"
                >
                  <div class="flex items-center justify-between gap-4">
                    <div class="min-w-0 space-y-1">
                      <Label for="mapping-basic-auth">Basic Auth 跳过</Label>
                      <p class="text-xs leading-5 text-muted-foreground">
                        开启并填写用户名和密码后，访问时会自动带上这组登录信息，避免每次打开都弹出浏览器自带的Basic Auth登录框。
                      </p>
                    </div>
                    <Switch
                      id="mapping-basic-auth"
                      v-model="basicAuthInjectionModel"
                      :disabled="isMappingAuthService"
                    />
                  </div>

                  <div
                    v-if="basicAuthInjectionModel"
                    class="grid gap-3 sm:grid-cols-2"
                  >
                    <div class="space-y-2">
                      <Label for="mapping-basic-auth-username">用户名</Label>
                      <Input
                        id="mapping-basic-auth-username"
                        v-model="mappingForm.basic_auth.username"
                        autocomplete="username"
                        placeholder="admin"
                      />
                    </div>
                    <div class="space-y-2">
                      <Label for="mapping-basic-auth-password">密码</Label>
                      <Input
                        id="mapping-basic-auth-password"
                        v-model="mappingForm.basic_auth.password"
                        type="password"
                        autocomplete="new-password"
                      />
                    </div>
                    <p
                      v-if="basicAuthValidationMessage"
                      class="sm:col-span-2 text-xs text-destructive"
                    >
                      {{ basicAuthValidationMessage }}
                    </p>
                  </div>
                </div>

                <div
                  class="flex items-center justify-between gap-4 rounded-lg border px-4 py-3"
                >
                  <div class="min-w-0 space-y-1">
                    <Label for="mapping-proxy-headers">协议头</Label>
                    <p class="text-xs leading-5 text-muted-foreground">
                      <template v-if="gatewayProxyHeadersBlockedReason">
                        {{ gatewayProxyHeadersBlockedReason }}
                      </template>
                      <template v-else>
                        开启后向上游发送
                        <code>X-Forwarded-*</code>
                        等代理头。
                      </template>
                    </p>
                  </div>
                  <Switch
                    id="mapping-proxy-headers"
                    v-model="sendProxyHeadersModel"
                    :disabled="
                      isSavingMappings || !!gatewayProxyHeadersBlockedReason
                    "
                  />
                </div>

                <div
                  class="flex items-center justify-between gap-4 rounded-lg border px-4 py-3"
                >
                  <div class="min-w-0 space-y-1">
                    <Label for="mapping-host-response">Host响应</Label>
                    <p class="text-xs leading-5 text-muted-foreground">
                      <template v-if="gatewayHostResponseBlockedReason">
                        {{ gatewayHostResponseBlockedReason }}
                      </template>
                      <template v-else>
                        开启后向上游保留用户访问时的
                        <code>Host</code>
                        。
                      </template>
                    </p>
                  </div>
                  <Switch
                    id="mapping-host-response"
                    v-model="preserveHostModel"
                    :disabled="
                      isSavingMappings || !!gatewayHostResponseBlockedReason
                    "
                  />
                </div>
              </div>
            </div>
          </Transition>
        </div>
        <DialogFooter
          class="shrink-0 border-t px-6 py-4 max-sm:pb-[calc(env(safe-area-inset-bottom)+1rem)]"
        >
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
      <DialogContent
        class="flex max-h-[85vh] flex-col overflow-hidden sm:max-w-[820px]"
      >
        <DialogHeader class="shrink-0">
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
            <div class="flex items-center gap-2">
              <Button
                variant="outline"
                size="icon"
                :disabled="isDiscovering"
                @click="toggleDiscoverSettings"
              >
                <SlidersHorizontal class="h-4 w-4" />
              </Button>
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
          </div>
          <ScanDiscoveryTargetsSettings
            ref="discoverTargetsSettingsRef"
            v-show="isDiscoverSettingsOpen"
            class="mt-3"
          />
        </DialogHeader>

        <div class="flex-1 min-h-0 overflow-auto">
          <div class="py-2">
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

            <div
              v-else-if="discoveredData"
              class="rounded-md border bg-background"
            >
              <Table class="min-w-[42rem]" container-class="overflow-visible">
                <TableHeader
                  class="sticky top-0 z-10 bg-background shadow-sm [&_th]:sticky [&_th]:top-0 [&_th]:z-10 [&_th]:bg-background"
                >
                  <TableRow>
                    <TableHead class="w-[50px] text-center">
                      <input
                        type="checkbox"
                        class="h-4 w-4 cursor-pointer"
                        :checked="isAllSelected"
                        @change="onToggleAllDiscoverSelect"
                      />
                    </TableHead>
                    <TableHead v-if="showDiscoverHostColumn" class="w-[140px]">
                      主机
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
                    :key="`${resolveDiscoveredServiceHost(svc)}-${svc.port}-${index}`"
                  >
                    <TableCell class="text-center">
                      <input
                        type="checkbox"
                        class="h-4 w-4 cursor-pointer"
                        :value="svc"
                        v-model="selectedServices"
                      />
                    </TableCell>
                    <TableCell
                      v-if="showDiscoverHostColumn"
                      class="font-mono text-xs text-muted-foreground"
                    >
                      {{ resolveDiscoveredServiceHost(svc) }}
                    </TableCell>
                    <TableCell class="font-medium">{{ svc.port }}</TableCell>
                    <TableCell>
                      <span
                        v-if="svc.requiresBasicAuth"
                        class="text-amber-600 bg-amber-500/10 text-xs px-2 py-0.5 rounded"
                      >
                        Basic Auth
                      </span>
                      <span
                        v-else-if="svc.httpStatus === 401"
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
        </div>

        <DialogFooter class="mt-2 shrink-0 items-center sm:justify-between">
          <span class="text-sm text-muted-foreground">
            <template v-if="discoveredData">
              已扫描 {{ discoveredData.totalPortsScanned }} 个端口，选中
              {{ selectedServices.length }} /
              {{ discoveredData.services.length }}
              项
              <template v-if="discoveredData.scanCidrs?.length">
                ，覆盖 {{ discoveredData.scanCidrs.length }} 个网段 /
                {{
                  discoveredData.scanHostCount ||
                  discoveredData.scannedHosts ||
                  0
                }}
                台主机
              </template>
              <template
                v-if="
                  !discoveredData.scanCidrs?.length &&
                  discoveredData.scannedHosts &&
                  discoveredData.scannedHosts > 1
                "
              >
                ，覆盖
                {{
                  discoveredData.scanScope ||
                  `${discoveredData.scannedHosts} 台主机`
                }}
              </template>
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
import {
  computed,
  nextTick,
  onMounted,
  onUnmounted,
  reactive,
  ref,
  watch,
} from "vue";
import { useRouter } from "vue-router";
import {
  CircleAlert,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Download,
  GripVertical,
  Image,
  PanelsTopLeft,
  Plus,
  RefreshCw,
  Route as RouteIcon,
  Search,
  Settings,
  ShieldCheck,
  SlidersHorizontal,
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
import {
  Popover,
  PopoverAnchor,
  PopoverContent,
} from "@/components/ui/popover";
import { Switch } from "@/components/ui/switch";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { VueDraggable } from "vue-draggable-plus";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import HostTrafficActivity from "@/components/HostTrafficActivity.vue";
import InlineCommentEditor from "@admin-shared/components/InlineCommentEditor.vue";
import ProxyTargetInputField from "@admin-shared/components/common/ProxyTargetInputField.vue";
import ScanDiscoveryTargetsSettings from "@/components/ScanDiscoveryTargetsSettings.vue";
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
import { copyTextToClipboard } from "@admin-shared/utils/copyTextToClipboard";
import { useConfigStore } from "../store/config";
import { isAnySubdomainRoutingMode } from "../lib/reverse-proxy-submode";
import {
  ConfigAPI,
  DashboardAPI,
  ScanAPI,
  SystemAPI,
  type DiscoveredServiceInfo,
  type HostMappingBasicAuthProbeResult,
  type ScanDiscoverResponse,
} from "../lib/api";
import { docsUrls } from "../lib/docs";
import type {
  GatewayHostResponseDetails,
  GatewayProxyHeadersDetails,
  HostTrafficStats,
  HostMapping,
  SubdomainModeConfig,
  TrafficStats,
} from "../types";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { downloadBlob } from "@admin-shared/utils/downloadBlob";

type MappingInputMode = "subdomain" | "full_host";
type MappingDialogView = "basic" | "advanced";
type MappingDialogMotionDirection = "forward" | "back";

type DiscoveredHostService = DiscoveredServiceInfo & {
  suggestedSubdomain: string;
};

type DiscoveredHostResponse = Omit<ScanDiscoverResponse, "services"> & {
  services: DiscoveredHostService[];
};

type EdgeClientIpProvider = "aliyun_esa" | "tencent_edgeone";

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
const discoverTargetsSettingsRef = ref<InstanceType<
  typeof ScanDiscoveryTargetsSettings
> | null>(null);
const isDiscoverSettingsOpen = ref(false);

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

const edgeClientIpProviderOptions: Array<{
  value: EdgeClientIpProvider;
  label: string;
  description: string;
  headerHint: string;
}> = [
  {
    value: "tencent_edgeone",
    label: "腾讯 EdgeOne",
    description:
      "适合腾讯 EdgeOne 回源场景，由网关识别 EO 头并回填真实客户端 IP。",
    headerHint: "网关读取 EO-Connecting-IP，并转发到 X-Forwarded-For",
  },
  {
    value: "aliyun_esa",
    label: "阿里云 ESA",
    description:
      "适合阿里云 ESA 回源场景，由网关识别 ESA 真实 IP 头并回填客户端 IP。",
    headerHint:
      "网关读取 Ali-Real-Client-IP，并转发到 X-Forwarded-For；请开启 ESA 托管转换请求头选项",
  },
];

const resolveEdgeClientIpProvider = (
  value: Pick<
    SubdomainModeConfig,
    "edge_client_ip_enabled" | "aliyun_esa_enabled" | "tencent_edgeone_enabled"
  >,
): EdgeClientIpProvider | null => {
  if (!value.edge_client_ip_enabled) return null;
  if (value.tencent_edgeone_enabled) return "tencent_edgeone";
  if (value.aliyun_esa_enabled) return "aliyun_esa";
  return null;
};

const getEdgeClientIpProviderLabel = (
  provider: EdgeClientIpProvider | null,
): string => {
  if (provider === "tencent_edgeone") return "腾讯 EdgeOne";
  if (provider === "aliyun_esa") return "阿里云 ESA";
  return "";
};

const resolveDiscoveredServiceHost = (
  service: Pick<DiscoveredServiceInfo, "host">,
) => service.host?.trim() || discoveredData.value?.host?.trim() || "127.0.0.1";

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

const normalizePublicPort = (value: unknown): number => {
  const port =
    typeof value === "number"
      ? value
      : Number.parseInt(String(value ?? "").trim(), 10);
  if (!Number.isFinite(port) || port <= 0) return 0;
  return Math.floor(port);
};

const parsePublicAuthBaseUrlPort = (
  value: string | undefined,
  scheme?: "http" | "https",
): number => {
  const trimmed = value?.trim();
  if (!trimmed) return 0;

  try {
    const parsed = new URL(trimmed);
    if (scheme && parsed.protocol !== `${scheme}:`) return 0;
    return normalizePublicPort(parsed.port);
  } catch {
    return 0;
  }
};

const syncPublicAuthBaseUrlPort = (
  value: string | undefined,
  port: number,
): string => {
  const trimmed = value?.trim();
  if (!trimmed || !port) return trimmed || "";

  try {
    const parsed = new URL(trimmed);
    const scheme =
      parsed.protocol === "https:"
        ? "https"
        : parsed.protocol === "http:"
          ? "http"
          : null;
    if (!scheme) return "";

    const isDefaultPort =
      (scheme === "https" && port === 443) ||
      (scheme === "http" && port === 80);
    parsed.port = isDefaultPort ? "" : String(port);
    parsed.pathname = parsed.pathname.replace(/\/+$/, "") || "/";
    parsed.search = "";
    parsed.hash = "";
    return parsed.toString().replace(/\/$/, "");
  } catch {
    return "";
  }
};

const createDefaultModeForm = (): SubdomainModeConfig => ({
  root_domain: "",
  auth_host: "",
  auth_target: "http://localhost:7997",
  cookie_domain: "",
  edge_client_ip_enabled: false,
  aliyun_esa_enabled: false,
  tencent_edgeone_enabled: false,
  public_auth_base_url: "",
  public_http_port: 0,
  public_https_port: 0,
  auth_cache_ttl_seconds: 1,
  auth_cache_unauthorized_ttl_seconds: 1,
  default_access_mode: "login_first",
  auto_add_whitelist_on_login: true,
  passkey_rp_mode: "auth_host",
  passkey_rp_id: "",
});

const DEFAULT_AUTH_SUBDOMAIN = "auth";
const DEFAULT_ACCESS_MODE: HostMapping["access_mode"] = "login_first";
const HOME_ASSISTANT_TARGET_PORT = 8123;

const createDisabledMappingBasicAuth = (): HostMapping["basic_auth"] => ({
  enabled: false,
  username: "",
  password: "",
});

const normalizeMappingBasicAuth = (
  value?: Partial<HostMapping["basic_auth"]> | null,
): HostMapping["basic_auth"] => {
  const raw = value ?? {};
  const username = typeof raw.username === "string" ? raw.username.trim() : "";
  const password = typeof raw.password === "string" ? raw.password : "";

  if (raw.enabled !== true) {
    return createDisabledMappingBasicAuth();
  }

  return {
    enabled: true,
    username,
    password,
  };
};

const normalizeBasicAuthProbeTarget = (value: string): string => {
  const trimmed = value.trim();
  if (!trimmed) return "";

  try {
    const parsed = new URL(trimmed);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return "";
    }
    parsed.hash = "";
    return parsed.toString();
  } catch {
    return "";
  }
};

const createDefaultMapping = (): HostMapping => ({
  host: "",
  target: "",
  use_auth: true,
  access_mode: DEFAULT_ACCESS_MODE,
  suppress_toolbar: false,
  preserve_host: true,
  basic_auth: createDisabledMappingBasicAuth(),
  locations: [],
  service_role: "app",
  title: "",
  title_override: "",
  favicon: "",
});

const searchQuery = ref("");
const router = useRouter();
const isDialogOpen = ref(false);
const mappingDialogView = ref<MappingDialogView>("basic");
const mappingDialogMotionDirection =
  ref<MappingDialogMotionDirection>("forward");
const deleteDialogState = ref<DeleteDialogState | null>(null);
const editingHost = ref<string | null>(null);
const mappingInputMode = ref<MappingInputMode>("subdomain");
const mappingSubdomain = ref("");
const accessEntryPort = ref("7999");
const brokenFaviconKeys = ref(new Set<string>());
const draggableVisibleMappings = ref<HostMapping[]>([]);
const gatewayProxyHeadersDetails = ref<GatewayProxyHeadersDetails | null>(null);
const gatewayHostResponseDetails = ref<GatewayHostResponseDetails | null>(null);
const isLoadingGatewayProxyHeaders = ref(false);
const isLoadingGatewayHostResponse = ref(false);
const gatewayProxyHeadersLoadError = ref("");
const gatewayHostResponseLoadError = ref("");
const trafficRealtimeStats = ref<TrafficStats | null>(null);
const mappingDialogScrollRef = ref<HTMLElement | null>(null);
const mappingDialogKeyboardInset = ref(0);
let gatewayProxyHeadersRequestId = 0;
let gatewayHostResponseRequestId = 0;
let trafficRealtimeTimer: number | null = null;
let isTrafficRealtimeLoading = false;
let mappingDialogKeyboardScrollTimer: number | null = null;
const mappingMetadataTarget = ref("");
const openProtocolHeadersWarningHost = ref<string | null>(null);
const openLocationRulesTooltipHost = ref<string | null>(null);
const isTouchInteraction = ref(false);
const sendProxyHeaders = ref(true);
const preserveHost = ref(true);
const sendProxyHeadersTouched = ref(false);
const preserveHostTouched = ref(false);
const mappingAdvancedCleanupHosts = ref<string[]>([]);
const basicAuthProbeCache = ref(
  new Map<string, HostMappingBasicAuthProbeResult>(),
);
const isLoadingBasicAuthProbe = ref(false);
const modeForm = reactive<SubdomainModeConfig>(createDefaultModeForm());
const mappingForm = reactive<HostMapping>(createDefaultMapping());
let basicAuthProbeTimer: number | null = null;
let basicAuthProbeRequestId = 0;
let interactionMediaQuery: MediaQueryList | null = null;

const currentModeConfig = computed(
  () => configStore.config?.subdomain_mode ?? createDefaultModeForm(),
);
const authServicePort = computed(
  () => parseTargetPort(currentModeConfig.value.auth_target) ?? 7997,
);
const isAuthServiceTarget = (target: string): boolean =>
  parseTargetPort(target) === authServicePort.value;
const getLocationRulesCount = (mapping: HostMapping): number =>
  mapping.locations?.length ?? 0;
const isLocationRulesTooltipOpen = (host: string): boolean =>
  openLocationRulesTooltipHost.value === host;
const savedRootDomain = computed(() =>
  normalizeRootDomainValue(currentModeConfig.value.root_domain),
);
const savedEdgeClientIpProvider = computed(() =>
  resolveEdgeClientIpProvider(currentModeConfig.value),
);
const savedEdgeClientIpProviderLabel = computed(() =>
  savedEdgeClientIpProvider.value
    ? `${getEdgeClientIpProviderLabel(savedEdgeClientIpProvider.value)} 真实 IP`
    : "",
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
const basicAuthProbeTargetKey = computed(() =>
  normalizeBasicAuthProbeTarget(mappingForm.target),
);
const currentBasicAuthProbeResult = computed(() => {
  const target = basicAuthProbeTargetKey.value;
  if (!target) return null;
  return basicAuthProbeCache.value.get(target) ?? null;
});
const showToolbar = computed({
  get: () => !mappingForm.suppress_toolbar,
  set: (value: boolean) => {
    mappingForm.suppress_toolbar = !value;
  },
});
const mappingUseAuth = computed({
  get: () => !isMappingAuthService.value && mappingForm.use_auth,
  set: (value: boolean) => {
    mappingForm.use_auth = value;
  },
});
const basicAuthInjectionModel = computed({
  get: () => !isMappingAuthService.value && mappingForm.basic_auth.enabled,
  set: (value: boolean) => {
    mappingForm.basic_auth.enabled = value;
    if (!value) {
      mappingForm.basic_auth.username = "";
      mappingForm.basic_auth.password = "";
    }
  },
});
const basicAuthValidationMessage = computed(() => {
  if (!basicAuthInjectionModel.value) return "";
  const username = mappingForm.basic_auth.username.trim();
  if (!username || !mappingForm.basic_auth.password) {
    return "请填写用户名和密码，才能自动跳过浏览器登录框。";
  }
  if (username.includes(":")) {
    return "用户名不能包含英文冒号（:）。";
  }
  return "";
});
const canShowBasicAuthInjection = computed(
  () =>
    !isMappingAuthService.value &&
    (basicAuthInjectionModel.value ||
    currentBasicAuthProbeResult.value?.requiresBasicAuth === true),
);
const mappingDialogContentStyle = computed(() => ({
  "--mapping-dialog-keyboard-inset": `${mappingDialogKeyboardInset.value}px`,
  "--mapping-dialog-mobile-max-height": `calc(88dvh - ${mappingDialogKeyboardInset.value}px)`,
}));
const mappingDialogScrollStyle = computed(() => ({
  scrollPaddingTop: "96px",
  scrollPaddingBottom: `${Math.max(mappingDialogKeyboardInset.value, 96)}px`,
}));
const sendProxyHeadersModel = computed({
  get: () => sendProxyHeaders.value,
  set: (value: boolean) => {
    sendProxyHeadersTouched.value = true;
    sendProxyHeaders.value = value;
  },
});
const preserveHostModel = computed({
  get: () => preserveHost.value,
  set: (value: boolean) => {
    preserveHostTouched.value = true;
    preserveHost.value = value;
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
const mappingModeDescription = computed(() => {
  if (mappingInputMode.value === "subdomain" && canUseRootDomainSuffix.value) {
    return `只填写前缀，保存时会自动拼接 .${savedRootDomain.value}。`;
  }

  if (canUseRootDomainSuffix.value) {
    return `输入完整 Host，不会自动拼接 .${savedRootDomain.value}。`;
  }

  if (!savedRootDomain.value) {
    return "保存根域名后可使用固定后缀；当前需要输入完整域名。";
  }

  return "根域名有未保存修改，保存后可使用固定后缀。";
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
const mappingDraftHost = computed(() => composedPreviewHost.value);
const mappingAdvancedHostLabel = computed(
  () => mappingDraftHost.value || "未填写 Host",
);
const mappingAdvancedTargetLabel = computed(
  () => mappingForm.target.trim() || "未填写目标",
);
const isGatewayAdvancedAvailableByMode = computed(() =>
  isAnySubdomainRoutingMode(configStore.config),
);
const gatewayProxyHeadersBlockedReason = computed(() => {
  if (isMappingAuthService.value) return "鉴权服务不参与协议头配置。";
  if (isLoadingGatewayProxyHeaders.value) return "正在加载协议头配置。";
  if (gatewayProxyHeadersLoadError.value) {
    return gatewayProxyHeadersLoadError.value;
  }
  if (gatewayProxyHeadersDetails.value) {
    return gatewayProxyHeadersDetails.value.availability.available
      ? ""
      : gatewayProxyHeadersDetails.value.availability.reason;
  }
  if (!isGatewayAdvancedAvailableByMode.value) {
    return "协议头仅可在子域映射模式下编辑。";
  }
  return "";
});
const gatewayHostResponseBlockedReason = computed(() => {
  if (isMappingAuthService.value) return "鉴权服务不参与 Host 响应配置。";
  if (isLoadingGatewayHostResponse.value) return "正在加载 Host 响应配置。";
  if (gatewayHostResponseLoadError.value) {
    return gatewayHostResponseLoadError.value;
  }
  if (gatewayHostResponseDetails.value) {
    return gatewayHostResponseDetails.value.availability.available
      ? ""
      : gatewayHostResponseDetails.value.availability.reason;
  }
  if (!isGatewayAdvancedAvailableByMode.value) {
    return "Host 响应仅可在子域映射模式下编辑。";
  }
  return "";
});
const mappingAdvancedSummary = computed(() => {
  const items = [
    mappingUseAuth.value ? "要求登录" : "公开访问",
    showToolbar.value ? "显示传送门" : "隐藏传送门",
  ];

  if (isMappingAuthService.value) {
    items.push("鉴权入口");
  } else {
    if (basicAuthInjectionModel.value) {
      items.push("注入凭证");
    }
    items.push(sendProxyHeaders.value ? "发送协议头" : "关闭协议头");
    items.push(preserveHost.value ? "保留 Host" : "使用上游 Host");
  }

  return items.join(" · ");
});
const mappingViewTransitionEnterActiveClass =
  "motion-safe:transition-[opacity,transform] motion-safe:duration-200 motion-safe:ease-out motion-safe:will-change-transform motion-reduce:transition-none";
const mappingViewTransitionLeaveActiveClass =
  "absolute inset-x-6 top-0 motion-safe:transition-[opacity,transform] motion-safe:duration-200 motion-safe:ease-out motion-safe:will-change-transform motion-reduce:hidden";
const mappingViewTransitionEnterFromClass = computed(() =>
  mappingDialogMotionDirection.value === "forward"
    ? "opacity-0 motion-safe:translate-x-6"
    : "opacity-0 motion-safe:-translate-x-6",
);
const mappingViewTransitionLeaveToClass = computed(() =>
  mappingDialogMotionDirection.value === "forward"
    ? "opacity-0 motion-safe:-translate-x-6"
    : "opacity-0 motion-safe:translate-x-6",
);
const defaultAuthServicePublicPort = computed(
  () => normalizePublicPort(accessEntryPort.value) || 7999,
);
const authServicePublicPort = computed({
  get: () => {
    const explicitHttpsPort = parsePublicAuthBaseUrlPort(
      modeForm.public_auth_base_url,
      "https",
    );
    const explicitHttpPort = parsePublicAuthBaseUrlPort(
      modeForm.public_auth_base_url,
      "http",
    );
    const configuredHttpsPort = normalizePublicPort(modeForm.public_https_port);
    const configuredHttpPort = normalizePublicPort(modeForm.public_http_port);
    return (
      explicitHttpsPort ||
      explicitHttpPort ||
      configuredHttpsPort ||
      configuredHttpPort ||
      defaultAuthServicePublicPort.value
    );
  },
  set: (value: number | string) => {
    const port = normalizePublicPort(value);
    modeForm.public_https_port = port || 0;
    modeForm.public_http_port = 0;
    modeForm.public_auth_base_url = syncPublicAuthBaseUrlPort(
      modeForm.public_auth_base_url,
      port,
    );
  },
});
const draftAuthServicePublicPort = computed(() =>
  String(authServicePublicPort.value || defaultAuthServicePublicPort.value),
);
const displayAccessEntryPort = computed(() => {
  const explicitHttpsPort = parsePublicAuthBaseUrlPort(
    currentModeConfig.value.public_auth_base_url,
    "https",
  );
  const explicitHttpPort = parsePublicAuthBaseUrlPort(
    currentModeConfig.value.public_auth_base_url,
    "http",
  );
  const configuredHttpsPort = normalizePublicPort(
    currentModeConfig.value.public_https_port,
  );
  const configuredHttpPort = normalizePublicPort(
    currentModeConfig.value.public_http_port,
  );
  const configuredPort =
    explicitHttpsPort ||
    configuredHttpsPort ||
    explicitHttpPort ||
    configuredHttpPort;
  return configuredPort > 0
    ? String(configuredPort)
    : accessEntryPort.value.trim() || "7999";
});
const isEdgeClientIPModeEditable = computed(
  () => configStore.config?.run_type === 3,
);
const activeEdgeClientIpProvider = computed(() =>
  resolveEdgeClientIpProvider(modeForm),
);
const isEdgeClientIPActive = computed(
  () =>
    isEdgeClientIPModeEditable.value &&
    activeEdgeClientIpProvider.value !== null,
);
const shouldOmitAccessEntryPort = computed(() => {
  if (isEdgeClientIPActive.value) {
    return true;
  }
  const parsedPort = Number.parseInt(displayAccessEntryPort.value, 10);
  return parsedPort === 80 || parsedPort === 443;
});
const formatHostWithAccessEntryPort = (host: string): string =>
  shouldOmitAccessEntryPort.value
    ? host
    : `${host}:${displayAccessEntryPort.value}`;
const shouldOmitDraftAuthServicePublicPort = computed(() => {
  if (isEdgeClientIPActive.value) {
    return true;
  }
  const parsedPort = normalizePublicPort(authServicePublicPort.value);
  return parsedPort === 80 || parsedPort === 443;
});
const formatAuthServiceHostWithPublicPort = (host: string): string =>
  shouldOmitDraftAuthServicePublicPort.value
    ? host
    : `${host}:${draftAuthServicePublicPort.value}`;
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
const hostTrafficSamples = computed(() => {
  const samples = new Map<string, HostTrafficStats>();
  for (const item of trafficRealtimeStats.value?.by_host ?? []) {
    const host = normalizeHostLike(item.host);
    if (!host) continue;
    samples.set(host, item);
  }
  return samples;
});
const getHostTrafficSample = (host: string): HostTrafficStats | null =>
  hostTrafficSamples.value.get(normalizeHostLike(host)) ?? null;
const visibleMappingsSignature = computed(() =>
  visibleMappings.value
    .map(
      (mapping) =>
        `${normalizeHostLike(mapping.host)}::${mapping.target.trim()}`,
    )
    .join("|"),
);
const hasProtocolHeadersSensitiveMappings = computed(() =>
  visibleMappings.value.some(
    (mapping) => parseTargetPort(mapping.target) === HOME_ASSISTANT_TARGET_PORT,
  ),
);
const listedGatewayProxyHeaderTargets = computed(() => {
  const targets = new Set<string>();

  for (const item of gatewayProxyHeadersDetails.value?.items ?? []) {
    const target = item.target.trim();
    if (target) {
      targets.add(target);
    }
  }

  return targets;
});
const disabledGatewayProxyHeaderTargets = computed(() => {
  const targets = new Set<string>();
  const disabledHosts = new Set(
    (configStore.config?.gateway_proxy_headers?.disabled_hosts ?? []).map(
      normalizeHostLike,
    ),
  );

  for (const mapping of visibleMappings.value) {
    const target = mapping.target.trim();
    if (target && disabledHosts.has(normalizeHostLike(mapping.host))) {
      targets.add(target);
    }
  }

  if (gatewayProxyHeadersDetails.value) {
    for (const item of gatewayProxyHeadersDetails.value.items) {
      const target = item.target.trim();
      if (target && item.send_proxy_headers === false) {
        targets.add(target);
      }
    }
    return targets;
  }

  return targets;
});
const shouldShowProtocolHeadersWarning = (mapping: HostMapping): boolean => {
  const target = mapping.target.trim();
  if (!target || parseTargetPort(target) !== HOME_ASSISTANT_TARGET_PORT) {
    return false;
  }

  if (
    gatewayProxyHeadersDetails.value &&
    !listedGatewayProxyHeaderTargets.value.has(target)
  ) {
    return false;
  }

  return !disabledGatewayProxyHeaderTargets.value.has(target);
};
const isProtocolHeadersWarningOpen = (host: string): boolean =>
  openProtocolHeadersWarningHost.value === host;

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
  const host = mappingDraftHost.value;
  const target = mappingForm.target.trim();

  if (!host || !target) return false;
  if (mappingInputMode.value === "subdomain" && !canUseRootDomainSuffix.value) {
    return false;
  }

  try {
    const parsed = new URL(target);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return false;
    }
    return !basicAuthValidationMessage.value;
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
  modeForm.edge_client_ip_enabled = next.edge_client_ip_enabled;
  modeForm.aliyun_esa_enabled = next.aliyun_esa_enabled;
  modeForm.tencent_edgeone_enabled = next.tencent_edgeone_enabled;
  modeForm.public_auth_base_url = next.public_auth_base_url;
  modeForm.public_http_port = normalizePublicPort(next.public_http_port);
  modeForm.public_https_port = normalizePublicPort(next.public_https_port);
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
  () =>
    [
      modeForm.edge_client_ip_enabled,
      modeForm.aliyun_esa_enabled,
      modeForm.tencent_edgeone_enabled,
    ] as const,
  ([enabled, aliyunEnabled, tencentEnabled]) => {
    if (!enabled) {
      if (modeForm.aliyun_esa_enabled) {
        modeForm.aliyun_esa_enabled = false;
      }
      if (modeForm.tencent_edgeone_enabled) {
        modeForm.tencent_edgeone_enabled = false;
      }
      return;
    }

    if (tencentEnabled && aliyunEnabled) {
      modeForm.aliyun_esa_enabled = false;
      return;
    }

    if (!aliyunEnabled && !tencentEnabled) {
      modeForm.aliyun_esa_enabled = true;
    }
  },
);

watch(
  filteredMappings,
  () => {
    syncDraggableVisibleMappings();
  },
  { immediate: true },
);

watch(
  visibleMappingsSignature,
  () => {
    void loadGatewayProxyHeadersDetails();
  },
  { immediate: true },
);

watch(
  [mappingDraftHost, gatewayProxyHeadersDetails, gatewayHostResponseDetails],
  () => {
    if (isDialogOpen.value) {
      applyMappingGatewayDraftFromConfig();
    }
  },
);

watch(
  [() => isDialogOpen.value, basicAuthProbeTargetKey, isMappingAuthService],
  () => {
    scheduleBasicAuthProbe();
  },
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
const showDiscoverHostColumn = computed(() => {
  const hosts = new Set(
    (discoveredData.value?.services || [])
      .map((service) => service.host?.trim())
      .filter(Boolean),
  );
  return hosts.size > 1;
});

function updateInteractionMode() {
  if (typeof window === "undefined") {
    return;
  }

  isTouchInteraction.value = window.matchMedia(
    "(hover: none), (pointer: coarse)",
  ).matches;
}

function handleLocationRulesTooltipOpenChange(
  host: string,
  nextOpen: boolean,
) {
  if (nextOpen) {
    openLocationRulesTooltipHost.value = host;
    return;
  }

  if (openLocationRulesTooltipHost.value === host) {
    openLocationRulesTooltipHost.value = null;
  }
}

function handleLocationRulesTooltipTriggerClick(host: string) {
  if (!isTouchInteraction.value) {
    return;
  }

  openLocationRulesTooltipHost.value =
    openLocationRulesTooltipHost.value === host ? null : host;
}

onMounted(async () => {
  interactionMediaQuery = window.matchMedia(
    "(hover: none), (pointer: coarse)",
  );
  updateInteractionMode();
  if (typeof interactionMediaQuery.addEventListener === "function") {
    interactionMediaQuery.addEventListener("change", updateInteractionMode);
  } else {
    interactionMediaQuery.addListener(updateInteractionMode);
  }

  window.visualViewport?.addEventListener(
    "resize",
    handleMappingDialogViewportResize,
  );
  window.visualViewport?.addEventListener(
    "scroll",
    handleMappingDialogViewportResize,
  );
  if (!configStore.config) {
    await configStore.loadConfig();
  }
  void loadAccessEntryPort();
  startTrafficRealtimePolling();
});

onUnmounted(() => {
  window.visualViewport?.removeEventListener(
    "resize",
    handleMappingDialogViewportResize,
  );
  window.visualViewport?.removeEventListener(
    "scroll",
    handleMappingDialogViewportResize,
  );
  clearMappingDialogKeyboardScrollTimer();
  clearBasicAuthProbeTimer();
  if (interactionMediaQuery) {
    if (typeof interactionMediaQuery.removeEventListener === "function") {
      interactionMediaQuery.removeEventListener("change", updateInteractionMode);
    } else {
      interactionMediaQuery.removeListener(updateInteractionMode);
    }
    interactionMediaQuery = null;
  }
  basicAuthProbeRequestId += 1;
  stopTrafficRealtimePolling();
});

async function loadAccessEntryPort() {
  try {
    const info = await SystemAPI.getAccessEntry();
    accessEntryPort.value = info.port.trim() || "7999";
  } catch (error) {
    console.warn("load access entry port failed:", error);
  }
}

async function loadTrafficRealtime() {
  if (isTrafficRealtimeLoading) return;
  isTrafficRealtimeLoading = true;
  try {
    trafficRealtimeStats.value = await DashboardAPI.getRealtime();
  } catch (error) {
    console.warn("load host traffic realtime failed:", error);
  } finally {
    isTrafficRealtimeLoading = false;
  }
}

function startTrafficRealtimePolling() {
  stopTrafficRealtimePolling();
  void loadTrafficRealtime();
  trafficRealtimeTimer = window.setInterval(() => {
    void loadTrafficRealtime();
  }, 1000);
}

function stopTrafficRealtimePolling() {
  if (trafficRealtimeTimer !== null) {
    window.clearInterval(trafficRealtimeTimer);
    trafficRealtimeTimer = null;
  }
}

let protocolHeadersWarningCloseTimer: number | null = null;

const clearProtocolHeadersWarningCloseTimer = () => {
  if (protocolHeadersWarningCloseTimer !== null) {
    window.clearTimeout(protocolHeadersWarningCloseTimer);
    protocolHeadersWarningCloseTimer = null;
  }
};

function openProtocolHeadersWarning(host: string) {
  clearProtocolHeadersWarningCloseTimer();
  openProtocolHeadersWarningHost.value = host;
}

function scheduleCloseProtocolHeadersWarning(host: string) {
  if (openProtocolHeadersWarningHost.value !== host) {
    return;
  }

  clearProtocolHeadersWarningCloseTimer();
  protocolHeadersWarningCloseTimer = window.setTimeout(() => {
    if (openProtocolHeadersWarningHost.value === host) {
      openProtocolHeadersWarningHost.value = null;
    }
    protocolHeadersWarningCloseTimer = null;
  }, 120);
}

function toggleProtocolHeadersWarning(host: string) {
  clearProtocolHeadersWarningCloseTimer();
  openProtocolHeadersWarningHost.value =
    openProtocolHeadersWarningHost.value === host ? null : host;
}

function handleProtocolHeadersWarningOpenChange(
  host: string,
  nextOpen: boolean,
) {
  clearProtocolHeadersWarningCloseTimer();

  if (nextOpen) {
    openProtocolHeadersWarningHost.value = host;
    return;
  }

  if (openProtocolHeadersWarningHost.value === host) {
    openProtocolHeadersWarningHost.value = null;
  }
}

const normalizeDisabledHosts = (
  hosts: string[] | undefined | null,
): string[] => [
  ...new Set((hosts ?? []).map(normalizeHostLike).filter(Boolean)),
];

const hasSameDisabledHosts = (left: string[], right: string[]): boolean => {
  const leftHosts = normalizeDisabledHosts(left);
  const rightHosts = normalizeDisabledHosts(right);
  return (
    leftHosts.length === rightHosts.length &&
    leftHosts.every((host, index) => host === rightHosts[index])
  );
};

function cancelGatewayProxyHeadersLoad() {
  gatewayProxyHeadersRequestId += 1;
  isLoadingGatewayProxyHeaders.value = false;
}

function cancelGatewayHostResponseLoad() {
  gatewayHostResponseRequestId += 1;
  isLoadingGatewayHostResponse.value = false;
}

const resolveSendProxyHeadersForHost = (host: string): boolean => {
  const normalizedHost = normalizeHostLike(host);
  if (!normalizedHost) return true;

  const disabledHosts = new Set(
    normalizeDisabledHosts(
      gatewayProxyHeadersDetails.value?.config.disabled_hosts ??
        configStore.config?.gateway_proxy_headers?.disabled_hosts,
    ),
  );
  return !disabledHosts.has(normalizedHost);
};

const resolvePreserveHostForHost = (host: string): boolean => {
  const normalizedHost = normalizeHostLike(host);
  if (!normalizedHost) return true;

  const disabledHosts = new Set(
    normalizeDisabledHosts(
      gatewayHostResponseDetails.value?.config.disabled_hosts ??
        configStore.config?.gateway_host_response?.disabled_hosts,
    ),
  );
  return !disabledHosts.has(normalizedHost);
};

function applyGatewayProxyHeadersDetails(details: GatewayProxyHeadersDetails) {
  gatewayProxyHeadersDetails.value = details;
  if (configStore.config) {
    configStore.config = {
      ...configStore.config,
      gateway_proxy_headers: {
        disabled_hosts: [...details.config.disabled_hosts],
      },
    };
  }
  applyMappingGatewayDraftFromConfig();
}

function applyGatewayHostResponseDetails(details: GatewayHostResponseDetails) {
  gatewayHostResponseDetails.value = details;
  if (configStore.config) {
    configStore.config = {
      ...configStore.config,
      gateway_host_response: {
        disabled_hosts: [...details.config.disabled_hosts],
      },
    };
  }
  applyMappingGatewayDraftFromConfig();
}

function applyMappingGatewayDraftFromConfig(host = mappingDraftHost.value) {
  const normalizedHost = normalizeHostLike(host);
  if (!sendProxyHeadersTouched.value) {
    sendProxyHeaders.value = resolveSendProxyHeadersForHost(normalizedHost);
  }
  if (!preserveHostTouched.value) {
    preserveHost.value = resolvePreserveHostForHost(normalizedHost);
  }
}

function resetMappingAdvancedState(host = "") {
  mappingDialogView.value = "basic";
  mappingDialogMotionDirection.value = "forward";
  mappingAdvancedCleanupHosts.value = [];
  sendProxyHeadersTouched.value = false;
  preserveHostTouched.value = false;
  sendProxyHeaders.value = resolveSendProxyHeadersForHost(host);
  preserveHost.value = resolvePreserveHostForHost(host);
  gatewayProxyHeadersLoadError.value = "";
  gatewayHostResponseLoadError.value = "";
}

function setBasicAuthProbeCacheResult(
  target: string,
  result: HostMappingBasicAuthProbeResult,
) {
  const next = new Map(basicAuthProbeCache.value);
  next.set(target, result);
  basicAuthProbeCache.value = next;
}

function clearBasicAuthProbeTimer() {
  if (basicAuthProbeTimer === null) return;
  window.clearTimeout(basicAuthProbeTimer);
  basicAuthProbeTimer = null;
}

function clearMappingDialogKeyboardScrollTimer() {
  if (mappingDialogKeyboardScrollTimer === null) return;
  window.clearTimeout(mappingDialogKeyboardScrollTimer);
  mappingDialogKeyboardScrollTimer = null;
}

function resolveMappingDialogKeyboardInset(): number {
  const viewport = window.visualViewport;
  if (!viewport) return 0;
  const inset = window.innerHeight - viewport.height - viewport.offsetTop;
  return inset > 80 ? Math.ceil(inset) : 0;
}

function updateMappingDialogKeyboardInset() {
  mappingDialogKeyboardInset.value = isDialogOpen.value
    ? resolveMappingDialogKeyboardInset()
    : 0;
}

function isMappingDialogKeyboardInput(
  target: Element | null,
): target is HTMLElement {
  if (!(target instanceof HTMLElement)) return false;
  const tagName = target.tagName.toLowerCase();
  if (tagName !== "input" && tagName !== "textarea") return false;
  return mappingDialogScrollRef.value?.contains(target) === true;
}

function scrollMappingDialogInputIntoView(
  target: HTMLElement,
  behavior: ScrollBehavior = "smooth",
) {
  updateMappingDialogKeyboardInset();

  const container = mappingDialogScrollRef.value;
  if (!container) {
    target.scrollIntoView({ block: "center", inline: "nearest", behavior });
    return;
  }

  const targetRect = target.getBoundingClientRect();
  const containerRect = container.getBoundingClientRect();
  const viewport = window.visualViewport;
  const viewportTop = viewport?.offsetTop ?? 0;
  const viewportBottom = viewport
    ? viewport.offsetTop + viewport.height
    : window.innerHeight;
  const visibleTop = Math.max(containerRect.top, viewportTop + 12);
  const visibleBottom = Math.min(containerRect.bottom, viewportBottom - 16);
  const visibleHeight = visibleBottom - visibleTop;

  if (visibleHeight <= 0) {
    target.scrollIntoView({ block: "center", inline: "nearest", behavior });
    return;
  }

  const desiredCenter = visibleTop + visibleHeight / 2;
  const targetCenter = targetRect.top + targetRect.height / 2;
  const maxScrollTop = Math.max(
    0,
    container.scrollHeight - container.clientHeight,
  );
  const nextScrollTop = Math.min(
    maxScrollTop,
    Math.max(0, container.scrollTop + targetCenter - desiredCenter),
  );

  container.scrollTo({
    top: nextScrollTop,
    behavior,
  });

  window.setTimeout(() => {
    target.scrollIntoView({
      block: "center",
      inline: "nearest",
      behavior,
    });
  }, 0);
}

function scheduleMappingDialogInputScrollIntoView(target: HTMLElement) {
  clearMappingDialogKeyboardScrollTimer();

  let attempts = 0;
  const run = () => {
    scrollMappingDialogInputIntoView(target, attempts === 0 ? "auto" : "smooth");
    attempts += 1;
    if (attempts >= 4) {
      mappingDialogKeyboardScrollTimer = null;
      return;
    }
    mappingDialogKeyboardScrollTimer = window.setTimeout(
      run,
      attempts === 1 ? 120 : 240,
    );
  };

  run();
}

function handleMappingDialogFocusIn(event: FocusEvent) {
  const target = event.target as Element | null;
  if (!isMappingDialogKeyboardInput(target)) return;
  scheduleMappingDialogInputScrollIntoView(target);
}

function handleMappingDialogViewportResize() {
  updateMappingDialogKeyboardInset();
  if (!isDialogOpen.value) return;
  const activeElement = document.activeElement;
  if (!isMappingDialogKeyboardInput(activeElement)) return;

  scheduleMappingDialogInputScrollIntoView(activeElement);
}

async function runBasicAuthProbe(target: string) {
  const normalizedTarget = normalizeBasicAuthProbeTarget(target);
  if (!normalizedTarget) {
    isLoadingBasicAuthProbe.value = false;
    return;
  }
  if (basicAuthProbeCache.value.has(normalizedTarget)) {
    isLoadingBasicAuthProbe.value = false;
    return;
  }

  const requestId = ++basicAuthProbeRequestId;
  isLoadingBasicAuthProbe.value = true;

  try {
    const result =
      await ConfigAPI.probeHostMappingBasicAuth(normalizedTarget);
    setBasicAuthProbeCacheResult(normalizedTarget, result);
  } catch (error) {
    setBasicAuthProbeCacheResult(normalizedTarget, {
      requiresBasicAuth: false,
      httpStatus: null,
      error: extractErrorMessage(error, "Basic Auth 探测失败"),
    });
  } finally {
    if (
      requestId === basicAuthProbeRequestId &&
      basicAuthProbeTargetKey.value === normalizedTarget
    ) {
      isLoadingBasicAuthProbe.value = false;
    }
  }
}

function scheduleBasicAuthProbe() {
  clearBasicAuthProbeTimer();

  const target = basicAuthProbeTargetKey.value;
  if (
    !isDialogOpen.value ||
    isMappingAuthService.value ||
    !target ||
    basicAuthProbeCache.value.has(target)
  ) {
    basicAuthProbeRequestId += 1;
    isLoadingBasicAuthProbe.value = false;
    return;
  }

  isLoadingBasicAuthProbe.value = true;
  basicAuthProbeTimer = window.setTimeout(() => {
    basicAuthProbeTimer = null;
    void runBasicAuthProbe(target);
  }, 450);
}

function openMappingAdvancedView() {
  mappingDialogMotionDirection.value = "forward";
  mappingDialogView.value = "advanced";
  void loadGatewayAdvancedDetails();
}

function returnMappingBasicView() {
  mappingDialogMotionDirection.value = "back";
  mappingDialogView.value = "basic";
}

function addMappingAdvancedCleanupHost(host: string | null) {
  const normalizedHost = host ? normalizeHostLike(host) : "";
  if (!normalizedHost) return;
  if (mappingAdvancedCleanupHosts.value.includes(normalizedHost)) return;
  mappingAdvancedCleanupHosts.value = [
    ...mappingAdvancedCleanupHosts.value,
    normalizedHost,
  ];
}

const collectMappingAdvancedCleanupHosts = (
  previousHost: string | null,
): string[] =>
  normalizeDisabledHosts([
    ...mappingAdvancedCleanupHosts.value,
    ...(previousHost ? [previousHost] : []),
  ]);

async function loadGatewayProxyHeadersDetails(
  options: { force?: boolean; trackLoading?: boolean } = {},
) {
  const requestId = ++gatewayProxyHeadersRequestId;

  if (!options.force && !hasProtocolHeadersSensitiveMappings.value) {
    gatewayProxyHeadersDetails.value = null;
    return;
  }

  if (options.trackLoading) {
    isLoadingGatewayProxyHeaders.value = true;
    gatewayProxyHeadersLoadError.value = "";
  }

  try {
    const details = await ConfigAPI.getGatewayProxyHeaders();
    if (requestId !== gatewayProxyHeadersRequestId) {
      return;
    }
    applyGatewayProxyHeadersDetails(details);
  } catch (error) {
    if (requestId !== gatewayProxyHeadersRequestId) {
      return;
    }
    if (options.trackLoading) {
      gatewayProxyHeadersLoadError.value = extractErrorMessage(
        error,
        "加载协议头配置失败",
      );
    }
    console.warn("load gateway proxy headers failed:", error);
  } finally {
    if (options.trackLoading && requestId === gatewayProxyHeadersRequestId) {
      isLoadingGatewayProxyHeaders.value = false;
    }
  }
}

async function loadGatewayHostResponseDetails(
  options: { trackLoading?: boolean } = {},
) {
  const requestId = ++gatewayHostResponseRequestId;

  if (options.trackLoading) {
    isLoadingGatewayHostResponse.value = true;
    gatewayHostResponseLoadError.value = "";
  }

  try {
    const details = await ConfigAPI.getGatewayHostResponse();
    if (requestId !== gatewayHostResponseRequestId) {
      return;
    }
    applyGatewayHostResponseDetails(details);
  } catch (error) {
    if (requestId !== gatewayHostResponseRequestId) {
      return;
    }
    if (options.trackLoading) {
      gatewayHostResponseLoadError.value = extractErrorMessage(
        error,
        "加载 Host 响应配置失败",
      );
    }
    console.warn("load gateway host response failed:", error);
  } finally {
    if (options.trackLoading && requestId === gatewayHostResponseRequestId) {
      isLoadingGatewayHostResponse.value = false;
    }
  }
}

async function loadGatewayAdvancedDetails() {
  await Promise.all([
    loadGatewayProxyHeadersDetails({ force: true, trackLoading: true }),
    loadGatewayHostResponseDetails({ trackLoading: true }),
  ]);
}

function resetModeForm() {
  applyModeForm(currentModeConfig.value);
}

function selectEdgeClientIpProvider(provider: EdgeClientIpProvider) {
  if (!isEdgeClientIPModeEditable.value) return;

  modeForm.edge_client_ip_enabled = true;
  modeForm.aliyun_esa_enabled = provider === "aliyun_esa";
  modeForm.tencent_edgeone_enabled = provider === "tencent_edgeone";
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

function handleMappingInputModeChange(nextMode: MappingInputMode) {
  setMappingInputMode(nextMode);
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
      edge_client_ip_enabled: modeForm.edge_client_ip_enabled,
      aliyun_esa_enabled: modeForm.aliyun_esa_enabled,
      tencent_edgeone_enabled: modeForm.tencent_edgeone_enabled,
      public_auth_base_url: modeForm.public_auth_base_url.trim(),
      public_http_port: normalizePublicPort(modeForm.public_http_port),
      public_https_port: normalizePublicPort(modeForm.public_https_port),
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
  resetMappingAdvancedState("");
  isDialogOpen.value = true;
  void loadGatewayAdvancedDetails();
}

function openEditDialog(mapping: HostMapping) {
  editingHost.value = mapping.host;
  const editorState = resolveMappingEditorState(
    mapping.host,
    canUseRootDomainSuffix.value ? savedRootDomain.value : "",
  );
  mappingInputMode.value = editorState.mode;
  mappingSubdomain.value = editorState.value;

  Object.assign(mappingForm, {
    ...mapping,
    basic_auth: normalizeMappingBasicAuth(mapping.basic_auth),
  });
  mappingMetadataTarget.value = mapping.target.trim();
  resetMappingAdvancedState(mapping.host);
  isDialogOpen.value = true;
  void loadGatewayAdvancedDetails();
}

function closeDialog() {
  clearMappingDialogKeyboardScrollTimer();
  mappingDialogKeyboardInset.value = 0;
  isDialogOpen.value = false;
  editingHost.value = null;
  mappingInputMode.value = canUseRootDomainSuffix.value
    ? "subdomain"
    : "full_host";
  mappingSubdomain.value = "";
  mappingMetadataTarget.value = "";
  Object.assign(mappingForm, createDefaultMapping());
  resetMappingAdvancedState("");
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
  const host = mappingDraftHost.value;
  const basicAuth =
    serviceRole === "auth"
      ? createDisabledMappingBasicAuth()
      : normalizeMappingBasicAuth(input.basic_auth);

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
    basic_auth: basicAuth.enabled
      ? basicAuth
      : createDisabledMappingBasicAuth(),
    locations: serviceRole === "auth" ? [] : [...(input.locations ?? [])],
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

async function copyMappingHost(mapping: HostMapping) {
  const host = formatHostWithAccessEntryPort(mapping.host);
  if (!host) return;

  try {
    const result = await copyTextToClipboard(host);
    if (result.verified) {
      toast.success("域名已复制", { description: host });
      return;
    }

    toast.info("已尝试复制域名", {
      description: host,
    });
  } catch {
    toast.error("复制域名失败", {
      description: "当前页面可能运行在受限环境中，请手动复制。",
    });
  }
}

function openGatewayLocations(host: string) {
  void router.push({
    path: "/system/gateway-locations",
    query: { host },
  });
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

function getMappingMetadataBasicAuth(): HostMapping["basic_auth"] | null {
  if (!basicAuthInjectionModel.value || basicAuthValidationMessage.value) {
    return null;
  }

  const basicAuth = normalizeMappingBasicAuth(mappingForm.basic_auth);
  return basicAuth.enabled ? basicAuth : null;
}

async function refreshMappingMetadata() {
  if (!canRefreshMappingMetadata.value) return;

  await runRefreshMappingMetadata(
    () =>
      ConfigAPI.fetchHostMappingMetadata(
        mappingForm.target.trim(),
        getMappingMetadataBasicAuth(),
      ),
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
        preserve_host: true,
        basic_auth: createDisabledMappingBasicAuth(),
        locations: [],
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

const mergeGatewayDisabledHostsForMapping = (
  currentDisabledHosts: string[],
  previousHosts: string[],
  nextHost: string,
  enabledForNextHost: boolean,
): string[] => {
  const disabledHosts = new Set(normalizeDisabledHosts(currentDisabledHosts));
  const normalizedNextHost = normalizeHostLike(nextHost);

  for (const host of normalizeDisabledHosts(previousHosts)) {
    disabledHosts.delete(host);
  }

  if (normalizedNextHost) {
    if (enabledForNextHost) {
      disabledHosts.delete(normalizedNextHost);
    } else {
      disabledHosts.add(normalizedNextHost);
    }
  }

  return [...disabledHosts];
};

async function saveMappingGatewayAdvanced(
  normalized: HostMapping,
  previousHost: string | null,
) {
  const nextConfigHost =
    normalized.service_role === "auth" ? "" : normalized.host;
  const cleanupHosts = collectMappingAdvancedCleanupHosts(previousHost);
  const currentProxyDisabledHosts = normalizeDisabledHosts(
    gatewayProxyHeadersDetails.value?.config.disabled_hosts ??
      configStore.config?.gateway_proxy_headers?.disabled_hosts,
  );
  const currentHostResponseDisabledHosts = normalizeDisabledHosts(
    gatewayHostResponseDetails.value?.config.disabled_hosts ??
      configStore.config?.gateway_host_response?.disabled_hosts,
  );
  const nextProxyDisabledHosts = mergeGatewayDisabledHostsForMapping(
    currentProxyDisabledHosts,
    cleanupHosts,
    nextConfigHost,
    normalized.service_role === "auth" ? true : sendProxyHeaders.value,
  );
  const nextHostResponseDisabledHosts = mergeGatewayDisabledHostsForMapping(
    currentHostResponseDisabledHosts,
    cleanupHosts,
    nextConfigHost,
    normalized.service_role === "auth" ? true : preserveHost.value,
  );
  const shouldUpdateProxyHeaders = !hasSameDisabledHosts(
    currentProxyDisabledHosts,
    nextProxyDisabledHosts,
  );
  const shouldUpdateHostResponse = !hasSameDisabledHosts(
    currentHostResponseDisabledHosts,
    nextHostResponseDisabledHosts,
  );

  if (shouldUpdateProxyHeaders) {
    cancelGatewayProxyHeadersLoad();
    const details = await ConfigAPI.updateGatewayProxyHeaders({
      disabled_hosts: nextProxyDisabledHosts,
    });
    applyGatewayProxyHeadersDetails(details);
  }

  if (shouldUpdateHostResponse) {
    cancelGatewayHostResponseLoad();
    const details = await ConfigAPI.updateGatewayHostResponse({
      disabled_hosts: nextHostResponseDisabledHosts,
    });
    applyGatewayHostResponseDetails(details);
  }
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
    const previousHost = editingHost.value;
    const index = editingHost.value
      ? next.findIndex((item) => item.host === editingHost.value)
      : -1;

    if (index >= 0) {
      next[index] = normalized;
    } else {
      next.push(normalized);
    }

    await configStore.saveHostMappings(next);
    if (previousHost !== normalized.host) {
      addMappingAdvancedCleanupHost(previousHost);
    }
    editingHost.value = normalized.host;
    Object.assign(mappingForm, normalized);

    try {
      await saveMappingGatewayAdvanced(normalized, previousHost);
    } catch (error) {
      mappingDialogMotionDirection.value = "forward";
      mappingDialogView.value = "advanced";
      toast.error("Host 映射已保存，高级配置保存失败", {
        description: extractErrorMessage(error, "高级配置保存失败"),
      });
      return;
    }

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
  isDiscoverSettingsOpen.value = false;
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
    void nextTick().then(() => triggerScan());
  }
}

async function toggleDiscoverSettings() {
  isDiscoverSettingsOpen.value = !isDiscoverSettingsOpen.value;
  if (isDiscoverSettingsOpen.value) {
    await nextTick();
    void discoverTargetsSettingsRef.value?.loadTargets();
  }
}

async function triggerScan() {
  let targetCidrs: string[] | undefined;
  try {
    await nextTick();
    targetCidrs = await discoverTargetsSettingsRef.value?.ensureSaved();
  } catch {
    return;
  }

  resetSelection();
  await runDiscoverServices(
    () => ScanAPI.discover({ target_cidrs: targetCidrs }),
    {
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
    },
  );
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
  if (
    !isDiscoverSelectionValid.value ||
    !savedRootDomain.value ||
    !discoveredData.value
  ) {
    return;
  }

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
        target: `http://${resolveDiscoveredServiceHost(service)}:${service.port}/`,
        use_auth: service.detail.rule.use_auth,
        access_mode: DEFAULT_ACCESS_MODE,
        suppress_toolbar: false,
        preserve_host: true,
        basic_auth: createDisabledMappingBasicAuth(),
        locations: [],
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
