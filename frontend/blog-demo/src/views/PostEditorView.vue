<script setup lang="ts">
import { ref, reactive, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import type { BlogPost } from '../types/blog'
import { useBlogStore } from '../stores/blog'
import { useAuthStore } from '../stores/auth'
import { useThemeStore } from '../stores/theme'
import MarkdownEditor from '../components/editor/MarkdownEditor.vue'
import PostInspector from '../components/editor/PostInspector.vue'
import { ArrowLeft, Globe, Lock, Save, Sparkles, Check } from 'lucide-vue-next'
import { countWords } from '../lib/utils'
import confetti from 'canvas-confetti'

const route = useRoute()
const router = useRouter()
const blogStore = useBlogStore()
const authStore = useAuthStore()
const themeStore = useThemeStore()

const saving = ref(false)
const isEditing = ref(false)

const postForm = reactive<Partial<BlogPost>>({
  title: '',
  slug: '',
  content: '',
  excerpt: '',
  cover_image: 'https://images.unsplash.com/photo-1618005182384-a83a8bd57fbe?w=1200&auto=format&fit=crop&q=80',
  category: '',
  tags: ['技术', 'HertaBase'],
  is_public: true,
  featured: false,
})

onMounted(async () => {
  const editId = route.query.id as string
  if (editId) {
    const existing = await blogStore.getPostBySlugOrId(editId)
    if (existing) {
      if (existing.author && authStore.user?.id && !authStore.isAdmin && existing.author !== authStore.user.id) {
        themeStore.addToast({ type: 'warning', message: '您无权编辑其他人创建的文章' })
        router.push('/admin')
        return
      }
      isEditing.value = true
      Object.assign(postForm, existing)
    }
  }
})

const handleSave = async (isPublic: boolean) => {
  if (!postForm.title?.trim()) {
    themeStore.addToast({ type: 'warning', message: '请填写文章标题' })
    return
  }

  if (!postForm.content?.trim()) {
    themeStore.addToast({ type: 'warning', message: '请在正文中编写文章内容' })
    return
  }

  saving.value = true
  postForm.is_public = isPublic

  try {
    if (isEditing.value && postForm.id) {
      await blogStore.updatePost(postForm.id, postForm)
    } else {
      const created = await blogStore.createPost(postForm)
      postForm.id = created.id
      isEditing.value = true
    }

    if (isPublic) {
      // 触发五彩纸屑庆祝特效
      try {
        confetti({
          particleCount: 80,
          spread: 70,
          origin: { y: 0.6 },
        })
      } catch {}
    }

    setTimeout(() => {
      router.push(`/post/${postForm.slug || postForm.id}`)
    }, 800)
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <div class="space-y-6 animate-fade-in max-w-7xl mx-auto">
    <!-- 顶部动作条 -->
    <div class="flex flex-wrap items-center justify-between gap-4 p-4 rounded-2xl bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800">
      <div class="flex items-center gap-3 flex-1 min-w-[280px]">
        <button
          @click="router.back()"
          class="p-2 rounded-xl text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-100 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition shrink-0"
        >
          <ArrowLeft class="w-5 h-5" />
        </button>

        <input
          v-model="postForm.title"
          type="text"
          placeholder="在此输入文章标题..."
          class="w-full text-lg sm:text-xl font-extrabold bg-transparent text-zinc-900 dark:text-zinc-100 placeholder-zinc-400 focus:outline-none"
        />
      </div>

      <div class="flex items-center gap-3 shrink-0">
        <span class="text-xs text-zinc-400 hidden sm:inline">
          共 {{ countWords(postForm.content) }} 字
        </span>

        <button
          @click="handleSave(false)"
          :disabled="saving"
          class="btn-secondary text-xs px-3.5 py-2"
        >
          <Lock class="w-3.5 h-3.5" />
          <span>存草稿</span>
        </button>

        <button
          @click="handleSave(true)"
          :disabled="saving"
          class="btn-primary text-xs px-4 py-2"
        >
          <Globe class="w-3.5 h-3.5" />
          <span>{{ saving ? '发布中...' : (isEditing ? '更新发布' : '公开发布') }}</span>
        </button>
      </div>
    </div>

    <!-- 写作工作室主体：左侧编辑器 + 右侧 Gutenberg 检查器 -->
    <div class="grid grid-cols-1 lg:grid-cols-12 gap-6 items-start">
      <!-- 左侧 Markdown 编辑器 -->
      <div class="lg:col-span-8 h-[calc(100vh-14rem)] min-h-[580px]">
        <MarkdownEditor v-model="postForm.content!" />
      </div>

      <!-- 右侧文章属性侧边栏 -->
      <div class="lg:col-span-4">
        <PostInspector
          :post="postForm"
          :saving="saving"
          @save="handleSave"
        />
      </div>
    </div>
  </div>
</template>
