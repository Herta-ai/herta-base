import path from 'node:path';

import babel from '@rolldown/plugin-babel';
import { tanstackRouter } from '@tanstack/router-plugin/vite';
import react, { reactCompilerPreset } from '@vitejs/plugin-react';
import unocss from 'unocss/vite';
import { defineConfig } from 'vite';

// https://vite.dev/config/
export default defineConfig({
  base: '/webui/',
  plugins: [
    tanstackRouter({
      autoCodeSplitting: true,
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
  server: {
    // port: 5173,
    proxy: {
      '/api': 'http://127.0.0.1:8080',
      '/_/': 'http://127.0.0.1:8080',
    },
  },
});
