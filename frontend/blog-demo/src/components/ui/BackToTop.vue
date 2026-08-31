<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { ArrowUp } from 'lucide-vue-next'

const visible = ref(false)
const scrollProgress = ref(0)

const handleScroll = () => {
  const scrollY = window.scrollY
  visible.value = scrollY > 280

  const docHeight = document.documentElement.scrollHeight - window.innerHeight
  if (docHeight > 0) {
    scrollProgress.value = Math.min(100, Math.round((scrollY / docHeight) * 100))
  }
}

const scrollToTop = () => {
  window.scrollTo({
    top: 0,
    behavior: 'smooth',
  })
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
  <Transition
    enter-active-class="transition duration-300 ease-out"
    enter-from-class="opacity-0 translate-y-4 scale-90"
    enter-to-class="opacity-100 translate-y-0 scale-100"
    leave-active-class="transition duration-200 ease-in"
    leave-from-class="opacity-100 translate-y-0 scale-100"
    leave-to-class="opacity-0 translate-y-4 scale-90"
  >
    <div
      v-if="visible"
      class="fixed bottom-6 right-6 z-40 flex flex-col items-center gap-1.5"
    >
      <button
        @click="scrollToTop"
        class="group relative w-11 h-11 rounded-2xl bg-white/90 dark:bg-zinc-900/90 border border-zinc-200/80 dark:border-zinc-700/80 shadow-lg shadow-zinc-900/5 dark:shadow-black/30 backdrop-blur-md flex-center text-zinc-600 dark:text-zinc-300 hover:text-white hover:bg-emerald-500 dark:hover:bg-emerald-600 hover:border-emerald-500 dark:hover:border-emerald-600 hover:scale-105 active:scale-95 transition-all duration-200 cursor-pointer"
        title="回到顶部"
        aria-label="回到顶部"
      >
        <ArrowUp class="w-5 h-5 group-hover:-translate-y-0.5 transition-transform duration-200" />

        <!-- 悬浮提示 Tooltip -->
        <span class="absolute -top-9 px-2 py-1 rounded-lg text-[10px] font-semibold bg-zinc-900 text-white dark:bg-white dark:text-zinc-900 opacity-0 group-hover:opacity-100 transition-opacity duration-200 pointer-events-none whitespace-nowrap shadow-md">
          回到顶部 {{ scrollProgress > 0 ? `(${scrollProgress}%)` : '' }}
        </span>
      </button>
    </div>
  </Transition>
</template>
