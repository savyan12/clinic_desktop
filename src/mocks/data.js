const pad = (value) => String(value).padStart(2, "0");

export function isoDate(offset = 0) {
  const date = new Date();
  date.setDate(date.getDate() + offset);
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

export function isoDateTime(offset = 0, hour = 9, minute = 0) {
  return `${isoDate(offset)}T${pad(hour)}:${pad(minute)}:00+02:00`;
}

export const db = {
  settings: {
    clinic_name: "عيادة عين الحياة",
    clinic_subtitle: "للعلاج الطبيعي والتأهيل",
    phone: "091-555-2026",
    address: "طرابلس، ليبيا",
    receipt_footer: "نتمنى لكم دوام الصحة والعافية",
    currency: "د.ل",
    print_size: "A4",
    show_logo: true,
  },
  users: [
    {
      id: 1,
      name: "سالم الورفلي",
      username: "admin",
      role: "admin",
      is_active: true,
      created_at: "2025-01-05T09:00:00+02:00",
      last_login_at: isoDateTime(0, 7, 45),
    },
    {
      id: 2,
      name: "سارة الرويفي",
      username: "reception",
      role: "receptionist",
      is_active: true,
      created_at: "2025-02-12T10:30:00+02:00",
      last_login_at: isoDateTime(0, 8, 5),
    },
    {
      id: 3,
      name: "أسماء الترهوني",
      username: "asmaa",
      role: "receptionist",
      is_active: false,
      created_at: "2025-06-18T11:00:00+02:00",
      last_login_at: isoDateTime(-18, 16, 20),
    },
  ],
  doctors: [
    {
      id: 1,
      name: "د. أحمد الشريف",
      phone: "0912345678",
      commission_rate: "30.000",
      is_active: true,
      sessions_count: 42,
      earned: "1890.000",
      paid: "1200.000",
      remaining: "690.000",
    },
    {
      id: 2,
      name: "د. مريم سالم",
      phone: "0923456781",
      commission_rate: "25.000",
      is_active: true,
      sessions_count: 35,
      earned: "1325.000",
      paid: "900.000",
      remaining: "425.000",
    },
    {
      id: 3,
      name: "د. خالد الفيتوري",
      phone: "0942211098",
      commission_rate: "28.000",
      is_active: true,
      sessions_count: 27,
      earned: "1044.000",
      paid: "700.000",
      remaining: "344.000",
    },
  ],
  services: [
    {
      id: 1,
      name: "علاج طبيعي",
      price: "80.000",
      is_active: true,
      updated_at: isoDateTime(-12, 10, 0),
    },
    {
      id: 2,
      name: "تمارين علاجية",
      price: "45.000",
      is_active: true,
      updated_at: isoDateTime(-30, 11, 0),
    },
    {
      id: 3,
      name: "علاج يدوي",
      price: "65.000",
      is_active: true,
      updated_at: isoDateTime(-8, 12, 0),
    },
    {
      id: 4,
      name: "تأهيل إصابات رياضية",
      price: "95.000",
      is_active: true,
      updated_at: isoDateTime(-45, 9, 0),
    },
    {
      id: 5,
      name: "جلسة أطفال",
      price: "70.000",
      is_active: true,
      updated_at: isoDateTime(-21, 13, 0),
    },
    {
      id: 6,
      name: "علاج كهربائي",
      price: "35.000",
      is_active: true,
      updated_at: isoDateTime(-17, 15, 0),
    },
    {
      id: 7,
      name: "تأهيل ما بعد الجراحة",
      price: "110.000",
      is_active: true,
      updated_at: isoDateTime(-60, 10, 0),
    },
    {
      id: 8,
      name: "تقييم أولي",
      price: "50.000",
      is_active: false,
      updated_at: isoDateTime(-4, 14, 0),
    },
  ],
  sponsors: [
    {
      id: 1,
      name: "شركة المدار الجديد",
      phone: "0213400000",
      address: "طرابلس",
      is_active: true,
      patients_count: 6,
      total_due: "3820.000",
      total_paid: "2400.000",
      remaining: "1420.000",
    },
    {
      id: 2,
      name: "صندوق الضمان الاجتماعي",
      phone: "0213602211",
      address: "طرابلس",
      is_active: true,
      patients_count: 4,
      total_due: "2650.000",
      total_paid: "1800.000",
      remaining: "850.000",
    },
    {
      id: 3,
      name: "شركة ليبيا للتأمين",
      phone: "0214448010",
      address: "بن عاشور",
      is_active: true,
      patients_count: 3,
      total_due: "1950.000",
      total_paid: "1550.000",
      remaining: "400.000",
    },
  ],
  patients: [],
  sessions: [],
  subscriptions: [],
  walletTransactions: [],
  debts: [],
  payments: [],
  paymentAllocations: [],
  payouts: [],
  auditLogs: [],
};

const patientSeed = [
  ["علي المصراتي", "0912345678", 34, "male", "حي الأندلس", 1, "120.000"],
  ["فاطمة العبيدي", "0927654321", 29, "female", "بن عاشور", null, "40.000"],
  ["هدى الترهوني", "0913334060", 41, "female", "قرقارش", 2, "0.000"],
  [
    "خالد بن عمران",
    "0915557820",
    37,
    "male",
    "زاوية الدهماني",
    null,
    "210.000",
  ],
  ["نجلاء الفيتوري", "0924102200", 26, "female", "الفرناج", 3, "25.000"],
  ["عمر الزوي", "0926603120", 45, "male", "سوق الجمعة", null, "0.000"],
  ["مريم السنوسي", "0914801710", 52, "female", "تاجوراء", 1, "60.000"],
  ["أحمد القماطي", "0921123080", 31, "male", "جنزور", null, "85.000"],
  ["سلمى الدرسي", "0918706630", 24, "female", "الظهرة", 2, "0.000"],
  ["يوسف المقريف", "0943309122", 38, "male", "عين زارة", null, "150.000"],
  ["آمنة الزليطني", "0916084112", 33, "female", "صلاح الدين", 3, "0.000"],
  ["محمد بوشناف", "0929041011", 56, "male", "الهضبة", null, "95.000"],
  ["نور الهادي", "0917022188", 19, "female", "غوط الشعال", null, "30.000"],
  ["إبراهيم العكاري", "0923347100", 47, "male", "أبو سليم", 1, "0.000"],
  ["ريم البرعصي", "0941087122", 36, "female", "الدريبي", null, "175.000"],
  ["محمود الغرياني", "0912445890", 63, "male", "طريق المطار", 2, "0.000"],
  ["سندس الككلي", "0927788114", 28, "female", "السراج", null, "55.000"],
  ["طارق بن ناصر", "0919982201", 42, "male", "فشلوم", 3, "0.000"],
  ["رؤى التاجوري", "0945003311", 22, "female", "تاجوراء", null, "100.000"],
  ["أنس المجدوب", "0926517700", 35, "male", "زاوية الدهماني", 1, "20.000"],
];

db.patients = patientSeed.map((item, index) => ({
  id: index + 1,
  name: item[0],
  phone: item[1],
  age: item[2],
  gender: item[3],
  area: item[4],
  emergency_phone: index % 3 === 0 ? `091700${pad(index)}11` : null,
  sponsor_id: item[5],
  sponsor: item[5]
    ? db.sponsors.find((sponsor) => sponsor.id === item[5])
    : null,
  wallet_balance: item[6],
  debt_total:
    index % 4 === 0 ? "160.000" : index % 5 === 0 ? "75.000" : "0.000",
  subscription_status: index < 8 ? "active" : index < 11 ? "completed" : null,
  is_active: true,
  created_at: isoDateTime(-(index + 10), 10, 0),
}));

const serviceSnapshot = (ids) =>
  ids.map((id) => {
    const service = db.services.find((entry) => entry.id === id);
    return { service_id: service.id, name: service.name, price: service.price };
  });

db.subscriptions = [
  {
    id: 1,
    patient_id: 1,
    service_ids: [1, 2],
    services: serviceSnapshot([1, 2]),
    sessions_count: 10,
    sessions_used: 8,
    remaining: 2,
    base_total: "1250.000",
    discount: "100.000",
    net_total: "1150.000",
    paid_amount: "800.000",
    remaining_amount: "350.000",
    start_date: isoDate(-45),
    expected_end_date: isoDate(5),
    status: "active",
    sponsor_id: 1,
  },
  {
    id: 2,
    patient_id: 2,
    service_ids: [1],
    services: serviceSnapshot([1]),
    sessions_count: 8,
    sessions_used: 7,
    remaining: 1,
    base_total: "640.000",
    discount: "40.000",
    net_total: "600.000",
    paid_amount: "600.000",
    remaining_amount: "0.000",
    start_date: isoDate(-40),
    expected_end_date: isoDate(4),
    status: "active",
    sponsor_id: null,
  },
  {
    id: 3,
    patient_id: 4,
    service_ids: [3, 6],
    services: serviceSnapshot([3, 6]),
    sessions_count: 12,
    sessions_used: 4,
    remaining: 8,
    base_total: "1200.000",
    discount: "0.000",
    net_total: "1200.000",
    paid_amount: "1200.000",
    remaining_amount: "0.000",
    start_date: isoDate(-25),
    expected_end_date: isoDate(25),
    status: "active",
    sponsor_id: null,
  },
  {
    id: 4,
    patient_id: 5,
    service_ids: [4],
    services: serviceSnapshot([4]),
    sessions_count: 6,
    sessions_used: 6,
    remaining: 0,
    base_total: "570.000",
    discount: "20.000",
    net_total: "550.000",
    paid_amount: "300.000",
    remaining_amount: "250.000",
    start_date: isoDate(-70),
    expected_end_date: isoDate(-10),
    status: "completed",
    sponsor_id: 3,
  },
  {
    id: 5,
    patient_id: 7,
    service_ids: [1, 3],
    services: serviceSnapshot([1, 3]),
    sessions_count: 10,
    sessions_used: 3,
    remaining: 7,
    base_total: "1450.000",
    discount: "150.000",
    net_total: "1300.000",
    paid_amount: "650.000",
    remaining_amount: "650.000",
    start_date: isoDate(-22),
    expected_end_date: isoDate(35),
    status: "stopped",
    sponsor_id: 1,
  },
  {
    id: 6,
    patient_id: 8,
    service_ids: [2],
    services: serviceSnapshot([2]),
    sessions_count: 5,
    sessions_used: 3,
    remaining: 2,
    base_total: "225.000",
    discount: "0.000",
    net_total: "225.000",
    paid_amount: "225.000",
    remaining_amount: "0.000",
    start_date: isoDate(-18),
    expected_end_date: isoDate(8),
    status: "active",
    sponsor_id: null,
  },
];

const sessionSeed = [
  [
    1,
    1,
    [1, 2],
    "individual",
    null,
    "open",
    8,
    30,
    "cash",
    "125.000",
    "5.000",
    "120.000",
  ],
  [
    2,
    2,
    [1],
    "subscription",
    2,
    "open",
    9,
    5,
    "subscription",
    "80.000",
    "5.000",
    "75.000",
  ],
  [
    3,
    3,
    [3, 6],
    "individual",
    null,
    "open",
    9,
    40,
    "debt",
    "100.000",
    "0.000",
    "100.000",
  ],
  [
    4,
    1,
    [3],
    "individual",
    null,
    "open",
    10,
    15,
    "wallet",
    "65.000",
    "0.000",
    "65.000",
  ],
  [
    5,
    2,
    [4],
    "individual",
    null,
    "closed",
    11,
    0,
    "transfer",
    "95.000",
    "0.000",
    "95.000",
  ],
  [
    6,
    3,
    [2, 6],
    "individual",
    null,
    "closed",
    11,
    40,
    "mixed",
    "80.000",
    "0.000",
    "80.000",
  ],
  [
    7,
    1,
    [1, 2],
    "subscription",
    1,
    "closed",
    12,
    15,
    "subscription",
    "115.000",
    "0.000",
    "115.000",
  ],
  [
    8,
    2,
    [1],
    "individual",
    null,
    "cancelled",
    13,
    0,
    "cash",
    "80.000",
    "0.000",
    "80.000",
  ],
];

db.sessions = sessionSeed.map((item, index) => ({
  id: index + 1,
  code: `S-${String(index + 1001).padStart(4, "0")}`,
  patient_id: item[0],
  doctor_id: item[1],
  services: serviceSnapshot(item[2]),
  service_ids: item[2],
  type: item[3],
  subscription_id: item[4],
  status: item[5],
  opened_at: isoDateTime(index < 6 ? 0 : -1, item[6], item[7]),
  closed_at:
    item[5] === "closed"
      ? isoDateTime(index < 6 ? 0 : -1, item[6] + 1, item[7])
      : null,
  cancelled_at: item[5] === "cancelled" ? isoDateTime(-1, 13, 20) : null,
  services_total: item[9],
  discount: item[10],
  net_total: item[11],
  settlement: {
    coverage: item[8],
    wallet_amount:
      item[8] === "wallet"
        ? item[11]
        : item[8] === "mixed"
          ? "20.000"
          : "0.000",
    paid_amount: ["cash", "transfer"].includes(item[8])
      ? item[11]
      : item[8] === "mixed"
        ? "30.000"
        : "0.000",
    debt_amount:
      item[8] === "debt" ? item[11] : item[8] === "mixed" ? "30.000" : "0.000",
    method: item[8] === "transfer" ? "transfer" : "cash",
    sponsor_id: item[8] === "debt" ? 2 : null,
  },
  created_by: 2,
  closed_by: item[5] === "closed" ? 2 : null,
  cancel_reason: item[5] === "cancelled" ? "أُلغي بناءً على طلب المريض" : null,
  commission_rate:
    item[5] === "closed"
      ? db.doctors.find((doctor) => doctor.id === item[1]).commission_rate
      : null,
  commission_amount:
    item[5] === "closed"
      ? (
          (Number(item[11]) *
            Number(
              db.doctors.find((doctor) => doctor.id === item[1])
                .commission_rate,
            )) /
          100
        ).toFixed(3)
      : null,
}));

db.walletTransactions = [
  {
    id: 1,
    patient_id: 1,
    type: "topup",
    amount: "200.000",
    balance_after: "200.000",
    created_at: isoDateTime(-15, 10, 0),
    created_by: 2,
  },
  {
    id: 2,
    patient_id: 1,
    type: "debit",
    amount: "-80.000",
    balance_after: "120.000",
    session_id: 31,
    created_at: isoDateTime(-6, 11, 0),
    created_by: 2,
  },
  {
    id: 3,
    patient_id: 4,
    type: "topup",
    amount: "210.000",
    balance_after: "210.000",
    created_at: isoDateTime(-2, 9, 30),
    created_by: 2,
  },
];

db.debts = [
  {
    id: 1,
    patient_id: 1,
    sponsor_id: 1,
    session_id: 23,
    subscription_id: null,
    kind: "session",
    amount: "240.000",
    paid_amount: "80.000",
    remaining: "160.000",
    status: "partial",
    created_at: isoDateTime(-28, 12, 0),
  },
  {
    id: 2,
    patient_id: 5,
    sponsor_id: 3,
    session_id: null,
    subscription_id: 4,
    kind: "subscription",
    amount: "250.000",
    paid_amount: "0.000",
    remaining: "250.000",
    status: "open",
    created_at: isoDateTime(-70, 10, 0),
  },
  {
    id: 3,
    patient_id: 6,
    sponsor_id: null,
    session_id: 19,
    subscription_id: null,
    kind: "session",
    amount: "75.000",
    paid_amount: "0.000",
    remaining: "75.000",
    status: "open",
    created_at: isoDateTime(-12, 15, 0),
  },
  {
    id: 4,
    patient_id: 11,
    sponsor_id: 3,
    session_id: null,
    subscription_id: null,
    kind: "opening",
    amount: "400.000",
    paid_amount: "100.000",
    remaining: "300.000",
    status: "partial",
    created_at: isoDateTime(-90, 9, 0),
  },
];

db.payments = [
  {
    id: 1,
    patient_id: 1,
    type: "wallet_topup",
    method: "cash",
    amount: "200.000",
    created_at: isoDateTime(-15, 10, 0),
    created_by: 2,
  },
  {
    id: 2,
    patient_id: 5,
    type: "session_payment",
    method: "transfer",
    amount: "95.000",
    session_id: 5,
    created_at: isoDateTime(0, 12, 0),
    created_by: 2,
  },
  {
    id: 3,
    patient_id: 1,
    type: "debt_payment",
    method: "cash",
    amount: "80.000",
    created_at: isoDateTime(-10, 13, 0),
    created_by: 2,
  },
  {
    id: 4,
    patient_id: 2,
    type: "subscription_payment",
    method: "cash",
    amount: "600.000",
    subscription_id: 2,
    created_at: isoDateTime(-40, 10, 0),
    created_by: 2,
  },
  {
    id: 5,
    patient_id: 8,
    type: "refund",
    method: "cash",
    amount: "-45.000",
    created_at: isoDateTime(-4, 16, 0),
    created_by: 1,
    note: "استرجاع يدوي معتمد",
  },
];

db.paymentAllocations = [
  { id: 1, payment_id: 3, debt_id: 1, amount: "80.000" },
];

db.payouts = [
  {
    id: 1,
    doctor_id: 1,
    amount: "600.000",
    date: isoDate(-35),
    method: "cash",
    note: "دفعة شهرية",
    created_by: 1,
  },
  {
    id: 2,
    doctor_id: 1,
    amount: "600.000",
    date: isoDate(-5),
    method: "transfer",
    note: "تحويل مصرفي",
    created_by: 1,
  },
  {
    id: 3,
    doctor_id: 2,
    amount: "900.000",
    date: isoDate(-7),
    method: "transfer",
    note: "مستحقات الفترة",
    created_by: 1,
  },
];

db.auditLogs = [
  {
    id: 1,
    user_id: 2,
    action: "session_created",
    entity_type: "session",
    entity_id: 1,
    old_values: null,
    new_values: { status: "open", net_total: "120.000" },
    reason: null,
    created_at: isoDateTime(0, 8, 30),
  },
  {
    id: 2,
    user_id: 2,
    action: "session_closed",
    entity_type: "session",
    entity_id: 5,
    old_values: { status: "open" },
    new_values: { status: "closed", net_total: "95.000" },
    reason: null,
    created_at: isoDateTime(0, 12, 0),
  },
  {
    id: 3,
    user_id: 1,
    action: "service_updated",
    entity_type: "service",
    entity_id: 1,
    old_values: { price: "75.000" },
    new_values: { price: "80.000" },
    reason: "تحديث قائمة الأسعار",
    created_at: isoDateTime(-12, 10, 0),
  },
  {
    id: 4,
    user_id: 2,
    action: "wallet_topped_up",
    entity_type: "patient",
    entity_id: 1,
    old_values: { wallet_balance: "0.000" },
    new_values: { wallet_balance: "200.000" },
    reason: null,
    created_at: isoDateTime(-15, 10, 0),
  },
  {
    id: 5,
    user_id: 1,
    action: "doctor_paid",
    entity_type: "doctor",
    entity_id: 1,
    old_values: { remaining: "1290.000" },
    new_values: { remaining: "690.000" },
    reason: "صرف دفعة شهرية",
    created_at: isoDateTime(-5, 13, 0),
  },
];

export function nextId(collection) {
  return Math.max(0, ...collection.map((item) => item.id)) + 1;
}

export function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

export function hydratePatient(patient) {
  return {
    ...patient,
    sponsor: patient.sponsor_id
      ? db.sponsors.find((item) => item.id === patient.sponsor_id) || null
      : null,
  };
}

export function hydrateSession(session) {
  return {
    ...session,
    patient: hydratePatient(
      db.patients.find((item) => item.id === session.patient_id),
    ),
    doctor: db.doctors.find((item) => item.id === session.doctor_id),
    subscription: session.subscription_id
      ? db.subscriptions.find((item) => item.id === session.subscription_id)
      : null,
    created_by_user: db.users.find((item) => item.id === session.created_by),
  };
}

export function hydrateSubscription(subscription) {
  return {
    ...subscription,
    patient: hydratePatient(
      db.patients.find((item) => item.id === subscription.patient_id),
    ),
    sponsor: subscription.sponsor_id
      ? db.sponsors.find((item) => item.id === subscription.sponsor_id)
      : null,
  };
}
