<template>
  <div class="page-container page-stack" data-form-flow-scope>
    <PageHeader title="إعدادات النظام" subtitle="الطباعة والخطوط والنسخ الاحتياطي المحلي">
      <template #actions><v-btn color="primary" prepend-icon="mdi-content-save-outline" data-form-flow-primary :loading="saving" @click="save">حفظ الإعدادات</v-btn></template>
    </PageHeader>
    <v-skeleton-loader v-if="loading" type="heading, article, image" />
    <InlineError v-else-if="error" :message="error" retry @retry="load" />
    <div v-else class="settings-content page-stack">
      <section class="surface-card settings-section">
        <h2 class="section-heading">وضوح النص وأحجام الخطوط</h2>
        <p>اختر الحجم والوزن المناسبين. تُعرض القيمة الفعلية بالبكسل وتظهر النتيجة مباشرة في معاينة كل مجموعة.</p>

        <div class="typography-grid">
          <article v-for="control in typographyControls" :key="control.sizeKey" class="type-control">
            <div class="type-control-main">
              <div class="type-control-heading">
                <strong>{{ control.label }}</strong>
                <span>{{ control.description }}</span>
              </div>

              <div class="segmented-field">
                <div class="segment-label">
                  <span>الحجم</span>
                  <bdi dir="ltr">{{ form[control.sizeKey] }} px</bdi>
                </div>
                <v-btn-toggle
                  v-model="form[control.sizeKey]"
                  mandatory
                  color="primary"
                  variant="outlined"
                  class="segmented-toggle size-toggle"
                  role="radiogroup"
                  :aria-label="`حجم ${control.label}`"
                  @keydown="handleSegmentKeydown($event, control.sizeKey, control.sizes)"
                >
                  <v-btn
                    v-for="option in control.sizes"
                    :key="option.value"
                    :value="option.value"
                    :data-segment-option="segmentOptionId(control.sizeKey, option.value)"
                    role="radio"
                    :aria-checked="form[control.sizeKey] === option.value"
                    :aria-label="`${option.title}، ${option.value} بكسل`"
                  >{{ option.title }}</v-btn>
                </v-btn-toggle>
              </div>

              <div class="segmented-field">
                <div class="segment-label"><span>وزن الخط</span></div>
                <v-btn-toggle
                  v-model="form[control.weightKey]"
                  mandatory
                  color="primary"
                  variant="outlined"
                  class="segmented-toggle weight-toggle"
                  role="radiogroup"
                  :aria-label="`وزن ${control.label}`"
                  @keydown="handleSegmentKeydown($event, control.weightKey, fontWeights)"
                >
                  <v-btn
                    v-for="option in fontWeights"
                    :key="option.value"
                    :value="option.value"
                    :data-segment-option="segmentOptionId(control.weightKey, option.value)"
                    role="radio"
                    :aria-checked="form[control.weightKey] === option.value"
                    :aria-label="`${option.title}، وزن ${option.value}`"
                  >
                    <span>{{ option.title }}</span>
                    <small>{{ option.value }}</small>
                  </v-btn>
                </v-btn-toggle>
              </div>
            </div>

            <aside class="type-sample" aria-live="polite">
              <span>معاينة حيّة</span>
              <strong :style="previewStyleFor(control)">{{ control.preview }}</strong>
              <small><bdi dir="ltr">{{ form[control.sizeKey] }} px · {{ form[control.weightKey] }}</bdi></small>
            </aside>
          </article>
        </div>
      </section>

      <section class="surface-card settings-section">
        <h2 class="section-heading">العملة والطباعة</h2>
        <p>الأرقام لاتينية دون فاصل آلاف، وبحد أقصى 3 منازل عشرية باستخدام النقطة.</p>
        <div class="form-grid">
          <v-text-field v-model="form.currency" label="رمز العملة" />
          <v-text-field model-value="80 د.ل" label="معاينة التنسيق" disabled />
          <v-select v-model="form.print_size" label="الحجم الافتراضي" :items="['A4', 'A5']" />
          <v-switch v-model="form.show_logo" label="إظهار علامة المركز" color="primary" hide-details />
        </div>
      </section>

      <section v-if="desktop.isDesktop" class="surface-card settings-section" data-form-flow-scope>
        <div class="section-row"><div><h2 class="section-heading">نسخة احتياطية على USB</h2><p>يُنشأ مجلد بتاريخ اليوم، وتُفحص البصمة ثم تُجرّب الاستعادة في قاعدة مؤقتة قبل اعتماد النسخة.</p></div><v-btn variant="outlined" prepend-icon="mdi-refresh" :loading="loadingDrives" @click="refreshDrives">تحديث الوحدات</v-btn></div>
        <InlineError v-if="backupError" :message="backupError" />
        <v-select v-model="selectedDrive" label="اختر وحدة USB" :items="desktop.drives" :item-title="driveTitle" item-value="id" no-data-text="لا توجد وحدة USB متصلة" />
        <div class="backup-warning"><v-icon icon="mdi-usb-flash-drive-outline" /><span>لا تنزع وحدة USB أثناء النسخ أو التحقق. لا تُعد النسخة ناجحة إلا بعد ظهور رسالة اكتمال التحقق.</span></div>
        <div v-if="backupResult" class="backup-result"><v-icon icon="mdi-shield-check-outline" /><div><strong>تم إنشاء نسخة موثقة ومجرّبة الاستعادة</strong><span>{{ backupResult.folder }}</span><span>{{ formatDate(backupResult.created_at) }} · {{ formatBytes(backupResult.dump_size) }}</span></div></div>
        <div class="backup-actions"><v-btn color="primary" prepend-icon="mdi-database-export-outline" data-form-flow-primary :disabled="!selectedDrive" :loading="backingUp" @click="createBackup">إنشاء النسخة والتحقق منها</v-btn></div>
      </section>
    </div>
  </div>
</template>

<script setup>
import { nextTick, onMounted, reactive, ref } from 'vue'
import { getSettings, updateSettings } from '@/api/settings'
import InlineError from '@/components/common/InlineError.vue'
import PageHeader from '@/components/common/PageHeader.vue'
import { desktopErrorMessage } from '@/desktop/bridge'
import { useDesktopStore } from '@/stores/desktop'
import { useUiStore } from '@/stores/ui'
import { applyTypographySettings } from '@/utils/typography'

const ui = useUiStore()
const desktop = useDesktopStore()
const form = reactive({ clinic_name: '', clinic_subtitle: '', phone: '', address: '', receipt_footer: '', currency: 'د.ل', print_size: 'A4', show_logo: true, body_font_size: 16, body_font_weight: 400, table_font_size: 15, table_font_weight: 400, heading_font_size: 32, heading_font_weight: 600, section_font_size: 22, section_font_weight: 600 })
const loading = ref(true)
const error = ref('')
const saving = ref(false)
const loadingDrives = ref(false)
const backingUp = ref(false)
const backupError = ref('')
const backupResult = ref(null)
const selectedDrive = ref(null)
const fontWeights = [{ title: 'عادي', value: 400 }, { title: 'بارز', value: 600 }]
const sizeLabels = ['صغير', 'متوسط', 'كبير', 'كبير جدًا']
const sizeOptions = (values) => values.map((value, index) => ({ title: sizeLabels[index], value }))
const typographyControls = [
  {
    label: 'النص العام',
    description: 'النص المستخدم داخل البطاقات والنماذج.',
    preview: 'نص واضح وسهل القراءة',
    sizeKey: 'body_font_size',
    weightKey: 'body_font_weight',
    sizes: sizeOptions([14, 16, 18, 20]),
    defaultSize: 16,
    defaultWeight: 400,
  },
  {
    label: 'نص الجداول',
    description: 'القيم والعناوين داخل الجداول.',
    preview: 'اسم المريض وقيمة الجلسة',
    sizeKey: 'table_font_size',
    weightKey: 'table_font_weight',
    sizes: sizeOptions([13, 15, 17, 18]),
    defaultSize: 15,
    defaultWeight: 400,
  },
  {
    label: 'عنوان الصفحة',
    description: 'العنوان الرئيسي أعلى كل صفحة.',
    preview: 'إعدادات النظام',
    sizeKey: 'heading_font_size',
    weightKey: 'heading_font_weight',
    sizes: sizeOptions([24, 32, 36, 40]),
    defaultSize: 32,
    defaultWeight: 600,
  },
  {
    label: 'عنوان القسم',
    description: 'عناوين البطاقات والأقسام الداخلية.',
    preview: 'بيانات الجلسة',
    sizeKey: 'section_font_size',
    weightKey: 'section_font_weight',
    sizes: sizeOptions([18, 22, 25, 28]),
    defaultSize: 22,
    defaultWeight: 600,
  },
]

onMounted(async () => { await load(); if (desktop.isDesktop) await refreshDrives() })

async function load() {
  loading.value = true
  error.value = ''
  try {
    const settings = (await getSettings()).data
    Object.assign(form, settings)
    normalizeTypographySelections()
    applyTypographySettings(form)
  } catch (apiError) {
    error.value = apiError.message
  } finally {
    loading.value = false
  }
}

async function save() {
  saving.value = true
  try {
    const settings = (await updateSettings(form)).data
    Object.assign(form, settings)
    normalizeTypographySelections()
    applyTypographySettings(form)
    ui.showSnackbar('تم حفظ إعدادات الطباعة وأحجام الخطوط.')
  } catch (apiError) {
    ui.showSnackbar(apiError.message, 'error')
  } finally {
    saving.value = false
  }
}

function closestOptionValue(value, options, fallback) {
  const numericValue = Number(value)
  if (!Number.isFinite(numericValue)) return fallback
  return options.reduce((closest, option) => (
    Math.abs(option.value - numericValue) < Math.abs(closest - numericValue)
      ? option.value
      : closest
  ), options[0].value)
}

function normalizeTypographySelections() {
  typographyControls.forEach((control) => {
    form[control.sizeKey] = closestOptionValue(form[control.sizeKey], control.sizes, control.defaultSize)
    form[control.weightKey] = closestOptionValue(form[control.weightKey], fontWeights, control.defaultWeight)
  })
}

const previewStyleFor = (control) => ({
  fontSize: `${form[control.sizeKey]}px`,
  fontWeight: form[control.weightKey],
})

const segmentOptionId = (key, value) => `${key}-${value}`

async function handleSegmentKeydown(event, key, options) {
  const values = options.map((option) => option.value)
  const currentIndex = Math.max(0, values.indexOf(form[key]))
  let nextIndex = currentIndex

  if (event.key === 'ArrowLeft' || event.key === 'ArrowDown') nextIndex = Math.min(values.length - 1, currentIndex + 1)
  else if (event.key === 'ArrowRight' || event.key === 'ArrowUp') nextIndex = Math.max(0, currentIndex - 1)
  else if (event.key === 'Home') nextIndex = 0
  else if (event.key === 'End') nextIndex = values.length - 1
  else return

  event.preventDefault()
  const nextValue = values[nextIndex]
  form[key] = nextValue
  await nextTick()
  document.querySelector(`[data-segment-option="${segmentOptionId(key, nextValue)}"]`)?.focus()
}

async function refreshDrives() {
  loadingDrives.value = true
  backupError.value = ''
  try {
    await desktop.refreshDrives()
    if (!desktop.drives.some((drive) => drive.id === selectedDrive.value)) selectedDrive.value = null
  } catch (cause) {
    backupError.value = desktopErrorMessage(cause, 'تعذر قراءة وحدات USB المتصلة.')
  } finally {
    loadingDrives.value = false
  }
}

async function createBackup() {
  backingUp.value = true
  backupError.value = ''
  backupResult.value = null
  try {
    backupResult.value = await desktop.createBackup(selectedDrive.value)
    ui.showSnackbar('تم إنشاء النسخة على USB والتحقق من البصمة ونجاح الاستعادة التجريبية.')
  } catch (cause) {
    backupError.value = desktopErrorMessage(cause, 'تعذر إنشاء النسخة الاحتياطية.')
    ui.showSnackbar('لم تُعتمد النسخة الاحتياطية لأن التحقق لم يكتمل.', 'error')
  } finally {
    backingUp.value = false
  }
}

const driveTitle = (drive) => `${drive.label} — ${drive.path}${drive.free_bytes ? ` (${formatBytes(drive.free_bytes)} متاح)` : ''}`
const formatDate = (value) => new Intl.DateTimeFormat('ar-LY-u-nu-latn', { dateStyle: 'long', timeStyle: 'short' }).format(new Date(value))
function formatBytes(value) {
  const bytes = Number(value || 0)
  return bytes < 1024 * 1024 ? `${Math.max(1, Math.round(bytes / 1024))} KB` : `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}
</script>

<style scoped>
.settings-content {
  width: 100%;
}

.settings-section {
  display: grid;
  gap: 16px;
}

.settings-section > p {
  margin: -8px 0 0;
  color: var(--color-ink-muted);
}

.form-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
}

.typography-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
}

.type-control {
  min-width: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(165px, .42fr);
  gap: 20px;
  padding: 20px;
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-field);
}

.type-control-main,
.type-control-heading,
.segmented-field {
  min-width: 0;
  display: grid;
}

.type-control-main {
  gap: 16px;
}

.type-control-heading {
  gap: 3px;
}

.type-control-heading strong {
  font-size: 17px;
  font-weight: 600;
}

.type-control-heading span,
.segment-label,
.type-sample > span,
.type-sample small {
  color: var(--color-ink-muted);
  font-size: 12px;
}

.segmented-field {
  gap: 7px;
}

.segment-label {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  font-weight: 600;
}

.segment-label bdi {
  color: var(--color-ink);
  font-variant-numeric: tabular-nums;
}

.segmented-toggle {
  width: 100%;
  display: grid !important;
  grid-auto-flow: column;
  grid-auto-columns: minmax(0, 1fr);
}

.segmented-toggle :deep(.v-btn) {
  min-width: 0;
  min-height: 42px;
  padding-inline: 8px;
}

.segmented-toggle :deep(.v-btn__content) {
  min-width: 0;
  white-space: normal;
}

.weight-toggle :deep(.v-btn__content) {
  display: flex;
  gap: 6px;
}

.weight-toggle small {
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  opacity: .78;
}

.type-sample {
  min-width: 0;
  min-height: 160px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 16px;
  overflow: hidden;
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-field);
  background: var(--color-pearl);
  text-align: center;
}

.type-sample strong {
  max-width: 100%;
  display: block;
  line-height: 1.3;
  overflow-wrap: anywhere;
}

.type-sample small {
  direction: ltr;
  font-variant-numeric: tabular-nums;
}

.section-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.section-row p {
  margin: 6px 0 0;
}

.backup-warning {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 14px;
  color: var(--color-warning);
  border: 1px solid var(--color-warning-border);
  border-radius: var(--radius-field);
  background: var(--color-warning-bg);
}

.backup-result {
  display: flex;
  gap: 12px;
  padding: 16px;
  color: var(--color-success);
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-field);
  background: var(--color-success-bg);
}

.backup-result div,
.backup-result span {
  display: block;
}

.backup-result span {
  margin-top: 3px;
  font-size: 13px;
  overflow-wrap: anywhere;
}

.backup-actions {
  display: flex;
  justify-content: flex-end;
}

@media (max-width: 1100px) {
  .typography-grid {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 700px) {
  .type-control {
    grid-template-columns: 1fr;
  }

  .type-sample {
    min-height: 120px;
  }
}

@media (max-width: 640px) {
  .form-grid {
    grid-template-columns: 1fr;
  }

  .section-row {
    flex-direction: column;
  }

  .size-toggle :deep(.v-btn) {
    padding-inline: 4px;
    font-size: 11px;
  }
}
</style>
