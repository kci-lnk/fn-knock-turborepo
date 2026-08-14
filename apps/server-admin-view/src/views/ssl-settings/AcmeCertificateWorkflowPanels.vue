<script setup lang="ts">
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import AcmeApplicationDialog from "./AcmeApplicationDialog.vue";
import AcmeJobPanel from "./AcmeJobPanel.vue";
import type { AcmeCertificateController } from "./acme-certificate-contract";

const props = defineProps<{ controller: AcmeCertificateController }>();
const {
  analysis,
  canStopActiveJob,
  closeDeleteDialog,
  confirmDeleteCandidate,
  deleteCandidate,
  deleteCandidateLabel,
  dialogMode,
  dnsProviders,
  editingApplication,
  focusCredentialsFromJob,
  handleDeleteDialogOpenChange,
  isActionBlocked,
  isDialogOpen,
  isDialogSubmitting,
  isMutating,
  isRefreshingLogs,
  isStoppingJob,
  isTableLocked,
  job,
  logs,
  refreshLogs,
  selectedApplicationLabel,
  stopActiveJob,
  submitDialog,
  t,
} = props.controller;
</script>

<template>
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
      :runtime-locked="isTableLocked"
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
</template>
