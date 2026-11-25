import { render, screen, within } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import type { OrderReport } from '$api/OrderReport';
import type { ReportTotals } from '$api/ReportTotals';
import { formatMoney as format } from '#lib/format/index.js';
import OrdersReport from './OrdersReportHarness.test-helper.svelte';

vi.mock('#lib/ui/Chart.svelte', async () => ({
	default: (await import('./ChartStub.test-helper.svelte')).default
}));

/** As the screen reads it: the testing library folds the non-breaking spaces of the text it finds. */
const formatMoney = (kopecks: number) => format(kopecks).replace(/\s+/g, ' ');
const rub = (r: number) => r * 100;
const totals = (over: Partial<ReportTotals> = {}): ReportTotals => ({
	orders: 2,
	without_repair: 1,
	revenue: rub(1400),
	discounts: rub(100),
	costs: rub(300),
	profit: rub(1100),
	master_pay: rub(550),
	...over
});
const report = (over: Partial<OrderReport> = {}): OrderReport => ({
	totals: totals(),
	groups: [
		{ key: '1', label: 'Антон', totals: totals({ orders: 1, without_repair: 0 }) },
		{ key: '', label: 'Без мастера', totals: totals({ orders: 1, revenue: rub(500) }) }
	],
	step: 'day',
	timeline: [
		{ start: '2026-11-24', totals: totals() },
		{ start: '2026-11-25', totals: totals({ orders: 0 }) }
	],
	master_percent: 50,
	...over
});

const show = (r: OrderReport, groupBy = 'master' as const) =>
	render(OrdersReport, { report: r, groupBy });

describe('OrdersReport', () => {
	it('the tiles show the sums as money with their notes', () => {
		show(report());
		const tiles = screen.getByRole('group', { name: 'Итоги' });
		expect(within(tiles).getByText(formatMoney(rub(1400)))).toBeInTheDocument();
		expect(within(tiles).getByText(`скидки: ${formatMoney(rub(100))}`)).toBeInTheDocument();
		expect(within(tiles).getByText(formatMoney(rub(300)))).toBeInTheDocument();
		expect(within(tiles).getByText('расходы и запчасти')).toBeInTheDocument();
		expect(within(tiles).getByText(formatMoney(rub(1100)))).toBeInTheDocument();
		expect(
			within(tiles).getByText(`в среднем ${formatMoney(rub(550))} с заказа`)
		).toBeInTheDocument();
		expect(within(tiles).getByText('без ремонта: 1')).toBeInTheDocument();
		expect(within(tiles).getByText('50 % прибыли')).toBeInTheDocument();
	});

	it('the scrolling table is a named region the keyboard can reach', () => {
		show(report());
		const region = screen.getByRole('region', { name: /Итоги по группам/ });
		expect(region).toHaveAttribute('tabindex', '0');
		expect(within(region).getByRole('table', { name: 'Итоги по группам' })).toBeInTheDocument();
	});

	it('the table has the groups and the last row «Итого»', () => {
		show(report());
		const rows = within(screen.getByRole('table', { name: 'Итоги по группам' })).getAllByRole(
			'row'
		);
		// The header, two groups, the total.
		expect(rows).toHaveLength(4);
		expect(within(rows[1]!).getByText('Антон')).toBeInTheDocument();
		expect(within(rows[2]!).getByText('Без мастера')).toBeInTheDocument();
		expect(within(rows[3]!).getByText('Итого')).toBeInTheDocument();
		expect(within(rows[3]!).getByText(formatMoney(rub(1400)))).toBeInTheDocument();
	});

	it('an empty report says so and has no table', () => {
		show(
			report({
				totals: totals({
					orders: 0,
					without_repair: 0,
					revenue: 0,
					discounts: 0,
					costs: 0,
					profit: 0,
					master_pay: 0
				}),
				groups: [],
				timeline: []
			})
		);
		expect(screen.getByText('За этот период выданных заказов нет')).toBeInTheDocument();
		expect(screen.queryByRole('table')).toBeNull();
	});

	it('twelve groups make eleven bars: ten and «Остальные»', () => {
		const groups = Array.from({ length: 12 }, (_, i) => ({
			key: String(i),
			label: `Мастер ${i + 1}`,
			totals: totals({ orders: 1 })
		}));
		show(report({ groups }));
		const chart = screen.getByRole('img', { name: 'По мастерам' });
		const categories = chart.dataset.categories!.split('|');
		expect(categories).toHaveLength(11);
		expect(categories[0]).toBe('Мастер 1');
		expect(categories[10]).toBe('Остальные');
		// The table keeps all twelve, and the total.
		expect(
			within(screen.getByRole('table', { name: 'Итоги по группам' })).getAllByRole('row')
		).toHaveLength(14);
	});

	it('the time chart has the labels of days', () => {
		show(report());
		expect(screen.getByRole('img', { name: 'По времени' }).dataset.categories).toBe('24.11|25.11');
	});

	it('the time chart of months has «ноя 2026»', () => {
		show(report({ step: 'month', timeline: [{ start: '2026-11-01', totals: totals() }] }));
		expect(screen.getByRole('img', { name: 'По времени' }).dataset.categories).toBe('ноя 2026');
	});
});
