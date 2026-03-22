<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { Info } from "lucide-vue-next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { ConfigAPI } from "../../lib/api";
import type { AuthCredentialSettings } from "../../types";
import { useConfigStore } from "../../store/config";

type DurationUnit = "second" | "minute" | "hour" | "day" | "week" | "year";

type DurationField = {
  value: number;
  unit: DurationUnit;
};

const configStore = useConfigStore();
const settings = ref<AuthCredentialSettings | null>(null);

const durationUnits: Array<{
  value: DurationUnit;
  label: string;
  seconds: number;
}> = [
  { value: "second", label: "秒", seconds: 1 },
  { value: "minute", label: "分钟", seconds: 60 },
  { value: "hour", label: "小时", seconds: 3600 },
  { value: "day", label: "天", seconds: 24 * 3600 },
  { value: "week", label: "周", seconds: 7 * 24 * 3600 },
  { value: "year", label: "年", seconds: 365 * 24 * 3600 },
];

const durationUnitMap = Object.fromEntries(
  durationUnits.map((item) => [item.value, item.seconds]),
) as Record<DurationUnit, number>;

const form = reactive<{
  session: DurationField;
  rememberMe: DurationField;
}>({
  session: {
    value: 24,
    unit: "hour",
  },
  rememberMe: {
    value: 1,
    unit: "year",
  },
});

const { isPending: isLoading, run: runLoadSettings } = useAsyncAction({
  onError: (error) => {
    toast.error("加载失败", {
      description: extractErrorMessage(error, "无法获取凭据设置"),
    });
  },
});
const showLoadingSkeleton = useDelayedLoading(isLoading);
const { isPending: isSaving, run: runSaveSettings } = useAsyncAction({
  onError: (error) => {
    toast.error("保存失败", {
      description: extractErrorMessage(error, "凭据设置保存失败"),
    });
  },
});

const clampDurationValue = (value: unknown) =>
  Math.max(1, Math.floor(Number(value) || 0));

const toSeconds = (field: DurationField): number =>
  clampDurationValue(field.value) * durationUnitMap[field.unit];

const splitDuration = (seconds: number): DurationField => {
  const safeSeconds = Math.max(1, Math.floor(Number(seconds) || 1));
  const matchedUnit =
    [...durationUnits]
      .reverse()
      .find((unit) => safeSeconds % unit.seconds === 0) ?? durationUnits[0]!;

  return {
    value: Math.max(1, safeSeconds / matchedUnit.seconds),
    unit: matchedUnit.value,
  };
};

const formatDuration = (seconds: number): string => {
  const normalized = splitDuration(seconds);
  const label =
    durationUnits.find((item) => item.value === normalized.unit)?.label ||
    normalized.unit;
  return `${normalized.value} ${label}`;
};

const sessionTtlSeconds = computed(() => toSeconds(form.session));
const rememberMeTtlSeconds = computed(() => toSeconds(form.rememberMe));

const isDirty = computed(() => {
  if (!settings.value) return false;
  return (
    settings.value.session_ttl_seconds !== sessionTtlSeconds.value ||
    settings.value.remember_me_ttl_seconds !== rememberMeTtlSeconds.value
  );
});

const applyFromSettings = (data: AuthCredentialSettings) => {
  settings.value = data;
  Object.assign(form.session, splitDuration(data.session_ttl_seconds));
  Object.assign(form.rememberMe, splitDuration(data.remember_me_ttl_seconds));
};

const fetchSettings = async () => {
  await runLoadSettings(async () => {
    const data = await ConfigAPI.getAuthCredentialSettings();
    applyFromSettings(data);
  });
};

const resetForm = () => {
  if (settings.value) applyFromSettings(settings.value);
};

const saveSettings = async () => {
  const nextSessionTtl = sessionTtlSeconds.value;
  const nextRememberMeTtl = rememberMeTtlSeconds.value;

  if (nextSessionTtl < 60 || nextRememberMeTtl < 60) {
    toast.error("时长过短", {
      description: "登录有效期至少需要 60 秒。",
    });
    return;
  }

  if (nextRememberMeTtl < nextSessionTtl) {
    toast.error("设置不合理", {
      description: "记住我有效期不能短于普通登录有效期。",
    });
    return;
  }

  await runSaveSettings(
    () =>
      ConfigAPI.updateAuthCredentialSettings({
        session_ttl_seconds: nextSessionTtl,
        remember_me_ttl_seconds: nextRememberMeTtl,
      }),
    {
      onSuccess: async (data) => {
        applyFromSettings(data);
        await configStore.loadConfig();
        toast.success("凭据设置已更新");
      },
    },
  );
};

onMounted(fetchSettings);
</script>

<template>
  <Card>
    <CardHeader>
      <CardTitle class="text-md">登录凭据有效期</CardTitle>
      <CardDescription class="mt-1.5">
        自定义管理员登录后的会话保持时长。未勾选“记住我”时默认 24
        小时，勾选后默认 1 年，当前设置会持久化到 Redis。
      </CardDescription>
    </CardHeader>

    <CardContent v-if="isLoading && showLoadingSkeleton" class="border-t p-0">
      <div class="space-y-4 p-6">
        <Skeleton class="h-6 w-1/3" />
        <Skeleton class="h-4 w-2/3" />
      </div>
    </CardContent>

    <CardContent v-else-if="!isLoading" class="border-t p-0 divide-y">
      <div class="border-b border-zinc-200 bg-zinc-50/40 px-6 py-5">
        <Alert
          class="items-start rounded-xl border-zinc-200 bg-zinc-50/70 text-zinc-900 shadow-none"
        >
          <Info class="mt-0.5 h-4 w-4 shrink-0" />
          <AlertTitle>修改后仅影响新的登录会话</AlertTitle>
          <AlertDescription class="text-sm leading-6 text-zinc-700">
            已经签发的 Cookie、Redis
            会话和自动放行白名单不会被追溯更新；新设置会在下一次 TOTP 或 Passkey
            登录时生效。
          </AlertDescription>
        </Alert>
      </div>

      <div
        class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
      >
        <div class="space-y-1 pr-6">
          <Label class="text-base">普通登录有效期</Label>
          <div class="text-sm text-muted-foreground">
            管理员完成验证但未勾选“记住我”时，会话保持有效的时长。
          </div>
        </div>
        <div class="flex items-center gap-2 shrink-0">
          <Input
            v-model.number="form.session.value"
            type="number"
            min="1"
            step="1"
            class="w-24 text-center"
            :disabled="isSaving"
          />
          <Select v-model="form.session.unit" :disabled="isSaving">
            <SelectTrigger class="w-[110px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="unit in durationUnits"
                :key="unit.value"
                :value="unit.value"
              >
                {{ unit.label }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div class="sm:col-span-2 -mt-1 text-xs text-muted-foreground">
          当前将保存为 {{ formatDuration(sessionTtlSeconds) }}。
        </div>
      </div>

      <div
        class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
      >
        <div class="space-y-1 pr-6">
          <Label class="text-base">记住我有效期</Label>
          <div class="text-sm text-muted-foreground">
            用户勾选“记住我”后，登录会话以及随登录创建的自动放行时长会按这里设置。
          </div>
        </div>
        <div class="flex items-center gap-2 shrink-0">
          <Input
            v-model.number="form.rememberMe.value"
            type="number"
            min="1"
            step="1"
            class="w-24 text-center"
            :disabled="isSaving"
          />
          <Select v-model="form.rememberMe.unit" :disabled="isSaving">
            <SelectTrigger class="w-[110px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="unit in durationUnits"
                :key="unit.value"
                :value="unit.value"
              >
                {{ unit.label }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div class="sm:col-span-2 -mt-1 text-xs text-muted-foreground">
          当前将保存为 {{ formatDuration(rememberMeTtlSeconds) }}。
        </div>
      </div>
    </CardContent>

    <CardContent v-else class="min-h-[200px]" aria-hidden="true" />

    <div
      class="flex items-center justify-between rounded-b-xl border-t bg-muted/20 p-6"
    >
      <div class="text-sm text-muted-foreground">
        <span v-if="isDirty">您有未保存的更改</span>
        <span v-else>所有设置已是最新状态</span>
      </div>
      <div class="flex gap-3">
        <Button
          variant="ghost"
          @click="resetForm"
          :disabled="!isDirty || isSaving"
        >
          放弃
        </Button>
        <Button :disabled="!isDirty || isSaving" @click="saveSettings">
          <span
            v-if="isSaving"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          保存更改
        </Button>
      </div>
    </div>
  </Card>
</template>
