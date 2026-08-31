<script setup lang="ts">
import { ref } from 'vue'
import type { BlogPost } from '../../types/blog'
import { useBlogStore } from '../../stores/blog'
import { Sliders, Sparkles, Image, Tag, Layers, Eye, Globe, Lock, Plus, X } from 'lucide-vue-next'
import { slugify } from '../../lib/utils'

const props = defineProps<{
  post: Partial<BlogPost>
  saving?: boolean
}>()

const emit = defineEmits<{
  (e: 'save', isPublic: boolean): void
}>()

const blogStore = useBlogStore()
const newTagInput = ref('')

const addTag = () => {
  const tag = newTagInput.value.trim()
  if (tag && !props.post.tags?.includes(tag)) {
    if (!props.post.tags) props.post.tags = []
    props.post.tags.push(tag)
    newTagInput.value = ''
  }
}

const removeTag = (index: number) => {
  props.post.tags?.splice(index, 1)
}

const generateSlugFromTitle = () => {
  if (props.post.title && !props.post.slug) {
    props.post.slug = slugify(props.post.title)
  }
}

const randomCover = () => {
  const images = [
    'https://images.unsplash.com/photo-1618005182384-a83a8bd57fbe?w=1200&auto=format&fit=crop&q=80',
    'https://images.unsplash.com/photo-1555066931-4365d14bab8c?w=1200&auto=format&fit=crop&q=80',
    'https://images.unsplash.com/photo-1677442136019-21780efad99a?w=1200&auto=format&fit=crop&q=80',
    'https://images.unsplash.com/photo-1526374965328-7f61d4dc18c5?w=1200&auto=format&fit=crop&q=80',
    'https://images.unsplash.com/photo-1451187580459-43490279c0fa?w=1200&auto=format&fit=crop&q=80',
  ]
  const random = images[Math.floor(Math.random() * images.length)]
  props.post.cover_image = random
}
</script>

<template>
  <aside class="card-base p-6 space-y-6">
    <div class="flex items-center gap-2 pb-3 border-b border-zinc-100 dark:border-zinc-800 text-zinc-900 dark:text-zinc-100">
      <Sliders class="w-4 h-4 text-emerald-500" />
      <h3 class="text-xs font-bold uppercase tracking-wider">文章属性面板 (Inspector)</h3>
    </div>

    <!-- 发布操作按钮 -->
    <div class="space-y-2">
      <button
        @click="emit('save', true)"
        :disabled="saving"
        class="w-full btn-primary py-2.5 text-xs font-bold"
      >
        <Globe class="w-3.5 h-3.5" />
        {{ saving ? '正在同步...' : (post.is_public ? '更新已发布文章' : '公开发布文章') }}
      </button>

      <button
        @click="emit('save', false)"
        :disabled="saving"
        class="w-full btn-secondary py-2 text-xs"
      >
        <Lock class="w-3.5 h-3.5" />
        保存为私密草稿
      </button>
    </div>

    <!-- URL 别名 Slug -->
    <div class="space-y-1.5">
      <label class="text-xs font-semibold text-zinc-700 dark:text-zinc-300">URL 别名 (Slug)</label>
      <div class="flex gap-2">
        <input
          v-model="post.slug"
          type="text"
          placeholder="my-first-post"
          class="input-base text-xs font-mono"
        />
        <button
          @click="generateSlugFromTitle"
          type="button"
          class="btn-secondary text-[11px] px-3 shrink-0"
          title="根据标题自动生成"
        >
          自动
        </button>
      </div>
    </div>

    <!-- 专栏分类选择 -->
    <div class="space-y-1.5">
      <div class="flex items-center justify-between">
        <label class="text-xs font-semibold text-zinc-700 dark:text-zinc-300 flex items-center gap-1.5">
          <Layers class="w-3.5 h-3.5 text-emerald-500" />
          所属专栏目录
        </label>
        <button
          v-if="post.category"
          type="button"
          @click="post.category = ''"
          class="text-[11px] text-zinc-400 hover:text-rose-500 transition-colors flex items-center gap-0.5 cursor-pointer"
          title="清除选中的目录"
        >
          <X class="w-3 h-3" />
          <span>清除选中</span>
        </button>
      </div>
      <select
        v-model="post.category"
        class="w-full input-base text-xs"
      >
        <option value="">-- 未设置目录（留空） --</option>
        <option v-for="cat in blogStore.categories" :key="cat.id" :value="cat.name">
          {{ cat.name }}
        </option>
      </select>
    </div>

    <!-- 标签管理器 -->
    <div class="space-y-2">
      <label class="text-xs font-semibold text-zinc-700 dark:text-zinc-300 flex items-center gap-1.5">
        <Tag class="w-3.5 h-3.5 text-emerald-500" />
        标签设置
      </label>
      <div class="flex flex-wrap gap-1.5 mb-1.5">
        <span
          v-for="(tag, index) in post.tags || []"
          :key="tag"
          class="badge-base bg-zinc-100 dark:bg-zinc-800 text-zinc-800 dark:text-zinc-100 border border-zinc-200/90 dark:border-zinc-700 text-xs py-1 pl-2.5 pr-1.5 inline-flex items-center gap-1.5 shadow-2xs"
        >
          <span>#{{ tag }}</span>
          <button
            type="button"
            @click="removeTag(index)"
            class="w-4 h-4 rounded-full bg-zinc-200/80 dark:bg-zinc-700 text-zinc-500 dark:text-zinc-300 hover:bg-rose-500 hover:text-white dark:hover:bg-rose-600 dark:hover:text-white flex-center transition-colors cursor-pointer"
            title="移除标签"
            aria-label="移除标签"
          >
            <X class="w-2.5 h-2.5" />
          </button>
        </span>
      </div>
      <div class="flex gap-2">
        <input
          v-model="newTagInput"
          type="text"
          placeholder="输入标签按回车添加"
          class="input-base text-xs"
          @keydown.enter.prevent="addTag"
        />
        <button
          @click="addTag"
          type="button"
          class="btn-secondary text-xs px-3 shrink-0 flex-center"
          title="添加标签"
        >
          <Plus class="w-4 h-4" />
        </button>
      </div>
    </div>

    <!-- 封面图设置 -->
    <div class="space-y-2">
      <div class="flex items-center justify-between">
        <label class="text-xs font-semibold text-zinc-700 dark:text-zinc-300 flex items-center gap-1.5">
          <Image class="w-3.5 h-3.5 text-emerald-500" />
          封面图片
        </label>
        <button
          type="button"
          @click="randomCover"
          class="inline-flex items-center gap-1 px-2.5 py-1 rounded-lg text-[11px] font-semibold bg-emerald-50 dark:bg-emerald-950/80 text-emerald-600 dark:text-emerald-300 border border-emerald-200/60 dark:border-emerald-800/80 hover:bg-emerald-100 dark:hover:bg-emerald-900 transition-colors cursor-pointer shadow-2xs"
        >
          <Sparkles class="w-3 h-3 text-emerald-500" />
          随机高清封面
        </button>
      </div>

      <input
        v-model="post.cover_image"
        type="text"
        placeholder="https://images.unsplash.com/..."
        class="input-base text-xs font-mono"
      />

      <div v-if="post.cover_image" class="h-28 rounded-xl overflow-hidden bg-zinc-100 dark:bg-zinc-800 border border-zinc-200 dark:border-zinc-700 mt-2">
        <img :src="post.cover_image" alt="封面预览" class="w-full h-full object-cover" />
      </div>
    </div>

    <!-- 摘要设置 -->
    <div class="space-y-1.5">
      <label class="text-xs font-semibold text-zinc-700 dark:text-zinc-300">文章摘要 / Excerpt</label>
      <textarea
        v-model="post.excerpt"
        rows="3"
        placeholder="简要概括文章主旨，若留空将在首页截取前100字..."
        class="input-base text-xs resize-none"
      ></textarea>
    </div>

    <!-- 特色置顶头条现代 Switch 开关 -->
    <div
      @click="post.featured = !post.featured"
      class="p-3.5 rounded-xl bg-zinc-50 dark:bg-zinc-800/50 border border-zinc-200/80 dark:border-zinc-800 flex items-center justify-between cursor-pointer hover:border-emerald-500/40 dark:hover:border-emerald-500/40 transition select-none shadow-xs"
    >
      <div class="flex items-center gap-2.5">
        <div
          class="w-8 h-8 rounded-xl flex-center transition"
          :class="post.featured ? 'bg-amber-500/15 text-amber-500' : 'bg-zinc-200 dark:bg-zinc-700 text-zinc-400'"
        >
          <Sparkles class="w-4 h-4" />
        </div>
        <div>
          <div class="text-xs font-bold text-zinc-800 dark:text-zinc-200">设为首页置顶头条</div>
          <div class="text-[10px] text-zinc-400">将在首页大图头条位突出展示</div>
        </div>
      </div>

      <!-- 完美对齐的滑动开关 Switch -->
      <div
        class="relative w-11 h-6 rounded-full transition-colors duration-200 flex items-center px-0.5 shrink-0"
        :class="post.featured ? 'bg-emerald-500' : 'bg-zinc-300 dark:bg-zinc-700'"
      >
        <div
          class="w-5 h-5 rounded-full bg-white shadow-md transform transition-transform duration-200"
          :class="post.featured ? 'translate-x-5' : 'translate-x-0'"
        ></div>
      </div>
    </div>
  </aside>
</template>
