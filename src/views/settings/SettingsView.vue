<template>
  <div class="page-container page-stack">
    <PageHeader title="إعدادات النظام" subtitle="بيانات المركز والطباعة والخطوط والنسخ الاحتياطي المحلي">
      <template #actions><v-btn color="primary" prepend-icon="mdi-content-save-outline" :loading="saving" @click="save">حفظ الإعدادات</v-btn></template>
    </PageHeader>
    <v-skeleton-loader v-if="loading" type="heading, article, image" />
    <InlineError v-else-if="error" :message="error" retry @retry="load" />
    <div v-else class="settings-grid">
      <div class="page-stack">
        <section class="surface-card settings-section">
          <h2 class="section-heading">بيانات المركز</h2>
          <p>تظهر هذه البيانات في الفواتير والإيصالات وكشوف الحساب.</p>
          <div class="form-grid"><v-text-field v-model="form.clinic_name" label="اسم المركز" /><v-text-field v-model="form.clinic_subtitle" label="الوصف المختصر" /><v-text-field v-model="form.phone" label="رقم الهاتف" dir="ltr" /><v-text-field v-model="form.address" label="العنوان" /></div>
          <v-textarea v-model="form.receipt_footer" label="تذييل الإيصال" rows="2" />
        </section>

        <section class="surface-card settings-section">
          <h2 class="section-heading">وضوح النص وأحجام الخطوط</h2>
          <p>الأوزان المتاحة هي 400 للنص العادي و600 للنص البارز؛ وهي الأوزان المثبتة داخل التطبيق.</p>
          <div class="typography-grid">
            <div class="type-control"><strong>النص العام</strong><v-slider v-model="form.body_font_size" :min="14" :max="20" :step="1" thumb-label /><v-select v-model="form.body_font_weight" label="الوزن" :items="fontWeights" /></div>
            <div class="type-control"><strong>نص الجداول</strong><v-slider v-model="form.table_font_size" :min="13" :max="18" :step="1" thumb-label /><v-select v-model="form.table_font_weight" label="الوزن" :items="fontWeights" /></div>
            <div class="type-control"><strong>عنوان الصفحة</strong><v-slider v-model="form.heading_font_size" :min="24" :max="40" :step="1" thumb-label /><v-select v-model="form.heading_font_weight" label="الوزن" :items="fontWeights" /></div>
            <div class="type-control"><strong>عنوان القسم</strong><v-slider v-model="form.section_font_size" :min="18" :max="28" :step="1" thumb-label /><v-select v-model="form.section_font_weight" label="الوزن" :items="fontWeights" /></div>
          </div>
          <div class="type-preview" :style="previewStyle"><strong>معاينة النص</strong><span>هذا نص تجريبي للتأكد من وضوح القراءة داخل النظام.</span></div>
        </section>

        <section class="surface-card settings-section">
          <h2 class="section-heading">العملة والطباعة</h2>
          <p>الأرقام لاتينية دون فاصل آلاف، وبحد أقصى 3 منازل عشرية باستخدام النقطة.</p>
          <div class="form-grid"><v-text-field v-model="form.currency" label="رمز العملة" /><v-text-field model-value="80 د.ل" label="معاينة التنسيق" disabled /><v-select v-model="form.print_size" label="الحجم الافتراضي" :items="['A4', 'A5']" /><v-switch v-model="form.show_logo" label="إظهار علامة المركز" color="primary" hide-details /></div>
        </section>

        <section v-if="desktop.isDesktop" class="surface-card settings-section">
          <div class="section-row"><div><h2 class="section-heading">نسخة احتياطية على USB</h2><p>يُنشأ مجلد بتاريخ اليوم، وتُفحص البصمة ثم تُجرّب الاستعادة في قاعدة مؤقتة قبل اعتماد النسخة.</p></div><v-btn variant="outlined" prepend-icon="mdi-refresh" :loading="loadingDrives" @click="refreshDrives">تحديث الوحدات</v-btn></div>
          <InlineError v-if="backupError" :message="backupError" />
          <v-select v-model="selectedDrive" label="اختر وحدة USB" :items="desktop.drives" :item-title="driveTitle" item-value="id" no-data-text="لا توجد وحدة USB متصلة" />
          <div class="backup-warning"><v-icon icon="mdi-usb-flash-drive-outline" /><span>لا تنزع وحدة USB أثناء النسخ أو التحقق. لا تُعد النسخة ناجحة إلا بعد ظهور رسالة اكتمال التحقق.</span></div>
          <div v-if="backupResult" class="backup-result"><v-icon icon="mdi-shield-check-outline" /><div><strong>تم إنشاء نسخة موثقة ومجرّبة الاستعادة</strong><span>{{ backupResult.folder }}</span><span>{{ formatDate(backupResult.created_at) }} · {{ formatBytes(backupResult.dump_size) }}</span></div></div>
          <div class="backup-actions"><v-btn color="primary" prepend-icon="mdi-database-export-outline" :disabled="!selectedDrive" :loading="backingUp" @click="createBackup">إنشاء النسخة والتحقق منها</v-btn></div>
        </section>
      </div>

      <aside class="receipt-preview surface-card">
        <span class="preview-label">معاينة الإيصال</span>
        <div class="preview-brand"><div v-if="form.show_logo" class="preview-icon"><ClinicLogo :size="64" /></div><h2>{{ form.clinic_name || 'اسم المركز' }}</h2><p>{{ form.clinic_subtitle }}</p></div>
        <div class="preview-meta"><span>{{ form.phone }}</span><span>{{ form.address }}</span></div>
        <div class="preview-lines"><span></span><span></span><span></span><span></span></div>
        <p class="preview-footer">{{ form.receipt_footer }}</p>
      </aside>
    </div>
  </div>
</template>

<script setup>
import { computed, onMounted, reactive, ref } from 'vue'
import { getSettings, updateSettings } from '@/api/settings'
import ClinicLogo from '@/components/common/ClinicLogo.vue'
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
const fontWeights = [{ title: '400 — عادي', value: 400 }, { title: '600 — بارز', value: 600 }]
const previewStyle = computed(() => ({ fontSize: `${form.body_font_size}px`, fontWeight: form.body_font_weight }))

onMounted(async () => { await load(); if (desktop.isDesktop) await refreshDrives() })

async function load() {
  loading.value = true
  error.value = ''
  try {
    const settings = (await getSettings()).data
    Object.assign(form, settings)
    applyTypographySettings(settings)
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
    applyTypographySettings(settings)
    ui.showSnackbar('تم حفظ بيانات المركز والطباعة وأحجام الخطوط.')
  } catch (apiError) {
    ui.showSnackbar(apiError.message, 'error')
  } finally {
    saving.value = false
  }
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
.settings-grid { display: grid; grid-template-columns: minmax(0, 1fr) 360px; gap: 24px; align-items: start; }
.settings-section { display: grid; gap: 16px; }.settings-section p { margin: -8px 0 0; color: var(--color-ink-muted); }
.form-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 16px; }
.typography-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 16px; }.type-control { display: grid; gap: 8px; padding: 16px; border: 1px solid var(--color-hairline); border-radius: var(--radius-field); }.type-control strong { font-weight: 600; }
.type-preview { display: grid; gap: 4px; padding: 16px; border-radius: var(--radius-field); background: var(--color-pearl); }.type-preview strong { font-weight: 600; }
.receipt-preview { position: sticky; top: 24px; min-height: 520px; display: flex; flex-direction: column; align-items: center; gap: 16px; text-align: center; }.preview-label { align-self: flex-start; color: var(--color-ink-muted); font-size: 13px; }.preview-brand h2 { margin: 10px 0 2px; font-size: 22px; }.preview-brand p, .preview-meta span { margin: 0; color: var(--color-ink-muted); font-size: 13px; }.preview-icon { width: 64px; height: 64px; margin: auto; overflow: hidden; border: 1px solid var(--color-hairline); border-radius: 14px; background: var(--color-canvas); }.preview-meta { display: grid; gap: 2px; }.preview-lines { width: 100%; display: grid; gap: 16px; margin-top: 20px; }.preview-lines span { height: 1px; background: var(--color-hairline); }.preview-footer { margin-top: auto; color: var(--color-ink-muted); font-size: 13px; }
.section-row { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; }.section-row p { margin: 6px 0 0; }.backup-warning { display: flex; align-items: center; gap: 10px; padding: 14px; color: var(--color-warning); border: 1px solid var(--color-warning-border); border-radius: var(--radius-field); background: var(--color-warning-bg); }.backup-result { display: flex; gap: 12px; padding: 16px; color: var(--color-success); border: 1px solid var(--color-hairline); border-radius: var(--radius-field); background: var(--color-success-bg); }.backup-result div, .backup-result span { display: block; }.backup-result span { margin-top: 3px; font-size: 13px; overflow-wrap: anywhere; }.backup-actions { display: flex; justify-content: flex-end; }
@media (max-width: 950px) { .settings-grid { grid-template-columns: 1fr; }.receipt-preview { position: static; } } @media (max-width: 640px) { .form-grid, .typography-grid { grid-template-columns: 1fr; }.section-row { flex-direction: column; } }
</style>
