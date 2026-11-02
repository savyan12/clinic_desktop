import { onBeforeUnmount, ref, watch } from 'vue'

export function useDialogOptions(isOpen, fetchOptions) {
  const data = ref(null)
  const loading = ref(false)
  const error = ref('')
  let version = 0

  async function load() {
    if (!isOpen()) return
    const current = ++version
    loading.value = true
    error.value = ''
    try {
      const result = await fetchOptions()
      if (current === version) data.value = result
    } catch (cause) {
      if (current === version) error.value = cause.message || 'تعذر تحميل الخيارات. أعد المحاولة.'
    } finally {
      if (current === version) loading.value = false
    }
  }

  watch(isOpen, (open) => {
    version += 1
    loading.value = false
    if (open) load()
  }, { immediate: true })
  onBeforeUnmount(() => { version += 1 })

  return { data, loading, error, load }
}
