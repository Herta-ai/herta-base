import MarkdownIt from 'markdown-it'
import hljs from 'highlight.js'
import DOMPurify from 'dompurify'
import type { TOCItem } from '../types/blog'
import { slugify } from './utils'

// 创建 markdown-it 实例
const md: MarkdownIt = new MarkdownIt({
  html: true,
  linkify: true,
  typographer: true,
  highlight: (str: string, lang: string): string => {
    // 拦截 Mermaid 图表代码块
    if (lang && lang.trim().toLowerCase() === 'mermaid') {
      return `<div class="mermaid-diagram my-6 flex justify-center overflow-x-auto p-4 rounded-2xl bg-zinc-50 dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 shadow-2xs" data-mermaid="${encodeURIComponent(str)}"><pre class="mermaid">${md.utils.escapeHtml(str)}</pre></div>`
    }

    if (lang && hljs.getLanguage(lang)) {
      try {
        const highlighted = hljs.highlight(str, { language: lang, ignoreIllegals: true }).value
        return `<pre class="hljs"><div class="code-header flex justify-between items-center text-xs text-zinc-400 mb-2 pb-1 border-b border-zinc-700/50"><span>${lang}</span><button class="copy-code-btn hover:text-emerald-400 cursor-pointer" data-code="${encodeURIComponent(str)}">复制</button></div><code>${highlighted}</code></pre>`
      } catch {
        // ignore error
      }
    }
    const escaped = md.utils.escapeHtml(str)
    return `<pre class="hljs"><code>${escaped}</code></pre>`
  },
})

// 自定义标题渲染，为 h2, h3 自动注入 id 属性以便目录锚点跳转
const defaultHeadingRenderer = md.renderer.rules.heading_open || ((tokens: any[], idx: number, options: any, _env: any, self: any) => self.renderToken(tokens, idx, options))

md.renderer.rules.heading_open = (tokens: any[], idx: number, options: any, env: any, self: any) => {
  const token = tokens[idx]
  const nextToken = tokens[idx + 1]
  if (nextToken && nextToken.content) {
    const headingText = nextToken.content.trim()
    const id = slugify(headingText) || `heading-${idx}`
    token.attrSet('id', id)
    token.attrSet('class', 'group relative cursor-pointer')
  }
  return defaultHeadingRenderer(tokens, idx, options, env, self)
}

/**
 * 渲染 Markdown 文本为安全的 HTML
 */
export function renderMarkdown(content: string = ''): string {
  if (!content) return ''
  const rawHtml = md.render(content)
  return DOMPurify.sanitize(rawHtml, {
    ADD_TAGS: ['svg', 'g', 'path', 'rect', 'circle', 'text', 'line', 'polygon', 'polyline', 'marker', 'defs', 'style', 'foreignObject', 'tspan', 'clippath', 'use'],
    ADD_ATTR: ['target', 'id', 'data-code', 'data-mermaid', 'viewBox', 'd', 'fill', 'stroke', 'stroke-width', 'transform', 'class', 'style', 'xmlns', 'width', 'height', 'x', 'y', 'rx', 'ry', 'cx', 'cy', 'r', 'points', 'marker-end', 'marker-start', 'text-anchor', 'font-family', 'font-size', 'font-weight', 'preserveAspectRatio'],
  })
}

/**
 * 从 Markdown 源码中提取 H2 与 H3 标题大纲
 */
export function extractTOC(content: string = ''): TOCItem[] {
  if (!content) return []
  const items: TOCItem[] = []
  const lines = content.split('\n')
  let inCodeBlock = false

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trim()
    if (line.startsWith('```')) {
      inCodeBlock = !inCodeBlock
      continue
    }
    if (inCodeBlock) continue

    const match = line.match(/^(#{2,3})\s+(.+)$/)
    if (match) {
      const level = match[1].length
      const text = match[2].replace(/[*_~`]/g, '').trim()
      const id = slugify(text) || `heading-${i}`
      items.push({
        id,
        text,
        level,
      })
    }
  }

  return items
}
