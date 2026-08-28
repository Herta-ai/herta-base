import dayjs from 'dayjs'
import 'dayjs/locale/zh-cn'
import relativeTime from 'dayjs/plugin/relativeTime'

dayjs.extend(relativeTime)
dayjs.locale('zh-cn')

/**
 * 格式化标准日期：2026年3月15日
 */
export function formatDate(dateStr?: string | Date): string {
  if (!dateStr) return '未知时间'
  return dayjs(dateStr).format('YYYY年M月D日')
}

/**
 * 格式化相对时间：如 “3 分钟前”、“昨天 18:20”
 */
export function formatTimeAgo(dateStr?: string | Date): string {
  if (!dateStr) return '刚刚'
  const d = dayjs(dateStr)
  const now = dayjs()
  const diffDays = now.diff(d, 'day')

  if (diffDays === 0) {
    return d.fromNow()
  } else if (diffDays === 1) {
    return `昨天 ${d.format('HH:mm')}`
  } else if (diffDays < 7) {
    return `${diffDays}天前`
  } else if (d.year() === now.year()) {
    return d.format('M月D日')
  } else {
    return d.format('YYYY年M月D日')
  }
}

/**
 * 中文字数与词数统计
 */
export function countWords(content: string = ''): number {
  if (!content) return 0
  // 去除 markdown 语法标记
  const clean = content
    .replace(/```[\s\S]*?```/g, '')
    .replace(/#+\s/g, '')
    .replace(/[*_~`>]/g, '')
    .trim()
  // 匹配汉字与英文单词
  const chineseChars = clean.match(/[\u4e00-\u9fa5]/g) || []
  const englishWords = clean.replace(/[\u4e00-\u9fa5]/g, ' ').match(/[a-zA-Z0-9_-]+/g) || []
  return chineseChars.length + englishWords.length
}

/**
 * 估算阅读时长（中文按 350 字/分钟，代码/英文按 200 词/分钟）
 */
export function calculateReadingTime(content: string = ''): string {
  const words = countWords(content)
  const minutes = Math.max(1, Math.ceil(words / 320))
  return `${minutes} 分钟阅读`
}

/**
 * 截断文本并添加省略号
 */
export function truncateText(text: string = '', maxLength: number = 120): string {
  if (!text) return ''
  const clean = text
    .replace(/```[\s\S]*?```/g, '')
    .replace(/#+\s/g, '')
    .replace(/[*_~`>[\]()]/g, '')
    .replace(/\s+/g, ' ')
    .trim()
  if (clean.length <= maxLength) return clean
  return clean.slice(0, maxLength) + '...'
}

/**
 * 生成标题 Slug
 */
export function slugify(text: string = ''): string {
  return text
    .toLowerCase()
    .trim()
    .replace(/[^\w\s\u4e00-\u9fa5-]/g, '')
    .replace(/[\s_-]+/g, '-')
    .replace(/^-+|-+$/g, '')
}

/**
 * 生成头像备用 URL 或首字母颜色
 */
export function getAuthorAvatar(name: string = '博主', avatarUrl?: string): string {
  if (avatarUrl && avatarUrl.trim()) return avatarUrl
  const seed = encodeURIComponent(name)
  return `https://api.dicebear.com/7.x/bottts/svg?seed=${seed}&backgroundColor=b6e3f4,c0aede,d1d4f9,ffd5dc,ffdfbf`
}

/**
 * 复制文本到剪贴板
 */
export async function copyToClipboard(text: string): Promise<boolean> {
  try {
    if (navigator.clipboard && window.isSecureContext) {
      await navigator.clipboard.writeText(text)
      return true
    } else {
      const textarea = document.createElement('textarea')
      textarea.value = text
      textarea.style.position = 'fixed'
      textarea.style.opacity = '0'
      document.body.appendChild(textarea)
      textarea.select()
      const success = document.execCommand('copy')
      document.body.removeChild(textarea)
      return success
    }
  } catch {
    return false
  }
}
