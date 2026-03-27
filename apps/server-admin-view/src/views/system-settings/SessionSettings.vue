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
import { isAnySubdomainRoutingMode } from "../../lib/reverse-proxy-submode";
import type {
  AuthCredentialSettings,
  HostMapping,
  PostLoginIpGrantMode,
} from "../../types";
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

const ipGrantDurationUnits = durationUnits.filter(
  (unit) =>
    unit.value === "second" || unit.value === "minute" || unit.value === "hour",
);

const durationUnitMap = Object.fromEntries(
  durationUnits.map((item) => [item.value, item.seconds]),
) as Record<DurationUnit, number>;

const form = reactive<{
  session: DurationField;
  rememberMe: DurationField;
  postLoginIpGrantMode: PostLoginIpGrantMode;
  customGrant: DurationField;
}>({
  session: {
    value: 24,
    unit: "hour",
  },
  rememberMe: {
    value: 1,
    unit: "year",
  },
  postLoginIpGrantMode: "follow_session",
  customGrant: {
    value: 1,
    unit: "hour",
  },
});

const postLoginIpGrantModeOptions: Array<{
  value: PostLoginIpGrantMode;
  title: string;
  description: string;
}> = [
  {
    value: "follow_session",
    title: "跟随会话",
    description: "登录后自动授权当前 IP，退出、踢出或过期时一并撤销。",
  },
  {
    value: "disabled",
    title: "不自动白名单IP",
    description: "只保留当前浏览器会话，同 IP 的新浏览器仍需重新登录。",
  },
  {
    value: "custom",
    title: "自定义",
    description: "给当前 IP 签发固定时长授权；到期前可继续访问，但主动退出登录时会立即撤销。",
  },
];

const { isPending: isLoading, run: runLoadSettings } = useAsyncAction({
  onError: (error) => {
    toast.error("加载失败", {
      description: extractErrorMessage(error, "无法获取会话设置"),
    });
  },
});
const showLoadingSkeleton = useDelayedLoading(isLoading);
const { isPending: isSaving, run: runSaveSettings } = useAsyncAction({
  onError: (error) => {
    toast.error("保存失败", {
      description: extractErrorMessage(error, "会话设置保存失败"),
    });
  },
});

const clampDurationValue = (value: unknown) =>
  Math.max(1, Math.floor(Number(value) || 0));

const toSeconds = (field: DurationField): number =>
  clampDurationValue(field.value) * durationUnitMap[field.unit];

const splitDuration = (
  seconds: number,
  units = durationUnits,
): DurationField => {
  const safeSeconds = Math.max(1, Math.floor(Number(seconds) || 1));
  const matchedUnit =
    [...units].reverse().find((unit) => safeSeconds % unit.seconds === 0) ??
    units[0] ??
    durationUnits[0]!;

  return {
    value: Math.max(1, safeSeconds / matchedUnit.seconds),
    unit: matchedUnit.value,
  };
};

const formatDuration = (seconds: number, units = durationUnits): string => {
  const normalized = splitDuration(seconds, units);
  const label =
    units.find((item) => item.value === normalized.unit)?.label ||
    normalized.unit;
  return `${normalized.value} ${label}`;
};

const isDirectMode = computed(() => configStore.config?.run_type === 0);
const isSubdomainRoutingMode = computed(() =>
  isAnySubdomainRoutingMode(configStore.config),
);
const normalizeDomainName = (value: string | null | undefined) =>
  String(value ?? "")
    .trim()
    .toLowerCase()
    .replace(/^\./, "")
    .replace(/\.$/, "");
const isHostWithinDomain = (host: string, domain: string): boolean => {
  const normalizedHost = normalizeDomainName(host);
  const normalizedDomain = normalizeDomainName(domain);
  if (!normalizedHost || !normalizedDomain) return false;
  return (
    normalizedHost === normalizedDomain ||
    normalizedHost.endsWith(`.${normalizedDomain}`)
  );
};
const isAuthServiceMapping = (mapping: HostMapping): boolean =>
  mapping.service_role === "auth";
const effectiveSharedCookieDomain = computed(() => {
  const explicit = configStore.config?.subdomain_mode?.cookie_domain?.trim();
  if (explicit) return explicit;
  const rootDomain = configStore.config?.subdomain_mode?.root_domain?.trim();
  return rootDomain || "";
});
const incompatibleCookieScopeHosts = computed(() => {
  if (!isSubdomainRoutingMode.value) return [];
  const sharedDomain = normalizeDomainName(effectiveSharedCookieDomain.value);
  const mappings = configStore.config?.host_mappings ?? [];
  return mappings
    .filter((mapping) => mapping.use_auth && !isAuthServiceMapping(mapping))
    .map((mapping) => normalizeDomainName(mapping.host))
    .filter(
      (host): host is string =>
        Boolean(host) &&
        (!sharedDomain || !isHostWithinDomain(host, sharedDomain)),
    );
});
const sessionTtlSeconds = computed(() => toSeconds(form.session));
const rememberMeTtlSeconds = computed(() => toSeconds(form.rememberMe));
const customGrantTtlSeconds = computed(() => toSeconds(form.customGrant));

const isDirty = computed(() => {
  if (!settings.value) return false;
  const storedGrantTtl = settings.value.post_login_ip_grant_ttl_seconds ?? 3600;
  const shouldCompareCustomGrantTtl =
    settings.value.post_login_ip_grant_mode === "custom" ||
    form.postLoginIpGrantMode === "custom";
  return (
    settings.value.session_ttl_seconds !== sessionTtlSeconds.value ||
    settings.value.remember_me_ttl_seconds !== rememberMeTtlSeconds.value ||
    settings.value.post_login_ip_grant_mode !== form.postLoginIpGrantMode ||
    (shouldCompareCustomGrantTtl &&
      storedGrantTtl !== customGrantTtlSeconds.value)
  );
});

const grantModeSummary = computed(() => {
  switch (form.postLoginIpGrantMode) {
    case "follow_session":
      return "登录成功后，当前 IP 会随浏览器会话一起获得授权。";
    case "disabled":
      return "只授权当前浏览器会话，不会额外给 IP 放行。";
    case "custom":
      return `当前将保存为固定 ${formatDuration(
        customGrantTtlSeconds.value,
        ipGrantDurationUnits,
      )} 的登录后 IP 授权；主动退出登录时会立即撤销。`;
    default:
      return "";
  }
});

const applyFromSettings = (data: AuthCredentialSettings) => {
  settings.value = data;
  Object.assign(form.session, splitDuration(data.session_ttl_seconds));
  Object.assign(form.rememberMe, splitDuration(data.remember_me_ttl_seconds));
  form.postLoginIpGrantMode = data.post_login_ip_grant_mode;
  Object.assign(
    form.customGrant,
    splitDuration(
      data.post_login_ip_grant_ttl_seconds ?? 3600,
      ipGrantDurationUnits,
    ),
  );
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
  const nextCustomGrantTtl = customGrantTtlSeconds.value;

  if (nextSessionTtl < 60 || nextRememberMeTtl < 60) {
    toast.error("时长过短", {
      description: "登录会话有效期至少需要 60 秒。",
    });
    return;
  }

  if (nextRememberMeTtl < nextSessionTtl) {
    toast.error("设置不合理", {
      description: "记住我有效期不能短于普通登录有效期。",
    });
    return;
  }

  if (form.postLoginIpGrantMode === "custom" && nextCustomGrantTtl < 60) {
    toast.error("设置不合理", {
      description: "自定义登录后 IP 授权时长至少需要 60 秒。",
    });
    return;
  }

  await runSaveSettings(
    () =>
      ConfigAPI.updateAuthCredentialSettings({
        session_ttl_seconds: nextSessionTtl,
        remember_me_ttl_seconds: nextRememberMeTtl,
        post_login_ip_grant_mode: form.postLoginIpGrantMode,
        post_login_ip_grant_ttl_seconds:
          form.postLoginIpGrantMode === "custom" ? nextCustomGrantTtl : null,
      }),
    {
      onSuccess: async (data) => {
        applyFromSettings(data);
        await configStore.loadConfig();
        toast.success("会话设置已更新");
      },
    },
  );
};

onMounted(fetchSettings);
</script>

<template>
  <Card>
    <CardHeader>
      <CardTitle class="text-md">凭据与会话</CardTitle>
      <CardDescription class="mt-1.5">
        统一管理登录凭据签发后的会话时长，以及非直连模式下登录成功后是否自动给当前
        IP 授权。
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
            已经签发的 Cookie、Redis 会话和登录后 IP
            授权不会被追溯改写，新设置会在下一次 TOTP 或 Passkey 登录时生效。
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
            用户勾选“记住我”后，浏览器会话保持有效的时长。
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

      <div class="space-y-4 p-6">
        <div
          v-if="isDirectMode"
          class="rounded-xl border border-zinc-200 bg-zinc-50/40 px-4 py-4"
        >
          <div class="text-sm font-medium text-zinc-900">当前为直连模式</div>
          <div class="mt-1 text-sm leading-6 text-zinc-700">
            登录后 IP
            授权策略仅在反代模式和子域模式下生效。直连模式下，这里的设置不会参与实际放行。
          </div>
        </div>

        <div class="space-y-1">
          <Label class="text-base">登录后 IP 授权方式</Label>
          <div class="text-sm text-muted-foreground">
            控制登录成功后是否顺带授权当前访问 IP。
          </div>
        </div>

        <div class="grid gap-3 md:grid-cols-3">
          <button
            v-for="option in postLoginIpGrantModeOptions"
            :key="option.value"
            type="button"
            class="rounded-xl border px-4 py-4 text-left transition-colors"
            :class="
              form.postLoginIpGrantMode === option.value
                ? 'border-primary bg-primary/5'
                : 'border-border bg-background hover:border-zinc-300'
            "
            :disabled="isSaving || isDirectMode"
            @click="form.postLoginIpGrantMode = option.value"
          >
            <div class="text-sm font-medium text-foreground">
              {{ option.title }}
            </div>
            <div class="mt-1 text-sm leading-6 text-muted-foreground">
              {{ option.description }}
            </div>
          </button>
        </div>

        <div
          v-if="form.postLoginIpGrantMode === 'custom'"
          class="grid gap-3 rounded-xl border bg-muted/15 p-4 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center"
        >
          <div class="space-y-1 pr-6">
            <Label class="text-base">自定义授权时长</Label>
            <div class="text-sm text-muted-foreground">
              当前 IP 在这段时间内可继续访问；若主动点击退出登录，会同时撤销这份登录后 IP 授权。
            </div>
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <Input
              v-model.number="form.customGrant.value"
              type="number"
              min="1"
              step="1"
              class="w-24 text-center"
              :disabled="isSaving || isDirectMode"
            />
            <Select
              v-model="form.customGrant.unit"
              :disabled="isSaving || isDirectMode"
            >
              <SelectTrigger class="w-[110px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="unit in ipGrantDurationUnits"
                  :key="unit.value"
                  :value="unit.value"
                >
                  {{ unit.label }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        <div
          class="rounded-lg bg-muted/20 px-4 py-3 text-sm text-muted-foreground"
        >
          {{ grantModeSummary }}
        </div>

        <div
          v-if="
            form.postLoginIpGrantMode === 'disabled' && isSubdomainRoutingMode
          "
          class="rounded-lg border border-zinc-200 bg-zinc-50/40 px-4 py-3 text-sm text-zinc-700"
        >
          <template v-if="effectiveSharedCookieDomain">
            当前浏览器会话会按共享 Cookie 域
            <code>{{ effectiveSharedCookieDomain }}</code>
            在同一父域下复用；
            <template v-if="incompatibleCookieScopeHosts.length > 0">
              以下 Host 不在这个父域下：
              <code>{{ incompatibleCookieScopeHosts.join("、") }}</code
              >，它们会改为在各自域名下单独登录，不会再复用统一登录态。
            </template>
            <template v-else>
              当前所有鉴权 Host 都在共享域内，可以直接复用这份登录态。
            </template>
          </template>
          <template v-else>
            当前没有可用的共享 Cookie 域。关闭模式下，不同 Host
            之间不会共享登录态，会分别使用各自 Host
            的浏览器会话并在各自域名下登录；如果希望复用一次登录，建议先配置根域名。
          </template>
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
