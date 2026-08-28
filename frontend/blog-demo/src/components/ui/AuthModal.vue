<script setup lang="ts">
import { ref } from 'vue'
import { useThemeStore } from '../../stores/theme'
import { useAuthStore } from '../../stores/auth'
import Modal from './Modal.vue'
import { Lock, Mail, User, ShieldCheck, Sparkles } from 'lucide-vue-next'

const themeStore = useThemeStore()
const authStore = useAuthStore()

const email = ref('')
const password = ref('')
const displayName = ref('')

const handleLogin = async () => {
  if (!email.value || !password.value) {
    themeStore.addToast({ type: 'warning', message: '请填写完整的邮箱与密码' })
    return
  }
  const ok = await authStore.login(email.value, password.value)
  if (ok) {
    themeStore.closeAuth()
    email.value = ''
    password.value = ''
  }
}

const handleRegister = async () => {
  if (!email.value || !password.value || !displayName.value) {
    themeStore.addToast({ type: 'warning', message: '请填写昵称、邮箱与密码' })
    return
  }
  const ok = await authStore.register(email.value, password.value, displayName.value)
  if (ok) {
    themeStore.closeAuth()
    email.value = ''
    password.value = ''
    displayName.value = ''
  }
}

const fillDemoAdmin = () => {
  email.value = 'admin@herta.ai'
  password.value = '123456'
  displayName.value = '黑塔空间站主管'
}
</script>

<template>
  <Modal
    v-model="themeStore.authModalOpen"
    :title="themeStore.authMode === 'login' ? '登录 HertaBlog' : '加入 HertaBlog 创作者社区'"
    max-width="max-w-md"
  >
    <div class="space-y-4">
      <!-- 切换选项卡 -->
      <div class="flex p-1 rounded-xl bg-zinc-100 dark:bg-zinc-800/80">
        <button
          @click="themeStore.authMode = 'login'"
          class="flex-1 py-1.5 text-xs font-semibold rounded-lg transition"
          :class="themeStore.authMode === 'login' ? 'bg-white dark:bg-zinc-700 text-emerald-600 dark:text-emerald-400 shadow-sm' : 'text-zinc-500 hover:text-zinc-800 dark:hover:text-zinc-200'"
        >
          账号登录
        </button>
        <button
          @click="themeStore.authMode = 'register'"
          class="flex-1 py-1.5 text-xs font-semibold rounded-lg transition"
          :class="themeStore.authMode === 'register' ? 'bg-white dark:bg-zinc-700 text-emerald-600 dark:text-emerald-400 shadow-sm' : 'text-zinc-500 hover:text-zinc-800 dark:hover:text-zinc-200'"
        >
          注册新账号
        </button>
      </div>

      <!-- 快速填入演示账号 -->
      <div class="p-3 rounded-xl bg-emerald-50/70 dark:bg-emerald-950/30 border border-emerald-200/60 dark:border-emerald-800/60 flex items-center justify-between text-xs text-emerald-800 dark:text-emerald-300">
        <div class="flex items-center gap-2">
          <ShieldCheck class="w-4 h-4 text-emerald-500" />
          <span>内置演示管理员账号</span>
        </div>
        <button
          @click="fillDemoAdmin"
          type="button"
          class="px-2 py-1 rounded bg-emerald-500 hover:bg-emerald-600 text-white font-medium transition cursor-pointer"
        >
          一键填入
        </button>
      </div>

      <!-- 表单字段 -->
      <form @submit.prevent="themeStore.authMode === 'login' ? handleLogin() : handleRegister()" class="space-y-3.5 pt-1">
        <div v-if="themeStore.authMode === 'register'" class="space-y-1">
          <label class="text-xs font-semibold text-zinc-700 dark:text-zinc-300">用户昵称</label>
          <div class="relative">
            <User class="w-4 h-4 text-zinc-400 absolute left-3.5 top-3" />
            <input
              v-model="displayName"
              type="text"
              placeholder="请输入您的昵称"
              class="input-base pl-10 text-sm"
              required
            />
          </div>
        </div>

        <div class="space-y-1">
          <label class="text-xs font-semibold text-zinc-700 dark:text-zinc-300">电子邮箱</label>
          <div class="relative">
            <Mail class="w-4 h-4 text-zinc-400 absolute left-3.5 top-3" />
            <input
              v-model="email"
              type="email"
              placeholder="user@example.com"
              class="input-base pl-10 text-sm"
              required
            />
          </div>
        </div>

        <div class="space-y-1">
          <label class="text-xs font-semibold text-zinc-700 dark:text-zinc-300">账户密码</label>
          <div class="relative">
            <Lock class="w-4 h-4 text-zinc-400 absolute left-3.5 top-3" />
            <input
              v-model="password"
              type="password"
              placeholder="••••••••"
              class="input-base pl-10 text-sm"
              required
            />
          </div>
        </div>

        <button
          type="submit"
          :disabled="authStore.loading"
          class="w-full btn-primary py-2.5 text-sm mt-2"
        >
          <Sparkles v-if="!authStore.loading" class="w-4 h-4" />
          {{ authStore.loading ? '正在处理中...' : (themeStore.authMode === 'login' ? '立即登录' : '创建并加入') }}
        </button>
      </form>
    </div>
  </Modal>
</template>
