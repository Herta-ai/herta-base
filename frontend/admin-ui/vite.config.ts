import process from 'node:process'
import { fileURLToPath } from 'node:url'
import { defineConfig, loadEnv } from 'vite'
import vue from '@vitejs/plugin-vue'
import vueJsx from '@vitejs/plugin-vue-jsx'
import unocss from 'unocss/vite'
import dayjs from 'dayjs'
import utc from 'dayjs/plugin/utc'
import timezone from 'dayjs/plugin/timezone'
import vueDevTools from 'vite-plugin-vue-devtools'
import icons from 'unplugin-icons/vite'
import iconsResolver from 'unplugin-icons/resolver'
import components from 'unplugin-vue-components/vite'
import vueRouter from 'vue-router/vite'
import layouts from 'vite-plugin-vue-layouts'
import { NaiveUiResolver } from 'unplugin-vue-components/resolvers'
import { FileSystemIconLoader } from 'unplugin-icons/loaders'
import { setupHtmlPlugin } from './build/html'
import { ICONS_ASSETS } from './src/const'

dayjs.extend(utc)
dayjs.extend(timezone)

const buildTime = dayjs.tz(Date.now(), 'Asia/Shanghai').format('YYYY-MM-DD HH:mm:ss')
const svgReplaceReg = /^<svg\s/

// https://vite.dev/config/
export default defineConfig((configEnv) => {
  const viteEnv = loadEnv(configEnv.mode, process.cwd()) as unknown as Env.ImportMeta
  return {
    base: viteEnv.VITE_BASE_URL,
    // @todo: bundledDev对虚拟模块支持不好，等待修复后再使用
    // https://github.com/vitejs/vite/issues/22864
    // experimental: {
    //   bundledDev: true,
    // },
    resolve: {
      alias: {
        '~': fileURLToPath(new URL('./', import.meta.url)),
        '@': fileURLToPath(new URL('./src', import.meta.url)),
      },
    },
    css: {
      preprocessorOptions: {
        scss: {
          api: 'modern-compiler',
          additionalData: `@use "@/styles/scss/global.scss" as *;`,
        },
      },
    },
    plugins: [
      vueRouter({
        dts: './src/types/typed-router.d.ts',
      }),
      vue(),
      vueJsx(),
      layouts(),
      unocss(),
      icons({
        customCollections: {
          local: FileSystemIconLoader(ICONS_ASSETS, svg =>
            svg.replace(svgReplaceReg, '<svg width="1em" height="1em" ')),
        },
      }),
      components({
        dts: './src/types/components.d.ts',
        resolvers: [
          NaiveUiResolver(),
          iconsResolver({
            prefix: 'i',
            customCollections: ['local'],
          }),
        ],
      }),
      vueDevTools({
        launchEditor: viteEnv.VITE_DEVTOOLS_LAUNCH_EDITOR || 'webstorm',
      }),
      setupHtmlPlugin(buildTime),
    ],
    define: {
      BUILD_TIME: JSON.stringify(buildTime),
    },
  }
})
