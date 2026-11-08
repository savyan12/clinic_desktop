const defaults = {
  body_font_size: 16,
  body_font_weight: 400,
  table_font_size: 15,
  table_font_weight: 400,
  heading_font_size: 28,
  heading_font_weight: 600,
  section_font_size: 20,
  section_font_weight: 600,
}

export function applyTypographySettings(settings = {}) {
  const values = { ...defaults, ...settings }
  const root = document.documentElement.style

  // Update the design-system primitives as well as their compatibility
  // aliases. Some shared components intentionally consume the primitives,
  // while older screens still use the --font-* names.
  root.setProperty('--fs-body', `${values.body_font_size}px`)
  root.setProperty('--fw-body', String(values.body_font_weight))
  root.setProperty('--fs-table', `${values.table_font_size}px`)
  root.setProperty('--fw-table', String(values.table_font_weight))
  root.setProperty('--fs-page', `${values.heading_font_size}px`)
  root.setProperty('--fw-page', String(values.heading_font_weight))
  root.setProperty('--fs-section', `${values.section_font_size}px`)
  root.setProperty('--fw-section', String(values.section_font_weight))

  root.setProperty('--font-body-size', `${values.body_font_size}px`)
  root.setProperty('--font-body-weight', String(values.body_font_weight))
  root.setProperty('--font-table-size', `${values.table_font_size}px`)
  root.setProperty('--font-table-weight', String(values.table_font_weight))
  root.setProperty('--font-heading-size', `${values.heading_font_size}px`)
  root.setProperty('--font-heading-weight', String(values.heading_font_weight))
  root.setProperty('--font-section-size', `${values.section_font_size}px`)
  root.setProperty('--font-section-weight', String(values.section_font_weight))
}
