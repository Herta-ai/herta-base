import {
  defineConfig,
  presetIcons,
  presetTypography,
  presetWind3,
  transformerDirectives,
  transformerVariantGroup,
} from 'unocss'

export default defineConfig({
  shortcuts: {
    'flex-center': 'flex justify-center items-center',
    'flex-between': 'flex justify-between items-center',
    'card-base': 'bg-white dark:bg-zinc-900 border border-zinc-200/80 dark:border-zinc-800 rounded-2xl transition-all duration-300',
    'card-hover': 'hover:shadow-xl hover:shadow-zinc-200/50 dark:hover:shadow-zinc-950/50 hover:border-zinc-300 dark:hover:border-zinc-700',
    'btn-primary': 'inline-flex items-center justify-center gap-2 px-4 py-2 rounded-xl bg-gradient-to-r from-emerald-500 to-teal-600 hover:from-emerald-600 hover:to-teal-700 text-white font-medium shadow-md shadow-emerald-500/20 active:scale-98 transition duration-200 cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed',
    'btn-secondary': 'inline-flex items-center justify-center gap-2 px-4 py-2 rounded-xl bg-zinc-100 hover:bg-zinc-200 dark:bg-zinc-800 dark:hover:bg-zinc-700 text-zinc-800 dark:text-zinc-200 font-medium transition duration-200 cursor-pointer',
    'btn-outline': 'inline-flex items-center justify-center gap-2 px-4 py-2 rounded-xl border border-zinc-300 dark:border-zinc-700 hover:bg-zinc-100 dark:hover:bg-zinc-800 text-zinc-700 dark:text-zinc-300 font-medium transition duration-200 cursor-pointer',
    'input-base': 'w-full px-4 py-2.5 rounded-xl bg-zinc-50 dark:bg-zinc-800/60 border border-zinc-200 dark:border-zinc-700 text-zinc-900 dark:text-zinc-100 placeholder-zinc-400 dark:placeholder-zinc-500 focus:outline-none focus:ring-2 focus:ring-emerald-500/40 focus:border-emerald-500 transition duration-200',
    'badge-base': 'inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full text-xs font-semibold',
  },
  theme: {
    fontFamily: {
      sans: ['"Inter"', '"PingFang SC"', '"Hiragino Sans GB"', '"Microsoft YaHei"', 'sans-serif'],
      serif: ['"Noto Serif SC"', '"Source Han Serif CN"', 'Georgia', 'serif'],
      mono: ['"JetBrains Mono"', '"Fira Code"', 'monospace'],
    },
    colors: {
      brand: {
        50: '#ecfdf5',
        100: '#d1fae5',
        200: '#a7f3d0',
        300: '#6ee7b7',
        400: '#34d399',
        500: '#10b981',
        600: '#059669',
        700: '#047857',
        800: '#065f46',
        900: '#064e3b',
      },
    },
  },
  presets: [
    presetWind3(),
    presetIcons({
      scale: 1.2,
      warn: true,
      extraProperties: {
        'display': 'inline-block',
        'vertical-align': 'middle',
      },
    }),
    presetTypography(),
  ],
  transformers: [
    transformerDirectives(),
    transformerVariantGroup(),
  ],
})
