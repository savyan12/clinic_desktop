import { nextTick, onBeforeUnmount, onMounted, watch } from 'vue'
import { useRoute } from 'vue-router'

const fieldSelector = 'input:not([type="hidden"]), select, textarea, [contenteditable="true"]'
const surfaceSelector = '[role="dialog"], .v-dialog, .v-navigation-drawer'

function isVisible(element) {
  if (!element?.isConnected || element.closest('[hidden], [aria-hidden="true"]')) return false

  for (let current = element; current && current !== document.documentElement; current = current.parentElement) {
    const style = window.getComputedStyle(current)
    if (style.display === 'none' || style.visibility === 'hidden') return false
    if (current.classList.contains('v-overlay') && !current.classList.contains('v-overlay--active')) return false
  }

  return element.getClientRects().length > 0
}

function isInteractiveVuetifySelect(element) {
  const input = element.closest('.v-input')
  if (!input || input.classList.contains('v-input--readonly')) return false
  return input.classList.contains('v-select') || input.classList.contains('v-autocomplete') || input.classList.contains('v-combobox')
}

export function isFormFlowField(element) {
  if (!(element instanceof HTMLElement) || !element.matches(fieldSelector)) return false
  if (!isVisible(element) || element.matches('[disabled], [aria-disabled="true"], [data-form-flow-ignore]')) return false
  if (element.closest('.v-input--disabled, [data-form-flow-ignore]')) return false
  if ((element.matches('[readonly]') || element.readOnly) && !isInteractiveVuetifySelect(element)) return false
  if (element.closest('.v-input--readonly')) return false
  return true
}

export function getFormFlowFields(scope) {
  if (!scope) return []
  return Array.from(scope.querySelectorAll(fieldSelector)).filter(isFormFlowField)
}

function hasActiveField(scope) {
  return scope?.contains(document.activeElement) && isFormFlowField(document.activeElement)
}

function isAutofocusField(field) {
  return field.getAttribute('type') !== 'search' && !field.closest('[data-form-flow-filter]')
}

export function focusFirstFormFlowField(scope) {
  if (!scope?.isConnected) return false
  if (hasActiveField(scope)) return true
  const field = getFormFlowFields(scope).find(isAutofocusField)
  if (!field) return false
  field.focus({ preventScroll: true })
  field.scrollIntoView({ block: 'nearest', inline: 'nearest' })
  return document.activeElement === field
}

function isAvailableAction(element) {
  return element instanceof HTMLElement && isVisible(element) && !element.matches(':disabled, [aria-disabled="true"]')
}

function findPrimaryAction(scope, field) {
  const explicit = Array.from(scope.querySelectorAll('[data-form-flow-primary]')).find(isAvailableAction)
  if (explicit) return explicit

  const form = field.form || field.closest('form') || (scope.matches?.('form') ? scope : null)
  const submit = form ? Array.from(form.querySelectorAll('button[type="submit"], input[type="submit"]')).find(isAvailableAction) : null
  if (submit) return submit

  const fallback = Array.from(scope.querySelectorAll('.v-card-actions button, .dialog-actions button')).filter(isAvailableAction)
  return fallback.at(-1) || null
}

function submitScope(scope, field) {
  const action = findPrimaryAction(scope, field)
  if (action) {
    action.click()
    return
  }

  const form = field.form || field.closest('form') || (scope.matches?.('form') ? scope : null)
  form?.requestSubmit()
}

function fieldScope(field) {
  return field.closest('form, [role="dialog"], .v-dialog, .v-navigation-drawer, [data-form-flow-scope]')
    || document.querySelector('[data-form-flow-page]')
    || document.querySelector('.v-main')
    || document.body
}

function isExpandedCombobox(field) {
  const combobox = field.closest('[role="combobox"]')
  return field.getAttribute('aria-expanded') === 'true' || combobox?.getAttribute('aria-expanded') === 'true'
}

function handleFilterEnter(event, field) {
  const filter = field.closest('[data-form-flow-filter]')
  if (!filter && field.getAttribute('type') !== 'search') return false

  event.preventDefault()
  const action = filter ? Array.from(filter.querySelectorAll('[data-form-flow-filter-action]')).find(isAvailableAction) : null
  if (action) action.click()
  else field.form?.requestSubmit()
  return true
}

export function handleFormFlowEnter(event) {
  if (event.key !== 'Enter' || event.defaultPrevented || event.isComposing || event.keyCode === 229) return
  if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return

  const field = event.target
  if (!(field instanceof HTMLElement) || !field.matches(fieldSelector)) return
  if (field.matches('textarea, [contenteditable="true"]') || !isFormFlowField(field) || isExpandedCombobox(field)) return
  if (handleFilterEnter(event, field)) return

  const scope = fieldScope(field)
  const fields = getFormFlowFields(scope)
  const index = fields.indexOf(field)
  if (index < 0) return

  event.preventDefault()
  const next = fields[index + 1]
  if (next) {
    next.focus({ preventScroll: true })
    next.scrollIntoView({ block: 'nearest', inline: 'nearest' })
    return
  }

  submitScope(scope, field)
}

function fieldsChanged(previous, current) {
  return previous.length !== current.length || previous.some((field, index) => field !== current[index])
}

function activeFormSurfaces() {
  const surfaces = new Set()
  document.querySelectorAll(fieldSelector).forEach((field) => {
    const surface = field.closest(surfaceSelector)
    if (!surface || !isVisible(surface)) return
    if (surface.classList.contains('v-navigation-drawer') && !surface.classList.contains('v-navigation-drawer--active')) return
    surfaces.add(surface)
  })
  return Array.from(surfaces)
}

function pageSurface() {
  return document.querySelector('[data-form-flow-page]') || document.querySelector('.route-page') || document.querySelector('.v-main') || document.querySelector('main') || document.body
}

export function useFormFlow() {
  const route = useRoute()
  const openSurfaces = new Set()
  const focusedSurfaces = new WeakSet()
  const surfaceFields = new WeakMap()
  const surfaceTimers = new Map()
  let currentPage = null
  let currentPageFields = []
  let pageFocused = false
  let pageTimer = null
  let observer = null

  function scheduleSurfaceFocus(surface) {
    if (surfaceTimers.has(surface)) return
    const timer = window.setTimeout(async () => {
      surfaceTimers.delete(surface)
      await nextTick()
      if (!surface.isConnected || !isVisible(surface)) return
      if (focusFirstFormFlowField(surface)) focusedSurfaces.add(surface)
    }, 100)
    surfaceTimers.set(surface, timer)
  }

  function schedulePageFocus(page) {
    if (pageTimer !== null) return
    pageTimer = window.setTimeout(async () => {
      pageTimer = null
      await nextTick()
      if (activeFormSurfaces().length || page !== pageSurface()) return
      if (focusFirstFormFlowField(page)) pageFocused = true
    }, 80)
  }

  function scan() {
    const surfaces = activeFormSurfaces()
    const activeSet = new Set(surfaces)

    openSurfaces.forEach((surface) => {
      if (activeSet.has(surface)) return
      openSurfaces.delete(surface)
      focusedSurfaces.delete(surface)
      surfaceFields.delete(surface)
      const timer = surfaceTimers.get(surface)
      if (timer) window.clearTimeout(timer)
      surfaceTimers.delete(surface)
    })

    surfaces.forEach((surface) => {
      const fields = getFormFlowFields(surface)
      const previous = surfaceFields.get(surface) || []
      const changed = fieldsChanged(previous, fields)
      surfaceFields.set(surface, fields)

      if (!openSurfaces.has(surface)) {
        openSurfaces.add(surface)
        scheduleSurfaceFocus(surface)
      } else if (!focusedSurfaces.has(surface) || (changed && !hasActiveField(surface))) {
        scheduleSurfaceFocus(surface)
      }
    })

    if (surfaces.length) return
    const page = pageSurface()
    const fields = getFormFlowFields(page)
    if (page !== currentPage) {
      currentPage = page
      currentPageFields = fields
      pageFocused = false
    } else if (fieldsChanged(currentPageFields, fields)) {
      currentPageFields = fields
      if (!hasActiveField(page)) pageFocused = false
    }
    if (!pageFocused) schedulePageFocus(page)
  }

  watch(() => route.fullPath, async () => {
    currentPage = null
    currentPageFields = []
    pageFocused = false
    await nextTick()
    scan()
  }, { flush: 'post' })

  onMounted(() => {
    document.addEventListener('keydown', handleFormFlowEnter)
    observer = new MutationObserver(scan)
    observer.observe(document.body, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ['class', 'style', 'aria-hidden', 'disabled', 'readonly'],
    })
    scan()
  })

  onBeforeUnmount(() => {
    document.removeEventListener('keydown', handleFormFlowEnter)
    observer?.disconnect()
    if (pageTimer !== null) window.clearTimeout(pageTimer)
    surfaceTimers.forEach((timer) => window.clearTimeout(timer))
    surfaceTimers.clear()
  })
}
