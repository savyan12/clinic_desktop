import { createRouter, createWebHistory } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import { useDesktopStore } from '@/stores/desktop'
import AppLayout from '@/layouts/AppLayout.vue'
import AuthLayout from '@/layouts/AuthLayout.vue'
import PrintLayout from '@/layouts/PrintLayout.vue'
import ActivationView from '@/views/activation/ActivationView.vue'
import AuditLogView from '@/views/audit/AuditLogView.vue'
import LoginView from '@/views/auth/LoginView.vue'
import DashboardView from '@/views/dashboard/DashboardView.vue'
import DoctorDetailsView from '@/views/doctors/DoctorDetailsView.vue'
import DoctorsView from '@/views/doctors/DoctorsView.vue'
import ExpensesView from '@/views/expenses/ExpensesView.vue'
import PatientDetailsView from '@/views/patients/PatientDetailsView.vue'
import PatientsView from '@/views/patients/PatientsView.vue'
import PatientStatementPrintView from '@/views/print/PatientStatementPrintView.vue'
import ReportPrintView from '@/views/print/ReportPrintView.vue'
import SessionPrintView from '@/views/print/SessionPrintView.vue'
import SponsorStatementPrintView from '@/views/print/SponsorStatementPrintView.vue'
import ReportsView from '@/views/reports/ReportsView.vue'
import ServicesView from '@/views/services/ServicesView.vue'
import SessionDetailsView from '@/views/sessions/SessionDetailsView.vue'
import SessionsView from '@/views/sessions/SessionsView.vue'
import SettingsView from '@/views/settings/SettingsView.vue'
import SponsorDetailsView from '@/views/sponsors/SponsorDetailsView.vue'
import SponsorsView from '@/views/sponsors/SponsorsView.vue'
import SubscriptionDetailsView from '@/views/subscriptions/SubscriptionDetailsView.vue'
import SubscriptionsView from '@/views/subscriptions/SubscriptionsView.vue'
import UsersView from '@/views/users/UsersView.vue'

const roles = ['admin', 'receptionist']

const router = createRouter({
  history: createWebHistory(),
  scrollBehavior: () => ({ top: 0 }),
  routes: [
    {
      path: '/activation',
      name: 'activation',
      component: ActivationView,
      meta: { title: 'تفعيل النظام', activation: true },
    },
    {
      path: '/login',
      component: AuthLayout,
      meta: { guest: true },
      children: [{ path: '', name: 'login', component: LoginView, meta: { guest: true, title: 'تسجيل الدخول' } }],
    },
    {
      path: '/',
      component: AppLayout,
      meta: { requiresAuth: true },
      children: [
        { path: '', name: 'dashboard', component: DashboardView, meta: { title: 'الرئيسية', roles } },
        { path: 'sessions', name: 'sessions', component: SessionsView, meta: { title: 'الجلسات', roles } },
        { path: 'sessions/:id', name: 'session-details', component: SessionDetailsView, meta: { title: 'تفاصيل الجلسة', roles } },
        { path: 'patients', name: 'patients', component: PatientsView, meta: { title: 'المرضى', roles } },
        { path: 'patients/:id', name: 'patient-details', component: PatientDetailsView, meta: { title: 'ملف المريض', roles } },
        { path: 'subscriptions', name: 'subscriptions', component: SubscriptionsView, meta: { title: 'الاشتراكات', roles } },
        { path: 'subscriptions/:id', name: 'subscription-details', component: SubscriptionDetailsView, meta: { title: 'تفاصيل الاشتراك', roles } },
        { path: 'doctors', name: 'doctors', component: DoctorsView, meta: { title: 'الأطباء ومستحقاتهم', roles: ['admin'] } },
        { path: 'doctors/:id', name: 'doctor-details', component: DoctorDetailsView, meta: { title: 'تفاصيل الطبيب', roles: ['admin'] } },
        { path: 'services', name: 'services', component: ServicesView, meta: { title: 'الخدمات', roles: ['admin'] } },
        { path: 'sponsors', name: 'sponsors', component: SponsorsView, meta: { title: 'الجهات الراعية', roles: ['admin'] } },
        { path: 'sponsors/:id', name: 'sponsor-details', component: SponsorDetailsView, meta: { title: 'تفاصيل الجهة الراعية', roles: ['admin'] } },
        { path: 'expenses', name: 'expenses', component: ExpensesView, meta: { title: 'المشتريات والإيجار', roles: ['admin'] } },
        { path: 'users', name: 'users', component: UsersView, meta: { title: 'المستخدمون', roles: ['admin'] } },
        { path: 'reports/:type?', name: 'reports', component: ReportsView, meta: { title: 'التقارير', roles: ['admin'] } },
        { path: 'audit-log', name: 'audit-log', component: AuditLogView, meta: { title: 'سجل التدقيق', roles: ['admin'] } },
        { path: 'settings', name: 'settings', component: SettingsView, meta: { title: 'إعدادات النظام', roles: ['admin'] } },
      ],
    },
    {
      path: '/print',
      component: PrintLayout,
      meta: { requiresAuth: true },
      children: [
        { path: 'session/:id', component: SessionPrintView, meta: { roles, title: 'طباعة إيصال جلسة', customPrintTemplate: true } },
        { path: 'statement/patient/:id', component: PatientStatementPrintView, meta: { roles, title: 'كشف حساب مريض', customPrintTemplate: true } },
        { path: 'statement/sponsor/:id', component: SponsorStatementPrintView, meta: { roles: ['admin'], title: 'كشف حساب جهة راعية' } },
        { path: 'report/:type', component: ReportPrintView, meta: { roles: ['admin'], title: 'طباعة تقرير', customPrintTemplate: true } },
      ],
    },
    { path: '/:pathMatch(.*)*', redirect: '/' },
  ],
})

router.beforeEach(async (to) => {
  const desktop = useDesktopStore()
  await desktop.initialize()
  if (desktop.isDesktop && (!desktop.activated || !desktop.ready)) {
    if (to.name !== 'activation') return { name: 'activation' }
    return true
  }
  if (to.name === 'activation') return { name: 'login' }
  const auth = useAuthStore()
  if (to.meta.guest && auth.isAuthenticated) return { name: 'dashboard' }
  if (to.matched.some((record) => record.meta.requiresAuth) && !auth.isAuthenticated) return { name: 'login', query: { redirect: to.fullPath } }
  const allowedRoles = to.meta.roles
  if (allowedRoles && !allowedRoles.includes(auth.role)) return { name: 'dashboard', query: { forbidden: '1' } }
  return true
})

router.afterEach((to) => {
  document.title = `${to.meta.title || 'الرئيسية'} | منظومة العيادة`
})

export default router
