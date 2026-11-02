import { createVuetify } from 'vuetify'
import { ar } from 'vuetify/locale'

export default createVuetify({
  locale: {
    locale: 'ar',
    fallback: 'ar',
    messages: { ar },
  },
  rtl: {
    ar: true,
  },
  theme: {
    defaultTheme: 'clinicLight',
    themes: {
      clinicLight: {
        dark: false,
        colors: {
          primary: '#0066cc',
          'primary-focus': '#0071e3',
          surface: '#ffffff',
          background: '#ffffff',
          'on-surface': '#1d1d1f',
          'on-background': '#1d1d1f',
          ink: '#1d1d1f',
          muted: '#4f4f54',
          parchment: '#f5f5f7',
          pearl: '#fafafc',
          hairline: '#e0e0e0',
          success: '#1a7f37',
          error: '#c5221f',
          warning: '#9a5b00',
          info: '#0066cc',
        },
      },
    },
  },
  defaults: {
    global: {
      elevation: 0,
    },
    VBtn: {
      rounded: 'pill',
      elevation: 0,
      minHeight: 44,
    },
    VCard: {
      rounded: 'lg',
      elevation: 0,
      variant: 'flat',
    },
    VTextField: {
      variant: 'outlined',
      density: 'comfortable',
      hideDetails: 'auto',
      rounded: 'lg',
    },
    VSelect: {
      variant: 'outlined',
      density: 'comfortable',
      hideDetails: 'auto',
      rounded: 'lg',
    },
    VAutocomplete: {
      variant: 'outlined',
      density: 'comfortable',
      hideDetails: 'auto',
      rounded: 'lg',
    },
    VTextarea: {
      variant: 'outlined',
      hideDetails: 'auto',
      rounded: 'lg',
    },
    VChip: {
      rounded: 'pill',
      elevation: 0,
    },
    VDialog: {
      maxWidth: 760,
      transition: 'fade-transition',
    },
    VWindowItem: {
      transition: false,
      reverseTransition: false,
    },
  },
})
