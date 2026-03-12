<template>
  <footer class="flex justify-center px-4 pt-6 pb-2 text-xs text-muted-foreground/60">
    <div class="w-full max-w-sm flex flex-col items-center gap-2">
      <div
        v-if="clientIp"
        class="w-full flex flex-col items-center gap-1 text-center sm:flex-row sm:flex-wrap sm:justify-center sm:gap-x-1.5 sm:gap-y-1"
      >
        <span class="inline-flex items-center gap-1.5 min-w-0 sm:shrink-0">
          <span class="break-all text-center sm:break-normal sm:whitespace-nowrap">{{ clientIp }}</span>
        </span>
        <span
          v-if="ipLocation"
          class="break-words sm:break-normal sm:whitespace-nowrap sm:shrink-0"
        >
          {{ ipLocation }}
        </span>
      </div>

      <div class="flex items-center justify-center gap-2">
        <a
          :href="APP_GITHUB_URL"
          target="_blank"
          rel="noopener noreferrer"
          class="inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 transition-colors hover:text-foreground hover:bg-background/70"
          title="打开 GitHub 项目页"
        >
          <Github class="h-3.5 w-3.5" />
          <span>fn-knock-turborepo</span>
        </a>
      </div>
    </div>
  </footer>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { ArrowUpRight, Github, Globe } from 'lucide-vue-next';
import { apiClient } from '@/lib/api';

const APP_GITHUB_URL = 'https://github.com/kci-lnk/fn-knock-turborepo';

const clientIp = ref('');
const ipLocation = ref('');

const projectEntryHref = computed(() => {
  if (typeof window === 'undefined') return '/';

  const pathname = window.location.pathname || '/';

  if (pathname === '/__auth__' || pathname === '/__auth__/') {
    return '/';
  }

  if (pathname.startsWith('/__auth__/')) {
    return pathname.replace(/^\/__auth__/, '') || '/';
  }

  if (pathname === '/auth' || pathname === '/auth/') {
    return '/';
  }

  if (pathname.startsWith('/auth/')) {
    return pathname.replace(/^\/auth/, '') || '/';
  }

  return '/';
});

async function fetchIpInfo() {
  try {
    const res = await apiClient.get('/ip');
    if (res.data?.success) {
      clientIp.value = res.data.data.ip;
      ipLocation.value = res.data.data.location;
    }
  } catch (error) {
    console.error('Failed to fetch IP info:', error);
  }
}

onMounted(() => {
  void fetchIpInfo();
});
</script>
