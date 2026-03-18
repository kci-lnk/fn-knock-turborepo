<template>
  <Card
    v-if="
      !hasLoadedSSLStatus || (isLoading && showLoadingSkeleton && !sslStatus)
    "
  >
    <CardHeader>
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-3">
          <CardTitle>SSL 证书</CardTitle>
        </div>
      </div>
      <CardDescription>加载中...</CardDescription>
    </CardHeader>
    <CardContent class="grid gap-4">
      <div class="rounded-lg border bg-muted/30 p-4 grid gap-3">
        <div
          class="grid grid-cols-[88px_minmax(0,1fr)] gap-y-3 text-sm sm:grid-cols-[100px_minmax(0,1fr)]"
        >
          <Skeleton class="h-4 w-16" />
          <Skeleton class="h-4 w-56" />
          <Skeleton class="h-4 w-16" />
          <Skeleton class="h-4 w-64" />
          <Skeleton class="h-4 w-16" />
          <Skeleton class="h-4 w-40" />
          <Skeleton class="h-4 w-16" />
          <Skeleton class="h-4 w-48" />
        </div>
      </div>
    </CardContent>
  </Card>

  <div v-else class="grid gap-4">
    <Card class="overflow-hidden">
      <CardHeader>
        <div class="flex flex-wrap items-start justify-between gap-3">
          <div class="grid gap-1">
            <CardTitle class="flex items-center gap-3">
              <span>HTTPS 当前状态</span>
              <Badge
                :variant="activeCertificate ? 'default' : 'secondary'"
                :class="
                  activeCertificate ? 'bg-green-600 hover:bg-green-600' : ''
                "
              >
                {{ primaryCertificateBadgeLabel }}
              </Badge>
            </CardTitle>
            <CardDescription class="leading-6">
              {{ statusOverviewText }}
            </CardDescription>
          </div>
          <div class="flex flex-wrap gap-2">
            <Badge variant="outline">
              {{ deploymentModeLabel }}
            </Badge>
            <Badge variant="secondary">
              证书库 {{ certificates.length }} 张
            </Badge>
          </div>
        </div>
      </CardHeader>

      <CardContent v-if="libraryCoverage" class="pt-0">
        <Alert
          :variant="
            libraryCoverage.status === 'missing' ? 'destructive' : 'default'
          "
        >
          <AlertTitle>子域名模式闭环提示</AlertTitle>
          <AlertDescription class="grid gap-2">
            <p>{{ libraryCoverage.summary }}</p>
            <p
              v-if="
                libraryCoverage.combined_covering_certificate_ids.length > 1
              "
              class="text-xs text-muted-foreground"
            >
              组合覆盖证书数：{{
                libraryCoverage.combined_covering_certificate_ids.length
              }}
            </p>
            <div
              v-if="libraryCoverage.warnings.length"
              class="grid gap-1 text-xs text-muted-foreground"
            >
              <div v-for="warning in libraryCoverage.warnings" :key="warning">
                {{ warning }}
              </div>
            </div>
            <div class="flex flex-wrap gap-2">
              <Button
                v-if="showMultiSniSuggestion"
                size="sm"
                variant="outline"
                :disabled="isUpdatingDeploymentMode"
                @click="updateDeploymentMode('multi_sni')"
              >
                切换到多证书 SNI
              </Button>
              <Button
                v-if="recommendedCertificateId"
                size="sm"
                :disabled="isActivating"
                @click="activateRecommendedCertificate"
              >
                立即切换到推荐证书
              </Button>
            </div>
          </AlertDescription>
        </Alert>
      </CardContent>

      <CardFooter
        v-if="activeCertificate"
        class="flex flex-wrap justify-end gap-2 border-t pt-6"
      >
        <ConfirmDangerPopover
          title="确认停用当前活动证书"
          description="停用后将关闭 HTTPS，但证书仍会保留在证书库里，后续可以再次启用。"
          confirm-text="确认停用"
          :loading="isClearing"
          :disabled="isClearing"
          :on-confirm="handleClear"
        >
          <template #trigger>
            <Button variant="destructive" size="sm" :disabled="isClearing">
              停用 HTTPS
            </Button>
          </template>
        </ConfirmDangerPopover>
      </CardFooter>
    </Card>

    <ConfigCollapsibleCard
      title="部署方式与网关下发"
      :configured="deploymentSectionConfigured"
      :ready="hasLoadedSSLStatus"
      edit-label="查看配置"
      collapsed-content-class="min-h-[76px] flex flex-col items-start gap-3 sm:h-[40px] sm:flex-row sm:items-center sm:justify-between"
      summary-class="text-xs text-muted-foreground max-w-full whitespace-normal break-words sm:truncate"
      expanded-content-class="p-0 sm:p-0"
      actions-class="border-t bg-muted/30 px-4 py-4 sm:px-6 flex flex-col-reverse gap-2 rounded-b-lg sm:flex-row sm:items-center sm:justify-end"
    >
      <template #summary>
        {{ deploymentSummary }}
      </template>

      <template #default>
        <div class="divide-y divide-border">
          <div class="p-4 sm:p-6 grid gap-2">
            <div class="flex flex-wrap items-center justify-between gap-2">
              <div class="text-base font-semibold">HTTPS 部署方式</div>
              <Badge variant="outline">{{ deploymentModeShortLabel }}</Badge>
            </div>
            <p class="text-sm text-muted-foreground">
              这个切换只决定 Go
              网关收到的是“当前这一张证书”，还是“整套证书集合”。
            </p>
            <p class="text-xs text-muted-foreground">
              {{ deploymentModeDescription }}
            </p>
            <p v-if="deploymentModeMismatch" class="text-xs text-amber-600">
              已保存配置是{{
                configuredDeploymentModeLabel
              }}，但网关当前实际运行在{{ deploymentModeShortLabel }}。
            </p>
            <p v-else-if="gatewaySyncError" class="text-xs text-amber-600">
              {{ gatewaySyncError }}
            </p>
          </div>

          <div class="grid gap-3 p-4 sm:p-6 lg:grid-cols-2">
            <div
              class="rounded-lg border p-4 grid gap-3 transition-colors"
              :class="deploymentCardClass('single_active')"
            >
              <div class="flex flex-wrap items-start justify-between gap-2">
                <div class="grid gap-1">
                  <div class="text-sm font-medium">单活动证书</div>
                  <p class="text-xs text-muted-foreground">
                    只向网关下发当前活动证书。切换“当前证书”时，对外 HTTPS
                    证书也会一起切换。
                  </p>
                </div>
                <Badge
                  v-if="sslStatus?.deploymentMode === 'single_active'"
                  variant="default"
                  class="bg-green-600 hover:bg-green-600"
                >
                  当前模式
                </Badge>
              </div>

              <div class="grid gap-2 text-xs text-muted-foreground">
                <div>预计下发：{{ singleActivePreview.count }} 张</div>
                <div>对外证书：{{ singleActivePreview.defaultLabel }}</div>
                <div v-if="singleActivePreview.domainSummary">
                  覆盖域名：{{ singleActivePreview.domainSummary }}
                </div>
              </div>

              <Button
                variant="outline"
                class="justify-start"
                :disabled="
                  isUpdatingDeploymentMode ||
                  sslStatus?.deploymentMode === 'single_active'
                "
                @click="updateDeploymentMode('single_active')"
              >
                <span
                  v-if="
                    isUpdatingDeploymentMode &&
                    pendingDeploymentMode === 'single_active'
                  "
                  class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent"
                ></span>
                {{
                  sslStatus?.deploymentMode === "single_active"
                    ? "当前正在使用"
                    : "切换到单活动证书"
                }}
              </Button>
            </div>

            <div
              class="rounded-lg border p-4 grid gap-3 transition-colors"
              :class="deploymentCardClass('multi_sni')"
            >
              <div class="flex flex-wrap items-start justify-between gap-2">
                <div class="grid gap-1">
                  <div class="text-sm font-medium">多证书 SNI</div>
                  <p class="text-xs text-muted-foreground">
                    将证书库中的多张证书一起下发到网关，由网关按访问域名自动匹配证书。
                  </p>
                </div>
                <Badge
                  v-if="sslStatus?.deploymentMode === 'multi_sni'"
                  variant="default"
                  class="bg-green-600 hover:bg-green-600"
                >
                  当前模式
                </Badge>
              </div>

              <div class="grid gap-2 text-xs text-muted-foreground">
                <div>预计下发：{{ multiSniPreview.count }} 张</div>
                <div>默认证书：{{ multiSniPreview.defaultLabel }}</div>
                <div
                  v-if="multiSniPreview.previewItems.length"
                  class="flex flex-wrap gap-1.5"
                >
                  <Badge
                    v-for="item in multiSniPreview.previewItems"
                    :key="item.id"
                    variant="secondary"
                    class="max-w-full"
                  >
                    <span class="truncate">{{ item.label }}</span>
                    <span
                      v-if="item.isDefault"
                      class="ml-1 text-[10px] text-muted-foreground"
                      >默认</span
                    >
                  </Badge>
                  <Badge
                    v-if="multiSniPreview.remainingCount > 0"
                    variant="outline"
                  >
                    +{{ multiSniPreview.remainingCount }} 张
                  </Badge>
                </div>
              </div>

              <Button
                class="justify-start"
                :disabled="
                  isUpdatingDeploymentMode ||
                  !certificates.length ||
                  sslStatus?.deploymentMode === 'multi_sni'
                "
                @click="updateDeploymentMode('multi_sni')"
              >
                <span
                  v-if="
                    isUpdatingDeploymentMode &&
                    pendingDeploymentMode === 'multi_sni'
                  "
                  class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
                ></span>
                {{
                  sslStatus?.deploymentMode === "multi_sni"
                    ? "当前正在使用"
                    : "切换到多证书 SNI"
                }}
              </Button>
            </div>
          </div>

          <div class="p-4 sm:p-6">
            <div
              class="rounded-lg border border-dashed bg-muted/20 p-4 grid gap-2"
            >
              <div class="text-xs font-medium">网关当前已接收的证书集</div>
              <p class="text-xs text-muted-foreground">
                {{ gatewayDeploymentSummary }}
              </p>
              <div
                v-if="deployedGatewayCertificates.length"
                class="flex flex-wrap gap-1.5"
              >
                <Badge
                  v-for="certificate in deployedGatewayCertificates"
                  :key="gatewayCertificateKey(certificate)"
                  variant="secondary"
                  class="max-w-full"
                >
                  <span class="truncate">{{
                    gatewayCertificateLabel(certificate)
                  }}</span>
                  <span
                    v-if="certificate.is_default"
                    class="ml-1 text-[10px] text-muted-foreground"
                  >
                    默认
                  </span>
                </Badge>
              </div>
            </div>
          </div>
        </div>
      </template>

      <template #actions="{ collapse }">
        <Button variant="outline" @click="collapse">折叠</Button>
      </template>
    </ConfigCollapsibleCard>

    <ConfigCollapsibleCard
      v-if="activeCertificate || subdomainCoverage"
      title="当前证书详情"
      :configured="Boolean(activeCertificate?.certInfo)"
      :ready="hasLoadedSSLStatus"
      edit-label="查看详情"
      collapsed-content-class="min-h-[76px] flex flex-col items-start gap-3 sm:h-[40px] sm:flex-row sm:items-center sm:justify-between"
      summary-class="text-xs text-muted-foreground max-w-full whitespace-normal break-words sm:truncate"
      expanded-content-class="p-0 sm:p-0"
      actions-class="border-t bg-muted/30 px-4 py-4 sm:px-6 flex flex-col gap-2 rounded-b-lg sm:flex-row sm:justify-end"
    >
      <template #summary>
        {{ currentCertificateSummary }}
      </template>

      <template #default>
        <div class="p-4 sm:p-6">
          <div
            v-if="activeCertificate?.certInfo"
            class="grid gap-4 xl:grid-cols-[minmax(0,1.2fr)_minmax(0,1fr)]"
          >
            <div class="rounded-lg border bg-muted/20 p-4 grid gap-4">
              <div
                class="grid grid-cols-[88px_minmax(0,1fr)] gap-y-3 text-sm sm:grid-cols-[100px_minmax(0,1fr)]"
              >
                <span class="text-muted-foreground font-medium">名称</span>
                <span class="min-w-0 font-medium">{{
                  activeCertificate.label
                }}</span>

                <span class="text-muted-foreground font-medium">来源</span>
                <span class="min-w-0 text-xs">{{
                  sourceLabel(activeCertificate.source)
                }}</span>

                <span class="text-muted-foreground font-medium">签发者</span>
                <span class="min-w-0 font-mono text-xs break-all">{{
                  formatDN(activeCertificate.certInfo.issuer)
                }}</span>

                <span class="text-muted-foreground font-medium">签发给</span>
                <span class="min-w-0 font-mono text-xs break-all">{{
                  formatDN(activeCertificate.certInfo.subject)
                }}</span>

                <span class="text-muted-foreground font-medium">有效期</span>
                <span class="min-w-0 text-xs">
                  <span>{{
                    formatDate(activeCertificate.certInfo.validFrom)
                  }}</span>
                  <span class="mx-1 text-muted-foreground">至</span>
                  <span
                    :class="isExpired ? 'text-destructive font-semibold' : ''"
                  >
                    {{ formatDate(activeCertificate.certInfo.validTo) }}
                  </span>
                  <Badge
                    v-if="isExpired"
                    variant="destructive"
                    class="ml-2 text-[10px]"
                    >已过期</Badge
                  >
                  <Badge
                    v-else-if="isExpiringSoon"
                    variant="outline"
                    class="ml-2 text-[10px] border-yellow-500 text-yellow-600"
                  >
                    即将过期
                  </Badge>
                </span>

                <span class="text-muted-foreground font-medium">域名</span>
                <div class="min-w-0 flex flex-wrap gap-1.5">
                  <Badge
                    v-for="dns in activeCertificate.certInfo.dnsNames"
                    :key="dns"
                    variant="secondary"
                    class="font-mono text-xs"
                  >
                    {{ dns }}
                  </Badge>
                  <span
                    v-if="!activeCertificate.certInfo.dnsNames.length"
                    class="text-xs text-muted-foreground"
                  >
                    无
                  </span>
                </div>

                <span class="text-muted-foreground font-medium">更新时间</span>
                <span class="min-w-0 text-xs text-muted-foreground">
                  {{ formatDate(activeCertificate.updated_at) }}
                </span>
              </div>
            </div>

            <div
              v-if="subdomainCoverage"
              class="rounded-lg border bg-background/80 p-4 grid gap-3"
            >
              <div class="flex flex-wrap items-center justify-between gap-3">
                <div class="text-sm font-medium">活动证书覆盖分析</div>
                <Badge
                  :variant="coverageBadgeVariant(subdomainCoverage)"
                  :class="coverageBadgeClass(subdomainCoverage)"
                >
                  {{ coverageBadgeLabel(subdomainCoverage) }}
                </Badge>
              </div>
              <p class="text-sm text-muted-foreground">
                {{ subdomainCoverage.summary }}
              </p>
              <div
                class="grid grid-cols-[88px_minmax(0,1fr)] gap-y-3 text-sm sm:grid-cols-[100px_minmax(0,1fr)]"
              >
                <span class="text-muted-foreground font-medium">鉴权服务</span>
                <span class="min-w-0 font-mono text-xs break-all">
                  {{ subdomainCoverage.auth_host || "未配置" }}
                </span>

                <span class="text-muted-foreground font-medium">推荐域名</span>
                <span class="min-w-0 font-mono text-xs break-all">
                  {{
                    subdomainCoverage.recommended_domains.length
                      ? subdomainCoverage.recommended_domains.join(", ")
                      : "暂无推荐"
                  }}
                </span>

                <span class="text-muted-foreground font-medium">Host 覆盖</span>
                <span class="min-w-0 text-xs">
                  {{ subdomainCoverage.covered_hosts.length }} /
                  {{
                    subdomainCoverage.covered_hosts.length +
                    subdomainCoverage.uncovered_hosts.length
                  }}
                  条映射已覆盖
                </span>
              </div>
              <div
                v-if="subdomainCoverage.uncovered_hosts.length"
                class="text-xs text-amber-600"
              >
                未覆盖 Host：{{
                  uncoveredHostsPreview(subdomainCoverage.uncovered_hosts)
                }}
              </div>
              <div
                v-if="subdomainCoverage.warnings.length"
                class="grid gap-1 text-xs text-muted-foreground"
              >
                <div
                  v-for="warning in subdomainCoverage.warnings"
                  :key="warning"
                >
                  {{ warning }}
                </div>
              </div>
            </div>
          </div>

          <Alert v-else variant="default">
            <AlertTitle>当前没有活动证书</AlertTitle>
            <AlertDescription class="grid gap-2">
              <p>你可以上传新的手动证书，或从证书库里选择一张设为当前证书。</p>
              <p v-if="subdomainCoverage" class="text-xs text-muted-foreground">
                {{ subdomainCoverage.summary }}
              </p>
            </AlertDescription>
          </Alert>
        </div>
      </template>

      <template #actions="{ collapse }">
        <Button variant="outline" @click="collapse">折叠</Button>
      </template>
    </ConfigCollapsibleCard>

    <ConfigCollapsibleCard
      title="上传手动证书"
      :configured="manualUploadConfigured"
      :ready="hasLoadedSSLStatus"
      edit-label="展开表单"
      collapsed-content-class="min-h-[76px] flex flex-col items-start gap-3 sm:h-[40px] sm:flex-row sm:items-center sm:justify-between"
      summary-class="text-xs text-muted-foreground max-w-full whitespace-normal break-words sm:truncate"
      expanded-content-class="p-0 sm:p-0"
      actions-class="border-t bg-muted/30 px-4 py-4 sm:px-6 flex flex-col-reverse gap-2 rounded-b-lg sm:flex-row sm:items-center sm:justify-end"
    >
      <template #summary>
        {{ manualUploadSummary }}
      </template>

      <template #default>
        <div class="divide-y divide-border">
          <div class="p-4 sm:p-6 grid gap-2">
            <div class="text-base font-semibold">上传新证书</div>
            <p class="text-sm text-muted-foreground">
              支持直接粘贴 PEM
              文本，也支持从飞牛共享目录导入。保存时可以选择只入库，或直接启用为当前证书。
            </p>
          </div>

          <div class="p-4 sm:p-6 grid gap-6">
            <CertForm
              v-model:cert="formData.cert"
              v-model:sslKey="formData.key"
              :share-name="sslSharedFiles.shareName"
              :shared-files="sslSharedFiles.files"
              :shared-files-available="sslSharedFiles.available"
              :shared-files-loading="isLoadingSharedFiles"
              :shared-files-error="sharedFilesError"
              :shared-file-selecting="isReadingSharedFile"
              @request-shared-files="handleSharedFilesRequest"
              @select-shared-file="handleCreateSharedFileSelect"
            />

            <Alert v-if="errorMessage" variant="destructive">
              <AlertTitle>证书验证失败</AlertTitle>
              <AlertDescription>{{ errorMessage }}</AlertDescription>
            </Alert>
          </div>
        </div>
      </template>

      <template #actions="{ collapse }">
        <Button variant="outline" @click="collapse">折叠</Button>
        <Button
          variant="outline"
          :disabled="isSaving || (!formData.cert && !formData.key)"
          @click="resetManualUploadForm"
        >
          清空
        </Button>
        <Button
          variant="outline"
          :disabled="isSaving || !formData.cert || !formData.key"
          @click="handleSave(false)"
        >
          <span
            v-if="isSaving && pendingSaveMode === 'store'"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          仅保存到证书库
        </Button>
        <Button
          :disabled="isSaving || !formData.cert || !formData.key"
          @click="handleSave(true)"
        >
          <span
            v-if="isSaving && pendingSaveMode === 'activate'"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          保存并启用
        </Button>
      </template>
    </ConfigCollapsibleCard>

    <ConfigCollapsibleCard
      v-if="certificates.length"
      title="证书库"
      :configured="true"
      :ready="hasLoadedSSLStatus"
      edit-label="查看证书"
      collapsed-content-class="min-h-[76px] flex flex-col items-start gap-3 sm:h-[40px] sm:flex-row sm:items-center sm:justify-between"
      summary-class="text-xs text-muted-foreground max-w-full whitespace-normal break-words sm:truncate"
      expanded-content-class="p-0 sm:p-0"
      actions-class="border-t bg-muted/30 px-4 py-4 sm:px-6 flex flex-col gap-2 rounded-b-lg sm:flex-row sm:justify-end"
    >
      <template #summary>
        {{ certificateLibrarySummary }}
      </template>

      <template #default>
        <div class="p-4 sm:p-6 grid gap-3 xl:grid-cols-2">
          <div
            v-for="certificate in certificates"
            :key="certificate.id"
            class="rounded-lg border bg-muted/15 p-4 grid gap-3"
          >
            <div class="flex flex-wrap items-start justify-between gap-3">
              <div class="grid gap-1 min-w-0">
                <div class="flex flex-wrap items-center gap-2">
                  <div class="font-medium break-all">
                    {{ certificateDisplayLabel(certificate) }}
                  </div>
                  <Badge
                    v-if="certificate.is_active"
                    variant="default"
                    class="bg-green-600 hover:bg-green-600"
                  >
                    {{ certificateRoleLabel(certificate) }}
                  </Badge>
                  <Badge variant="outline">
                    {{ sourceLabel(certificate.source) }}
                  </Badge>
                  <Badge
                    v-if="certificate.coverage"
                    :variant="coverageBadgeVariant(certificate.coverage)"
                    :class="coverageBadgeClass(certificate.coverage)"
                  >
                    {{ coverageBadgeLabel(certificate.coverage) }}
                  </Badge>
                </div>
                <div class="text-xs text-muted-foreground font-mono break-all">
                  {{
                    certificate.certInfo?.dnsNames?.join(", ") ||
                    certificate.primary_domain ||
                    "未解析到域名信息"
                  }}
                </div>
              </div>

              <div class="flex flex-wrap gap-2">
                <Button
                  v-if="!certificate.is_active"
                  size="sm"
                  :disabled="isActivating"
                  @click="activateCertificate(certificate.id)"
                >
                  <span
                    v-if="
                      isActivating && activatingCertificateId === certificate.id
                    "
                    class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
                  ></span>
                  {{ activateButtonLabel }}
                </Button>
                <ConfirmDangerPopover
                  title="确认删除证书"
                  description="删除后会从证书库移除这张证书；如果它当前正在使用，HTTPS 也会一并停用。"
                  confirm-text="确认删除"
                  :loading="
                    isDeleting && deletingCertificateId === certificate.id
                  "
                  :disabled="
                    isDeleting && deletingCertificateId === certificate.id
                  "
                  :on-confirm="() => deleteCertificate(certificate.id)"
                >
                  <template #trigger>
                    <Button
                      variant="destructive"
                      size="sm"
                      :disabled="
                        isDeleting && deletingCertificateId === certificate.id
                      "
                    >
                      删除
                    </Button>
                  </template>
                </ConfirmDangerPopover>
              </div>
            </div>

            <div class="grid gap-2 text-xs text-muted-foreground">
              <div>
                有效期：{{ formatDate(certificate.certInfo?.validFrom || "") }}
                <span class="mx-1">至</span>
                {{ formatDate(certificate.certInfo?.validTo || "") }}
              </div>
              <div>更新于：{{ formatDate(certificate.updated_at) }}</div>
              <div v-if="certificate.coverage?.summary">
                {{ certificate.coverage.summary }}
              </div>
            </div>
          </div>
        </div>
      </template>

      <template #actions="{ collapse }">
        <Button variant="outline" @click="collapse">折叠</Button>
      </template>
    </ConfigCollapsibleCard>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardFooter,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Alert, AlertTitle, AlertDescription } from "@/components/ui/alert";
import { Skeleton } from "@/components/ui/skeleton";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import CertForm from "@admin-shared/components/ssl/CertForm.vue";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { ConfigAPI } from "../../lib/api";
import type {
  SSLDeploymentMode,
  SSLCertificateSummary,
  SSLSharedFilesPayload,
  SSLStatus,
  SSLCertificateSource,
  SubdomainCertificateCoverage,
} from "../../types";
import { toast } from "@admin-shared/utils/toast";

type DeploymentPreviewItem = {
  id: string;
  label: string;
  isDefault: boolean;
};

type GatewayCertificateItem = NonNullable<
  SSLStatus["gateway_status"]
>["certificates"][number];

const sslStatus = ref<SSLStatus | null>(null);
const hasLoadedSSLStatus = ref(false);
const errorMessage = ref("");
const sharedFilesError = ref("");
const formData = ref({ cert: "", key: "" });
const pendingSaveMode = ref<"store" | "activate" | null>(null);
const activatingCertificateId = ref<string | null>(null);
const deletingCertificateId = ref<string | null>(null);
const pendingDeploymentMode = ref<"single_active" | "multi_sni" | null>(null);

const defaultSSLSharedFiles: SSLSharedFilesPayload = {
  shareName: "fn-knock",
  available: false,
  files: [],
};
const sslSharedFiles = ref<SSLSharedFilesPayload>(defaultSSLSharedFiles);
const hasLoadedSharedFiles = ref(false);

const { isPending: isSaving, run: runSaveSSL } = useAsyncAction({
  onError: (error) => {
    const message = extractErrorMessage(
      error,
      "保存失败，请检查证书和私钥格式。",
    );
    errorMessage.value = message;
    toast.error(message);
  },
});
const { isPending: isClearing, run: runClearSSL } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, "停用 HTTPS 失败"));
  },
});
const { isPending: isLoading, run: runLoadSSLStatus } = useAsyncAction({
  onError: (error) => {
    console.error("Failed to load SSL status:", error);
  },
});
const { isPending: isLoadingSharedFiles, run: runLoadSharedFiles } =
  useAsyncAction({
    onError: (error) => {
      const message = extractErrorMessage(error, "读取飞牛共享目录失败");
      sharedFilesError.value = message;
      toast.error(message);
    },
  });
const { isPending: isReadingSharedFile, run: runReadSharedFile } =
  useAsyncAction({
    onError: (error) => {
      toast.error(extractErrorMessage(error, "读取共享文件失败"));
    },
  });
const { isPending: isActivating, run: runActivateSSL } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, "切换证书失败"));
  },
});
const { isPending: isDeleting, run: runDeleteSSL } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, "删除证书失败"));
  },
});
const { isPending: isUpdatingDeploymentMode, run: runUpdateDeploymentMode } =
  useAsyncAction({
    onError: (error) => {
      toast.error(extractErrorMessage(error, "切换部署模式失败"));
    },
  });

const showLoadingSkeleton = useDelayedLoading(isLoading);

const certificates = computed(() => sslStatus.value?.certificates || []);
const activeCertificate = computed(
  () => certificates.value.find((certificate) => certificate.is_active) || null,
);
const deployedGatewayCertificates = computed(
  () => sslStatus.value?.gateway_status?.certificates || [],
);
const subdomainCoverage = computed(
  () =>
    sslStatus.value?.subdomain_coverage ??
    activeCertificate.value?.coverage ??
    null,
);
const libraryCoverage = computed(
  () => sslStatus.value?.library_coverage ?? null,
);
const recommendedCertificateId = computed(
  () => libraryCoverage.value?.suggested_certificate_id || "",
);

const primaryCertificateBadgeLabel = computed(() => {
  if (!activeCertificate.value) return "未启用";
  if (sslStatus.value?.deploymentMode === "multi_sni") return "默认 / 兜底";
  return "已启用";
});
const deploymentModeLabel = computed(() => {
  if (sslStatus.value?.deploymentMode === "multi_sni")
    return "部署模式：多证书 SNI";
  return "部署模式：单活动证书";
});
const deploymentModeShortLabel = computed(() => {
  if (sslStatus.value?.deploymentMode === "multi_sni") return "多证书 SNI";
  return "单活动证书";
});
const configuredDeploymentModeLabel = computed(() => {
  if (sslStatus.value?.configuredDeploymentMode === "multi_sni")
    return "多证书 SNI";
  return "单活动证书";
});
const deploymentModeDescription = computed(() => {
  if (sslStatus.value?.deploymentMode === "multi_sni") {
    return "网关会按 SNI 为不同域名分配证书，适合同一入口下并存多张父域/子域证书。";
  }
  return "网关当前只会对外提供一张默认证书，适合单域或单套证书场景。";
});
const deploymentModeMismatch = computed(
  () =>
    Boolean(sslStatus.value?.configuredDeploymentMode) &&
    sslStatus.value?.configuredDeploymentMode !==
      sslStatus.value?.deploymentMode,
);
const gatewaySyncError = computed(
  () => sslStatus.value?.gateway_status?.sync_error || "",
);
const showMultiSniSuggestion = computed(
  () =>
    sslStatus.value?.deploymentMode !== "multi_sni" &&
    (libraryCoverage.value?.combined_covering_certificate_ids.length || 0) > 1,
);
const activateButtonLabel = computed(() => {
  if (sslStatus.value?.deploymentMode === "multi_sni") return "设为默认证书";
  return "设为当前证书";
});
const gatewayDeploymentSummary = computed(() => {
  if (!deployedGatewayCertificates.value.length) {
    return "当前网关还没有接收到任何证书。";
  }

  const defaultCertificate = deployedGatewayCertificates.value.find(
    (certificate) => certificate.is_default,
  );
  const defaultLabel = defaultCertificate
    ? gatewayCertificateLabel(defaultCertificate)
    : "未标记";

  if (sslStatus.value?.deploymentMode === "multi_sni") {
    return `当前网关持有 ${deployedGatewayCertificates.value.length} 张证书，并会按 SNI 自动选证；默认/兜底证书是 ${defaultLabel}。`;
  }

  return `当前网关持有 ${deployedGatewayCertificates.value.length} 张证书；单活动证书模式下，对外统一返回 ${defaultLabel}。`;
});
const statusOverviewText = computed(() => {
  if (!activeCertificate.value?.certInfo) {
    return `当前还没有活动证书。上传手动证书或从证书库启用一张证书后，这里会显示当前真正对外返回的 HTTPS 证书。`;
  }

  const parts = [
    `当前对外证书：${certificateDisplayLabel(activeCertificate.value)}`,
    `来源 ${sourceLabel(activeCertificate.value.source)}`,
  ];

  if (isExpired.value) {
    parts.push(
      `已过期（${formatDate(activeCertificate.value.certInfo.validTo)}）`,
    );
  } else if (isExpiringSoon.value) {
    parts.push(
      `30 天内到期（${formatDate(activeCertificate.value.certInfo.validTo)}）`,
    );
  } else {
    parts.push(
      `有效至 ${formatDate(activeCertificate.value.certInfo.validTo)}`,
    );
  }

  return parts.join(" · ");
});
const deploymentSummary = computed(
  () =>
    `${deploymentModeShortLabel.value} · 网关已下发 ${deployedGatewayCertificates.value.length} 张证书`,
);
const deploymentSectionConfigured = computed(() =>
  Boolean(
    certificates.value.length || deployedGatewayCertificates.value.length,
  ),
);
const currentCertificateSummary = computed(() => {
  if (!activeCertificate.value) {
    return subdomainCoverage.value
      ? `当前未启用证书 · ${subdomainCoverage.value.summary}`
      : "当前没有活动证书";
  }

  const parts = [certificateDisplayLabel(activeCertificate.value)];
  const domainSummary = certificateDomainSummary(activeCertificate.value);
  if (domainSummary) parts.push(domainSummary);
  if (isExpired.value) {
    parts.push("已过期");
  } else if (isExpiringSoon.value) {
    parts.push("30 天内到期");
  } else {
    parts.push("运行中");
  }
  return parts.join(" · ");
});
const manualUploadConfigured = computed(() =>
  Boolean(
    certificates.value.length || formData.value.cert || formData.value.key,
  ),
);
const manualUploadSummary = computed(() => {
  if (formData.value.cert || formData.value.key) {
    return "已填写待保存的证书内容";
  }
  if (certificates.value.length) {
    return `证书库已有 ${certificates.value.length} 张证书，需要时再展开上传表单`;
  }
  return "当前还没有手动上传证书，建议先在这里准备一份可用证书";
});
const certificateLibrarySummary = computed(() => {
  const activeLabel = activeCertificate.value
    ? `当前 ${certificateDisplayLabel(activeCertificate.value)}`
    : "当前未设置活动证书";
  return `共 ${certificates.value.length} 张证书 · ${activeLabel}`;
});
const singleActivePreview = computed(() =>
  buildDeploymentPreview("single_active"),
);
const multiSniPreview = computed(() => buildDeploymentPreview("multi_sni"));

const isExpired = computed(() => {
  const validTo = activeCertificate.value?.certInfo?.validTo;
  if (!validTo) return false;
  return new Date(validTo) < new Date();
});

const isExpiringSoon = computed(() => {
  const validTo = activeCertificate.value?.certInfo?.validTo;
  if (!validTo) return false;
  const expiresAt = new Date(validTo);
  const now = new Date();
  const thirtyDays = 30 * 24 * 60 * 60 * 1000;
  return expiresAt > now && expiresAt.getTime() - now.getTime() < thirtyDays;
});

onMounted(() => {
  loadSSLStatus();
});

async function loadSSLStatus() {
  await runLoadSSLStatus(async () => {
    sslStatus.value = await ConfigAPI.getSSLStatus();
  });
  hasLoadedSSLStatus.value = true;
}

async function handleSave(activate: boolean) {
  pendingSaveMode.value = activate ? "activate" : "store";
  errorMessage.value = "";
  await runSaveSSL(async () => {
    await ConfigAPI.setSSL({
      label: "手动上传证书",
      source: "manual",
      cert: formData.value.cert,
      key: formData.value.key,
      activate,
    });
    formData.value = { cert: "", key: "" };
    await loadSSLStatus();
    toast.success(activate ? "证书已保存并启用" : "证书已保存到证书库");
  });
  pendingSaveMode.value = null;
}

function resetManualUploadForm() {
  formData.value = { cert: "", key: "" };
  errorMessage.value = "";
}

async function handleClear() {
  await runClearSSL(async () => {
    await ConfigAPI.deleteSSL();
    await loadSSLStatus();
    toast.success("已停用当前 HTTPS 证书");
  });
}

async function activateCertificate(id: string) {
  activatingCertificateId.value = id;
  await runActivateSSL(async () => {
    await ConfigAPI.activateSSLCertificate(id);
    await loadSSLStatus();
    toast.success(
      sslStatus.value?.deploymentMode === "multi_sni"
        ? "已切换默认证书"
        : "已切换当前活动证书",
    );
  });
  activatingCertificateId.value = null;
}

async function activateRecommendedCertificate() {
  if (!recommendedCertificateId.value) return;
  await activateCertificate(recommendedCertificateId.value);
}

async function updateDeploymentMode(mode: "single_active" | "multi_sni") {
  if (!sslStatus.value || sslStatus.value.deploymentMode === mode) return;
  pendingDeploymentMode.value = mode;
  await runUpdateDeploymentMode(async () => {
    sslStatus.value = await ConfigAPI.updateSSLDeploymentMode(mode);
    toast.success(
      mode === "multi_sni"
        ? "已切换到多证书 SNI 部署"
        : "已切换到单活动证书模式",
    );
  });
  pendingDeploymentMode.value = null;
}

async function deleteCertificate(id: string) {
  deletingCertificateId.value = id;
  await runDeleteSSL(async () => {
    await ConfigAPI.deleteSSLCertificate(id);
    await loadSSLStatus();
    toast.success("证书已删除");
  });
  deletingCertificateId.value = null;
}

async function loadSharedFiles(force = false) {
  if (hasLoadedSharedFiles.value && !force) return;

  sharedFilesError.value = "";
  const nextFiles = await runLoadSharedFiles(async () =>
    ConfigAPI.getSSLSharedFiles(),
  );
  if (!nextFiles) return;

  sslSharedFiles.value = nextFiles;
  hasLoadedSharedFiles.value = true;
}

async function handleSharedFilesRequest(payload: {
  field: "cert" | "sslKey";
  force?: boolean;
}) {
  await loadSharedFiles(Boolean(payload.force));
}

async function applySharedFileSelection(
  target: { cert: string; key: string },
  payload: { field: "cert" | "sslKey"; relativePath: string },
) {
  const result = await runReadSharedFile(async () =>
    ConfigAPI.readSSLSharedFile(payload.relativePath),
  );
  if (!result) return;

  if (payload.field === "cert") {
    target.cert = result.content;
  } else {
    target.key = result.content;
  }

  const label = payload.field === "cert" ? "证书" : "私钥";
  toast.success(`已从飞牛目录载入${label}文件：${result.file.name}`);
}

async function handleCreateSharedFileSelect(payload: {
  field: "cert" | "sslKey";
  relativePath: string;
}) {
  await applySharedFileSelection(formData.value, payload);
}

function certificateDisplayLabel(certificate: SSLCertificateSummary): string {
  return (
    certificate.label ||
    certificate.primary_domain ||
    certificate.certInfo?.dnsNames?.[0] ||
    certificate.id
  );
}

function certificateDomainSummary(
  certificate: SSLCertificateSummary | null,
): string {
  const domains = certificate?.certInfo?.dnsNames || [];
  if (!domains.length) return "";
  const preview = domains.slice(0, 3).join(", ");
  if (domains.length <= 3) return preview;
  return `${preview} 等 ${domains.length} 个域名`;
}

function buildDeploymentPreview(mode: SSLDeploymentMode) {
  const items =
    mode === "single_active"
      ? activeCertificate.value
        ? [activeCertificate.value]
        : []
      : [...certificates.value].sort((a, b) => {
          if (a.is_active === b.is_active) return 0;
          return a.is_active ? -1 : 1;
        });

  const defaultCertificate =
    items.find((certificate) => certificate.is_active) || items[0] || null;

  return {
    count: items.length,
    defaultLabel: defaultCertificate
      ? certificateDisplayLabel(defaultCertificate)
      : "未设置",
    domainSummary: certificateDomainSummary(defaultCertificate),
    previewItems: items
      .slice(0, 3)
      .map<DeploymentPreviewItem>((certificate) => ({
        id: certificate.id,
        label: certificateDisplayLabel(certificate),
        isDefault: defaultCertificate?.id === certificate.id,
      })),
    remainingCount: Math.max(items.length - 3, 0),
  };
}

function deploymentCardClass(mode: SSLDeploymentMode) {
  if (sslStatus.value?.deploymentMode === mode) {
    return "border-green-500 bg-green-50/60";
  }
  return "bg-muted/20";
}

function certificateRoleLabel(certificate: SSLCertificateSummary) {
  if (!certificate.is_active) return "";
  if (sslStatus.value?.deploymentMode === "multi_sni") return "默认 / 兜底";
  return "当前活动";
}

function gatewayCertificateLabel(certificate: GatewayCertificateItem) {
  return (
    certificate.label ||
    certificate.domains?.[0] ||
    certificate.id ||
    "未命名证书"
  );
}

function gatewayCertificateKey(certificate: GatewayCertificateItem) {
  return (
    certificate.id ||
    `${gatewayCertificateLabel(certificate)}-${certificate.domains?.join(",") || "no-domains"}`
  );
}

function sourceLabel(source: SSLCertificateSource): string {
  if (source === "acme") return "ACME";
  if (source === "ca") return "本地 CA";
  return "手动上传";
}

function coverageBadgeVariant(coverage: SubdomainCertificateCoverage) {
  if (coverage.status === "missing") return "destructive";
  if (coverage.status === "partial") return "outline";
  return "default";
}

function coverageBadgeClass(coverage: SubdomainCertificateCoverage) {
  if (coverage.status === "ready") return "bg-green-600 hover:bg-green-600";
  if (coverage.status === "partial") return "border-amber-500 text-amber-700";
  return "";
}

function coverageBadgeLabel(coverage: SubdomainCertificateCoverage) {
  if (coverage.status === "ready") return "完全覆盖";
  if (coverage.status === "partial") return "部分覆盖";
  return "未覆盖";
}

function uncoveredHostsPreview(hosts: string[]) {
  if (hosts.length === 0) return "";
  const preview = hosts.slice(0, 4).join(", ");
  if (hosts.length <= 4) return preview;
  return `${preview} 等 ${hosts.length} 个`;
}

function formatDN(dn: string): string {
  return dn.replace(/\n/g, ", ");
}

function formatDate(dateStr: string): string {
  if (!dateStr) return "";
  const date = new Date(dateStr);
  if (Number.isNaN(date.getTime())) return dateStr;
  return date.toLocaleDateString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}
</script>
