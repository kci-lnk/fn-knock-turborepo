<template>
  <div class="min-h-screen flex flex-col bg-muted/40 p-4">
    <div class="flex flex-1 items-center justify-center">
      <Card v-if="isCheckingAuth" class="w-full max-w-sm">
        <CardHeader>
          <Skeleton class="h-8 w-44 mx-auto" />
          <Skeleton class="h-4 w-48 mx-auto mt-2" />
        </CardHeader>
        <CardContent class="flex flex-col gap-4">
          <Skeleton class="h-4 w-full" />
          <Skeleton class="h-9 w-full rounded-md" />
        </CardContent>
      </Card>

      <Card v-else class="w-full max-w-sm">
        <CardHeader>
          <CardTitle class="text-2xl text-center">安全验证已通过</CardTitle>
          <CardDescription class="text-center">
            您的IP已被允许访问
          </CardDescription>
        </CardHeader>

        <CardContent class="flex flex-col gap-4">
          <p class="text-sm text-center text-muted-foreground">如果不再需要访问，请点击下方按钮退出并撤销您的授权。</p>
          <div v-if="isPasskeySupported && !isPasskeyAvailable" class="flex flex-col gap-2">
            <Button class="w-full" :disabled="isPasskeyBinding" @click="handlePasskeyBind">
              <span v-if="isPasskeyBinding"
                class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
              开启 Passkey 一键登录
            </Button>
            <p class="text-xs text-center text-muted-foreground">当前浏览器支持 Passkey，但尚未绑定</p>
          </div>
          <p v-if="passkeyError" class="text-xs text-center text-destructive">{{ passkeyError }}</p>
          <Button variant="destructive" @click="handleLogout" class="w-full" :disabled="isLoading">
            <span v-if="isLoading" class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
            退出登录
          </Button>
        </CardContent>
      </Card>
    </div>

    <AuthFooter />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { useRouter } from 'vue-router';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Skeleton } from '@/components/ui/skeleton';
import {
    normalizeCreationOptions,
    serializeCredential,
} from '@frontend-core/passkey/utils';
import { apiClient } from '@/lib/api';
import AuthFooter from '@/components/AuthFooter.vue';

const router = useRouter();
const isLoading = ref(false);
const isPasskeySupported = ref(false);
const isPasskeyAvailable = ref(false);
const isPasskeyBinding = ref(false);
const passkeyError = ref('');
const isCheckingAuth = ref(true);

async function checkAuth() {
    try {
        const res = await apiClient.get('/verify');
        if (!res.data || res.data.success !== true) {
            console.warn('验证失败:', res.data?.message || '未知错误');
            await router.replace('/login');
            return false;
        }
        return true;
    } catch (e: any) {
        console.error('身份验证请求异常:', e);
        await router.replace('/login');
        return false;
    } finally {
        isCheckingAuth.value = false;
    }
}
onMounted(async () => {
    const isAuthenticated = await checkAuth();
    if (!isAuthenticated) {
        return;
    }
    await initPasskeySupport();
});

async function initPasskeySupport() {
    if (typeof window === 'undefined' || !window.PublicKeyCredential) {
        isPasskeySupported.value = false;
        return;
    }
    isPasskeySupported.value = true;
    try {
        const res = await apiClient.get('/passkey/status');
        isPasskeyAvailable.value = !!res.data?.data?.available;
    } catch {
        isPasskeyAvailable.value = false;
    }
}

async function handleLogout() {
    isLoading.value = true;
    try {
        await apiClient.get('/logout');
        router.push('/login');
    } catch (e) {
        console.error('Logout failed:', e);
    } finally {
        isLoading.value = false;
    }
}

async function handlePasskeyBind() {
    if (!isPasskeySupported.value || isPasskeyAvailable.value) return;
    isPasskeyBinding.value = true;
    passkeyError.value = '';
    try {
        const tokenRes = await apiClient.post('/passkey/bind-token');
        const bindToken = tokenRes.data?.data?.token;
        if (!bindToken) {
            throw new Error('无法获取绑定凭证');
        }
        const optionsRes = await apiClient.post('/passkey/register/options', {
            token: bindToken,
        });
        const creationOptions = normalizeCreationOptions(optionsRes.data.data);
        const credential = await navigator.credentials.create({
            publicKey: creationOptions,
        });
        if (!credential) {
            throw new Error('未获取到 Passkey 响应');
        }
        const deviceName =
            (navigator as any).userAgentData?.platform || navigator.platform || 'Unknown Device';
        const payload = serializeCredential(credential as PublicKeyCredential);
        const verifyRes = await apiClient.post('/passkey/register/verify', {
            token: bindToken,
            deviceName,
            credential: payload,
        });
        if (verifyRes.data.success) {
            isPasskeyAvailable.value = true;
            return;
        }
        throw new Error(verifyRes.data.message || 'Passkey 绑定失败');
    } catch (e: any) {
        passkeyError.value = e?.response?.data?.message || e?.message || 'Passkey 绑定失败';
    } finally {
        isPasskeyBinding.value = false;
    }
}
</script>
