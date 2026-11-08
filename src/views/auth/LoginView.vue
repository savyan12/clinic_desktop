<template>
  <v-card class="login-card" width="100%">
    <v-card-text class="login-content">
      <div class="login-heading">
        <h1>تسجيل الدخول</h1>
        <p>استخدم حسابك المعتمد للوصول إلى مساحة العمل.</p>
      </div>
      <InlineError v-if="error" :message="error" />
      <v-form @submit.prevent="submit">
        <div class="login-fields">
          <v-text-field
            v-model="form.username"
            label="اسم المستخدم"
            autocomplete="username"
            prepend-inner-icon="mdi-account-outline"
            :error-messages="fieldErrors.username"
          />
          <v-text-field
            v-model="form.password"
            label="كلمة المرور"
            :type="showPassword ? 'text' : 'password'"
            autocomplete="current-password"
            prepend-inner-icon="mdi-lock-outline"
            :append-inner-icon="
              showPassword ? 'mdi-eye-off-outline' : 'mdi-eye-outline'
            "
            :error-messages="fieldErrors.password"
            @click:append-inner="showPassword = !showPassword"
          />
          <v-btn
            type="submit"
            color="primary"
            size="large"
            block
            data-form-flow-primary
            :loading="auth.loading"
          >تسجيل الدخول</v-btn>
        </div>
      </v-form>
      <div v-if="isMock" class="mock-credentials">
        <strong>بيانات العرض التجريبي</strong>
        <button type="button" @click="fill('admin')">
          <span>مدير النظام</span><code>admin</code>
        </button>
        <button type="button" @click="fill('reception')">
          <span>موظف الاستقبال</span><code>reception</code>
        </button>
        <small>أي كلمة مرور مقبولة في وضع البيانات التجريبية.</small>
      </div>
    </v-card-text>
  </v-card>
</template>

<script setup>
import { reactive, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import InlineError from "@/components/common/InlineError.vue";
import { useAuthStore } from "@/stores/auth";

const auth = useAuthStore();
const router = useRouter();
const route = useRoute();
const isMock = import.meta.env.VITE_USE_MOCK === "true";
const form = reactive({ username: "", password: "" });
const fieldErrors = reactive({});
const error = ref("");
const showPassword = ref(false);

function fill(username) {
  form.username = username;
  form.password = "demo";
}

async function submit() {
  error.value = "";
  Object.keys(fieldErrors).forEach((key) => delete fieldErrors[key]);
  if (!form.username || !form.password) {
    if (!form.username) fieldErrors.username = ["اسم المستخدم مطلوب."];
    if (!form.password) fieldErrors.password = ["كلمة المرور مطلوبة."];
    return;
  }
  try {
    await auth.login(form);
    router.push(route.query.redirect || "/");
  } catch (apiError) {
    error.value = apiError.message;
    Object.assign(fieldErrors, apiError.errors);
  }
}
</script>

<style scoped>
.login-card {
  border-radius: var(--radius-dialog) !important;
  box-shadow: var(--shadow-raised) !important;
}
.login-content {
  display: grid;
  gap: var(--space-6);
  padding: var(--space-8) !important;
}
.login-heading {
  padding-bottom: var(--space-5);
  border-bottom: 1px solid var(--color-border);
}
.login-content h1 {
  margin: 0;
  color: var(--color-foreground);
  font-size: 26px;
  line-height: 1.35;
}
.login-content p {
  margin: var(--space-2) 0 0;
  color: var(--color-muted-foreground);
  font-size: 14px;
}
.login-fields {
  display: grid;
  gap: var(--space-4);
}
.mock-credentials {
  display: grid;
  gap: var(--space-2);
  padding: var(--space-4);
  background: var(--color-surface-subtle);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
}
.mock-credentials strong {
  font-size: 14px;
  font-weight: 600;
}
.mock-credentials button {
  min-height: 44px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--space-2) var(--space-3);
  color: var(--color-primary);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-sm);
  background: var(--color-surface);
  cursor: pointer;
}
.mock-credentials button:hover { border-color: var(--color-primary); background: var(--color-primary-subtle); }
.mock-credentials code {
  direction: ltr;
  color: var(--color-foreground);
  font-family: inherit;
  font-variant-numeric: tabular-nums;
}
.mock-credentials small {
  color: var(--color-muted-foreground);
  line-height: 1.5;
}
@media (max-width: 520px) {
  .login-content { padding: var(--space-6) var(--space-5) !important; }
}
</style>
