<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useThemeStore } from '../../stores/theme'
import { useBlogStore } from '../../stores/blog'
import { Search, X, BookOpen, ArrowRight, Tag } from 'lucide-vue-next'
import { formatDate } from '../../lib/utils'

const router = useRouter()
const themeStore = useThemeStore()
const blogStore = useBlogStore()

const query = ref('')
const searchInput = ref<HTMLInputElement | null>(null)

const results = computed(() => {
  const q = query.value.trim().toLowerCase()
  if (!q) return []
  return blogStore.posts
    .filter(p => p.is_public !== false)
    .filter(p =>
      p.title.toLowerCase().includes(q) ||
      p.excerpt?.toLowerCase().includes(q) ||
      p.category?.toLowerCase().includes(q) ||
      p.tags?.some(t => t.toLowerCase().includes(q))
    )
    .slice(0, 8)
})

watch(() => themeStore.searchModalOpen, (open) => {
  if (open) {
    query.value = ''
    nextTick(() => {
      searchInput.value?.focus()
    })
  }
})

const handleKeydown = (e: KeyboardEvent) => {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
    e.preventDefault()
    if (themeStore.searchModalOpen) {
      themeStore.closeSearch()
    } else {
      themeStore.openSearch()
    }
  }
}

onMounted(() => window.addEventListener('keydown', handleKeydown))
onUnmounted(() => window.removeEventListener('keydown', handleKeydown))

const goToPost = (post: any) => {
  themeStore.closeSearch()
  router.push(`/post/${post.slug || post.id}`)
}
</script>

<template>
  <Teleport to="body">
    <Transition
      enter-active-class="transition duration-150 ease-out"
      enter-from-class="opacity-0"
      enter-to-class="opacity-100"
      leave-active-class="transition duration-100 ease-in"
      leave-from-class="opacity-100"
      leave-to-class="opacity-0"
    >
      <div
        v-if="themeStore.searchModalOpen"
        class="fixed inset-0 z-50 flex items-start justify-center pt-20 p-4 bg-zinc-950/60 backdrop-blur-md"
        @click.self="themeStore.closeSearch"
      >
        <div class="w-full max-w-2xl bg-white dark:bg-zinc-900 rounded-2xl shadow-2xl border border-zinc-200 dark:border-zinc-800 overflow-hidden">
          <!-- 搜索输入框 -->
          <div class="relative flex items-center px-4 py-3.5 border-b border-zinc-200 dark:border-zinc-800">
            <Search class="w-5 h-5 text-zinc-400 ml-2 shrink-0" />
            <input
              ref="searchInput"
              v-model="query"
              type="text"
              placeholder="搜索文章标题、内容、分类或标签..."
              class="w-full px-3 py-1.5 bg-transparent text-zinc-900 dark:text-zinc-100 placeholder-zinc-400 focus:outline-none text-base"
              @keydown.esc="themeStore.closeSearch"
            />
            <button
              v-if="query"
              @click="query = ''"
              class="p-1 text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-200 transition mr-2"
            >
              <X class="w-4 h-4" />
            </button>
            <kbd class="hidden sm:inline-block px-2 py-0.5 text-xs font-semibold text-zinc-500 bg-zinc-100 dark:bg-zinc-800 border border-zinc-200 dark:border-zinc-700 rounded-md">ESC</kbd>
          </div>

          <!-- 搜索结果列表 -->
          <div class="max-h-96 overflow-y-auto p-3">
            <div v-if="query.trim() && results.length === 0" class="py-12 text-center text-zinc-400 text-sm">
              <BookOpen class="w-10 h-10 mx-auto mb-3 opacity-40" />
              未找到与“{{ query }}”相关的文章
            </div>

            <div v-else-if="!query.trim()" class="py-6 px-3">
              <div class="text-xs font-semibold uppercase tracking-wider text-zinc-400 mb-3">热门专栏分类</div>
              <div class="flex flex-wrap gap-2 mb-6">
                <button
                  v-for="cat in blogStore.computedCategories"
                  :key="cat.id"
                  @click="query = cat.name"
                  class="px-3 py-1.5 rounded-xl bg-zinc-100 hover:bg-emerald-50 hover:text-emerald-600 dark:bg-zinc-800 dark:hover:bg-emerald-950/40 dark:hover:text-emerald-400 text-xs font-medium text-zinc-600 dark:text-zinc-300 transition"
                >
                  {{ cat.name }} ({{ cat.count || 0 }})
                </button>
              </div>

              <div class="text-xs font-semibold uppercase tracking-wider text-zinc-400 mb-3">常用搜索标签</div>
              <div class="flex flex-wrap gap-1.5">
                <button
                  v-for="tag in blogStore.tags.slice(0, 8)"
                  :key="tag.name"
                  @click="query = tag.name"
                  class="badge-base bg-zinc-100 dark:bg-zinc-800 text-zinc-600 dark:text-zinc-400 hover:bg-zinc-200 dark:hover:bg-zinc-700 transition"
                >
                  <Tag class="w-3 h-3" />
                  #{{ tag.name }}
                </button>
              </div>
            </div>

            <div v-else class="space-y-1">
              <div
                v-for="post in results"
                :key="post.id"
                @click="goToPost(post)"
                class="flex items-center justify-between p-3 rounded-xl hover:bg-zinc-100 dark:hover:bg-zinc-800/80 cursor-pointer group transition"
              >
                <div class="min-w-0 flex-1 pr-4">
                  <div class="flex items-center gap-2 mb-1">
                    <span v-if="post.category" class="px-2 py-0.5 rounded-md text-[11px] font-semibold bg-emerald-100 text-emerald-700 dark:bg-emerald-950 dark:text-emerald-300">
                      {{ post.category }}
                    </span>
                    <span class="text-xs text-zinc-400">{{ formatDate(post.created_at) }}</span>
                  </div>
                  <h4 class="text-sm font-semibold text-zinc-800 dark:text-zinc-100 group-hover:text-emerald-600 dark:group-hover:text-emerald-400 truncate transition">
                    {{ post.title }}
                  </h4>
                  <p class="text-xs text-zinc-500 dark:text-zinc-400 truncate mt-0.5">
                    {{ post.excerpt }}
                  </p>
                </div>
                <ArrowRight class="w-4 h-4 text-zinc-400 group-hover:text-emerald-500 group-hover:translate-x-1 transition shrink-0" />
              </div>
            </div>
          </div>

          <!-- 底部提示 -->
          <div class="px-4 py-2.5 bg-zinc-50 dark:bg-zinc-950 border-t border-zinc-100 dark:border-zinc-800 flex items-center justify-between text-xs text-zinc-400">
            <span>支持中英双语检索 &amp; 标签分类</span>
            <span>按 ↵ 回车查看文章</span>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
