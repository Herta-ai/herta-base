<script setup lang="ts">
import { useRoute } from 'vue-router'
import { useStore } from '@/hooks'
import { $t } from '@/locales'

const route = useRoute()
const { changeLocale, localeOptions } = useStore('app')
const { resetStore } = useStore('auth')
const { toggleThemeScheme, themeScheme } = useStore('theme')

function handleLanguageSelect(key: I18n.LangType) {
  changeLocale(key)
}

function handleLogout() {
  resetStore()
}
</script>

<template>
  <div v-if="route.name !== 'login'" class="wh-full flex flex-col from-layout to-layout/90 bg-gradient-to-br transition-all duration-500">
    <!-- Header -->
    <header class="h-14 flex justify-between bg-container/95 px-6 shadow-lg backdrop-blur-md transition-all duration-300">
      <div class="flex items-center space-x-3">
        <div class="h-8 w-8 flex items-center justify-center rounded-lg from-primary to-primary/70 bg-gradient-to-br shadow-md">
          <div class="i-carbon-application text-white" />
        </div>
        <h1 class="text-2xl from-primary to-primary/70 bg-gradient-to-r bg-clip-text text-transparent font-bold">
          {{ $t('system.title') }}
        </h1>
      </div>

      <div class="flex-center space-x-2">
        <!-- Theme Toggle -->
        <n-button circle quaternary class="transition-all duration-300 hover:bg-primary/10" @click="toggleThemeScheme()">
          <template #icon>
            <div v-if="themeScheme === 'dark'" class="i-carbon-moon text-primary" />
            <div v-else-if="themeScheme === 'auto'" class="i-carbon-laptop text-primary" />
            <div v-else class="i-carbon-sun text-primary" />
          </template>
        </n-button>

        <!-- Language Select -->
        <n-dropdown trigger="hover" :options="localeOptions" @select="handleLanguageSelect">
          <n-button circle quaternary class="transition-all duration-300 hover:bg-primary/10">
            <template #icon>
              <div class="i-carbon-language text-primary" />
            </template>
          </n-button>
        </n-dropdown>

        <!-- Logout -->
        <n-button circle quaternary class="transition-all duration-300 hover:bg-red-500/10" @click="handleLogout">
          <template #icon>
            <div class="i-carbon-logout text-red-500" />
          </template>
        </n-button>
      </div>
    </header>

    <!-- Content -->
    <main class="flex-1 overflow-auto p-6">
      <div class="mx-auto max-w-7xl">
        <div class="min-h-[calc(100vh-10rem)] rounded-2xl bg-container p-6 shadow-xl backdrop-blur-sm transition-all duration-300 hover:shadow-2xl">
          <router-view />
        </div>
      </div>
    </main>
  </div>
  <template v-else>
    <router-view />
  </template>
</template>

<style scoped></style>
