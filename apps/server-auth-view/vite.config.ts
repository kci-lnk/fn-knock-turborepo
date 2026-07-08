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
  'node_modules/vue-i18n/',
  'node_modules/@vueuse/',
  'node_modules/pinia/',
])

const isUiChunk = createChunkMatcher([
  'node_modules/lucide-vue-next/',
  'node_modules/reka-ui/',
  'node_modules/@floating-ui/',
  'node_modules/class-variance-authority/',
  'node_modules/clsx/',
  'node_modules/tailwind-merge/',
  'node_modules/vue-input-otp/',
])

const isAuthCoreChunk = createChunkMatcher([
  'node_modules/axios/',
  'node_modules/crypto-js/',
  'node_modules/altcha/',
  'node_modules/@altcha/',
  'packages/frontend-core/src/',
])

export default defineConfig({
  base: './',
  publicDir: path.resolve(__dirname, '../../packages/icons'),
  plugins: [
    vue({
            template: {
                compilerOptions: {
                    isCustomElement: function (tag) { return tag === 'altcha-widget'; }
                }
            }
    }),
    tailwindcss(),
  ],
  build: {
    cssMinify: 'esbuild',
    rolldownOptions: {
      output: {
        manualChunks(id) {
          if (isFrameworkChunk(id)) {
            return 'framework'
          }
          if (isUiChunk(id)) {
            return 'ui-vendor'
          }
          if (isAuthCoreChunk(id)) {
            return 'auth-core'
          }
        },
      },
    },
  },
  resolve: {
    alias: {
      '@/components/ui': path.resolve(__dirname, '../../packages/ui-vue/src/components/ui'),
      '@/lib/utils': path.resolve(__dirname, '../../packages/ui-vue/src/lib/utils.ts'),
      '@admin-shared': path.resolve(__dirname, '../../packages/admin-shared/src'),
      '@frontend-core': path.resolve(__dirname, '../../packages/frontend-core/src'),
      '@fn-knock/i18n/core': path.resolve(__dirname, '../../packages/i18n/src/core.ts'),
      '@fn-knock/i18n/vue/auth': path.resolve(__dirname, '../../packages/i18n/src/vue-auth.ts'),
      '@fn-knock/i18n/vue': path.resolve(__dirname, '../../packages/i18n/src/vue.ts'),
      '@fn-knock/i18n': path.resolve(__dirname, '../../packages/i18n/src/index.ts'),
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    proxy: {
      '/__fn-knock': {
        target: 'http://localhost:7997',
        changeOrigin: true,
      },
      '/api': {
        target: 'http://localhost:7997',
        changeOrigin: true,
      }
    }
  }
})
