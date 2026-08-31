<script setup lang="ts">
import { Sparkles, Server, Cpu, Layers, Code, Heart, Mail, Github, Globe } from 'lucide-vue-next'
import { getAuthorAvatar } from '../lib/utils'
</script>

<template>
  <div class="max-w-4xl mx-auto space-y-10 animate-fade-in">
    <!-- 关于页面 Banner -->
    <div class="card-base p-8 sm:p-12 bg-gradient-to-br from-emerald-500/10 via-teal-500/10 to-blue-500/10 dark:from-emerald-950/30 dark:via-zinc-900 dark:to-zinc-900 border-emerald-200/50 dark:border-emerald-900/50 space-y-4">
      <div class="inline-flex items-center gap-2 px-3 py-1 rounded-full text-xs font-bold bg-emerald-500 text-white shadow-sm">
        <Sparkles class="w-3.5 h-3.5" />
        关于本站
      </div>

      <h1 class="text-3xl sm:text-4xl font-extrabold text-zinc-900 dark:text-zinc-100 tracking-tight">
        HertaBlog - 记录思考与技术演进
      </h1>

      <p class="text-sm sm:text-base text-zinc-600 dark:text-zinc-400 leading-relaxed max-w-2xl">
        这是一个基于 <strong>HertaBase</strong> 自托管轻量级后端构建的现代化中文技术博客。我们专注于系统架构、Rust 高性能实战、大模型 Agent 协同工作流与现代前端工程化。
      </p>
    </div>

    <!-- 站长简介卡片 -->
    <div class="card-base p-8 flex flex-col sm:flex-row items-center sm:items-start gap-6">
      <img
        :src="getAuthorAvatar('黑塔空间站主管')"
        alt="站长头像"
        class="w-24 h-24 rounded-3xl object-cover shadow-lg border-2 border-white dark:border-zinc-800 shrink-0"
      />

      <div class="space-y-3 text-center sm:text-left">
        <div>
          <h2 class="text-xl font-bold text-zinc-900 dark:text-zinc-100">黑塔空间站主管</h2>
          <p class="text-xs font-semibold text-emerald-600 dark:text-emerald-400">核心架构师 &bull; 开源贡献者</p>
        </div>

        <p class="text-xs sm:text-sm text-zinc-600 dark:text-zinc-400 leading-relaxed">
          十余年分布式系统与全栈研发经验，热衷于构建具备极致性能与优雅开发者体验的基础设施软件。通过写作记录每一次架构探索与工程洞见。
        </p>

        <div class="flex items-center justify-center sm:justify-start gap-4 text-xs text-zinc-500 pt-1">
          <a href="https://herta.ai" target="_blank" class="flex items-center gap-1 hover:text-emerald-500 transition">
            <Globe class="w-4 h-4" /> 官方站点
          </a>
          <a href="mailto:admin@herta.ai" class="flex items-center gap-1 hover:text-emerald-500 transition">
            <Mail class="w-4 h-4" /> 邮件交流
          </a>
        </div>
      </div>
    </div>

    <!-- 技术栈全景卡片 -->
    <div class="card-base p-8 space-y-6">
      <div class="flex items-center gap-2 text-zinc-900 dark:text-zinc-100 font-bold text-lg">
        <Server class="w-5 h-5 text-emerald-500" />
        <h3>驱动本博客的技术底座</h3>
      </div>

      <div class="grid grid-cols-1 sm:grid-cols-2 gap-4 text-xs">
        <div class="p-4 rounded-2xl bg-zinc-50 dark:bg-zinc-800/50 border border-zinc-200/80 dark:border-zinc-800 space-y-1.5">
          <div class="font-bold text-sm text-zinc-900 dark:text-zinc-100 flex items-center gap-1.5">
            <Cpu class="w-4 h-4 text-orange-500" />
            HertaBase (Rust 后端)
          </div>
          <p class="text-zinc-500 dark:text-zinc-400 leading-relaxed">
            单二进制高性能 BaaS 引擎，原生提供数据集合 CRUD、API Rules 行级权限控制与前端静态托管。
          </p>
        </div>

        <div class="p-4 rounded-2xl bg-zinc-50 dark:bg-zinc-800/50 border border-zinc-200/80 dark:border-zinc-800 space-y-1.5">
          <div class="font-bold text-sm text-zinc-900 dark:text-zinc-100 flex items-center gap-1.5">
            <Layers class="w-4 h-4 text-emerald-500" />
            Vue 3.5 &amp; Vite 8
          </div>
          <p class="text-zinc-500 dark:text-zinc-400 leading-relaxed">
            响应式系统全面重构，配合 Vite 8 极致毫秒级热更新与高效打包。
          </p>
        </div>

        <div class="p-4 rounded-2xl bg-zinc-50 dark:bg-zinc-800/50 border border-zinc-200/80 dark:border-zinc-800 space-y-1.5">
          <div class="font-bold text-sm text-zinc-900 dark:text-zinc-100 flex items-center gap-1.5">
            <Code class="w-4 h-4 text-blue-500" />
            UnoCSS &amp; Typography
          </div>
          <p class="text-zinc-500 dark:text-zinc-400 leading-relaxed">
            原子化 CSS 引擎，内置暗黑模式切换、中文优雅排版与代码高亮。
          </p>
        </div>

        <div class="p-4 rounded-2xl bg-zinc-50 dark:bg-zinc-800/50 border border-zinc-200/80 dark:border-zinc-800 space-y-1.5">
          <div class="font-bold text-sm text-zinc-900 dark:text-zinc-100 flex items-center gap-1.5">
            <Heart class="w-4 h-4 text-rose-500" />
            @hb/sdk 官方套件
          </div>
          <p class="text-zinc-500 dark:text-zinc-400 leading-relaxed">
            基于标准 Fetch 的官方 TypeScript SDK，无运行时依赖，类型安全。
          </p>
        </div>
      </div>
    </div>
  </div>
</template>
