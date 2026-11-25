import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { MoveItem } from '$api/MoveItem';
import type { MovesReport } from '$api/MovesReport';
import MovesReportView from './MovesReport.svelte';

const item = (over: Partial<MoveItem> = {}): MoveItem => ({
	at: '2026-11-24T10:30:00',
	order_id: 7,
	user_name: 'Анна',
	from: 1,
	from_name: 'Центр',
	to: 2,
	to_name: 'Север',
	...over
});
const report = (over: Partial<MovesReport> = {}): MovesReport => ({
	routes: [
		{ from: 1, from_name: 'Центр', to: 2, to_name: 'Север', count: 2 },
		{ from: null, from_name: 'Без точки', to: 1, to_name: 'Центр', count: 1 }
	],
	items: [
		item(),
		item({ order_id: 5, from: null, from_name: 'Без точки', to: 1, to_name: 'Центр' })
	],
	total: 3,
	...over
});
const show = (r: MovesReport, onorder = vi.fn()) => {
	render(MovesReportView, { report: r, onorder });
	return onorder;
};

describe('MovesReport', () => {
	it('the routes table has «Откуда → Куда» and «Сколько раз»', () => {
		show(report());
		const table = screen.getByRole('table', { name: 'Маршруты' });
		const rows = within(table).getAllByRole('row');
		expect(rows).toHaveLength(3);
		expect(within(rows[0]!).getByText('Откуда → Куда')).toBeInTheDocument();
		expect(within(rows[0]!).getByText('Сколько раз')).toBeInTheDocument();
		expect(within(rows[1]!).getByText('Центр → Север')).toBeInTheDocument();
		expect(within(rows[1]!).getByText('2')).toBeInTheDocument();
		expect(within(rows[2]!).getByText('Без точки → Центр')).toBeInTheDocument();
	});

	it('the list has when, the order, from, to and who', async () => {
		const onorder = show(report());
		const table = screen.getByRole('table', { name: 'Перемещения' });
		const head = within(table).getAllByRole('columnheader');
		expect(head.map((h) => h.textContent)).toEqual(['Когда', 'Заказ', 'Откуда', 'Куда', 'Кто']);
		const rows = within(table).getAllByRole('row');
		expect(rows).toHaveLength(3);
		expect(within(rows[1]!).getByText('Север')).toBeInTheDocument();
		expect(within(rows[1]!).getByText('Анна')).toBeInTheDocument();
		await userEvent.click(within(rows[1]!).getByRole('button', { name: '№ 7' }));
		expect(onorder).toHaveBeenCalledWith(7, [7, 5]);
	});

	it('says how many are shown when there are more', () => {
		show(report({ total: 612 }));
		expect(screen.getByText('Показаны последние 2 из 612')).toBeInTheDocument();
	});

	it('does not say it when all are shown', () => {
		show(report({ total: 2 }));
		expect(screen.queryByText(/Показаны последние/)).toBeNull();
	});

	it('an empty report says so and has no table', () => {
		show(report({ routes: [], items: [], total: 0 }));
		expect(
			screen.getByText('За этот период заказы между точками не перемещались')
		).toBeInTheDocument();
		expect(screen.queryByRole('table')).toBeNull();
	});
});
