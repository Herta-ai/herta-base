import { defineConfig } from 'vite'
import react, { reactCompilerPreset } from '@vitejs/plugin-react'
import unocss from 'unocss/vite'
import babel from '@rolldown/plugin-babel'
import { tanstackRouter  } from '@tanstack/router-plugin/vite'
import path from 'node:path'

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    tanstackRouter({
      autoCodeSplitting: true
    }),
    react(),
    babel({ presets: [reactCompilerPreset()] }),
    unocss(),
  ],
  resolve: {
    alias: {
      '@': path.resolve(import.meta.dirname, './src'),
    },
  },
})
