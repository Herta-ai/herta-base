<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import type { TOCItem } from '../../types/blog'
import { ListTree, Hash } from 'lucide-vue-next'

const props = defineProps<{
  items: TOCItem[]
}>()

const activeId = ref<string>('')

const scrollToHeading = (id: string) => {
  const el = document.getElementById(id)
  if (el) {
    el.scrollIntoView({ behavior: 'smooth', block: 'start' })
    activeId.value = id
  }
}

const handleScroll = () => {
  const headings = props.items.map(item => document.getElementById(item.id)).filter(Boolean) as HTMLElement[]
  if (!headings.length) return

  const scrollY = window.scrollY
  const offset = 120

  for (let i = headings.length - 1; i >= 0; i--) {
    const heading = headings[i]
    if (heading.offsetTop - offset <= scrollY) {
      activeId.value = heading.id
      return
    }
  }

  activeId.value = headings[0]?.id || ''
}

onMounted(() => {
  window.addEventListener('scroll', handleScroll, { passive: true })
  handleScroll()
})

onUnmounted(() => {
  window.removeEventListener('scroll', handleScroll)
})
</script>

<template>
  <div v-if="items.length > 0" class="card-base p-5 sticky top-24 max-h-[calc(100vh-8rem)] overflow-y-auto space-y-3">
    <div class="flex items-center gap-2 pb-2 border-b border-zinc-100 dark:border-zinc-800 text-zinc-900 dark:text-zinc-100">
      <ListTree class="w-4 h-4 text-emerald-500" />
      <h4 class="text-xs font-bold uppercase tracking-wider">文章大纲目录</h4>
    </div>

    <nav class="space-y-1 text-xs">
      <a
        v-for="item in items"
        :key="item.id"
        href="javascript:void(0)"
        @click="scrollToHeading(item.id)"
        class="block py-1 px-2.5 rounded-lg transition leading-relaxed truncate"
        :class="[
          item.level === 3 ? 'ml-3 text-[11px]' : 'font-medium',
          activeId === item.id
            ? 'bg-emerald-50 text-emerald-600 dark:bg-emerald-950/60 dark:text-emerald-400 font-bold border-l-2 border-emerald-500'
            : 'text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-200 hover:bg-zinc-100 dark:hover:bg-zinc-800/60'
        ]"
      >
        <span class="inline-flex items-center gap-1">
          <Hash v-if="activeId === item.id" class="w-3 h-3 text-emerald-500" />
          {{ item.text }}
        </span>
      </a>
    </nav>
  </div>
</template>
