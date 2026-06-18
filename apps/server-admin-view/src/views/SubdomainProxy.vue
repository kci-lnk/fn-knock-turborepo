<template>
  <div class="space-y-6">
    <ConfigCollapsibleCard
      :title="t('admin.subdomainProxy.configTitle')"
      :configured="isSubdomainModeConfigured"
      :ready="!configStore.isLoading"
      :edit-label="t('admin.subdomainProxy.editConfig')"
      summary-class="text-xs text-muted-foreground truncate max-w-full"
      expanded-content-class="p-0 sm:p-0"
      actions-class="border-t bg-muted/30 px-4 py-4 sm:px-6 flex flex-col-reverse items-stretch gap-2 rounded-b-lg sm:flex-row sm:items-center sm:justify-end"
    >
      <template #summary>
        <template v-if="savedRootDomain">
          {{
            t("admin.subdomainProxy.rootDomainSummary", {
              domain: savedRootDomain,
            })
          }}
          <span v-if="authServiceMapping">
            ·
            {{
              t("admin.subdomainProxy.authServiceSummary", {
                host: authServiceMapping.host,
              })
            }}
          </span>
          <span v-else>
            · {{ t("admin.subdomainProxy.authServiceMissingSummary") }}
          </span>
          <span v-if="savedEdgeClientIpProviderLabel">
            · {{ savedEdgeClientIpProviderLabel }}
          </span>
        </template>
        <template v-else>{{
          t("admin.subdomainProxy.notConfiguredSummary")
        }}</template>
      </template>

      <template #default>
        <div class="divide-y divide-border">
          <div class="p-4 sm:p-6">
            <div class="space-y-1">
              <h3 class="text-base font-semibold">
                {{ t("admin.subdomainProxy.configTitle") }}
              </h3>
              <p class="text-sm text-muted-foreground">
                {{ t("admin.subdomainProxy.sectionDescription") }}
              </p>
            </div>
          </div>

          <div class="grid gap-4 p-4 sm:p-6">
            <div class="max-w-xs space-y-2">
              <Label for="root-domain">{{
                t("admin.subdomainProxy.domainLabel")
              }}</Label>
              <Input
                id="root-domain"
                v-model="modeForm.root_domain"
                placeholder="example.com"
              />
              <p class="text-xs text-muted-foreground">
                {{ t("admin.subdomainProxy.domainHint") }}
              </p>
            </div>
            <div class="rounded-lg border px-4 py-3">
              <div
                class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
              >
                <div class="space-y-1">
                  <Label>{{
                    t("admin.subdomainProxy.currentAuthService")
                  }}</Label>
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
                        {{
                          t("admin.subdomainProxy.authRedirectHint", {
                            url: `https://${formatAuthServiceHostWithPublicPort(
                              authServiceMapping.host,
                            )}`,
                          })
                        }}
                      </div>
                    </template>
                    <p v-else class="text-muted-foreground">
                      {{ t("admin.subdomainProxy.noAuthService") }}
                    </p>
                  </div>
                </div>

                <div class="flex flex-col items-end gap-2">
                  <Badge
                    :variant="authServiceMapping ? 'secondary' : 'outline'"
                  >
                    {{
                      authServiceMapping
                        ? t("admin.subdomainProxy.configured")
                        : t("admin.subdomainProxy.notConfigured")
                    }}
                  </Badge>

                  <ConfirmDangerPopover
                    v-if="authServiceMapping"
                    :title="t('admin.subdomainProxy.deleteAuthTitle')"
                    :description="
                      t('admin.subdomainProxy.deleteAuthDescription', {
                        host: authServiceMapping.host,
                      })
                    "
                    :confirm-text="t('admin.subdomainProxy.deleteAuthAction')"
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
                        {{ t("admin.subdomainProxy.deleteAuthAction") }}
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
                    {{ t("admin.subdomainProxy.authServicePort") }}
                  </Label>
                  <p class="text-xs leading-5 text-muted-foreground">
                    {{ t("admin.subdomainProxy.authServicePortHint") }}
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
                    <Label for="edge-client-ip-enabled">{{
                      t("admin.subdomainProxy.edgeClientIpTitle")
                    }}</Label>
                    <p class="text-xs text-muted-foreground">
                      {{ t("admin.subdomainProxy.edgeClientIpDescription") }}
                    </p>
                    <p class="text-xs text-muted-foreground">
                      {{
                        t(
                          "admin.subdomainProxy.edgeClientIpProviderDescription",
                        )
                      }}
                    </p>
                    <p
                      v-if="!isEdgeClientIPModeEditable"
                      class="text-xs text-amber-600"
                    >
                      {{ t("admin.subdomainProxy.edgeClientIpNotEditable") }}
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
                              ? t("admin.subdomainProxy.current")
                              : t("admin.subdomainProxy.switch")
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
        <Button variant="outline" @click="collapse">{{
          t("admin.subdomainProxy.collapse")
        }}</Button>
        <Button
          variant="outline"
          :disabled="isSavingMode || !isModeDirty"
          @click="resetModeForm"
        >
          {{ t("admin.subdomainProxy.discardChanges") }}
        </Button>
        <Button
          :disabled="isSavingMode || !isModeValid || !isModeDirty"
          @click="saveMode"
        >
          <span
            v-if="isSavingMode"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          {{ t("admin.subdomainProxy.saveConfig") }}
        </Button>
      </template>
    </ConfigCollapsibleCard>

    <Card>
      <CardHeader>
        <CardTitle class="flex items-center justify-between">
          <span>{{ t("admin.subdomainProxy.mappingsTitle") }}</span>
          <div class="flex items-center gap-2">
            <DocsLinkButton :href="docsUrls.guides.subdomainProxy" />
            <Button
              v-if="!authServiceMapping"
              :disabled="!canManageNewMappings || isSavingMappings"
              variant="default"
              @click="addAuthService"
            >
              <ShieldCheck class="mr-2 h-4 w-4" />
              {{ t("admin.subdomainProxy.addAuthService") }}
            </Button>
            <div v-if="authServiceMapping" class="flex items-center">
              <Button
                :variant="discoverButtonVariant"
                :disabled="!canManageNewMappings || isDiscovering"
                class="rounded-r-none"
                @click="openDiscoverDialog"
              >
                <Search class="mr-2 h-4 w-4" />
                {{
                  isDiscovering
                    ? t("admin.subdomainProxy.discovering")
                    : t("admin.subdomainProxy.discover")
                }}
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
                    {{ t("admin.subdomainProxy.clearAllConfig") }}
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    :disabled="
                      !hasRegularHostMappings ||
                      isSavingMappings ||
                      isClearingAllSubdomainConfig
                    "
                    @select="openStaleCleanupDialog"
                  >
                    <Eraser class="mr-2 h-4 w-4" />
                    {{ t("admin.subdomainProxy.cleanupStaleServices") }}
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    :disabled="configStore.isLoading"
                    @click="openCreateDialog"
                  >
                    <Plus class="mr-2 h-4 w-4" />
                    {{ t("admin.subdomainProxy.addMapping") }}
                  </DropdownMenuItem>
                  <DropdownMenuItem @click="syncRoutes" :disabled="isSyncing">
                    <RefreshCw
                      class="mr-2 h-4 w-4"
                      :class="{ 'animate-spin': isSyncing }"
                    />
                    {{
                      isSyncing
                        ? t("admin.subdomainProxy.syncing")
                        : t("admin.subdomainProxy.syncRoutes")
                    }}
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
                    {{
                      isRefreshingTitles
                        ? t("admin.subdomainProxy.refreshing")
                        : t("admin.subdomainProxy.refreshIconsTitles")
                    }}
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
                    {{
                      isExportingBookmarks
                        ? t("admin.subdomainProxy.exporting")
                        : t("admin.subdomainProxy.exportBookmarks")
                    }}
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
          </div>
        </CardTitle>
        <CardDescription>
          {{ t("admin.subdomainProxy.mappingsDescription") }}
        </CardDescription>
      </CardHeader>
      <CardContent class="space-y-4">
        <SearchInput
          v-model="searchQuery"
          :placeholder="t('admin.subdomainProxy.searchPlaceholder')"
          class="max-w-xs"
        />
        <p
          v-if="visibleMappings.length > 1"
          class="text-xs text-muted-foreground"
        >
          {{ t("admin.subdomainProxy.orderHintPrefix") }}
          <a
            href="#/system/gateway-proxy-headers"
            class="underline underline-offset-2 hover:text-foreground"
          >
            {{ t("admin.subdomainProxy.disableProxyHeaders") }} </a
          >{{ t("admin.subdomainProxy.orderHintMiddle") }}

          <a
            href="#/system/gateway-host-response"
            class="underline underline-offset-2 hover:text-foreground"
          >
            {{ t("admin.subdomainProxy.disableHostHeader") }}
          </a>
        </p>
        <p
          v-if="!savedRootDomain || isRootDomainPendingSave"
          class="text-xs text-amber-600"
        >
          {{
            !savedRootDomain
              ? t("admin.subdomainProxy.rootDomainRequired")
              : t("admin.subdomainProxy.rootDomainDirty")
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
                  {{ t("admin.subdomainProxy.columns.title") }}
                </TableHead>
                <TableHead>{{
                  t("admin.subdomainProxy.columns.domain")
                }}</TableHead>
                <TableHead>{{
                  t("admin.subdomainProxy.columns.target")
                }}</TableHead>
                <TableHead class="w-[7rem] min-w-[7rem] max-w-[7rem]">
                  {{ t("admin.subdomainProxy.columns.traffic") }}
                </TableHead>
                <TableHead class="w-[5.5rem] min-w-[5.5rem]">
                  {{ t("admin.subdomainProxy.columns.status") }}
                </TableHead>
                <TableHead class="text-right">{{
                  t("admin.subdomainProxy.columns.actions")
                }}</TableHead>
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
                  {{ t("admin.subdomainProxy.emptyMappings") }}
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
                    :aria-label="t('admin.subdomainProxy.dragSortAria')"
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
                          :aria-label="
                            t('admin.subdomainProxy.homeAssistantWarningAria', {
                              host: formatHostWithAccessEntryPort(mapping.host),
                            })
                          "
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
                                {{
                                  t(
                                    "admin.subdomainProxy.homeAssistantWarningTitle",
                                  )
                                }}
                              </p>
                            </div>
                            <p class="text-xs leading-5 text-muted-foreground">
                              {{
                                t(
                                  "admin.subdomainProxy.homeAssistantWarningDescription",
                                )
                              }}
                            </p>
                          </div>
                          <a
                            href="#/system/gateway-proxy-headers"
                            class="inline-flex rounded-md border border-destructive/20 bg-destructive/5 px-2.5 py-1.5 text-xs font-medium text-destructive transition hover:bg-destructive/10"
                          >
                            {{
                              t("admin.subdomainProxy.goDisableProtocolHeaders")
                            }}
                          </a>
                        </div>
                      </PopoverContent>
                    </Popover>
                    <div class="min-w-0 flex-1">
                      <InlineCommentEditor
                        :text="getMappingDisplayTitle(mapping)"
                        :placeholder="
                          t('admin.subdomainProxy.titlePlaceholder')
                        "
                        :empty-text="t('admin.subdomainProxy.notFetched')"
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
                    :title="
                      t('admin.subdomainProxy.copyHostTitle', {
                        host: formatHostWithAccessEntryPort(mapping.host),
                      })
                    "
                    :aria-label="
                      t('admin.subdomainProxy.copyHostAria', {
                        host: formatHostWithAccessEntryPort(mapping.host),
                      })
                    "
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
                      {{ t("admin.subdomainProxy.authServiceBadge") }}
                    </Badge>
                    <ShieldCheck
                      v-if="mapping.use_auth"
                      class="h-3.5 w-3.5 shrink-0"
                    />
                    <Badge v-else variant="secondary">{{
                      t("admin.subdomainProxy.publicAccess")
                    }}</Badge>
                    <PanelsTopLeft
                      v-if="
                        isGatewayPortalEnabled &&
                        mapping.use_auth &&
                        !mapping.suppress_toolbar &&
                        !isWebSocketProxyTargetUrl(mapping.target)
                      "
                      class="h-3.5 w-3.5 shrink-0"
                    />
                    <TooltipProvider v-if="getLocationRulesCount(mapping) > 0">
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
                            :aria-label="
                              t('admin.subdomainProxy.locationRulesAria', {
                                host: formatHostWithAccessEntryPort(
                                  mapping.host,
                                ),
                                count: getLocationRulesCount(mapping),
                              })
                            "
                            @click="
                              handleLocationRulesTooltipTriggerClick(
                                mapping.host,
                              )
                            "
                          >
                            <RouteIcon class="h-3.5 w-3.5" />
                          </button>
                        </TooltipTrigger>
                        <TooltipContent side="top" align="center">
                          <p>
                            {{
                              t("admin.subdomainProxy.locationRulesCount", {
                                count: getLocationRulesCount(mapping),
                              })
                            }}
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
                      {{ t("admin.subdomainProxy.edit") }}
                    </Button>
                    <Button
                      v-if="!isAuthServiceTarget(mapping.target)"
                      variant="ghost"
                      size="sm"
                      @click="openGatewayLocations(mapping.host)"
                    >
                      {{ t("admin.subdomainProxy.paths") }}
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                      :disabled="isSavingMappings"
                      @click="openDeleteMappingDialog(mapping.host)"
                    >
                      {{ t("admin.subdomainProxy.delete") }}
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
            :aria-label="t('admin.subdomainProxy.backToBasicAria')"
            @click="returnMappingBasicView"
          >
            <ChevronLeft class="h-4 w-4 shrink-0" />
            <span class="text-sm font-semibold">{{
              t("admin.subdomainProxy.advancedConfig")
            }}</span>
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
                  <Label for="mapping-display-title">{{
                    t("admin.subdomainProxy.displayTitle")
                  }}</Label>
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
                    {{
                      isRefreshingMappingMetadata
                        ? t("admin.subdomainProxy.refreshing")
                        : t("admin.subdomainProxy.refreshTitle")
                    }}
                  </Button>
                </div>
                <Input
                  id="mapping-display-title"
                  v-model="mappingForm.title_override"
                  :placeholder="t('admin.subdomainProxy.titleAutoPlaceholder')"
                />
                <p class="text-xs text-muted-foreground">
                  {{ t("admin.subdomainProxy.titleHelp") }}
                  <span v-if="mappingResolvedTitle">
                    {{
                      t("admin.subdomainProxy.fetchedTitle", {
                        title: mappingResolvedTitle,
                      })
                    }}
                  </span>
                  <span v-else-if="mappingForm.target.trim()">
                    {{ t("admin.subdomainProxy.noFetchedTitle") }}
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
                    :aria-label="t('admin.subdomainProxy.hostInputModeAria')"
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
                      {{ t("admin.subdomainProxy.fixedSuffix") }}
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
                      {{ t("admin.subdomainProxy.fullHost") }}
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
                    {{
                      t("admin.subdomainProxy.finalHost", {
                        host:
                          composedPreviewHost ||
                          t("admin.subdomainProxy.notFilled"),
                      })
                    }}
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
                <Label for="mapping-target">{{
                  t("admin.subdomainProxy.targetLabel")
                }}</Label>
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
                    <span class="block text-sm font-medium">{{
                      t("admin.subdomainProxy.advancedConfig")
                    }}</span>
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
                      <p class="text-xs text-muted-foreground">
                        {{ t("admin.subdomainProxy.targetLabel") }}
                      </p>
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
                    <Label for="mapping-auth">{{
                      t("admin.subdomainProxy.authRequired")
                    }}</Label>
                    <p class="text-xs leading-5 text-muted-foreground">
                      {{ t("admin.subdomainProxy.authRequiredDescription") }}
                    </p>
                  </div>
                  <Switch
                    id="mapping-auth"
                    v-model="mappingUseAuth"
                    :disabled="isMappingAuthService"
                  />
                </div>

                <div
                  v-if="!isMappingWebSocketTarget"
                  class="flex items-center justify-between gap-4 rounded-lg border px-4 py-3"
                >
                  <div class="min-w-0 space-y-1">
                    <Label for="mapping-toolbar">{{
                      t("admin.subdomainProxy.toolbar")
                    }}</Label>
                    <p class="text-xs leading-5 text-muted-foreground">
                      {{ t("admin.subdomainProxy.toolbarDescription") }}
                      <a
                        href="#/system/gateway-portal"
                        class="font-medium text-foreground underline underline-offset-4 transition hover:text-primary"
                      >
                        {{ t("admin.subdomainProxy.toolbarSettingsLink") }}
                      </a>
                      {{ t("admin.subdomainProxy.toolbarSettingsSuffix") }}
                    </p>
                  </div>
                  <TooltipProvider v-if="shouldShowPortalDisabledTooltip">
                    <Tooltip
                      :open="isPortalDisabledTooltipOpen"
                      @update:open="handlePortalDisabledTooltipOpenChange"
                    >
                      <TooltipTrigger as-child>
                        <span
                          class="inline-flex cursor-help rounded-full focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
                          tabindex="0"
                          @click="handlePortalDisabledTooltipTriggerClick"
                        >
                          <Switch
                            id="mapping-toolbar"
                            class="pointer-events-none"
                            :model-value="showToolbar"
                            disabled
                          />
                        </span>
                      </TooltipTrigger>
                      <TooltipContent side="top" align="end" class="max-w-xs">
                        <p>
                          {{
                            t("admin.subdomainProxy.portalDisabledDescription")
                          }}
                        </p>
                      </TooltipContent>
                    </Tooltip>
                  </TooltipProvider>
                  <Switch v-else id="mapping-toolbar" v-model="showToolbar" />
                </div>

                <div
                  v-if="canShowBasicAuthInjection"
                  class="space-y-3 rounded-lg border px-4 py-3"
                >
                  <div class="flex items-center justify-between gap-4">
                    <div class="min-w-0 space-y-1">
                      <Label for="mapping-basic-auth">{{
                        t("admin.subdomainProxy.basicAuthSkip")
                      }}</Label>
                      <p class="text-xs leading-5 text-muted-foreground">
                        {{ t("admin.subdomainProxy.basicAuthSkipDescription") }}
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
                      <Label for="mapping-basic-auth-username">{{
                        t("admin.subdomainProxy.username")
                      }}</Label>
                      <Input
                        id="mapping-basic-auth-username"
                        v-model="mappingForm.basic_auth.username"
                        autocomplete="username"
                        placeholder="admin"
                      />
                    </div>
                    <div class="space-y-2">
                      <Label for="mapping-basic-auth-password">{{
                        t("admin.subdomainProxy.password")
                      }}</Label>
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
                    <Label for="mapping-proxy-headers">{{
                      t("admin.subdomainProxy.proxyHeaders")
                    }}</Label>
                    <p class="text-xs leading-5 text-muted-foreground">
                      <template v-if="gatewayProxyHeadersBlockedReason">
                        {{ gatewayProxyHeadersBlockedReason }}
                      </template>
                      <template v-else>
                        {{ t("admin.subdomainProxy.proxyHeadersDescription") }}
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
                    <Label for="mapping-host-response">{{
                      t("admin.subdomainProxy.hostResponse")
                    }}</Label>
                    <p class="text-xs leading-5 text-muted-foreground">
                      <template v-if="gatewayHostResponseBlockedReason">
                        {{ gatewayHostResponseBlockedReason }}
                      </template>
                      <template v-else>
                        {{ t("admin.subdomainProxy.hostResponseDescription") }}
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
          <Button variant="outline" @click="closeDialog">{{
            t("admin.subdomainProxy.cancel")
          }}</Button>
          <Button
            :disabled="!isMappingValid || isSavingMappings"
            @click="saveMapping"
          >
            {{ t("admin.subdomainProxy.saveMapping") }}
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
          <Button variant="outline" @click="closeDeleteDialog">{{
            t("admin.subdomainProxy.cancel")
          }}</Button>
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
              <DialogTitle>{{
                t("admin.subdomainProxy.discoverTitle")
              }}</DialogTitle>
              <DialogDescription>
                {{
                  t("admin.subdomainProxy.discoverDescription", {
                    domain: savedRootDomain,
                  })
                }}
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
                {{
                  isDiscovering
                    ? t("admin.subdomainProxy.scanning")
                    : t("admin.subdomainProxy.refreshServices")
                }}
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
                {{ t("admin.subdomainProxy.probing") }}
              </p>
            </div>

            <div
              v-else-if="discoveredData && discoveredData.services.length === 0"
              class="text-center py-16 text-muted-foreground"
            >
              {{
                discoveredData.foundServices > 0
                  ? t("admin.subdomainProxy.discoverAllAdded")
                  : t("admin.subdomainProxy.discoverEmpty")
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
                      {{ t("admin.subdomainProxy.discoverColumns.host") }}
                    </TableHead>
                    <TableHead class="w-[80px]">{{
                      t("admin.subdomainProxy.discoverColumns.port")
                    }}</TableHead>
                    <TableHead class="w-[100px]">{{
                      t("admin.subdomainProxy.discoverColumns.status")
                    }}</TableHead>
                    <TableHead class="min-w-[10rem]">{{
                      t("admin.subdomainProxy.discoverColumns.serviceId")
                    }}</TableHead>
                    <TableHead class="w-[260px] min-w-[18rem]">
                      {{
                        t(
                          "admin.subdomainProxy.discoverColumns.suggestedSubdomain",
                        )
                      }}
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
                        {{ t("admin.subdomainProxy.authRequiredShort") }}
                      </span>
                      <span
                        v-else
                        class="text-green-600 bg-green-500/10 text-xs px-2 py-0.5 rounded"
                      >
                        {{ svc.httpStatus }}
                      </span>
                    </TableCell>
                    <TableCell class="min-w-[10rem] text-sm">
                      {{
                        svc.detail.label ||
                        svc.detail.name ||
                        t("admin.subdomainProxy.unknownService")
                      }}
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
              {{
                t("admin.subdomainProxy.discoveredScannedPorts", {
                  count: discoveredData.totalPortsScanned,
                })
              }}，{{
                t("admin.subdomainProxy.selectedItems", {
                  count: `${selectedServices.length} / ${discoveredData.services.length}`,
                })
              }}
              <template v-if="discoveredData.scanCidrs?.length">
                ，{{
                  t("admin.subdomainProxy.coveredCidrsHosts", {
                    cidrs: discoveredData.scanCidrs.length,
                    hosts:
                      discoveredData.scanHostCount ||
                      discoveredData.scannedHosts ||
                      0,
                  })
                }}
              </template>
              <template
                v-if="
                  !discoveredData.scanCidrs?.length &&
                  discoveredData.scannedHosts &&
                  discoveredData.scannedHosts > 1
                "
              >
                {{
                  t("admin.subdomainProxy.coveredHosts", {
                    hosts:
                      discoveredData.scanScope || discoveredData.scannedHosts,
                  })
                }}
              </template>
            </template>
          </span>
          <div class="space-x-2">
            <Button variant="outline" @click="dismissDiscoverDialog">
              {{ t("admin.subdomainProxy.cancel") }}
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
              {{ t("admin.subdomainProxy.addSelected") }}
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <StaleHostMappingsCleanupDialog
      ref="staleCleanupDialogRef"
      :mappings="allMappings"
      :save-mappings="saveHostMappingsForCleanup"
      :is-auth-service-target="isAuthServiceTarget"
    />
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
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import {
  CircleAlert,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Download,
  Eraser,
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
import StaleHostMappingsCleanupDialog from "@/components/StaleHostMappingsCleanupDialog.vue";
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
import {
  isHttpProxyTargetProtocol,
  isSupportedProxyTargetUrl,
  isWebSocketProxyTargetUrl,
} from "@admin-shared/utils/proxyTargetInput";
import { copyTextToClipboard } from "@admin-shared/utils/copyTextToClipboard";
import { useConfigStore } from "../store/config";
import {
  isAnySubdomainRoutingMode,
  shouldOmitPublicAccessEntryPort,
} from "../lib/reverse-proxy-submode";
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
const { t } = useI18n();
const discoverTargetsSettingsRef = ref<InstanceType<
  typeof ScanDiscoveryTargetsSettings
> | null>(null);
const staleCleanupDialogRef = ref<InstanceType<
  typeof StaleHostMappingsCleanupDialog
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

const edgeClientIpProviderOptions = computed<
  Array<{
    value: EdgeClientIpProvider;
    label: string;
    description: string;
    headerHint: string;
  }>
>(() => [
  {
    value: "tencent_edgeone",
    label: t("admin.subdomainProxy.edgeProviders.tencentEdgeOne.label"),
    description: t(
      "admin.subdomainProxy.edgeProviders.tencentEdgeOne.description",
    ),
    headerHint: t(
      "admin.subdomainProxy.edgeProviders.tencentEdgeOne.headerHint",
    ),
  },
  {
    value: "aliyun_esa",
    label: t("admin.subdomainProxy.edgeProviders.aliyunEsa.label"),
    description: t("admin.subdomainProxy.edgeProviders.aliyunEsa.description"),
    headerHint: t("admin.subdomainProxy.edgeProviders.aliyunEsa.headerHint"),
  },
]);

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
  if (provider === "tencent_edgeone")
    return t("admin.subdomainProxy.edgeProviders.tencentEdgeOne.label");
  if (provider === "aliyun_esa")
    return t("admin.subdomainProxy.edgeProviders.aliyunEsa.label");
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
    if (parsed.protocol === "https:" || parsed.protocol === "wss:") return 443;
    if (parsed.protocol === "http:" || parsed.protocol === "ws:") return 80;
  } catch {
    // ignore
  }

  return null;
};

const isHttpTargetUrl = (target: string): boolean => {
  try {
    const parsed = new URL(target.trim());
    return (
      isHttpProxyTargetProtocol(parsed.protocol) && Boolean(parsed.hostname)
    );
  } catch {
    return false;
  }
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
const isPortalDisabledTooltipOpen = ref(false);
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
  isHttpTargetUrl(target) && parseTargetPort(target) === authServicePort.value;
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
    ? t("admin.subdomainProxy.edgeRealIpSummary", {
        provider: getEdgeClientIpProviderLabel(savedEdgeClientIpProvider.value),
      })
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
const isGatewayPortalEnabled = computed(
  () => configStore.config?.gateway_portal?.enabled !== false,
);
const shouldShowPortalDisabledTooltip = computed(
  () => !isGatewayPortalEnabled.value,
);
const regularHostMappings = computed(() =>
  allMappings.value.filter((mapping) => !isAuthServiceTarget(mapping.target)),
);
const hasRegularHostMappings = computed(
  () => regularHostMappings.value.length > 0,
);
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
const isMappingWebSocketTarget = computed(() =>
  isWebSocketProxyTargetUrl(mappingForm.target),
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
  get: () => !isMappingWebSocketTarget.value && !mappingForm.suppress_toolbar,
  set: (value: boolean) => {
    if (isMappingWebSocketTarget.value) {
      mappingForm.suppress_toolbar = true;
      return;
    }
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
    return t("admin.subdomainProxy.basicAuthMissing");
  }
  if (username.includes(":")) {
    return t("admin.subdomainProxy.basicAuthUsernameColon");
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
    return t("admin.subdomainProxy.deleteAuthTitle");
  }

  if (target.kind === "clear_all") {
    return target.step === 1
      ? t("admin.subdomainProxy.clearAllTitle")
      : t("admin.subdomainProxy.clearAllSecondTitle");
  }

  return t("admin.subdomainProxy.deleteMappingTitle");
});
const deleteDialogDescription = computed(() => {
  const target = deleteDialogState.value;
  if (!target) return "";

  if (target.kind === "auth_service") {
    return t("admin.subdomainProxy.deleteAuthDescriptionPlain", {
      host: target.host,
    });
  }

  if (target.kind === "clear_all") {
    const mappingsCount = allMappings.value.length;
    return target.step === 1
      ? t("admin.subdomainProxy.clearAllDescriptionFirst", {
          count: mappingsCount,
        })
      : t("admin.subdomainProxy.clearAllDescriptionSecond");
  }

  return t("admin.subdomainProxy.deleteMappingDescription", {
    host: target.host,
  });
});
const deleteDialogConfirmLabel = computed(() => {
  const target = deleteDialogState.value;
  if (!target) return t("admin.subdomainProxy.confirm");

  if (target.kind === "auth_service") {
    return t("admin.subdomainProxy.deleteAuthAction");
  }

  if (target.kind === "clear_all") {
    return target.step === 1
      ? t("admin.subdomainProxy.continueConfirm")
      : t("admin.subdomainProxy.confirmClear");
  }

  return t("admin.subdomainProxy.deleteMapping");
});
const mappingModeDescription = computed(() => {
  if (mappingInputMode.value === "subdomain" && canUseRootDomainSuffix.value) {
    return t("admin.subdomainProxy.subdomainModeDescription", {
      domain: savedRootDomain.value,
    });
  }

  if (canUseRootDomainSuffix.value) {
    return t("admin.subdomainProxy.fullHostModeDescription", {
      domain: savedRootDomain.value,
    });
  }

  if (!savedRootDomain.value) {
    return t("admin.subdomainProxy.suffixAfterSavingRoot");
  }

  return t("admin.subdomainProxy.suffixAfterSavingChanges");
});
const mappingInputLabel = computed(() =>
  mappingInputMode.value === "subdomain"
    ? t("admin.subdomainProxy.subdomainPrefix")
    : t("admin.subdomainProxy.fullHost"),
);
const fullHostInputHint = computed(() => {
  if (canUseRootDomainSuffix.value) {
    return t("admin.subdomainProxy.fullHostInputHintWithRoot", {
      domain: savedRootDomain.value,
    });
  }

  return t("admin.subdomainProxy.fullHostInputHint");
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
  () => mappingDraftHost.value || t("admin.subdomainProxy.missingHost"),
);
const mappingAdvancedTargetLabel = computed(
  () => mappingForm.target.trim() || t("admin.subdomainProxy.missingTarget"),
);
const isGatewayAdvancedAvailableByMode = computed(() =>
  isAnySubdomainRoutingMode(configStore.config),
);
const gatewayProxyHeadersBlockedReason = computed(() => {
  if (isMappingAuthService.value)
    return t("admin.subdomainProxy.proxyHeadersAuthBlocked");
  if (isLoadingGatewayProxyHeaders.value)
    return t("admin.subdomainProxy.proxyHeadersLoading");
  if (gatewayProxyHeadersLoadError.value) {
    return gatewayProxyHeadersLoadError.value;
  }
  if (gatewayProxyHeadersDetails.value) {
    return gatewayProxyHeadersDetails.value.availability.available
      ? ""
      : gatewayProxyHeadersDetails.value.availability.reason;
  }
  if (!isGatewayAdvancedAvailableByMode.value) {
    return t("admin.subdomainProxy.proxyHeadersModeBlocked");
  }
  return "";
});
const gatewayHostResponseBlockedReason = computed(() => {
  if (isMappingAuthService.value)
    return t("admin.subdomainProxy.hostResponseAuthBlocked");
  if (isLoadingGatewayHostResponse.value)
    return t("admin.subdomainProxy.hostResponseLoading");
  if (gatewayHostResponseLoadError.value) {
    return gatewayHostResponseLoadError.value;
  }
  if (gatewayHostResponseDetails.value) {
    return gatewayHostResponseDetails.value.availability.available
      ? ""
      : gatewayHostResponseDetails.value.availability.reason;
  }
  if (!isGatewayAdvancedAvailableByMode.value) {
    return t("admin.subdomainProxy.hostResponseModeBlocked");
  }
  return "";
});
const mappingAdvancedSummary = computed(() => {
  const items = [
    mappingUseAuth.value
      ? t("admin.subdomainProxy.authRequired")
      : t("admin.subdomainProxy.publicAccess"),
  ];
  if (!isMappingWebSocketTarget.value) {
    items.push(
      showToolbar.value
        ? t("admin.subdomainProxy.toolbar")
        : t("admin.subdomainProxy.hideToolbar"),
    );
  }

  if (isMappingAuthService.value) {
    items.push(t("admin.subdomainProxy.authEntry"));
  } else {
    if (basicAuthInjectionModel.value) {
      items.push(t("admin.subdomainProxy.injectCredentials"));
    }
    items.push(
      sendProxyHeaders.value
        ? t("admin.subdomainProxy.sendProxyHeaders")
        : t("admin.subdomainProxy.disableProxyHeaders"),
    );
    items.push(
      preserveHost.value
        ? t("admin.subdomainProxy.preserveHost")
        : t("admin.subdomainProxy.useUpstreamHost"),
    );
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
const configuredAuthServicePublicPort = computed(() => {
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
    configuredHttpPort
  );
});
const authServicePublicPort = computed({
  get: () => {
    return (
      configuredAuthServicePublicPort.value ||
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
const configuredAccessEntryPort = computed(() => {
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
  return configuredPort > 0 ? configuredPort : 0;
});
const displayAccessEntryPort = computed(() =>
  configuredAccessEntryPort.value > 0
    ? String(configuredAccessEntryPort.value)
    : accessEntryPort.value.trim() || "7999",
);
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
  if (
    shouldOmitPublicAccessEntryPort(configStore.config) &&
    configuredAccessEntryPort.value <= 0
  ) {
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
  if (
    (isEdgeClientIPActive.value ||
      shouldOmitPublicAccessEntryPort(configStore.config)) &&
    configuredAuthServicePublicPort.value <= 0
  ) {
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
  getMappingDisplayTitle(mapping) || t("admin.subdomainProxy.notFetched");
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

  return isSupportedProxyTargetUrl(target) && !basicAuthValidationMessage.value;
});

const { isPending: isSavingMode, run: runSaveMode } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.subdomainProxy.saveFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.subdomainProxy.saveModeFailed"),
      ),
    });
  },
});

const { isPending: isSavingMappings, run: runSaveMappings } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.subdomainProxy.saveFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.subdomainProxy.saveMappingFailed"),
      ),
    });
  },
});

const {
  isPending: isClearingAllSubdomainConfig,
  run: runClearAllSubdomainConfig,
} = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.subdomainProxy.clearFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.subdomainProxy.clearConfigFailed"),
      ),
    });
  },
});

const { isPending: isSyncing, run: runSyncRoutes } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.subdomainProxy.syncFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.subdomainProxy.syncGatewayFailed"),
      ),
    });
  },
});

const { isPending: isRefreshingTitles, run: runRefreshTitles } = useAsyncAction(
  {
    onError: (error) => {
      toast.error(t("admin.subdomainProxy.refreshFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.subdomainProxy.refreshAllTitlesFailed"),
        ),
      });
    },
  },
);

const {
  isPending: isRefreshingMappingMetadata,
  run: runRefreshMappingMetadata,
} = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.subdomainProxy.refreshFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.subdomainProxy.refreshMetadataFailed"),
      ),
    });
  },
});

const { isPending: isExportingBookmarks, run: runExportBookmarks } =
  useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.subdomainProxy.exportFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.subdomainProxy.exportBookmarksFailed"),
        ),
      });
    },
  });

const { isPending: isDiscovering, run: runDiscoverServices } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.subdomainProxy.discoverFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.subdomainProxy.discoverServicesFailed"),
      ),
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

watch(shouldShowPortalDisabledTooltip, (visible) => {
  if (!visible) {
    isPortalDisabledTooltipOpen.value = false;
  }
});

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

function handleLocationRulesTooltipOpenChange(host: string, nextOpen: boolean) {
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

function handlePortalDisabledTooltipOpenChange(nextOpen: boolean) {
  isPortalDisabledTooltipOpen.value = nextOpen;
}

function handlePortalDisabledTooltipTriggerClick() {
  if (!shouldShowPortalDisabledTooltip.value || !isTouchInteraction.value) {
    return;
  }

  isPortalDisabledTooltipOpen.value = !isPortalDisabledTooltipOpen.value;
}

onMounted(async () => {
  interactionMediaQuery = window.matchMedia("(hover: none), (pointer: coarse)");
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
      interactionMediaQuery.removeEventListener(
        "change",
        updateInteractionMode,
      );
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
    scrollMappingDialogInputIntoView(
      target,
      attempts === 0 ? "auto" : "smooth",
    );
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
    const result = await ConfigAPI.probeHostMappingBasicAuth(normalizedTarget);
    setBasicAuthProbeCacheResult(normalizedTarget, result);
  } catch (error) {
    setBasicAuthProbeCacheResult(normalizedTarget, {
      requiresBasicAuth: false,
      httpStatus: null,
      error: extractErrorMessage(
        error,
        t("admin.subdomainProxy.basicAuthProbeFailed"),
      ),
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
        t("admin.subdomainProxy.proxyHeadersLoadFailed"),
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
        t("admin.subdomainProxy.hostResponseLoadFailed"),
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
    toast.info(t("admin.subdomainProxy.switchedToSuffixMode"), {
      description: t("admin.subdomainProxy.switchedToSuffixDescription", {
        domain: savedRootDomain.value,
      }),
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
    toast.success(t("admin.subdomainProxy.modeSaved"));
    if (result?.ssl_auto_selection?.message) {
      if (result.ssl_auto_selection.applied) {
        toast.success(result.ssl_auto_selection.message, {
          description: result.ssl_auto_selection.label
            ? t("admin.subdomainProxy.switchedCertificate", {
                label: result.ssl_auto_selection.label,
              })
            : undefined,
        });
      } else {
        toast.error(t("admin.subdomainProxy.sslAutoSwitchIncomplete"), {
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
  const isWebSocketTarget = isWebSocketProxyTargetUrl(normalizedTarget);
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
    suppress_toolbar:
      serviceRole === "auth"
        ? false
        : isWebSocketTarget
          ? true
          : input.suppress_toolbar,
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
    toast.success(t("admin.subdomainProxy.orderUpdated"));
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
      toast.success(t("admin.subdomainProxy.hostCopied"), {
        description: host,
      });
      return;
    }

    toast.info(t("admin.subdomainProxy.copyAttempted"), {
      description: host,
    });
  } catch {
    toast.error(t("admin.subdomainProxy.copyFailed"), {
      description: t("admin.subdomainProxy.copyRestricted"),
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
    throw new Error(t("admin.subdomainProxy.concurrentSave"));
  }

  try {
    await configStore.saveHostMappings(
      allMappings.value.map((item) =>
        item.host === mapping.host
          ? { ...item, title_override: nextTitleOverride }
          : item,
      ),
    );
    toast.success(t("admin.subdomainProxy.titleUpdated"));
  } catch (error) {
    toast.error(t("admin.subdomainProxy.saveFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.subdomainProxy.titleSaveFailed"),
      ),
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
        toast.success(t("admin.subdomainProxy.metadataRefreshed"), {
          description: metadata.title.trim()
            ? t("admin.subdomainProxy.fetchedTitle", {
                title: metadata.title.trim(),
              })
            : t("admin.subdomainProxy.metadataNoTitle"),
        });
      },
    },
  );
}

async function addAuthService() {
  if (!canManageNewMappings.value) {
    toast.error(t("admin.subdomainProxy.cannotAddAuthService"), {
      description: !savedRootDomain.value
        ? t("admin.subdomainProxy.saveRootFirst")
        : t("admin.subdomainProxy.rootDirtyAddAuth"),
    });
    return;
  }

  if (authServiceMapping.value) {
    toast.error(t("admin.subdomainProxy.authServiceExists"), {
      description: t("admin.subdomainProxy.authServiceExistsDescription", {
        host: authServiceMapping.value.host,
      }),
    });
    return;
  }

  const host = composeHostFromSubdomain(
    DEFAULT_AUTH_SUBDOMAIN,
    savedRootDomain.value,
  );
  const target = resolveDefaultAuthServiceTarget();

  if (!host) {
    toast.error(t("admin.subdomainProxy.defaultAuthGenerateFailed"), {
      description: t("admin.subdomainProxy.confirmRootSaved"),
    });
    return;
  }

  const duplicateHost = allMappings.value.find((item) => item.host === host);
  if (duplicateHost) {
    toast.error(t("admin.subdomainProxy.defaultAuthSubdomainExists"), {
      description: t(
        "admin.subdomainProxy.defaultAuthSubdomainExistsDescription",
        { host },
      ),
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

    toast.success(t("admin.subdomainProxy.authServiceAdded"), {
      description: `${host} -> ${target}`,
    });
  });
}

function openClearAllConfigDialog() {
  if (allMappings.value.length === 0) {
    toast.error(t("admin.subdomainProxy.noClearableMappings"));
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
    toast.error(t("admin.subdomainProxy.noCurrentAuthService"));
    return false;
  }

  const authHost = authServiceMapping.value.host;

  const removed = await runSaveMappings(async () => {
    await configStore.saveHostMappings(
      allMappings.value.filter((item) => !isAuthServiceTarget(item.target)),
    );

    toast.success(t("admin.subdomainProxy.authServiceDeleted"), {
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

    toast.success(t("admin.subdomainProxy.allCleared"), {
      description:
        mappingsCount > 0
          ? t("admin.subdomainProxy.clearedMappingsDescription", {
              count: mappingsCount,
            })
          : t("admin.subdomainProxy.modeConfigKept"),
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
    toast.error(t("admin.subdomainProxy.hostExists"), {
      description: t("admin.subdomainProxy.hostExistsDescription", {
        host: normalized.host,
      }),
    });
    return;
  }

  const duplicateAuthService = allMappings.value.find(
    (item) =>
      isAuthServiceTarget(item.target) && item.host !== editingHost.value,
  );
  if (normalized.service_role === "auth" && duplicateAuthService) {
    toast.error(t("admin.subdomainProxy.authServiceExists"), {
      description: t("admin.subdomainProxy.duplicateAuthServiceDescription", {
        host: duplicateAuthService.host,
      }),
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
      toast.error(t("admin.subdomainProxy.advancedSaveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.subdomainProxy.advancedConfigSaveFailed"),
        ),
      });
      return;
    }

    toast.success(
      index >= 0
        ? t("admin.subdomainProxy.mappingUpdated")
        : t("admin.subdomainProxy.mappingAdded"),
    );
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
    toast.success(t("admin.subdomainProxy.mappingDeleted"));

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
    toast.error(t("admin.subdomainProxy.cannotDiscover"), {
      description: !savedRootDomain.value
        ? t("admin.subdomainProxy.saveRootFirst")
        : t("admin.subdomainProxy.rootDirtyDiscover"),
    });
    return;
  }

  openDiscoverDialogState();
  if (!discoveredData.value) {
    void nextTick().then(() => triggerScan());
  }
}

function openStaleCleanupDialog() {
  void staleCleanupDialogRef.value?.open();
}

const saveHostMappingsForCleanup = async (mappings: HostMapping[]) => {
  await configStore.saveHostMappings(mappings);
};

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
    toast.error(t("admin.subdomainProxy.duplicateDiscoverHosts"), {
      description: duplicateHosts.join(", "),
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
    toast.success(
      t("admin.subdomainProxy.addedMappings", {
        count: selectedServices.value.length,
      }),
    );
    dismissDiscoverDialog();
  });
}

async function syncRoutes() {
  await runSyncRoutes(() => ConfigAPI.syncRoutes(), {
    onSuccess: (result) => {
      if (result.success) {
        toast.success(t("admin.subdomainProxy.syncedGateway"), {
          description: t("admin.subdomainProxy.syncedGatewayDescription", {
            pathRules: result.data?.synced_rules ?? 0,
            hostRules: result.data?.synced_host_rules ?? 0,
          }),
        });
        return;
      }
      toast.error(t("admin.subdomainProxy.syncFailed"), {
        description: result.message || t("admin.subdomainProxy.syncNoSuccess"),
      });
    },
  });
}

async function refreshAllTitles() {
  await runRefreshTitles(() => configStore.refreshAllHostMappingTitles(), {
    onSuccess: (summary) => {
      toast.success(t("admin.subdomainProxy.titlesRefreshDone"), {
        description: t("admin.subdomainProxy.titlesRefreshDescription", {
          updated: summary.updated,
          failed: summary.failed,
          skipped: summary.skipped,
        }),
      });
      brokenFaviconKeys.value = new Set();
    },
  });
}

async function exportBookmarks() {
  await runExportBookmarks(() => ConfigAPI.downloadHostMappingBookmarks(), {
    onSuccess: (blob) => {
      downloadBlob(blob, buildBookmarkExportFilename(savedRootDomain.value));
      toast.success(t("admin.subdomainProxy.bookmarksExported"), {
        description: t("admin.subdomainProxy.bookmarksExportDescription", {
          count: visibleMappings.value.length,
        }),
      });
    },
  });
}
</script>
