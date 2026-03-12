<template>
  <div class="min-h-screen flex flex-col items-center justify-center bg-muted/40 p-4 relative">
    <Card class="w-full max-w-sm">
      <CardHeader>
        <CardTitle class="text-2xl text-center">安全验证</CardTitle>
        <CardDescription class="text-center" v-if="!isAltchaVerified">
          请先完成下方的人机验证
        </CardDescription>
        <CardDescription class="text-center" v-else>
          请输入您的六位数动态密码完成登录
        </CardDescription>
      </CardHeader>

      <CardContent>
        <form @submit.prevent="handleLogin" class="flex flex-col gap-6 items-center" autocomplete="off">

          <div v-if="!isAltchaVerified && canUseNativeAltcha" class="w-full flex justify-center mt-2">
            <altcha-widget ref="altchaWidgetRef" :challengeurl="challengeUrl" @statechange="onAltchaStateChange"
              hidefooter hidelogo class="w-full"
              style="--altcha-color-border:pink;--altcha-border-width:3px;--altcha-border-radius:8px; --altcha-max-width: 360px;"
              :strings="JSON.stringify({
                label: '我不是机器人',
                verified: '验证通过',
                verifying: '正在验证...',
                wait: '请稍候...',
                error: '验证错误'
              })">
            </altcha-widget>
          </div>
          <div v-else-if="!isAltchaVerified" class="w-full mt-2 space-y-3">
            <Button type="button" class="w-full" :disabled="isAltchaFallbackLoading" @click="handleAltchaFallbackVerify">
              <span v-if="isAltchaFallbackLoading"
                class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
              {{ isAltchaFallbackLoading ? '正在验证...' : '我不是机器人' }}
            </Button>
          </div>

          <div class="w-full" v-if="isPasskeySupported && isPasskeyAvailable">
            <Button type="button" :variant="isAltchaVerified ? 'secondary' : 'default'" class="w-full"
              :disabled="isPasskeyLoading" @click="handlePasskeyLogin">
              <span v-if="isPasskeyLoading"
                class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
              Passkey 一键登录
            </Button>
          </div>


          <div class="w-full flex justify-center" v-if="isAltchaVerified">
            <InputOTP inputmode="numeric" :maxlength="6" v-model="token" :disabled="isLoading" :autofocus="true"
              autocomplete="off" data-form-type="other" data-1p-ignore="true" data-lpignore="true" data-bwignore="true">
              <InputOTPGroup>
                <InputOTPSlot v-for="i in 6" :key="i - 1" :index="i - 1" />
              </InputOTPGroup>
            </InputOTP>
          </div>

          <div class="w-full flex justify-center" v-if="isAltchaVerified">
            <div
              class="flex items-center justify-center space-x-3 py-2 px-4 rounded-lg transition-colors hover:bg-muted/50 cursor-pointer group">
              <Checkbox id="rememberMe" v-model:checked="rememberMe"
                class="data-[state=checked]:bg-primary data-[state=checked]:border-primary" />
              <label for="rememberMe"
                class="text-sm font-medium leading-none cursor-pointer select-none text-muted-foreground group-hover:text-foreground transition-colors">
                记住我
              </label>
            </div>
          </div>

          <Dialog :open="showErrorDialog" @update:open="showErrorDialog = $event">
            <DialogContent :show-close-button="false">
              <DialogHeader>
                <DialogTitle>提示</DialogTitle>
                <DialogDescription>
                  {{ errorMessage }}
                </DialogDescription>
              </DialogHeader>
              <DialogFooter>
                <Button @click="showErrorDialog = false">确定</Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>
          <Dialog :open="showPasskeyBindDialog" @update:open="showPasskeyBindDialog = $event">
            <DialogContent :show-close-button="false">
              <DialogHeader>
                <DialogTitle>开启 Passkey 一键登录</DialogTitle>
                <DialogDescription>
                  是否在当前设备上绑定 Passkey？绑定后可直接一键登录。
                </DialogDescription>
              </DialogHeader>
              <div v-if="passkeyBindError" class="text-sm text-destructive">{{ passkeyBindError }}</div>
              <DialogFooter class="gap-2">
                <Button variant="outline" @click="skipPasskeyBind">稍后再说</Button>
                <Button :disabled="isBindingPasskey" @click="handlePasskeyBind">
                  <span v-if="isBindingPasskey"
                    class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
                  立即开启
                </Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>
          <Button type="submit" class="w-full" :disabled="isLoading" v-if="isAltchaVerified">
            <span v-if="isLoading"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
            立即验证
          </Button>

        </form>
      </CardContent>
    </Card>

    <div 
      v-if="clientIp" 
      class="absolute bottom-6 flex items-center gap-1.5 text-xs text-muted-foreground/50 hover:text-muted-foreground transition-colors duration-300 select-none"
    >
      <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="w-3.5 h-3.5">
        <circle cx="12" cy="12" r="10"/>
        <path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20"/>
        <path d="M2 12h20"/>
      </svg>
      <span>{{ clientIp }}</span>
      <span v-if="ipLocation" class="mx-0.5 opacity-50">|</span>
      <span v-if="ipLocation">{{ ipLocation }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { useRouter } from 'vue-router';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import { InputOTP, InputOTPGroup, InputOTPSlot } from '@/components/ui/input-otp';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import {
  normalizeCreationOptions,
  normalizeRequestOptions,
  serializeCredential,
} from '@frontend-core/passkey/utils';
import CryptoJS from 'crypto-js';
import { apiClient, buildAuthApiPath } from '@/lib/api';

import 'altcha';

const router = useRouter();

const token = ref('');
const rememberMe = ref(false);
const errorMessage = ref('');
const showErrorDialog = ref(false);
const isLoading = ref(false);
const isPasskeySupported = ref(false);
const isPasskeyAvailable = ref(false);
const isPasskeyLoading = ref(false);
const showPasskeyBindDialog = ref(false);
const isBindingPasskey = ref(false);
const passkeyBindError = ref('');
const passkeyBindToken = ref('');
const pendingRunType = ref<0 | 1 | null>(null);

const clientIp = ref('');
const ipLocation = ref('');

const altchaWidgetRef = ref<any>(null);
const isAltchaVerified = ref(false);
const altchaPayload = ref('');
const canUseNativeAltcha = ref(true);
const isAltchaFallbackLoading = ref(false);

const ALTCHA_HASH_BATCH_SIZE = 2000;
const SUPPORTED_ALTCHA_ALGORITHMS = ['SHA-256', 'SHA-384', 'SHA-512'] as const;
const challengeUrl = buildAuthApiPath('/challenge');

type AltchaAlgorithm = (typeof SUPPORTED_ALTCHA_ALGORITHMS)[number];
type AltchaChallenge = {
  algorithm: AltchaAlgorithm;
  challenge: string;
  maxnumber: number;
  salt: string;
  signature: string;
};

function onAltchaStateChange(ev: CustomEvent) {
  if (ev.detail.state === 'verified') {
    isAltchaVerified.value = true;
    altchaPayload.value = ev.detail.payload;
    errorMessage.value = '';
  } else {
    isAltchaVerified.value = false;
    altchaPayload.value = '';
  }
}

async function fetchIpInfo() {
  try {
    const res = await apiClient.get('/ip');
    if (res.data && res.data.success) {
      clientIp.value = res.data.data.ip;
      ipLocation.value = res.data.data.location;
    }
  } catch (e) {
    console.error('Failed to fetch IP info:', e);
  }
}

onMounted(async () => {
  canUseNativeAltcha.value = typeof window !== 'undefined'
    && window.isSecureContext
    && typeof window.crypto !== 'undefined'
    && !!window.crypto.subtle
    && typeof window.crypto.subtle.digest === 'function';
  fetchIpInfo();
  await initPasskeySupport();
});

function hashAltchaInput(algorithm: AltchaAlgorithm, input: string): string {
  switch (algorithm) {
    case 'SHA-256':
      return CryptoJS.SHA256(input).toString(CryptoJS.enc.Hex);
    case 'SHA-384':
      return CryptoJS.SHA384(input).toString(CryptoJS.enc.Hex);
    case 'SHA-512':
      return CryptoJS.SHA512(input).toString(CryptoJS.enc.Hex);
    default:
      throw new Error(`Unsupported ALTCHA algorithm: ${algorithm}`);
  }
}

function normalizeAltchaChallenge(payload: any): AltchaChallenge {
  const algorithmRaw = String(payload?.algorithm || 'SHA-256').toUpperCase();
  if (!SUPPORTED_ALTCHA_ALGORITHMS.includes(algorithmRaw as AltchaAlgorithm)) {
    throw new Error('不支持的 ALTCHA 算法');
  }
  const challenge = String(payload?.challenge || '').toLowerCase();
  const salt = String(payload?.salt || '');
  const signature = String(payload?.signature || '');
  const maxnumber = Number(payload?.maxnumber);
  if (!challenge || !salt || !signature || !Number.isFinite(maxnumber) || maxnumber < 0) {
    throw new Error('ALTCHA challenge 数据无效');
  }
  return {
    algorithm: algorithmRaw as AltchaAlgorithm,
    challenge,
    maxnumber: Math.floor(maxnumber),
    salt,
    signature,
  };
}

async function solveAltchaChallenge(challenge: AltchaChallenge): Promise<number> {
  for (let number = 0; number <= challenge.maxnumber; number += 1) {
    const digest = hashAltchaInput(challenge.algorithm, `${challenge.salt}${number}`).toLowerCase();
    if (digest === challenge.challenge) {
      return number;
    }
    if (number > 0 && number % ALTCHA_HASH_BATCH_SIZE === 0) {
      await new Promise((resolve) => setTimeout(resolve, 0));
    }
  }
  throw new Error('人机验证求解失败，请刷新页面后重试');
}

function buildAltchaPayload(challenge: AltchaChallenge, number: number): string {
  return btoa(JSON.stringify({
    algorithm: challenge.algorithm,
    challenge: challenge.challenge,
    number,
    salt: challenge.salt,
    signature: challenge.signature,
  }));
}

async function handleAltchaFallbackVerify() {
  if (isAltchaFallbackLoading.value) return;
  isAltchaFallbackLoading.value = true;
  errorMessage.value = '';
  try {
    const res = await apiClient.get('/challenge');
    const challenge = normalizeAltchaChallenge(res.data);
    const number = await solveAltchaChallenge(challenge);
    altchaPayload.value = buildAltchaPayload(challenge, number);
    isAltchaVerified.value = true;
  } catch (e: any) {
    isAltchaVerified.value = false;
    altchaPayload.value = '';
    errorMessage.value = e?.response?.data?.message || e?.message || '人机验证失败，请重试';
    showErrorDialog.value = true;
  } finally {
    isAltchaFallbackLoading.value = false;
  }
}

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

async function handleLogin() {
  if (token.value.length !== 6) {
    errorMessage.value = '请输入完整的 6 位身份验证码';
    showErrorDialog.value = true;
    return;
  }
  if (!isAltchaVerified.value || !altchaPayload.value) {
    errorMessage.value = '请先完成人机验证';
    showErrorDialog.value = true;
    return;
  }

  isLoading.value = true;
  errorMessage.value = '';

  try {
    const res = await apiClient.post('/login', {
      token: token.value,
      altcha: altchaPayload.value,
      rememberMe: rememberMe.value
    });

    if (res.data.success) {
      const runType = res.data.data?.run_type;
      const passkey = res.data.data?.passkey;
      if (isPasskeySupported.value && passkey?.can_bind && passkey?.bind_token) {
        passkeyBindToken.value = passkey.bind_token;
        pendingRunType.value = runType;
        showPasskeyBindDialog.value = true;
        return;
      }
      completeLogin(runType);
    } else {
      errorMessage.value = res.data.message || '验证失败，请重试';
      showErrorDialog.value = true;
      resetLoginState();
    }
  } catch (e: any) {
    console.error('Login error:', e);
    errorMessage.value = e?.response?.data?.message || '验证失败，请重试';
    showErrorDialog.value = true;
    resetLoginState();
  } finally {
    isLoading.value = false;
  }
}

function completeLogin(runType: 0 | 1) {
  pendingRunType.value = null;
  if (runType === 0) {
    router.push('/');
  } else {
    window.location.href = '/';
  }
}

async function handlePasskeyLogin() {
  if (!isPasskeySupported.value || !isPasskeyAvailable.value) return;
  isPasskeyLoading.value = true;
  errorMessage.value = '';
  try {
    const optionsRes = await apiClient.post('/passkey/auth/options');
    const requestOptions = normalizeRequestOptions(optionsRes.data.data);
    const credential = await navigator.credentials.get({
      publicKey: requestOptions,
    });
    if (!credential) {
      throw new Error('未获取到 Passkey 响应');
    }
    const payload = serializeCredential(credential as PublicKeyCredential);
    const verifyRes = await apiClient.post('/passkey/auth/verify', {
      credential: payload,
      rememberMe: rememberMe.value,
    });
    if (verifyRes.data.success) {
      completeLogin(verifyRes.data.data?.run_type);
      return;
    }
    throw new Error(verifyRes.data.message || 'Passkey 验证失败');
  } catch (e: any) {
    errorMessage.value = e?.response?.data?.message || e?.message || 'Passkey 登录失败，请重试';
    showErrorDialog.value = true;
  } finally {
    isPasskeyLoading.value = false;
  }
}

async function handlePasskeyBind() {
  if (!passkeyBindToken.value) {
    passkeyBindError.value = '绑定凭证无效，请重新登录';
    return;
  }
  isBindingPasskey.value = true;
  passkeyBindError.value = '';
  try {
    const optionsRes = await apiClient.post('/passkey/register/options', {
      token: passkeyBindToken.value,
    });
    const creationOptions = normalizeCreationOptions(optionsRes.data.data);
    const credential = await navigator.credentials.create({
      publicKey: creationOptions,
    });
    if (!credential) {
      throw new Error('未获取到 Passkey 响应');
    }
    const deviceName = (navigator as any).userAgentData?.platform || navigator.platform || 'Unknown Device';
    const payload = serializeCredential(credential as PublicKeyCredential);
    const verifyRes = await apiClient.post('/passkey/register/verify', {
      token: passkeyBindToken.value,
      deviceName,
      credential: payload,
    });
    if (verifyRes.data.success) {
      isPasskeyAvailable.value = true;
      showPasskeyBindDialog.value = false;
      passkeyBindToken.value = '';
      if (pendingRunType.value !== null) {
        completeLogin(pendingRunType.value);
      }
      return;
    }
    throw new Error(verifyRes.data.message || 'Passkey 绑定失败');
  } catch (e: any) {
    passkeyBindError.value = e?.response?.data?.message || e?.message || 'Passkey 绑定失败';
  } finally {
    isBindingPasskey.value = false;
  }
}

function skipPasskeyBind() {
  showPasskeyBindDialog.value = false;
  passkeyBindToken.value = '';
  passkeyBindError.value = '';
  if (pendingRunType.value !== null) {
    completeLogin(pendingRunType.value);
  }
}

function resetLoginState() {
  token.value = '';
  isAltchaVerified.value = false;
  altchaPayload.value = '';
  if (canUseNativeAltcha.value && altchaWidgetRef.value) {
    altchaWidgetRef.value.reset();
  }
}
</script>
