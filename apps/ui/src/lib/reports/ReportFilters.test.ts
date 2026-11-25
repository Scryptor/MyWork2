import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ReportView } from './view.js';

// The filters of «Отчёты»: «В Excel» only with «data_export»; typed text rewrites the history entry.
vi.mock('#lib/api/catalogs.js', () => ({ listDictionary: vi.fn().mockResolvedValue([]) }));
vi.mock('#lib/api/masters.js', () => ({ listMasters: vi.fn().mockResolvedValue([]) }));
vi.mock('#lib/api/orders.js', () => ({ orderValues: vi.fn().mockResolvedValue([]) }));
const rights = vi.hoisted(() => ({ export: true }));
vi.mock('#lib/app/context.svelte.js', () => ({
	app: { points: [], can: (p: string) => p !== 'data_export' || rights.export }
}));

import ReportFilters from './ReportFilters.svelte';

const view: ReportView = {
	tab: 'orders',
	period: 'month',
	from: null,
	to: null,
	groupBy: 'master'
};

beforeEach(() => {
	rights.export = true;
});

describe('ReportFilters', () => {
	it('has «В Excel» with the right', () => {
		render(ReportFilters, { view, today: '2026-11-25', onchange: vi.fn(), onexport: vi.fn() });
		expect(screen.getByRole('button', { name: 'В Excel' })).toBeInTheDocument();
	});

	it('has no «В Excel» without the right', () => {
		rights.export = false;
		render(ReportFilters, { view, today: '2026-11-25', onchange: vi.fn(), onexport: vi.fn() });
		expect(screen.queryByRole('button', { name: 'В Excel' })).toBeNull();
	});

	it('applies a typed brand as a replacement of the history entry', async () => {
		const onchange = vi.fn();
		render(ReportFilters, { view, today: '2026-11-25', onchange, onexport: vi.fn() });
		await userEvent.type(screen.getByRole('combobox', { name: 'Бренд' }), 'Acme');
		await userEvent.tab();
		expect(onchange).toHaveBeenCalled();
		for (const call of onchange.mock.calls) expect(call[1]).toBe(true);
	});
});
