<script setup lang="ts">
import type { BlogComment } from '../../types/blog'
import { useAuthStore } from '../../stores/auth'
import { useBlogStore } from '../../stores/blog'
import { Trash2, CornerDownRight } from 'lucide-vue-next'
import { formatTimeAgo, getAuthorAvatar } from '../../lib/utils'

const props = defineProps<{
  comment: BlogComment
}>()

const authStore = useAuthStore()
const blogStore = useBlogStore()

const canDelete = () => {
  if (authStore.isAdmin) return true
  if (authStore.user?.id && authStore.user.id === props.comment.author) return true
  return false
}

const handleDelete = () => {
  if (confirm('确定要删除这条评论吗？')) {
    blogStore.deleteComment(props.comment.id)
  }
}
</script>

<template>
  <div class="flex items-start gap-4 p-4 rounded-2xl bg-zinc-50/60 dark:bg-zinc-800/40 border border-zinc-100 dark:border-zinc-800/80 transition">
    <img
      :src="getAuthorAvatar(comment.author_name || '读者', comment.author_avatar)"
      :alt="comment.author_name || '读者'"
      class="w-10 h-10 rounded-full object-cover border border-zinc-200 dark:border-zinc-700 shrink-0"
    />

    <div class="flex-1 space-y-1.5 min-w-0">
      <div class="flex items-center justify-between gap-2">
        <div class="flex items-center gap-2">
          <span class="text-xs font-bold text-zinc-900 dark:text-zinc-100">
            {{ comment.author_name || '匿名读者' }}
          </span>
          <span v-if="comment.author?.includes('admin')" class="badge-base bg-emerald-100 text-emerald-700 dark:bg-emerald-950 dark:text-emerald-300 text-[10px]">
            站长
          </span>
        </div>
        <span class="text-[11px] text-zinc-400">
          {{ formatTimeAgo(comment.created_at) }}
        </span>
      </div>

      <p class="text-xs sm:text-sm text-zinc-700 dark:text-zinc-300 leading-relaxed whitespace-pre-wrap">
        {{ comment.content }}
      </p>

      <div v-if="canDelete()" class="pt-1 flex justify-end">
        <button
          @click="handleDelete"
          class="inline-flex items-center gap-1 text-[11px] text-rose-500 hover:text-rose-600 transition"
        >
          <Trash2 class="w-3 h-3" />
          <span>删除</span>
        </button>
      </div>
    </div>
  </div>
</template>
