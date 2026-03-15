import { ref } from "vue";

export const isRouteNavigating = ref(false);
export const pendingNavPath = ref<string | null>(null);
