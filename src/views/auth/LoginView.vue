<template>
  <v-card class="login-card" width="100%" max-width="460">
    <v-card-text class="login-content">
      <div>
        <h1>تسجيل الدخول</h1>
        <p>أدخل بيانات حسابك للوصول إلى منظومة العيادة.</p>
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
            :loading="auth.loading"
          >دخول</v-btn>
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
  border-radius: 18px !important;
}
.login-content {
  display: grid;
  gap: 24px;
  padding: 32px !important;
}
.login-content h1 {
  margin: 0;
  font-size: 30px;
}
.login-content p {
  margin: 6px 0 0;
  color: var(--color-ink-muted);
}
.login-fields {
  display: grid;
  gap: 16px;
}
.mock-credentials {
  display: grid;
  gap: 8px;
  padding: 16px;
  border-radius: 12px;
  background: var(--color-parchment);
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
  padding: 8px 12px;
  color: var(--color-primary);
  border: 1px solid var(--color-hairline);
  border-radius: 8px;
  background: var(--color-canvas);
  cursor: pointer;
}
.mock-credentials code {
  direction: ltr;
  color: var(--color-ink);
}
.mock-credentials small {
  color: var(--color-ink-muted);
}
</style>
