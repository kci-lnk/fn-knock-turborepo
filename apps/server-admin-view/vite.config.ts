import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'

const createChunkMatcher = (patterns: string[]) => (id: string) =>
  patterns.some((pattern) => id.includes(pattern))

const isFrameworkChunk = createChunkMatcher([
  'node_modules/vue/',
  'node_modules/@vue/',
  'node_modules/vue-router/',
  'node_modules/pinia/',
  'node_modules/@vueuse/',
])

const isUiChunk = createChunkMatcher([
  'node_modules/lucide-vue-next/',
  'node_modules/reka-ui/',
  'node_modules/@floating-ui/',
  'node_modules/@tanstack/',
  'node_modules/class-variance-authority/',
  'node_modules/clsx/',
  'node_modules/tailwind-merge/',
  'node_modules/vue-sonner/',
  'node_modules/nprogress/',
])

const isEchartsChunk = createChunkMatcher([
  'node_modules/vue-echarts/',
])

export default defineConfig({
  base: './',
  publicDir: path.resolve(__dirname, '../../packages/icons'),
  plugins: [
    vue(),
    tailwindcss(),
  ],
  optimizeDeps: {
    exclude: ['qrcode.vue'],
  },
  build: {
    rolldownOptions: {
      output: {
        manualChunks(id) {
          if (isFrameworkChunk(id)) {
            return 'framework'
          }
          if (isUiChunk(id)) {
            return 'ui-vendor'
          }
          if (id.includes('node_modules/zrender/')) {
            return 'zrender-vendor'
          }
          if (id.includes('node_modules/echarts/charts/')) {
            return 'echarts-charts'
          }
          if (id.includes('node_modules/echarts/components/')) {
            return 'echarts-components'
          }
          if (
            id.includes('node_modules/echarts/core/') ||
            id.includes('node_modules/echarts/renderers/')
          ) {
            return 'echarts-core'
          }
          if (isEchartsChunk(id)) {
            return 'echarts-vendor'
          }
        },
      },
    },
  },
  resolve: {
    alias: {
      '@/components/ui': path.resolve(__dirname, '../../packages/ui-vue/src/components/ui'),
      '@/lib/utils': path.resolve(__dirname, '../../packages/ui-vue/src/lib/utils.ts'),
      '@frontend-core': path.resolve(__dirname, '../../packages/frontend-core/src'),
      '@admin-shared': path.resolve(__dirname, '../../packages/admin-shared/src'),
      '@fn-knock/i18n/core': path.resolve(__dirname, '../../packages/i18n/src/core.ts'),
      '@fn-knock/i18n/vue/admin': path.resolve(__dirname, '../../packages/i18n/src/vue-admin.ts'),
      '@fn-knock/i18n/vue': path.resolve(__dirname, '../../packages/i18n/src/vue.ts'),
      '@fn-knock/i18n': path.resolve(__dirname, '../../packages/i18n/src/index.ts'),
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    proxy: {
      '/__fn-knock': {
        target: 'http://localhost:7998',
        changeOrigin: true,
      },
      '/api': {
        target: 'http://localhost:7998',
        changeOrigin: true,
      }
    }
  }
})
