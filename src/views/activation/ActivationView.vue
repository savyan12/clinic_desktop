<template>
  <main class="activation-page" data-form-flow-page>
    <section class="activation-shell">
      <header class="activation-brand">
        <ClinicLogo :size="84" />
        <div>
          <h1>{{ desktop.needsRuntimeAttention ? 'تعذر تشغيل منظومة العيادة' : 'تهيئة منظومة العيادة' }}</h1>
        </div>
      </header>

      <InlineError v-if="error" :message="error" />

      <template v-if="desktop.needsRuntimeAttention">
        <div class="runtime-problem">
          <v-icon icon="mdi-database-alert-outline" size="44" />
          <div>
            <h2>{{ desktop.runtime.recovery_required ? 'يلزم استرداد إعدادات التشغيل' : 'تعذر تشغيل الخدمات المحلية' }}</h2>
            <p>
              {{
                desktop.runtime.error ||
                  "أعد المحاولة. إذا استمرت المشكلة، لا تحذف بيانات التطبيق."
              }}
            </p>
          </div>
        </div>
        <div class="activation-actions">
          <v-btn
            color="primary"
            prepend-icon="mdi-restart"
            :loading="desktop.loading"
            @click="retryRuntime"
          >إعادة المحاولة</v-btn>
        </div>
      </template>

      <template v-else>
        <v-btn-toggle
          v-model="mode"
          mandatory
          color="primary"
          variant="outlined"
          class="segmented-control mode-toggle"
        >
          <v-btn value="new" prepend-icon="mdi-star-outline">تفعيل جديد</v-btn>
          <v-btn value="restore" prepend-icon="mdi-backup-restore">استكمال من نسخة احتياطية</v-btn>
        </v-btn-toggle>

        <section v-if="mode === 'new'" class="activation-form" data-form-flow-scope>
          <div class="section-copy">
            <h2>إنشاء منظومة جديدة</h2>
            <p>سيتم تهيئة المنظومة للعمل فور تعبئة البيانات</p>
          </div>
          <div class="form-grid">
            <v-text-field v-model="form.clinic_name" label="اسم العيادة" />
            <v-text-field
              v-model="form.clinic_subtitle"
              label="الوصف المختصر (اختياري)"
            />
            <v-text-field v-model="form.admin_name" label="اسم مدير النظام" />
            <v-text-field
              v-model="form.admin_username"
              label="اسم المستخدم"
              dir="ltr"
            />
            <v-text-field
              v-model="form.admin_password"
              label="كلمة المرور"
              :type="showPassword ? 'text' : 'password'"
              dir="ltr"
              :append-inner-icon="
                showPassword ? 'mdi-eye-off-outline' : 'mdi-eye-outline'
              "
              @click:append-inner="showPassword = !showPassword"
            />
            <v-text-field
              v-model="passwordConfirmation"
              label="تأكيد كلمة المرور"
              type="password"
              dir="ltr"
            />
          </div>
          <v-alert
            v-if="desktop.loading"
            type="info"
            variant="tonal"
            density="compact"
            role="status"
            class="activation-wait-note"
          >
            قد تستغرق التهيئة الأولى حتى 3 دقائق على بعض الأجهزة. اترك التطبيق مفتوحاً؛ لا تضغط الزر مرة أخرى.
          </v-alert>
          <div class="activation-actions">
            <v-btn
              color="primary"
              prepend-icon="mdi-check-circle-outline"
              data-form-flow-primary
              :loading="desktop.loading"
              :disabled="desktop.loading"
              @click="activateNew"
            >إنشاء وتفعيل النظام</v-btn>
          </div>
        </section>

        <section v-else class="activation-form" data-form-flow-scope>
          <div class="section-copy">
            <h2>الاستعادة من USB</h2>
            <p>
              تعرض المنظومة النسخ المكتملة التي نجح اختبار استعادتها فقط، ثم
              تتحقق من البصمة قبل الاستكمال.
            </p>
          </div>
          <div class="drive-row">
            <v-select
              v-model="selectedDrive"
              label="وحدة USB"
              :items="desktop.drives"
              :item-title="driveTitle"
              item-value="id"
              :loading="loadingDrives"
              no-data-text="لا توجد وحدة USB متصلة"
              @update:model-value="driveChanged"
            />
            <v-btn
              variant="outlined"
              prepend-icon="mdi-refresh"
              :loading="loadingDrives"
              @click="refreshDrives"
            >تحديث</v-btn>
          </div>
          <v-select
            v-model="selectedBackup"
            label="النسخة الاحتياطية"
            :items="desktop.backups"
            :item-title="backupTitle"
            item-value="id"
            :disabled="!selectedDrive"
            :loading="loadingBackups"
            no-data-text="لا توجد نسخة مكتملة على الوحدة المحددة"
          />
          <div v-if="selectedBackupRecord" class="backup-summary">
            <span>تاريخ النسخة:
              {{ formatDate(selectedBackupRecord.created_at) }}</span>
            <span>الحجم: {{ formatBytes(selectedBackupRecord.dump_size) }}</span>
            <span><v-icon icon="mdi-shield-check-outline" /> تم التحقق واختبار
              الاستعادة</span>
          </div>
          <div class="activation-actions">
            <v-btn
              color="primary"
              prepend-icon="mdi-backup-restore"
              data-form-flow-primary
              :disabled="!selectedBackup"
              :loading="desktop.loading"
              @click="restore"
            >التحقق والاستعادة والتفعيل</v-btn>
          </div>
        </section>
      </template>
    </section>
  </main>
</template>

<script setup>
import { computed, onMounted, reactive, ref } from "vue";
import { useRouter } from "vue-router";
import ClinicLogo from "@/components/common/ClinicLogo.vue";
import InlineError from "@/components/common/InlineError.vue";
import { desktopErrorMessage } from "@/desktop/bridge";
import { useAuthStore } from "@/stores/auth";
import { useDesktopStore } from "@/stores/desktop";

const router = useRouter();
const desktop = useDesktopStore();
const auth = useAuthStore();
const mode = ref("new");
const error = ref("");
const showPassword = ref(false);
const passwordConfirmation = ref("");
const loadingDrives = ref(false);
const loadingBackups = ref(false);
const selectedDrive = ref(null);
const selectedBackup = ref(null);
const form = reactive({
  clinic_name: "عيادة عين الحياة",
  clinic_subtitle: "للعلاج الطبيعي والتأهيل",
  admin_name: "",
  admin_username: "admin",
  admin_password: "",
});
const selectedBackupRecord = computed(() =>
  desktop.backups.find((item) => item.id === selectedBackup.value),
);

onMounted(() => {
  if (!desktop.needsRuntimeAttention) refreshDrives();
});

async function refreshDrives() {
  loadingDrives.value = true;
  error.value = "";
  try {
    await desktop.refreshDrives();
    if (!desktop.drives.some((drive) => drive.id === selectedDrive.value)) {
      selectedDrive.value = null;
      selectedBackup.value = null;
      desktop.backups = [];
    }
  } catch (cause) {
    error.value = desktopErrorMessage(cause, "تعذر قراءة وحدات USB المتصلة.");
  } finally {
    loadingDrives.value = false;
  }
}

async function driveChanged() {
  selectedBackup.value = null;
  loadingBackups.value = true;
  error.value = "";
  try {
    await desktop.loadBackups(selectedDrive.value);
  } catch (cause) {
    error.value = desktopErrorMessage(cause, "تعذر قراءة النسخ الاحتياطية.");
  } finally {
    loadingBackups.value = false;
  }
}

async function activateNew() {
  error.value = "";
  if (
    !form.clinic_name.trim() ||
    !form.admin_name.trim() ||
    !form.admin_username.trim()
  ) {
    error.value = "أدخل اسم العيادة واسم المدير واسم المستخدم.";
    return;
  }
  if (form.admin_password.length < 8) {
    error.value = "كلمة المرور يجب ألا تقل عن 8 أحرف.";
    return;
  }
  if (form.admin_password !== passwordConfirmation.value) {
    error.value = "تأكيد كلمة المرور غير مطابق.";
    return;
  }
  try {
    await desktop.activateNew({ ...form });
    auth.clear();
    await router.replace({ name: "login" });
  } catch (cause) {
    error.value = desktopErrorMessage(cause, "تعذر تفعيل النظام الجديد.");
  }
}

async function restore() {
  error.value = "";
  try {
    await desktop.activateFromBackup(selectedDrive.value, selectedBackup.value);
    auth.clear();
    await router.replace({ name: "login" });
  } catch (cause) {
    error.value = desktopErrorMessage(cause, "تعذر استعادة النسخة الاحتياطية.");
  }
}

async function retryRuntime() {
  error.value = "";
  try {
    await desktop.restartRuntime();
    await router.replace({ name: "login" });
  } catch (cause) {
    error.value = desktopErrorMessage(
      cause,
      "تعذر إعادة تشغيل الخدمات المحلية.",
    );
  }
}

const driveTitle = (drive) =>
  `${drive.label} — ${drive.path}${drive.free_bytes ? ` (${formatBytes(drive.free_bytes)} متاح)` : ""}`;
const backupTitle = (backup) =>
  `${formatDate(backup.created_at)} — ${formatBytes(backup.dump_size)}`;
const formatDate = (value) =>
  new Intl.DateTimeFormat("ar-LY-u-nu-latn", {
    dateStyle: "long",
    timeStyle: "short",
  }).format(new Date(value));
function formatBytes(value) {
  const bytes = Number(value || 0);
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
</script>

<style scoped>
.activation-page {
  min-height: 100dvh;
  display: grid;
  place-items: center;
  padding: 32px;
  background: var(--color-background);
}
.activation-shell {
  width: min(940px, 100%);
  display: grid;
  gap: 24px;
  padding: 32px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-dialog);
  background: var(--color-surface);
  box-shadow: var(--shadow-raised);
}
.activation-brand {
  display: flex;
  align-items: center;
  gap: 18px;
  padding-bottom: 24px;
  border-bottom: 1px solid var(--color-hairline);
}
.activation-brand h1,
.activation-brand p,
.section-copy h2,
.section-copy p {
  margin: 0;
}
.activation-brand h1 {
  font-size: var(--font-heading-size);
}
.activation-brand p,
.section-copy p {
  margin-top: 5px;
  color: var(--color-ink-muted);
}
.mode-toggle {
  width: fit-content;
}
.activation-form {
  display: grid;
  gap: 20px;
}
.form-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
}
.drive-row {
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 12px;
  align-items: start;
}
.backup-summary {
  display: flex;
  flex-wrap: wrap;
  gap: 12px 24px;
  padding: 16px;
  color: var(--color-success);
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-field);
  background: var(--color-success-bg);
}
.activation-actions {
  display: flex;
  justify-content: flex-end;
}
.runtime-problem {
  display: flex;
  align-items: flex-start;
  gap: 16px;
  padding: 20px;
  color: var(--color-danger);
  border: 1px solid var(--color-danger-border);
  border-radius: var(--radius-field);
  background: var(--color-danger-bg);
}
.runtime-problem h2,
.runtime-problem p {
  margin: 0;
}
.runtime-problem p {
  margin-top: 6px;
}
@media (max-width: 680px) {
  .activation-page {
    padding: 16px;
  }
  .activation-shell {
    padding: 22px;
  }
  .activation-brand {
    align-items: flex-start;
  }
  .form-grid,
  .drive-row {
    grid-template-columns: 1fr;
  }
  .mode-toggle {
    width: 100%;
  }
  .mode-toggle :deep(.v-btn) {
    flex: 1;
  }
}
</style>
