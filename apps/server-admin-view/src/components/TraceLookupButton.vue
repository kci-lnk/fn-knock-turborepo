<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { Route } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { isTraceId, normalizeTraceId } from "@/lib/trace-id";

const { t } = useI18n();
const router = useRouter();
const open = ref(false);
const value = ref("");
const invalid = ref(false);

const submit = () => {
  const traceId = normalizeTraceId(value.value);
  invalid.value = !isTraceId(traceId);
  if (invalid.value) return;
  open.value = false;
  void router.push(`/traces/${encodeURIComponent(traceId)}`);
};
</script>

<template>
  <Button variant="outline" class="shrink-0" @click="open = true">
    <Route class="mr-2 h-4 w-4" />
    {{ t("admin.trace.lookup") }}
  </Button>

  <Dialog v-model:open="open">
    <DialogContent class="sm:max-w-[560px]">
      <DialogHeader>
        <DialogTitle>{{ t("admin.trace.lookupTitle") }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.trace.lookupDescription") }}
        </DialogDescription>
      </DialogHeader>
      <form class="space-y-2" @submit.prevent="submit">
        <Label for="trace-id-query">{{ t("admin.trace.label") }}</Label>
        <Input
          id="trace-id-query"
          v-model="value"
          autocomplete="off"
          spellcheck="false"
          class="font-mono"
          :aria-invalid="invalid"
          :placeholder="t('admin.trace.inputPlaceholder')"
          @input="invalid = false"
        />
        <p v-if="invalid" class="text-sm text-destructive" role="alert">
          {{ t("admin.trace.invalid") }}
        </p>
        <DialogFooter class="pt-3">
          <Button type="submit">{{ t("admin.trace.search") }}</Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
