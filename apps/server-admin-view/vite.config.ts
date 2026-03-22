import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'

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
          if (id.includes('node_modules/zrender')) {
            return 'zrender-vendor'
          }
          if (id.includes('node_modules/echarts/charts')) {
            return 'echarts-charts'
          }
          if (id.includes('node_modules/echarts/components')) {
            return 'echarts-components'
          }
          if (id.includes('node_modules/echarts/core') || id.includes('node_modules/echarts/renderers')) {
            return 'echarts-core'
          }
          if (id.includes('node_modules/echarts')) {
            return 'echarts-vendor'
          }
          if (id.includes('node_modules')) {
            return 'vendor'
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
