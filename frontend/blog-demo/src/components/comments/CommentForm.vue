<script setup lang="ts">
import { ref } from 'vue'
import { useAuthStore } from '../../stores/auth'
import { useBlogStore } from '../../stores/blog'
import { useThemeStore } from '../../stores/theme'
import { Send, User, Mail, Sparkles } from 'lucide-vue-next'

const props = defineProps<{
  postId: string
}>()

const authStore = useAuthStore()
const blogStore = useBlogStore()
const themeStore = useThemeStore()

const content = ref('')
const guestName = ref('')
const guestEmail = ref('')
const submitting = ref(false)

const handleSubmit = async () => {
  if (!content.value.trim()) {
    themeStore.addToast({ type: 'warning', message: '请输入评论内容' })
    return
  }

  if (!authStore.isAuthenticated && !guestName.value.trim()) {
    themeStore.addToast({ type: 'warning', message: '请填写您的昵称或登录账号' })
    return
  }

  submitting.value = true
  try {
    await blogStore.addComment({
      post: props.postId,
      content: content.value.trim(),
      author_name: authStore.user?.displayName || guestName.value.trim(),
      author_email: authStore.user?.email || guestEmail.value.trim(),
    })
    content.value = ''
  } finally {
    submitting.value = false
  }
}
</script>

<template>
  <form @submit.prevent="handleSubmit" class="card-base p-5 space-y-4">
    <!-- 用户登录状态条 -->
    <div class="flex items-center justify-between text-xs pb-2 border-b border-zinc-100 dark:border-zinc-800">
      <div v-if="authStore.isAuthenticated" class="flex items-center gap-2">
        <span class="text-zinc-500">当前身份:</span>
        <span class="font-bold text-emerald-600 dark:text-emerald-400">{{ authStore.user?.displayName }}</span>
      </div>
      <div v-else class="flex items-center gap-2 text-zinc-500">
        <span>您正在以游客身份留言</span>
        <span>&bull;</span>
        <button
          type="button"
          @click="themeStore.openAuth('login')"
          class="text-emerald-600 dark:text-emerald-400 font-semibold hover:underline"
        >
          登录账号
        </button>
      </div>
    </div>

    <!-- 游客信息填写框 -->
    <div v-if="!authStore.isAuthenticated" class="grid grid-cols-1 sm:grid-cols-2 gap-3">
      <div class="relative">
        <User class="w-4 h-4 text-zinc-400 absolute left-3.5 top-3" />
        <input
          v-model="guestName"
          type="text"
          placeholder="您的昵称 (必填)"
          class="input-base pl-10 text-xs"
          required
        />
      </div>
      <div class="relative">
        <Mail class="w-4 h-4 text-zinc-400 absolute left-3.5 top-3" />
        <input
          v-model="guestEmail"
          type="email"
          placeholder="电子邮箱 (选填，仅用于头像识别)"
          class="input-base pl-10 text-xs"
        />
      </div>
    </div>

    <!-- 评论输入框 -->
    <div class="relative">
      <textarea
        v-model="content"
        rows="3"
        placeholder="写下您的思考与见解，欢迎友善交流..."
        class="input-base text-sm resize-none"
        required
      ></textarea>
    </div>

    <!-- 提交操作 -->
    <div class="flex items-center justify-between pt-1">
      <span class="text-xs text-zinc-400">支持换行与纯文本讨论</span>
      <button
        type="submit"
        :disabled="submitting"
        class="btn-primary text-xs px-4 py-2"
      >
        <Send class="w-3.5 h-3.5" />
        <span>{{ submitting ? '正在发送...' : '发表评论' }}</span>
      </button>
    </div>
  </form>
</template>
