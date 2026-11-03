export const labels = {
  roles: { admin: 'مدير النظام', receptionist: 'موظف استقبال' },
  sessionStatus: { open: 'مفتوحة', closed: 'مغلقة', cancelled: 'ملغاة' },
  sessionType: { individual: 'فردية', subscription: 'اشتراك' },
  subscriptionStatus: { active: 'نشط', completed: 'مكتمل', stopped: 'موقوف', cancelled: 'ملغي' },
  subscriptionPlan: { monthly: 'شهري', legacy_package: 'حزمة جلسات' },
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
}

export function labelFor(group, value) {
  return labels[group]?.[String(value)] ?? value ?? '—'
}

export const paymentMethods = [
  { title: 'نقداً', value: 'cash' },
  { title: 'تحويل مصرفي', value: 'transfer' },
]
