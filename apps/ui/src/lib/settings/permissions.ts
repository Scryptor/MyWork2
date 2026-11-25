import type { Permission } from '$api/Permission';
import { ru } from '#lib/i18n/ru.js';

export interface PermissionGroup {
	id: keyof typeof ru.roles.groups;
	label: string;
	permissions: readonly Permission[];
}

/** The rights of a role as checkboxes, in groups; every right of `Permission` is in exactly one. */
export const PERMISSION_GROUPS: readonly PermissionGroup[] = [
	{
		id: 'orders',
		label: ru.roles.groups.orders,
		permissions: ['orders_create', 'orders_edit', 'orders_delete', 'order_dates_edit']
	},
	{
		id: 'clients',
		label: ru.roles.groups.clients,
		permissions: [
			'clients_view',
			'clients_create',
			'clients_edit',
			'clients_delete',
			'clients_merge'
		]
	},
	{
		id: 'stock',
		label: ru.roles.groups.stock,
		permissions: ['stock_view', 'stock_create', 'stock_edit', 'stock_delete']
	},
	{ id: 'reports', label: ru.roles.groups.reports, permissions: ['reports_view', 'data_export'] },
	{
		id: 'settings',
		label: ru.roles.groups.settings,
		permissions: ['settings_manage', 'users_manage', 'audit_view']
	}
];
