const defaults = {
  body_font_size: 16,
  body_font_weight: 400,
  table_font_size: 15,
  table_font_weight: 400,
  heading_font_size: 32,
  heading_font_weight: 600,
  section_font_size: 22,
  section_font_weight: 600,
}

export function applyTypographySettings(settings = {}) {
  const values = { ...defaults, ...settings }
  const root = document.documentElement.style
  root.setProperty('--font-body-size', `${values.body_font_size}px`)
  root.setProperty('--font-body-weight', String(values.body_font_weight))
  root.setProperty('--font-table-size', `${values.table_font_size}px`)
  root.setProperty('--font-table-weight', String(values.table_font_weight))
  root.setProperty('--font-heading-size', `${values.heading_font_size}px`)
  root.setProperty('--font-heading-weight', String(values.heading_font_weight))
  root.setProperty('--font-section-size', `${values.section_font_size}px`)
  root.setProperty('--font-section-weight', String(values.section_font_weight))
}
