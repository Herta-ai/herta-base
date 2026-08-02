<script setup lang="ts">
import { computed, reactive } from 'vue'
import { definePage } from 'vue-router/experimental'
import { useNaiveForm, useStore } from '@/hooks'
import { $t } from '@/locales'
import type { FormRules } from 'naive-ui'

definePage({
  name: 'login',
  meta: {
    i18nKey: 'route.login',
    title: 'Login',
  },
})

const { changeLocale, localeOptions } = useStore('app')
const { login, loginLoading } = useStore('auth')
const { toggleThemeScheme, themeScheme } = useStore('theme')
const { setFormRef, validate } = useNaiveForm()

const model = reactive({
  username: '',
  password: '',
})

const rules = computed<FormRules>(() => ({
  username: [
    {
      required: true,
      message: $t('page.login.form.password.required'),
      trigger: 'input',
    },
    {
      min: 6,
      message: $t('page.login.form.username.invalid'),
      trigger: 'input',
    },
  ],
  password: [
    {
      required: true,
      message: $t('page.login.form.password.required'),
      trigger: 'input',
    },
    {
      min: 6,
      message: $t('page.login.form.password.invalid'),
      trigger: 'input',
    },
  ],
}))

async function handleLogin() {
  try {
    await validate()
    await login(model.username, model.password)
  }
  catch (error) {
    console.error(error)
  }
}

function handleLanguageSelect(key: I18n.LangType) {
  changeLocale(key)
}
</script>

<template>
  <div class="wh-full flex-col-center bg-layout transition-colors duration-300">
    <div class="max-w-md w-full rounded-xl bg-container p-6 shadow-xl transition-colors duration-300">
      <!-- Header -->
      <div class="mb-6 flex-y-center space-y-2">
        <h1 class="text-2xl w-full text-center text-primary font-bold">
          {{ $t('system.title') }}
        </h1>
        <div class="absolute right-4 top-4 flex-center space-x-4">
          <!-- Theme Toggle -->
          <n-button circle quaternary @click="toggleThemeScheme()">
            <template #icon>
              <div v-if="themeScheme === 'dark'" class="i-carbon-moon text-xl" />
              <div v-else-if="themeScheme === 'auto'" class="i-carbon-laptop text-xl" />
              <div v-else class="i-carbon-sun text-xl" />
            </template>
          </n-button>

          <!-- Language Select -->
          <n-dropdown trigger="hover" :options="localeOptions" @select="handleLanguageSelect">
            <n-button circle quaternary>
              <template #icon>
                <div class="i-carbon-language text-xl" />
              </template>
            </n-button>
          </n-dropdown>
        </div>
      </div>

      <!-- Form -->
      <n-form :ref="setFormRef" :model="model" :rules="rules" size="large" :show-label="false">
        <n-form-item path="username">
          <n-input v-model:value="model.username" :placeholder="$t('page.login.form.username.placeholder')">
            <template #prefix>
              <div class="i-carbon-user text-gray-400" />
            </template>
          </n-input>
        </n-form-item>

        <n-form-item path="password">
          <n-input
            v-model:value="model.password"
            type="password"
            show-password-on="click"
            :placeholder="$t('page.login.form.password.placeholder')"
            @keyup.enter="handleLogin"
          >
            <template #prefix>
              <div class="i-carbon-locked text-gray-400" />
            </template>
          </n-input>
        </n-form-item>

        <n-button type="primary" block size="large" :loading="loginLoading" @click="handleLogin">
          {{ $t('page.login.confirm') }}
        </n-button>
      </n-form>
    </div>
  </div>
</template>

<style scoped lang="scss">
:deep(input:autofill) {
  /* 隐藏自动填充后的背景 */
  transition: background-color 50000s ease-in-out 0s;
  -webkit-text-fill-color: var(--n-text-color);
}
</style>
