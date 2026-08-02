import { ref } from 'vue'
import type { ComponentPublicInstance } from 'vue'
import type { FormInst } from 'naive-ui'

export function useNaiveForm() {
  const formRef = ref<FormInst | null>(null)

  function setFormRef(ref: Element | ComponentPublicInstance | null) {
    formRef.value = ref as FormInst | null
  }
  async function validate() {
    await formRef.value?.validate()
  }

  async function restoreValidation() {
    formRef.value?.restoreValidation()
  }

  return {
    setFormRef,
    validate,
    restoreValidation,
  }
}
