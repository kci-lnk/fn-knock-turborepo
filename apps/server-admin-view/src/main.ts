import { createApp } from 'vue'
import './assets/index.css'
import 'driver.js/dist/driver.css'
import App from './App.vue'
import router from './router'
import { pinia } from './store'

const app = createApp(App)
app.use(pinia)
app.use(router)

app.mount('#app')
