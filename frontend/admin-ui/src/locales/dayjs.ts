import { locale } from 'dayjs'
import 'dayjs/locale/zh-cn'
import 'dayjs/locale/en'
import { useAppStore } from '@/store/modules/app.ts'

/**
 * Set dayjs locale
 *
 * @param lang
 */
export function setDayjsLocale(lang: I18n.LangType = 'zh-CN') {
  const appStore = useAppStore()
  const localMap = {
    'zh-CN': 'zh-cn',
    'en-US': 'en',
  } satisfies Record<I18n.LangType, string>
  const l = lang || appStore.locale || 'zh-CN'

  locale(localMap[l])
}
