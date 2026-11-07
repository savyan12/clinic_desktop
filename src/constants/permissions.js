export const permissions = {
  admin: [
    'sessions.create', 'sessions.editOpen', 'sessions.close', 'sessions.cancelOpen',
    'sessions.editClosed', 'sessions.cancelClosed', 'patients.create', 'subscriptions.create',
    'subscriptions.manage', 'wallet.topup', 'debts.settle', 'refunds.create',
    'masters.manage', 'users.manage', 'payouts.create', 'expenses.manage', 'reports.view', 'audit.view',
    'settings.manage',
  ],
  receptionist: [
    'sessions.create', 'sessions.editOpen', 'sessions.close', 'sessions.cancelOpen',
    'patients.create', 'subscriptions.create', 'wallet.topup', 'debts.settle',
  ],
}
