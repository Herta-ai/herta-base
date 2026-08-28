<script setup lang="ts">
import { ref, computed } from 'vue'
import { renderMarkdown } from '../../lib/markdown'
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
} from 'lucide-vue-next'

const props = defineProps<{
  modelValue: string
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', val: string): void
}>()

const textareaRef = ref<HTMLTextAreaElement | null>(null)
const viewMode = ref<'edit' | 'preview' | 'split'>('split')

const previewHtml = computed(() => {
  return renderMarkdown(props.modelValue)
})

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
    <div class="flex flex-wrap items-center justify-between gap-2 px-4 py-2.5 bg-zinc-50 dark:bg-zinc-900 border-b border-zinc-200 dark:border-zinc-800 text-zinc-600 dark:text-zinc-400">
      <div class="flex flex-wrap items-center gap-1">
        <button
          @click="insertText('**', '**', '粗体文字')"
          type="button"
          class="p-1.5 rounded-lg hover:bg-zinc-200 dark:hover:bg-zinc-800 hover:text-zinc-900 dark:hover:text-zinc-100 transition"
          title="粗体 (Ctrl+B)"
        >
          <Bold class="w-4 h-4" />
        </button>

        <button
          @click="insertText('*', '*', '斜体文字')"
          type="button"
          class="p-1.5 rounded-lg hover:bg-zinc-200 dark:hover:bg-zinc-800 hover:text-zinc-900 dark:hover:text-zinc-100 transition"
          title="斜体 (Ctrl+I)"
        >
          <Italic class="w-4 h-4" />
        </button>

        <span class="w-px h-4 bg-zinc-300 dark:bg-zinc-700 mx-1"></span>

        <button
          @click="insertText('# ', '', '一级标题')"
          type="button"
          class="p-1.5 rounded-lg hover:bg-zinc-200 dark:hover:bg-zinc-800 hover:text-zinc-900 dark:hover:text-zinc-100 transition"
          title="一级标题"
        >
          <Heading1 class="w-4 h-4" />
        </button>

        <button
          @click="insertText('## ', '', '二级标题')"
          type="button"
          class="p-1.5 rounded-lg hover:bg-zinc-200 dark:hover:bg-zinc-800 hover:text-zinc-900 dark:hover:text-zinc-100 transition"
          title="二级标题"
        >
          <Heading2 class="w-4 h-4" />
        </button>

        <button
          @click="insertText('### ', '', '三级标题')"
          type="button"
          class="p-1.5 rounded-lg hover:bg-zinc-200 dark:hover:bg-zinc-800 hover:text-zinc-900 dark:hover:text-zinc-100 transition"
          title="三级标题"
        >
          <Heading3 class="w-4 h-4" />
        </button>

        <span class="w-px h-4 bg-zinc-300 dark:bg-zinc-700 mx-1"></span>

        <button
          @click="insertText('> ', '', '引用内容')"
          type="button"
          class="p-1.5 rounded-lg hover:bg-zinc-200 dark:hover:bg-zinc-800 hover:text-zinc-900 dark:hover:text-zinc-100 transition"
          title="引用块"
        >
          <Quote class="w-4 h-4" />
        </button>

        <button
          @click="insertText('```ts\n', '\n```', '// 在此编写代码')"
          type="button"
          class="p-1.5 rounded-lg hover:bg-zinc-200 dark:hover:bg-zinc-800 hover:text-zinc-900 dark:hover:text-zinc-100 transition"
          title="代码块"
        >
          <Code class="w-4 h-4" />
        </button>

        <button
          @click="insertText('[', '](https://example.com)', '链接描述')"
          type="button"
          class="p-1.5 rounded-lg hover:bg-zinc-200 dark:hover:bg-zinc-800 hover:text-zinc-900 dark:hover:text-zinc-100 transition"
          title="插入链接"
        >
          <Link class="w-4 h-4" />
        </button>

        <button
          @click="insertText('![图片描述](', ')', 'https://images.unsplash.com/photo-1618005182384-a83a8bd57fbe?w=1200')"
          type="button"
          class="p-1.5 rounded-lg hover:bg-zinc-200 dark:hover:bg-zinc-800 hover:text-zinc-900 dark:hover:text-zinc-100 transition"
          title="插入图片"
        >
          <Image class="w-4 h-4" />
        </button>

        <button
          @click="insertText('- ', '', '列表项')"
          type="button"
          class="p-1.5 rounded-lg hover:bg-zinc-200 dark:hover:bg-zinc-800 hover:text-zinc-900 dark:hover:text-zinc-100 transition"
          title="无序列表"
        >
          <List class="w-4 h-4" />
        </button>

        <button
          @click="insertText('1. ', '', '有序项')"
          type="button"
          class="p-1.5 rounded-lg hover:bg-zinc-200 dark:hover:bg-zinc-800 hover:text-zinc-900 dark:hover:text-zinc-100 transition"
          title="有序列表"
        >
          <ListOrdered class="w-4 h-4" />
        </button>

        <button
          @click="insertText('| 标题 1 | 标题 2 |\n|---|---|\n| 内容 1 | 内容 2 |\n')"
          type="button"
          class="p-1.5 rounded-lg hover:bg-zinc-200 dark:hover:bg-zinc-800 hover:text-zinc-900 dark:hover:text-zinc-100 transition"
          title="表格"
        >
          <Table class="w-4 h-4" />
        </button>
      </div>

      <!-- 视图模式切换 -->
      <div class="flex items-center p-1 rounded-xl bg-zinc-200/70 dark:bg-zinc-800 text-xs">
        <button
          @click="viewMode = 'edit'"
          type="button"
          class="px-2.5 py-1 rounded-lg transition"
          :class="viewMode === 'edit' ? 'bg-white dark:bg-zinc-700 text-emerald-600 dark:text-emerald-400 font-bold shadow-xs' : 'text-zinc-500 hover:text-zinc-800 dark:hover:text-zinc-200'"
        >
          <Edit3 class="w-3.5 h-3.5 inline mr-1" /> 编辑
        </button>
        <button
          @click="viewMode = 'split'"
          type="button"
          class="px-2.5 py-1 rounded-lg transition hidden md:inline-flex items-center"
          :class="viewMode === 'split' ? 'bg-white dark:bg-zinc-700 text-emerald-600 dark:text-emerald-400 font-bold shadow-xs' : 'text-zinc-500 hover:text-zinc-800 dark:hover:text-zinc-200'"
        >
          <Columns class="w-3.5 h-3.5 inline mr-1" /> 双栏
        </button>
        <button
          @click="viewMode = 'preview'"
          type="button"
          class="px-2.5 py-1 rounded-lg transition"
          :class="viewMode === 'preview' ? 'bg-white dark:bg-zinc-700 text-emerald-600 dark:text-emerald-400 font-bold shadow-xs' : 'text-zinc-500 hover:text-zinc-800 dark:hover:text-zinc-200'"
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
