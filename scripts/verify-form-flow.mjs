import assert from 'node:assert/strict'
import { createServer } from 'vite'

globalThis.HTMLElement = class HTMLElement {}

class TestElement extends globalThis.HTMLElement {
  constructor(tagName, attributes = {}, classes = []) {
    super()
    this.tagName = tagName.toLowerCase()
    this.attributes = new Map(Object.entries(attributes).map(([name, value]) => [name, String(value)]))
    this.classNames = new Set(classes)
    this.children = []
    this.parentElement = null
    this.disabled = this.attributes.has('disabled')
    this.readOnly = this.attributes.has('readonly')
    this.style = { display: 'block', visibility: 'visible' }
    this.rects = [{}]
    this.clicks = 0
    this.submits = 0
    this.classList = { contains: (name) => this.classNames.has(name) }
  }

  get isConnected() { return true }
  get type() { return this.getAttribute('type') || '' }
  get form() { return this.closest('form') }
  getAttribute(name) { return this.attributes.get(name) ?? null }
  setAttribute(name, value = '') { this.attributes.set(name, String(value)) }
  append(...children) {
    children.forEach((child) => {
      child.parentElement = this
      this.children.push(child)
    })
    return this
  }
  matches(selector) { return selector.split(',').some((part) => this.matchesOne(part.trim())) }
  matchesOne(selector) {
    if (selector === 'input:not([type="hidden"])') return this.tagName === 'input' && this.type !== 'hidden'
    if (selector === ':disabled') return this.disabled
    if (selector.startsWith('.')) return this.classNames.has(selector.slice(1))

    const match = selector.match(/^([a-z-]+)?(?:\[([^=\]]+)(?:="?([^"\]]+)"?)?\])?$/i)
    if (!match) return false
    const [, tag, attribute, value] = match
    if (tag && this.tagName !== tag.toLowerCase()) return false
    if (!attribute) return Boolean(tag)
    if (!this.attributes.has(attribute)) return false
    return value === undefined || this.getAttribute(attribute) === value
  }
  closest(selector) {
    for (let current = this; current; current = current.parentElement) {
      if (current.matches(selector)) return current
    }
    return null
  }
  contains(element) {
    for (let current = element; current; current = current.parentElement) {
      if (current === this) return true
    }
    return false
  }
  querySelectorAll(selector) {
    const matches = []
    const visit = (element) => {
      element.children.forEach((child) => {
        if (child.matches(selector)) matches.push(child)
        visit(child)
      })
    }
    visit(this)
    return matches
  }
  focus() { testDocument.activeElement = this }
  scrollIntoView() {}
  getClientRects() { return this.rects }
  click() { this.clicks += 1 }
  requestSubmit() { this.submits += 1 }
}

const documentElement = new TestElement('html')
const body = new TestElement('body')
documentElement.append(body)
const testDocument = {
  documentElement,
  body,
  activeElement: body,
  createElement: (tagName) => new TestElement(tagName),
  querySelector: () => null,
  querySelectorAll: (selector) => body.querySelectorAll(selector),
}
globalThis.document = testDocument
globalThis.window = {
  getComputedStyle: (element) => element.style,
}

const server = await createServer({ server: { middlewareMode: true }, appType: 'custom', logLevel: 'silent' })
let passed = 0

function element(tagName, attributes, classes) {
  return new TestElement(tagName, attributes, classes)
}

function enter(target, overrides = {}) {
  const event = {
    key: 'Enter',
    keyCode: 13,
    target,
    defaultPrevented: false,
    isComposing: false,
    altKey: false,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    preventDefault() { this.defaultPrevented = true },
    ...overrides,
  }
  return event
}

function check(name, run) {
  run()
  passed += 1
  process.stdout.write(`PASS ${name}\n`)
}

try {
  const { focusFirstFormFlowField, getFormFlowFields, handleFormFlowEnter, isFormFlowField } = await server.ssrLoadModule('/src/composables/useFormFlow.js')

  check('Autofocus skips hidden, disabled, readonly, and ignored fields', () => {
    const scope = element('form')
    const hidden = element('input', { type: 'hidden' })
    const disabled = element('input', { disabled: '' })
    const readonly = element('input', { readonly: '' })
    const ignored = element('input', { 'data-form-flow-ignore': '' })
    const filters = element('div', { 'data-form-flow-filter': '' })
    const search = element('input')
    const available = element('input')
    filters.append(search)
    scope.append(hidden, disabled, readonly, ignored, filters, available)
    testDocument.activeElement = body
    assert.equal(focusFirstFormFlowField(scope), true)
    assert.equal(testDocument.activeElement, available)
  })

  check('Enter moves to the next available field in DOM order', () => {
    const scope = element('form')
    const first = element('input')
    const skipped = element('input', { disabled: '' })
    const next = element('input')
    scope.append(first, skipped, next)
    first.focus()
    const event = enter(first)
    handleFormFlowEnter(event)
    assert.equal(event.defaultPrevented, true)
    assert.equal(testDocument.activeElement, next)
  })

  check('Enter on the final field invokes the declared primary action', () => {
    const scope = element('form')
    const field = element('input')
    const action = element('button', { 'data-form-flow-primary': '' })
    scope.append(field, action)
    field.focus()
    handleFormFlowEnter(enter(field))
    assert.equal(action.clicks, 1)
  })

  check('Textarea Enter remains available for new lines', () => {
    const scope = element('form')
    const textarea = element('textarea')
    const action = element('button', { 'data-form-flow-primary': '' })
    scope.append(textarea, action)
    textarea.focus()
    const event = enter(textarea)
    handleFormFlowEnter(event)
    assert.equal(event.defaultPrevented, false)
    assert.equal(action.clicks, 0)
  })

  check('Filter Enter invokes only its filter action', () => {
    const filter = element('div', { 'data-form-flow-filter': '' })
    const field = element('input')
    const action = element('button', { 'data-form-flow-filter-action': '' })
    filter.append(field, action)
    field.focus()
    const event = enter(field)
    handleFormFlowEnter(event)
    assert.equal(event.defaultPrevented, true)
    assert.equal(action.clicks, 1)
  })

  check('Arabic IME composition is never interrupted', () => {
    const scope = element('form')
    const first = element('input')
    const next = element('input')
    scope.append(first, next)
    first.focus()
    const event = enter(first, { isComposing: true, keyCode: 229 })
    handleFormFlowEnter(event)
    assert.equal(event.defaultPrevented, false)
    assert.equal(testDocument.activeElement, first)
  })

  check('Expanded combobox Enter remains available for option selection', () => {
    const scope = element('form')
    const combobox = element('input', { role: 'combobox', 'aria-expanded': 'true' })
    const next = element('input')
    scope.append(combobox, next)
    combobox.focus()
    const event = enter(combobox)
    handleFormFlowEnter(event)
    assert.equal(event.defaultPrevented, false)
    assert.equal(testDocument.activeElement, combobox)
  })

  check('Interactive Vuetify selects are included while true readonly fields are skipped', () => {
    const scope = element('div')
    const selectRoot = element('div', {}, ['v-input', 'v-select'])
    const selectInput = element('input', { readonly: '' })
    const readonly = element('input', { readonly: '' })
    selectRoot.append(selectInput)
    scope.append(selectRoot, readonly)
    assert.equal(isFormFlowField(selectInput), true)
    assert.equal(isFormFlowField(readonly), false)
    assert.deepEqual(getFormFlowFields(scope), [selectInput])
  })

  process.stdout.write(`Form flow verification passed: ${passed} checks.\n`)
} finally {
  await server.close()
}
