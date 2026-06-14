import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'

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
  resolve: {
    alias: {
      '@/components/ui': path.resolve(__dirname, '../../packages/ui-vue/src/components/ui'),
      '@/lib/utils': path.resolve(__dirname, '../../packages/ui-vue/src/lib/utils.ts'),
      '@frontend-core': path.resolve(__dirname, '../../packages/frontend-core/src'),
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
