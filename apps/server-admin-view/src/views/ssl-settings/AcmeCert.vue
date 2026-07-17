<template>
  <div class="grid gap-4">
    <Card class="border-border/80 shadow-sm">
      <CardHeader>
        <div
          class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between"
        >
          <div class="grid gap-1">
            <CardTitle class="flex flex-wrap items-center gap-2">
              {{
                configStore.isWindowsDeployment
                  ? t("admin.acmeCert.dns01Title")
                  : t("admin.acmeCert.title")
              }}
              <Badge :variant="acmeStatusBadgeVariant">{{
                acmeStatusLabel
              }}</Badge>
              <Badge v-if="isTableLocked" variant="outline">
                {{ lockReasonLabel }}
              </Badge>
            </CardTitle>
            <CardDescription>
              {{
                configStore.isWindowsDeployment
                  ? t("admin.acmeCert.dns01Description")
                  : t("admin.acmeCert.description")
              }}
            </CardDescription>
          </div>
          <div class="flex flex-wrap items-center gap-2">
            <RefreshButton
              :loading="isOverviewLoading || isProvidersLoading"
              :disabled="isOverviewLoading || isProvidersLoading"
              @click="refresh"
            />
            <Button
              :disabled="
                !isAcmeInstalled ||
                isTableLocked ||
                isDialogSubmitting ||
                !dnsProviders.length
              "
              @click="openCreateDialog"
            >
              {{ t("admin.acmeCert.newApplication") }}
            </Button>
          </div>
        </div>
      </CardHeader>
    </Card>

    <Alert
      v-if="shouldPromptAcmeInitialization"
      class="border-amber-200 bg-amber-50 text-amber-950 dark:border-amber-900/50 dark:bg-amber-950/20 dark:text-amber-100"
    >
      <AlertTriangle class="h-4 w-4" />
      <AlertTitle>
        {{ t("admin.acmeCert.initializePromptTitle") }}
      </AlertTitle>
      <AlertDescription
        class="grid gap-3 text-amber-900 sm:grid-cols-[1fr_auto] sm:items-center dark:text-amber-100/90"
      >
        <span>{{ t("admin.acmeCert.initializePromptDescription") }}</span>
        <Button
          type="button"
          size="sm"
          variant="outline"
          class="shrink-0 border-amber-300 bg-background/80 text-amber-950 hover:bg-background dark:border-amber-700 dark:text-amber-100"
          @click="goToAcmeInitialization"
        >
          {{ t("admin.acmeCert.goInitialize") }}
        </Button>
      </AlertDescription>
    </Alert>

    <Card class="border-border/80 shadow-sm">
      <CardHeader>
        <CardTitle>{{ t("admin.acmeCert.applicationList") }}</CardTitle>
        <CardDescription>
          {{ t("admin.acmeCert.applicationListDescription") }}
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div class="relative">
          <div
            v-if="isTableLocked"
            class="absolute inset-0 z-10 flex items-center justify-center rounded-lg border bg-background/80 p-4 backdrop-blur-sm"
          >
            <div class="max-w-md text-center">
              <div class="text-sm font-medium">{{ lockMessageTitle }}</div>
              <div class="mt-1 text-xs text-muted-foreground">
                {{ lockMessageDescription }}
              </div>
            </div>
          </div>

          <div class="overflow-x-auto rounded-lg border">
            <Table class="table-fixed">
              <TableHeader>
                <TableRow>
                  <TableHead class="w-[100px] whitespace-normal">
                    {{ t("admin.acmeCert.dnsProvider") }}
                  </TableHead>
                  <TableHead class="w-[120px] whitespace-normal">
                    {{ t("admin.acmeCert.domain") }}
                  </TableHead>
                  <TableHead class="w-[180px] whitespace-normal">
                    {{ t("admin.acmeCert.statusOverview") }}
                  </TableHead>
                  <TableHead class="w-[150px] whitespace-normal">{{
                    t("admin.acmeCert.validity")
                  }}</TableHead>
                  <TableHead class="w-[156px] whitespace-normal text-right">
                    {{ t("admin.acmeCert.actions") }}
                  </TableHead>
                </TableRow>
              </TableHeader>

              <TableBody>
                <template v-if="isOverviewLoading && !applications.length">
                  <TableRow v-for="index in 4" :key="index">
                    <TableCell class="align-top whitespace-normal">
                      <Skeleton class="h-4 w-16" />
                    </TableCell>
                    <TableCell class="align-top whitespace-normal">
                      <Skeleton class="h-4 w-36" />
                    </TableCell>
                    <TableCell class="align-top whitespace-normal">
                      <Skeleton class="h-4 w-24" />
                    </TableCell>
                    <TableCell class="align-top whitespace-normal">
                      <Skeleton class="h-4 w-24" />
                    </TableCell>
                    <TableCell class="align-top whitespace-normal text-right">
                      <div class="ml-auto inline-flex">
                        <Skeleton class="h-8 w-16 rounded-r-none" />
                        <Skeleton class="h-8 w-8 rounded-l-none border-l" />
                      </div>
                    </TableCell>
                  </TableRow>
                </template>

                <TableRow v-else-if="!applications.length">
                  <TableCell
                    colspan="5"
                    class="py-10 text-center text-muted-foreground"
                  >
                    {{
                      shouldPromptAcmeInitialization
                        ? t("admin.acmeCert.emptyApplicationsBeforeInit")
                        : t("admin.acmeCert.emptyApplications")
                    }}
                  </TableCell>
                </TableRow>

                <TableRow
                  v-for="application in applications"
                  :key="application.id"
                >
                  <TableCell class="align-top whitespace-normal break-words">
                    <div class="grid gap-1">
                      <div class="font-medium">
                        {{ application.providerLabel }}
                      </div>
                      <div class="font-mono text-xs text-muted-foreground">
                        {{ application.dnsType }}
                      </div>
                    </div>
                  </TableCell>

                  <TableCell class="align-top whitespace-normal break-all">
                    <div class="grid gap-1">
                      <div class="font-medium">
                        {{ application.name || application.primaryDomain }}
                      </div>
                      <div
                        class="font-mono text-xs text-muted-foreground break-all"
                      >
                        {{ application.domains.join(", ") }}
                      </div>
                      <div class="text-xs text-muted-foreground">
                        {{
                          application.renewEnabled
                            ? t("admin.acmeCert.autoRenewEnabled")
                            : t("admin.acmeCert.autoRenewDisabled")
                        }}
                      </div>
                    </div>
                  </TableCell>

                  <TableCell class="align-top whitespace-normal break-words">
                    <div class="grid gap-1">
                      <div class="flex flex-wrap gap-1">
                        <Badge :variant="certificateBadgeVariant(application)">
                          {{ certificateStatusLabel(application) }}
                        </Badge>
                        <Badge :variant="libraryBadgeVariant(application)">
                          {{ libraryStatusLabel(application) }}
                        </Badge>
                        <Badge
                          :variant="
                            jobBadgeVariant(application.latestJob?.status)
                          "
                        >
                          {{ latestJobLabel(application) }}
                        </Badge>
                      </div>
                      <div
                        v-if="application.certificate?.exists"
                        class="text-xs text-muted-foreground break-all"
                      >
                        {{
                          application.certificate?.issuer ||
                          t("admin.acmeCert.unknownIssuer")
                        }}
                      </div>
                    </div>
                  </TableCell>

                  <TableCell
                    class="align-top whitespace-normal break-words text-xs leading-5 text-muted-foreground"
                  >
                    {{ formatCertificateRange(application) }}
                  </TableCell>

                  <TableCell class="align-top whitespace-normal text-right">
                    <div class="inline-flex items-center gap-2">
                      <div class="inline-flex">
                        <Button
                          type="button"
                          size="sm"
                          class="rounded-r-none"
                          :disabled="isActionBlocked()"
                          @click="requestCertificate(application.id)"
                        >
                          {{ primaryActionLabel(application) }}
                        </Button>
                        <DropdownMenu>
                          <DropdownMenuTrigger as-child>
                            <Button
                              type="button"
                              size="sm"
                              variant="default"
                              class="rounded-l-none border-l border-primary-foreground/20 px-2"
                              :disabled="isSecondaryActionDisabled(application)"
                            >
                              <ChevronDown class="h-4 w-4" />
                            </Button>
                          </DropdownMenuTrigger>
                          <DropdownMenuContent align="end" class="w-44">
                            <DropdownMenuItem
                              :disabled="isActionBlocked()"
                              @select="openEditDialog(application.id)"
                            >
                              {{ t("admin.acmeCert.editApplication") }}
                            </DropdownMenuItem>
                            <DropdownMenuItem
                              v-if="application.latestJob?.id"
                              @select="viewJob(application.latestJob.id)"
                            >
                              {{ t("admin.acmeCert.viewLogs") }}
                            </DropdownMenuItem>
                            <DropdownMenuItem
                              v-if="application.certificate?.exists"
                              :disabled="isActionBlocked()"
                              @select="downloadCertificate(application)"
                            >
                              {{ t("admin.acmeCert.downloadCertificate") }}
                            </DropdownMenuItem>
                            <DropdownMenuItem
                              v-if="application.certificate?.exists"
                              :disabled="isActionBlocked()"
                              @select="syncLibrary(application)"
                            >
                              {{
                                application.library?.linked
                                  ? t("admin.acmeCert.updateToLibrary")
                                  : t("admin.acmeCert.addToLibrary")
                              }}
                            </DropdownMenuItem>
                            <DropdownMenuItem
                              v-if="application.certificate?.exists"
                              :disabled="isActionBlocked()"
                              @select="deployCertificate(application)"
                            >
                              {{ t("admin.acmeCert.setAsCurrentCertificate") }}
                            </DropdownMenuItem>
                            <DropdownMenuSeparator
                              v-if="application.certificate?.exists"
                            />
                            <DropdownMenuItem
                              v-if="application.certificate?.exists"
                              variant="destructive"
                              :disabled="isActionBlocked()"
                              @select="openDeleteDialog(application)"
                            >
                              {{ t("admin.acmeCert.deleteCertificate") }}
                            </DropdownMenuItem>
                          </DropdownMenuContent>
                        </DropdownMenu>
                      </div>

                      <ConfirmDangerPopover
                        :title="
                          t('admin.acmeCert.confirmDeleteApplicationTitle')
                        "
                        :description="deleteApplicationDescription(application)"
                        :confirm-text="t('admin.acmeCert.deleteApplication')"
                        :loading="deletingApplicationId === application.id"
                        :disabled="isDeleteApplicationBlocked()"
                        :on-confirm="() => removeApplication(application)"
                        content-class="w-80 text-left"
                      >
                        <template #trigger>
                          <Button
                            type="button"
                            variant="ghost"
                            size="icon"
                            class="h-8 w-8 text-destructive hover:bg-destructive/10 hover:text-destructive"
                            :disabled="isDeleteApplicationBlocked()"
                          >
                            <Trash2 class="h-4 w-4" />
                          </Button>
                        </template>
                      </ConfirmDangerPopover>
                    </div>
                  </TableCell>
                </TableRow>
              </TableBody>
            </Table>
          </div>
        </div>
      </CardContent>
    </Card>

    <AcmeJobPanel
      v-if="job"
      :job="job"
      :logs="logs"
      :analysis="analysis"
      :application-label="selectedApplicationLabel"
      :is-refreshing="isRefreshingLogs"
      :can-stop="canStopActiveJob"
      :is-stopping="isStoppingJob"
      :stop-action="stopActiveJob"
      @refresh="refreshLogs"
      @focus-credentials="focusCredentialsFromJob"
    />

    <AcmeApplicationDialog
      v-model:open="isDialogOpen"
      :mode="dialogMode"
      :initial-value="editingApplication"
      :dns-providers="dnsProviders"
      :pending="isDialogSubmitting"
      @submit="submitDialog"
    />

    <Dialog
      :open="Boolean(deleteCandidate)"
      @update:open="handleDeleteDialogOpenChange"
    >
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>
            {{ t("admin.acmeCert.confirmDeleteCertificateTitle") }}
          </DialogTitle>
          <DialogDescription class="leading-6">
            {{
              t("admin.acmeCert.confirmDeleteCertificateDescription", {
                target:
                  deleteCandidateLabel ||
                  t("admin.acmeCert.currentApplication"),
              })
            }}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            :disabled="isMutating"
            @click="closeDeleteDialog"
          >
            {{ t("common.cancel") }}
          </Button>
          <Button
            type="button"
            variant="destructive"
            :disabled="isActionBlocked() || !deleteCandidate"
            @click="confirmDeleteCandidate"
          >
            {{ t("common.confirmDelete") }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
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
import RefreshButton from "@/components/RefreshButton.vue";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import AcmeApplicationDialog from "./AcmeApplicationDialog.vue";
import AcmeJobPanel from "./AcmeJobPanel.vue";
import { AlertTriangle, ChevronDown, Trash2 } from "lucide-vue-next";
import { useAcmeCertificateController } from "./useAcmeCertificateController";

const {
  acmeStatusBadgeVariant,
  acmeStatusLabel,
  analysis,
  applications,
  canStopActiveJob,
  certificateBadgeVariant,
  certificateStatusLabel,
  closeDeleteDialog,
  configStore,
  confirmDeleteCandidate,
  deleteApplicationDescription,
  deleteCandidate,
  deleteCandidateLabel,
  deletingApplicationId,
  deployCertificate,
  dialogMode,
  dnsProviders,
  downloadCertificate,
  editingApplication,
  focusCredentialsFromJob,
  formatCertificateRange,
  goToAcmeInitialization,
  handleDeleteDialogOpenChange,
  isAcmeInstalled,
  isActionBlocked,
  isDeleteApplicationBlocked,
  isDialogOpen,
  isDialogSubmitting,
  isMutating,
  isOverviewLoading,
  isProvidersLoading,
  isRefreshingLogs,
  isSecondaryActionDisabled,
  isStoppingJob,
  isTableLocked,
  job,
  jobBadgeVariant,
  latestJobLabel,
  libraryBadgeVariant,
  libraryStatusLabel,
  lockMessageDescription,
  lockMessageTitle,
  lockReasonLabel,
  logs,
  openCreateDialog,
  openDeleteDialog,
  openEditDialog,
  primaryActionLabel,
  refresh,
  refreshLogs,
  removeApplication,
  requestCertificate,
  selectedApplicationLabel,
  shouldPromptAcmeInitialization,
  stopActiveJob,
  submitDialog,
  syncLibrary,
  t,
  viewJob,
} = useAcmeCertificateController();
</script>
