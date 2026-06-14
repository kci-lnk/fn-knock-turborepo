import { createApp } from 'vue'
import './assets/index.css'
import App from './App.vue'
import router from './router'
import { createFnKnockI18n } from '@fn-knock/i18n/vue'

const app = createApp(App)
const i18n = createFnKnockI18n()
app.use(router)
app.use(i18n)

app.mount('#app')
