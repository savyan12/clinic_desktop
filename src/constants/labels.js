export const labels = {
  roles: { admin: 'مدير النظام', receptionist: 'موظف استقبال' },
  sessionStatus: { open: 'مفتوحة', closed: 'مغلقة', cancelled: 'ملغاة' },
  sessionType: { individual: 'فردية', subscription: 'اشتراك' },
  subscriptionStatus: { active: 'نشط', completed: 'مكتمل', cancelled: 'ملغي' },
  debtStatus: { open: 'مفتوح', partial: 'جزئي', paid: 'مسدد', void: 'ملغى' },
  debtKind: { session: 'جلسة', subscription: 'اشتراك', opening: 'افتتاحي' },
  expenseType: { purchase: 'مشتريات', rent: 'إيجار' },
  expenseStatus: { active: 'نشط', cancelled: 'ملغي' },
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

export const expenseTypes = [
  { title: 'مشتريات', value: 'purchase' },
  { title: 'إيجار', value: 'rent' },
]
