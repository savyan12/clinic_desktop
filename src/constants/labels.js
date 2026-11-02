export const labels = {
  roles: { admin: 'مدير النظام', receptionist: 'موظف استقبال' },
  sessionStatus: { open: 'مفتوحة', closed: 'مغلقة', cancelled: 'ملغاة' },
  sessionType: { individual: 'فردية', subscription: 'اشتراك' },
  subscriptionStatus: { active: 'نشط', completed: 'مكتمل', stopped: 'موقوف', cancelled: 'ملغي' },
  debtStatus: { open: 'مفتوح', partial: 'جزئي', paid: 'مسدد', void: 'ملغى' },
  debtKind: { session: 'جلسة', subscription: 'اشتراك', opening: 'افتتاحي' },
  paymentType: {
    session_payment: 'دفعة جلسة',
    subscription_payment: 'دفعة اشتراك',
    wallet_topup: 'شحن رصيد',
    debt_payment: 'تسديد دين',
    refund: 'استرجاع',
    adjustment: 'تسوية',
  },
  paymentMethod: { cash: 'نقداً', transfer: 'تحويل مصرفي' },
  coverage: {
    subscription: 'اشتراك',
    cash: 'نقداً',
    transfer: 'تحويل',
    wallet: 'رصيد',
    debt: 'دين',
    mixed: 'مختلط',
  },
  gender: { male: 'ذكر', female: 'أنثى' },
  active: { true: 'نشط', false: 'معطل' },
  auditAction: {
    session_created: 'إنشاء جلسة',
    session_closed: 'إغلاق جلسة',
    session_cancelled: 'إلغاء جلسة',
    patient_created: 'إضافة مريض',
    subscription_created: 'إنشاء اشتراك',
    subscription_cancelled: 'إلغاء اشتراك',
    wallet_topped_up: 'شحن رصيد',
    debt_settled: 'تسوية دين',
    doctor_paid: 'صرف مستحقات طبيب',
    service_updated: 'تعديل خدمة',
    user_updated: 'تعديل مستخدم',
  },
}

export function labelFor(group, value) {
  return labels[group]?.[String(value)] ?? value ?? '—'
}

export const paymentMethods = [
  { title: 'نقداً', value: 'cash' },
  { title: 'تحويل مصرفي', value: 'transfer' },
]
