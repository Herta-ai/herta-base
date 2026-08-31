<script setup lang="ts">
import { useThemeStore } from '../../stores/theme'
import { CheckCircle2, AlertCircle, Info, AlertTriangle, X } from 'lucide-vue-next'

const themeStore = useThemeStore()
</script>

<template>
  <Teleport to="body">
    <div class="fixed bottom-5 right-5 z-[9999] flex flex-col gap-2.5 max-w-sm w-full pointer-events-none">
      <TransitionGroup
        enter-active-class="transform transition duration-300 ease-out"
        enter-from-class="translate-y-4 opacity-0 scale-95"
        enter-to-class="translate-y-0 opacity-100 scale-100"
        leave-active-class="transform transition duration-200 ease-in"
        leave-from-class="translate-y-0 opacity-100 scale-100"
        leave-to-class="translate-y-2 opacity-0 scale-95"
      >
        <div
          v-for="toast in themeStore.toasts"
          :key="toast.id"
          class="pointer-events-auto flex items-start gap-3 p-4 rounded-2xl shadow-2xl border backdrop-blur-md transition-all"
          :class="[
            toast.type === 'success' ? 'bg-emerald-50/95 dark:bg-emerald-950/90 border-emerald-200 dark:border-emerald-800 text-emerald-900 dark:text-emerald-100' :
            toast.type === 'error' ? 'bg-rose-50/95 dark:bg-rose-950/90 border-rose-200 dark:border-rose-800 text-rose-900 dark:text-rose-100' :
            toast.type === 'warning' ? 'bg-amber-50/95 dark:bg-amber-950/90 border-amber-200 dark:border-amber-800 text-amber-900 dark:text-amber-100' :
            'bg-zinc-50/95 dark:bg-zinc-900/90 border-zinc-200 dark:border-zinc-800 text-zinc-900 dark:text-zinc-100'
          ]"
        >
          <CheckCircle2 v-if="toast.type === 'success'" class="w-5 h-5 text-emerald-500 shrink-0 mt-0.5" />
          <AlertCircle v-else-if="toast.type === 'error'" class="w-5 h-5 text-rose-500 shrink-0 mt-0.5" />
          <AlertTriangle v-else-if="toast.type === 'warning'" class="w-5 h-5 text-amber-500 shrink-0 mt-0.5" />
          <Info v-else class="w-5 h-5 text-blue-500 shrink-0 mt-0.5" />

          <div class="flex-1 text-sm font-medium leading-snug">
            {{ toast.message }}
          </div>

          <button
            @click="themeStore.removeToast(toast.id)"
            class="text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-200 transition"
          >
            <X class="w-4 h-4" />
          </button>
        </div>
      </TransitionGroup>
    </div>
  </Teleport>
</template>
