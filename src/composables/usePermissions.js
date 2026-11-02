import { computed } from 'vue'
import { useAuthStore } from '@/stores/auth'
import { permissions } from '@/constants/permissions'

export function usePermissions() {
  const auth = useAuthStore()
  const can = (ability) => permissions[auth.role]?.includes(ability) ?? false
  const isAdmin = computed(() => auth.role === 'admin')
  return { can, isAdmin, role: computed(() => auth.role) }
}

