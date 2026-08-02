<script lang="ts" setup>
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import noPermission from '~icons/local/no-permission'
import notFound from '~icons/local/not-found'
import serviceError from '~icons/local/service-error'
import { $t } from '@/locales'
import type { Component } from 'vue'

defineOptions({ name: 'ExceptionBase' })

const props = defineProps<Props>()

type ExceptionType = '403' | '404' | '500'

interface Props {
  /**
   * Exception type
   *
   * - 403: no permission
   * - 404: not found
   * - 500: service error
   */
  type: ExceptionType
}

const router = useRouter()

const iconMap: Record<ExceptionType, Component> = {
  403: noPermission,
  404: notFound,
  500: serviceError,
}

const icon = computed(() => iconMap[props.type])
</script>

<template>
  <div class="size-full min-h-520px flex-col-center gap-24px overflow-hidden">
    <component :is="icon" class="flex text-400px text-primary" />
    <NButton type="primary" @click="router.push('/home')">
      {{ $t('common.backToHome') }}
    </NButton>
  </div>
</template>

<style scoped></style>
