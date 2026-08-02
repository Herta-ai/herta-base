// @unocss-include
import { usePreferredColorScheme } from '@vueuse/core'
import { getColorPalette, getRgb } from '@/utils/color'
import { DARK_CLASS } from '@/const'
import { localStg } from '@/utils/storage'
import { toggleHtmlClass } from '@/utils/common'
import { $t } from '@/locales'
import { themeSettings } from '@/theme/settings'

export function setupLoading() {
  const colorScheme = usePreferredColorScheme()
  const themeColor = themeSettings.themeColor
  const darkMode = localStg.get('darkMode') || colorScheme.value === 'dark'
  const palette = getColorPalette(themeColor)

  const { r, g, b } = getRgb(themeColor)

  const primaryColor = `--primary-color: ${r} ${g} ${b}`

  const svgCssVars = Array.from(palette.entries(), ([key, value]) => `--logo-color-${key}: ${value}`)
    .join(';')

  const cssVars = `${primaryColor}; ${svgCssVars}`

  if (darkMode) {
    toggleHtmlClass(DARK_CLASS).add()
  }

  const loadingClasses = [
    'left-0 top-0',
    'left-0 bottom-0 animate-delay-500',
    'right-0 top-0 animate-delay-1000',
    'right-0 bottom-0 animate-delay-1500',
  ]

  const dot = loadingClasses
    .map((item) => {
      return `<div class="absolute w-16px h-16px bg-primary rounded-8px animate-pulse ${item}"></div>`
    })
    .join('\n')

  const loading = `
<div class="fixed-center flex-col dark:bg-#121212" style="${cssVars}">
  <div class="w-128px h-128px">
    ${getLogoSvg()}
  </div>
  <div class="w-56px h-56px my-36px">
    <div class="relative h-full animate-spin">
      ${dot}
    </div>
  </div>
  <h2 class="text-28px font-500 text-primary">${$t('system.title')}</h2>
</div>`

  const app = document.getElementById('app')

  if (app) {
    app.innerHTML = loading
  }
}

function getLogoSvg() {
  return `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" aria-hidden="true" role="img" class="iconify iconify--logos" width="100%" height="100%" preserveAspectRatio="xMidYMid meet" viewBox="0 0 256 198"><path fill="#41B883" d="M204.8 0H256L128 220.8L0 0h97.92L128 51.2L157.44 0h47.36Z"></path><path fill="#41B883" d="m0 0l128 220.8L256 0h-51.2L128 132.48L50.56 0H0Z"></path><path fill="#35495E" d="M50.56 0L128 133.12L204.8 0h-47.36L128 51.2L97.92 0H50.56Z"></path></svg>`
}

setupLoading()
