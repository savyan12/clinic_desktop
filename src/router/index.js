import { createRouter, createWebHistory } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import { useDesktopStore } from '@/stores/desktop'
import { useStartupStore } from '@/stores/startup'
import AppLayout from '@/layouts/AppLayout.vue'
import AuthLayout from '@/layouts/AuthLayout.vue'
import PrintLayout from '@/layouts/PrintLayout.vue'

const ActivationView = () => import('@/views/activation/ActivationView.vue')
const AuditLogView = () => import('@/views/audit/AuditLogView.vue')
const LoginView = () => import('@/views/auth/LoginView.vue')
const DashboardView = () => import('@/views/dashboard/DashboardView.vue')
const DoctorDetailsView = () => import('@/views/doctors/DoctorDetailsView.vue')
const DoctorsView = () => import('@/views/doctors/DoctorsView.vue')
const ExpensesView = () => import('@/views/expenses/ExpensesView.vue')
const PatientDetailsView = () => import('@/views/patients/PatientDetailsView.vue')
const PatientsView = () => import('@/views/patients/PatientsView.vue')
const PatientStatementPrintView = () => import('@/views/print/PatientStatementPrintView.vue')
const ReportPrintView = () => import('@/views/print/ReportPrintView.vue')
const SessionPrintView = () => import('@/views/print/SessionPrintView.vue')
const SponsorStatementPrintView = () => import('@/views/print/SponsorStatementPrintView.vue')
const ReportsView = () => import('@/views/reports/ReportsView.vue')
const ServicesView = () => import('@/views/services/ServicesView.vue')
const SessionDetailsView = () => import('@/views/sessions/SessionDetailsView.vue')
const SessionsView = () => import('@/views/sessions/SessionsView.vue')
const SettingsView = () => import('@/views/settings/SettingsView.vue')
const SponsorDetailsView = () => import('@/views/sponsors/SponsorDetailsView.vue')
const SponsorsView = () => import('@/views/sponsors/SponsorsView.vue')
const SubscriptionDetailsView = () => import('@/views/subscriptions/SubscriptionDetailsView.vue')
const SubscriptionsView = () => import('@/views/subscriptions/SubscriptionsView.vue')
const UsersView = () => import('@/views/users/UsersView.vue')

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
  const startup = useStartupStore()
  if (!await startup.initialize()) return false
  const desktop = useDesktopStore()
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
