<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useBlogStore } from '../stores/blog'
import { useThemeStore } from '../stores/theme'
import { renderMarkdown, extractTOC } from '../lib/markdown'
import { formatDate, calculateReadingTime, countWords, getAuthorAvatar, copyToClipboard } from '../lib/utils'
import TableOfContents from '../components/post/TableOfContents.vue'
import ShareBar from '../components/post/ShareBar.vue'
import AuthorBio from '../components/post/AuthorBio.vue'
import RelatedPosts from '../components/post/RelatedPosts.vue'
import CommentList from '../components/comments/CommentList.vue'
import { Calendar, Clock, Eye, FileText, ChevronRight, Home, Sparkles, ArrowLeft } from 'lucide-vue-next'

const route = useRoute()
const router = useRouter()
const blogStore = useBlogStore()
const themeStore = useThemeStore()

const loading = ref(true)
const post = computed(() => blogStore.currentPost)

const toc = computed(() => {
  return extractTOC(post.value?.content || '')
})

const renderedContent = computed(() => {
  return renderMarkdown(post.value?.content || '')
})

const loadPost = async () => {
  const idOrSlug = route.params.slug as string
  if (!idOrSlug) return

  loading.value = true
  await blogStore.getPostBySlugOrId(idOrSlug)
  loading.value = false
  window.scrollTo({ top: 0, behavior: 'instant' })
}

watch(() => route.params.slug, () => {
  loadPost()
})

onMounted(() => {
  loadPost()

  // 监听代码块内“复制”按钮点击
  document.addEventListener('click', async (e) => {
    const target = (e.target as HTMLElement).closest('.copy-code-btn')
    if (target) {
      const code = decodeURIComponent(target.getAttribute('data-code') || '')
      if (code) {
        const ok = await copyToClipboard(code)
        if (ok) {
          const originalText = target.textContent
          target.textContent = '已复制!'
          setTimeout(() => {
            target.textContent = originalText
          }, 1500)
        }
      }
    }
  })
})
</script>

<template>
  <div class="max-w-6xl mx-auto space-y-8 animate-fade-in">
    <!-- 面包屑导航 -->
    <nav class="flex items-center gap-2 text-xs text-zinc-400">
      <router-link to="/" class="flex items-center gap-1 hover:text-emerald-500 transition">
        <Home class="w-3.5 h-3.5" />
        首页
      </router-link>
      <ChevronRight class="w-3 h-3" />
      <router-link
        v-if="post?.category"
        :to="`/category/${post.category}`"
        class="hover:text-emerald-500 transition"
      >
        {{ post.category }}
      </router-link>
      <ChevronRight v-if="post?.category" class="w-3 h-3" />
      <span class="text-zinc-600 dark:text-zinc-300 truncate max-w-xs">{{ post?.title || '文章加载中' }}</span>
    </nav>

    <!-- 加载中骨架屏 -->
    <div v-if="loading" class="space-y-6">
      <div class="h-12 w-3/4 rounded-2xl skeleton-shimmer"></div>
      <div class="h-80 w-full rounded-3xl skeleton-shimmer"></div>
      <div class="space-y-3">
        <div class="h-4 w-full rounded skeleton-shimmer"></div>
        <div class="h-4 w-5/6 rounded skeleton-shimmer"></div>
        <div class="h-4 w-4/6 rounded skeleton-shimmer"></div>
      </div>
    </div>

    <!-- 文章未找到 404 -->
    <div v-else-if="!post" class="card-base p-16 text-center space-y-4">
      <h2 class="text-2xl font-bold text-zinc-800 dark:text-zinc-200">未找到该文章</h2>
      <p class="text-xs text-zinc-500">文章可能已被作者删除，或链接地址有误。</p>
      <router-link to="/" class="inline-flex btn-primary text-xs px-4 py-2">
        <ArrowLeft class="w-4 h-4" />
        返回博客首页
      </router-link>
    </div>

    <!-- 文章详情主体 -->
    <article v-else class="space-y-8">
      <!-- 文章头部元数据卡片 -->
      <header class="space-y-5">
        <div class="flex flex-wrap items-center gap-3">
          <span v-if="post.category" class="px-3 py-1 rounded-lg text-xs font-bold bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border border-emerald-500/20">
            {{ post.category }}
          </span>
          <div class="flex items-center gap-1.5 text-xs text-zinc-400">
            <Calendar class="w-3.5 h-3.5" />
            <span>{{ formatDate(post.created_at) }}</span>
          </div>
          <div class="flex items-center gap-1.5 text-xs text-zinc-400">
            <Clock class="w-3.5 h-3.5" />
            <span>{{ calculateReadingTime(post.content) }}</span>
          </div>
          <div class="flex items-center gap-1.5 text-xs text-zinc-400">
            <FileText class="w-3.5 h-3.5" />
            <span>约 {{ countWords(post.content) }} 字</span>
          </div>
          <div class="flex items-center gap-1.5 text-xs text-zinc-400">
            <Eye class="w-3.5 h-3.5" />
            <span>{{ post.views || 1 }} 浏览</span>
          </div>
        </div>

        <h1 class="text-2xl sm:text-3xl md:text-4xl font-extrabold text-zinc-900 dark:text-zinc-100 tracking-tight leading-tight">
          {{ post.title }}
        </h1>

        <!-- 作者与互动 -->
        <div class="flex items-center gap-3 pt-1">
          <img
            :src="getAuthorAvatar('作者')"
            alt="作者头像"
            class="w-10 h-10 rounded-full object-cover border border-zinc-200 dark:border-zinc-700"
          />
          <div>
            <div class="text-sm font-bold text-zinc-800 dark:text-zinc-200">黑塔空间站主管</div>
            <div class="text-xs text-zinc-400">核心架构师 &bull; HertaBase Core Team</div>
          </div>
        </div>
      </header>

      <!-- 封面全景大图 -->
      <div v-if="post.cover_image" class="w-full h-64 sm:h-96 rounded-3xl overflow-hidden shadow-xl bg-zinc-900 border border-zinc-200 dark:border-zinc-800">
        <img :src="post.cover_image" :alt="post.title" class="w-full h-full object-cover" />
      </div>

      <!-- 正文双栏布局 (正文 + 目录) -->
      <div class="grid grid-cols-1 lg:grid-cols-12 gap-8 items-start">
        <!-- Markdown 正文 -->
        <div class="lg:col-span-8 space-y-8">
          <div class="card-base p-6 sm:p-10">
            <div
              class="markdown-body max-w-none"
              v-html="renderedContent"
            ></div>

            <!-- 底部文章标签 -->
            <div v-if="post.tags && post.tags.length" class="pt-8 mt-8 border-t border-zinc-100 dark:border-zinc-800 flex flex-wrap items-center gap-2">
              <span class="text-xs font-semibold text-zinc-400">标签:</span>
              <router-link
                v-for="tag in post.tags"
                :key="tag"
                :to="`/tag/${tag}`"
                class="badge-base bg-zinc-100 hover:bg-emerald-50 hover:text-emerald-600 dark:bg-zinc-800 dark:hover:bg-emerald-950/60 dark:hover:text-emerald-400 text-zinc-600 dark:text-zinc-300 transition"
              >
                #{{ tag }}
              </router-link>
            </div>
          </div>

          <!-- 分享与点赞栏 -->
          <ShareBar
            :post-id="post.id"
            :likes="post.likes"
            :comment-count="blogStore.getCommentsByPostId(post.id).length"
          />

          <!-- 作者简介卡片 -->
          <AuthorBio />

          <!-- 相关推荐 -->
          <RelatedPosts
            :current-post-id="post.id"
            :category="post.category"
            :tags="post.tags"
          />

          <!-- 评论区 -->
          <CommentList :post-id="post.id" />
        </div>

        <!-- 右侧文章大纲目录 (TOC) -->
        <div class="hidden lg:block lg:col-span-4">
          <TableOfContents :items="toc" />
        </div>
      </div>
    </article>
  </div>
</template>
