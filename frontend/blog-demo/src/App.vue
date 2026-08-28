<script setup lang="ts">
import Navbar from './components/layout/Navbar.vue'
import Footer from './components/layout/Footer.vue'
import SearchModal from './components/ui/SearchModal.vue'
import SetupModal from './components/ui/SetupModal.vue'
import AuthModal from './components/ui/AuthModal.vue'
import Toast from './components/ui/Toast.vue'
</script>

<template>
  <div class="min-h-screen flex flex-col bg-zinc-50 dark:bg-zinc-950 text-zinc-800 dark:text-zinc-200 transition-colors">
    <!-- 顶部导航栏 -->
    <Navbar />

    <!-- 路由主体内容 -->
    <main class="flex-1 max-w-7xl w-full mx-auto px-4 sm:px-6 lg:px-8 pt-8 pb-16">
      <router-view />
    </main>

    <!-- 底部页脚 -->
    <Footer />

    <!-- 全局弹窗与消息提示 -->
    <SearchModal />
    <SetupModal />
    <AuthModal />
    <Toast />
  </div>
</template>
