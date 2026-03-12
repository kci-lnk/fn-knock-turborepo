import { defineStore } from 'pinia';
import { ref } from 'vue';
import type { AppConfig, ProxyMapping } from '../types';
import { ConfigAPI } from '../lib/api';

export const useConfigStore = defineStore('config', () => {
    const config = ref<AppConfig | null>(null);
    const isLoading = ref(true);
    const isError = ref(false);

    async function loadConfig() {
        isLoading.value = true;
        isError.value = false;
        try {
            config.value = await ConfigAPI.getConfig();
        } catch (e) {
            console.error(e);
            isError.value = true;
        } finally {
            isLoading.value = false;
        }
    }

    async function saveDefaultRoute(path: string) {
        await ConfigAPI.updateDefaultRoute(path);
        if (config.value) config.value.default_route = path;
        await loadConfig();
    }

    async function setRunType(type: 0 | 1) {
        await ConfigAPI.updateRunType(type);
        if (config.value) config.value.run_type = type;
        await loadConfig(); // refresh to be safe
    }

    async function saveProxyMappings(mappings: ProxyMapping[]) {
        await ConfigAPI.updateProxyMappings(mappings);
        await loadConfig();
    }

    return {
        config,
        isLoading,
        isError,
        loadConfig,
        setRunType,
        saveProxyMappings,
        saveDefaultRoute
    };
});
