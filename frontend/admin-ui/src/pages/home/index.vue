<script setup lang="ts">
import { ref } from 'vue'

definePage({
  alias: '/',
  name: 'home',
  meta: {
    i18nKey: 'route.home',
    title: 'Home',
  },
})

const techStack = ref([
  {
    category: '核心框架',
    items: [
      { name: 'Vue', desc: '渐进式 JavaScript 框架', version: '3.6' },
      { name: 'Vite', desc: '极速的前端构建工具', version: '8' },
      { name: 'TypeScript', desc: '静态类型检查', version: '5.9' },
    ],
  },
  {
    category: '路由与状态管理',
    items: [
      { name: 'Vue Router', desc: 'Vue 官方路由', version: '5.0' },
      { name: 'Pinia', desc: '新一代 Vue 状态管理', version: '3.0' },
      { name: 'pinia-plugin-persistedstate', desc: 'Pinia 状态持久化插件', version: '4.7' },
    ],
  },
  {
    category: 'UI 与样式设计',
    items: [
      { name: 'Naive UI', desc: '尤雨溪推荐的 Vue 3 组件库', version: '2.43' },
      { name: 'UnoCSS', desc: '高性能的即时原子化 CSS 引擎', version: '66.6' },
      { name: 'SCSS', desc: '更快速的 Sass 编译支持', version: '1.97' },
    ],
  },
  {
    category: '请求与通信',
    items: [
      { name: 'Alova', desc: '轻量级的下一代请求策略库', version: '3.5' },
    ],
  },
  {
    category: '实用工具库',
    items: [
      { name: '@vueuse/core', desc: '强大的 Vue 组合式 API 工具集合', version: '14.2' },
      { name: 'Vue I18n', desc: '国际化支持', version: '11.2' },
      { name: 'Day.js', desc: '轻量级日期时间处理库', version: '1.11' },
      { name: 'Defu', desc: '配置深度合并工具', version: '6.1' },
      { name: 'Colord', desc: '极致轻量的高性能颜色操作与转换库', version: '2.9' },
      { name: 'Crypto-js', desc: '常用加密算法工具', version: '4.2' },
      { name: 'NProgress', desc: '顶部加载进度条', version: '0.2' },
    ],
  },
  {
    category: '工程化与代码规范',
    items: [
      { name: 'ESLint', desc: '统一的代码风格格式化', version: '10.0' },
      { name: 'Git Hooks', desc: '基于 simple-git-hooks 实现轻量级 Git 钩子', version: '2.13' },
      { name: 'Commitlint', desc: '遵循 Conventional Commits（约定式提交）规范', version: '20.4' },
      { name: '自动导入', desc: '配合 unplugin-vue-components 实现组件和 API 的自动按需引入', version: '31.0' },
    ],
  },
])

const features = ref([
  { title: '开箱即用', desc: '提供完整的前端工程化解决方案，无需繁琐配置' },
  { title: '高规范', desc: '采用严格的代码规范和 Git 工作流，确保代码质量' },
  { title: '强类型', desc: '全面使用 TypeScript，提供完整的类型安全保障' },
  { title: '高性能', desc: '基于最新的 Vite 8 和 Vue 3.6，提供极致的开发体验' },
  { title: '国际化', desc: '内置 Vue I18n 支持，轻松实现多语言切换' },
  { title: '主题定制', desc: '支持主题切换和个性化定制' },
])
</script>

<template>
  <div class="mx-auto max-w-5xl p-6">
    <div class="mb-12 text-center">
      <h1 class="text-5xl mb-4 from-green-500 to-green-400 bg-gradient-to-r bg-clip-text text-transparent font-bold">
        🚀 Vue3-ESS-Starter
      </h1>
      <p class="text-2xl mb-4 text-gray-500">
        现代化的 Vue 3 核心启动模板
      </p>
      <p class="mx-auto max-w-2xl text-base text-gray-600 leading-relaxed">
        基于 Vite 8 和 Vue 3.6 构建，集成了当前前端生态中最优秀的工具链与依赖，
        旨在为中后台系统或企业级 Web 应用提供一个开箱即用、高规范、强类型的前端工程化底座。
      </p>
    </div>

    <div class="mb-12">
      <h2 class="text-3xl mb-6 border-b-2 border-gray-100 pb-2 font-bold">
        ✨ 项目特点
      </h2>
      <n-grid :cols="3" :x-gap="16" :y-gap="16">
        <n-grid-item v-for="(feature, index) in features" :key="index">
          <n-card hoverable>
            <h3>{{ feature.title }}</h3>
            <p>{{ feature.desc }}</p>
          </n-card>
        </n-grid-item>
      </n-grid>
    </div>

    <div class="mb-12">
      <h2 class="text-3xl mb-6 border-b-2 border-gray-100 pb-2 font-bold">
        🔧 技术栈
      </h2>
      <n-tabs type="line" animated>
        <n-tab-pane v-for="(category, index) in techStack" :key="index" :name="category.category" :tab="category.category">
          <n-grid :cols="2" :x-gap="16" :y-gap="16">
            <n-grid-item v-for="(item, idx) in category.items" :key="idx">
              <n-card hoverable>
                <div class="mb-2">
                  <h4 class="mb-2">
                    {{ item.name }}
                  </h4>
                  <p class="mb-3 text-gray-500">
                    {{ item.desc }}
                  </p>
                  <n-tag size="small" type="info">
                    v{{ item.version }}
                  </n-tag>
                </div>
              </n-card>
            </n-grid-item>
          </n-grid>
        </n-tab-pane>
      </n-tabs>
    </div>

    <div class="mb-12">
      <h2 class="text-3xl mb-6 border-b-2 border-gray-100 pb-2 font-bold">
        📦 快速开始
      </h2>
      <n-card>
        <h3 class="mb-2 mt-6 first:mt-0">
          1. 环境准备
        </h3>
        <p>推荐 Bun 版本 >= 1.3</p>

        <h3 class="mb-2 mt-6">
          2. 安装依赖
        </h3>
        <n-code language="bash" code="bun install" />

        <h3 class="mb-2 mt-6">
          3. 初始化 Git Hooks
        </h3>
        <n-code language="bash" code="bun prepare" />

        <h3 class="mb-2 mt-6">
          4. 启动开发服务器
        </h3>
        <n-code language="bash" code="bun dev" />
      </n-card>
    </div>

    <div class="mb-12">
      <h2 class="text-3xl mb-6 border-b-2 border-gray-100 pb-2 font-bold">
        📜 脚本命令
      </h2>
      <n-data-table
        :columns="[
          { title: '命令', key: 'command' },
          { title: '描述', key: 'description' },
        ]"
        :data="[
          { command: 'bun dev', description: '启动 Vite 开发服务器' },
          { command: 'bun build', description: '生产环境构建（包含 vue-tsc 严格类型检查）' },
          { command: 'bun preview', description: '在本地预览生产环境的构建产物' },
          { command: 'bun typecheck', description: '仅执行 TypeScript 类型检查，不生成编译文件' },
          { command: 'bun lint', description: '运行 ESLint 检查并自动修复代码格式问题' },
          { command: 'bun prepare', description: '安装 Git 钩子配置（通常无需手动运行）' },
        ]"
        :pagination="false"
      />
    </div>
  </div>
</template>
