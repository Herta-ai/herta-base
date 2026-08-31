<script setup lang="ts">
import { useThemeStore } from '../../stores/theme'
import { Sparkles, ArrowUp, Database, Github, Heart } from 'lucide-vue-next'

const themeStore = useThemeStore()

const scrollToTop = () => {
  window.scrollTo({ top: 0, behavior: 'smooth' })
}
</script>

<template>
  <footer class="mt-20 border-t border-zinc-200 dark:border-zinc-800 bg-white/50 dark:bg-zinc-950/50 backdrop-blur-xs transition-colors">
    <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-12">
      <div class="grid grid-cols-1 md:grid-cols-4 gap-8 mb-12">
        <!-- 品牌与简介 -->
        <div class="md:col-span-2 space-y-4">
          <div class="flex items-center gap-3">
            <div class="w-8 h-8 rounded-xl bg-gradient-to-tr from-emerald-500 to-teal-600 flex-center text-white shadow-md shadow-emerald-500/20">
              <Sparkles class="w-4 h-4" />
            </div>
            <span class="text-lg font-bold bg-gradient-to-r from-zinc-900 to-emerald-600 dark:from-white dark:to-emerald-400 bg-clip-text text-transparent">
              HertaBlog
            </span>
          </div>
          <p class="text-sm text-zinc-500 dark:text-zinc-400 leading-relaxed max-w-sm">
            基于 <strong>HertaBase</strong> 自托管轻量级后端构建的现代化技术博客空间。探索系统架构、大模型 Agent 协同与现代前端工程的无限可能。
          </p>
          <div class="flex items-center gap-3 pt-1">
            <button
              @click="themeStore.openSetup()"
              class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-zinc-100 dark:bg-zinc-900 hover:bg-emerald-50 hover:text-emerald-600 dark:hover:bg-emerald-950/40 dark:hover:text-emerald-400 text-xs font-medium text-zinc-600 dark:text-zinc-400 transition"
            >
              <Database class="w-3.5 h-3.5" />
              <span>HertaBase 数据底座向导</span>
            </button>
          </div>
        </div>

        <!-- 专栏导航 -->
        <div class="space-y-3">
          <h4 class="text-xs font-bold uppercase tracking-wider text-zinc-900 dark:text-zinc-100">精选专栏</h4>
          <ul class="space-y-2 text-sm text-zinc-500 dark:text-zinc-400">
            <li><router-link to="/category/arch" class="hover:text-emerald-500 transition">架构设计与分布式</router-link></li>
            <li><router-link to="/category/ai" class="hover:text-emerald-500 transition">大模型与 AI Agent</router-link></li>
            <li><router-link to="/category/rust" class="hover:text-emerald-500 transition">Rust 高性能实战</router-link></li>
            <li><router-link to="/category/frontend" class="hover:text-emerald-500 transition">Vue 3.5 &amp; Vite 8 工程化</router-link></li>
          </ul>
        </div>

        <!-- 技术底座 -->
        <div class="space-y-3">
          <h4 class="text-xs font-bold uppercase tracking-wider text-zinc-900 dark:text-zinc-100">技术栈支持</h4>
          <ul class="space-y-2 text-sm text-zinc-500 dark:text-zinc-400">
            <li>HertaBase (Rust + Salvo + RocksDB)</li>
            <li>Vue 3.5 + Vite 8 + TypeScript</li>
            <li>UnoCSS (Wind3 &amp; Typography)</li>
            <li>Pinia + Vue Router 4</li>
          </ul>
        </div>
      </div>

      <!-- 底部版权与回到顶部 -->
      <div class="pt-8 border-t border-zinc-200/80 dark:border-zinc-800/80 flex flex-col sm:flex-row items-center justify-between gap-4 text-xs text-zinc-400">
        <div class="flex items-center gap-1">
          <span>&copy; {{ new Date().getFullYear() }} HertaBlog. Made with</span>
          <Heart class="w-3.5 h-3.5 text-rose-500 fill-rose-500 inline" />
          <span>and powered by HertaBase.</span>
        </div>

        <button
          @click="scrollToTop"
          class="group flex items-center gap-1.5 px-3.5 py-1.5 rounded-full bg-zinc-100 dark:bg-zinc-900 border border-zinc-200/80 dark:border-zinc-800 text-zinc-600 dark:text-zinc-400 hover:text-emerald-600 dark:hover:text-emerald-400 hover:border-emerald-500/30 hover:bg-emerald-50/50 dark:hover:bg-emerald-950/30 transition-all duration-200 cursor-pointer shadow-xs"
        >
          <span class="font-medium">回到顶部</span>
          <ArrowUp class="w-3.5 h-3.5 group-hover:-translate-y-0.5 transition-transform duration-200 text-emerald-500" />
        </button>
      </div>
    </div>
  </footer>
</template>
