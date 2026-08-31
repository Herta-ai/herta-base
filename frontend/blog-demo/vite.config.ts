import { fileURLToPath, URL } from 'node:url'
import vue from '@vitejs/plugin-vue'
import unocss from 'unocss/vite'
import { defineConfig, loadEnv } from 'vite'

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '')
  const base = env.VITE_BASE_URL

  return {
    base,
    plugins: [
      vue(),
      unocss(),
    ],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
    server: {
      port: 5174,
      proxy: {
        '/api': {
          target: 'http://127.0.0.1:8080',
          changeOrigin: true,
        },
        '/_/': {
          target: 'http://127.0.0.1:8080',
          changeOrigin: true,
        },
        '/sse': {
          target: 'http://127.0.0.1:8080',
          changeOrigin: true,
        },
      },
    },
  }
})
