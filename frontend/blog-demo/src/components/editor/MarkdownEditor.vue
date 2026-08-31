<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted } from 'vue'
import { renderMarkdown } from '../../lib/markdown'
import { renderMermaidDiagrams } from '../../lib/mermaid'
import { useThemeStore } from '../../stores/theme'
import {
  Bold,
  Italic,
  Heading1,
  Heading2,
  Heading3,
  Quote,
  Code,
  Link,
  Image,
  List,
  ListOrdered,
  Table,
  Eye,
  Edit3,
  Columns,
  GitBranch,
} from 'lucide-vue-next'

const props = defineProps<{
  modelValue: string
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', val: string): void
}>()

const themeStore = useThemeStore()
const textareaRef = ref<HTMLTextAreaElement | null>(null)
const previewRef = ref<HTMLElement | null>(null)
const viewMode = ref<'edit' | 'preview' | 'split'>('split')

const previewHtml = computed(() => {
  return renderMarkdown(props.modelValue)
})

const updateDiagrams = () => {
  nextTick(() => {
    if (previewRef.value) {
      renderMermaidDiagrams(previewRef.value, themeStore.isDark)
    }
  })
}

watch([previewHtml, () => themeStore.isDark, viewMode], updateDiagrams)
onMounted(updateDiagrams)

const insertText = (before: string, after: string = '', defaultText: string = '') => {
  const el = textareaRef.value
  if (!el) return

  const start = el.selectionStart
  const end = el.selectionEnd
  const selected = props.modelValue.substring(start, end) || defaultText
  const replacement = `${before}${selected}${after}`

  const newValue = props.modelValue.substring(0, start) + replacement + props.modelValue.substring(end)
  emit('update:modelValue', newValue)

  setTimeout(() => {
    el.focus()
    el.setSelectionRange(start + before.length, start + before.length + selected.length)
  }, 0)
}
</script>

<template>
  <div class="card-base flex flex-col h-full border border-zinc-200 dark:border-zinc-800 overflow-hidden">
    <!-- 工具栏 -->
    <div class="flex flex-wrap items-center justify-between gap-2 px-4 py-2.5 bg-zinc-50 dark:bg-zinc-900 border-b border-zinc-200/80 dark:border-zinc-800 text-zinc-600 dark:text-zinc-400">
      <div class="flex flex-wrap items-center gap-1.5">
        <button
          @click="insertText('**', '**', '粗体文字')"
          type="button"
          class="w-7.5 h-7.5 rounded-lg bg-white dark:bg-zinc-800 text-zinc-700 dark:text-zinc-300 border border-zinc-200 dark:border-zinc-700 hover:bg-emerald-500 hover:text-white dark:hover:bg-emerald-600 dark:hover:text-white hover:border-emerald-500 dark:hover:border-emerald-600 flex-center transition-all duration-150 cursor-pointer shadow-2xs"
          title="粗体 (Ctrl+B)"
        >
          <Bold class="w-3.5 h-3.5" />
        </button>

        <button
          @click="insertText('*', '*', '斜体文字')"
          type="button"
          class="w-7.5 h-7.5 rounded-lg bg-white dark:bg-zinc-800 text-zinc-700 dark:text-zinc-300 border border-zinc-200 dark:border-zinc-700 hover:bg-emerald-500 hover:text-white dark:hover:bg-emerald-600 dark:hover:text-white hover:border-emerald-500 dark:hover:border-emerald-600 flex-center transition-all duration-150 cursor-pointer shadow-2xs"
          title="斜体 (Ctrl+I)"
        >
          <Italic class="w-3.5 h-3.5" />
        </button>

        <span class="w-px h-4 bg-zinc-200 dark:bg-zinc-700 mx-0.5"></span>

        <button
          @click="insertText('# ', '', '一级标题')"
          type="button"
          class="w-7.5 h-7.5 rounded-lg bg-white dark:bg-zinc-800 text-zinc-700 dark:text-zinc-300 border border-zinc-200 dark:border-zinc-700 hover:bg-emerald-500 hover:text-white dark:hover:bg-emerald-600 dark:hover:text-white hover:border-emerald-500 dark:hover:border-emerald-600 flex-center transition-all duration-150 cursor-pointer shadow-2xs"
          title="一级标题"
        >
          <Heading1 class="w-3.5 h-3.5" />
        </button>

        <button
          @click="insertText('## ', '', '二级标题')"
          type="button"
          class="w-7.5 h-7.5 rounded-lg bg-white dark:bg-zinc-800 text-zinc-700 dark:text-zinc-300 border border-zinc-200 dark:border-zinc-700 hover:bg-emerald-500 hover:text-white dark:hover:bg-emerald-600 dark:hover:text-white hover:border-emerald-500 dark:hover:border-emerald-600 flex-center transition-all duration-150 cursor-pointer shadow-2xs"
          title="二级标题"
        >
          <Heading2 class="w-3.5 h-3.5" />
        </button>

        <button
          @click="insertText('### ', '', '三级标题')"
          type="button"
          class="w-7.5 h-7.5 rounded-lg bg-white dark:bg-zinc-800 text-zinc-700 dark:text-zinc-300 border border-zinc-200 dark:border-zinc-700 hover:bg-emerald-500 hover:text-white dark:hover:bg-emerald-600 dark:hover:text-white hover:border-emerald-500 dark:hover:border-emerald-600 flex-center transition-all duration-150 cursor-pointer shadow-2xs"
          title="三级标题"
        >
          <Heading3 class="w-3.5 h-3.5" />
        </button>

        <span class="w-px h-4 bg-zinc-200 dark:bg-zinc-700 mx-0.5"></span>

        <button
          @click="insertText('> ', '', '引用内容')"
          type="button"
          class="w-7.5 h-7.5 rounded-lg bg-white dark:bg-zinc-800 text-zinc-700 dark:text-zinc-300 border border-zinc-200 dark:border-zinc-700 hover:bg-emerald-500 hover:text-white dark:hover:bg-emerald-600 dark:hover:text-white hover:border-emerald-500 dark:hover:border-emerald-600 flex-center transition-all duration-150 cursor-pointer shadow-2xs"
          title="引用块"
        >
          <Quote class="w-3.5 h-3.5" />
        </button>

        <button
          @click="insertText('```ts\n', '\n```', '// 在此编写代码')"
          type="button"
          class="w-7.5 h-7.5 rounded-lg bg-white dark:bg-zinc-800 text-zinc-700 dark:text-zinc-300 border border-zinc-200 dark:border-zinc-700 hover:bg-emerald-500 hover:text-white dark:hover:bg-emerald-600 dark:hover:text-white hover:border-emerald-500 dark:hover:border-emerald-600 flex-center transition-all duration-150 cursor-pointer shadow-2xs"
          title="代码块"
        >
          <Code class="w-3.5 h-3.5" />
        </button>

        <button
          @click="insertText('[', '](https://example.com)', '链接描述')"
          type="button"
          class="w-7.5 h-7.5 rounded-lg bg-white dark:bg-zinc-800 text-zinc-700 dark:text-zinc-300 border border-zinc-200 dark:border-zinc-700 hover:bg-emerald-500 hover:text-white dark:hover:bg-emerald-600 dark:hover:text-white hover:border-emerald-500 dark:hover:border-emerald-600 flex-center transition-all duration-150 cursor-pointer shadow-2xs"
          title="插入链接"
        >
          <Link class="w-3.5 h-3.5" />
        </button>

        <button
          @click="insertText('![图片描述](', ')', 'https://images.unsplash.com/photo-1618005182384-a83a8bd57fbe?w=1200')"
          type="button"
          class="w-7.5 h-7.5 rounded-lg bg-white dark:bg-zinc-800 text-zinc-700 dark:text-zinc-300 border border-zinc-200 dark:border-zinc-700 hover:bg-emerald-500 hover:text-white dark:hover:bg-emerald-600 dark:hover:text-white hover:border-emerald-500 dark:hover:border-emerald-600 flex-center transition-all duration-150 cursor-pointer shadow-2xs"
          title="插入图片"
        >
          <Image class="w-3.5 h-3.5" />
        </button>

        <button
          @click="insertText('- ', '', '列表项')"
          type="button"
          class="w-7.5 h-7.5 rounded-lg bg-white dark:bg-zinc-800 text-zinc-700 dark:text-zinc-300 border border-zinc-200 dark:border-zinc-700 hover:bg-emerald-500 hover:text-white dark:hover:bg-emerald-600 dark:hover:text-white hover:border-emerald-500 dark:hover:border-emerald-600 flex-center transition-all duration-150 cursor-pointer shadow-2xs"
          title="无序列表"
        >
          <List class="w-3.5 h-3.5" />
        </button>

        <button
          @click="insertText('1. ', '', '有序项')"
          type="button"
          class="w-7.5 h-7.5 rounded-lg bg-white dark:bg-zinc-800 text-zinc-700 dark:text-zinc-300 border border-zinc-200 dark:border-zinc-700 hover:bg-emerald-500 hover:text-white dark:hover:bg-emerald-600 dark:hover:text-white hover:border-emerald-500 dark:hover:border-emerald-600 flex-center transition-all duration-150 cursor-pointer shadow-2xs"
          title="有序列表"
        >
          <ListOrdered class="w-3.5 h-3.5" />
        </button>

        <button
          @click="insertText('| 标题 1 | 标题 2 |\n|---|---|\n| 内容 1 | 内容 2 |\n')"
          type="button"
          class="w-7.5 h-7.5 rounded-lg bg-white dark:bg-zinc-800 text-zinc-700 dark:text-zinc-300 border border-zinc-200 dark:border-zinc-700 hover:bg-emerald-500 hover:text-white dark:hover:bg-emerald-600 dark:hover:text-white hover:border-emerald-500 dark:hover:border-emerald-600 flex-center transition-all duration-150 cursor-pointer shadow-2xs"
          title="表格"
        >
          <Table class="w-3.5 h-3.5" />
        </button>

        <span class="w-px h-4 bg-zinc-200 dark:bg-zinc-700 mx-0.5"></span>

        <button
          @click="insertText('```mermaid\ngraph TD\n  A[客户端 Client] -->|API 请求| B(HertaBase 网关)\n  B --> C[(SurrealDB 数据库引擎)]\n  B --> D[RocksDB 存储引擎]\n```\n')"
          type="button"
          class="w-7.5 h-7.5 rounded-lg bg-white dark:bg-zinc-800 text-zinc-700 dark:text-zinc-300 border border-zinc-200 dark:border-zinc-700 hover:bg-emerald-500 hover:text-white dark:hover:bg-emerald-600 dark:hover:text-white hover:border-emerald-500 dark:hover:border-emerald-600 flex-center transition-all duration-150 cursor-pointer shadow-2xs"
          title="插入 Mermaid 流程图 / 架构图"
        >
          <GitBranch class="w-3.5 h-3.5" />
        </button>
      </div>

      <!-- 视图模式切换 -->
      <div class="flex items-center p-1 rounded-xl bg-zinc-200/80 dark:bg-zinc-950 border border-zinc-300/60 dark:border-zinc-800 text-xs">
        <button
          @click="viewMode = 'edit'"
          type="button"
          class="px-3 py-1.5 rounded-lg transition font-bold text-xs cursor-pointer"
          :class="viewMode === 'edit'
            ? 'bg-emerald-500 text-white shadow-sm shadow-emerald-500/20'
            : 'bg-transparent text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 hover:bg-white/60 dark:hover:bg-zinc-800'"
        >
          <Edit3 class="w-3.5 h-3.5 inline mr-1" /> 编辑
        </button>
        <button
          @click="viewMode = 'split'"
          type="button"
          class="px-3 py-1.5 rounded-lg transition hidden md:inline-flex items-center font-bold text-xs cursor-pointer"
          :class="viewMode === 'split'
            ? 'bg-emerald-500 text-white shadow-sm shadow-emerald-500/20'
            : 'bg-transparent text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 hover:bg-white/60 dark:hover:bg-zinc-800'"
        >
          <Columns class="w-3.5 h-3.5 inline mr-1" /> 双栏
        </button>
        <button
          @click="viewMode = 'preview'"
          type="button"
          class="px-3 py-1.5 rounded-lg transition font-bold text-xs cursor-pointer"
          :class="viewMode === 'preview'
            ? 'bg-emerald-500 text-white shadow-sm shadow-emerald-500/20'
            : 'bg-transparent text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 hover:bg-white/60 dark:hover:bg-zinc-800'"
        >
          <Eye class="w-3.5 h-3.5 inline mr-1" /> 预览
        </button>
      </div>
    </div>

    <!-- 编辑与预览区域 -->
    <div class="flex-1 flex min-h-[500px] overflow-hidden">
      <!-- Markdown 源码输入框 -->
      <div
        v-show="viewMode === 'edit' || viewMode === 'split'"
        class="flex-1 p-4 bg-white dark:bg-zinc-900 overflow-y-auto"
        :class="{ 'border-r border-zinc-200 dark:border-zinc-800': viewMode === 'split' }"
      >
        <textarea
          ref="textareaRef"
          :value="modelValue"
          @input="emit('update:modelValue', ($event.target as HTMLTextAreaElement).value)"
          placeholder="在此使用 Markdown 语法书写您的文章..."
          class="w-full h-full min-h-[460px] bg-transparent resize-none focus:outline-none font-mono text-sm leading-relaxed text-zinc-900 dark:text-zinc-100 placeholder-zinc-400"
        ></textarea>
      </div>

      <!-- 实时预览区 -->
      <div
        ref="previewRef"
        v-show="viewMode === 'preview' || viewMode === 'split'"
        class="flex-1 p-6 bg-zinc-50/50 dark:bg-zinc-950/50 overflow-y-auto"
      >
        <div
          v-if="previewHtml"
          class="markdown-body max-w-none"
          v-html="previewHtml"
        ></div>
        <div v-else class="py-20 text-center text-zinc-400 text-xs">
          开始在左侧编写内容，即刻在此获得所见即所得的预览效果...
        </div>
      </div>
    </div>
  </div>
</template>
