import { onBeforeUnmount } from 'vue'

export function useDebounce(delay = 350) {
  let timer
  onBeforeUnmount(() => clearTimeout(timer))
  return (callback) => {
    clearTimeout(timer)
    timer = setTimeout(callback, delay)
  }
}

