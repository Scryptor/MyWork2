import { render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

vi.mock('$app/paths', () => ({ resolve: (p: string) => `/${p}` }));

import LegacyStockNotice from './LegacyStockNotice.svelte';

// «Склад» of a base imported before the stock module: the stock of MyWork 2 waits on «Импорт», and the first
// position added here closes its transfer for good (decision 17).
describe('LegacyStockNotice', () => {
	it('the one who can transfer gets the way to it', () => {
		render(LegacyStockNotice, { canMigrate: true });
		expect(screen.getByText('Склад MyWork 2 ещё не перенесён')).toBeInTheDocument();
		expect(screen.getByText(/Перенесите его до того, как добавлять позиции/)).toBeInTheDocument();
		expect(screen.getByRole('link', { name: 'Перейти к переносу' })).toHaveAttribute(
			'href',
			'/settings/import'
		);
		expect(screen.queryByText(/может администратор/)).not.toBeInTheDocument();
	});

	it('the others are told who can do it, without the way', () => {
		render(LegacyStockNotice, { canMigrate: false });
		expect(screen.getByText('Склад MyWork 2 ещё не перенесён')).toBeInTheDocument();
		expect(screen.getByText(/Не добавляйте позиции, пока склад не перенесён/)).toBeInTheDocument();
		expect(screen.getByText(/может администратор/)).toBeInTheDocument();
		expect(screen.queryByRole('link')).not.toBeInTheDocument();
	});
});
