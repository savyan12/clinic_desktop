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
          primary: '#1a62e0',
          secondary: '#344054',
          accent: '#1a62e0',
          surface: '#ffffff',
          background: '#f3f5f9',
          'on-surface': '#14161f',
          'on-background': '#14161f',
          ink: '#14161f',
          muted: '#667085',
          parchment: '#f3f5f9',
          pearl: '#f6f7fb',
          hairline: '#e6e9f0',
          success: '#0b7a55',
          error: '#b42318',
          warning: '#9a5b00',
          info: '#1a62e0',
        },
      },
    },
  },
  defaults: {
    global: {
      elevation: 0,
    },
    VBtn: {
      rounded: 'lg',
      elevation: 0,
      minHeight: 44,
    },
    VCard: {
      rounded: 'xl',
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
      scrollable: true,
    },
    VWindowItem: {
      transition: false,
      reverseTransition: false,
    },
  },
})
