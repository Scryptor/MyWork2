import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { ImportReportView } from '$api/ImportReportView';
import type { ReportGroup } from '$api/ReportGroup';
import { ru } from '#lib/i18n/ru.js';

vi.mock('$app/paths', () => ({ resolve: (p: string) => `/${p}` }));

import ReportView from './ReportViewHarness.test-helper.svelte';

const entry = (key: string, over: Record<string, string> = {}) => ({
	source: 'Catalog',
	key,
	field: '',
	value: '',
	message: '',
	...over
});

const group = (over: Partial<ReportGroup>): ReportGroup => ({
	code: 'date_unparsed',
	severity: 'warning',
	title: 'Не разобрана дата',
	count: 1,
	order_ids: [],
	entries: [],
	entries_count: 0,
	...over
});

const view = (groups: ReportGroup[]): ImportReportView => ({
	id: 2,
	imported_at: '2026-11-19T14:05:09',
	source_path: '/old',
	mode: 'into_empty',
	orders: 11,
	clients: 7,
	users: 4,
	files: { copied: 6, bytes: 2048, missing: 0 },
	groups,
	warnings_text: 'Отчёт',
	files_pending: false
});

describe('ReportView groups', () => {
	it('a group without orders opens into its entries; a cut list says where the rest is', async () => {
		const g = group({
			code: 'client_missing',
			title: 'Клиент не найден — восстановлен из заказа',
			count: 250,
			entries: [
				entry('5', { field: 'ClientId', value: '77', message: 'Клиента нет в ClientsMap.' }),
				entry('6')
			],
			entries_count: 250
		});
		render(ReportView, { report: view([g]), onorder: vi.fn() });
		const toggle = screen.getByRole('button', { name: /^Клиент не найден/ });
		expect(toggle).toHaveAttribute('aria-expanded', 'false');
		await userEvent.click(toggle);
		expect(toggle).toHaveAttribute('aria-expanded', 'true');
		const list = screen.getByRole('list', { name: ru.importReport.entries(g.title) });
		const rows = within(list).getAllByRole('listitem');
		expect(rows).toHaveLength(2);
		expect(rows[0]).toHaveTextContent('Заказ № 5 · Клиент');
		expect(rows[0]).toHaveTextContent('77');
		expect(rows[0]).toHaveTextContent('Клиента нет в ClientsMap.');
		expect(
			screen.getByText('Показаны первые 2 из 250; полный список — в «Сохранить отчёт»')
		).toBeInTheDocument();
	});

	it('a complete list has no «Показаны первые»', async () => {
		const g = group({
			code: 'status_custom',
			count: 1,
			entries: [entry('Свой')],
			entries_count: 1
		});
		render(ReportView, { report: view([g]), onorder: vi.fn() });
		await userEvent.click(screen.getByRole('button', { name: /^Не разобрана дата/ }));
		expect(screen.queryByText(/Показаны первые/)).not.toBeInTheDocument();
	});

	it('a group with orders still opens into numbers, and a number goes to its order', async () => {
		const onorder = vi.fn();
		const g = group({ count: 2, order_ids: [4, 9] });
		render(ReportView, { report: view([g]), onorder });
		await userEvent.click(screen.getByRole('button', { name: /^Не разобрана дата/ }));
		await userEvent.click(screen.getByRole('button', { name: '№ 9' }));
		expect(onorder).toHaveBeenCalledWith(9, [4, 9]);
		expect(screen.queryByRole('list', { name: /^Записи:/ })).not.toBeInTheDocument();
	});

	it('a group with orders and warnings without an order shows both; the cut counts the latter', async () => {
		const g = group({
			count: 253,
			order_ids: [3, 5, 9],
			entries: [
				entry('c0', { source: 'ClientsMap', field: 'Birthday', value: '31.02.1990' }),
				entry('h1', { source: 'HistoryBD', value: 'вчера' })
			],
			entries_count: 250
		});
		render(ReportView, { report: view([g]), onorder: vi.fn() });
		await userEvent.click(screen.getByRole('button', { name: /^Не разобрана дата/ }));
		const numbers = screen.getByRole('list', { name: ru.legacy.groupOrders(g.title) });
		expect(
			within(numbers)
				.getAllByRole('button')
				.map((b) => b.textContent)
		).toEqual(['№ 3', '№ 5', '№ 9']);
		const list = screen.getByRole('list', { name: ru.importReport.entries(g.title) });
		expect(within(list).getAllByRole('listitem')).toHaveLength(2);
		expect(list).toHaveTextContent('31.02.1990');
		expect(screen.getByText(ru.importReport.cut(2, 250))).toBeInTheDocument();
	});

	it('all warnings without an order shown: no cut line, even when the group has more', async () => {
		const g = group({
			count: 4,
			order_ids: [3, 5, 9],
			entries: [entry('c0', { source: 'ClientsMap' })],
			entries_count: 1
		});
		render(ReportView, { report: view([g]), onorder: vi.fn() });
		await userEvent.click(screen.getByRole('button', { name: /^Не разобрана дата/ }));
		expect(
			screen.getByRole('list', { name: ru.importReport.entries(g.title) })
		).toBeInTheDocument();
		expect(screen.queryByText(/Показаны первые/)).not.toBeInTheDocument();
	});

	it('a group with neither orders nor entries has no arrow', () => {
		render(ReportView, { report: view([group({ count: 1 })]), onorder: vi.fn() });
		expect(screen.queryByRole('button', { name: /^Не разобрана дата/ })).not.toBeInTheDocument();
	});

	it('every group has «Что это?» with the text of its code', async () => {
		const g = group({
			code: 'barcode_duplicate',
			title: 'Одинаковый штрихкод у нескольких заказов'
		});
		render(ReportView, { report: view([g]), onorder: vi.fn() });
		await userEvent.click(
			screen.getByRole('button', { name: ru.help.open('Одинаковый штрихкод у нескольких заказов') })
		);
		const dialog = await screen.findByRole('dialog');
		expect(dialog).toHaveTextContent('около 9 тысяч разных кодов');
		expect(dialog).toHaveTextContent('сканер');
	});
});

describe('explanations', () => {
	it('all 44 codes have a text with what was, what the transfer did and what to do', () => {
		const codes = Object.keys(ru.importReport.explain);
		expect(codes).toHaveLength(44);
		for (const code of codes) {
			const text = ru.importReport.explain[code as keyof typeof ru.importReport.explain];
			expect(text.length, code).toBeGreaterThanOrEqual(3);
			for (const p of text) expect(p.trim(), code).not.toBe('');
		}
	});

	it('the dictionaries: a final dot joins values only in list fields; counted are the values left out', () => {
		const text = ru.importReport.explain.dictionary_from_orders.join(' ');
		expect(text).not.toMatch(/регистром или точкой в конце/);
		expect(text).toContain('у пунктов полей-списков — ещё и знаком препинания в конце');
		expect(text).toContain('сколько таких не попало');
	});

	it('users: the screen of users is there; a lost password of the admins is named', () => {
		const all = Object.values(ru.importReport.explain).flat().join(' ');
		expect(all).not.toContain('появится позже');
		expect(ru.importReport.explain.users_manage_granted.join(' ')).toContain('пароль не перенёсся');
		expect(ru.importReport.explain.user_password_invalid.join(' ')).toContain(
			'«Настройки → Пользователи»'
		);
	});
});
