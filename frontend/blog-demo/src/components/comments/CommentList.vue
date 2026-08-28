<script setup lang="ts">
import { computed } from 'vue'
import { useBlogStore } from '../../stores/blog'
import CommentItem from './CommentItem.vue'
import CommentForm from './CommentForm.vue'
import { MessageSquare, MessageCircle } from 'lucide-vue-next'

const props = defineProps<{
  postId: string
}>()

const blogStore = useBlogStore()

const comments = computed(() => {
  return blogStore.getCommentsByPostId(props.postId)
})
</script>

<template>
  <section id="comments-section" class="pt-10 space-y-6">
    <div class="flex items-center gap-2">
      <MessageSquare class="w-5 h-5 text-emerald-500" />
      <h3 class="text-lg font-bold text-zinc-900 dark:text-zinc-100">
        读者互动评论 ({{ comments.length }})
      </h3>
    </div>

    <!-- 发表评论表单 -->
    <CommentForm :post-id="postId" />

    <!-- 评论列表 -->
    <div v-if="comments.length > 0" class="space-y-3 pt-2">
      <CommentItem
        v-for="c in comments"
        :key="c.id"
        :comment="c"
      />
    </div>

    <!-- 无评论空状态 -->
    <div v-else class="card-base p-8 text-center text-zinc-400 space-y-2">
      <MessageCircle class="w-8 h-8 mx-auto opacity-30 text-emerald-500" />
      <p class="text-xs">暂无评论，快来抢占第一条沙发吧！</p>
    </div>
  </section>
</template>
