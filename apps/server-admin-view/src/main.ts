import { createApp } from "vue";
import "./assets/index.css";
import "nprogress/nprogress.css";
import App from "./App.vue";
import router from "./router";
import { pinia } from "./store";
import { createFnKnockI18n } from "@fn-knock/i18n/vue/admin";
import { applyStoredThemeMode } from "@/components/ui/theme-toggle";

applyStoredThemeMode();

const bootstrap = async () => {
  const app = createApp(App);
  const i18n = await createFnKnockI18n({ scope: "admin" });
  app.use(pinia);
  app.use(router);
  app.use(i18n);

  app.mount("#app");
};

void bootstrap();
