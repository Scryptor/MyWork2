// Fictional API records for unit tests of src/lib/app and src/lib/orders.
import type { Client } from '$api/Client';
import type { CurrentUser } from '$api/CurrentUser';
import type { Order } from '$api/Order';
import type { OrderListItem } from '$api/OrderListItem';
import type { ServicePoint } from '$api/ServicePoint';
import type { Status } from '$api/Status';

export function item(id: number, over: Partial<OrderListItem> = {}): OrderListItem {
	return {
		id,
		received_at: '2026-11-15T10:24:00',
		issued_at: null,
		client_id: id + 1000,
		client_name: 'ТЕСТОВ ИВАН ПЕТРОВИЧ',
		client_phone: '+79001234567',
		client_blacklisted: false,
		service_point_id: null,
		status_id: 1,
		device_type: 'Смартфон',
		brand: 'Тест',
		model: 'Модель',
		serial: '',
		malfunction: 'Не включается',
		master: '',
		estimate_from: null,
		estimate_to: null,
		prepayment: 0,
		final_price: null,
		need_call: false,
		need_parts: false,
		barcode: '',
		deleted_at: null,
		version: 1,
		...over
	};
}

export function order(id: number, over: Partial<Order> = {}): Order {
	return {
		id,
		client_id: id + 1000,
		service_point_id: null,
		status_id: 1,
		received_at: '2026-11-15T10:24:00',
		issued_at: null,
		prepaid_at: null,
		device_type: 'Смартфон',
		brand: 'Тест',
		model: 'Модель',
		serial: '',
		color: '',
		device_password: '',
		completeness: '',
		device_condition: '',
		malfunction: 'Не включается',
		work_done: '',
		internal_comment: '',
		master: '',
		barcode: '',
		estimate_from: null,
		estimate_to: null,
		final_price: null,
		prepayment: 0,
		extra_costs: 0,
		discount: 0,
		warranty: '',
		warranty_term: null,
		need_call: false,
		need_parts: false,
		created_at: '2026-11-15T10:24:00',
		updated_at: '2026-11-15T10:24:00',
		deleted_at: null,
		version: 1,
		...over
	};
}

export function client(id: number, over: Partial<Client> = {}): Client {
	return {
		id,
		full_name: 'ТЕСТОВА АННА',
		phones: [{ phone: '+79007654321', note: '' }],
		address: '',
		note: '',
		source: '',
		blacklisted: false,
		created_at: '2026-11-15T10:24:00',
		updated_at: '2026-11-15T10:24:00',
		version: 1,
		...over
	};
}

export const STATUSES: Status[] = [
	{ id: 1, name: 'Принят', kind: 'open', color: 'gray', sort_order: 1, archived: false },
	{
		id: 2,
		name: 'Диагностика',
		kind: 'diagnostics',
		color: 'blue',
		sort_order: 2,
		archived: false
	},
	{ id: 3, name: 'Готов', kind: 'ready', color: 'green', sort_order: 3, archived: false },
	{ id: 4, name: 'Выдан', kind: 'issued', color: 'slate', sort_order: 4, archived: false }
];

export const POINTS: ServicePoint[] = [
	{ id: 1, name: 'Центр', sort_order: 1, archived: false },
	{ id: 2, name: 'Склад', sort_order: 2, archived: true }
];

export function user(over: Partial<CurrentUser> = {}): CurrentUser {
	return {
		user_id: 1,
		name: 'Тестовый',
		permissions: ['orders_create', 'orders_edit'],
		open_mode: false,
		...over
	};
}
