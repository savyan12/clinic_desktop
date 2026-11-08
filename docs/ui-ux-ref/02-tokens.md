# الـ Tokens

الملف التنفيذي: `tokens.css`. هذا الملف يشرح القرارات.

## الألوان

القرار الأساسي: نبعد عن أخضر الشعار كلون للواجهة. الأخضر يبقى للنجاح (مدفوع، مكتمل) فقط، واللون التفاعلي أزرق مأخوذ من Chrono Desk.

| الدور | القيمة | ملاحظة |
|---|---|---|
| canvas | `#F3F5F9` | بين خلفيات Nexus (`#F3F7FA`) و NevBank (`#EFF0F5`) |
| surface | `#FFFFFF` | cards وsidebar وmodals |
| surface-subtle | `#F6F7FB` | رأس الجدول |
| ink | `#14161F` | عناوين + البطاقة الداكنة (مثل Whale/SaasCa) |
| accent | `#1A62E0` | أغمق شوية من أزرق Chrono (`#1E75FF`) لأن الأبيض عليه 4.16:1 فقط، والقيمة الجديدة 5.43:1 |
| accent-tint | `#EAF1FE` | nav نشط، صف محدد |
| success / warning / danger | انظر tokens.css | chip = خلفية فاتحة + نص غامق، كلها فوق 4.5:1 |
| KPI tints | `#E5EFFB` `#F4ECFF` `#E2FAEA` `#FFF8DE` | مأخوذة من بطاقات Chrono |

> لو بغينا نغيّر هوية اللون لاحقًا (أخضر مثل Nexus/NevBank مثلاً)، نغيّر `--accent*` فقط. ما نلمس باقي الملفات.

### استعمال الألوان الدلالية (ثابت في كل النظام)

| الحالة | اللون |
|---|---|
| مدفوع، مكتمل، نشط، مستحق مصروف | success |
| قريب الانتهاء، مدفوع جزئيًا، معلّق | warning |
| دين، ملغي، منتهي، خطأ | danger |
| معلومة، مجدول | info |
| اشتراك/باقة (تمييز النوع) | violet |

## الخط

- الأساسي: **IBM Plex Sans Arabic** (أوزان 400/500/600/700)، ثم Noto Sans Arabic، ثم system-ui.
- الأرقام لاتينية (حسب إعداد العملة الحالي) مع `font-variant-numeric: tabular-nums` في الجداول والمبالغ.
- التنوين في عنوان "تحتاج متابعة قريبًا" يبان مكسور في الخط الحالي. جرّب الخط الجديد، وإن بقي المشكل زد `line-height` للعناوين إلى 1.4.

### أدوار الخط القابلة للتخصيص (من صفحة الإعدادات)

| الدور | المتغيرات | الافتراضي |
|---|---|---|
| النص العام | `--fs-body` / `--fw-body` | 16 / 400 |
| نص الجداول | `--fs-table` / `--fw-table` | 15 / 400 |
| عنوان القسم | `--fs-section` / `--fw-section` | 20 / 600 |
| عنوان الصفحة | `--fs-page` / `--fw-page` | 32 / 600 |

أحجام الاختيارات (صغير، متوسط، كبير، كبير جدًا) تتحول لقيم px تُكتب على `:root`. أي مكون يستعمل المتغيرات هذي فقط.

## المسافات والزوايا

- أساس 4px. padding الصفحة 32، padding الـ card 24، gap الشبكة 20.
- الزوايا: card 18، control 12، chip كامل.
- ارتفاع الـ control: 44، وفي فلاتر الجداول 36.

## الظلال
Card بظل ناعم لكنه واضح (`--shadow-card`) مثل Nexus، وترتفع شوية عند hover لو قابلة للنقر (`--shadow-card-hover`). الظل القوي للـ modal والـ dropdown فقط.

## بديل لون التفاعل
`tokens.accent-emerald.css`: نسخة زمردية قريبة من Nexus. لا تُستعمل مع `tokens.css` الأزرق إلا كـ override. أخضر Nexus الأصلي (`#0AB980`) تباينه مع الأبيض 2.54:1 فقط، فاستعملنا `#087F5B` (5.0:1).

## تباين النص (تم الحساب)

| الزوج | النسبة |
|---|---|
| text على surface | 10.46 |
| text-muted على surface | 4.97 |
| text-muted على canvas | 4.56 |
| أبيض على accent | 5.43 |
| success-fg على success-bg | 4.86 |
| warning-fg على warning-bg | 4.95 |
| danger-fg على danger-bg | 5.75 |
| text-faint على surface | 3.15 (للـ placeholder والـ disabled فقط) |

## ربط Vuetify (إذا المشروع عليه)

```ts
// plugins/vuetify.ts (sketch)
import { createVuetify } from 'vuetify'

export default createVuetify({
  theme: {
    defaultTheme: 'clinic',
    themes: {
      clinic: {
        dark: false,
        colors: {
          background: '#F3F5F9',
          surface: '#FFFFFF',
          primary: '#1A62E0',
          success: '#0E9F6E',
          warning: '#F5A524',
          error: '#E5484D',
          info: '#1A62E0',
        },
      },
    },
  },
  defaults: {
    VBtn: { rounded: 'lg', elevation: 0, class: 'text-none' },
    VCard: { rounded: 'xl', elevation: 0 },
    VTextField: { variant: 'outlined', density: 'comfortable', hideDetails: 'auto' },
    VSelect: { variant: 'outlined', density: 'comfortable', hideDetails: 'auto' },
  },
})
```

الـ floating label يتخلص منه بمكون `AppField` (label فوق الحقل) يلف `v-text-field` — شوف `03-components.md`.
