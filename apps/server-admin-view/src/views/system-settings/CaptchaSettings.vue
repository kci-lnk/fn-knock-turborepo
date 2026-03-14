<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue';
import { ExternalLink, Eye, EyeOff } from 'lucide-vue-next';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Skeleton } from '@/components/ui/skeleton';
import { toast } from '@admin-shared/utils/toast';
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction';
import { useDelayedLoading } from '@admin-shared/composables/useDelayedLoading';
import { CaptchaAPI } from '../../lib/api';
import type { CaptchaSettings as CaptchaSettingsModel } from '@frontend-core/captcha/types';

const settings = ref<CaptchaSettingsModel | null>(null);
const turnstileSiteFieldId = 'captcha-turnstile-public-token';
const turnstileSecretFieldId = 'captcha-turnstile-private-token';
const turnstileGettingStartedUrl = 'https://www.cloudflare-cn.com/application-services/products/turnstile/';
const isTurnstileSiteVisible = ref(false);
const isTurnstileSecretVisible = ref(false);
const form = reactive<CaptchaSettingsModel>({
  provider: 'pow',
  widget_mode: 'normal',
  pow: {},
  turnstile: {
    site_key: '',
    secret_key: '',
  },
});

const { isPending: isLoading, run: runLoadSettings } = useAsyncAction({
  onError: (error) => {
    toast.error('加载失败', { description: extractErrorMessage(error, '无法获取验证码设置') });
  },
});
const showLoadingSkeleton = useDelayedLoading(isLoading);
const { isPending: isSaving, run: runSaveSettings } = useAsyncAction({
  onError: (error) => {
    toast.error('保存失败', { description: extractErrorMessage(error, '验证码设置保存失败') });
  },
});

const isDirty = computed(() => {
  if (!settings.value) return false;
  return (
    settings.value.provider !== form.provider ||
    settings.value.turnstile.site_key !== form.turnstile.site_key ||
    settings.value.turnstile.secret_key !== form.turnstile.secret_key
  );
});

const applyFromSettings = (data: CaptchaSettingsModel) => {
  settings.value = data;
  form.provider = data.provider;
  form.widget_mode = 'normal';
  form.pow = {};
  form.turnstile.site_key = data.turnstile.site_key;
  form.turnstile.secret_key = data.turnstile.secret_key;
};

const fetchSettings = async () => {
  await runLoadSettings(async () => {
    const data = await CaptchaAPI.getSettings();
    applyFromSettings(data);
  });
};

const resetForm = () => {
  if (settings.value) applyFromSettings(settings.value);
};

const saveSettings = async () => {
  if (form.provider === 'turnstile') {
    if (!form.turnstile.site_key.trim() || !form.turnstile.secret_key.trim()) {
      toast.error('参数不完整', {
        description: '启用 Cloudflare Turnstile 时，需要填写 site_key 和 secret_key。',
      });
      return;
    }
  }

  await runSaveSettings(
    () => CaptchaAPI.updateSettings({
      provider: form.provider,
      widget_mode: 'normal',
      pow: {},
      turnstile: {
        site_key: form.turnstile.site_key.trim(),
        secret_key: form.turnstile.secret_key.trim(),
      },
    }),
    {
      onSuccess: (data) => {
        applyFromSettings(data);
        toast.success('验证码设置已更新');
      },
    },
  );
};

onMounted(fetchSettings);
</script>

<template>
  <Card>
    <CardHeader>
      <CardTitle class="text-md">验证码供应商</CardTitle>
      <CardDescription class="mt-1.5">
        配置登录页的人机验证方式。当前设置会作用于认证端登录流程，PoW 为内置方案，Cloudflare Turnstile 需要填写站点参数。
      </CardDescription>
    </CardHeader>

    <CardContent v-if="isLoading && showLoadingSkeleton" class="border-t p-0">
      <div class="space-y-4 p-6">
        <Skeleton class="h-6 w-1/3" />
        <Skeleton class="h-4 w-2/3" />
      </div>
    </CardContent>

    <CardContent v-else-if="!isLoading" class="border-t p-0 divide-y">
      <div class="flex flex-col gap-4 bg-muted/10 p-6 sm:flex-row sm:items-center sm:justify-between">
        <div class="min-w-0 space-y-1 sm:flex-1 sm:pr-6">
          <Label class="text-base">验证码类型</Label>
          <div class="text-sm text-muted-foreground">
            你可以在系统内置 PoW 验证码和 Cloudflare Turnstile 之间切换。
          </div>
        </div>
        <Select v-model="form.provider" :disabled="isSaving">
          <SelectTrigger class="w-full sm:shrink-0" style="width: min(100%, 300px);">
            <SelectValue placeholder="选择验证码类型" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="pow">PoW 验证码</SelectItem>
            <SelectItem value="turnstile">Cloudflare Turnstile</SelectItem>
          </SelectContent>
        </Select>
      </div>

      <div
        v-if="form.provider === 'turnstile'"
        class="divide-y animate-in fade-in slide-in-from-top-2 duration-300"
      >
        <div class="grid gap-4 bg-muted/10 p-6">
          <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
            <div class="space-y-1">
              <Label class="text-base">开通 Cloudflare Turnstile</Label>
              <div class="text-sm text-muted-foreground">
                如果你还没有创建 Turnstile 小组件，可以先打开官方指引，在 Cloudflare 后台完成开通后再把密钥填回这里。
              </div>
            </div>
            <Button as-child variant="outline" class="shrink-0">
              <a :href="turnstileGettingStartedUrl" target="_blank" rel="noreferrer noopener">
                <ExternalLink class="mr-2 h-4 w-4" />
                打开Turnstile
              </a>
            </Button>
          </div>
          <div class="grid gap-2 text-sm text-muted-foreground">
            <div>1. 登录 Cloudflare 后台，进入 Turnstile。</div>
            <div>2. 创建一个新的 Widget，类型保持普通可见模式即可。</div>
            <div>3. 创建完成后复制 站点密钥 和 密钥，粘贴到下面两个输入框。</div>
          </div>
        </div>

        <div class="captcha-key-row">
          <div class="captcha-key-copy min-w-0 space-y-1">
            <Label class="text-base" :for="turnstileSiteFieldId">站点密钥</Label>
            <div class="text-sm leading-relaxed text-muted-foreground">
              前端渲染 Turnstile 控件时使用，对应 Cloudflare 控制台中的站点公开密钥。
            </div>
          </div>
          <div class="captcha-key-input-wrap w-full">
            <div class="relative">
              <Input
                :id="turnstileSiteFieldId"
                v-model="form.turnstile.site_key"
                :type="isTurnstileSiteVisible ? 'text' : 'password'"
                name="captchaPublicToken"
                autocomplete="off"
                autocapitalize="off"
                autocorrect="off"
                spellcheck="false"
                data-form-type="other"
                data-1p-ignore="true"
                data-lpignore="true"
                data-bwignore="true"
                placeholder="填写 Turnstile site_key"
                class="pr-10"
                :disabled="isSaving"
              />
              <button
                type="button"
                class="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground transition-colors hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
                :aria-label="isTurnstileSiteVisible ? '隐藏 站点密钥' : '显示 站点密钥'"
                :disabled="isSaving"
                @click="isTurnstileSiteVisible = !isTurnstileSiteVisible"
              >
                <component :is="isTurnstileSiteVisible ? EyeOff : Eye" class="h-4 w-4" />
              </button>
            </div>
          </div>
        </div>

        <div class="captcha-key-row">
          <div class="captcha-key-copy min-w-0 space-y-1">
            <Label class="text-base" :for="turnstileSecretFieldId">密钥</Label>
            <div class="text-sm leading-relaxed text-muted-foreground">
              后端向 Cloudflare 校验 token 时使用，仅在服务端保存和消费。
            </div>
          </div>
          <div class="captcha-key-input-wrap w-full">
            <div class="relative">
              <Input
                :id="turnstileSecretFieldId"
                v-model="form.turnstile.secret_key"
                :type="isTurnstileSecretVisible ? 'text' : 'password'"
                name="captchaPrivateToken"
                autocomplete="new-password"
                autocapitalize="off"
                autocorrect="off"
                spellcheck="false"
                data-form-type="other"
                data-1p-ignore="true"
                data-lpignore="true"
                data-bwignore="true"
                placeholder="填写 Turnstile secret_key"
                class="pr-10"
                :disabled="isSaving"
              />
              <button
                type="button"
                class="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground transition-colors hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
                :aria-label="isTurnstileSecretVisible ? '隐藏 密钥' : '显示 密钥'"
                :disabled="isSaving"
                @click="isTurnstileSecretVisible = !isTurnstileSecretVisible"
              >
                <component :is="isTurnstileSecretVisible ? EyeOff : Eye" class="h-4 w-4" />
              </button>
            </div>
          </div>
        </div>
      </div>
    </CardContent>

    <CardContent v-else class="min-h-[200px]" aria-hidden="true" />

    <div class="flex items-center justify-between rounded-b-xl border-t bg-muted/20 p-6">
      <div class="text-sm text-muted-foreground">
        <span v-if="isDirty">您有未保存的更改</span>
        <span v-else>所有设置已是最新状态</span>
      </div>
      <div class="flex gap-3">
        <Button variant="ghost" @click="resetForm" :disabled="!isDirty || isSaving">放弃</Button>
        <Button :disabled="!isDirty || isSaving" @click="saveSettings">
          <span v-if="isSaving" class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
          保存更改
        </Button>
      </div>
    </div>
  </Card>
</template>

<style scoped>
.captcha-key-row {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  padding: 1.5rem;
}

@media (min-width: 768px) {
  .captcha-key-row {
    display: grid;
    grid-template-columns: 320px minmax(0, 1fr);
    align-items: start;
    column-gap: 2rem;
  }

  .captcha-key-copy {
    padding-top: 0.25rem;
  }

  .captcha-key-input-wrap {
    width: 88%;
    justify-self: end;
    margin-top: 0.875rem;
  }
}
</style>
