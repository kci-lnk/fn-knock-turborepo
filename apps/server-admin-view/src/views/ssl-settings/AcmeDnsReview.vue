<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useNow } from "@vueuse/core";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import { AcmeAPI, type TxtSelection } from "@/lib/api/acme";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import type { AcmeCertificateController } from "./acme-certificate-contract";

const { controller } = defineProps<{ controller: AcmeCertificateController }>();
const {
  dnsReport: report,
  t,
  requestCertificate,
  openEditDialog,
  checkingDns,
  showDnsError,
  fetchOverview,
} = controller;
const selected = ref<TxtSelection[]>([]);
const confirming = ref(false);
const busy = ref(false);
const error = ref("");
const outcomes = ref<{ name: string; value: string; outcome: string }[]>([]);
const now = useNow({ interval: 1000 });
const remaining = computed(() =>
  Math.max(
    0,
    Math.ceil(
      (report.value?.cooldown?.retryAt ?? 0) - now.value.getTime() / 1000,
    ),
  ),
);
const locked = computed(() => busy.value || checkingDns.value);
const waiting = computed(() =>
  outcomes.value.some(
    (r) =>
      ["deleted", "absent"].includes(r.outcome) &&
      report.value?.groups.some(
        (g) => g.name === r.name && g.values.includes(r.value),
      ),
  ),
);
watch(
  () => report.value?.snapshot,
  () => {
    selected.value = [];
    confirming.value = false;
  },
  { flush: "sync" },
);
watch(
  () => report.value?.applicationId,
  () => {
    outcomes.value = [];
    error.value = "";
  },
);
function toggle(name: string, value: string, checked: boolean) {
  selected.value = selected.value.filter(
    (r) => r.name !== name || r.value !== value,
  );
  if (checked) selected.value.push({ name, value });
}
async function refresh(remove = false) {
  const current = report.value;
  if (!current || locked.value) return;
  busy.value = true;
  error.value = "";
  try {
    if (remove) {
      const submitted = selected.value;
      const result = await AcmeAPI.deleteTxt(current, submitted);
      outcomes.value = result.results;
      report.value = result.report;
      selected.value = submitted.filter(
        (r) =>
          result.report.groups.some(
            (g) => g.name === r.name && g.selectable.includes(r.value),
          ) &&
          result.results.some(
            (done) =>
              done.name === r.name &&
              done.value === r.value &&
              done.outcome === "failed",
          ),
      );
      confirming.value = false;
    } else report.value = await AcmeAPI.checkDns(current.applicationId);
    await fetchOverview({ silent: true, preserveSelection: true });
  } catch (e) {
    if (!showDnsError(e))
      error.value = extractErrorMessage(e, t("admin.acmeCert.operationFailed"));
  } finally {
    busy.value = false;
  }
}
function edit() {
  const id = report.value?.applicationId;
  report.value = null;
  if (id) void openEditDialog(id);
}
</script>

<template>
  <Dialog
    :open="!!report"
    @update:open="
      (open) => {
        if (!open && !locked) report = null;
      }
    "
  >
    <DialogContent
      v-if="report"
      class="max-h-[88vh] overflow-y-auto sm:max-w-xl"
    >
      <DialogHeader>
        <DialogTitle>{{
          t(confirming ? "admin.acmeDns.confirm" : "admin.acmeDns.title")
        }}</DialogTitle>
        <DialogDescription>{{
          t(confirming ? "admin.acmeDns.impact" : "admin.acmeDns.description")
        }}</DialogDescription>
      </DialogHeader>
      <div v-if="confirming" class="grid gap-2">
        <div
          v-for="row in selected"
          :key="JSON.stringify(row)"
          class="rounded border p-2 break-all font-mono text-sm"
        >
          {{ row.name }} · TXT<br />{{ row.value }}
        </div>
      </div>
      <template v-else>
        <p v-if="report.groups.length" class="text-xs text-muted-foreground">
          {{ t("admin.acmeDns.checked") }}
          {{ new Date(report.checkedAt).toLocaleString() }}
        </p>
        <p v-if="report.cooldown" role="status">
          {{
            t(
              `admin.acmeDns.${report.cooldown.source === "unknown" ? "unknownSource" : report.cooldown.source}`,
            )
          }}
          · {{ new Date(report.cooldown.retryAt * 1000).toLocaleString() }} ·
          {{ t("admin.acmeDns.remaining", { seconds: remaining }) }}
          {{ report.cooldown.estimated ? t("admin.acmeDns.estimated") : "" }}
        </p>
        <section
          v-for="group in report.groups"
          :key="group.name"
          class="grid gap-1 rounded-md border p-3 text-sm"
        >
          <strong class="break-all font-mono">{{ group.name }}</strong>
          <span class="break-all text-xs text-muted-foreground">{{
            group.domains.join(", ")
          }}</span>
          <p>
            {{
              t(
                `admin.acmeDns.${group.status === "propagating" ? "propagation" : group.status}`,
              )
            }}
          </p>
          <code
            v-for="target in group.delegation"
            :key="target"
            class="break-all"
            >{{ target }}</code
          >
          <label
            v-for="value in group.values"
            :key="value"
            class="flex items-start gap-2 py-1"
          >
            <input
              v-if="group.selectable.includes(value)"
              type="checkbox"
              class="mt-1"
              :disabled="
                locked ||
                outcomes.some(
                  (r) =>
                    r.name === group.name &&
                    r.value === value &&
                    ['deleted', 'absent'].includes(r.outcome),
                )
              "
              :checked="
                selected.some((r) => r.name === group.name && r.value === value)
              "
              @change="
                toggle(
                  group.name,
                  value,
                  ($event.target as HTMLInputElement).checked,
                )
              "
            />
            <code class="select-all break-all">TXT {{ value }}</code>
          </label>
          <small
            v-if="group.values.some((v) => !group.selectable.includes(v))"
            class="text-muted-foreground"
            >{{ t("admin.acmeDns.manual") }}</small
          >
        </section>
        <div aria-live="polite">
          <p v-if="waiting">{{ t("admin.acmeDns.propagation") }}</p>
          <p
            v-for="r in outcomes"
            :key="JSON.stringify(r)"
            class="break-all text-xs"
          >
            {{ r.name }} · {{ r.value }} · {{ t(`admin.acmeDns.${r.outcome}`) }}
          </p>
        </div>
      </template>
      <p v-if="error" role="alert" class="text-destructive">{{ error }}</p>
      <DialogFooter class="flex-wrap gap-2">
        <Button
          variant="outline"
          :disabled="locked"
          @click="confirming ? (confirming = false) : (report = null)"
          >{{ t("admin.acmeDns.cancel") }}</Button
        >
        <Button
          v-if="confirming"
          variant="destructive"
          :disabled="locked"
          @click="refresh(true)"
          >{{ t("admin.acmeDns.remove") }}</Button
        >
        <template v-else>
          <Button variant="ghost" :disabled="locked" @click="edit">{{
            t("admin.acmeDns.edit")
          }}</Button>
          <Button variant="outline" :disabled="locked" @click="refresh()">{{
            t("admin.acmeDns.recheck")
          }}</Button>
          <Button
            v-if="selected.length"
            variant="destructive"
            :disabled="locked"
            @click="confirming = true"
            >{{ t("admin.acmeDns.remove") }}</Button
          >
          <Button
            v-if="
              ['review', 'ready'].includes(report.status) ||
              (report.status === 'cooldown' && !remaining)
            "
            :disabled="locked || waiting"
            @click="requestCertificate(report.applicationId, report.snapshot)"
          >
            {{
              t(
                report.status === "review"
                  ? "admin.acmeDns.keep"
                  : "admin.acmeDns.proceed",
              )
            }}
          </Button>
        </template>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
