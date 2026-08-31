import mermaid from 'mermaid'

let initialized = false
let currentTheme = ''

/**
 * 初始化 Mermaid 配置
 */
export function initMermaid(isDark: boolean = false) {
  const theme = isDark ? 'dark' : 'default'
  if (initialized && currentTheme === theme) return

  mermaid.initialize({
    startOnLoad: false,
    theme: theme,
    securityLevel: 'loose',
    fontFamily: 'Inter, PingFang SC, Hiragino Sans GB, Microsoft YaHei, sans-serif',
    themeVariables: isDark
      ? {
          darkMode: true,
          background: '#18181b',
          mainBkg: '#27272a',
          textColor: '#f4f4f5',
          lineColor: '#10b981',
          primaryColor: '#059669',
          primaryTextColor: '#ffffff',
          primaryBorderColor: '#10b981',
          nodeBorder: '#3f3f46',
          clusterBkg: '#18181b',
          clusterBorder: '#3f3f46',
          titleColor: '#34d399',
          edgeLabelBackground: '#27272a',
        }
      : {
          darkMode: false,
          background: '#ffffff',
          mainBkg: '#f4f4f5',
          textColor: '#18181b',
          lineColor: '#10b981',
          primaryColor: '#10b981',
          primaryTextColor: '#ffffff',
          primaryBorderColor: '#059669',
          nodeBorder: '#e4e4e7',
          clusterBkg: '#fafafa',
          clusterBorder: '#e4e4e7',
          titleColor: '#059669',
          edgeLabelBackground: '#ffffff',
        },
  })

  initialized = true
  currentTheme = theme
}

/**
 * 渲染指定 DOM 容器内的所有 Mermaid 图表
 */
export async function renderMermaidDiagrams(container: HTMLElement | null, isDark: boolean = false) {
  if (!container) return

  // 根据当前暗黑模式状态重新初始化主题
  initMermaid(isDark)

  const elements = container.querySelectorAll<HTMLElement>('.mermaid-diagram[data-mermaid]')
  if (!elements.length) return

  for (let i = 0; i < elements.length; i++) {
    const el = elements[i]
    const rawCode = decodeURIComponent(el.getAttribute('data-mermaid') || '').trim()
    if (!rawCode) continue

    const diagramId = `mermaid-svg-${Date.now()}-${i}-${Math.random().toString(36).substr(2, 6)}`

    try {
      // 验证语法
      const isValid = await mermaid.parse(rawCode).catch(() => false)
      if (!isValid) {
        el.innerHTML = `<div class="p-4 rounded-xl bg-amber-500/10 border border-amber-500/30 text-amber-600 dark:text-amber-400 text-xs font-mono whitespace-pre-wrap"><div class="font-bold mb-1 flex items-center gap-1.5"><span>⚠️</span> Mermaid 语法编辑中 / 解析错误:</div><code>${rawCode}</code></div>`
        continue
      }

      const { svg } = await mermaid.render(diagramId, rawCode)
      el.innerHTML = svg

      // 调整内部 svg 宽高与响应式展示
      const svgEl = el.querySelector('svg')
      if (svgEl) {
        svgEl.style.maxWidth = '100%'
        svgEl.style.height = 'auto'
      }
    } catch (err: any) {
      el.innerHTML = `<div class="p-4 rounded-xl bg-amber-500/10 border border-amber-500/30 text-amber-600 dark:text-amber-400 text-xs font-mono whitespace-pre-wrap"><div class="font-bold mb-1 flex items-center gap-1.5"><span>⚠️</span> Mermaid 图表渲染异常:</div><code>${rawCode}</code></div>`
    }
  }
}
