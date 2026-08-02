<script setup lang="ts">
definePage({
  name: 'not-found',
  meta: {
    i18nKey: 'route.not-found',
    title: 'NotFound',
  },
})
</script>

<template>
  <exception-base type="404" />
</template>
