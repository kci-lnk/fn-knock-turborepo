<script setup lang="ts">
import { ChevronDown, Trash2 } from "lucide-vue-next";
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
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
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
import type { AcmeCertificateController } from "./acme-certificate-contract";

const props = defineProps<{ controller: AcmeCertificateController }>();
const {
  applications,
  certificateBadgeVariant,
  certificateStatusLabel,
  deleteApplicationDescription,
  deletingApplicationId,
  deployCertificate,
  downloadCertificate,
  formatCertificateRange,
  isActionBlocked,
  isDeleteApplicationBlocked,
  isOverviewLoading,
  isSecondaryActionDisabled,
  isTableLocked,
  jobBadgeVariant,
  latestJobLabel,
  libraryBadgeVariant,
  libraryStatusLabel,
  lockMessageDescription,
  lockMessageTitle,
  openDeleteDialog,
  openEditDialog,
  primaryActionLabel,
  removeApplication,
  requestCertificate,
  shouldPromptAcmeInitialization,
  syncLibrary,
  t,
  viewJob,
} = props.controller;
</script>

<template>
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
                              :aria-label="t('common.moreActions')"
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
                            :aria-label="t('common.confirmDelete')"
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
</template>
