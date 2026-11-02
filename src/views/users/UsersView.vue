<template>
  <div class="page-container page-stack">
    <PageHeader title="المستخدمون" subtitle="إدارة حسابات موظفي الاستقبال دون حذف السجل التاريخي" action-label="إضافة مستخدم" action-icon="mdi-account-plus-outline" @action="openCreate" />
    <div class="surface-card user-filter"><v-text-field v-model="search" label="بحث بالاسم أو اسم المستخدم" prepend-inner-icon="mdi-magnify" clearable @update:model-value="searchChanged" /></div>
    <DataTableCard :headers="headers" :items="users" :loading="loading" :error="error" :meta="meta" :empty="{ icon: 'mdi-account-cog-outline', title: 'لا توجد حسابات', description: 'أضف حساب موظف استقبال جديدًا.', actionLabel: 'إضافة مستخدم' }" @retry="load" @page="changePage" @empty-action="openCreate">
      <template #rows="{ items }"><tr v-for="user in items" :key="user.id"><td><strong>{{ user.name }}</strong></td><td><span class="ltr-value">{{ user.username }}</span></td><td>موظف استقبال</td><td>{{ formatDate(user.created_at) }}</td><td><StatusChip type="active" :value="user.is_active" /></td><td>{{ user.last_login_at ? formatDateTime(user.last_login_at) : 'لم يسجل الدخول' }}</td><td><v-menu><template #activator="{ props }"><v-btn v-bind="props" icon="mdi-dots-vertical" variant="text" /></template><v-list density="compact"><v-list-item prepend-icon="mdi-pencil-outline" title="تعديل" @click="openEdit(user)" /><v-list-item prepend-icon="mdi-lock-reset" title="إعادة تعيين كلمة المرور" @click="openReset(user)" /><v-list-item :prepend-icon="user.is_active ? 'mdi-account-off-outline' : 'mdi-account-check-outline'" :title="user.is_active ? 'تعطيل الحساب' : 'إعادة تفعيل الحساب'" @click="askToggle(user)" /></v-list></v-menu></td></tr></template>
    </DataTableCard>
  </div>

  <v-dialog v-model="userDialog" persistent max-width="620">
    <v-card><v-card-title class="dialog-title">{{ editing ? 'تعديل المستخدم' : 'إضافة مستخدم' }}</v-card-title><v-card-text class="dialog-content"><InlineError v-if="dialogError" :message="dialogError" /><v-text-field v-model="form.name" label="اسم الموظف" :error-messages="fieldErrors.name" /><v-text-field v-model="form.username" label="اسم المستخدم" dir="ltr" :error-messages="fieldErrors.username" /><v-text-field v-if="!editing" v-model="form.password" label="كلمة المرور الأولية" type="password" :error-messages="fieldErrors.password" /><v-text-field v-if="!editing" v-model="form.password_confirmation" label="تأكيد كلمة المرور" type="password" :error-messages="fieldErrors.password_confirmation" /><v-text-field label="الدور" model-value="موظف استقبال" disabled /><v-switch v-model="form.is_active" label="الحساب نشط" color="primary" hide-details /></v-card-text><v-card-actions class="dialog-actions"><v-btn variant="text" :disabled="saving" @click="userDialog = false">إلغاء</v-btn><v-btn color="primary" :loading="saving" @click="saveUserForm">حفظ</v-btn></v-card-actions></v-card>
  </v-dialog>

  <v-dialog v-model="resetDialog" persistent max-width="520">
    <v-card><v-card-title class="dialog-title">إعادة تعيين كلمة المرور</v-card-title><v-card-text class="dialog-content"><p>المستخدم: {{ selectedUser?.name }}</p><InlineError v-if="resetError" :message="resetError" /><v-text-field v-model="passwordForm.password" label="كلمة المرور الجديدة" type="password" /><v-text-field v-model="passwordForm.password_confirmation" label="تأكيد كلمة المرور" type="password" /></v-card-text><v-card-actions class="dialog-actions"><v-btn variant="text" :disabled="saving" @click="resetDialog = false">إلغاء</v-btn><v-btn color="primary" :loading="saving" @click="savePassword">تأكيد</v-btn></v-card-actions></v-card>
  </v-dialog>

  <ConfirmDialog v-model="toggleConfirm" :title="selectedUser?.is_active ? 'تعطيل الحساب' : 'إعادة تفعيل الحساب'" :message="selectedUser?.is_active ? 'سيُمنع المستخدم من تسجيل الدخول وتُلغى رموزه النشطة.' : 'سيتمكن المستخدم من تسجيل الدخول مجددًا.'" :confirm-label="selectedUser?.is_active ? 'تعطيل' : 'تفعيل'" :danger="selectedUser?.is_active" :loading="saving" @confirm="toggleUser" />
</template>

<script setup>
import { reactive, ref, onMounted } from 'vue'
import { createUser, listUsers, resetUserPassword, updateUser } from '@/api/users'
import ConfirmDialog from '@/components/common/ConfirmDialog.vue'
import DataTableCard from '@/components/common/DataTableCard.vue'
import InlineError from '@/components/common/InlineError.vue'
import PageHeader from '@/components/common/PageHeader.vue'
import StatusChip from '@/components/common/StatusChip.vue'
import { useDebounce } from '@/composables/useDebounce'
import { useUiStore } from '@/stores/ui'

const ui = useUiStore(); const debounce = useDebounce()
const users = ref([]); const meta = ref(null); const loading = ref(true); const error = ref(''); const search = ref(''); const page = ref(1)
const userDialog = ref(false); const resetDialog = ref(false); const toggleConfirm = ref(false); const editing = ref(false); const selectedUser = ref(null); const saving = ref(false); const dialogError = ref(''); const resetError = ref(''); const fieldErrors = reactive({})
const form = reactive({ name: '', username: '', password: '', password_confirmation: '', is_active: true }); const passwordForm = reactive({ password: '', password_confirmation: '' })
const headers = [{ title: 'اسم الموظف', key: 'name' }, { title: 'اسم المستخدم', key: 'username' }, { title: 'الدور', key: 'role' }, { title: 'تاريخ الإنشاء', key: 'created' }, { title: 'الحالة', key: 'status' }, { title: 'آخر دخول', key: 'last' }, { title: 'الإجراءات', key: 'actions' }]
onMounted(load)
async function load() { loading.value = true; error.value = ''; try { const response = await listUsers({ search: search.value, page: page.value }); users.value = response.data; meta.value = response.meta } catch (apiError) { error.value = apiError.message } finally { loading.value = false } }
function clearFields() { Object.keys(fieldErrors).forEach((key) => delete fieldErrors[key]) }
function openCreate() { editing.value = false; selectedUser.value = null; Object.assign(form, { name: '', username: '', password: '', password_confirmation: '', is_active: true }); dialogError.value = ''; clearFields(); userDialog.value = true }
function openEdit(user) { editing.value = true; selectedUser.value = user; Object.assign(form, { name: user.name, username: user.username, password: '', password_confirmation: '', is_active: user.is_active }); dialogError.value = ''; clearFields(); userDialog.value = true }
function openReset(user) { selectedUser.value = user; Object.assign(passwordForm, { password: '', password_confirmation: '' }); resetError.value = ''; resetDialog.value = true }
function askToggle(user) { selectedUser.value = user; toggleConfirm.value = true }
async function saveUserForm() { saving.value = true; dialogError.value = ''; clearFields(); try { if (editing.value) await updateUser(selectedUser.value.id, form); else await createUser(form); ui.showSnackbar(editing.value ? 'تم تحديث حساب المستخدم.' : 'تم إنشاء حساب موظف الاستقبال.'); userDialog.value = false; await load() } catch (apiError) { dialogError.value = apiError.message; Object.assign(fieldErrors, apiError.errors) } finally { saving.value = false } }
async function savePassword() { saving.value = true; try { await resetUserPassword(selectedUser.value.id, passwordForm); ui.showSnackbar('تمت إعادة تعيين كلمة المرور.'); resetDialog.value = false } catch (apiError) { resetError.value = apiError.message } finally { saving.value = false } }
async function toggleUser() { saving.value = true; try { await updateUser(selectedUser.value.id, { name: selectedUser.value.name, username: selectedUser.value.username, is_active: !selectedUser.value.is_active }); ui.showSnackbar(selectedUser.value.is_active ? 'تم تعطيل الحساب وإلغاء رموزه النشطة.' : 'تمت إعادة تفعيل الحساب.'); toggleConfirm.value = false; await load() } catch (apiError) { ui.showSnackbar(apiError.message, 'error') } finally { saving.value = false } }
function searchChanged() { page.value = 1; debounce(load) } function changePage(value) { page.value = value; load() }
const formatDate = (value) => new Intl.DateTimeFormat('ar-LY-u-nu-latn', { dateStyle: 'medium' }).format(new Date(value)); const formatDateTime = (value) => new Intl.DateTimeFormat('ar-LY-u-nu-latn', { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(value))
</script>

<style scoped>
.user-filter { max-width: 560px; padding: 18px 20px; }.dialog-title { padding: 24px 24px 4px; font-size: 22px; font-weight: 600; }.dialog-content { display: grid; gap: 16px; padding: 12px 24px !important; }.dialog-content p { margin: 0; color: var(--color-ink-muted); }.dialog-actions { gap: 8px; justify-content: flex-end; padding: 12px 24px 24px; }
</style>

